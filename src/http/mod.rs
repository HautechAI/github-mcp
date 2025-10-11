use crate::config::Config;
// Re-export canonical RateMeta from shared types to avoid duplication and unify JSON shape.
pub use crate::types::RateMeta;
use base64::Engine; // for URL_SAFE_NO_PAD.encode/decode
use log::warn;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, RETRY_AFTER, USER_AGENT};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorInfo {
    pub code: String,
    pub message: String,
    pub retriable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Meta {
    pub rate: Option<RateMeta>,
}

#[derive(Debug, Clone)]
pub struct RestResponse<T> {
    pub value: Option<T>,
    pub meta: Meta,
    pub error: Option<ErrorInfo>,
    pub status: StatusCode,
    pub headers: Option<HeaderMap>,
}

pub fn build_client(cfg: &Config) -> reqwest::Result<Client> {
    let mut default_headers = HeaderMap::new();
    default_headers.insert(USER_AGENT, HeaderValue::from_str(&cfg.user_agent).unwrap());
    // Authorization header is injected per request to allow token rotation later.
    let builder = Client::builder()
        .default_headers(default_headers)
        .timeout(Duration::from_secs(cfg.timeout_secs))
        // Handle redirects explicitly in callers (e.g., logs ZIP).
        .redirect(reqwest::redirect::Policy::none())
        .use_rustls_tls();
    builder.build()
}

fn auth_header(token: &str) -> HeaderValue {
    HeaderValue::from_str(&format!("Bearer {}", token)).expect("valid header")
}

pub fn map_status_to_error(status: StatusCode, message: String) -> ErrorInfo {
    let (code, retriable) = match status {
        StatusCode::BAD_REQUEST => ("bad_request", false),
        StatusCode::UNAUTHORIZED => ("unauthorized", false),
        StatusCode::FORBIDDEN => ("forbidden", false),
        StatusCode::NOT_FOUND => ("not_found", false),
        StatusCode::CONFLICT => ("conflict", false),
        StatusCode::TOO_MANY_REQUESTS => ("rate_limited", true),
        s if s.is_server_error() => ("upstream_error", true),
        _ => ("server_error", false),
    };
    ErrorInfo {
        code: code.to_string(),
        message,
        retriable,
    }
}

pub fn extract_rate_from_rest(headers: &HeaderMap) -> RateMeta {
    let remaining = headers
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i32>().ok());
    let used = headers
        .get("x-ratelimit-used")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i32>().ok());
    let reset_at = headers
        .get("x-ratelimit-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .map(|epoch| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(epoch, 0)
                .unwrap()
                .to_rfc3339()
        });
    RateMeta {
        remaining,
        used,
        reset_at,
    }
}

// Percent-encode a single URL path segment per RFC3986 (encode all but unreserved)
// Unreserved: ALPHA / DIGIT / "-" / "." / "_" / "~"
pub fn encode_path_segment(s: &str) -> String {
    fn is_unreserved(b: u8) -> bool {
        matches!(b,
            b'A'..=b'Z' |
            b'a'..=b'z' |
            b'0'..=b'9' |
            b'-' | b'.' | b'_' | b'~'
        )
    }
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if is_unreserved(b) {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{:02X}", b));
        }
    }
    out
}

fn compute_backoff(attempt: u32, retry_after: Option<Duration>) -> Duration {
    if let Some(d) = retry_after {
        return d;
    }
    // Exponential backoff with jitter: base 200ms * 2^attempt, max 5s.
    let base = 200u64.saturating_mul(1u64 << attempt.min(5));
    let max = 5_000u64.min(base);
    let jitter = fastrand::u64(0..=max / 2);
    Duration::from_millis(max / 2 + jitter)
}

pub async fn rest_get_json<T: for<'de> Deserialize<'de>>(
    client: &Client,
    cfg: &Config,
    path: &str,
) -> RestResponse<T> {
    let url = format!("{}{}", cfg.api_url, path);
    let debug = std::env::var("GITHUB_MCP_DEBUG").ok().as_deref() == Some("1");
    if debug {
        eprintln!("[debug] REST GET {}", url);
    }
    let mut attempt: u32 = 0;
    loop {
        let res = client
            .get(&url)
            .header(AUTHORIZATION, auth_header(&cfg.token))
            .header("X-GitHub-Api-Version", &cfg.api_version)
            .header(
                ACCEPT,
                HeaderValue::from_static("application/vnd.github+json"),
            )
            .send()
            .await;

        let res = match res {
            Ok(r) => r,
            Err(e) => {
                warn!("REST GET error sending request: {}", e);
                if attempt < 5 {
                    tokio::time::sleep(compute_backoff(attempt, None)).await;
                    attempt += 1;
                    continue;
                }
                return RestResponse {
                    value: None,
                    meta: Meta { rate: None },
                    error: Some(ErrorInfo {
                        code: "upstream_error".into(),
                        message: e.to_string(),
                        retriable: true,
                    }),
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    headers: None,
                };
            }
        };

        let status = res.status();
        let headers = res.headers().clone();
        if debug {
            if let Some(link) = headers.get("link").and_then(|v| v.to_str().ok()) {
                eprintln!("[debug] Link: {}", link);
            }
        }
        let rate = extract_rate_from_rest(&headers);
        let retry_after = headers
            .get(RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_secs);

        if status.is_success() {
            match res.json::<T>().await {
                Ok(val) => {
                    return RestResponse {
                        value: Some(val),
                        meta: Meta { rate: Some(rate) },
                        error: None,
                        status,
                        headers: Some(headers),
                    };
                }
                Err(e) => {
                    return RestResponse {
                        value: None,
                        meta: Meta { rate: Some(rate) },
                        error: Some(ErrorInfo {
                            code: "server_error".into(),
                            message: e.to_string(),
                            retriable: false,
                        }),
                        status,
                        headers: Some(headers),
                    };
                }
            }
        }

        // Retry on 429/5xx
        if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()) && attempt < 5 {
            let backoff = compute_backoff(attempt, retry_after);
            warn!(
                "REST GET {} retrying (status {}), backoff {:?}",
                url, status, backoff
            );
            tokio::time::sleep(backoff).await;
            attempt += 1;
            continue;
        }
        let text = res.text().await.unwrap_or_default();
        let err = map_status_to_error(status, text);
        return RestResponse {
            value: None,
            meta: Meta { rate: Some(rate) },
            error: Some(err),
            status,
            headers: Some(headers),
        };
    }
}

// Variant of GET returning JSON but allowing a custom Accept header (e.g., star+json)
pub async fn rest_get_json_with_accept<T: for<'de> Deserialize<'de>>(
    client: &Client,
    cfg: &Config,
    path: &str,
    accept: &str,
) -> RestResponse<T> {
    let url = format!("{}{}", cfg.api_url, path);
    let mut attempt: u32 = 0;
    loop {
        let res = client
            .get(&url)
            .header(AUTHORIZATION, auth_header(&cfg.token))
            .header("X-GitHub-Api-Version", &cfg.api_version)
            .header(ACCEPT, HeaderValue::from_str(accept).unwrap())
            .send()
            .await;

        let res = match res {
            Ok(r) => r,
            Err(e) => {
                if attempt < 5 {
                    tokio::time::sleep(compute_backoff(attempt, None)).await;
                    attempt += 1;
                    continue;
                }
                return RestResponse {
                    value: None,
                    meta: Meta { rate: None },
                    error: Some(ErrorInfo {
                        code: "upstream_error".into(),
                        message: e.to_string(),
                        retriable: true,
                    }),
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    headers: None,
                };
            }
        };

        let status = res.status();
        let headers = res.headers().clone();
        let rate = extract_rate_from_rest(&headers);
        let retry_after = headers
            .get(RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_secs);

        if status.is_success() {
            match res.json::<T>().await {
                Ok(val) => {
                    return RestResponse {
                        value: Some(val),
                        meta: Meta { rate: Some(rate) },
                        error: None,
                        status,
                        headers: Some(headers),
                    };
                }
                Err(e) => {
                    return RestResponse {
                        value: None,
                        meta: Meta { rate: Some(rate) },
                        error: Some(ErrorInfo {
                            code: "server_error".into(),
                            message: e.to_string(),
                            retriable: false,
                        }),
                        status,
                        headers: Some(headers),
                    };
                }
            }
        }

        if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()) && attempt < 5 {
            let backoff = compute_backoff(attempt, retry_after);
            tokio::time::sleep(backoff).await;
            attempt += 1;
            continue;
        }
        let text = res.text().await.unwrap_or_default();
        let err = map_status_to_error(status, text);
        return RestResponse {
            value: None,
            meta: Meta { rate: Some(rate) },
            error: Some(err),
            status,
            headers: Some(headers),
        };
    }
}

// Generic JSON PUT helper
pub async fn rest_put_json<TReq: Serialize, TResp: for<'de> Deserialize<'de>>(
    client: &Client,
    cfg: &Config,
    path: &str,
    body: &TReq,
) -> RestResponse<TResp> {
    let url = format!("{}{}", cfg.api_url, path);
    let mut attempt: u32 = 0;
    loop {
        let res = client
            .put(&url)
            .header(AUTHORIZATION, auth_header(&cfg.token))
            .header("X-GitHub-Api-Version", &cfg.api_version)
            .header(
                ACCEPT,
                HeaderValue::from_static("application/vnd.github+json"),
            )
            .json(body)
            .send()
            .await;
        let res = match res {
            Ok(r) => r,
            Err(e) => {
                if attempt < 5 {
                    tokio::time::sleep(compute_backoff(attempt, None)).await;
                    attempt += 1;
                    continue;
                }
                return RestResponse {
                    value: None,
                    meta: Meta { rate: None },
                    error: Some(ErrorInfo {
                        code: "upstream_error".into(),
                        message: e.to_string(),
                        retriable: true,
                    }),
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    headers: None,
                };
            }
        };
        let status = res.status();
        let headers = res.headers().clone();
        let rate = extract_rate_from_rest(&headers);
        if status.is_success() {
            match res.json::<TResp>().await {
                Ok(val) => {
                    return RestResponse {
                        value: Some(val),
                        meta: Meta { rate: Some(rate) },
                        error: None,
                        status,
                        headers: Some(headers),
                    };
                }
                Err(e) => {
                    return RestResponse {
                        value: None,
                        meta: Meta { rate: Some(rate) },
                        error: Some(ErrorInfo {
                            code: "server_error".into(),
                            message: e.to_string(),
                            retriable: false,
                        }),
                        status,
                        headers: Some(headers),
                    };
                }
            }
        }
        if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()) && attempt < 5 {
            tokio::time::sleep(compute_backoff(attempt, None)).await;
            attempt += 1;
            continue;
        }
        let text = res.text().await.unwrap_or_default();
        let err = map_status_to_error(status, text);
        return RestResponse {
            value: None,
            meta: Meta { rate: Some(rate) },
            error: Some(err),
            status,
            headers: Some(headers),
        };
    }
}

// Generic JSON PATCH helper
pub async fn rest_patch_json<TReq: Serialize, TResp: for<'de> Deserialize<'de>>(
    client: &Client,
    cfg: &Config,
    path: &str,
    body: &TReq,
) -> RestResponse<TResp> {
    let url = format!("{}{}", cfg.api_url, path);
    let mut attempt: u32 = 0;
    loop {
        let res = client
            .patch(&url)
            .header(AUTHORIZATION, auth_header(&cfg.token))
            .header("X-GitHub-Api-Version", &cfg.api_version)
            .header(
                ACCEPT,
                HeaderValue::from_static("application/vnd.github+json"),
            )
            .json(body)
            .send()
            .await;
        let res = match res {
            Ok(r) => r,
            Err(e) => {
                if attempt < 5 {
                    tokio::time::sleep(compute_backoff(attempt, None)).await;
                    attempt += 1;
                    continue;
                }
                return RestResponse {
                    value: None,
                    meta: Meta { rate: None },
                    error: Some(ErrorInfo {
                        code: "upstream_error".into(),
                        message: e.to_string(),
                        retriable: true,
                    }),
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    headers: None,
                };
            }
        };
        let status = res.status();
        let headers = res.headers().clone();
        let rate = extract_rate_from_rest(&headers);
        if status.is_success() {
            match res.json::<TResp>().await {
                Ok(val) => {
                    return RestResponse {
                        value: Some(val),
                        meta: Meta { rate: Some(rate) },
                        error: None,
                        status,
                        headers: Some(headers),
                    }
                }
                Err(e) => {
                    return RestResponse {
                        value: None,
                        meta: Meta { rate: Some(rate) },
                        error: Some(ErrorInfo {
                            code: "server_error".into(),
                            message: e.to_string(),
                            retriable: false,
                        }),
                        status,
                        headers: Some(headers),
                    }
                }
            }
        }
        if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()) && attempt < 5 {
            tokio::time::sleep(compute_backoff(attempt, None)).await;
            attempt += 1;
            continue;
        }
        let text = res.text().await.unwrap_or_default();
        let err = map_status_to_error(status, text);
        return RestResponse {
            value: None,
            meta: Meta { rate: Some(rate) },
            error: Some(err),
            status,
            headers: Some(headers),
        };
    }
}

// Generic JSON POST helper
pub async fn rest_post_json<TReq: Serialize, TResp: for<'de> Deserialize<'de>>(
    client: &Client,
    cfg: &Config,
    path: &str,
    body: &TReq,
) -> RestResponse<TResp> {
    let url = format!("{}{}", cfg.api_url, path);
    let mut attempt: u32 = 0;
    loop {
        let res = client
            .post(&url)
            .header(AUTHORIZATION, auth_header(&cfg.token))
            .header("X-GitHub-Api-Version", &cfg.api_version)
            .header(
                ACCEPT,
                HeaderValue::from_static("application/vnd.github+json"),
            )
            .json(body)
            .send()
            .await;
        let res = match res {
            Ok(r) => r,
            Err(e) => {
                if attempt < 5 {
                    tokio::time::sleep(compute_backoff(attempt, None)).await;
                    attempt += 1;
                    continue;
                }
                return RestResponse {
                    value: None,
                    meta: Meta { rate: None },
                    error: Some(ErrorInfo {
                        code: "upstream_error".into(),
                        message: e.to_string(),
                        retriable: true,
                    }),
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    headers: None,
                };
            }
        };
        let status = res.status();
        let headers = res.headers().clone();
        let rate = extract_rate_from_rest(&headers);
        if status.is_success() || status == StatusCode::ACCEPTED {
            match res.json::<TResp>().await {
                Ok(val) => {
                    return RestResponse {
                        value: Some(val),
                        meta: Meta { rate: Some(rate) },
                        error: None,
                        status,
                        headers: Some(headers),
                    }
                }
                Err(e) => {
                    return RestResponse {
                        value: None,
                        meta: Meta { rate: Some(rate) },
                        error: Some(ErrorInfo {
                            code: "server_error".into(),
                            message: e.to_string(),
                            retriable: false,
                        }),
                        status,
                        headers: Some(headers),
                    }
                }
            }
        }
        if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()) && attempt < 5 {
            tokio::time::sleep(compute_backoff(attempt, None)).await;
            attempt += 1;
            continue;
        }
        let text = res.text().await.unwrap_or_default();
        let err = map_status_to_error(status, text);
        return RestResponse {
            value: None,
            meta: Meta { rate: Some(rate) },
            error: Some(err),
            status,
            headers: Some(headers),
        };
    }
}

pub async fn rest_get_text_with_accept(
    client: &Client,
    cfg: &Config,
    path: &str,
    accept: &str,
) -> RestResponse<String> {
    let url = format!("{}{}", cfg.api_url, path);
    let mut attempt: u32 = 0;
    loop {
        let res = client
            .get(&url)
            .header(AUTHORIZATION, auth_header(&cfg.token))
            .header("X-GitHub-Api-Version", &cfg.api_version)
            .header(ACCEPT, HeaderValue::from_str(accept).unwrap())
            .send()
            .await;

        let res = match res {
            Ok(r) => r,
            Err(e) => {
                if attempt < 5 {
                    tokio::time::sleep(compute_backoff(attempt, None)).await;
                    attempt += 1;
                    continue;
                }
                return RestResponse {
                    value: None,
                    meta: Meta { rate: None },
                    error: Some(ErrorInfo {
                        code: "upstream_error".into(),
                        message: e.to_string(),
                        retriable: true,
                    }),
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    headers: None,
                };
            }
        };

        let status = res.status();
        let headers = res.headers().clone();
        let rate = extract_rate_from_rest(&headers);
        let retry_after = headers
            .get(RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_secs);
        let text = res.text().await.unwrap_or_default();
        if status.is_success() {
            return RestResponse {
                value: Some(text),
                meta: Meta { rate: Some(rate) },
                error: None,
                status,
                headers: Some(headers),
            };
        }
        if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()) && attempt < 5 {
            let backoff = compute_backoff(attempt, retry_after);
            tokio::time::sleep(backoff).await;
            attempt += 1;
            continue;
        }
        let err = map_status_to_error(status, text);
        return RestResponse {
            value: None,
            meta: Meta { rate: Some(rate) },
            error: Some(err),
            status,
            headers: Some(headers),
        };
    }
}

pub fn has_next_page_from_link(headers: &HeaderMap) -> bool {
    if let Some(link) = headers.get("link").and_then(|v| v.to_str().ok()) {
        // Simple check for rel="next"
        return link.contains("rel=\"next\"");
    }
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQlResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphQlError>>, // standard GraphQL errors
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQlError {
    pub message: String,
}

pub async fn graphql_post<
    TReq: Serialize,
    TResp: for<'de> Deserialize<'de>,
    TRate: for<'de> Deserialize<'de>,
>(
    client: &Client,
    cfg: &Config,
    query: &str,
    variables: &TReq,
) -> (Option<TResp>, Meta, Option<ErrorInfo>) {
    let mut attempt: u32 = 0;
    let body = serde_json::json!({ "query": query, "variables": variables });
    loop {
        let res = client
            .post(&cfg.graphql_url)
            .header(AUTHORIZATION, auth_header(&cfg.token))
            .header(ACCEPT, HeaderValue::from_static("application/json"))
            .json(&body)
            .send()
            .await;

        let res = match res {
            Ok(r) => r,
            Err(e) => {
                if attempt < 5 {
                    tokio::time::sleep(compute_backoff(attempt, None)).await;
                    attempt += 1;
                    continue;
                }
                return (
                    None,
                    Meta { rate: None },
                    Some(ErrorInfo {
                        code: "upstream_error".into(),
                        message: e.to_string(),
                        retriable: true,
                    }),
                );
            }
        };

        let status = res.status();
        let text = res.text().await.unwrap_or_default();

        if status.is_success() {
            // Parse both typed and value to extract rateLimit if present
            let v: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    return (
                        None,
                        Meta { rate: None },
                        Some(ErrorInfo {
                            code: "server_error".into(),
                            message: e.to_string(),
                            retriable: false,
                        }),
                    );
                }
            };
            let parsed: Result<GraphQlResponse<TResp>, _> = serde_json::from_value(v.clone());
            match parsed {
                Ok(resp) => {
                    if let Some(errors) = resp.errors {
                        let msg = errors
                            .iter()
                            .map(|e| e.message.clone())
                            .collect::<Vec<_>>()
                            .join("; ");
                        return (
                            None,
                            Meta { rate: None },
                            Some(ErrorInfo {
                                code: "upstream_error".into(),
                                message: msg,
                                retriable: true,
                            }),
                        );
                    }
                    let rate = v.get("data").and_then(|d| d.get("rateLimit")).map(|rl| {
                        let remaining = rl
                            .get("remaining")
                            .and_then(|x| x.as_i64())
                            .map(|x| x as i32);
                        let used = rl.get("used").and_then(|x| x.as_i64()).map(|x| x as i32);
                        let reset_at = rl
                            .get("resetAt")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string());
                        RateMeta {
                            remaining,
                            used,
                            reset_at,
                        }
                    });
                    return (resp.data, Meta { rate }, None);
                }
                Err(e) => {
                    return (
                        None,
                        Meta { rate: None },
                        Some(ErrorInfo {
                            code: "server_error".into(),
                            message: e.to_string(),
                            retriable: false,
                        }),
                    );
                }
            }
        }

        // Retry on 429/5xx
        if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()) && attempt < 5 {
            let retry_after = None;
            tokio::time::sleep(compute_backoff(attempt, retry_after)).await;
            attempt += 1;
            continue;
        }
        let err = map_status_to_error(status, text);
        return (None, Meta { rate: None }, Some(err));
    }
}

// REST opaque cursor codec: base64(JSON { page, per_page })
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RestCursor {
    pub page: u32,
    pub per_page: u32,
    // Optional absolute or relative path to use for the next request; if present, prefer it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

pub fn encode_rest_cursor(c: RestCursor) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(&c).unwrap())
}

pub fn decode_rest_cursor(s: &str) -> Option<RestCursor> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s)
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

// Extract the relative path (including query) for rel="next" from a Link header if present.
// Returns values like "/repos/o/r/pulls/1/comments?page=2".
pub fn extract_next_path_from_link(headers: &HeaderMap) -> Option<String> {
    let link = headers.get("link")?.to_str().ok()?;
    // Find the segment with rel="next"
    // Example: <https://api.github.com/repos/o/r/pulls/1/comments?page=2>; rel="next", <...>; rel="last"
    for part in link.split(',') {
        let trimmed = part.trim();
        if !trimmed.contains("rel=\"next\"") {
            continue;
        }
        let start = trimmed.find('<')? + 1;
        let end = trimmed.find('>')?;
        let url_str = &trimmed[start..end];
        // Try to parse as absolute URL first; fall back to assuming it's already a path
        if let Ok(u) = url::Url::parse(url_str) {
            let mut p = u.path().to_string();
            if let Some(q) = u.query() {
                p.push('?');
                p.push_str(q);
            }
            return Some(p);
        } else {
            return Some(url_str.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_cursor_roundtrip() {
        let c = RestCursor {
            page: 2,
            per_page: 30,
            path: None,
        };
        let s = encode_rest_cursor(c.clone());
        let d = decode_rest_cursor(&s).unwrap();
        assert_eq!(c, d);
    }

    #[test]
    fn error_mapping_matrix() {
        assert_eq!(
            map_status_to_error(StatusCode::BAD_REQUEST, "".into()).code,
            "bad_request"
        );
        assert_eq!(
            map_status_to_error(StatusCode::UNAUTHORIZED, "".into()).code,
            "unauthorized"
        );
        assert_eq!(
            map_status_to_error(StatusCode::FORBIDDEN, "".into()).code,
            "forbidden"
        );
        assert_eq!(
            map_status_to_error(StatusCode::NOT_FOUND, "".into()).code,
            "not_found"
        );
        assert_eq!(
            map_status_to_error(StatusCode::CONFLICT, "".into()).code,
            "conflict"
        );
        let rl = map_status_to_error(StatusCode::TOO_MANY_REQUESTS, "".into());
        assert_eq!(rl.code, "rate_limited");
        assert!(rl.retriable);
        let s5 = map_status_to_error(StatusCode::INTERNAL_SERVER_ERROR, "".into());
        assert_eq!(s5.code, "upstream_error");
        assert!(s5.retriable);
    }
}

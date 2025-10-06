use github_mcp::config::Config;
use github_mcp::http::{
    decode_rest_cursor, encode_rest_cursor, extract_rate_from_rest, map_status_to_error, RestCursor,
};
use reqwest::header::HeaderMap;

#[test]
fn rest_cursor_codec_roundtrip() {
    let c = RestCursor {
        page: 3,
        per_page: 50,
    };
    let enc = encode_rest_cursor(c.clone());
    let dec = decode_rest_cursor(&enc).unwrap();
    assert_eq!(c, dec);
}

#[test]
fn status_error_mapping() {
    let e = map_status_to_error(reqwest::StatusCode::TOO_MANY_REQUESTS, "rate".into());
    assert_eq!(e.code, "rate_limited");
    assert!(e.retriable);
}

#[test]
fn rest_rate_headers() {
    let mut h = HeaderMap::new();
    h.insert("x-ratelimit-remaining", "4999".parse().unwrap());
    h.insert("x-ratelimit-used", "1".parse().unwrap());
    // Use a fixed epoch for deterministic test
    h.insert("x-ratelimit-reset", "0".parse().unwrap());
    let rate = extract_rate_from_rest(&h);
    assert_eq!(rate.remaining, Some(4999));
    assert_eq!(rate.used, Some(1));
    assert!(rate.reset_at.is_some());
}

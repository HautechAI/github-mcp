#![allow(non_snake_case)] // GraphQL/REST field names map directly; keep original casing
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::sync::{Mutex, OnceLock};

use log::{debug, info};
// use reqwest::header::HeaderMap; // not needed currently
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::http;
use crate::mcp::{mcp_wrap, IncludeRateGuard};
use crate::tools::*;

// Minimal diagnostics helper: writes to stderr and optionally to a file if MCP_DIAG_LOG is set.
static DIAG_FILE: OnceLock<Option<Mutex<File>>> = OnceLock::new();

fn get_diag_file() -> Option<&'static Mutex<File>> {
    DIAG_FILE
        .get_or_init(|| {
            if let Ok(path) = std::env::var("MCP_DIAG_LOG") {
                if !path.is_empty() {
                    if let Ok(f) = OpenOptions::new().create(true).append(true).open(path) {
                        return Some(Mutex::new(f));
                    }
                }
            }
            None
        })
        .as_ref()
}

macro_rules! diag {
    ($($arg:tt)*) => {{
        // Always to stderr with prefix
        eprintln!("[github-mcp][diag] {}", format_args!($($arg)*));
        // Optionally to file
        if let Some(mf) = get_diag_file() {
            if let Ok(mut f) = mf.lock() {
                let _ = writeln!(f, "[github-mcp][diag] {}", format_args!($($arg)*));
            }
        }
    }};
}
// uuid::Uuid not used; remove to satisfy clippy

// MCP protocol version we target
const PROTOCOL_VERSION: &str = "2024-11-05";

// Minimal JSON-RPC 2.0 types
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Id {
    Str(String),
    Num(i64),
    Null,
}

#[derive(Debug, Serialize, Deserialize)]
struct Request {
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Value,
    id: Option<Id>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Response {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
    id: Option<Id>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

fn rpc_error(id: Option<Id>, code: i64, message: &str, data: Option<Value>) -> Response {
    Response {
        jsonrpc: "2.0".into(),
        result: None,
        error: Some(RpcError {
            code,
            message: message.into(),
            data,
        }),
        id,
    }
}

fn rpc_ok(id: Option<Id>, result: Value) -> Response {
    Response {
        jsonrpc: "2.0".into(),
        result: Some(result),
        error: None,
        id,
    }
}

pub fn run_stdio_server() -> anyhow::Result<()> {
    info!(
        "Starting github-mcp stdio server; protocol={}",
        PROTOCOL_VERSION
    );
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = io::stdout();

    // Panic hook to log early exits/panics
    std::panic::set_hook(Box::new(|info| {
        eprintln!("[github-mcp][diag] panic: {}", info);
        if let Some(mf) = get_diag_file() {
            if let Ok(mut f) = mf.lock() {
                let _ = writeln!(f, "[github-mcp][diag] panic: {}", info);
            }
        }
    }));

    // Diagnostics for startup and handshake
    diag!(
        "stdio server ready; NDJSON mode; protocol={}",
        PROTOCOL_VERSION
    );

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            diag!("stdin EOF; exiting main loop");
            break;
        }
        let raw = line.trim_end_matches(['\r', '\n']).to_string();
        if raw.is_empty() {
            continue;
        }
        diag!(
            "stdin line bytes={} first100={}",
            raw.len(),
            &raw.chars().take(100).collect::<String>()
        );

        let req: Result<Request, _> = serde_json::from_str(&raw);
        let Some(request) = req.ok() else {
            diag!("JSON parse error; sending -32700");
            let resp = rpc_error(None, -32700, "Parse error", None);
            write_json_line_response(&mut stdout, &resp)?;
            continue;
        };

        // Log and ignore notifications
        if request.id.is_none() {
            if request.method == "initialized" || request.method == "notifications/initialized" {
                diag!("initialized notification received");
            } else {
                diag!("notification ignored; method={}", request.method);
            }
            continue; // no response to notifications
        }

        debug!("Received method={}", request.method);
        if request.method == "initialize" {
            diag!("initialize request received");
        }
        let resp = dispatch(request);
        write_json_line_response(&mut stdout, &resp)?;
    }
    Ok(())
}

fn write_json_line_response(out: &mut dyn Write, resp: &Response) -> anyhow::Result<()> {
    let payload = serde_json::to_string(resp)?;
    writeln!(out, "{}", payload)?;
    out.flush()?;
    diag!(
        "response written; json_len={} has_error={} has_result={}",
        payload.len(),
        resp.error.is_some(),
        resp.result.is_some()
    );
    Ok(())
}

fn dispatch(req: Request) -> Response {
    match req.method.as_str() {
        "initialize" => handle_initialize(req.id),
        "tools/list" => handle_tools_list(req.id),
        "tools/call" => handle_tools_call(req.id, req.params),
        other => rpc_error(
            req.id,
            -32601,
            &format!("Method not found: {}", other),
            None,
        ),
    }
}

fn handle_initialize(id: Option<Id>) -> Response {
    diag!("handle_initialize invoked");
    rpc_ok(
        id,
        serde_json::json!({
            "protocolVersion": PROTOCOL_VERSION,
            "serverInfo": {
                "name": "github-mcp",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": {
                "tools": { "listChanged": false }
            }
        }),
    )
}

fn handle_tools_list(id: Option<Id>) -> Response {
    let mut tools = tool_descriptors();
    // Gate the built-in ping tool behind env flag
    if !is_ping_enabled() {
        tools.retain(|t| t.name != "ping");
    }
    // Omit nextCursor when not paginating to align with MCP Inspector schema
    rpc_ok(id, serde_json::json!({ "tools": tools }))
}

#[derive(Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

fn handle_tools_call(id: Option<Id>, params: Value) -> Response {
    let parsed: Result<ToolCallParams, _> = serde_json::from_value(params);
    let Ok(call) = parsed else {
        return rpc_error(id, -32602, "Invalid params", None);
    };
    // Read reserved top-level flag for output shaping.
    // Note: thread-local scoping is OK for current sync runtime; if moving to async with thread-hopping,
    // consider task-local or explicit parameter plumbing.
    let include_rate = call
        .arguments
        .get("_include_rate")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    // Strip the reserved flag before passing arguments to handlers to avoid leaking unknown fields.
    let mut args = call.arguments.clone();
    if let Some(obj) = args.as_object_mut() {
        obj.remove("_include_rate");
    }
    let _guard = IncludeRateGuard::set(include_rate);
    match call.name.as_str() {
        "ping" => {
            if !is_ping_enabled() {
                return rpc_error(id, -32601, "Tool not found: ping (disabled)", None);
            }
            handle_ping(id, args)
        }
        "list_issues" => handle_list_issues(id, args),
        "get_issue" => handle_get_issue(id, args),
        "list_issue_comments_plain" => handle_list_issue_comments(id, args),
        "list_pull_requests" => handle_list_pull_requests(id, args),
        "get_pull_request" => handle_get_pull_request(id, args),
        "get_pr_status_summary" => handle_get_pr_status_summary(id, args),
        "list_pr_comments_plain" => handle_list_pr_comments(id, args),
        "list_pr_review_comments_plain" => handle_list_pr_review_comments(id, args),
        // Unified alias for review comments
        "list_pr_review_comments" => handle_list_pr_review_comments(id, args),
        "list_pr_review_threads_light" => handle_list_pr_review_threads(id, args),
        "resolve_pr_review_thread" => handle_resolve_pr_review_thread(id, args),
        "unresolve_pr_review_thread" => handle_unresolve_pr_review_thread(id, args),
        "list_pr_reviews_light" => handle_list_pr_reviews(id, args),
        "list_pr_reviews" => handle_list_pr_reviews(id, args),
        "list_pr_commits_light" => handle_list_pr_commits(id, args),
        "list_pr_commits" => handle_list_pr_commits(id, args),
        "list_pr_files_light" => handle_list_pr_files(id, args),
        "list_pr_files" => handle_list_pr_files(id, args),
        "get_pr_diff" => handle_get_pr_text(id, args, true),
        "get_pr_patch" => handle_get_pr_text(id, args, false),
        "pr_summary" => handle_pr_summary(id, args),
        "list_workflows_light" => handle_list_workflows(id, args),
        "list_workflow_runs_light" => handle_list_workflow_runs(id, args),
        "get_workflow_run_light" => handle_get_workflow_run(id, args),
        "list_workflow_jobs_light" => handle_list_workflow_jobs(id, args),
        "get_workflow_job_logs" => handle_get_workflow_job_logs(id, args),
        "rerun_workflow_run" => handle_rerun_workflow_run(id, args),
        "rerun_workflow_run_failed" => handle_rerun_workflow_run_failed(id, args),
        "cancel_workflow_run" => handle_cancel_workflow_run(id, args),
        "list_repo_secrets_light" => handle_list_repo_secrets(id, args),
        "list_repo_variables_light" => handle_list_repo_variables(id, args),
        "list_environments_light" => handle_list_environments(id, args),
        "list_environment_variables_light" => handle_list_environment_variables(id, args),
        // New methods per Issue #91
        "list_commits" => handle_list_commits(id, args),
        "get_commit" => handle_get_commit(id, args),
        "list_tags" => handle_list_tags(id, args),
        "get_tag" => handle_get_tag(id, args),
        "list_branches" => handle_list_branches(id, args),
        "list_releases" => handle_list_releases(id, args),
        "get_release" => handle_get_release(id, args),
        "list_starred_repositories" => handle_list_starred_repositories(id, args),
        "merge_pr" => handle_merge_pr(id, args),
        "search_issues" => handle_search_issues(id, args),
        "search_pull_requests" => handle_search_pull_requests(id, args),
        "search_repositories" => handle_search_repositories(id, args),
        "update_issue" => handle_update_issue(id, args),
        "update_pull_request" => handle_update_pull_request(id, args),
        "fork_repository" => handle_fork_repository(id, args),
        _ => rpc_error(id, -32601, &format!("Tool not found: {}", call.name), None),
    }
}

fn handle_ping(id: Option<Id>, params: Value) -> Response {
    let input: PingInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(_) => PingInput { message: None },
    };
    let message = input.message.unwrap_or_else(|| "pong".to_string());
    let structured = serde_json::to_value(PingOutput {
        message: message.clone(),
    })
    .unwrap();
    let wrapped = mcp_wrap(structured, Some(message), false);
    rpc_ok(id, wrapped)
}

fn is_ping_enabled() -> bool {
    // Default OFF; truthy values: 1/true/yes/on (case-insensitive)
    if let Ok(v) = std::env::var("GITHUB_MCP_ENABLE_PING") {
        let s = v.trim().to_ascii_lowercase();
        return matches!(s.as_str(), "1" | "true" | "yes" | "on");
    }
    false
}

fn enforce_limit(limit: Option<u32>) -> Result<u32, String> {
    let l = limit.unwrap_or(30);
    if l == 0 || l > 100 {
        return Err("limit must be 1..=100".into());
    }
    Ok(l)
}

// Removed unused ListIssuesVars; we build vars as serde_json::Value

fn handle_list_issues(id: Option<Id>, params: Value) -> Response {
    let input: ListIssuesInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false })) };
        let query = r#"
        query ListIssues($owner: String!, $repo: String!, $first: Int = 30, $after: String, $states: [IssueState!], $filterBy: IssueFilters, $orderBy: IssueOrder) {
          repository(owner: $owner, name: $repo) {
            issues(first: $first, after: $after, states: $states, filterBy: $filterBy, orderBy: $orderBy) {
              nodes { id number title state createdAt updatedAt author { login } }
              pageInfo { hasNextPage endCursor }
            }
          }
          rateLimit { remaining used resetAt }
        }
        "#;
        // Build states mapping with special handling for "all" to omit the variable.
        let states_var: Option<Vec<String>> = match input.state.as_deref() {
            Some("open") => Some(vec!["OPEN".to_string()]),
            Some("closed") => Some(vec!["CLOSED".to_string()]),
            Some("all") => None, // omit to include both
            None => None,
            Some(_) => None,
        };

        // Map sort/direction to IssueOrder at top level
        let order_by = input.sort.as_deref().map(|s| {
            let field = match s {
                "created" => "CREATED_AT",
                "updated" => "UPDATED_AT",
                "comments" => "COMMENTS",
                _ => "UPDATED_AT",
            };
            let dir = input
                .direction
                .as_deref()
                .unwrap_or("desc")
                .to_ascii_uppercase();
            serde_json::json!({ "field": field, "direction": dir })
        });

        // Build variables object, omitting optional fields when None
        let mut vars = serde_json::Map::new();
        vars.insert("owner".into(), serde_json::Value::String(input.owner));
        vars.insert("repo".into(), serde_json::Value::String(input.repo));
        vars.insert("first".into(), serde_json::Value::Number((limit as i64).into()));
        if let Some(after) = input.cursor { vars.insert("after".into(), serde_json::Value::String(after)); }
        if let Some(st) = states_var { vars.insert("states".into(), serde_json::Value::Array(st.into_iter().map(serde_json::Value::String).collect())); }
        let mut filter = serde_json::Map::new();
        if let Some(labels) = input.labels { filter.insert("labels".into(), serde_json::Value::Array(labels.into_iter().map(serde_json::Value::String).collect())); }
        if let Some(c) = input.creator { filter.insert("createdBy".into(), serde_json::Value::String(c)); }
        if let Some(m) = input.mentions { filter.insert("mentioned".into(), serde_json::Value::String(m)); }
        if let Some(a) = input.assignee { filter.insert("assignee".into(), serde_json::Value::String(a)); }
        if let Some(since) = input.since { filter.insert("since".into(), serde_json::Value::String(since)); }
        if !filter.is_empty() { vars.insert("filterBy".into(), serde_json::Value::Object(filter)); }
        if let Some(ob) = order_by { vars.insert("orderBy".into(), ob); }
        let vars = serde_json::Value::Object(vars);
        #[derive(Deserialize)]
        struct RespNode { id: String, number: i64, title: String, state: String, createdAt: String, updatedAt: String, author: Option<Author> }
        #[derive(Deserialize)]
        struct Author { login: String }
        #[derive(Deserialize)]
        struct RespIssues { nodes: Vec<RespNode>, pageInfo: PageInfo }
        #[derive(Deserialize)]
        struct PageInfo { hasNextPage: bool, endCursor: Option<String> }
        #[derive(Deserialize)]
        struct Repo { issues: RespIssues }
        #[derive(Deserialize)]
        struct Data { repository: Option<Repo> }
        let (data, gql_meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let repo = match data.and_then(|d| d.repository) { Some(r) => r, None => return (None, Meta { next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Repository not found".into(), retriable: false })) };
        let include_author = input.include_author.unwrap_or(false);
        let items: Vec<ListIssuesOutputItem> = repo.issues.nodes.into_iter().map(|n| ListIssuesOutputItem{
            id: n.id,
            number: n.number,
            title: n.title,
            state: n.state,
            created_at: n.createdAt,
            updated_at: n.updatedAt,
            author_login: if include_author { n.author.map(|a| a.login) } else { None },
        }).collect();
        let meta = Meta { next_cursor: repo.issues.pageInfo.endCursor, has_more: repo.issues.pageInfo.hasNextPage, rate: gql_meta.rate };
        (Some(items), meta, None)
    });
    let out = ListIssuesOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    // Prefer a short text summary when items present; otherwise serialize JSON
    let text = out.items.as_ref().map(|v| format!("{} issues", v.len()));
    let is_error = out.error.is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn parse_page_cursor(
    cursor: Option<String>,
    page: Option<u32>,
    per_page: Option<u32>,
) -> (u32, u32, Option<String>) {
    if let Some(c) = cursor {
        if let Some(decoded) = http::decode_rest_cursor(&c) {
            return (decoded.page, decoded.per_page, Some(c));
        }
    }
    (page.unwrap_or(1), per_page.unwrap_or(30).min(100), None)
}

// Convenience: convert cursor/limit into REST page/per_page
fn page_per_from_cursor(cursor: Option<String>, limit: Option<u32>) -> (u32, u32, Option<String>) {
    parse_page_cursor(cursor, None, limit)
}

fn handle_list_workflows(id: Option<Id>, params: Value) -> Response {
    let input: ListWorkflowsInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        // Workflows REST light: page/per_page only; no cursor field in input schema
        let (page, per_page, _cur) = parse_page_cursor(None, input.page, input.per_page);
        let path = format!(
            "/repos/{}/{}/actions/workflows?per_page={}&page={}",
            input.owner, input.repo, per_page, page
        );
        #[derive(Deserialize)]
        struct Workflows {
            workflows: Vec<Workflow>,
            #[allow(dead_code)]
            total_count: i64,
        }
        #[derive(Deserialize)]
        struct Workflow {
            id: i64,
            name: String,
            path: String,
            state: String,
        }
        let resp = http::rest_get_json::<Workflows>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let rate = resp.meta.rate;
        let items = resp.value.map(|v| {
            v.workflows
                .into_iter()
                .map(|w| WorkflowItem {
                    id: w.id,
                    name: w.name,
                    path: w.path,
                    state: w.state,
                })
                .collect()
        });
        let has_more = resp
            .headers
            .as_ref()
            .map(http::has_next_page_from_link)
            .unwrap_or(false);
        let next_cursor = if has_more {
            Some(http::encode_rest_cursor(http::RestCursor {
                page: page + 1,
                per_page,
                path: None,
            }))
        } else {
            None
        };
        (
            items,
            Meta {
                next_cursor,
                has_more,
                rate,
            },
            None,
        )
    });
    let out = ListWorkflowsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = out.items.as_ref().map(|v| format!("{} workflows", v.len()));
    let is_error = out.error.is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_list_workflow_runs(id: Option<Id>, params: Value) -> Response {
    let input: ListWorkflowRunsInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        // Workflow runs REST light: page/per_page only; no cursor field in input schema
        let (page, per_page, _cur) = parse_page_cursor(None, input.page, input.per_page);
        let path = format!(
            "/repos/{}/{}/actions/runs?per_page={}&page={}",
            input.owner, input.repo, per_page, page
        );
        #[derive(Deserialize)]
        struct Runs {
            workflow_runs: Vec<Run>,
        }
        #[derive(Deserialize)]
        struct Run {
            id: i64,
            run_number: i64,
            event: String,
            status: String,
            conclusion: Option<String>,
            head_sha: String,
            created_at: String,
            updated_at: String,
        }
        let resp = http::rest_get_json::<Runs>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let rate = resp.meta.rate;
        let items = resp.value.map(|v| {
            v.workflow_runs
                .into_iter()
                .map(|r| WorkflowRunItem {
                    id: r.id,
                    run_number: r.run_number,
                    event: r.event,
                    status: r.status,
                    conclusion: r.conclusion,
                    head_sha: r.head_sha,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                })
                .collect()
        });
        let has_more = resp
            .headers
            .as_ref()
            .map(http::has_next_page_from_link)
            .unwrap_or(false);
        let next_cursor = if has_more {
            Some(http::encode_rest_cursor(http::RestCursor {
                page: page + 1,
                per_page,
                path: None,
            }))
        } else {
            None
        };
        (
            items,
            Meta {
                next_cursor,
                has_more,
                rate,
            },
            None,
        )
    });
    let out = ListWorkflowRunsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = out
        .items
        .as_ref()
        .map(|v| format!("{} workflow runs", v.len()));
    let is_error = out.error.is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_get_workflow_run(id: Option<Id>, params: Value) -> Response {
    let input: GetWorkflowRunInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (item, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        let exclude = input.exclude_pull_requests.unwrap_or(false);
        let path = format!(
            "/repos/{}/{}/actions/runs/{}{}",
            input.owner,
            input.repo,
            input.run_id,
            if exclude {
                "?exclude_pull_requests=true"
            } else {
                ""
            }
        );
        #[derive(Deserialize)]
        struct Run {
            id: i64,
            run_number: i64,
            event: String,
            status: String,
            conclusion: Option<String>,
            head_sha: String,
            created_at: String,
            updated_at: String,
        }
        let resp = http::rest_get_json::<Run>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let rate = resp.meta.rate;
        let r = resp.value.unwrap();
        let item = WorkflowRunItem {
            id: r.id,
            run_number: r.run_number,
            event: r.event,
            status: r.status,
            conclusion: r.conclusion,
            head_sha: r.head_sha,
            created_at: r.created_at,
            updated_at: r.updated_at,
        };
        (
            Some(item),
            Meta {
                next_cursor: None,
                has_more: false,
                rate,
            },
            None,
        )
    });
    let out = GetWorkflowRunOutput {
        item,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = out
        .item
        .as_ref()
        .map(|i| format!("run #{} status {}", i.run_number, i.status));
    let is_error = out.error.is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_list_workflow_jobs(id: Option<Id>, params: Value) -> Response {
    let input: ListWorkflowJobsInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        // Workflow jobs REST light: page/per_page only; no cursor field in input schema
        let (page, per_page, _cur) = parse_page_cursor(None, input.page, input.per_page);
        let filter = input.filter.unwrap_or_else(|| "all".into());
        let path = format!(
            "/repos/{}/{}/actions/runs/{}/jobs?filter={}&per_page={}&page={}",
            input.owner, input.repo, input.run_id, filter, per_page, page
        );
        #[derive(Deserialize)]
        struct Job {
            id: i64,
            name: String,
            status: String,
            conclusion: Option<String>,
            started_at: Option<String>,
            completed_at: Option<String>,
        }
        #[derive(Deserialize)]
        struct Jobs {
            jobs: Vec<Job>,
        }
        let resp = http::rest_get_json::<Jobs>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let rate = resp.meta.rate;
        let items = resp.value.map(|v| {
            v.jobs
                .into_iter()
                .map(|j| WorkflowJobItem {
                    id: j.id,
                    name: j.name,
                    status: j.status,
                    conclusion: j.conclusion,
                    started_at: j.started_at,
                    completed_at: j.completed_at,
                })
                .collect()
        });
        let has_more = resp
            .headers
            .as_ref()
            .map(http::has_next_page_from_link)
            .unwrap_or(false);
        let next_cursor = if has_more {
            Some(http::encode_rest_cursor(http::RestCursor {
                page: page + 1,
                per_page,
                path: None,
            }))
        } else {
            None
        };
        (
            items,
            Meta {
                next_cursor,
                has_more,
                rate,
            },
            None,
        )
    });
    let out = ListWorkflowJobsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = out.items.as_ref().map(|v| format!("{} jobs", v.len()));
    let is_error = out.error.is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_get_workflow_job_logs(id: Option<Id>, params: Value) -> Response {
    use reqwest::StatusCode;
    let input: GetJobLogsInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (logs, truncated, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    false,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        // Step 1: call the GitHub logs endpoint, expecting 302 to ZIP
        let path = format!(
            "/repos/{}/{}/actions/jobs/{}/logs",
            input.owner, input.repo, input.job_id
        );
        let url = format!("{}{}", cfg.api_url, path);
        let res = client
            .get(&url)
            .bearer_auth(&cfg.token)
            .header("X-GitHub-Api-Version", &cfg.api_version)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await;
        let res = match res {
            Ok(r) => r,
            Err(e) => {
                return (
                    None,
                    false,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "upstream_error".into(),
                        message: e.to_string(),
                        retriable: true,
                    }),
                )
            }
        };
        let status = res.status();
        // If GitHub returns 302, follow Location
        if status == StatusCode::FOUND
            || status == StatusCode::MOVED_PERMANENTLY
            || status == StatusCode::TEMPORARY_REDIRECT
            || status == StatusCode::PERMANENT_REDIRECT
        {
            let location = res
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            if location.is_none() {
                return (
                    None,
                    false,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "upstream_error".into(),
                        message: "Missing Location for logs redirect".into(),
                        retriable: true,
                    }),
                );
            }
            let loc = location.unwrap();
            // Redirect target is a pre-signed ZIP URL; no auth required.
            let bin = match client.get(loc).send().await {
                Ok(r) => r.bytes().await.ok(),
                Err(_) => None,
            };
            if bin.is_none() {
                return (
                    None,
                    false,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "upstream_error".into(),
                        message: "Failed to download logs ZIP".into(),
                        retriable: true,
                    }),
                );
            }
            let bytes = bin.unwrap();
            // unzip and aggregate .txt files
            let mut cursor = std::io::Cursor::new(bytes);
            let z = zip::ZipArchive::new(&mut cursor)
                .map_err(|e| e.to_string())
                .ok();
            if z.is_none() {
                return (
                    None,
                    false,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: "Invalid ZIP".into(),
                        retriable: false,
                    }),
                );
            }
            let mut z = z.unwrap();
            let mut lines: Vec<String> = Vec::new();
            let mut truncated_any = false;
            for i in 0..z.len() {
                let mut file = z.by_index(i).unwrap();
                if !file.name().ends_with(".txt") {
                    continue;
                }
                use std::io::Read;
                let mut buf = String::new();
                let _ = file.read_to_string(&mut buf);
                // Tail per file if requested
                let mut file_lines: Vec<String> = buf.lines().map(|l| l.to_string()).collect();
                if let Some(tail) = input.tail_lines {
                    if file_lines.len() > tail {
                        truncated_any = true;
                        let total = file_lines.len();
                        file_lines = file_lines.split_off(total - tail);
                    }
                }
                lines.extend(file_lines);
            }
            let truncated = truncated_any;
            if input.include_timestamps.unwrap_or(false) {
                let now = chrono::Utc::now().to_rfc3339();
                lines = lines
                    .into_iter()
                    .map(|l| format!("{} {}", now, l))
                    .collect();
            }
            let aggregated = lines.join("\n");
            (
                Some(aggregated),
                truncated,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: None,
                },
                None,
            )
        } else if status.is_success() {
            // Some GH instances may return raw text; handle gracefully
            let text = res.text().await.unwrap_or_default();
            (
                Some(text),
                false,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: None,
                },
                None,
            )
        } else {
            let body = res.text().await.unwrap_or_default();
            let err = http::map_status_to_error(status, body);
            (
                None,
                false,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: None,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            )
        }
    });
    let out = GetJobLogsOutput {
        logs,
        truncated,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = out.logs.as_ref().map(|s| {
        if out.truncated {
            format!("{}\n…(truncated)", s)
        } else {
            s.clone()
        }
    });
    let is_error = out.error.is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_rerun_workflow_run(id: Option<Id>, params: Value) -> Response {
    let input: RunIdInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (ok, queued, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    false,
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        let path = format!(
            "/repos/{}/{}/actions/runs/{}/rerun",
            input.owner, input.repo, input.run_id
        );
        let resp = client
            .post(format!("{}{}", cfg.api_url, path))
            .bearer_auth(&cfg.token)
            .header("X-GitHub-Api-Version", &cfg.api_version)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                return (
                    false,
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "upstream_error".into(),
                        message: e.to_string(),
                        retriable: true,
                    }),
                )
            }
        };
        let status = resp.status();
        if status.is_success() || status == reqwest::StatusCode::ACCEPTED {
            (
                true,
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: None,
                },
                None,
            )
        } else {
            let body = resp.text().await.unwrap_or_default();
            let err = http::map_status_to_error(status, body);
            (
                false,
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: None,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            )
        }
    });
    let out = OkOutput {
        ok,
        queued_run_id: queued,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = Some(if out.ok {
        match out.queued_run_id {
            Some(q) => format!("rerun accepted; queued run id {}", q),
            None => "rerun accepted".to_string(),
        }
    } else {
        "rerun failed".to_string()
    });
    let is_error = out.error.is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_rerun_workflow_run_failed(id: Option<Id>, params: Value) -> Response {
    let input: RunIdInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (ok, queued, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    false,
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        let path = format!(
            "/repos/{}/{}/actions/runs/{}/rerun-failed-jobs",
            input.owner, input.repo, input.run_id
        );
        let resp = client
            .post(format!("{}{}", cfg.api_url, path))
            .bearer_auth(&cfg.token)
            .header("X-GitHub-Api-Version", &cfg.api_version)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                return (
                    false,
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "upstream_error".into(),
                        message: e.to_string(),
                        retriable: true,
                    }),
                )
            }
        };
        let status = resp.status();
        if status.is_success() || status == reqwest::StatusCode::ACCEPTED {
            (
                true,
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: None,
                },
                None,
            )
        } else {
            let body = resp.text().await.unwrap_or_default();
            let err = http::map_status_to_error(status, body);
            (
                false,
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: None,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            )
        }
    });
    let out = OkOutput {
        ok,
        queued_run_id: queued,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = Some(if out.ok {
        match out.queued_run_id {
            Some(q) => format!("rerun-failed accepted; queued run id {}", q),
            None => "rerun-failed accepted".to_string(),
        }
    } else {
        "rerun-failed request failed".to_string()
    });
    let is_error = out.error.is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_list_repo_secrets(id: Option<Id>, params: Value) -> Response {
    let input: RepoInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        let (page, per_page, _cur) = parse_page_cursor(input.cursor, input.page, input.per_page);
        let path = format!(
            "/repos/{}/{}/actions/secrets?per_page={}&page={}",
            input.owner, input.repo, per_page, page
        );
        #[derive(Deserialize)]
        struct Resp {
            secrets: Vec<Secret>,
        }
        #[derive(Deserialize)]
        struct Secret {
            name: String,
            created_at: Option<String>,
            updated_at: Option<String>,
        }
        let resp = http::rest_get_json::<Resp>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let rate = resp.meta.rate;
        let items = resp.value.map(|v| {
            v.secrets
                .into_iter()
                .map(|s| RepoSecretItem {
                    name: s.name,
                    created_at: s.created_at,
                    updated_at: s.updated_at,
                })
                .collect()
        });
        let has_more = resp
            .headers
            .as_ref()
            .map(http::has_next_page_from_link)
            .unwrap_or(false);
        let next_cursor = if has_more {
            Some(http::encode_rest_cursor(http::RestCursor {
                page: page + 1,
                per_page,
                path: None,
            }))
        } else {
            None
        };
        (
            items,
            Meta {
                next_cursor,
                has_more,
                rate,
            },
            None,
        )
    });
    let out = ListRepoSecretsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = out
        .items
        .as_ref()
        .map(|v| format!("{} secrets (metadata)", v.len()));
    let wrapped = mcp_wrap(structured, text, out.error.is_some());
    rpc_ok(id, wrapped)
}

fn handle_list_repo_variables(id: Option<Id>, params: Value) -> Response {
    let input: RepoInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        // Environment variables: support cursor-based pagination
        let (page, per_page, _cur) = parse_page_cursor(input.cursor, input.page, input.per_page);
        let path = format!(
            "/repos/{}/{}/actions/variables?per_page={}&page={}",
            input.owner, input.repo, per_page, page
        );
        #[derive(Deserialize)]
        struct Resp {
            variables: Vec<Var>,
        }
        #[derive(Deserialize)]
        struct Var {
            name: String,
            value: Option<String>,
            created_at: Option<String>,
            updated_at: Option<String>,
        }
        let resp = http::rest_get_json::<Resp>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let rate = resp.meta.rate;
        let items = resp.value.map(|v| {
            v.variables
                .into_iter()
                .map(|x| RepoVariableItem {
                    name: x.name,
                    value: x.value,
                    created_at: x.created_at,
                    updated_at: x.updated_at,
                })
                .collect()
        });
        let has_more = resp
            .headers
            .as_ref()
            .map(http::has_next_page_from_link)
            .unwrap_or(false);
        let next_cursor = if has_more {
            Some(http::encode_rest_cursor(http::RestCursor {
                page: page + 1,
                per_page,
                path: None,
            }))
        } else {
            None
        };
        (
            items,
            Meta {
                next_cursor,
                has_more,
                rate,
            },
            None,
        )
    });
    let out = ListRepoVariablesOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = out.items.as_ref().map(|v| format!("{} variables", v.len()));
    let wrapped = mcp_wrap(structured, text, out.error.is_some());
    rpc_ok(id, wrapped)
}

fn handle_list_environments(id: Option<Id>, params: Value) -> Response {
    let input: RepoInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        // Environments: support cursor-based pagination
        let (page, per_page, _cur) = parse_page_cursor(input.cursor, input.page, input.per_page);
        let path = format!(
            "/repos/{}/{}/environments?per_page={}&page={}",
            input.owner, input.repo, per_page, page
        );
        #[derive(Deserialize)]
        struct Resp {
            environments: Vec<Env>,
        }
        #[derive(Deserialize)]
        struct Env {
            name: String,
            url: Option<String>,
        }
        let resp = http::rest_get_json::<Resp>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let rate = resp.meta.rate;
        let items = resp.value.map(|v| {
            v.environments
                .into_iter()
                .map(|e| EnvironmentItem {
                    name: e.name,
                    url: e.url,
                })
                .collect()
        });
        let has_more = resp
            .headers
            .as_ref()
            .map(http::has_next_page_from_link)
            .unwrap_or(false);
        let next_cursor = if has_more {
            Some(http::encode_rest_cursor(http::RestCursor {
                page: page + 1,
                per_page,
                path: None,
            }))
        } else {
            None
        };
        (
            items,
            Meta {
                next_cursor,
                has_more,
                rate,
            },
            None,
        )
    });
    let out = ListEnvironmentsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = out
        .items
        .as_ref()
        .map(|v| format!("{} environments", v.len()));
    let wrapped = mcp_wrap(structured, text, out.error.is_some());
    rpc_ok(id, wrapped)
}

fn handle_list_environment_variables(id: Option<Id>, params: Value) -> Response {
    let input: EnvVarsInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        // Environment variables: support cursor-based pagination
        let (page, per_page, _cur) = parse_page_cursor(input.cursor, input.page, input.per_page);
        // Ensure environment_name is URL-encoded in the path
        let env_enc = http::encode_path_segment(&input.environment_name);
        let path = format!(
            "/repos/{}/{}/environments/{}/variables?per_page={}&page={}",
            input.owner, input.repo, env_enc, per_page, page
        );
        #[derive(Deserialize)]
        struct Resp {
            variables: Vec<Var>,
        }
        #[derive(Deserialize)]
        struct Var {
            name: String,
            value: Option<String>,
            created_at: Option<String>,
            updated_at: Option<String>,
        }
        let resp = http::rest_get_json::<Resp>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let rate = resp.meta.rate;
        let items = resp.value.map(|v| {
            v.variables
                .into_iter()
                .map(|x| RepoVariableItem {
                    name: x.name,
                    value: x.value,
                    created_at: x.created_at,
                    updated_at: x.updated_at,
                })
                .collect()
        });
        let has_more = resp
            .headers
            .as_ref()
            .map(http::has_next_page_from_link)
            .unwrap_or(false);
        let next_cursor = if has_more {
            Some(http::encode_rest_cursor(http::RestCursor {
                page: page + 1,
                per_page,
                path: None,
            }))
        } else {
            None
        };
        (
            items,
            Meta {
                next_cursor,
                has_more,
                rate,
            },
            None,
        )
    });
    let out = ListRepoVariablesOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = out.items.as_ref().map(|v| format!("{} env vars", v.len()));
    let wrapped = mcp_wrap(structured, text, out.error.is_some());
    rpc_ok(id, wrapped)
}

fn handle_cancel_workflow_run(id: Option<Id>, params: Value) -> Response {
    let input: RunIdInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (ok, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    false,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        let path = format!(
            "/repos/{}/{}/actions/runs/{}/cancel",
            input.owner, input.repo, input.run_id
        );
        let resp = client
            .post(format!("{}{}", cfg.api_url, path))
            .bearer_auth(&cfg.token)
            .header("X-GitHub-Api-Version", &cfg.api_version)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                return (
                    false,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "upstream_error".into(),
                        message: e.to_string(),
                        retriable: true,
                    }),
                )
            }
        };
        let status = resp.status();
        if status.is_success() || status == reqwest::StatusCode::ACCEPTED {
            (
                true,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: None,
                },
                None,
            )
        } else {
            let body = resp.text().await.unwrap_or_default();
            let err = http::map_status_to_error(status, body);
            (
                false,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: None,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            )
        }
    });
    let out = OkOutput {
        ok,
        queued_run_id: None,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = Some(if out.ok {
        "cancel accepted".to_string()
    } else {
        "cancel failed".to_string()
    });
    let is_error = out.error.is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_list_pr_comments(id: Option<Id>, params: Value) -> Response {
    let input: ListPrCommentsInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false })) };
        let query = r#"
        query ListPrComments($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
          repository(owner: $owner, name: $repo) {
            pullRequest(number: $number) {
              comments(first: $first, after: $after) {
                nodes { id body createdAt updatedAt author { login } }
                pageInfo { hasNextPage endCursor }
              }
            }
          }
          rateLimit { remaining used resetAt }
        }
        "#;
        #[derive(Deserialize)] struct Author { login: String }
        #[derive(Deserialize)] struct Node { id: String, body: String, createdAt: String, updatedAt: String, author: Option<Author> }
        #[derive(Deserialize)] struct PageInfo { hasNextPage: bool, endCursor: Option<String> }
        #[derive(Deserialize)] struct Comments { nodes: Vec<Node>, pageInfo: PageInfo }
        #[derive(Deserialize)] struct PR { comments: Comments }
        #[derive(Deserialize)] struct Repo { pullRequest: Option<PR> }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number, "first": limit as i64, "after": input.cursor });
        let (data, gql_meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let pr = match data.and_then(|d| d.repository).and_then(|r| r.pullRequest) { Some(p) => p, None => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Pull request not found".into(), retriable: false })) };
        let include_author = input.include_author.unwrap_or(false);
        let items: Vec<PlainComment> = pr.comments.nodes.into_iter().map(|n| PlainComment{
            id: n.id, body: n.body, created_at: n.createdAt, updated_at: n.updatedAt,
            author_login: if include_author { n.author.map(|a| a.login) } else { None },
        }).collect();
        let meta = Meta { next_cursor: pr.comments.pageInfo.endCursor, has_more: pr.comments.pageInfo.hasNextPage, rate: gql_meta.rate };
        (Some(items), meta, None)
    });
    let out = ListPrCommentsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("items")
        .and_then(|v| v.as_array())
        .map(|v| format!("{} comments", v.len()));
    let is_error = structured
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn map_side(s: Option<String>) -> Option<String> {
    s.map(|x| x.to_uppercase())
}

fn handle_list_pr_review_comments(id: Option<Id>, params: Value) -> Response {
    let input: ListPrReviewCommentsInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        // Build REST client
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        // Decode opaque cursor if present; otherwise start at page=1 with per_page=limit
        let decoded_cursor = input
            .cursor
            .as_ref()
            .and_then(|s| http::decode_rest_cursor(s));
        let (page, per_page) = if let Some(c) = decoded_cursor.clone() {
            (c.page, c.per_page)
        } else {
            (1u32, limit)
        };
        // Temporary debug to stderr to diagnose pagination issue in tests
        if std::env::var("GITHUB_MCP_DEBUG").ok().as_deref() == Some("1") {
            eprintln!(
                "[debug] list_pr_review_comments: raw_cursor={:?} decoded={:?} -> page={} per_page={}",
                input.cursor, decoded_cursor, page, per_page
            );
        }
        // REST: GET /repos/{owner}/{repo}/pulls/{number}/comments
        // Prefer the exact next path from Link header if our cursor encodes it, otherwise build from owner/repo/number
        let path = if let Some(dc) = decoded_cursor.as_ref().and_then(|c| c.path.clone()) {
            dc
        } else {
            format!(
                "/repos/{}/{}/pulls/{}/comments?per_page={}&page={}",
                input.owner, input.repo, input.number, per_page, page
            )
        };
        #[derive(Deserialize)]
        struct RestUser {
            login: String,
        }
        #[derive(Deserialize)]
        struct RestReviewComment {
            id: Option<i64>,
            node_id: Option<String>,
            body: String,
            user: Option<RestUser>,
            created_at: String,
            updated_at: String,
            // Location fields
            path: Option<String>,
            line: Option<i64>,
            start_line: Option<i64>,
            side: Option<String>,
            start_side: Option<String>,
            original_line: Option<i64>,
            original_start_line: Option<i64>,
            diff_hunk: Option<String>,
            commit_id: Option<String>,
            original_commit_id: Option<String>,
        }
        let resp = http::rest_get_json::<Vec<RestReviewComment>>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let rate = resp.meta.rate;
        let include_author = input.include_author.unwrap_or(false);
        let include_loc = input.include_location.unwrap_or(false);
        let items = resp.value.map(|arr| {
            arr.into_iter()
                .map(|n| {
                    let id = n
                        .node_id
                        .clone()
                        .unwrap_or_else(|| n.id.map(|i| i.to_string()).unwrap_or_default());
                    ReviewCommentItem {
                        id,
                        body: n.body,
                        created_at: n.created_at,
                        updated_at: n.updated_at,
                        author_login: if include_author {
                            n.user.map(|u| u.login)
                        } else {
                            None
                        },
                        path: if include_loc { n.path } else { None },
                        line: if include_loc { n.line } else { None },
                        start_line: if include_loc { n.start_line } else { None },
                        side: if include_loc { map_side(n.side) } else { None },
                        start_side: if include_loc {
                            map_side(n.start_side)
                        } else {
                            None
                        },
                        original_line: if include_loc { n.original_line } else { None },
                        original_start_line: if include_loc {
                            n.original_start_line
                        } else {
                            None
                        },
                        diff_hunk: if include_loc { n.diff_hunk } else { None },
                        commit_sha: if include_loc { n.commit_id } else { None },
                        original_commit_sha: if include_loc {
                            n.original_commit_id
                        } else {
                            None
                        },
                    }
                })
                .collect::<Vec<ReviewCommentItem>>()
        });
        // Pagination via Link header
        let has_more = resp
            .headers
            .as_ref()
            .map(http::has_next_page_from_link)
            .unwrap_or(false);
        let next_cursor = if has_more {
            // Try to carry the exact next relative path from the Link header to avoid path-only matching issues in tests
            let next_path = resp
                .headers
                .as_ref()
                .and_then(http::extract_next_path_from_link);
            Some(http::encode_rest_cursor(http::RestCursor {
                page: page + 1,
                per_page,
                path: next_path,
            }))
        } else {
            None
        };
        (
            items,
            Meta {
                next_cursor,
                has_more,
                rate,
            },
            None,
        )
    });
    let out = ListPrReviewCommentsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    // Simple text summary; keep stable
    let text = structured
        .get("items")
        .and_then(|v| v.as_array())
        .map(|v| format!("{} review comments", v.len()));
    let is_error = structured
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_list_pr_review_threads(id: Option<Id>, params: Value) -> Response {
    let input: ListPrReviewThreadsInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false })) };
        let query = r#"
        query ListPrReviewThreads($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
          repository(owner: $owner, name: $repo) {
            pullRequest(number: $number) {
              reviewThreads(first: $first, after: $after) {
                nodes {
                  id isResolved isOutdated comments { totalCount } resolvedBy { login }
                  path line startLine diffSide startDiffSide
                }
                pageInfo { hasNextPage endCursor }
              }
            }
          }
          rateLimit { remaining used resetAt }
        }
        "#;
        #[derive(Deserialize)] struct Author { login: String }
        #[derive(Deserialize)] struct Node { id: String, isResolved: bool, isOutdated: bool, comments: Count, resolvedBy: Option<Author>, path: Option<String>, line: Option<i64>, startLine: Option<i64>, diffSide: Option<String>, startDiffSide: Option<String> }
        #[derive(Deserialize)] struct Count { totalCount: i64 }
        #[derive(Deserialize)] struct PageInfo { hasNextPage: bool, endCursor: Option<String> }
        #[derive(Deserialize)] struct Threads { nodes: Vec<Node>, pageInfo: PageInfo }
        #[derive(Deserialize)] struct PR { reviewThreads: Threads }
        #[derive(Deserialize)] struct Repo { pullRequest: Option<PR> }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number, "first": limit as i64, "after": input.cursor });
        let (data, gql_meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let pr = match data.and_then(|d| d.repository).and_then(|r| r.pullRequest) { Some(p) => p, None => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Pull request not found".into(), retriable: false })) };
        let include_author = input.include_author.unwrap_or(false);
        let include_loc = input.include_location.unwrap_or(false);
        let items: Vec<ReviewThreadItem> = pr.reviewThreads.nodes.into_iter().map(|n| ReviewThreadItem{
            id: n.id,
            is_resolved: n.isResolved,
            is_outdated: n.isOutdated,
            comments_count: n.comments.totalCount,
            resolved_by_login: if include_author { n.resolvedBy.map(|a| a.login) } else { None },
            path: if include_loc { n.path } else { None },
            line: if include_loc { n.line } else { None },
            start_line: if include_loc { n.startLine } else { None },
            side: if include_loc { map_side(n.diffSide) } else { None },
            start_side: if include_loc { map_side(n.startDiffSide) } else { None },
        }).collect();
        let meta = Meta { next_cursor: pr.reviewThreads.pageInfo.endCursor, has_more: pr.reviewThreads.pageInfo.hasNextPage, rate: gql_meta.rate };
        (Some(items), meta, None)
    });
    let out = ListPrReviewThreadsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("items")
        .and_then(|v| v.as_array())
        .map(|v| format!("{} review threads", v.len()));
    let is_error = structured
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_resolve_pr_review_thread(id: Option<Id>, params: Value) -> Response {
    let input: ResolveThreadInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let thread_id_for_vars = input.thread_id.clone();
    let (ok, meta, err, is_resolved) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    false,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                    false,
                )
            }
        };
        let query = r#"
        mutation ResolvePrReviewThread($thread_id: ID!) {
          resolveReviewThread(input: { threadId: $thread_id }) { thread { id isResolved } }
        }
        "#;
        #[derive(Deserialize)]
        struct Thread {
            #[allow(dead_code)]
            id: String,
            isResolved: bool,
        }
        #[derive(Deserialize)]
        struct Resp {
            resolveReviewThread: Option<Resolved>,
        }
        #[derive(Deserialize)]
        struct Resolved {
            thread: Thread,
        }
        let vars = serde_json::json!({ "thread_id": thread_id_for_vars });
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Resp, serde_json::Value>(
            &client, &cfg, query, &vars,
        )
        .await;
        if let Some(e) = err {
            return (
                false,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: None,
                },
                Some(ErrorShape {
                    code: e.code,
                    message: e.message,
                    retriable: e.retriable,
                }),
                false,
            );
        }
        let is_resolved = data
            .and_then(|d| d.resolveReviewThread)
            .map(|x| x.thread.isResolved)
            .unwrap_or(false);
        (
            true,
            Meta {
                next_cursor: None,
                has_more: false,
                rate: None,
            },
            None,
            is_resolved,
        )
    });
    let out = ResolveThreadOutput {
        ok,
        thread_id: input.thread_id,
        is_resolved,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = Some(format!(
        "thread resolved: {}",
        structured
            .get("is_resolved")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    ));
    let is_error = structured
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_unresolve_pr_review_thread(id: Option<Id>, params: Value) -> Response {
    let input: ResolveThreadInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let thread_id_for_vars = input.thread_id.clone();
    let (ok, meta, err, is_resolved) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    false,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                    false,
                )
            }
        };
        let query = r#"
        mutation UnresolvePrReviewThread($thread_id: ID!) {
          unresolveReviewThread(input: { threadId: $thread_id }) { thread { id isResolved } }
        }
        "#;
        #[derive(Deserialize)]
        struct Thread {
            #[allow(dead_code)]
            id: String,
            isResolved: bool,
        }
        #[derive(Deserialize)]
        struct Resp {
            unresolveReviewThread: Option<Resolved>,
        }
        #[derive(Deserialize)]
        struct Resolved {
            thread: Thread,
        }
        let vars = serde_json::json!({ "thread_id": thread_id_for_vars });
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Resp, serde_json::Value>(
            &client, &cfg, query, &vars,
        )
        .await;
        if let Some(e) = err {
            return (
                false,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: None,
                },
                Some(ErrorShape {
                    code: e.code,
                    message: e.message,
                    retriable: e.retriable,
                }),
                false,
            );
        }
        let is_resolved = data
            .and_then(|d| d.unresolveReviewThread)
            .map(|x| x.thread.isResolved)
            .unwrap_or(false);
        (
            true,
            Meta {
                next_cursor: None,
                has_more: false,
                rate: None,
            },
            None,
            is_resolved,
        )
    });
    let out = ResolveThreadOutput {
        ok,
        thread_id: input.thread_id,
        is_resolved,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = Some(format!(
        "thread resolved: {}",
        structured
            .get("is_resolved")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    ));
    let is_error = structured
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_list_pr_reviews(id: Option<Id>, params: Value) -> Response {
    let input: ListPrReviewsInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false })) };
        let query = r#"
        query ListPrReviews($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
          repository(owner: $owner, name: $repo) {
            pullRequest(number: $number) {
              reviews(first: $first, after: $after) {
                nodes { id state submittedAt author { login } }
                pageInfo { hasNextPage endCursor }
              }
            }
          }
          rateLimit { remaining used resetAt }
        }
        "#;
        #[derive(Deserialize)] struct Author { login: String }
        #[derive(Deserialize)] struct Node { id: String, state: String, submittedAt: Option<String>, author: Option<Author> }
        #[derive(Deserialize)] struct PageInfo { hasNextPage: bool, endCursor: Option<String> }
        #[derive(Deserialize)] struct Reviews { nodes: Vec<Node>, pageInfo: PageInfo }
        #[derive(Deserialize)] struct PR { reviews: Reviews }
        #[derive(Deserialize)] struct Repo { pullRequest: Option<PR> }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number, "first": limit as i64, "after": input.cursor });
        let (data, gql_meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let pr = match data.and_then(|d| d.repository).and_then(|r| r.pullRequest) { Some(p) => p, None => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Pull request not found".into(), retriable: false })) };
        let include_author = input.include_author.unwrap_or(false);
        let items: Vec<PrReviewItem> = pr.reviews.nodes.into_iter().map(|n| PrReviewItem{
            id: n.id, state: n.state, submitted_at: n.submittedAt, author_login: if include_author { n.author.map(|a| a.login) } else { None }
        }).collect();
        let meta = Meta { next_cursor: pr.reviews.pageInfo.endCursor, has_more: pr.reviews.pageInfo.hasNextPage, rate: gql_meta.rate };
        (Some(items), meta, None)
    });
    let out = ListPrReviewsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("items")
        .and_then(|v| v.as_array())
        .map(|v| format!("{} reviews", v.len()));
    let is_error = structured
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_list_pr_commits(id: Option<Id>, params: Value) -> Response {
    let input: ListPrCommitsInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false })) };
        let query = r#"
        query ListPrCommits($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
          repository(owner: $owner, name: $repo) {
            pullRequest(number: $number) {
              commits(first: $first, after: $after) {
                nodes { commit { oid messageHeadline authoredDate author { user { login } } } }
                pageInfo { hasNextPage endCursor }
              }
            }
          }
          rateLimit { remaining used resetAt }
        }
        "#;
        #[derive(Deserialize)] struct User { login: String }
        #[derive(Deserialize)] struct CommitAuthor { user: Option<User> }
        #[derive(Deserialize)] struct Commit { oid: String, messageHeadline: String, authoredDate: String, author: Option<CommitAuthor> }
        #[derive(Deserialize)] struct Node { commit: Commit }
        #[derive(Deserialize)] struct PageInfo { hasNextPage: bool, endCursor: Option<String> }
        #[derive(Deserialize)] struct Commits { nodes: Vec<Node>, pageInfo: PageInfo }
        #[derive(Deserialize)] struct PR { commits: Commits }
        #[derive(Deserialize)] struct Repo { pullRequest: Option<PR> }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number, "first": limit as i64, "after": input.cursor });
        let (data, gql_meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let pr = match data.and_then(|d| d.repository).and_then(|r| r.pullRequest) { Some(p) => p, None => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Pull request not found".into(), retriable: false })) };
        let include_author = input.include_author.unwrap_or(false);
        let items: Vec<PrCommitItem> = pr.commits.nodes.into_iter().map(|n| PrCommitItem{
            sha: n.commit.oid,
            title: n.commit.messageHeadline,
            authored_at: n.commit.authoredDate,
            author_login: if include_author { n.commit.author.and_then(|a| a.user.map(|u| u.login)) } else { None },
        }).collect();
        let meta = Meta { next_cursor: pr.commits.pageInfo.endCursor, has_more: pr.commits.pageInfo.hasNextPage, rate: gql_meta.rate };
        (Some(items), meta, None)
    });
    let out = ListPrCommitsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("items")
        .and_then(|v| v.as_array())
        .map(|v| format!("{} commits", v.len()));
    let is_error = structured
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_list_pr_files(id: Option<Id>, params: Value) -> Response {
    // Accept both legacy (page/per_page) and unified (cursor/limit)
    let input: ListPrFilesInput = match serde_json::from_value(params.clone()) {
        Ok(v) => v,
        Err(_) => {
            #[derive(Deserialize)]
            struct Unified {
                owner: String,
                repo: String,
                number: i64,
                cursor: Option<String>,
                limit: Option<u32>,
                include_patch: Option<bool>,
            }
            let uni: Unified = match serde_json::from_value(params) {
                Ok(v) => v,
                Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
            };
            ListPrFilesInput {
                owner: uni.owner,
                repo: uni.repo,
                number: uni.number,
                page: None,
                per_page: uni.limit,
                include_patch: uni.include_patch,
            }
        }
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        // Map REST pagination inputs; if page unspecified, derive from cursor/limit
        let (page, per_page, _cur) = parse_page_cursor(None, input.page, input.per_page);
        let path = format!(
            "/repos/{}/{}/pulls/{}/files?per_page={}&page={}",
            input.owner, input.repo, input.number, per_page, page
        );
        #[derive(Deserialize)]
        struct File {
            filename: String,
            status: String,
            additions: i64,
            deletions: i64,
            changes: i64,
            sha: String,
            patch: Option<String>,
        }
        let resp = http::rest_get_json::<Vec<File>>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let rate = resp.meta.rate;
        // No Link header available via rest_get_json; assume more pages if current page filled
        let has_more =
            (resp.value.as_ref().map(|v| v.len()).unwrap_or(0) as i64) >= i64::from(per_page);
        let next_cursor = if has_more {
            Some(http::encode_rest_cursor(http::RestCursor {
                page: page + 1,
                per_page,
                path: None,
            }))
        } else {
            None
        };
        let include_patch = input.include_patch.unwrap_or(false);
        let items: Vec<PrFileItem> = resp
            .value
            .unwrap_or_default()
            .into_iter()
            .map(|f| PrFileItem {
                filename: f.filename,
                status: f.status,
                additions: f.additions,
                deletions: f.deletions,
                changes: f.changes,
                sha: f.sha,
                patch: if include_patch { f.patch } else { None },
            })
            .collect();
        (
            Some(items),
            Meta {
                next_cursor,
                has_more,
                rate,
            },
            None,
        )
    });
    let out = ListPrFilesOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("items")
        .and_then(|v| v.as_array())
        .map(|v| format!("{} files", v.len()));
    let is_error = structured
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_pr_summary(id: Option<Id>, params: Value) -> Response {
    #[derive(Deserialize)]
    struct Input {
        owner: String,
        repo: String,
        number: i64,
        include_checks: Option<bool>,
        include_files: Option<bool>,
        include_reviews: Option<bool>,
    }
    #[derive(Serialize)]
    struct ChecksSummary {
        state: Option<String>,
        success: i64,
        failure: i64,
        pending: i64,
    }
    #[derive(Serialize)]
    struct SummaryItem {
        number: i64,
        title: String,
        state: String,
        is_draft: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        author_login: Option<String>,
        head_sha: String,
        base_ref: String,
        commits_count: i64,
        changed_files_count: i64,
        additions: i64,
        deletions: i64,
        review_states: std::collections::BTreeMap<String, i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        checks_summary: Option<ChecksSummary>,
        #[serde(skip_serializing_if = "Option::is_none")]
        files: Option<Vec<PrFileItem>>,
    }
    #[derive(Serialize)]
    struct Output {
        item: Option<SummaryItem>,
        meta: Meta,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<ErrorShape>,
    }

    let input: Input = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (item, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        // 1) GET PR
        #[derive(Deserialize)]
        struct PR {
            number: i64,
            title: String,
            state: String,
            draft: bool,
            user: Option<User>,
            head: Head,
            base: Base,
            commits: i64,
            changed_files: i64,
            additions: i64,
            deletions: i64,
        }
        #[derive(Deserialize)]
        struct User {
            login: String,
        }
        #[derive(Deserialize)]
        struct Head {
            sha: String,
        }
        #[derive(Deserialize)]
        struct Base {
            #[allow(dead_code)]
            label: String,
            #[serde(rename = "ref")]
            r#ref: String,
        }
        let pr_path = format!(
            "/repos/{}/{}/pulls/{}",
            input.owner, input.repo, input.number
        );
        let pr_resp = http::rest_get_json::<PR>(&client, &cfg, &pr_path).await;
        if let Some(err) = pr_resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: pr_resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let pr = pr_resp.value.unwrap();
        let mut review_states: std::collections::BTreeMap<String, i64> =
            std::collections::BTreeMap::new();
        let mut files_opt: Option<Vec<PrFileItem>> = None;
        let mut checks_opt: Option<ChecksSummary> = None;
        // 2) optionally reviews
        if input.include_reviews.unwrap_or(true) {
            #[derive(Deserialize)]
            struct Rev {
                state: String,
            }
            let reviews_path = format!(
                "/repos/{}/{}/pulls/{}/reviews?per_page=100&page=1",
                input.owner, input.repo, input.number
            );
            let reviews = http::rest_get_json::<Vec<Rev>>(&client, &cfg, &reviews_path).await;
            if let Some(v) = reviews.value {
                for r in v {
                    *review_states.entry(r.state).or_insert(0) += 1;
                }
            }
        }
        // 3) optionally files
        if input.include_files.unwrap_or(true) {
            let files_path = format!(
                "/repos/{}/{}/pulls/{}/files?per_page=100&page=1",
                input.owner, input.repo, input.number
            );
            #[derive(Deserialize)]
            struct File {
                filename: String,
                status: String,
                additions: i64,
                deletions: i64,
                changes: i64,
                sha: String,
                patch: Option<String>,
            }
            let resp = http::rest_get_json::<Vec<File>>(&client, &cfg, &files_path).await;
            files_opt = resp.value.map(|v| {
                v.into_iter()
                    .map(|f| PrFileItem {
                        filename: f.filename,
                        status: f.status,
                        additions: f.additions,
                        deletions: f.deletions,
                        changes: f.changes,
                        sha: f.sha,
                        patch: None,
                    })
                    .collect()
            });
        }
        // 4) optionally checks summary from status+check-runs
        if input.include_checks.unwrap_or(true) {
            // legacy statuses
            #[derive(Deserialize)]
            struct Status {
                state: String,
            }
            let status_path = format!(
                "/repos/{}/{}/commits/{}/status",
                input.owner, input.repo, pr.head.sha
            );
            let status_resp = http::rest_get_json::<Status>(&client, &cfg, &status_path).await;
            // check runs
            #[derive(Deserialize)]
            struct CheckRun {
                conclusion: Option<String>,
                status: String,
            }
            #[derive(Deserialize)]
            struct CheckRuns {
                total_count: i64,
                check_runs: Vec<CheckRun>,
            }
            let cr_path = format!(
                "/repos/{}/{}/commits/{}/check-runs",
                input.owner, input.repo, pr.head.sha
            );
            let checks_resp = http::rest_get_json_with_accept::<CheckRuns>(
                &client,
                &cfg,
                &cr_path,
                "application/vnd.github+json",
            )
            .await;
            let mut success = 0;
            let mut failure = 0;
            let mut pending = 0;
            if let Some(v) = checks_resp.value {
                for cr in v.check_runs {
                    match (cr.conclusion.as_deref(), cr.status.as_str()) {
                        (Some("success"), _) => success += 1,
                        (Some("failure"), _) | (Some("timed_out"), _) | (Some("cancelled"), _) => {
                            failure += 1
                        }
                        (_, s) if s != "completed" => pending += 1,
                        _ => {}
                    }
                }
            }
            let state = status_resp.value.map(|s| s.state);
            checks_opt = Some(ChecksSummary {
                state,
                success,
                failure,
                pending,
            });
        }
        let item = SummaryItem {
            number: pr.number,
            title: pr.title,
            state: pr.state,
            is_draft: pr.draft,
            author_login: pr.user.map(|u| u.login),
            head_sha: pr.head.sha,
            base_ref: pr.base.r#ref,
            commits_count: pr.commits,
            changed_files_count: pr.changed_files,
            additions: pr.additions,
            deletions: pr.deletions,
            review_states,
            checks_summary: checks_opt,
            files: files_opt,
        };
        (
            Some(item),
            Meta {
                next_cursor: None,
                has_more: false,
                rate: pr_resp.meta.rate,
            },
            None,
        )
    });
    let out = serde_json::to_value(Output {
        item,
        meta,
        error: err,
    })
    .unwrap();
    let text = Some("pr summary".to_string());
    let is_error = out
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(out, text, is_error);
    rpc_ok(id, wrapped)
}

// Placeholder implementations for new tools; will be implemented next
fn handle_list_commits(id: Option<Id>, params: Value) -> Response {
    let input: ListCommitsInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        // Start at page per cursor or default 1
        let (page, per_page, cur) = page_per_from_cursor(input.cursor, Some(limit));
        let mut path = format!(
            "/repos/{}/{}/commits?per_page={}&page={}",
            input.owner, input.repo, per_page, page
        );
        if let Some(sha) = input.sha {
            path.push_str(&format!("&sha={}", sha));
        }
        if let Some(p) = input.path {
            path.push_str(&format!("&path={}", http::encode_path_segment(&p)));
        }
        if let Some(a) = input.author {
            path.push_str(&format!("&author={}", a));
        }
        if let Some(s) = input.since {
            path.push_str(&format!("&since={}", s));
        }
        if let Some(u) = input.until {
            path.push_str(&format!("&until={}", u));
        }
        #[derive(Deserialize)]
        struct User {
            login: String,
        }
        #[derive(Deserialize)]
        struct CommitUser {
            name: Option<String>,
            email: Option<String>,
            date: Option<String>,
        }
        #[derive(Deserialize)]
        struct CommitObj {
            message: String,
            author: Option<CommitUser>,
        }
        #[derive(Deserialize)]
        struct RestCommit {
            sha: String,
            commit: CommitObj,
            author: Option<User>,
            committer: Option<User>,
            parents: Option<Vec<Parent>>,
            stats: Option<Stats>,
        }
        #[derive(Deserialize)]
        struct Parent {
            sha: String,
        }
        #[derive(Deserialize)]
        struct Stats {
            additions: i64,
            deletions: i64,
            total: i64,
        }
        let resp = http::rest_get_json::<Vec<RestCommit>>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let rate = resp.meta.rate;
        let include_author = input.include_author.unwrap_or(false);
        let include_stats = input.include_stats.unwrap_or(false);
        let items = resp.value.map(|v| {
            v.into_iter()
                .map(|c| ListCommitsItem {
                    sha: c.sha,
                    title: c.commit.message.lines().next().unwrap_or("").to_string(),
                    authored_at: c.commit.author.and_then(|a| a.date),
                    author_login: if include_author {
                        c.author.map(|u| u.login)
                    } else {
                        None
                    },
                    committer_login: if include_author {
                        c.committer.map(|u| u.login)
                    } else {
                        None
                    },
                    stats: if include_stats {
                        c.stats.map(|s| CommitStats {
                            additions: s.additions,
                            deletions: s.deletions,
                            total: s.total,
                        })
                    } else {
                        None
                    },
                })
                .collect()
        });
        let has_more = resp
            .headers
            .as_ref()
            .map(http::has_next_page_from_link)
            .unwrap_or(false);
        let next_cursor = if has_more {
            Some(http::encode_rest_cursor(http::RestCursor {
                page: cur
                    .and_then(|c| http::decode_rest_cursor(&c))
                    .map(|c| c.page)
                    .unwrap_or(page)
                    + 1,
                per_page,
                path: None,
            }))
        } else {
            None
        };
        (
            items,
            Meta {
                next_cursor,
                has_more,
                rate,
            },
            None,
        )
    });
    let out = ListCommitsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("items")
        .and_then(|v| v.as_array())
        .map(|v| format!("{} commits", v.len()));
    let is_error = structured
        .get("error")
        .map(|e| !e.is_null())
        .unwrap_or(false);
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_get_commit(id: Option<Id>, params: Value) -> Response {
    let input: GetCommitInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (item, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        let path = format!(
            "/repos/{}/{}/commits/{}",
            input.owner, input.repo, input.r#ref
        );
        #[derive(Deserialize)]
        struct User {
            login: String,
        }
        #[derive(Deserialize)]
        struct CommitUser {
            name: Option<String>,
            email: Option<String>,
            date: Option<String>,
        }
        #[derive(Deserialize)]
        struct CommitObj {
            message: String,
            author: Option<CommitUser>,
        }
        #[derive(Deserialize)]
        struct File {
            filename: String,
            status: String,
            additions: i64,
            deletions: i64,
            changes: i64,
            patch: Option<String>,
        }
        #[derive(Deserialize)]
        struct Parent {
            sha: String,
        }
        #[derive(Deserialize)]
        struct Stats {
            additions: i64,
            deletions: i64,
            total: i64,
        }
        #[derive(Deserialize)]
        struct Resp {
            sha: String,
            commit: CommitObj,
            author: Option<User>,
            committer: Option<User>,
            parents: Vec<Parent>,
            stats: Option<Stats>,
            files: Option<Vec<File>>,
        }
        let resp = http::rest_get_json::<Resp>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let r = resp.value.unwrap();
        let include_stats = input.include_stats.unwrap_or(true);
        let include_files = input.include_files.unwrap_or(false);
        let item = GetCommitItem {
            sha: r.sha,
            message: r.commit.message,
            authored_at: r.commit.author.and_then(|a| a.date),
            author_login: r.author.map(|u| u.login),
            committer_login: r.committer.map(|u| u.login),
            parents: r
                .parents
                .into_iter()
                .map(|p| CommitParent { sha: p.sha })
                .collect(),
            stats: if include_stats {
                r.stats.map(|s| CommitStats {
                    additions: s.additions,
                    deletions: s.deletions,
                    total: s.total,
                })
            } else {
                None
            },
            files: if include_files {
                r.files.map(|v| {
                    v.into_iter()
                        .map(|f| CommitFile {
                            filename: f.filename,
                            status: f.status,
                            additions: f.additions,
                            deletions: f.deletions,
                            changes: f.changes,
                            patch: f.patch,
                        })
                        .collect()
                })
            } else {
                None
            },
        };
        (
            Some(item),
            Meta {
                next_cursor: None,
                has_more: false,
                rate: resp.meta.rate,
            },
            None,
        )
    });
    let out = GetCommitOutput {
        item,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("item")
        .and_then(|v| if v.is_null() { None } else { Some(()) })
        .map(|_| "commit".to_string());
    let is_error = structured
        .get("error")
        .map(|e| !e.is_null())
        .unwrap_or(false);
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}
fn handle_list_tags(id: Option<Id>, params: Value) -> Response {
    let input: ListTagsInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        let (page, per_page, _cur) = page_per_from_cursor(input.cursor, Some(limit));
        let path = format!(
            "/repos/{}/{}/tags?per_page={}&page={}",
            input.owner, input.repo, per_page, page
        );
        #[derive(Deserialize)]
        struct Tag {
            name: String,
            commit: CommitRef,
            zipball_url: String,
            tarball_url: String,
        }
        #[derive(Deserialize)]
        struct CommitRef {
            sha: String,
            url: String,
        }
        let resp = http::rest_get_json::<Vec<Tag>>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let mut items: Vec<TagItem> = Vec::new();
        if let Some(arr) = resp.value {
            for t in arr {
                let ti = TagItem {
                    name: t.name,
                    commit_sha: t.commit.sha,
                    zipball_url: t.zipball_url,
                    tarball_url: t.tarball_url,
                    r#type: "lightweight".into(),
                    tagger: None,
                    message: None,
                };
                items.push(ti);
            }
        }
        let has_more = resp
            .headers
            .as_ref()
            .map(http::has_next_page_from_link)
            .unwrap_or(false);
        let next_cursor = if has_more {
            Some(http::encode_rest_cursor(http::RestCursor {
                page: page + 1,
                per_page,
                path: None,
            }))
        } else {
            None
        };
        (
            Some(items),
            Meta {
                next_cursor,
                has_more,
                rate: resp.meta.rate,
            },
            None,
        )
    });
    let out = ListTagsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("items")
        .and_then(|v| v.as_array())
        .map(|v| format!("{} tags", v.len()));
    let is_error = structured
        .get("error")
        .map(|e| !e.is_null())
        .unwrap_or(false);
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}
fn handle_get_tag(id: Option<Id>, params: Value) -> Response {
    let input: GetTagInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (item, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        let tag_enc = http::encode_path_segment(&input.tag);
        #[derive(Deserialize)]
        struct RefResp {
            object: Obj,
        }
        #[derive(Deserialize)]
        struct Obj {
            r#type: String,
            sha: String,
        }
        let ref_path = format!(
            "/repos/{}/{}/git/ref/tags/{}",
            input.owner, input.repo, tag_enc
        );
        let ref_resp = http::rest_get_json::<RefResp>(&client, &cfg, &ref_path).await;
        if let Some(err) = ref_resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: ref_resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let obj = ref_resp.value.unwrap().object;
        if obj.r#type == "tag" && input.resolve_annotated.unwrap_or(true) {
            #[derive(Deserialize)]
            struct TagObj {
                tag: String,
                message: Option<String>,
                tagger: Option<Tagger>,
                object: Obj2,
            }
            #[derive(Deserialize)]
            struct Tagger {
                name: Option<String>,
            }
            #[derive(Deserialize)]
            struct Obj2 {
                sha: String,
            }
            let path = format!("/repos/{}/{}/git/tags/{}", input.owner, input.repo, obj.sha);
            let resp = http::rest_get_json::<TagObj>(&client, &cfg, &path).await;
            if let Some(err) = resp.error {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: resp.meta.rate,
                    },
                    Some(ErrorShape {
                        code: err.code,
                        message: err.message,
                        retriable: err.retriable,
                    }),
                );
            }
            let t = resp.value.unwrap();
            let item = GetTagItem {
                name: t.tag,
                commit_sha: t.object.sha,
                r#type: "annotated".into(),
                tagger: t.tagger.and_then(|tg| tg.name),
                message: t.message,
            };
            (
                Some(item),
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                None,
            )
        } else {
            let item = GetTagItem {
                name: input.tag,
                commit_sha: obj.sha,
                r#type: "lightweight".into(),
                tagger: None,
                message: None,
            };
            (
                Some(item),
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: ref_resp.meta.rate,
                },
                None,
            )
        }
    });
    let out = GetTagOutput {
        item,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = Some("tag".to_string());
    let is_error = structured
        .get("error")
        .map(|e| !e.is_null())
        .unwrap_or(false);
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}
fn handle_list_branches(id: Option<Id>, params: Value) -> Response {
    let input: ListBranchesInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        let (page, per_page, _cur) = page_per_from_cursor(input.cursor, Some(limit));
        let mut path = format!(
            "/repos/{}/{}/branches?per_page={}&page={}",
            input.owner, input.repo, per_page, page
        );
        if let Some(p) = input.protected {
            path.push_str(&format!("&protected={}", if p { "true" } else { "false" }));
        }
        #[derive(Deserialize)]
        struct CommitRef {
            sha: String,
        }
        #[derive(Deserialize)]
        struct Branch {
            name: String,
            commit: CommitRef,
            protected: bool,
        }
        let resp = http::rest_get_json::<Vec<Branch>>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let items = resp.value.map(|v| {
            v.into_iter()
                .map(|b| BranchItem {
                    name: b.name,
                    commit_sha: b.commit.sha,
                    protected: b.protected,
                })
                .collect()
        });
        let has_more = resp
            .headers
            .as_ref()
            .map(http::has_next_page_from_link)
            .unwrap_or(false);
        let next_cursor = if has_more {
            Some(http::encode_rest_cursor(http::RestCursor {
                page: page + 1,
                per_page,
                path: None,
            }))
        } else {
            None
        };
        (
            items,
            Meta {
                next_cursor,
                has_more,
                rate: resp.meta.rate,
            },
            None,
        )
    });
    let out = ListBranchesOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("items")
        .and_then(|v| v.as_array())
        .map(|v| format!("{} branches", v.len()));
    let is_error = structured
        .get("error")
        .map(|e| !e.is_null())
        .unwrap_or(false);
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}
fn handle_list_releases(id: Option<Id>, params: Value) -> Response {
    let input: ListReleasesInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        let (page, per_page, _cur) = page_per_from_cursor(input.cursor, Some(limit));
        let path = format!(
            "/repos/{}/{}/releases?per_page={}&page={}",
            input.owner, input.repo, per_page, page
        );
        #[derive(Deserialize)]
        struct User {
            login: String,
        }
        #[derive(Deserialize)]
        struct Rel {
            id: i64,
            tag_name: String,
            name: Option<String>,
            draft: bool,
            prerelease: bool,
            created_at: Option<String>,
            published_at: Option<String>,
            author: Option<User>,
            assets: Vec<serde_json::Value>,
        }
        let resp = http::rest_get_json::<Vec<Rel>>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let items = resp.value.map(|v| {
            v.into_iter()
                .map(|r| ReleaseItem {
                    id: r.id,
                    tag_name: r.tag_name,
                    name: r.name,
                    draft: r.draft,
                    prerelease: r.prerelease,
                    created_at: r.created_at,
                    published_at: r.published_at,
                    author_login: r.author.map(|u| u.login),
                    assets_count: r.assets.len() as i64,
                })
                .collect()
        });
        let has_more = resp
            .headers
            .as_ref()
            .map(http::has_next_page_from_link)
            .unwrap_or(false);
        let next_cursor = if has_more {
            Some(http::encode_rest_cursor(http::RestCursor {
                page: page + 1,
                per_page,
                path: None,
            }))
        } else {
            None
        };
        (
            items,
            Meta {
                next_cursor,
                has_more,
                rate: resp.meta.rate,
            },
            None,
        )
    });
    let out = ListReleasesOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("items")
        .and_then(|v| v.as_array())
        .map(|v| format!("{} releases", v.len()));
    let is_error = structured
        .get("error")
        .map(|e| !e.is_null())
        .unwrap_or(false);
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}
fn handle_get_release(id: Option<Id>, params: Value) -> Response {
    let input: GetReleaseInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (item, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        #[derive(Deserialize)]
        struct User {
            login: String,
        }
        #[derive(Deserialize)]
        struct Asset {
            id: i64,
            name: String,
            content_type: String,
            size: i64,
            download_count: i64,
            browser_download_url: String,
        }
        #[derive(Deserialize)]
        struct Rel {
            id: i64,
            tag_name: String,
            name: Option<String>,
            draft: bool,
            prerelease: bool,
            created_at: Option<String>,
            published_at: Option<String>,
            body: Option<String>,
            author: Option<User>,
            assets: Vec<Asset>,
        }
        let path = if let Some(idv) = input.release_id {
            format!("/repos/{}/{}/releases/{}", input.owner, input.repo, idv)
        } else if let Some(tag) = input.tag {
            let tagenc = http::encode_path_segment(&tag);
            format!(
                "/repos/{}/{}/releases/tags/{}",
                input.owner, input.repo, tagenc
            )
        } else {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: None,
                },
                Some(ErrorShape {
                    code: "invalid_params".into(),
                    message: "Provide release_id or tag".into(),
                    retriable: false,
                }),
            );
        };
        let resp = http::rest_get_json::<Rel>(&client, &cfg, &path).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let r = resp.value.unwrap();
        let assets = r
            .assets
            .into_iter()
            .map(|a| ReleaseAsset {
                id: a.id,
                name: a.name,
                content_type: a.content_type,
                size: a.size,
                download_count: a.download_count,
                browser_download_url: a.browser_download_url,
            })
            .collect();
        let item = GetReleaseItem {
            id: r.id,
            tag_name: r.tag_name,
            name: r.name,
            draft: r.draft,
            prerelease: r.prerelease,
            created_at: r.created_at,
            published_at: r.published_at,
            body: r.body,
            author_login: r.author.map(|u| u.login),
            assets,
        };
        (
            Some(item),
            Meta {
                next_cursor: None,
                has_more: false,
                rate: resp.meta.rate,
            },
            None,
        )
    });
    let out = GetReleaseOutput {
        item,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = Some("release".to_string());
    let is_error = structured
        .get("error")
        .map(|e| !e.is_null())
        .unwrap_or(false);
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}
fn handle_list_starred_repositories(id: Option<Id>, params: Value) -> Response {
    let input: ListStarredReposInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        let (page, per_page, _cur) = page_per_from_cursor(input.cursor, Some(limit));
        let sort = input.sort.unwrap_or_else(|| "created".into());
        let direction = input.direction.unwrap_or_else(|| "desc".into());
        let accept = if input.include_starred_at.unwrap_or(false) {
            "application/vnd.github.star+json"
        } else {
            "application/vnd.github+json"
        };
        let path = format!(
            "/user/starred?per_page={}&page={}&sort={}&direction={}",
            per_page, page, sort, direction
        );
        #[derive(Deserialize)]
        struct Owner {
            login: String,
        }
        #[derive(Deserialize)]
        struct Repo {
            full_name: String,
            private: bool,
            description: Option<String>,
            language: Option<String>,
            stargazers_count: i64,
            html_url: String,
            owner: Owner,
        }
        #[derive(Deserialize)]
        struct Starred {
            starred_at: Option<String>,
            repo: Repo,
        }
        let starred_at = input.include_starred_at.unwrap_or(false);
        if starred_at {
            let resp =
                http::rest_get_json_with_accept::<Vec<Starred>>(&client, &cfg, &path, accept).await;
            if let Some(err) = resp.error {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: resp.meta.rate,
                    },
                    Some(ErrorShape {
                        code: err.code,
                        message: err.message,
                        retriable: err.retriable,
                    }),
                );
            }
            let items = resp.value.map(|v| {
                v.into_iter()
                    .map(|s| StarredRepoItem {
                        full_name: s.repo.full_name,
                        private: s.repo.private,
                        description: s.repo.description,
                        language: s.repo.language,
                        stargazers_count: s.repo.stargazers_count,
                        html_url: s.repo.html_url,
                        starred_at: s.starred_at,
                    })
                    .collect()
            });
            let has_more = resp
                .headers
                .as_ref()
                .map(http::has_next_page_from_link)
                .unwrap_or(false);
            let next_cursor = if has_more {
                Some(http::encode_rest_cursor(http::RestCursor {
                    page: page + 1,
                    per_page,
                    path: None,
                }))
            } else {
                None
            };
            return (
                items,
                Meta {
                    next_cursor,
                    has_more,
                    rate: resp.meta.rate,
                },
                None,
            );
        } else {
            let resp = http::rest_get_json::<Vec<Repo>>(&client, &cfg, &path).await;
            if let Some(err) = resp.error {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: resp.meta.rate,
                    },
                    Some(ErrorShape {
                        code: err.code,
                        message: err.message,
                        retriable: err.retriable,
                    }),
                );
            }
            let items = resp.value.map(|v| {
                v.into_iter()
                    .map(|r| StarredRepoItem {
                        full_name: r.full_name,
                        private: r.private,
                        description: r.description,
                        language: r.language,
                        stargazers_count: r.stargazers_count,
                        html_url: r.html_url,
                        starred_at: None,
                    })
                    .collect()
            });
            let has_more = resp
                .headers
                .as_ref()
                .map(http::has_next_page_from_link)
                .unwrap_or(false);
            let next_cursor = if has_more {
                Some(http::encode_rest_cursor(http::RestCursor {
                    page: page + 1,
                    per_page,
                    path: None,
                }))
            } else {
                None
            };
            return (
                items,
                Meta {
                    next_cursor,
                    has_more,
                    rate: resp.meta.rate,
                },
                None,
            );
        }
    });
    let out = ListStarredReposOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("items")
        .and_then(|v| v.as_array())
        .map(|v| format!("{} starred repos", v.len()));
    let is_error = structured
        .get("error")
        .map(|e| !e.is_null())
        .unwrap_or(false);
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}
fn handle_merge_pr(id: Option<Id>, params: Value) -> Response {
    let input: MergePrInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (item, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        #[derive(Serialize)]
        struct Body {
            merge_method: Option<String>,
            commit_title: Option<String>,
            commit_message: Option<String>,
            sha: Option<String>,
        }
        #[derive(Deserialize)]
        struct Resp {
            merged: bool,
            message: String,
            sha: Option<String>,
        }
        let path = format!(
            "/repos/{}/{}/pulls/{}/merge",
            input.owner, input.repo, input.number
        );
        let req = Body {
            merge_method: input.merge_method,
            commit_title: input.commit_title,
            commit_message: input.commit_message,
            sha: input.sha,
        };
        let resp = http::rest_put_json::<Body, Resp>(&client, &cfg, &path, &req).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let r = resp.value.unwrap();
        (
            Some(MergePrResult {
                merged: r.merged,
                message: r.message,
                sha: r.sha,
            }),
            Meta {
                next_cursor: None,
                has_more: false,
                rate: resp.meta.rate,
            },
            None,
        )
    });
    let out = MergePrOutput {
        item,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("item")
        .and_then(|v| v.get("message"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let is_error = structured
        .get("error")
        .map(|e| !e.is_null())
        .unwrap_or(false);
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}
fn handle_search_issues(id: Option<Id>, params: Value) -> Response {
    let input: SearchInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    handle_search_common(id, "issues", input, limit)
}
fn handle_search_pull_requests(id: Option<Id>, params: Value) -> Response {
    let input: SearchInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    handle_search_common(id, "issues", input, limit) // PRs are part of issues endpoint with type:pr in query
}
fn handle_search_repositories(id: Option<Id>, params: Value) -> Response {
    let input: SearchInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    handle_search_common(id, "repositories", input, limit)
}

fn handle_search_common(id: Option<Id>, index: &str, input: SearchInput, limit: u32) -> Response {
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (out_val, text, is_err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c)=>c, Err(e)=> { let v = serde_json::json!({"error": {"code":"server_error","message": e.to_string(),"retriable": false}}); return (v, Some("search error".to_string()), true) } };
        let (page, per_page, _cur) = page_per_from_cursor(input.cursor, Some(limit));
        let mut path = format!("/search/{}?per_page={}&page={}&q={}", index, per_page, page, urlencoding::encode(&input.q));
        if let Some(s) = input.sort { path.push_str(&format!("&sort={}", s)); }
        if let Some(o) = input.order { path.push_str(&format!("&order={}", o)); }
        #[derive(Deserialize)] struct RateMetaOnly { remaining: Option<i32>, used: Option<i32>, reset_at: Option<String> }
        if index == "repositories" {
            #[derive(Deserialize)] struct RepoItem { full_name: String, private: bool, description: Option<String>, language: Option<String>, stargazers_count: i64, forks_count: i64, open_issues_count: i64, html_url: String }
            #[derive(Deserialize)] struct Resp { total_count: i64, incomplete_results: bool, items: Vec<RepoItem> }
            let resp = http::rest_get_json::<Resp>(&client, &cfg, &path).await;
            if let Some(err) = resp.error { let v = serde_json::json!({"error": {"code": err.code, "message": err.message, "retriable": err.retriable}}); return (v, Some("search error".into()), true) }
            let has_more = resp.headers.as_ref().map(http::has_next_page_from_link).unwrap_or(false);
            let next_cursor = if has_more { Some(http::encode_rest_cursor(http::RestCursor{ page: page+1, per_page, path: None })) } else { None };
            let val = resp.value.unwrap();
            let items = val.items.into_iter().map(|r| SearchRepoItem{ full_name: r.full_name, private: r.private, description: r.description, language: r.language, stargazers_count: r.stargazers_count, forks_count: r.forks_count, open_issues_count: r.open_issues_count, html_url: r.html_url }).collect::<Vec<_>>();
            let out = SearchReposOutput{ items: Some(items), total_count: val.total_count, incomplete_results: val.incomplete_results, meta: Meta{ next_cursor, has_more, rate: resp.meta.rate }, error: None };
            let val = serde_json::to_value(out).unwrap();
            return (val, Some("search repositories".into()), false)
        } else {
            #[derive(Deserialize)] struct User { login: String }
            #[derive(Deserialize)] struct IssueItem { id: i64, number: i64, title: String, state: String, repository_url: String, user: Option<User>, created_at: String, updated_at: String, pull_request: Option<serde_json::Value> }
            #[derive(Deserialize)] struct Resp { total_count: i64, incomplete_results: bool, items: Vec<IssueItem> }
            let resp = http::rest_get_json::<Resp>(&client, &cfg, &path).await;
            if let Some(err) = resp.error { let v = serde_json::json!({"error": {"code": err.code, "message": err.message, "retriable": err.retriable}}); return (v, Some("search error".into()), true) }
            let has_more = resp.headers.as_ref().map(http::has_next_page_from_link).unwrap_or(false);
            let next_cursor = if has_more { Some(http::encode_rest_cursor(http::RestCursor{ page: page+1, per_page, path: None })) } else { None };
            let val = resp.value.unwrap();
            let items = val.items.iter().map(|it| SearchIssueItem{ id: it.id, number: it.number, title: it.title.clone(), state: it.state.clone(), repo_full_name: it.repository_url.split("/repos/").nth(1).unwrap_or("").to_string(), is_pull_request: it.pull_request.is_some(), author_login: it.user.as_ref().map(|u| u.login.clone()), created_at: it.created_at.clone(), updated_at: it.updated_at.clone() }).collect::<Vec<_>>();
            let out = SearchIssuesOutput{ items: Some(items), total_count: val.total_count, incomplete_results: val.incomplete_results, meta: Meta{ next_cursor, has_more, rate: resp.meta.rate }, error: None };
            let val = serde_json::to_value(out).unwrap();
            return (val, Some("search issues".into()), false)
        }
    });
    let wrapped = mcp_wrap(out_val, text, is_err);
    rpc_ok(id, wrapped)
}
fn handle_update_issue(id: Option<Id>, params: Value) -> Response {
    let input: UpdateIssueInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (item, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        #[derive(Serialize)]
        struct Body {
            title: Option<String>,
            body: Option<String>,
            labels: Option<Vec<String>>,
            assignees: Option<Vec<String>>,
            state: Option<String>,
            milestone: Option<i64>,
        }
        #[derive(Deserialize)]
        struct User {
            login: String,
        }
        #[derive(Deserialize)]
        struct Resp {
            id: i64,
            number: i64,
            title: String,
            body: Option<String>,
            state: String,
            assignees: Vec<User>,
            labels: Vec<serde_json::Value>,
            milestone: Option<serde_json::Value>,
            updated_at: String,
        }
        let path = format!(
            "/repos/{}/{}/issues/{}",
            input.owner, input.repo, input.number
        );
        let body = Body {
            title: input.title,
            body: input.body,
            labels: input.labels,
            assignees: input.assignees,
            state: input.state,
            milestone: input.milestone,
        };
        let resp = http::rest_patch_json::<Body, Resp>(&client, &cfg, &path, &body).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let r = resp.value.unwrap();
        let labels: Vec<String> = r
            .labels
            .into_iter()
            .filter_map(|lv| {
                lv.get("name")
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        let assignees = r.assignees.into_iter().map(|u| u.login).collect();
        let milestone = r
            .milestone
            .as_ref()
            .and_then(|m| m.get("number").and_then(|n| n.as_i64()));
        let item = UpdatedIssueItem {
            id: r.id,
            number: r.number,
            title: r.title,
            body: r.body,
            state: r.state,
            labels,
            assignees,
            milestone,
            updated_at: r.updated_at,
        };
        (
            Some(item),
            Meta {
                next_cursor: None,
                has_more: false,
                rate: resp.meta.rate,
            },
            None,
        )
    });
    let out = UpdateIssueOutput {
        item,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured.get("item").map(|_| "issue updated".to_string());
    let is_error = structured
        .get("error")
        .map(|e| !e.is_null())
        .unwrap_or(false);
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}
fn handle_update_pull_request(id: Option<Id>, params: Value) -> Response {
    let input: UpdatePullRequestInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (item, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        #[derive(Serialize)]
        struct Body {
            title: Option<String>,
            body: Option<String>,
            state: Option<String>,
            base: Option<String>,
            maintainer_can_modify: Option<bool>,
        }
        #[derive(Deserialize)]
        struct Resp {
            id: i64,
            number: i64,
            title: String,
            body: Option<String>,
            state: String,
            draft: bool,
            base: Base,
        }
        #[derive(Deserialize)]
        struct Base {
            #[serde(rename = "ref")]
            r#ref: String,
        }
        let path = format!(
            "/repos/{}/{}/pulls/{}",
            input.owner, input.repo, input.number
        );
        let body = Body {
            title: input.title,
            body: input.body,
            state: input.state,
            base: input.base,
            maintainer_can_modify: input.maintainer_can_modify,
        };
        let resp = http::rest_patch_json::<Body, Resp>(&client, &cfg, &path, &body).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let r = resp.value.unwrap();
        let item = UpdatedPrItem {
            id: r.id,
            number: r.number,
            title: r.title,
            body: r.body,
            state: r.state,
            is_draft: r.draft,
            base_ref: r.base.r#ref,
        };
        (
            Some(item),
            Meta {
                next_cursor: None,
                has_more: false,
                rate: resp.meta.rate,
            },
            None,
        )
    });
    let out = UpdatePullRequestOutput {
        item,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = Some("pull request updated".to_string());
    let is_error = structured
        .get("error")
        .map(|e| !e.is_null())
        .unwrap_or(false);
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}
fn handle_fork_repository(id: Option<Id>, params: Value) -> Response {
    let input: ForkRepositoryInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (item, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        #[derive(Serialize)]
        struct Body {
            organization: Option<String>,
        }
        #[derive(Deserialize)]
        struct Owner {
            login: String,
        }
        #[derive(Deserialize)]
        struct Resp {
            full_name: String,
            owner: Owner,
            private: bool,
            html_url: String,
            parent: Option<Parent>,
            created_at: String,
        }
        #[derive(Deserialize)]
        struct Parent {
            full_name: String,
        }
        let path = format!("/repos/{}/{}/forks", input.owner, input.repo);
        let body = Body {
            organization: input.organization,
        };
        let resp = http::rest_post_json::<Body, Resp>(&client, &cfg, &path, &body).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        let r = resp.value.unwrap();
        let item = ForkRepoItem {
            full_name: r.full_name,
            owner_login: r.owner.login,
            private: r.private,
            html_url: r.html_url,
            parent_full_name: r.parent.map(|p| p.full_name),
            created_at: r.created_at,
        };
        (
            Some(item),
            Meta {
                next_cursor: None,
                has_more: false,
                rate: resp.meta.rate,
            },
            None,
        )
    });
    let out = ForkRepositoryOutput {
        item,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("item")
        .and_then(|v| v.get("full_name"))
        .and_then(|v| v.as_str())
        .map(|s| format!("forked: {}", s));
    let is_error = structured
        .get("error")
        .map(|e| !e.is_null())
        .unwrap_or(false);
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_get_pr_text(id: Option<Id>, params: Value, is_diff: bool) -> Response {
    let input: GetPrTextInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (text, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) {
            Ok(c) => c,
            Err(e) => {
                return (
                    None,
                    Meta {
                        next_cursor: None,
                        has_more: false,
                        rate: None,
                    },
                    Some(ErrorShape {
                        code: "server_error".into(),
                        message: e.to_string(),
                        retriable: false,
                    }),
                )
            }
        };
        let path = format!(
            "/repos/{}/{}/pulls/{}",
            input.owner, input.repo, input.number
        );
        let accept = if is_diff {
            "application/vnd.github.v3.diff"
        } else {
            "application/vnd.github.v3.patch"
        };
        let resp = http::rest_get_text_with_accept(&client, &cfg, &path, accept).await;
        if let Some(err) = resp.error {
            return (
                None,
                Meta {
                    next_cursor: None,
                    has_more: false,
                    rate: resp.meta.rate,
                },
                Some(ErrorShape {
                    code: err.code,
                    message: err.message,
                    retriable: err.retriable,
                }),
            );
        }
        (
            resp.value,
            Meta {
                next_cursor: None,
                has_more: false,
                rate: resp.meta.rate,
            },
            None,
        )
    });
    let out = if is_diff {
        GetPrTextOutput {
            diff: text,
            patch: None,
            meta,
            error: err,
        }
    } else {
        GetPrTextOutput {
            diff: None,
            patch: text,
            meta,
            error: err,
        }
    };
    let structured = serde_json::to_value(&out).unwrap();
    // For diff/patch endpoints, set content text to the actual text body when available.
    let text = if is_diff {
        out.diff.clone()
    } else {
        out.patch.clone()
    };
    let is_error = structured
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_get_pr_status_summary(id: Option<Id>, params: Value) -> Response {
    #[derive(Deserialize)]
    struct Input {
        owner: String,
        repo: String,
        number: i64,
        include_failing_contexts: Option<bool>,
        limit_contexts: Option<u32>,
    }
    let input: Input = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let limit_contexts = input.limit_contexts.unwrap_or(10).min(100) as i64;
    let include_failing = input.include_failing_contexts.unwrap_or(false);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (summary, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false })) };
        let query = r#"
        query GetPrStatusSummary($owner: String!, $repo: String!, $number: Int!, $limit_contexts: Int = 10) {
          repository(owner: $owner, name: $repo) {
            pullRequest(number: $number) {
              commits(last: 1) { nodes { commit { oid statusCheckRollup { state contexts(first: $limit_contexts) { nodes { __typename ... on CheckRun { name conclusion } ... on StatusContext { context state } } } } } } }
            }
          }
        }
        "#;
        // These intermediates are kept for clarity; suppress dead_code as we match via ContextNode
        #[allow(dead_code)]
        #[derive(Deserialize)] struct CheckRun { name: String, conclusion: Option<String> }
        #[allow(dead_code)]
        #[derive(Deserialize)] struct StatusContext { context: String, state: Option<String> }
        #[derive(Deserialize)] struct ContextNode { __typename: String, #[serde(default)] name: Option<String>, #[serde(default)] conclusion: Option<String>, #[serde(default)] context: Option<String>, #[serde(default)] state: Option<String> }
        #[derive(Deserialize)] struct Contexts { nodes: Vec<ContextNode> }
        #[derive(Deserialize)] struct Rollup { #[allow(dead_code)] state: Option<String>, contexts: Option<Contexts> }
        #[derive(Deserialize)] struct Commit { statusCheckRollup: Option<Rollup> }
        #[derive(Deserialize)] struct CommitNode { commit: Commit }
        #[derive(Deserialize)] struct Commits { nodes: Vec<CommitNode> }
        #[derive(Deserialize)] struct PR { commits: Commits }
        #[derive(Deserialize)] struct Repo { pullRequest: Option<PR> }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number, "limit_contexts": limit_contexts });
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let pr = match data.and_then(|d| d.repository).and_then(|r| r.pullRequest) { Some(p) => p, None => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Pull request not found".into(), retriable: false })) };

        // Map union contexts
        let mut counts = (0,0,0); // success, pending, failure
        let mut failing: Vec<String> = Vec::new();
        if let Some(commit_node) = pr.commits.nodes.into_iter().next() {
            if let Some(rollup) = commit_node.commit.statusCheckRollup {
                if let Some(ctxs) = rollup.contexts { for n in ctxs.nodes { let (name_opt, state_opt) = if n.__typename == "CheckRun" { (n.name.clone(), n.conclusion.clone()) } else { (n.context.clone(), n.state.clone()) }; let state_up = state_opt.unwrap_or_default().to_uppercase(); match state_up.as_str() { "SUCCESS" | "NEUTRAL" => counts.0 += 1, "PENDING" | "QUEUED" | "IN_PROGRESS" => counts.1 += 1, "FAILURE" | "ERROR" | "CANCELLED" | "TIMED_OUT" => { counts.2 += 1; if include_failing { if let Some(nm) = name_opt { failing.push(nm); } } }, _ => {} } } }
            }
        }
        #[derive(Serialize)] struct Summary { overall_state: String, counts: Counts, #[serde(skip_serializing_if = "Option::is_none")] failing_contexts: Option<Vec<String>> }
        #[derive(Serialize)] struct Counts { success: i32, pending: i32, failure: i32 }
        let overall = if counts.2 > 0 { "FAILURE" } else if counts.1 > 0 { "PENDING" } else { "SUCCESS" };
        let summary = Summary { overall_state: overall.into(), counts: Counts { success: counts.0, pending: counts.1, failure: counts.2 }, failing_contexts: if include_failing { Some(failing) } else { None } };
        (Some(summary), Meta{ next_cursor: None, has_more: false, rate: None }, None)
    });
    let result = serde_json::to_value(summary).unwrap_or_else(|_| serde_json::json!({"overall_state":"SUCCESS","counts":{"success":0,"pending":0,"failure":0}}));
    let structured = serde_json::json!({"item": result, "meta": meta, "error": err});
    // Build a concise summary if possible
    let text = structured
        .get("item")
        .and_then(|i| i.get("counts"))
        .map(|c| {
            let s = c.get("success").and_then(|v| v.as_i64()).unwrap_or(0);
            let p = c.get("pending").and_then(|v| v.as_i64()).unwrap_or(0);
            let f = c.get("failure").and_then(|v| v.as_i64()).unwrap_or(0);
            format!("status: S={} P={} F={}", s, p, f)
        });
    let is_error = structured
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_list_pull_requests(id: Option<Id>, params: Value) -> Response {
    let input: ListPullRequestsInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false })) };
        let query = r#"
        query ListPullRequests($owner: String!, $repo: String!, $first: Int = 30, $after: String, $states: [PullRequestState!], $base: String, $head: String) {
          repository(owner: $owner, name: $repo) {
            pullRequests(first: $first, after: $after, states: $states, baseRefName: $base, headRefName: $head, orderBy: { field: UPDATED_AT, direction: DESC }) {
              nodes { id number title state createdAt updatedAt author { login } }
              pageInfo { hasNextPage endCursor }
            }
          }
          rateLimit { remaining used resetAt }
        }
        "#;
        #[derive(Deserialize)] struct Author { login: String }
        #[derive(Deserialize)] struct Node { id: String, number: i64, title: String, state: String, createdAt: String, updatedAt: String, author: Option<Author> }
        #[derive(Deserialize)] struct PageInfo { hasNextPage: bool, endCursor: Option<String> }
        #[derive(Deserialize)] struct PRs { nodes: Vec<Node>, pageInfo: PageInfo }
        #[derive(Deserialize)] struct Repo { pullRequests: PRs }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({
            "owner": input.owner,
            "repo": input.repo,
            "first": limit as i64,
            "after": input.cursor,
            "states": input.state.map(|s| vec![s.to_uppercase()]),
            "base": input.base,
            "head": input.head,
        });
        let (data, gql_meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let repo = match data.and_then(|d| d.repository) { Some(r) => r, None => return (None, Meta { next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Repository not found".into(), retriable: false })) };
        let include_author = input.include_author.unwrap_or(false);
        let items: Vec<ListPullRequestsItem> = repo.pullRequests.nodes.into_iter().map(|n| ListPullRequestsItem{
            id: n.id,
            number: n.number,
            title: n.title,
            state: n.state,
            created_at: n.createdAt,
            updated_at: n.updatedAt,
            author_login: if include_author { n.author.map(|a| a.login) } else { None },
        }).collect();
        let meta = Meta { next_cursor: repo.pullRequests.pageInfo.endCursor, has_more: repo.pullRequests.pageInfo.hasNextPage, rate: gql_meta.rate };
        (Some(items), meta, None)
    });
    let out = ListPullRequestsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("items")
        .and_then(|v| v.as_array())
        .map(|v| format!("{} pull requests", v.len()));
    let is_error = structured
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_get_pull_request(id: Option<Id>, params: Value) -> Response {
    let input: GetPullRequestInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (item, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false })) };
        let query = r#"
        query GetPullRequest($owner: String!, $repo: String!, $number: Int!) {
          repository(owner: $owner, name: $repo) {
            pullRequest(number: $number) {
              id number title body state isDraft merged mergedAt createdAt updatedAt author { login }
            }
          }
          rateLimit { remaining used resetAt }
        }
        "#;
        #[derive(Deserialize)] struct Author { login: String }
        #[derive(Deserialize)] struct PR { id: String, number: i64, title: String, body: Option<String>, state: String, isDraft: bool, merged: bool, mergedAt: Option<String>, createdAt: String, updatedAt: String, author: Option<Author> }
        #[derive(Deserialize)] struct Repo { pullRequest: Option<PR> }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number });
        let (data, gql_meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let pr = match data.and_then(|d| d.repository).and_then(|r| r.pullRequest) { Some(p) => p, None => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Pull request not found".into(), retriable: false })) };
        let include_author = input.include_author.unwrap_or(false);
        let item = GetPullRequestItem {
            id: pr.id,
            number: pr.number,
            title: pr.title,
            body: pr.body,
            state: pr.state,
            is_draft: pr.isDraft,
            created_at: pr.createdAt,
            updated_at: pr.updatedAt,
            merged: pr.merged,
            merged_at: pr.mergedAt,
            author_login: if include_author { pr.author.map(|a| a.login) } else { None },
        };
        (Some(item), Meta{ next_cursor: None, has_more: false, rate: gql_meta.rate }, None)
    });
    let out = GetPullRequestOutput {
        item,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("item")
        .and_then(|i| i.get("number"))
        .and_then(|n| n.as_i64())
        .zip(
            structured
                .get("item")
                .and_then(|i| i.get("state"))
                .and_then(|s| s.as_str().map(|s| s.to_string())),
        )
        .map(|(n, s)| format!("PR #{} {}", n, s));
    let is_error = structured
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_get_issue(id: Option<Id>, params: Value) -> Response {
    let input: GetIssueInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (item, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false })) };
        let query = r#"
        query GetIssue($owner: String!, $repo: String!, $number: Int!) {
          repository(owner: $owner, name: $repo) {
            issue(number: $number) { id number title body state createdAt updatedAt author { login } }
          }
        }
        "#;
        #[derive(Deserialize)]
        struct Author { login: String }
        #[derive(Deserialize)]
        struct Issue { id: String, number: i64, title: String, body: Option<String>, state: String, createdAt: String, updatedAt: String, author: Option<Author> }
        #[derive(Deserialize)]
        struct Repo { issue: Option<Issue> }
        #[derive(Deserialize)]
        struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number });
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let issue = match data.and_then(|d| d.repository).and_then(|r| r.issue) { Some(i) => i, None => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Issue not found".into(), retriable: false })) };
        let include_author = input.include_author.unwrap_or(false);
        let item = GetIssueOutputItem{
            id: issue.id,
            number: issue.number,
            title: issue.title,
            body: issue.body,
            state: issue.state,
            created_at: issue.createdAt,
            updated_at: issue.updatedAt,
            author_login: if include_author { issue.author.map(|a| a.login) } else { None },
        };
        (Some(item), Meta{ next_cursor: None, has_more: false, rate: None }, None)
    });
    let out = GetIssueOutput {
        item,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("item")
        .and_then(|i| i.get("number"))
        .and_then(|n| n.as_i64())
        .zip(
            structured
                .get("item")
                .and_then(|i| i.get("state"))
                .and_then(|s| s.as_str().map(|s| s.to_string())),
        )
        .map(|(n, s)| format!("Issue #{} {}", n, s));
    let is_error = structured
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

fn handle_list_issue_comments(id: Option<Id>, params: Value) -> Response {
    let input: ListIssueCommentsInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None),
    };
    let Ok(limit) = enforce_limit(input.limit) else {
        return rpc_error(id, -32602, "Invalid limit (1..=100)", None);
    };
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return rpc_error(id, -32603, &e, None),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false })) };
        let query = r#"
        query ListIssueComments($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
          repository(owner: $owner, name: $repo) {
            issue(number: $number) {
              comments(first: $first, after: $after) {
                nodes { id body createdAt updatedAt author { login } }
                pageInfo { hasNextPage endCursor }
              }
            }
          }
        }
        "#;
        #[derive(Deserialize)] struct Author { login: String }
        #[derive(Deserialize)] struct Node { id: String, body: String, createdAt: String, updatedAt: String, author: Option<Author> }
        #[derive(Deserialize)] struct PageInfo { hasNextPage: bool, endCursor: Option<String> }
        #[derive(Deserialize)] struct Comments { nodes: Vec<Node>, pageInfo: PageInfo }
        #[derive(Deserialize)] struct Issue { comments: Comments }
        #[derive(Deserialize)] struct Repo { issue: Option<Issue> }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number, "first": limit as i64, "after": input.cursor });
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let issue = match data.and_then(|d| d.repository).and_then(|r| r.issue) { Some(i) => i, None => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Issue not found".into(), retriable: false })) };
        let include_author = input.include_author.unwrap_or(false);
        let items: Vec<ListIssueCommentsItem> = issue.comments.nodes.into_iter().map(|n| ListIssueCommentsItem{
            id: n.id,
            body: n.body,
            author_login: if include_author { n.author.map(|a| a.login) } else { None },
            created_at: n.createdAt,
            updated_at: n.updatedAt,
        }).collect();
        let meta = Meta { next_cursor: issue.comments.pageInfo.endCursor, has_more: issue.comments.pageInfo.hasNextPage, rate: None };
        (Some(items), meta, None)
    });
    let out = ListIssueCommentsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
    let text = structured
        .get("items")
        .and_then(|v| v.as_array())
        .map(|v| format!("{} comments", v.len()));
    let is_error = structured
        .get("error")
        .and_then(|e| if e.is_null() { None } else { Some(e) })
        .is_some();
    let wrapped = mcp_wrap(structured, text, is_error);
    rpc_ok(id, wrapped)
}

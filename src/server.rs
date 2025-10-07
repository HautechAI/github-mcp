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
use crate::mcp::mcp_wrap;
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
        "ping" => handle_ping(req.id, req.params),
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
    let tools = tool_descriptors();
    // Optional nicety: include nextCursor: null for future pagination compatibility
    rpc_ok(
        id,
        serde_json::json!({ "tools": tools, "nextCursor": null }),
    )
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
    match call.name.as_str() {
        "ping" => handle_ping(id, call.arguments),
        "list_issues" => handle_list_issues(id, call.arguments),
        "get_issue" => handle_get_issue(id, call.arguments),
        "list_issue_comments_plain" => handle_list_issue_comments(id, call.arguments),
        "list_pull_requests" => handle_list_pull_requests(id, call.arguments),
        "get_pull_request" => handle_get_pull_request(id, call.arguments),
        "get_pr_status_summary" => handle_get_pr_status_summary(id, call.arguments),
        "list_pr_comments_plain" => handle_list_pr_comments(id, call.arguments),
        "list_pr_review_comments_plain" => handle_list_pr_review_comments(id, call.arguments),
        "list_pr_review_threads_light" => handle_list_pr_review_threads(id, call.arguments),
        "resolve_pr_review_thread" => handle_resolve_pr_review_thread(id, call.arguments),
        "unresolve_pr_review_thread" => handle_unresolve_pr_review_thread(id, call.arguments),
        "list_pr_reviews_light" => handle_list_pr_reviews(id, call.arguments),
        "list_pr_commits_light" => handle_list_pr_commits(id, call.arguments),
        "list_pr_files_light" => handle_list_pr_files(id, call.arguments),
        "get_pr_diff" => handle_get_pr_text(id, call.arguments, true),
        "get_pr_patch" => handle_get_pr_text(id, call.arguments, false),
        "list_workflows_light" => handle_list_workflows(id, call.arguments),
        "list_workflow_runs_light" => handle_list_workflow_runs(id, call.arguments),
        "get_workflow_run_light" => handle_get_workflow_run(id, call.arguments),
        "list_workflow_jobs_light" => handle_list_workflow_jobs(id, call.arguments),
        "get_workflow_job_logs" => handle_get_workflow_job_logs(id, call.arguments),
        "rerun_workflow_run" => handle_rerun_workflow_run(id, call.arguments),
        "rerun_workflow_run_failed" => handle_rerun_workflow_run_failed(id, call.arguments),
        "cancel_workflow_run" => handle_cancel_workflow_run(id, call.arguments),
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
        query ListIssues($owner: String!, $repo: String!, $first: Int = 30, $after: String, $states: [IssueState!], $filterBy: IssueFilters) {
          repository(owner: $owner, name: $repo) {
            issues(first: $first, after: $after, states: $states, filterBy: $filterBy) {
              nodes { id number title state createdAt updatedAt author { login } }
              pageInfo { hasNextPage endCursor }
            }
          }
          rateLimit { remaining used resetAt }
        }
        "#;
        let vars = serde_json::json!({
            "owner": input.owner,
            "repo": input.repo,
            "first": limit as i64,
            "after": input.cursor,
            "states": input.state.map(|s| vec![s.to_uppercase()]),
            "filterBy": {
                "labels": input.labels,
                "createdBy": input.creator,
                "mentioned": input.mentions,
                "assignee": input.assignee,
                "since": input.since,
                "orderBy": input.sort.map(|s| serde_json::json!({"field": s.to_uppercase(), "direction": input.direction.unwrap_or("desc".into()).to_uppercase()}))
            }
        });
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
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false })) };
        let query = r#"
        query ListPrReviewComments($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
          repository(owner: $owner, name: $repo) {
            pullRequest(number: $number) {
              reviewComments(first: $first, after: $after) {
                nodes {
                  id body createdAt updatedAt author { login }
                  path diffHunk line startLine side startSide originalLine originalStartLine
                  commit { oid } originalCommit { oid }
                  pullRequestReviewThread { path line startLine side startSide }
                }
                pageInfo { hasNextPage endCursor }
              }
            }
          }
          rateLimit { remaining used resetAt }
        }
        "#;
        #[derive(Deserialize)] struct Author { login: String }
        #[derive(Deserialize)] struct Commit { oid: String }
        #[derive(Deserialize)] struct Node { id: String, body: String, createdAt: String, updatedAt: String, author: Option<Author>, path: Option<String>, diffHunk: Option<String>, line: Option<i64>, startLine: Option<i64>, side: Option<String>, startSide: Option<String>, originalLine: Option<i64>, originalStartLine: Option<i64>, commit: Option<Commit>, originalCommit: Option<Commit>, pullRequestReviewThread: Option<ThreadLoc> }
        #[derive(Deserialize)] struct ThreadLoc { path: Option<String>, line: Option<i64>, startLine: Option<i64>, side: Option<String>, startSide: Option<String> }
        #[derive(Deserialize)] struct PageInfo { hasNextPage: bool, endCursor: Option<String> }
        #[derive(Deserialize)] struct RC { nodes: Vec<Node>, pageInfo: PageInfo }
        #[derive(Deserialize)] struct PR { reviewComments: RC }
        #[derive(Deserialize)] struct Repo { pullRequest: Option<PR> }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number, "first": limit as i64, "after": input.cursor });
        let (data, gql_meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let pr = match data.and_then(|d| d.repository).and_then(|r| r.pullRequest) { Some(p) => p, None => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Pull request not found".into(), retriable: false })) };
        let include_author = input.include_author.unwrap_or(false);
        let include_loc = input.include_location.unwrap_or(false);
        let items: Vec<ReviewCommentItem> = pr.reviewComments.nodes.into_iter().map(|n| {
            let t = n.pullRequestReviewThread.as_ref();
            ReviewCommentItem{
                id: n.id,
                body: n.body,
                created_at: n.createdAt,
                updated_at: n.updatedAt,
                author_login: if include_author { n.author.map(|a| a.login) } else { None },
                path: if include_loc { n.path.or_else(|| t.and_then(|tr| tr.path.clone())) } else { None },
                line: if include_loc { n.line.or_else(|| t.and_then(|tr| tr.line)) } else { None },
                start_line: if include_loc { n.startLine.or_else(|| t.and_then(|tr| tr.startLine)) } else { None },
                side: if include_loc { map_side(n.side.or_else(|| t.and_then(|tr| tr.side.clone()))) } else { None },
                start_side: if include_loc { map_side(n.startSide.or_else(|| t.and_then(|tr| tr.startSide.clone()))) } else { None },
                original_line: if include_loc { n.originalLine } else { None },
                original_start_line: if include_loc { n.originalStartLine } else { None },
                diff_hunk: if include_loc { n.diffHunk } else { None },
                commit_sha: if include_loc { n.commit.map(|c| c.oid) } else { None },
                original_commit_sha: if include_loc { n.originalCommit.map(|c| c.oid) } else { None },
            }
        }).collect();
        let meta = Meta { next_cursor: pr.reviewComments.pageInfo.endCursor, has_more: pr.reviewComments.pageInfo.hasNextPage, rate: gql_meta.rate };
        (Some(items), meta, None)
    });
    let out = ListPrReviewCommentsOutput {
        items,
        meta,
        error: err,
    };
    let structured = serde_json::to_value(&out).unwrap();
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
                  path line startLine side startSide
                }
                pageInfo { hasNextPage endCursor }
              }
            }
          }
          rateLimit { remaining used resetAt }
        }
        "#;
        #[derive(Deserialize)] struct Author { login: String }
        #[derive(Deserialize)] struct Node { id: String, isResolved: bool, isOutdated: bool, comments: Count, resolvedBy: Option<Author>, path: Option<String>, line: Option<i64>, startLine: Option<i64>, side: Option<String>, startSide: Option<String> }
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
            side: if include_loc { map_side(n.side) } else { None },
            start_side: if include_loc { map_side(n.startSide) } else { None },
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
    let input: ListPrFilesInput = match serde_json::from_value(params) {
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
        // Map REST pagination inputs
        let page = input.page.unwrap_or(1);
        let per_page = input.per_page.unwrap_or(30).min(100);
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
            Some(format!("page:{}", page + 1))
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
        .and_then(|c| {
            let s = c.get("success").and_then(|v| v.as_i64()).unwrap_or(0);
            let p = c.get("pending").and_then(|v| v.as_i64()).unwrap_or(0);
            let f = c.get("failure").and_then(|v| v.as_i64()).unwrap_or(0);
            Some(format!("status: S={} P={} F={}", s, p, f))
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

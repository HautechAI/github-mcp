use crate::config::Config;
use crate::http::{self};
use crate::tools::*;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Read, Write};
use uuid::Uuid;

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
    Response { jsonrpc: "2.0".into(), result: None, error: Some(RpcError { code, message: message.into(), data }), id }
}

fn rpc_ok(id: Option<Id>, result: Value) -> Response {
    Response { jsonrpc: "2.0".into(), result: Some(result), error: None, id }
}

pub fn run_stdio_server() -> anyhow::Result<()> {
    info!("Starting github-mcp stdio server; protocol={}", PROTOCOL_VERSION);
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    // Note: for milestone 1, we accept a single request for simplicity; future work can stream.
    if input.trim().is_empty() {
        return Ok(());
    }
    let req: Request = match serde_json::from_str(&input) {
        Ok(r) => r,
        Err(e) => {
            let resp = rpc_error(None, -32700, &format!("Parse error: {}", e), None);
            write_response(&resp)?;
            return Ok(());
        }
    };
    debug!("Received method={}", req.method);
    let resp = dispatch(req);
    write_response(&resp)?;
    Ok(())
}

fn write_response(resp: &Response) -> anyhow::Result<()> {
    let mut out = io::stdout();
    let payload = serde_json::to_string(resp)?;
    writeln!(out, "{}", payload)?;
    out.flush()?;
    Ok(())
}

fn dispatch(req: Request) -> Response {
    match req.method.as_str() {
        "initialize" => handle_initialize(req.id),
        "tools/list" => handle_tools_list(req.id),
        "tools/call" => handle_tools_call(req.id, req.params),
        "ping" => handle_ping(req.id, req.params),
        other => rpc_error(req.id, -32601, &format!("Method not found: {}", other), None),
    }
}

fn handle_initialize(id: Option<Id>) -> Response {
    rpc_ok(
        id,
        serde_json::json!({
            "server": {
                "name": "github-mcp",
                "version": env!("CARGO_PKG_VERSION"),
                "protocol": PROTOCOL_VERSION,
            }
        }),
    )
}

fn handle_tools_list(id: Option<Id>) -> Response {
    let tools = tool_descriptors();
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
        _ => rpc_error(id, -32601, &format!("Tool not found: {}", call.name), None),
    }
}

fn handle_ping(id: Option<Id>, params: Value) -> Response {
    let input: PingInput = match serde_json::from_value(params) {
        Ok(v) => v,
        Err(_) => PingInput { message: None },
    };
    let message = input.message.unwrap_or_else(|| "pong".to_string());
    rpc_ok(id, serde_json::to_value(PingOutput { message }).unwrap())
}

fn enforce_limit(limit: Option<u32>) -> Result<u32, String> {
    let l = limit.unwrap_or(30);
    if l == 0 || l > 100 { return Err("limit must be 1..=100".into()); }
    Ok(l)
}

#[derive(Deserialize)]
struct ListIssuesVars {
    owner: String,
    repo: String,
    first: i64,
    after: Option<String>,
    states: Option<Vec<String>>,
    filterBy: Option<serde_json::Value>,
}

fn handle_list_issues(id: Option<Id>, params: Value) -> Response {
    let input: ListIssuesInput = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let Ok(limit) = enforce_limit(input.limit) else { return rpc_error(id, -32602, "Invalid limit (1..=100)", None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
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
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
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
        let meta = Meta { next_cursor: repo.issues.pageInfo.endCursor, has_more: repo.issues.pageInfo.hasNextPage, rate: None };
        (Some(items), meta, None)
    });
    let out = ListIssuesOutput { items, meta, error: err };
    rpc_ok(id, serde_json::to_value(out).unwrap())
}

fn handle_list_pr_comments(id: Option<Id>, params: Value) -> Response {
    let input: ListPrCommentsInput = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let Ok(limit) = enforce_limit(input.limit) else { return rpc_error(id, -32602, "Invalid limit (1..=100)", None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
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
        }
        "#;
        #[derive(Deserialize)] struct Author { login: String }
        #[derive(Deserialize)] struct Node { id: String, body: String, createdAt: String, updatedAt: String, author: Option<Author> }
        #[derive(Deserialize)] struct Comments { nodes: Vec<Node>, pageInfo: PageInfo }
        #[derive(Deserialize)] struct PR { comments: Comments }
        #[derive(Deserialize)] struct Repo { pullRequest: Option<PR> }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number, "first": limit as i64, "after": input.cursor });
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let pr = match data.and_then(|d| d.repository).and_then(|r| r.pullRequest) { Some(p) => p, None => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Pull request not found".into(), retriable: false })) };
        let include_author = input.include_author.unwrap_or(false);
        let items: Vec<PlainComment> = pr.comments.nodes.into_iter().map(|n| PlainComment{
            id: n.id, body: n.body, created_at: n.createdAt, updated_at: n.updatedAt,
            author_login: if include_author { n.author.map(|a| a.login) } else { None },
        }).collect();
        let meta = Meta { next_cursor: pr.comments.pageInfo.endCursor, has_more: pr.comments.pageInfo.hasNextPage, rate: None };
        (Some(items), meta, None)
    });
    let out = ListPrCommentsOutput { items, meta, error: err };
    rpc_ok(id, serde_json::to_value(out).unwrap())
}

fn map_side(s: Option<String>) -> Option<String> { s.map(|x| x.to_uppercase()) }

fn handle_list_pr_review_comments(id: Option<Id>, params: Value) -> Response {
    let input: ListPrReviewCommentsInput = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let Ok(limit) = enforce_limit(input.limit) else { return rpc_error(id, -32602, "Invalid limit (1..=100)", None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
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
        }
        "#;
        #[derive(Deserialize)] struct Author { login: String }
        #[derive(Deserialize)] struct Commit { oid: String }
        #[derive(Deserialize)] struct Node { id: String, body: String, createdAt: String, updatedAt: String, author: Option<Author>, path: Option<String>, diffHunk: Option<String>, line: Option<i64>, startLine: Option<i64>, side: Option<String>, startSide: Option<String>, originalLine: Option<i64>, originalStartLine: Option<i64>, commit: Option<Commit>, originalCommit: Option<Commit>, pullRequestReviewThread: Option<ThreadLoc> }
        #[derive(Deserialize)] struct ThreadLoc { path: Option<String>, line: Option<i64>, startLine: Option<i64>, side: Option<String>, startSide: Option<String> }
        #[derive(Deserialize)] struct RC { nodes: Vec<Node>, pageInfo: PageInfo }
        #[derive(Deserialize)] struct PR { reviewComments: RC }
        #[derive(Deserialize)] struct Repo { pullRequest: Option<PR> }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number, "first": limit as i64, "after": input.cursor });
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let pr = match data.and_then(|d| d.repository).and_then(|r| r.pullRequest) { Some(p) => p, None => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Pull request not found".into(), retriable: false })) };
        let include_author = input.include_author.unwrap_or(false);
        let include_loc = input.include_location.unwrap_or(false);
        let items: Vec<ReviewCommentItem> = pr.reviewComments.nodes.into_iter().map(|n| ReviewCommentItem{
            id: n.id, body: n.body, created_at: n.createdAt, updated_at: n.updatedAt,
            author_login: if include_author { n.author.map(|a| a.login) } else { None },
            path: if include_loc { n.path.or_else(|| n.pullRequestReviewThread.and_then(|t| t.path)) } else { None },
            line: if include_loc { n.line.or_else(|| n.pullRequestReviewThread.as_ref().and_then(|t| t.line)) } else { None },
            start_line: if include_loc { n.startLine.or_else(|| n.pullRequestReviewThread.as_ref().and_then(|t| t.startLine)) } else { None },
            side: if include_loc { map_side(n.side.or_else(|| n.pullRequestReviewThread.and_then(|t| t.side))) } else { None },
            start_side: if include_loc { map_side(n.startSide.or_else(|| n.pullRequestReviewThread.and_then(|t| t.startSide))) } else { None },
            original_line: if include_loc { n.originalLine } else { None },
            original_start_line: if include_loc { n.originalStartLine } else { None },
            diff_hunk: if include_loc { n.diffHunk } else { None },
            commit_sha: if include_loc { n.commit.map(|c| c.oid) } else { None },
            original_commit_sha: if include_loc { n.originalCommit.map(|c| c.oid) } else { None },
        }).collect();
        let meta = Meta { next_cursor: pr.reviewComments.pageInfo.endCursor, has_more: pr.reviewComments.pageInfo.hasNextPage, rate: None };
        (Some(items), meta, None)
    });
    let out = ListPrReviewCommentsOutput { items, meta, error: err };
    rpc_ok(id, serde_json::to_value(out).unwrap())
}

fn handle_list_pr_review_threads(id: Option<Id>, params: Value) -> Response {
    let input: ListPrReviewThreadsInput = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let Ok(limit) = enforce_limit(input.limit) else { return rpc_error(id, -32602, "Invalid limit (1..=100)", None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
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
        }
        "#;
        #[derive(Deserialize)] struct Author { login: String }
        #[derive(Deserialize)] struct Node { id: String, isResolved: bool, isOutdated: bool, comments: Count, resolvedBy: Option<Author>, path: Option<String>, line: Option<i64>, startLine: Option<i64>, side: Option<String>, startSide: Option<String> }
        #[derive(Deserialize)] struct Count { totalCount: i64 }
        #[derive(Deserialize)] struct Threads { nodes: Vec<Node>, pageInfo: PageInfo }
        #[derive(Deserialize)] struct PR { reviewThreads: Threads }
        #[derive(Deserialize)] struct Repo { pullRequest: Option<PR> }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number, "first": limit as i64, "after": input.cursor });
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
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
        let meta = Meta { next_cursor: pr.reviewThreads.pageInfo.endCursor, has_more: pr.reviewThreads.pageInfo.hasNextPage, rate: None };
        (Some(items), meta, None)
    });
    let out = ListPrReviewThreadsOutput { items, meta, error: err };
    rpc_ok(id, serde_json::to_value(out).unwrap())
}

fn handle_resolve_pr_review_thread(id: Option<Id>, params: Value) -> Response {
    let input: ResolveThreadInput = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (ok, meta, err, is_resolved) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (false, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false }), false) };
        let query = r#"
        mutation ResolvePrReviewThread($thread_id: ID!) {
          resolveReviewThread(input: { threadId: $thread_id }) { thread { id isResolved } }
        }
        "#;
        #[derive(Deserialize)] struct Thread { id: String, isResolved: bool }
        #[derive(Deserialize)] struct Resp { resolveReviewThread: Option<Resolved> }
        #[derive(Deserialize)] struct Resolved { thread: Thread }
        let vars = serde_json::json!({ "thread_id": input.thread_id });
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Resp, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (false, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable }), false) }
        let is_resolved = data.and_then(|d| d.resolveReviewThread).map(|x| x.thread.isResolved).unwrap_or(false);
        (true, Meta{ next_cursor: None, has_more: false, rate: None }, None, is_resolved)
    });
    let out = ResolveThreadOutput { ok, thread_id: input.thread_id, is_resolved, meta, error: err };
    rpc_ok(id, serde_json::to_value(out).unwrap())
}

fn handle_unresolve_pr_review_thread(id: Option<Id>, params: Value) -> Response {
    let input: ResolveThreadInput = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (ok, meta, err, is_resolved) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (false, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false }), false) };
        let query = r#"
        mutation UnresolvePrReviewThread($thread_id: ID!) {
          unresolveReviewThread(input: { threadId: $thread_id }) { thread { id isResolved } }
        }
        "#;
        #[derive(Deserialize)] struct Thread { id: String, isResolved: bool }
        #[derive(Deserialize)] struct Resp { unresolveReviewThread: Option<Resolved> }
        #[derive(Deserialize)] struct Resolved { thread: Thread }
        let vars = serde_json::json!({ "thread_id": input.thread_id });
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Resp, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (false, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable }), false) }
        let is_resolved = data.and_then(|d| d.unresolveReviewThread).map(|x| x.thread.isResolved).unwrap_or(false);
        (true, Meta{ next_cursor: None, has_more: false, rate: None }, None, is_resolved)
    });
    let out = ResolveThreadOutput { ok, thread_id: input.thread_id, is_resolved, meta, error: err };
    rpc_ok(id, serde_json::to_value(out).unwrap())
}

fn handle_list_pr_reviews(id: Option<Id>, params: Value) -> Response {
    let input: ListPrReviewsInput = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let Ok(limit) = enforce_limit(input.limit) else { return rpc_error(id, -32602, "Invalid limit (1..=100)", None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
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
        }
        "#;
        #[derive(Deserialize)] struct Author { login: String }
        #[derive(Deserialize)] struct Node { id: String, state: String, submittedAt: Option<String>, author: Option<Author> }
        #[derive(Deserialize)] struct Reviews { nodes: Vec<Node>, pageInfo: PageInfo }
        #[derive(Deserialize)] struct PR { reviews: Reviews }
        #[derive(Deserialize)] struct Repo { pullRequest: Option<PR> }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number, "first": limit as i64, "after": input.cursor });
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let pr = match data.and_then(|d| d.repository).and_then(|r| r.pullRequest) { Some(p) => p, None => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Pull request not found".into(), retriable: false })) };
        let include_author = input.include_author.unwrap_or(false);
        let items: Vec<PrReviewItem> = pr.reviews.nodes.into_iter().map(|n| PrReviewItem{
            id: n.id, state: n.state, submitted_at: n.submittedAt, author_login: if include_author { n.author.map(|a| a.login) } else { None }
        }).collect();
        let meta = Meta { next_cursor: pr.reviews.pageInfo.endCursor, has_more: pr.reviews.pageInfo.hasNextPage, rate: None };
        (Some(items), meta, None)
    });
    let out = ListPrReviewsOutput { items, meta, error: err };
    rpc_ok(id, serde_json::to_value(out).unwrap())
}

fn handle_list_pr_commits(id: Option<Id>, params: Value) -> Response {
    let input: ListPrCommitsInput = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let Ok(limit) = enforce_limit(input.limit) else { return rpc_error(id, -32602, "Invalid limit (1..=100)", None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
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
        }
        "#;
        #[derive(Deserialize)] struct User { login: String }
        #[derive(Deserialize)] struct CommitAuthor { user: Option<User> }
        #[derive(Deserialize)] struct Commit { oid: String, messageHeadline: String, authoredDate: String, author: Option<CommitAuthor> }
        #[derive(Deserialize)] struct Node { commit: Commit }
        #[derive(Deserialize)] struct Commits { nodes: Vec<Node>, pageInfo: PageInfo }
        #[derive(Deserialize)] struct PR { commits: Commits }
        #[derive(Deserialize)] struct Repo { pullRequest: Option<PR> }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number, "first": limit as i64, "after": input.cursor });
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
        if let Some(e) = err { return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: e.code, message: e.message, retriable: e.retriable })) }
        let pr = match data.and_then(|d| d.repository).and_then(|r| r.pullRequest) { Some(p) => p, None => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "not_found".into(), message: "Pull request not found".into(), retriable: false })) };
        let include_author = input.include_author.unwrap_or(false);
        let items: Vec<PrCommitItem> = pr.commits.nodes.into_iter().map(|n| PrCommitItem{
            sha: n.commit.oid,
            title: n.commit.messageHeadline,
            authored_at: n.commit.authoredDate,
            author_login: if include_author { n.commit.author.and_then(|a| a.user.map(|u| u.login)) } else { None },
        }).collect();
        let meta = Meta { next_cursor: pr.commits.pageInfo.endCursor, has_more: pr.commits.pageInfo.hasNextPage, rate: None };
        (Some(items), meta, None)
    });
    let out = ListPrCommitsOutput { items, meta, error: err };
    rpc_ok(id, serde_json::to_value(out).unwrap())
}

fn handle_list_pr_files(id: Option<Id>, params: Value) -> Response {
    let input: ListPrFilesInput = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (items, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false })) };
        // Map REST pagination inputs
        let page = input.page.unwrap_or(1);
        let per_page = input.per_page.unwrap_or(30).min(100);
        let path = format!("/repos/{}/{}/pulls/{}/files?per_page={}&page={}", input.owner, input.repo, input.number, per_page, page);
        #[derive(Deserialize)] struct File { filename: String, status: String, additions: i64, deletions: i64, changes: i64, sha: String, patch: Option<String> }
        let resp = http::rest_get_json::<Vec<File>>(&client, &cfg, &path).await;
        if let Some(err) = resp.error { return (None, Meta{ next_cursor: None, has_more: false, rate: resp.meta.rate }, Some(ErrorShape{ code: err.code, message: err.message, retriable: err.retriable })) }
        let rate = resp.meta.rate;
        let has_more = http::has_next_page_from_link(&HeaderMap::new()); // placeholder; http::rest_get_json does not return headers
        let next_cursor = if has_more { Some(format!("page:{}", page + 1)) } else { None };
        let include_patch = input.include_patch.unwrap_or(false);
        let items: Vec<PrFileItem> = resp.value.unwrap_or_default().into_iter().map(|f| PrFileItem{
            filename: f.filename,
            status: f.status,
            additions: f.additions,
            deletions: f.deletions,
            changes: f.changes,
            sha: f.sha,
            patch: if include_patch { f.patch } else { None },
        }).collect();
        (Some(items), Meta{ next_cursor, has_more, rate }, None)
    });
    let out = ListPrFilesOutput { items, meta, error: err };
    rpc_ok(id, serde_json::to_value(out).unwrap())
}

fn handle_get_pr_text(id: Option<Id>, params: Value, is_diff: bool) -> Response {
    let input: GetPrTextInput = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (text, meta, err) = rt.block_on(async move {
        let client = match http::build_client(&cfg) { Ok(c) => c, Err(e) => return (None, Meta{ next_cursor: None, has_more: false, rate: None }, Some(ErrorShape{ code: "server_error".into(), message: e.to_string(), retriable: false })) };
        let path = format!("/repos/{}/{}/pulls/{}", input.owner, input.repo, input.number);
        let accept = if is_diff { "application/vnd.github.v3.diff" } else { "application/vnd.github.v3.patch" };
        let resp = http::rest_get_text_with_accept(&client, &cfg, &path, accept).await;
        if let Some(err) = resp.error { return (None, Meta{ next_cursor: None, has_more: false, rate: resp.meta.rate }, Some(ErrorShape{ code: err.code, message: err.message, retriable: err.retriable })) }
        (resp.value, Meta{ next_cursor: None, has_more: false, rate: resp.meta.rate }, None)
    });
    let out = if is_diff {
        GetPrTextOutput { diff: text, patch: None, meta, error: err }
    } else {
        GetPrTextOutput { diff: None, patch: text, meta, error: err }
    };
    rpc_ok(id, serde_json::to_value(out).unwrap())
}

fn handle_get_pr_status_summary(id: Option<Id>, params: Value) -> Response {
    #[derive(Deserialize)] struct Input { owner: String, repo: String, number: i64, include_failing_contexts: Option<bool>, limit_contexts: Option<u32> }
    let input: Input = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
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
        #[derive(Deserialize)] struct CheckRun { name: String, conclusion: Option<String> }
        #[derive(Deserialize)] struct StatusContext { context: String, state: Option<String> }
        #[derive(Deserialize)] struct ContextNode { __typename: String, #[serde(default)] name: Option<String>, #[serde(default)] conclusion: Option<String>, #[serde(default)] context: Option<String>, #[serde(default)] state: Option<String> }
        #[derive(Deserialize)] struct Contexts { nodes: Vec<ContextNode> }
        #[derive(Deserialize)] struct Rollup { state: Option<String>, contexts: Option<Contexts> }
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
    rpc_ok(id, serde_json::json!({"item": result, "meta": meta, "error": err}))
}

fn handle_list_pull_requests(id: Option<Id>, params: Value) -> Response {
    let input: ListPullRequestsInput = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let Ok(limit) = enforce_limit(input.limit) else { return rpc_error(id, -32602, "Invalid limit (1..=100)", None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
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
        }
        "#;
        #[derive(Deserialize)] struct Author { login: String }
        #[derive(Deserialize)] struct Node { id: String, number: i64, title: String, state: String, createdAt: String, updatedAt: String, author: Option<Author> }
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
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
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
        let meta = Meta { next_cursor: repo.pullRequests.pageInfo.endCursor, has_more: repo.pullRequests.pageInfo.hasNextPage, rate: None };
        (Some(items), meta, None)
    });
    let out = ListPullRequestsOutput { items, meta, error: err };
    rpc_ok(id, serde_json::to_value(out).unwrap())
}

fn handle_get_pull_request(id: Option<Id>, params: Value) -> Response {
    let input: GetPullRequestInput = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
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
        }
        "#;
        #[derive(Deserialize)] struct Author { login: String }
        #[derive(Deserialize)] struct PR { id: String, number: i64, title: String, body: Option<String>, state: String, isDraft: bool, merged: bool, mergedAt: Option<String>, createdAt: String, updatedAt: String, author: Option<Author> }
        #[derive(Deserialize)] struct Repo { pullRequest: Option<PR> }
        #[derive(Deserialize)] struct Data { repository: Option<Repo> }
        let vars = serde_json::json!({ "owner": input.owner, "repo": input.repo, "number": input.number });
        let (data, _meta, err) = http::graphql_post::<serde_json::Value, Data, serde_json::Value>(&client, &cfg, query, &vars).await;
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
        (Some(item), Meta{ next_cursor: None, has_more: false, rate: None }, None)
    });
    let out = GetPullRequestOutput { item, meta, error: err };
    rpc_ok(id, serde_json::to_value(out).unwrap())
}

fn handle_get_issue(id: Option<Id>, params: Value) -> Response {
    let input: GetIssueInput = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
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
    let out = GetIssueOutput { item, meta, error: err };
    rpc_ok(id, serde_json::to_value(out).unwrap())
}

fn handle_list_issue_comments(id: Option<Id>, params: Value) -> Response {
    let input: ListIssueCommentsInput = match serde_json::from_value(params) { Ok(v) => v, Err(e) => return rpc_error(id, -32602, &format!("Invalid params: {}", e), None) };
    let Ok(limit) = enforce_limit(input.limit) else { return rpc_error(id, -32602, "Invalid limit (1..=100)", None) };
    let cfg = match Config::from_env() { Ok(c) => c, Err(e) => return rpc_error(id, -32603, &e, None) };
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
        #[derive(Deserialize)] struct Node { id: String, body: String, createdAt: String, updatedAt: String, author: Option<Author> }
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
    let out = ListIssueCommentsOutput { items, meta, error: err };
    rpc_ok(id, serde_json::to_value(out).unwrap())
}

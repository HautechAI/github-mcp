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

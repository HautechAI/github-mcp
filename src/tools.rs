use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: &str = "2024-11-01"; // align with codex-tools-mcp cadence

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

pub fn tool_descriptors() -> Vec<ToolDescriptor> {
    // Milestone 4 will append real GitHub tools; for now includes ping and Issues tools.
    let ping = ToolDescriptor {
        name: "ping".into(),
        description: "Health check; echoes a message.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "message": {"type": "string"}
            }
        }),
    };

    let list_issues = ToolDescriptor {
        name: "list_issues".into(),
        description: "List issues in a repository".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "owner": {"type": "string"},
                "repo": {"type": "string"},
                "state": {"type": "string", "enum": ["open", "closed", "all"]},
                "labels": {"type": "array", "items": {"type": "string"}},
                "creator": {"type": "string"},
                "assignee": {"type": "string"},
                "mentions": {"type": "string"},
                "since": {"type": "string"},
                "sort": {"type": "string", "enum": ["created", "updated", "comments"]},
                "direction": {"type": "string", "enum": ["asc", "desc"]},
                "cursor": {"type": "string"},
                "limit": {"type": "integer"},
                "include_author": {"type": "boolean"}
            },
            "required": ["owner", "repo"]
        }),
    };

    let get_issue = ToolDescriptor {
        name: "get_issue".into(),
        description: "Get a single issue by number".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "owner": {"type": "string"},
                "repo": {"type": "string"},
                "number": {"type": "integer"},
                "include_author": {"type": "boolean"}
            },
            "required": ["owner", "repo", "number"]
        }),
    };

    let list_issue_comments_plain = ToolDescriptor {
        name: "list_issue_comments_plain".into(),
        description: "List issue comments (plain)".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "owner": {"type": "string"},
                "repo": {"type": "string"},
                "number": {"type": "integer"},
                "cursor": {"type": "string"},
                "limit": {"type": "integer"},
                "include_author": {"type": "boolean"}
            },
            "required": ["owner", "repo", "number"]
        }),
    };

    let list_prs = ToolDescriptor {
        name: "list_pull_requests".into(),
        description: "List pull requests".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "owner": {"type": "string"},
                "repo": {"type": "string"},
                "state": {"type": "string", "enum": ["open","closed","all"]},
                "base": {"type": "string"},
                "head": {"type": "string"},
                "cursor": {"type": "string"},
                "limit": {"type": "integer"},
                "include_author": {"type": "boolean"}
            },
            "required": ["owner", "repo"]
        }),
    };

    let get_pr = ToolDescriptor {
        name: "get_pull_request".into(),
        description: "Get a single PR".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "owner": {"type": "string"},
                "repo": {"type": "string"},
                "number": {"type": "integer"},
                "include_author": {"type": "boolean"}
            },
            "required": ["owner", "repo", "number"]
        }),
    };

    let list_pr_comments = ToolDescriptor {
        name: "list_pr_comments_plain".into(),
        description: "List PR issue comments (plain)".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties": {"owner":{"type":"string"},"repo":{"type":"string"},"number":{"type":"integer"},"cursor":{"type":"string"},"limit":{"type":"integer"},"include_author":{"type":"boolean"}},
            "required":["owner","repo","number"]
        }),
    };

    let list_pr_review_comments = ToolDescriptor {
        name: "list_pr_review_comments_plain".into(),
        description: "List PR review comments (plain)".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties": {"owner":{"type":"string"},"repo":{"type":"string"},"number":{"type":"integer"},"cursor":{"type":"string"},"limit":{"type":"integer"},"include_author":{"type":"boolean"},"include_location":{"type":"boolean"}},
            "required":["owner","repo","number"]
        }),
    };

    let list_pr_review_threads = ToolDescriptor {
        name: "list_pr_review_threads_light".into(),
        description: "List PR review threads (light)".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties": {"owner":{"type":"string"},"repo":{"type":"string"},"number":{"type":"integer"},"cursor":{"type":"string"},"limit":{"type":"integer"},"include_author":{"type":"boolean"},"include_location":{"type":"boolean"}},
            "required":["owner","repo","number"]
        }),
    };

    let resolve_thread = ToolDescriptor {
        name: "resolve_pr_review_thread".into(),
        description: "Resolve a PR review thread".into(),
        input_schema: serde_json::json!({"type":"object","additionalProperties":false,"properties":{"thread_id":{"type":"string"}},"required":["thread_id"]}),
    };

    let unresolve_thread = ToolDescriptor {
        name: "unresolve_pr_review_thread".into(),
        description: "Unresolve a PR review thread".into(),
        input_schema: serde_json::json!({"type":"object","additionalProperties":false,"properties":{"thread_id":{"type":"string"}},"required":["thread_id"]}),
    };

    let list_pr_reviews = ToolDescriptor {
        name: "list_pr_reviews_light".into(),
        description: "List PR reviews (light)".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties": {"owner":{"type":"string"},"repo":{"type":"string"},"number":{"type":"integer"},"cursor":{"type":"string"},"limit":{"type":"integer"},"include_author":{"type":"boolean"}},
            "required":["owner","repo","number"]
        }),
    };

    let list_pr_commits = ToolDescriptor {
        name: "list_pr_commits_light".into(),
        description: "List PR commits (light)".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties": {"owner":{"type":"string"},"repo":{"type":"string"},"number":{"type":"integer"},"cursor":{"type":"string"},"limit":{"type":"integer"},"include_author":{"type":"boolean"}},
            "required":["owner","repo","number"]
        }),
    };

    let list_pr_files = ToolDescriptor {
        name: "list_pr_files_light".into(),
        description: "List PR files (REST)".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties": {"owner":{"type":"string"},"repo":{"type":"string"},"number":{"type":"integer"},"page":{"type":"integer"},"per_page":{"type":"integer"},"include_patch":{"type":"boolean"}},
            "required":["owner","repo","number"]
        }),
    };

    let get_pr_diff = ToolDescriptor {
        name: "get_pr_diff".into(),
        description: "Get PR diff (REST)".into(),
        input_schema: serde_json::json!({"type":"object","additionalProperties":false,"properties":{"owner":{"type":"string"},"repo":{"type":"string"},"number":{"type":"integer"}},"required":["owner","repo","number"]}),
    };
    let get_pr_patch = ToolDescriptor {
        name: "get_pr_patch".into(),
        description: "Get PR patch (REST)".into(),
        input_schema: serde_json::json!({"type":"object","additionalProperties":false,"properties":{"owner":{"type":"string"},"repo":{"type":"string"},"number":{"type":"integer"}},"required":["owner","repo","number"]}),
    };

    vec![
        ping,
        list_issues,
        get_issue,
        list_issue_comments_plain,
        list_prs,
        get_pr,
        list_pr_comments,
        list_pr_review_comments,
        list_pr_review_threads,
        resolve_thread,
        unresolve_thread,
        list_pr_reviews,
        list_pr_commits,
        list_pr_files,
        get_pr_diff,
        get_pr_patch,
    ]
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PingInput {
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PingOutput {
    pub message: String,
}

// Shared result meta and error shapes used across tools.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RateMeta {
    pub remaining: Option<i32>,
    pub used: Option<i32>,
    pub reset_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Meta {
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub rate: Option<RateMeta>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ErrorShape {
    pub code: String,
    pub message: String,
    pub retriable: bool,
}

// Issues tool inputs
#[derive(Debug, Deserialize)]
pub struct ListIssuesInput {
    pub owner: String,
    pub repo: String,
    pub state: Option<String>,
    pub labels: Option<Vec<String>>,
    pub creator: Option<String>,
    pub assignee: Option<String>,
    pub mentions: Option<String>,
    pub since: Option<String>,
    pub sort: Option<String>,
    pub direction: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub include_author: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ListIssuesOutputItem {
    pub id: String,
    pub number: i64,
    pub title: String,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_login: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListIssuesOutput {
    pub items: Option<Vec<ListIssuesOutputItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct GetIssueInput {
    pub owner: String,
    pub repo: String,
    pub number: i64,
    pub include_author: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct GetIssueOutputItem {
    pub id: String,
    pub number: i64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_login: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GetIssueOutput {
    pub item: Option<GetIssueOutputItem>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ListIssueCommentsInput {
    pub owner: String,
    pub repo: String,
    pub number: i64,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub include_author: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ListIssueCommentsItem {
    pub id: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_login: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ListIssueCommentsOutput {
    pub items: Option<Vec<ListIssueCommentsItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

// PR inputs/outputs
#[derive(Debug, Deserialize)]
pub struct ListPullRequestsInput {
    pub owner: String,
    pub repo: String,
    pub state: Option<String>,
    pub base: Option<String>,
    pub head: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub include_author: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ListPullRequestsItem {
    pub id: String,
    pub number: i64,
    pub title: String,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_login: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListPullRequestsOutput {
    pub items: Option<Vec<ListPullRequestsItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct GetPullRequestInput {
    pub owner: String,
    pub repo: String,
    pub number: i64,
    pub include_author: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct GetPullRequestItem {
    pub id: String,
    pub number: i64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub state: String,
    pub is_draft: bool,
    pub created_at: String,
    pub updated_at: String,
    pub merged: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_login: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GetPullRequestOutput {
    pub item: Option<GetPullRequestItem>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ListPrCommentsInput {
    pub owner: String,
    pub repo: String,
    pub number: i64,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub include_author: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct PlainComment {
    pub id: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_login: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ListPrCommentsOutput {
    pub items: Option<Vec<PlainComment>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ListPrReviewCommentsInput {
    pub owner: String,
    pub repo: String,
    pub number: i64,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub include_author: Option<bool>,
    pub include_location: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ReviewCommentItem {
    pub id: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_login: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_side: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_line: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_start_line: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_hunk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_commit_sha: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListPrReviewCommentsOutput {
    pub items: Option<Vec<ReviewCommentItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ListPrReviewThreadsInput {
    pub owner: String,
    pub repo: String,
    pub number: i64,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub include_author: Option<bool>,
    pub include_location: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ReviewThreadItem {
    pub id: String,
    pub is_resolved: bool,
    pub is_outdated: bool,
    pub comments_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_by_login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_side: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListPrReviewThreadsOutput {
    pub items: Option<Vec<ReviewThreadItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ResolveThreadInput {
    pub thread_id: String,
}
#[derive(Debug, Serialize)]
pub struct ResolveThreadOutput {
    pub ok: bool,
    pub thread_id: String,
    pub is_resolved: bool,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ListPrReviewsInput {
    pub owner: String,
    pub repo: String,
    pub number: i64,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub include_author: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct PrReviewItem {
    pub id: String,
    pub state: String,
    pub submitted_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_login: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ListPrReviewsOutput {
    pub items: Option<Vec<PrReviewItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ListPrCommitsInput {
    pub owner: String,
    pub repo: String,
    pub number: i64,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub include_author: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct PrCommitItem {
    pub sha: String,
    pub title: String,
    pub authored_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_login: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ListPrCommitsOutput {
    pub items: Option<Vec<PrCommitItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ListPrFilesInput {
    pub owner: String,
    pub repo: String,
    pub number: i64,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub include_patch: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct PrFileItem {
    pub filename: String,
    pub status: String,
    pub additions: i64,
    pub deletions: i64,
    pub changes: i64,
    pub sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ListPrFilesOutput {
    pub items: Option<Vec<PrFileItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct GetPrTextInput {
    pub owner: String,
    pub repo: String,
    pub number: i64,
}
#[derive(Debug, Serialize)]
pub struct GetPrTextOutput {
    pub diff: Option<String>,
    pub patch: Option<String>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

// Actions / Workflows (REST) inputs/outputs
#[derive(Debug, Deserialize)]
pub struct ListWorkflowsInput {
    pub owner: String,
    pub repo: String,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}
#[derive(Debug, Serialize)]
pub struct WorkflowItem {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub state: String,
}
#[derive(Debug, Serialize)]
pub struct ListWorkflowsOutput {
    pub items: Option<Vec<WorkflowItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ListWorkflowRunsInput {
    pub owner: String,
    pub repo: String,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}
#[derive(Debug, Serialize)]
pub struct WorkflowRunItem {
    pub id: i64,
    pub run_number: i64,
    pub event: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub head_sha: String,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Serialize)]
pub struct ListWorkflowRunsOutput {
    pub items: Option<Vec<WorkflowRunItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct GetWorkflowRunInput {
    pub owner: String,
    pub repo: String,
    pub run_id: i64,
    pub exclude_pull_requests: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct GetWorkflowRunOutput {
    pub item: Option<WorkflowRunItem>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ListWorkflowJobsInput {
    pub owner: String,
    pub repo: String,
    pub run_id: i64,
    pub filter: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}
#[derive(Debug, Serialize)]
pub struct WorkflowJobItem {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ListWorkflowJobsOutput {
    pub items: Option<Vec<WorkflowJobItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct GetJobLogsInput {
    pub owner: String,
    pub repo: String,
    pub job_id: i64,
    pub tail_lines: Option<usize>,
    pub include_timestamps: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct GetJobLogsOutput {
    pub logs: Option<String>,
    pub truncated: bool,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct RunIdInput {
    pub owner: String,
    pub repo: String,
    pub run_id: i64,
}
#[derive(Debug, Serialize)]
pub struct OkOutput {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queued_run_id: Option<i64>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

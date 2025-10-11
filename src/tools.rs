use crate::types::RateMeta;
use serde::{Deserialize, Serialize};

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

    // Unified aliases (non-deprecated): prefer these going forward
    let list_pr_review_comments_unified = ToolDescriptor {
        name: "list_pr_review_comments".into(),
        description: "List PR review comments (unified; flags control optional fields)".into(),
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

    let list_pr_reviews_unified = ToolDescriptor {
        name: "list_pr_reviews".into(),
        description: "List PR reviews (unified)".into(),
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

    let list_pr_commits_unified = ToolDescriptor {
        name: "list_pr_commits".into(),
        description: "List PR commits (unified)".into(),
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

    let list_pr_files_unified = ToolDescriptor {
        name: "list_pr_files".into(),
        description: "List PR files (unified; REST)".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties": {"owner":{"type":"string"},"repo":{"type":"string"},"number":{"type":"integer"},"cursor":{"type":"string"},"limit":{"type":"integer"},"include_patch":{"type":"boolean"}},
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

    let pr_summary = ToolDescriptor {
        name: "pr_summary".into(),
        description: "Get PR summary including checks, files, reviews".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "owner": {"type": "string"},
                "repo": {"type": "string"},
                "number": {"type": "integer"},
                "include_checks": {"type": "boolean"},
                "include_files": {"type": "boolean"},
                "include_reviews": {"type": "boolean"}
            },
            "required": ["owner", "repo", "number"]
        }),
    };

    // New methods per Issue #91
    let list_commits = ToolDescriptor {
        name: "list_commits".into(),
        description: "List commits for a repository".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties":{
                "owner":{"type":"string"},
                "repo":{"type":"string"},
                "sha":{"type":"string"},
                "path":{"type":"string"},
                "author":{"type":"string"},
                "since":{"type":"string"},
                "until":{"type":"string"},
                "cursor":{"type":"string"},
                "limit":{"type":"integer"},
                "include_author":{"type":"boolean"},
                "include_stats":{"type":"boolean"}
            },
            "required":["owner","repo"]
        }),
    };
    let get_commit = ToolDescriptor {
        name: "get_commit".into(),
        description: "Get a single commit by SHA or ref".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties":{
                "owner":{"type":"string"},
                "repo":{"type":"string"},
                "ref":{"type":"string"},
                "include_stats":{"type":"boolean"},
                "include_files":{"type":"boolean"}
            },
            "required":["owner","repo","ref"]
        }),
    };
    let list_tags = ToolDescriptor {
        name: "list_tags".into(),
        description: "List tags for a repository".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties":{ "owner":{"type":"string"}, "repo":{"type":"string"}, "cursor":{"type":"string"}, "limit":{"type":"integer"}, "include_object":{"type":"boolean"}},
            "required":["owner","repo"]
        }),
    };
    let get_tag = ToolDescriptor {
        name: "get_tag".into(),
        description: "Get a tag by name (resolves annotated)".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties":{ "owner":{"type":"string"}, "repo":{"type":"string"}, "tag":{"type":"string"}, "resolve_annotated":{"type":"boolean"}},
            "required":["owner","repo","tag"]
        }),
    };
    let list_branches = ToolDescriptor {
        name: "list_branches".into(),
        description: "List branches in a repository".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties":{ "owner":{"type":"string"}, "repo":{"type":"string"}, "protected":{"type":"boolean"}, "cursor":{"type":"string"}, "limit":{"type":"integer"}},
            "required":["owner","repo"]
        }),
    };
    let list_releases = ToolDescriptor {
        name: "list_releases".into(),
        description: "List releases for a repository".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties":{ "owner":{"type":"string"}, "repo":{"type":"string"}, "cursor":{"type":"string"}, "limit":{"type":"integer"}},
            "required":["owner","repo"]
        }),
    };
    let get_release = ToolDescriptor {
        name: "get_release".into(),
        description: "Get a release by id or tag".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties":{ "owner":{"type":"string"}, "repo":{"type":"string"}, "release_id":{"type":"integer"}, "tag":{"type":"string"}},
            "required":["owner","repo"]
        }),
    };
    let list_starred_repositories = ToolDescriptor {
        name: "list_starred_repositories".into(),
        description: "List repositories starred by the authenticated user".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties":{ "cursor":{"type":"string"}, "limit":{"type":"integer"}, "sort":{"type":"string","enum":["created","updated"]}, "direction":{"type":"string","enum":["asc","desc"]}, "include_starred_at":{"type":"boolean"}},
            "required":[]
        }),
    };
    let merge_pr = ToolDescriptor {
        name: "merge_pr".into(),
        description: "Merge a pull request (requires write permissions)".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties":{
                "owner":{"type":"string"},"repo":{"type":"string"},"number":{"type":"integer"},
                "merge_method":{"type":"string","enum":["merge","squash","rebase"]},
                "commit_title":{"type":"string"},
                "commit_message":{"type":"string"},
                "sha":{"type":"string"}
            },
            "required":["owner","repo","number"]
        }),
    };
    let search_issues = ToolDescriptor {
        name: "search_issues".into(),
        description: "Search issues via GitHub Search API".into(),
        input_schema: serde_json::json!({"type":"object","additionalProperties":false,
            "properties": {"q":{"type":"string"}, "sort":{"type":"string"}, "order":{"type":"string","enum":["asc","desc"]}, "cursor":{"type":"string"}, "limit":{"type":"integer"}},
            "required":["q"]
        }),
    };
    let search_pull_requests = ToolDescriptor {
        name: "search_pull_requests".into(),
        description: "Search pull requests via GitHub Search API".into(),
        input_schema: serde_json::json!({"type":"object","additionalProperties":false,
            "properties": {"q":{"type":"string"}, "sort":{"type":"string"}, "order":{"type":"string","enum":["asc","desc"]}, "cursor":{"type":"string"}, "limit":{"type":"integer"}},
            "required":["q"]
        }),
    };
    let search_repositories = ToolDescriptor {
        name: "search_repositories".into(),
        description: "Search repositories via GitHub Search API".into(),
        input_schema: serde_json::json!({"type":"object","additionalProperties":false,
            "properties": {"q":{"type":"string"}, "sort":{"type":"string"}, "order":{"type":"string","enum":["asc","desc"]}, "cursor":{"type":"string"}, "limit":{"type":"integer"}},
            "required":["q"]
        }),
    };
    let update_issue = ToolDescriptor {
        name: "update_issue".into(),
        description: "Update an issue: title/body/labels/assignees/state/milestone".into(),
        input_schema: serde_json::json!({"type":"object","additionalProperties":false,
            "properties": {"owner":{"type":"string"},"repo":{"type":"string"},"number":{"type":"integer"},
                "title":{"type":"string"},"body":{"type":"string"},"labels":{"type":"array","items":{"type":"string"}},
                "assignees":{"type":"array","items":{"type":"string"}},"state":{"type":"string","enum":["open","closed"]},"milestone":{"type":"integer"}},
            "required":["owner","repo","number"]
        }),
    };
    let update_pull_request = ToolDescriptor {
        name: "update_pull_request".into(),
        description: "Update a pull request: title/body/state/base/maintainer_can_modify".into(),
        input_schema: serde_json::json!({"type":"object","additionalProperties":false,
            "properties": {"owner":{"type":"string"},"repo":{"type":"string"},"number":{"type":"integer"},
                "title":{"type":"string"},"body":{"type":"string"},"state":{"type":"string","enum":["open","closed"]},"base":{"type":"string"},"maintainer_can_modify":{"type":"boolean"}},
            "required":["owner","repo","number"]
        }),
    };
    let fork_repository = ToolDescriptor {
        name: "fork_repository".into(),
        description: "Fork a repository to the authenticated user or an organization".into(),
        input_schema: serde_json::json!({"type":"object","additionalProperties":false,
            "properties": {"owner":{"type":"string"},"repo":{"type":"string"},"organization":{"type":"string"}},
            "required":["owner","repo"]
        }),
    };

    // Secrets, Variables, Environments (REST light)
    let list_repo_secrets_light = ToolDescriptor {
        name: "list_repo_secrets_light".into(),
        description: "List repository Actions secrets (metadata only)".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties":{
                "owner":{"type":"string"},
                "repo":{"type":"string"},
                "cursor":{"type":"string"},
                "page":{"type":"integer"},
                "per_page":{"type":"integer"}
            },
            "required":["owner","repo"]
        }),
    };
    let list_repo_variables_light = ToolDescriptor {
        name: "list_repo_variables_light".into(),
        description: "List repository Actions variables (may include values)".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties":{
                "owner":{"type":"string"},
                "repo":{"type":"string"},
                "cursor":{"type":"string"},
                "page":{"type":"integer"},
                "per_page":{"type":"integer"}
            },
            "required":["owner","repo"]
        }),
    };
    let list_environments_light = ToolDescriptor {
        name: "list_environments_light".into(),
        description: "List repository environments (light)".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties":{
                "owner":{"type":"string"},
                "repo":{"type":"string"},
                "cursor":{"type":"string"},
                "page":{"type":"integer"},
                "per_page":{"type":"integer"}
            },
            "required":["owner","repo"]
        }),
    };
    let list_environment_variables_light = ToolDescriptor {
        name: "list_environment_variables_light".into(),
        description: "List environment-scoped Actions variables (may include values)".into(),
        input_schema: serde_json::json!({
            "type":"object","additionalProperties":false,
            "properties":{
                "owner":{"type":"string"},
                "repo":{"type":"string"},
                "environment_name":{"type":"string"},
                "cursor":{"type":"string"},
                "page":{"type":"integer"},
                "per_page":{"type":"integer"}
            },
            "required":["owner","repo","environment_name"]
        }),
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
        list_pr_review_comments_unified,
        list_pr_review_threads,
        resolve_thread,
        unresolve_thread,
        list_pr_reviews,
        list_pr_reviews_unified,
        list_pr_commits,
        list_pr_commits_unified,
        list_pr_files,
        list_pr_files_unified,
        get_pr_diff,
        get_pr_patch,
        pr_summary,
        list_repo_secrets_light,
        list_repo_variables_light,
        list_environments_light,
        list_environment_variables_light,
        // New methods
        list_commits,
        get_commit,
        list_tags,
        get_tag,
        list_branches,
        list_releases,
        get_release,
        list_starred_repositories,
        merge_pr,
        search_issues,
        search_pull_requests,
        search_repositories,
        update_issue,
        update_pull_request,
        fork_repository,
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
// RateMeta lives in types.rs; use the shared definition to avoid duplication.

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

// Secrets / Variables / Environments inputs and outputs (REST light)
#[derive(Debug, Deserialize)]
pub struct RepoInput {
    pub owner: String,
    pub repo: String,
    pub cursor: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}
#[derive(Debug, Deserialize)]
pub struct EnvVarsInput {
    pub owner: String,
    pub repo: String,
    pub environment_name: String,
    pub cursor: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct RepoSecretItem {
    pub name: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct RepoVariableItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct EnvironmentItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ListRepoSecretsOutput {
    pub items: Option<Vec<RepoSecretItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}
#[derive(Debug, Serialize)]
pub struct ListRepoVariablesOutput {
    pub items: Option<Vec<RepoVariableItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}
#[derive(Debug, Serialize)]
pub struct ListEnvironmentsOutput {
    pub items: Option<Vec<EnvironmentItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

// New unified/simple outputs for added tools
#[derive(Debug, Deserialize)]
pub struct ListCommitsInput {
    pub owner: String,
    pub repo: String,
    pub sha: Option<String>,
    pub path: Option<String>,
    pub author: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub include_author: Option<bool>,
    pub include_stats: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct ListCommitsItem {
    pub sha: String,
    pub title: String,
    pub authored_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub committer_login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<CommitStats>,
}
#[derive(Debug, Serialize)]
pub struct CommitStats {
    pub additions: i64,
    pub deletions: i64,
    pub total: i64,
}
#[derive(Debug, Serialize)]
pub struct ListCommitsOutput {
    pub items: Option<Vec<ListCommitsItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct GetCommitInput {
    pub owner: String,
    pub repo: String,
    pub r#ref: String,
    pub include_stats: Option<bool>,
    pub include_files: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct CommitParent {
    pub sha: String,
}
#[derive(Debug, Serialize)]
pub struct CommitFile {
    pub filename: String,
    pub status: String,
    pub additions: i64,
    pub deletions: i64,
    pub changes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct GetCommitItem {
    pub sha: String,
    pub message: String,
    pub authored_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub committer_login: Option<String>,
    pub parents: Vec<CommitParent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<CommitStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<CommitFile>>,
}
#[derive(Debug, Serialize)]
pub struct GetCommitOutput {
    pub item: Option<GetCommitItem>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ListTagsInput {
    pub owner: String,
    pub repo: String,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    // Currently unused until we return tag object details; kept for API parity
    #[allow(dead_code)]
    pub include_object: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct TagItem {
    pub name: String,
    pub commit_sha: String,
    pub zipball_url: String,
    pub tarball_url: String,
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagger: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ListTagsOutput {
    pub items: Option<Vec<TagItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct GetTagInput {
    pub owner: String,
    pub repo: String,
    pub tag: String,
    pub resolve_annotated: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct GetTagItem {
    pub name: String,
    pub commit_sha: String,
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagger: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct GetTagOutput {
    pub item: Option<GetTagItem>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ListBranchesInput {
    pub owner: String,
    pub repo: String,
    pub protected: Option<bool>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}
#[derive(Debug, Serialize)]
pub struct BranchItem {
    pub name: String,
    pub commit_sha: String,
    pub protected: bool,
}
#[derive(Debug, Serialize)]
pub struct ListBranchesOutput {
    pub items: Option<Vec<BranchItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ListReleasesInput {
    pub owner: String,
    pub repo: String,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}
#[derive(Debug, Serialize)]
pub struct ReleaseItem {
    pub id: i64,
    pub tag_name: String,
    pub name: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
    pub created_at: Option<String>,
    pub published_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_login: Option<String>,
    pub assets_count: i64,
}
#[derive(Debug, Serialize)]
pub struct ListReleasesOutput {
    pub items: Option<Vec<ReleaseItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct GetReleaseInput {
    pub owner: String,
    pub repo: String,
    pub release_id: Option<i64>,
    pub tag: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ReleaseAsset {
    pub id: i64,
    pub name: String,
    pub content_type: String,
    pub size: i64,
    pub download_count: i64,
    pub browser_download_url: String,
}
#[derive(Debug, Serialize)]
pub struct GetReleaseItem {
    pub id: i64,
    pub tag_name: String,
    pub name: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
    pub created_at: Option<String>,
    pub published_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_login: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}
#[derive(Debug, Serialize)]
pub struct GetReleaseOutput {
    pub item: Option<GetReleaseItem>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ListStarredReposInput {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub sort: Option<String>,
    pub direction: Option<String>,
    pub include_starred_at: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct StarredRepoItem {
    pub full_name: String,
    pub private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub stargazers_count: i64,
    pub html_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starred_at: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ListStarredReposOutput {
    pub items: Option<Vec<StarredRepoItem>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct MergePrInput {
    pub owner: String,
    pub repo: String,
    pub number: i64,
    pub merge_method: Option<String>,
    pub commit_title: Option<String>,
    pub commit_message: Option<String>,
    pub sha: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct MergePrResult {
    pub merged: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct MergePrOutput {
    pub item: Option<MergePrResult>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct SearchInput {
    pub q: String,
    pub sort: Option<String>,
    pub order: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}
#[derive(Debug, Serialize)]
pub struct SearchIssueItem {
    pub id: i64,
    pub number: i64,
    pub title: String,
    pub state: String,
    pub repo_full_name: String,
    pub is_pull_request: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_login: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Serialize)]
pub struct SearchIssuesOutput {
    pub items: Option<Vec<SearchIssueItem>>,
    pub total_count: i64,
    pub incomplete_results: bool,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}
#[derive(Debug, Serialize)]
pub struct SearchRepoItem {
    pub full_name: String,
    pub private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub stargazers_count: i64,
    pub forks_count: i64,
    pub open_issues_count: i64,
    pub html_url: String,
}
#[derive(Debug, Serialize)]
pub struct SearchReposOutput {
    pub items: Option<Vec<SearchRepoItem>>,
    pub total_count: i64,
    pub incomplete_results: bool,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateIssueInput {
    pub owner: String,
    pub repo: String,
    pub number: i64,
    pub title: Option<String>,
    pub body: Option<String>,
    pub labels: Option<Vec<String>>,
    pub assignees: Option<Vec<String>>,
    pub state: Option<String>,
    pub milestone: Option<i64>,
}
#[derive(Debug, Serialize)]
pub struct UpdatedIssueItem {
    pub id: i64,
    pub number: i64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub state: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<i64>,
    pub updated_at: String,
}
#[derive(Debug, Serialize)]
pub struct UpdateIssueOutput {
    pub item: Option<UpdatedIssueItem>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePullRequestInput {
    pub owner: String,
    pub repo: String,
    pub number: i64,
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<String>,
    pub base: Option<String>,
    pub maintainer_can_modify: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct UpdatedPrItem {
    pub id: i64,
    pub number: i64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub state: String,
    pub is_draft: bool,
    pub base_ref: String,
}
#[derive(Debug, Serialize)]
pub struct UpdatePullRequestOutput {
    pub item: Option<UpdatedPrItem>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

#[derive(Debug, Deserialize)]
pub struct ForkRepositoryInput {
    pub owner: String,
    pub repo: String,
    pub organization: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ForkRepoItem {
    pub full_name: String,
    pub owner_login: String,
    pub private: bool,
    pub html_url: String,
    pub parent_full_name: Option<String>,
    pub created_at: String,
}
#[derive(Debug, Serialize)]
pub struct ForkRepositoryOutput {
    pub item: Option<ForkRepoItem>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorShape>,
}

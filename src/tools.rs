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

    vec![ping, list_issues, get_issue, list_issue_comments_plain]
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct RateMeta { pub remaining: Option<i32>, pub used: Option<i32>, pub reset_at: Option<String> }

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Meta { pub next_cursor: Option<String>, pub has_more: bool, pub rate: Option<RateMeta> }

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ErrorShape { pub code: String, pub message: String, pub retriable: bool }

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
    #[serde(skip_serializing_if = "Option::is_none")] pub author_login: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListIssuesOutput { pub items: Option<Vec<ListIssuesOutputItem>>, pub meta: Meta, #[serde(skip_serializing_if = "Option::is_none")] pub error: Option<ErrorShape> }

#[derive(Debug, Deserialize)]
pub struct GetIssueInput { pub owner: String, pub repo: String, pub number: i64, pub include_author: Option<bool> }

#[derive(Debug, Serialize)]
pub struct GetIssueOutputItem {
    pub id: String,
    pub number: i64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub body: Option<String>,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub author_login: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GetIssueOutput { pub item: Option<GetIssueOutputItem>, pub meta: Meta, #[serde(skip_serializing_if = "Option::is_none")] pub error: Option<ErrorShape> }

#[derive(Debug, Deserialize)]
pub struct ListIssueCommentsInput { pub owner: String, pub repo: String, pub number: i64, pub cursor: Option<String>, pub limit: Option<u32>, pub include_author: Option<bool> }

#[derive(Debug, Serialize)]
pub struct ListIssueCommentsItem { pub id: String, pub body: String, #[serde(skip_serializing_if = "Option::is_none")] pub author_login: Option<String>, pub created_at: String, pub updated_at: String }

#[derive(Debug, Serialize)]
pub struct ListIssueCommentsOutput { pub items: Option<Vec<ListIssueCommentsItem>>, pub meta: Meta, #[serde(skip_serializing_if = "Option::is_none")] pub error: Option<ErrorShape> }

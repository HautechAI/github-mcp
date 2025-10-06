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
    // For milestone 1 we only expose ping and placeholder methods list; real tools arrive in later milestones.
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

    vec![ping]
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PingInput {
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PingOutput {
    pub message: String,
}

use serde_json::Value;

// Build an MCP-compliant result envelope for tools/call outputs.
// - content: always a single text block so clients can render something.
// - structuredContent: preserves the previous structured JSON shape to minimize breakage.
// - isError: included only when true to keep payloads small.
pub fn mcp_wrap(structured: Value, text_opt: Option<String>, is_error: bool) -> Value {
    let text = match text_opt {
        Some(s) => s,
        None => serde_json::to_string(&structured).unwrap_or_else(|_| "{}".to_string()),
    };
    let mut obj = serde_json::json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
    });
    if is_error {
        if let Some(map) = obj.as_object_mut() {
            map.insert("isError".to_string(), Value::Bool(true));
        }
    }
    obj
}


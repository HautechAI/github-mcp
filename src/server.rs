use crate::tools::{tool_descriptors, PingInput, PingOutput, PROTOCOL_VERSION};
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

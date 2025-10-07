use assert_cmd::Command;
use std::io::Write;

fn run(req: &serde_json::Value) -> anyhow::Result<String> {
    let mut cmd = Command::cargo_bin("github-mcp")?;
    let input = serde_json::to_string(req)?;
    let assert = cmd
        .arg("--log-level")
        .arg("warn")
        .write_stdin({
            let mut b = Vec::new();
            writeln!(b, "{}", input).unwrap();
            b
        })
        .assert();
    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    Ok(output)
}

#[test]
fn initialize_and_tools_list() -> anyhow::Result<()> {
    // initialize
    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "id": 1
    });
    let out = run(&init_req)?;
    assert!(out.contains("\"protocolVersion\""));

    // tools/list
    let list_req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "id": 2
    });
    let out = run(&list_req)?;
    assert!(out.contains("\"tools\""));
    assert!(out.contains("\"ping\""));
    // Ensure nextCursor is absent or a string (never null)
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    if let Some(result) = v.get("result") {
        if let Some(nc) = result.get("nextCursor") {
            assert!(nc.is_string(), "nextCursor must be a string when present; got: {}", nc);
        }
    }

    // tools/call ping (now MCP envelope)
    let call_req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": "ping", "arguments": {"message": "hello"}},
        "id": 3
    });
    let out = run(&call_req)?;
    // Expect MCP content envelope with text type
    assert!(out.contains("\"content\""));
    assert!(out.contains("\"type\":\"text\""));
    assert!(out.contains("hello"));
    // And structuredContent preserving previous shape
    assert!(out.contains("\"structuredContent\""));
    Ok(())
}

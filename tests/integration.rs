use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

fn run(req: &serde_json::Value) -> anyhow::Result<String> {
    let mut cmd = Command::cargo_bin("github-mcp")?;
    let input = serde_json::to_string(req)?;
    let assert = cmd.arg("--log-level").arg("warn").write_stdin(input).assert();
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
    assert!(out.contains("\"server\""));

    // tools/list
    let list_req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "id": 2
    });
    let out = run(&list_req)?;
    assert!(out.contains("\"tools\""));

    // tools/call ping
    let call_req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": "ping", "arguments": {"message": "hello"}},
        "id": 3
    });
    let out = run(&call_req)?;
    assert!(out.contains("hello"));
    Ok(())
}

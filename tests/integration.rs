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
    // Assert presence of core GitHub tools (names may evolve; keep to read-only basics)
    assert!(out.contains("\"list_issues\""));
    assert!(out.contains("\"get_issue\""));
    // Ensure nextCursor is absent or a string (never null)
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    if let Some(result) = v.get("result") {
        if let Some(nc) = result.get("nextCursor") {
            assert!(
                nc.is_string(),
                "nextCursor must be a string when present; got: {}",
                nc
            );
        }
    }
    Ok(())
}

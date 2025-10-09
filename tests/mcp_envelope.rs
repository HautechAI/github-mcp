use assert_cmd::Command;
use httpmock::{Method::GET, MockServer};
use std::io::Write;

fn run_with_env(req: &serde_json::Value, envs: &[(&str, &str)]) -> anyhow::Result<String> {
    let mut cmd = Command::cargo_bin("github-mcp")?;
    for (k, v) in envs {
        cmd.env(k, v);
    }
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
fn mcp_envelope_success_and_error_and_ping_gating() -> anyhow::Result<()> {
    // Success: mocked REST tool list_workflows_light 200
    let server_ok = MockServer::start();
    let _m_ok = server_ok.mock(|when, then| {
        when.method(GET)
            .path("/repos/o/r/actions/workflows")
            .query_param("per_page", "10")
            .query_param("page", "1");
        then.status(200)
            .header("Content-Type", "application/json")
            .body("{\n  \"workflows\": [\n    {\n      \"id\": 1, \"name\": \"CI\", \"path\": \".github/workflows/ci.yml\", \"state\": \"active\"\n    }\n  ],\n  \"total_count\": 1\n}");
    });
    let ok_req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_workflows_light","arguments":{"owner":"o","repo":"r","per_page":10,"page":1}}
    });
    let out_ok = run_with_env(
        &ok_req,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server_ok.base_url().as_str()),
        ],
    )?;
    assert!(out_ok.contains("\"content\""));
    assert!(out_ok.contains("\"structuredContent\""));
    assert!(!out_ok.contains("\"isError\":true"));

    // Error path: list_workflows_light with 404 from REST
    let server_err = MockServer::start();
    let _m_err = server_err.mock(|when, then| {
        when.method(GET).path("/repos/o/r/actions/workflows");
        then.status(404).body("no workflows");
    });
    let err_req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"list_workflows_light","arguments":{"owner":"o","repo":"r","per_page":10,"page":1}}
    });
    let out_err = run_with_env(
        &err_req,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server_err.base_url().as_str()),
        ],
    )?;
    assert!(out_err.contains("\"content\""));
    assert!(out_err.contains("\"structuredContent\""));
    assert!(out_err.contains("\"isError\":true"));
    assert!(out_err.contains("\"error\""));

    // Gating: default OFF => no ping in list, ping call => -32601
    let list_req = serde_json::json!({"jsonrpc":"2.0","method":"tools/list","id":3});
    let list_out = run_with_env(&list_req, &[])?;
    assert!(list_out.contains("\"tools\""));
    assert!(!list_out.contains("\"ping\""));

    let ping_call = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":4,
        "params":{"name":"ping","arguments":{"message":"ok"}}
    });
    let ping_out = run_with_env(&ping_call, &[])?;
    // Expect JSON-RPC error -32601
    let v: serde_json::Value = serde_json::from_str(&ping_out)?;
    assert!(
        v.get("error").is_some(),
        "expected error for ping when disabled"
    );
    assert_eq!(v["error"]["code"], -32601);

    // Gating: when enabled => ping listed and callable
    let list_on = run_with_env(&list_req, &[("GITHUB_MCP_ENABLE_PING", "true")])?;
    assert!(list_on.contains("\"ping\""));
    let ping_on = run_with_env(&ping_call, &[("GITHUB_MCP_ENABLE_PING", "1")])?;
    assert!(ping_on.contains("\"content\""));
    assert!(ping_on.contains("\"structuredContent\""));

    Ok(())
}

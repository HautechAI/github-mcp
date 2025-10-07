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
fn mcp_envelope_success_and_error() -> anyhow::Result<()> {
    // Success: ping
    let ping_req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"ping","arguments":{"message":"ok"}}
    });
    let out = run_with_env(&ping_req, &[])?;
    assert!(out.contains("\"content\""));
    assert!(out.contains("\"type\":\"text\""));
    assert!(out.contains("\"structuredContent\""));
    assert!(out.contains("ok"));
    assert!(!out.contains("\"isError\":true"));

    // Error path: list_workflows_light with 404 from REST
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/actions/workflows");
        then.status(404).body("no workflows");
    });
    let req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"list_workflows_light","arguments":{"owner":"o","repo":"r","per_page":10,"page":1}}
    });
    let out = run_with_env(
        &req,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    // Envelope present and marked as error
    assert!(out.contains("\"content\""));
    assert!(out.contains("\"structuredContent\""));
    assert!(out.contains("\"isError\":true"));
    // Structured error payload included
    assert!(out.contains("\"error\""));
    Ok(())
}

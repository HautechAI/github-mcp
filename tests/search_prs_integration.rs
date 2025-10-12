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
fn search_pull_requests_uses_issues_index_and_meta() -> anyhow::Result<()> {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(GET).path("/search/issues");
        then.status(200)
            .header("link", &format!("<{}/search/issues?q=type:pr&page=2>; rel=\"next\"", server.base_url()))
            .json_body(serde_json::json!({
                "total_count": 3, "incomplete_results": false,
                "items": [
                    {"id":1,"number":1,"title":"PR1","state":"open","repository_url":"https://api.github.com/repos/o/r","user":{"login":"a"},"created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-01T00:00:00Z","pull_request": {}}
                ]
            }));
    });
    let req = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"search_pull_requests","arguments":{"q":"repo:o/r state:open","limit":1}}});
    let out = run_with_env(
        &req,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    let v: serde_json::Value = serde_json::from_str(&out)?;
    let sc = v["result"]["structuredContent"].clone();
    assert_eq!(sc["items"][0]["is_pull_request"].as_bool().unwrap(), true);
    assert!(sc["meta"]["has_more"].as_bool().unwrap());
    assert!(sc["meta"]["next_cursor"].is_string());
    Ok(())
}

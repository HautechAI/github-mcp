use assert_cmd::Command;
use httpmock::{Method::GET, Method::POST, MockServer};
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
fn list_pr_comments_plain_happy() -> anyhow::Result<()> {
    let server = MockServer::start();
    let body = serde_json::json!({
      "data": {"repository": {"pullRequest": {"comments": {"nodes": [{"id":"IC_1","body":"hi","createdAt":"2025-01-01T00:00:00Z","updatedAt":"2025-01-01T00:00:00Z","author":{"login":"alice"}}],"pageInfo": {"hasNextPage": false, "endCursor": null}} }}}
    });
    let _m = server.mock(|when, then| {
        when.method(POST).path("/graphql");
        then.status(200).json_body(body);
    });
    let req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_pr_comments_plain","arguments": {"owner":"o","repo":"r","number":1,"limit":10,"include_author":true}}
    });
    let out = run_with_env(
        &req,
        &[
            ("GITHUB_TOKEN", "t"),
            (
                "GITHUB_GRAPHQL_URL",
                &format!("{}/graphql", server.base_url()),
            ),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out.contains("\"structuredContent\""));
    assert!(out.contains("\"items\""));
    assert!(out.contains("\"author_login\":\"alice\""));
    Ok(())
}

#[test]
fn get_pr_diff_and_patch_rest_headers() -> anyhow::Result<()> {
    let server = MockServer::start();
    let _m1 = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/pulls/1");
        then.status(200)
            .header("x-ratelimit-remaining", "4999")
            .header("x-ratelimit-used", "1")
            .header("x-ratelimit-reset", "0")
            .body("diff-data");
    });
    let req = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"get_pr_diff","arguments":{"owner":"o","repo":"r","number":1}}});
    let out = run_with_env(
        &req,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out.contains("diff-data")); // text content
    assert!(out.contains("\"structuredContent\""));

    let _m2 = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/pulls/2");
        then.status(200).body("patch-data");
    });
    let req2 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"get_pr_patch","arguments":{"owner":"o","repo":"r","number":2}}});
    let out2 = run_with_env(
        &req2,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out2.contains("patch-data"));
    assert!(out2.contains("\"structuredContent\""));
    Ok(())
}

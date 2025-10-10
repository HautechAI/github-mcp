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
fn rest_pagination_link_headers() -> anyhow::Result<()> {
    let server = MockServer::start();
    // Simulate Link header for next page
    let _m = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/actions/runs");
        then.status(200)
            .header("x-ratelimit-remaining","4999")
            .header("x-ratelimit-used","1")
            .header("x-ratelimit-reset","0")
            .header("link", "<https://api.example/repos/o/r/actions/runs?page=2>; rel=\"next\", <https://api.example/repos/o/r/actions/runs?page=3>; rel=\"last\"")
            .json_body(serde_json::json!({"workflow_runs":[]}));
    });
    let req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_workflow_runs_light","arguments":{"owner":"o","repo":"r","per_page":30}}
    });
    let out = run_with_env(
        &req,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out.contains("\"structuredContent\""));
    assert!(out.contains("\"has_more\":true"));
    assert!(out.contains("next_cursor"));
    // Default should omit rate
    assert!(!out.contains("\"rate\""));

    // With _include_rate=true: rate should appear in meta
    let req_inc = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"list_workflow_runs_light","arguments":{"owner":"o","repo":"r","per_page":30,"_include_rate":true}}
    });
    let out_inc = run_with_env(
        &req_inc,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out_inc.contains("\"rate\""));
    Ok(())
}

#[test]
fn graphql_rate_limit_meta() -> anyhow::Result<()> {
    let server = MockServer::start();
    let body = serde_json::json!({
      "data": {"repository": {"pullRequest": {"id":"PR_1","number":1,"title":"PR","body":null,"state":"OPEN","isDraft":false,"merged":false,"mergedAt":null,"createdAt":"2025-01-01T00:00:00Z","updatedAt":"2025-01-01T00:00:00Z","author":null}},
       "rateLimit": {"remaining":4999,"used":1,"resetAt":"1970-01-01T00:00:00Z"}}
    });
    let _m = server.mock(|when, then| {
        when.method(httpmock::Method::POST).path("/graphql");
        then.status(200).json_body(body);
    });
    let req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"get_pull_request","arguments":{"owner":"o","repo":"r","number":1}}
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
    // Default excludes rate and prunes meta entirely for non-paginated results.
    let v: serde_json::Value = serde_json::from_str(&out)?;
    let sc = v
        .get("result")
        .and_then(|r| r.get("structuredContent"))
        .cloned()
        .unwrap_or_default();
    assert!(
        sc.get("meta").is_none(),
        "expected meta omitted by default when has_more=false: {}",
        sc
    );

    // When explicitly requested via _include_rate, rate should appear.
    let req_include = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"get_pull_request","arguments":{"owner":"o","repo":"r","number":1,"_include_rate":true}}
    });
    let out2 = run_with_env(
        &req_include,
        &[
            ("GITHUB_TOKEN", "t"),
            (
                "GITHUB_GRAPHQL_URL",
                &format!("{}/graphql", server.base_url()),
            ),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out2.contains("\"rate\""));
    assert!(out2.contains("\"remaining\":4999"));
    Ok(())
}

#[test]
fn rest_non_paginated_meta_default_and_optin() -> anyhow::Result<()> {
    let server = MockServer::start();
    // get_workflow_run_light returns non-paginated result
    let _m = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/actions/runs/1");
        then.status(200)
            .header("x-ratelimit-remaining", "4999")
            .header("x-ratelimit-used", "1")
            .header("x-ratelimit-reset", "0")
            .json_body(serde_json::json!({
                "id":1,
                "run_number":10,
                "event":"push",
                "status":"completed",
                "conclusion":"success",
                "head_sha":"abc",
                "created_at":"2025-01-01T00:00:00Z",
                "updated_at":"2025-01-01T00:00:00Z"
            }));
    });
    let req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"get_workflow_run_light","arguments":{"owner":"o","repo":"r","run_id":1}}
    });
    let out = run_with_env(
        &req,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    let v: serde_json::Value = serde_json::from_str(&out)?;
    let sc = v
        .get("result")
        .and_then(|r| r.get("structuredContent"))
        .cloned()
        .unwrap_or_default();
    assert!(
        sc.get("meta").is_none(),
        "expected meta omitted by default when has_more=false"
    );

    // Opt-in rate
    let req2 = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"get_workflow_run_light","arguments":{"owner":"o","repo":"r","run_id":1,"_include_rate":true}}
    });
    let out2 = run_with_env(
        &req2,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out2.contains("\"rate\""));
    Ok(())
}

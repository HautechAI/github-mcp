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
    // No pagination, so meta should be pruned entirely by default
    let v: serde_json::Value = serde_json::from_str(&out)?;
    let sc = v
        .get("result")
        .and_then(|r| r.get("structuredContent"))
        .and_then(|sc| sc.as_object());
    assert!(
        sc.and_then(|o| o.get("meta")).is_none(),
        "expected meta to be omitted when has_more=false and include_rate not requested"
    );
    Ok(())
}

#[test]
fn list_pr_review_threads_light_include_location_and_empty() -> anyhow::Result<()> {
    // include_location true/false and empty threads
    let server = MockServer::start();
    // Empty threads response
    let body_empty = serde_json::json!({
        "data": {
            "repository": {
                "pullRequest": {
                    "reviewThreads": {
                        "nodes": [],
                        "pageInfo": { "hasNextPage": false, "endCursor": null }
                    }
                }
            }
        },
        "rateLimit": { "remaining": 4999, "used": 1, "resetAt": "1970-01-01T00:00:00Z" }
    });
    let _m1 = server.mock(|when, then| {
        when.method(POST).path("/graphql");
        then.status(200).json_body(body_empty.clone());
    });
    let req1 = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_pr_review_threads_light","arguments": {"owner":"o","repo":"r","number":1,"limit":10,"include_author":true,"include_location":true}}
    });
    let out1 = run_with_env(
        &req1,
        &[
            ("GITHUB_TOKEN", "t"),
            (
                "GITHUB_GRAPHQL_URL",
                &format!("{}/graphql", server.base_url()),
            ),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out1.contains("\"structuredContent\""));
    assert!(out1.contains("\"items\":[]"));

    // include_location=false should also succeed with empty
    let _m2 = server.mock(|when, then| {
        when.method(POST).path("/graphql");
        then.status(200).json_body(body_empty);
    });
    let req2 = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"list_pr_review_threads_light","arguments": {"owner":"o","repo":"r","number":1,"limit":10,"include_author":false,"include_location":false}}
    });
    let out2 = run_with_env(
        &req2,
        &[
            ("GITHUB_TOKEN", "t"),
            (
                "GITHUB_GRAPHQL_URL",
                &format!("{}/graphql", server.base_url()),
            ),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out2.contains("\"structuredContent\""));
    assert!(out2.contains("\"items\":[]"));
    Ok(())
}

#[test]
fn list_pr_review_comments_plain_include_location_and_empty() -> anyhow::Result<()> {
    let server = MockServer::start();
    // REST: empty array for review comments
    let _m1 = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/pulls/1/comments");
        then.status(200)
            .header("x-ratelimit-remaining","4999")
            .header("x-ratelimit-used","1")
            .header("x-ratelimit-reset","0")
            .json_body(serde_json::json!([]));
    });
    let req1 = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_pr_review_comments_plain","arguments": {"owner":"o","repo":"r","number":1,"limit":10,"include_author":true,"include_location":true}}
    });
    let out1 = run_with_env(
        &req1,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out1.contains("\"structuredContent\""));
    assert!(out1.contains("\"items\":[]"));

    // include_location=false also OK (still empty)
    let _m2 = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/pulls/1/comments");
        then.status(200).json_body(serde_json::json!([]));
    });
    let req2 = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"list_pr_review_comments_plain","arguments": {"owner":"o","repo":"r","number":1,"limit":10,"include_author":false,"include_location":false}}
    });
    let out2 = run_with_env(
        &req2,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out2.contains("\"structuredContent\""));
    assert!(out2.contains("\"items\":[]"));
    Ok(())
}

#[test]
fn list_pr_review_comments_plain_pagination_and_flags() -> anyhow::Result<()> {
    let server = MockServer::start();
    // Page 1: two comments, Link header indicates next page
    let _m1 = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/pulls/1/comments");
        then.status(200)
            .header("link", &format!("<{}/repos/o/r/pulls/1/comments?page=2>; rel=\"next\"", server.base_url()))
            .json_body(serde_json::json!([
                {"id": 11, "body": "c1", "created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-01T00:00:00Z","user":{"login":"alice"},"path":"f.rs","line":10,"start_line":9,"side":"right","start_side":"left","original_line":8,"original_start_line":7,"diff_hunk":"@@","commit_id":"abc","original_commit_id":"def"},
                {"id": 12, "body": "c2", "created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-01T00:00:00Z","user":null}
            ]));
    });
    let req1 = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_pr_review_comments_plain","arguments": {"owner":"o","repo":"r","number":1,"limit":2,"include_author":true,"include_location":true}}
    });
    let out1 = run_with_env(
        &req1,
        &[("GITHUB_TOKEN", "t"), ("GITHUB_API_URL", server.base_url().as_str())],
    )?;
    let v1: serde_json::Value = serde_json::from_str(&out1)?;
    let sc1 = v1.get("result").and_then(|r| r.get("structuredContent")).cloned().unwrap_or_default();
    assert_eq!(sc1["items"].as_array().unwrap().len(), 2);
    assert_eq!(sc1["items"][0]["author_login"].as_str().unwrap(), "alice");
    assert_eq!(sc1["items"][0]["side"].as_str().unwrap(), "RIGHT");
    assert_eq!(sc1["items"][0]["start_side"].as_str().unwrap(), "LEFT");
    assert!(sc1["meta"]["has_more"].as_bool().unwrap());
    let cursor = sc1["meta"]["next_cursor"].as_str().unwrap().to_string();

    // Page 2: single comment, no Link header => end
    let _m2 = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/pulls/1/comments");
        then.status(200)
            .json_body(serde_json::json!([
                {"id": 13, "body": "c3", "created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-01T00:00:00Z"}
            ]));
    });
    let req2 = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"list_pr_review_comments_plain","arguments": {"owner":"o","repo":"r","number":1,"cursor":cursor}}
    });
    let out2 = run_with_env(
        &req2,
        &[("GITHUB_TOKEN", "t"), ("GITHUB_API_URL", server.base_url().as_str())],
    )?;
    let v2: serde_json::Value = serde_json::from_str(&out2)?;
    let sc2 = v2.get("result").and_then(|r| r.get("structuredContent")).cloned().unwrap_or_default();
    assert_eq!(sc2["items"].as_array().unwrap().len(), 1);
    // When no more pages, default behavior prunes meta
    assert!(sc2.get("meta").is_none());
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

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
fn list_pr_review_comments_plain_minimal() -> anyhow::Result<()> {
    let server = MockServer::start();
    // Minimal fields only (include_location default=false)
    let body_min = serde_json::json!({
      "data": {"repository": {"pullRequest": {"reviewComments": {"nodes": [
        {"id":"RC_1","body":"c1","createdAt":"2025-01-02T00:00:00Z","updatedAt":"2025-01-02T00:00:00Z","author":{"login":"bob"}}
      ],"pageInfo": {"hasNextPage": false, "endCursor": null}} }}},
      "rateLimit": {"remaining": 4999, "used": 1, "resetAt": "1970-01-01T00:00:00Z"}
    });
    let _m1 = server.mock(|when, then| {
        when.method(POST).path("/graphql");
        then.status(200).json_body(body_min.clone());
    });
    let req_min = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_pr_review_comments_plain","arguments": {"owner":"o","repo":"r","number":1,"limit":10,"include_author":true}}
    });
    let out_min = run_with_env(
        &req_min,
        &[
            ("GITHUB_TOKEN", "t"),
            (
                "GITHUB_GRAPHQL_URL",
                &format!("{}/graphql", server.base_url()),
            ),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out_min.contains("\"structuredContent\""));
    assert!(out_min.contains("\"items\""));
    assert!(out_min.contains("\"author_login\":\"bob\""));
    // Should not require location fields; absence shouldn't error

    Ok(())
}

#[test]
fn list_pr_review_comments_plain_with_location() -> anyhow::Result<()> {
    let server = MockServer::start();
    let body_loc = serde_json::json!({
      "data": {"repository": {"pullRequest": {"reviewComments": {"nodes": [
        {"id":"RC_2","body":"c2","createdAt":"2025-01-02T00:00:00Z","updatedAt":"2025-01-02T00:00:00Z","author":{"login":"carol"},
         "path":"a/b.txt","line":10,"startLine":9,"side":"RIGHT","startSide":"RIGHT",
         "originalLine":8,"originalStartLine":7,
         "diffHunk":"@@ -1,2 +1,2 @@","commit":{"oid":"abc"},"originalCommit":{"oid":"def"},
         "pullRequestReviewThread": {"path":"a/b.txt","line":10,"startLine":9,"side":"RIGHT","startSide":"RIGHT"}
        }
      ],"pageInfo": {"hasNextPage": false, "endCursor": null}} }}},
      "rateLimit": {"remaining": 4999, "used": 1, "resetAt": "1970-01-01T00:00:00Z"}
    });
    let _m = server.mock(|when, then| {
        when.method(POST).path("/graphql");
        then.status(200).json_body(body_loc.clone());
    });
    let req_loc = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"list_pr_review_comments_plain","arguments": {"owner":"o","repo":"r","number":1,"limit":10,"include_author":true,"include_location":true}}
    });
    let out_loc = run_with_env(
        &req_loc,
        &[
            ("GITHUB_TOKEN", "t"),
            (
                "GITHUB_GRAPHQL_URL",
                &format!("{}/graphql", server.base_url()),
            ),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out_loc.contains("\"path\":\"a/b.txt\""));
    assert!(out_loc.contains("\"commit_sha\":\"abc\""));
    assert!(out_loc.contains("\"original_commit_sha\":\"def\""));
    Ok(())
}

#[test]
fn list_pr_review_threads_light_minimal() -> anyhow::Result<()> {
    let server = MockServer::start();
    // Minimal fields only
    let body_min = serde_json::json!({
      "data": {"repository": {"pullRequest": {"reviewThreads": {"nodes": [
        {"id":"T_1","isResolved":false,"isOutdated":false,"comments":{"totalCount":1},"resolvedBy":null}
      ],"pageInfo": {"hasNextPage": false, "endCursor": null}} }}},
      "rateLimit": {"remaining": 4999, "used": 1, "resetAt": "1970-01-01T00:00:00Z"}
    });
    let _m1 = server.mock(|when, then| {
        when.method(POST).path("/graphql");
        then.status(200).json_body(body_min.clone());
    });
    let req_min = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_pr_review_threads_light","arguments": {"owner":"o","repo":"r","number":1,"limit":10,"include_author":true}}
    });
    let out_min = run_with_env(
        &req_min,
        &[
            ("GITHUB_TOKEN", "t"),
            (
                "GITHUB_GRAPHQL_URL",
                &format!("{}/graphql", server.base_url()),
            ),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out_min.contains("\"structuredContent\""));
    assert!(out_min.contains("\"items\""));

    Ok(())
}

#[test]
fn list_pr_review_threads_light_with_location() -> anyhow::Result<()> {
    let server = MockServer::start();
    let body_loc = serde_json::json!({
      "data": {"repository": {"pullRequest": {"reviewThreads": {"nodes": [
        {"id":"T_2","isResolved":true,"isOutdated":false,"comments":{"totalCount":2},"resolvedBy":{"login":"zoe"},
         "path":"a/b.txt","line":10,"startLine":9,"side":"RIGHT","startSide":"RIGHT"}
      ],"pageInfo": {"hasNextPage": false, "endCursor": null}} }}},
      "rateLimit": {"remaining": 4999, "used": 1, "resetAt": "1970-01-01T00:00:00Z"}
    });
    let _m = server.mock(|when, then| {
        when.method(POST).path("/graphql");
        then.status(200).json_body(body_loc.clone());
    });
    let req_loc = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"list_pr_review_threads_light","arguments": {"owner":"o","repo":"r","number":1,"limit":10,"include_author":true,"include_location":true}}
    });
    let out_loc = run_with_env(
        &req_loc,
        &[
            ("GITHUB_TOKEN", "t"),
            (
                "GITHUB_GRAPHQL_URL",
                &format!("{}/graphql", server.base_url()),
            ),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out_loc.contains("\"path\":\"a/b.txt\""));
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

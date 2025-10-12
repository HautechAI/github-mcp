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
        .arg("--log-level").arg("warn")
        .write_stdin({ let mut b=Vec::new(); writeln!(b, "{}", input).unwrap(); b })
        .assert();
    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    Ok(output)
}

#[test]
fn list_pr_reviews_variants_and_commits_and_files_pagination() -> anyhow::Result<()> {
    let server = MockServer::start();
    // Reviews (GraphQL underlying per summary; list_pr_reviews and _light share)
    let graphql_reviews = serde_json::json!({
        "data": {"repository": {"pullRequest": {"reviews": {"nodes": [
            {"author": {"login": "a"}, "state":"APPROVED", "submittedAt":"2025-01-01T00:00:00Z"}
        ]}}}},
        "rateLimit": {"remaining":4999,"used":1,"resetAt":"1970-01-01T00:00:00Z"}
    });
    let _m_graphql_reviews = server.mock(|when, then| {
        when.method(POST).path("/graphql").body_contains("reviews");
        then.status(200).json_body(graphql_reviews.clone());
    });
    // Commits (GraphQL)
    let graphql_commits = serde_json::json!({
        "data": {"repository": {"pullRequest": {"commits": {"nodes": [
            {"commit": {"oid":"abc","messageHeadline":"h","changedFilesIfAvailable":2,"additionsIfAvailable":3,"deletionsIfAvailable":1,"pushedDate":"2025-01-01T00:00:00Z"}}
        ]}}}},
        "rateLimit": {"remaining":4999,"used":1,"resetAt":"1970-01-01T00:00:00Z"}
    });
    let _m_graphql_commits = server.mock(|when, then| {
        when.method(POST).path("/graphql").body_contains("commits");
        then.status(200).json_body(graphql_commits.clone());
    });
    // Files (REST) with pagination: first page has Link next
    let _m_files_p1 = server.mock(|when, then| {
        when.method(GET)
            .path("/repos/o/r/pulls/1/files")
            .query_param("per_page", "1")
            .query_param("page", "1");
        then.status(200)
            .header("link", &format!("<{}/repos/o/r/pulls/1/files?page=2>; rel=\"next\"", server.base_url()))
            .json_body(serde_json::json!([
                {"filename":"a.rs","status":"modified","additions":1,"deletions":1,"changes":2,"sha":"1","patch":"@@"}
            ]));
    });
    let _m_files_p2 = server.mock(|when, then| {
        when.method(GET)
            .path("/repos/o/r/pulls/1/files")
            .query_param("per_page", "1")
            .query_param("page", "2");
        then.status(200).json_body(serde_json::json!([
            {"filename":"b.rs","status":"added","additions":2,"deletions":0,"changes":2,"sha":"2","patch":null}
        ]));
    });
    // Fallback for unified alias or any additional page=1 requests
    let _m_files_fallback = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/pulls/1/files");
        then.status(200).json_body(serde_json::json!([
            {"filename":"a.rs","status":"modified","additions":1,"deletions":1,"changes":2,"sha":"1","patch":"@@"}
        ]));
    });

    let base = server.base_url();
    let env = [("GITHUB_TOKEN","t"),("GITHUB_GRAPHQL_URL", &format!("{}/graphql", base)), ("GITHUB_API_URL", base.as_str())];

    // list_pr_reviews_light
    let req_reviews_light = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_pr_reviews_light","arguments":{"owner":"o","repo":"r","number":1,"limit":10}}});
    let out_rev_l = run_with_env(&req_reviews_light, &env)?;
    assert!(out_rev_l.contains("\"items\""));
    // list_pr_reviews (unified)
    let req_reviews = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"list_pr_reviews","arguments":{"owner":"o","repo":"r","number":1,"limit":10}}});
    let out_rev = run_with_env(&req_reviews, &env)?; assert!(out_rev.contains("\"items\""));

    // list_pr_commits_light
    let req_commits_light = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":3,
        "params":{"name":"list_pr_commits_light","arguments":{"owner":"o","repo":"r","number":1,"limit":10}}});
    let out_com_l = run_with_env(&req_commits_light, &env)?; assert!(out_com_l.contains("\"items\""));
    // list_pr_commits (unified)
    let req_commits = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":4,
        "params":{"name":"list_pr_commits","arguments":{"owner":"o","repo":"r","number":1,"limit":10}}});
    let out_com = run_with_env(&req_commits, &env)?; assert!(out_com.contains("\"items\""));

    // list_pr_files_light page1 (legacy page/per_page)
    let req_files_l = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":5,
        "params":{"name":"list_pr_files_light","arguments":{"owner":"o","repo":"r","number":1,"per_page":1,"page":1}}});
    let out_files_l = run_with_env(&req_files_l, &env)?; println!("out_files_l={}", out_files_l);
    let v_l: serde_json::Value = serde_json::from_str(&out_files_l)?; let sc_l = v_l["result"]["structuredContent"].clone();
    assert!(sc_l["meta"]["has_more"].as_bool().unwrap());
    // page-2 via page/per_page
    let req_files_l2 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":6,
        "params":{"name":"list_pr_files_light","arguments":{"owner":"o","repo":"r","number":1,"per_page":1,"page":2}}});
    let out_files_l2 = run_with_env(&req_files_l2, &env)?; assert!(out_files_l2.contains("b.rs"));

    // list_pr_files unified alias (legacy params supported by same handler)
    let req_files = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":7,
        "params":{"name":"list_pr_files","arguments":{"owner":"o","repo":"r","number":1,"per_page":1,"page":1}}});
    let out_files = run_with_env(&req_files, &env)?; println!("out_files_unified={}", out_files);
    let v_u: serde_json::Value = serde_json::from_str(&out_files)?;
    let sc_u = v_u["result"]["structuredContent"].clone();
    assert!(sc_u["meta"]["has_more"].as_bool().unwrap());
    assert!(sc_u["meta"]["next_cursor"].is_string());
    Ok(())
}

#[test]
fn resolve_and_unresolve_review_thread_happy_and_error() -> anyhow::Result<()> {
    let server = MockServer::start();
    // GraphQL mutation happy path resolve
    let gql_ok = serde_json::json!({
        "data": {"resolveReviewThread": {"thread": {"id":"T1","isResolved": true}}},
        "rateLimit": {"remaining": 4999, "used": 1, "resetAt": "1970-01-01T00:00:00Z"}
    });
    let _m_ok = server.mock(|when, then| { when.method(POST).path("/graphql").body_contains("resolveReviewThread").body_contains("T1"); then.status(200).json_body(gql_ok.clone()); });
    // GraphQL mutation error path
    let gql_err = serde_json::json!({
        "errors": [{"message":"thread not found"}],
        "data": null,
        "rateLimit": {"remaining": 4999, "used": 1, "resetAt": "1970-01-01T00:00:00Z"}
    });
    let _m_err = server.mock(|when, then| { when.method(POST).path("/graphql").body_contains("resolveReviewThread").body_contains("MISSING"); then.status(200).json_body(gql_err.clone()); });

    let base = server.base_url();
    let env = [("GITHUB_TOKEN","t"),("GITHUB_GRAPHQL_URL", &format!("{}/graphql", base)), ("GITHUB_API_URL", base.as_str())];
    // resolve
    let req_res = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"resolve_pr_review_thread","arguments":{"thread_id":"T1"}}});
    let out_res = run_with_env(&req_res, &env)?; println!("out_resolve={}", out_res); let v_res: serde_json::Value = serde_json::from_str(&out_res)?; assert_eq!(v_res["result"]["structuredContent"]["is_resolved"], true);
    // unresolve
    let gql_unresolve = serde_json::json!({
        "data": {"unresolveReviewThread": {"thread": {"id":"T1","isResolved": false}}},
        "rateLimit": {"remaining": 4999, "used": 1, "resetAt": "1970-01-01T00:00:00Z"}
    });
    let _m_unres = server.mock(|when, then| { when.method(POST).path("/graphql").body_contains("unresolveReviewThread"); then.status(200).json_body(gql_unresolve.clone()); });
    let req_unres = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"unresolve_pr_review_thread","arguments":{"thread_id":"T1"}}});
    let out_unres = run_with_env(&req_unres, &env)?; assert!(out_unres.contains("false"));

    // representative error path: use mocked error response
    let req_err = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":3,
        "params":{"name":"resolve_pr_review_thread","arguments":{"thread_id":"MISSING"}}});
    let out_err = run_with_env(&req_err, &env)?; let v: serde_json::Value = serde_json::from_str(&out_err)?; assert!(v["result"]["isError"].as_bool().unwrap_or(false) || v.get("error").is_some());
    Ok(())
}

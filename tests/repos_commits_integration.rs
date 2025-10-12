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
fn list_commits_filters_and_pagination_and_get_commit_toggles() -> anyhow::Result<()> {
    let server = MockServer::start();
    // list_commits page 1 with author filter and since/until
    let _m_list1 = server.mock(|when, then| {
        when.method(GET)
            .path("/repos/o/r/commits")
            .query_param("per_page", "1")
            .query_param("page", "1")
            .query_param("author", "a")
            .query_param("since", "2025-01-01T00:00:00Z")
            .query_param("until", "2025-01-02T00:00:00Z");
        then.status(200)
            .header("link", &format!("<{}/repos/o/r/commits?page=2>; rel=\"next\"", server.base_url()))
            .json_body(serde_json::json!([
                {"sha":"c1","commit":{"message":"m1","author":{"name":"a","date":"2025-01-01T00:00:00Z"}},"author": {"login":"a"}}
            ]));
    });
    let base = server.base_url();
    let env = [("GITHUB_TOKEN", "t"), ("GITHUB_API_URL", base.as_str())];
    let req1 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_commits","arguments":{"owner":"o","repo":"r","limit":1,"author":"a","since":"2025-01-01T00:00:00Z","until":"2025-01-02T00:00:00Z","include_author":true}}});
    let out1 = run_with_env(&req1, &env)?;
    let v1: serde_json::Value = serde_json::from_str(&out1)?;
    let sc1 = v1["result"]["structuredContent"].clone();
    assert!(sc1["meta"]["has_more"].as_bool().unwrap());
    let next = sc1["meta"]["next_cursor"].as_str().unwrap().to_string();

    // page 2
    let _m_list2 = server.mock(|when, then| {
        when.method(GET)
            .path("/repos/o/r/commits")
            .query_param("per_page", "1")
            .query_param("page", "2");
        then.status(200).json_body(serde_json::json!([
            {"sha":"c2","commit":{"message":"m2","author":{"name":"b","date":"2025-01-01T01:00:00Z"}},"author": {"login":"b"}}
        ]));
    });
    let req2 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"list_commits","arguments":{"owner":"o","repo":"r","cursor":next}}});
    let out2 = run_with_env(&req2, &env)?;
    assert!(out2.contains("c2"));

    // get_commit without files/stats
    let _m_get1 = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/commits/c1");
        then.status(200).json_body(serde_json::json!({
            "sha":"c1","commit":{"message":"m1","author":{"name":"a","date":"2025-01-01T00:00:00Z"}}}
        ));
    });
    let req3 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":3,
        "params":{"name":"get_commit","arguments":{"owner":"o","repo":"r","ref":"c1"}}});
    let out3 = run_with_env(&req3, &env)?;
    let v3: serde_json::Value = serde_json::from_str(&out3)?;
    let sc3 = v3["result"]["structuredContent"].clone();
    assert!(sc3["item"]["files"].is_null());
    assert!(sc3["item"]["stats"].is_null());

    // get_commit include_files and stats
    let _m_get2 = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/commits/c1");
        then.status(200).json_body(serde_json::json!({
            "sha":"c1","commit":{"message":"m1","author":{"name":"a","date":"2025-01-01T00:00:00Z"}},
            "stats": {"total":2,"additions":2,"deletions":0},
            "files": [{"filename":"a","status":"modified","additions":2,"deletions":0,"changes":2,"patch":"@@"}]
        }));
    });
    let req4 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":4,
        "params":{"name":"get_commit","arguments":{"owner":"o","repo":"r","ref":"c1","include_files":true,"include_stats":true}}});
    let out4 = run_with_env(&req4, &env)?;
    let v4: serde_json::Value = serde_json::from_str(&out4)?;
    let sc4 = v4["result"]["structuredContent"]["item"].clone();
    assert!(sc4.get("files").is_some());
    assert!(sc4.get("stats").is_some());
    Ok(())
}

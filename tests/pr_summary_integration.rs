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
fn pr_summary_happy_with_checks_reviews_files() -> anyhow::Result<()> {
    let server = MockServer::start();
    // 1) PR details
    let _m_pr = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/pulls/1");
        then.status(200).json_body(serde_json::json!({
            "number": 1,
            "title": "Add feature",
            "state": "open",
            "draft": false,
            "user": {"login": "alice"},
            "head": {"sha": "abc123"},
            "base": {"label": "o:r:main", "ref": "main"},
            "commits": 3,
            "changed_files": 2,
            "additions": 10,
            "deletions": 2
        }));
    });
    // 2) Reviews
    let _m_reviews = server.mock(|when, then| {
        when.method(GET)
            .path("/repos/o/r/pulls/1/reviews")
            .query_param("per_page", "100")
            .query_param("page", "1");
        then.status(200).json_body(serde_json::json!([
            {"state":"APPROVED"},
            {"state":"CHANGES_REQUESTED"}
        ]));
    });
    // 3) Files
    let _m_files = server.mock(|when, then| {
        when.method(GET)
            .path("/repos/o/r/pulls/1/files")
            .query_param("per_page", "100")
            .query_param("page", "1");
        then.status(200).json_body(serde_json::json!([
            {"filename":"a.rs","status":"modified","additions":5,"deletions":1,"changes":6,"sha":"f1","patch":"@@"},
            {"filename":"b.rs","status":"added","additions":5,"deletions":1,"changes":6,"sha":"f2","patch":null}
        ]));
    });
    // 4) Status
    let _m_status = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/commits/abc123/status");
        then.status(200)
            .json_body(serde_json::json!({"state":"success"}));
    });
    // 5) Check runs
    let _m_checks = server.mock(|when, then| {
        when.method(GET)
            .path("/repos/o/r/commits/abc123/check-runs");
        then.status(200).json_body(serde_json::json!({
            "total_count": 3,
            "check_runs": [
                {"conclusion":"success","status":"completed"},
                {"conclusion":"failure","status":"completed"},
                {"conclusion":null,"status":"in_progress"}
            ]
        }));
    });

    let req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params": {"name":"pr_summary","arguments": {"owner":"o","repo":"r","number":1}}
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
        .unwrap();
    assert_eq!(sc["item"]["number"].as_i64().unwrap(), 1);
    assert_eq!(sc["item"]["author_login"].as_str().unwrap(), "alice");
    assert_eq!(sc["item"]["base_ref"].as_str().unwrap(), "main");
    assert_eq!(sc["item"]["files"].as_array().unwrap().len(), 2);
    assert_eq!(sc["item"]["review_states"]["APPROVED"].as_i64().unwrap(), 1);
    assert_eq!(
        sc["item"]["review_states"]["CHANGES_REQUESTED"]
            .as_i64()
            .unwrap(),
        1
    );
    assert_eq!(sc["item"]["checks_summary"]["success"].as_i64().unwrap(), 1);
    assert_eq!(sc["item"]["checks_summary"]["failure"].as_i64().unwrap(), 1);
    assert_eq!(sc["item"]["checks_summary"]["pending"].as_i64().unwrap(), 1);
    // No pagination; meta should be pruned by default
    assert!(sc.get("meta").is_none());
    Ok(())
}

#[test]
fn pr_summary_flags_disable_sections() -> anyhow::Result<()> {
    let server = MockServer::start();
    // PR only; other endpoints won't be called when flags false
    let _m_pr = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/pulls/1");
        then.status(200).json_body(serde_json::json!({
            "number": 1,
            "title": "Add feature",
            "state": "open",
            "draft": false,
            "user": {"login": "alice"},
            "head": {"sha": "abc123"},
            "base": {"label": "o:r:main", "ref": "main"},
            "commits": 3,
            "changed_files": 2,
            "additions": 10,
            "deletions": 2
        }));
    });
    let req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params": {"name":"pr_summary","arguments": {"owner":"o","repo":"r","number":1,"include_checks":false,"include_reviews":false,"include_files":false}}
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
        .unwrap();
    assert!(sc["item"]["checks_summary"].is_null());
    assert!(sc["item"]["files"].is_null());
    assert!(sc["item"]["review_states"].as_object().unwrap().is_empty());
    Ok(())
}

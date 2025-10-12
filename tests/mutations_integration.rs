use assert_cmd::Command;
use httpmock::{Method::PUT, Method::PATCH, Method::POST, MockServer};
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
fn merge_pr_update_issue_update_pr_and_fork() -> anyhow::Result<()> {
    let server = MockServer::start();
    // merge_pr
    let _m_merge = server.mock(|when, then| {
        when.method(PUT).path("/repos/o/r/pulls/1/merge");
        then.status(200).json_body(serde_json::json!({"merged":true,"message":"Merged","sha":"abc"}));
    });
    // update_issue
    let _m_issue = server.mock(|when, then| {
        when.method(PATCH).path("/repos/o/r/issues/2");
        then.status(200).json_body(serde_json::json!({
            "id": 22, "number": 2, "title":"t","body":"b","state":"open",
            "assignees": [{"login":"a"}],
            "labels": [{"name":"bug"}],
            "milestone": {"number": 1},
            "updated_at": "2025-01-01T00:00:00Z"
        }));
    });
    // update_pull_request
    let _m_pr = server.mock(|when, then| {
        when.method(PATCH).path("/repos/o/r/pulls/3");
        then.status(200).json_body(serde_json::json!({
            "id": 33, "number": 3, "title":"pt","body":"pb","state":"open","draft":false,
            "base": {"ref":"main"}
        }));
    });
    // fork_repository
    let _m_fork = server.mock(|when, then| {
        when.method(POST).path("/repos/o/r/forks");
        then.status(202).json_body(serde_json::json!({
            "full_name":"me/r","owner":{"login":"me"},"private":false,"html_url":"u","parent":{"full_name":"o/r"},"created_at":"1970-01-01T00:00:00Z"
        }));
    });

    // Execute calls
    let base_url = server.base_url();
    let env = [("GITHUB_TOKEN","t"),("GITHUB_API_URL", base_url.as_str())];
    let merge = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"merge_pr","arguments":{"owner":"o","repo":"r","number":1}}});
    let out_merge = run_with_env(&merge, &env)?; assert!(out_merge.contains("\"merged\":true"));

    let upd_issue = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"update_issue","arguments":{"owner":"o","repo":"r","number":2,"title":"t"}}});
    let out_issue = run_with_env(&upd_issue, &env)?; assert!(out_issue.contains("\"issue updated\""));

    let upd_pr = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":3,
        "params":{"name":"update_pull_request","arguments":{"owner":"o","repo":"r","number":3,"title":"pt"}}});
    let out_pr = run_with_env(&upd_pr, &env)?; assert!(out_pr.contains("\"pull request updated\""));

    let fork = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":4,
        "params":{"name":"fork_repository","arguments":{"owner":"o","repo":"r"}}});
    let out_fork = run_with_env(&fork, &env)?; assert!(out_fork.contains("\"forked:"));
    Ok(())
}

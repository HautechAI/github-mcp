use assert_cmd::Command;
use httpmock::{Method::POST, MockServer};
use std::io::Write;

fn frame(msg: &str) -> Vec<u8> {
    let mut v = Vec::new();
    write!(v, "Content-Length: {}\r\n\r\n{}", msg.len(), msg).unwrap();
    v
}

fn run_with_env(req: &serde_json::Value, envs: &[(&str, &str)]) -> anyhow::Result<String> {
    let mut cmd = Command::cargo_bin("github-mcp")?;
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let input = serde_json::to_string(req)?;
    let assert = cmd
        .arg("--log-level")
        .arg("warn")
        .write_stdin(frame(&input))
        .assert();
    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    Ok(output)
}

#[test]
fn list_issues_happy_path() -> anyhow::Result<()> {
    let server = MockServer::start();
    // Mock GraphQL response minimal
    let body = serde_json::json!({
      "data": {
        "repository": {
          "issues": {
            "nodes": [
              {"id":"I_1","number":1,"title":"One","state":"OPEN","createdAt":"2025-01-01T00:00:00Z","updatedAt":"2025-01-01T00:00:00Z","author": {"login":"alice"}}
            ],
            "pageInfo": {"hasNextPage": false, "endCursor": null}
          }
        }
      }
    });
    let _m = server.mock(|when, then| {
        when.method(POST).path("/graphql");
        then.status(200).json_body(body.clone());
    });

    let req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_issues","arguments": {"owner":"o","repo":"r","limit":10,"include_author":true}}
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
    assert!(out.contains("\"items\""));
    assert!(out.contains("\"author_login\":\"alice\""));
    Ok(())
}

#[test]
fn get_issue_happy_path() -> anyhow::Result<()> {
    let server = MockServer::start();
    let body = serde_json::json!({
      "data": {"repository": {"issue": {"id":"I_1","number":1,"title":"One","body":"b","state":"OPEN","createdAt":"2025-01-01T00:00:00Z","updatedAt":"2025-01-01T00:00:00Z","author":{"login":"alice"}}}}
    });
    let _m = server.mock(|when, then| {
        when.method(POST).path("/graphql");
        then.status(200).json_body(body);
    });
    let req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"get_issue","arguments": {"owner":"o","repo":"r","number":1,"include_author":true}}
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
    assert!(out.contains("\"item\""));
    assert!(out.contains("\"author_login\":\"alice\""));
    Ok(())
}

#[test]
fn list_issue_comments_happy_path() -> anyhow::Result<()> {
    let server = MockServer::start();
    let body = serde_json::json!({
      "data": {"repository": {"issue": {"comments": {"nodes": [{"id":"C_1","body":"hi","createdAt":"2025-01-01T00:00:00Z","updatedAt":"2025-01-01T00:00:00Z","author":{"login":"bob"}}],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}}
    });
    let _m = server.mock(|when, then| {
        when.method(POST).path("/graphql");
        then.status(200).json_body(body);
    });
    let req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_issue_comments_plain","arguments": {"owner":"o","repo":"r","number":1,"limit":10,"include_author":true}}
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
    assert!(out.contains("\"items\""));
    assert!(out.contains("\"author_login\":\"bob\""));
    Ok(())
}

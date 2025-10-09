use assert_cmd::Command;
use httpmock::{Method::POST, MockServer};
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
fn list_issues_state_all_omits_states_variable() -> anyhow::Result<()> {
    let server = MockServer::start();
    // Verify request variables omit "states" when state=="all"
    let _m = server.mock(|when, then| {
        when.method(POST)
            .path("/graphql")
            .matches(|req| {
                let body_bytes = req.body.as_deref().unwrap_or(&[]);
                let body = std::str::from_utf8(body_bytes).unwrap_or("");
                // Body should contain variables but not \"states\":
                // Safer: parse and check JSON
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
                    let vars = v.get("variables").cloned().unwrap_or_default();
                    // Either variables missing states or explicitly null
                    let no_states = match vars.get("states") {
                        None => true,
                        Some(val) => val.is_null(),
                    };
                    return no_states;
                }
                false
            });
        then.status(200).json_body(serde_json::json!({
          "data": {
            "repository": {
              "issues": {
                "nodes": [
                  {"id":"I_1","number":1,"title":"One","state":"OPEN","createdAt":"2025-01-01T00:00:00Z","updatedAt":"2025-01-01T00:00:00Z","author": null}
                ],
                "pageInfo": {"hasNextPage": false, "endCursor": null}
              }
            }
          }
        }));
    });

    let req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_issues","arguments": {"owner":"o","repo":"r","limit":10,"state":"all"}}
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
    Ok(())
}

#[test]
fn list_issues_order_by_mapping_created_asc() -> anyhow::Result<()> {
    let server = MockServer::start();
    // Verify orderBy variable is present and correct
    let _m = server.mock(|when, then| {
        when.method(POST)
            .path("/graphql")
            .matches(|req| {
                let body_bytes = req.body.as_deref().unwrap_or(&[]);
                let body = std::str::from_utf8(body_bytes).unwrap_or("");
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
                    if let Some(vars) = v.get("variables") {
                        if let Some(ob) = vars.get("orderBy") {
                            let field_ok = ob
                                .get("field")
                                .and_then(|x| x.as_str())
                                .map(|s| s == "CREATED_AT")
                                .unwrap_or(false);
                            let dir_ok = ob
                                .get("direction")
                                .and_then(|x| x.as_str())
                                .map(|s| s == "ASC")
                                .unwrap_or(false);
                            return field_ok && dir_ok;
                        }
                    }
                }
                false
            });
        then.status(200).json_body(serde_json::json!({
          "data": {
            "repository": {
              "issues": {
                "nodes": [],
                "pageInfo": {"hasNextPage": false, "endCursor": null}
              }
            }
          }
        }));
    });

    let req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_issues","arguments": {"owner":"o","repo":"r","limit":10,"sort":"created","direction":"asc"}}
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
    Ok(())
}

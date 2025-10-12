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
fn get_pr_status_summary_maps_union_and_counts() -> anyhow::Result<()> {
    let server = MockServer::start();
    // GraphQL response with CheckRun and StatusContext union nodes
    let body = serde_json::json!({
        "data": {
            "repository": {
                "pullRequest": {
                    "commits": {
                        "nodes": [
                            {"commit": {"statusCheckRollup": {
                                "state":"FAILURE",
                                "contexts": {"nodes": [
                                    {"__typename":"CheckRun","name":"build","conclusion":"success"},
                                    {"__typename":"CheckRun","name":"test","conclusion":"failure"},
                                    {"__typename":"StatusContext","context":"lint","state":"PENDING"}
                                ]}
                            }}}
                        ]
                    }
                }
            }
        },
        "rateLimit": {"remaining": 4999, "used": 1, "resetAt": "1970-01-01T00:00:00Z"}
    });
    let _m = server.mock(|when, then| {
        when.method(POST).path("/graphql");
        then.status(200).json_body(body);
    });
    let req = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"get_pr_status_summary","arguments":{"owner":"o","repo":"r","number":1,"include_failing_contexts":true,"limit_contexts":10}}});
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
    let v: serde_json::Value = serde_json::from_str(&out)?;
    let sc = v
        .get("result")
        .and_then(|r| r.get("structuredContent"))
        .cloned()
        .unwrap();
    assert_eq!(sc["item"]["counts"]["success"].as_i64().unwrap(), 1);
    assert_eq!(sc["item"]["counts"]["pending"].as_i64().unwrap(), 1);
    assert_eq!(sc["item"]["counts"]["failure"].as_i64().unwrap(), 1);
    assert!(sc["item"]["failing_contexts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str() == Some("test")));
    Ok(())
}

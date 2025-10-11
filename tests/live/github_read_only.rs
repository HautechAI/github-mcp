use assert_cmd::Command;
use std::io::Write;

const EXAMPLE_DOPPLER_CMD: &str =
    "doppler run -p github-mcp -c dev -- env E2E_OWNER=... E2E_REPO=... E2E_ISSUE_NUM=... cargo test -- --ignored";

fn run(req: &serde_json::Value) -> anyhow::Result<String> {
    let mut cmd = Command::cargo_bin("github-mcp")?;
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

fn env_var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|s| !s.is_empty())
}

fn ensure_prereqs(base_msg: &str) {
    // Enforce Doppler context by checking common env markers.
    let in_doppler = ["DOPPLER_PROJECT", "DOPPLER_CONFIG", "DOPPLER_ENVIRONMENT"]
        .into_iter()
        .any(|k| env_var(k).is_some());
    if !in_doppler {
        panic!("{} Missing Doppler context. Run via: {}", base_msg, EXAMPLE_DOPPLER_CMD);
    }

    // Require GitHub token in environment.
    if env_var("GITHUB_TOKEN").is_none() && env_var("GH_TOKEN").is_none() {
        panic!("{} Missing GITHUB_TOKEN/GH_TOKEN. Use Doppler or export a token before running ignored live tests.", base_msg);
    }
}

#[ignore]
#[test]
fn live_list_issues_basic() -> anyhow::Result<()> {
    ensure_prereqs("[live_list_issues_basic]");
    let owner = env_var("E2E_OWNER").unwrap_or_else(|| panic!(
        "[live_list_issues_basic] E2E_OWNER is required. Example: {}",
        EXAMPLE_DOPPLER_CMD
    ));
    let repo = env_var("E2E_REPO").unwrap_or_else(|| panic!(
        "[live_list_issues_basic] E2E_REPO is required. Example: {}",
        EXAMPLE_DOPPLER_CMD
    ));

    let req = serde_json::json!({
        "jsonrpc":"2.0",
        "method":"tools/call",
        "id": 1,
        "params": {"name": "list_issues", "arguments": {"owner": owner, "repo": repo, "limit": 5, "include_author": true}}
    });
    let out = run(&req)?;
    assert!(out.contains("\"structuredContent\""));
    assert!(out.contains("\"items\""));
    Ok(())
}

#[ignore]
#[test]
fn live_get_issue_if_fixture_provided() -> anyhow::Result<()> {
    ensure_prereqs("[live_get_issue_if_fixture_provided]");
    let owner = env_var("E2E_OWNER").unwrap_or_else(|| panic!(
        "[live_get_issue_if_fixture_provided] E2E_OWNER is required. Example: {}",
        EXAMPLE_DOPPLER_CMD
    ));
    let repo = env_var("E2E_REPO").unwrap_or_else(|| panic!(
        "[live_get_issue_if_fixture_provided] E2E_REPO is required. Example: {}",
        EXAMPLE_DOPPLER_CMD
    ));
    let number = env_var("E2E_ISSUE_NUM")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| panic!(
            "[live_get_issue_if_fixture_provided] E2E_ISSUE_NUM must be set to a valid number."
        ));

    let req = serde_json::json!({
        "jsonrpc":"2.0",
        "method":"tools/call",
        "id": 1,
        "params": {"name": "get_issue", "arguments": {"owner": owner, "repo": repo, "number": number, "include_author": true}}
    });
    let out = run(&req)?;
    assert!(out.contains("\"structuredContent\""));
    assert!(out.contains("\"item\""));
    Ok(())
}

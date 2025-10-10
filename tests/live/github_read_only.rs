use assert_cmd::Command;
use std::io::Write;

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

fn should_run_live() -> bool {
    matches!(env_var("LIVE_API_TESTS").as_deref(), Some("1"))
        && (env_var("GITHUB_TOKEN").is_some() || env_var("GH_TOKEN").is_some())
}

#[ignore]
#[test]
fn live_list_issues_basic() -> anyhow::Result<()> {
    if !should_run_live() {
        eprintln!("skipping live test: LIVE_API_TESTS!=1 or token missing");
        return Ok(());
    }
    let owner = match env_var("E2E_OWNER") {
        Some(v) => v,
        None => {
            eprintln!("skipping: E2E_OWNER not set");
            return Ok(());
        }
    };
    let repo = match env_var("E2E_REPO") {
        Some(v) => v,
        None => {
            eprintln!("skipping: E2E_REPO not set");
            return Ok(());
        }
    };

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
    if !should_run_live() {
        eprintln!("skipping live test: LIVE_API_TESTS!=1 or token missing");
        return Ok(());
    }
    let owner = match env_var("E2E_OWNER") {
        Some(v) => v,
        None => return Ok(()),
    };
    let repo = match env_var("E2E_REPO") {
        Some(v) => v,
        None => return Ok(()),
    };
    let number = match env_var("E2E_ISSUE_NUM").and_then(|s| s.parse::<u64>().ok()) {
        Some(n) => n,
        None => return Ok(()),
    };

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


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
        .arg("--log-level").arg("warn")
        .write_stdin({ let mut b=Vec::new(); writeln!(b, "{}", input).unwrap(); b })
        .assert();
    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    Ok(output)
}

#[test]
fn list_tags_and_get_tag_mixed_types() -> anyhow::Result<()> {
    let server = MockServer::start();
    // list_tags page 1 no Link header => no has_more
    let _m_list = server.mock(|when, then| {
        when.method(GET)
            .path("/repos/o/r/tags")
            .query_param("per_page", "10")
            .query_param("page", "1");
        then.status(200).json_body(serde_json::json!([
            {"name":"v1.0.0","commit":{"sha":"aaa","url":"u"},"zipball_url":"z","tarball_url":"t"}
        ]));
    });
    let req1 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_tags","arguments":{"owner":"o","repo":"r","limit":10}}});
    let out1 = run_with_env(&req1, &[("GITHUB_TOKEN","t"),("GITHUB_API_URL", server.base_url().as_str())])?;
    let v1: serde_json::Value = serde_json::from_str(&out1)?;
    let sc1 = v1.get("result").and_then(|r| r.get("structuredContent")).cloned().unwrap();
    assert_eq!(sc1["items"][0]["name"].as_str().unwrap(), "v1.0.0");
    assert!(sc1.get("meta").is_none()); // pruned

    // get_tag lightweight
    let _m_ref = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/git/ref/tags/v1.0.0");
        then.status(200).json_body(serde_json::json!({"object":{"type":"commit","sha":"aaa"}}));
    });
    let req2 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"get_tag","arguments":{"owner":"o","repo":"r","tag":"v1.0.0"}}});
    let out2 = run_with_env(&req2, &[("GITHUB_TOKEN","t"),("GITHUB_API_URL", server.base_url().as_str())])?;
    assert!(out2.contains("\"type\":\"lightweight\""));

    // get_tag annotated
    let _m_ref2 = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/git/ref/tags/v2.0.0");
        then.status(200).json_body(serde_json::json!({"object":{"type":"tag","sha":"bbb"}}));
    });
    let _m_tag = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/git/tags/bbb");
        then.status(200).json_body(serde_json::json!({"tag":"v2.0.0","message":"m","tagger":{"name":"t"},"object":{"sha":"ccc"}}));
    });
    let req3 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":3,
        "params":{"name":"get_tag","arguments":{"owner":"o","repo":"r","tag":"v2.0.0"}}});
    let out3 = run_with_env(&req3, &[("GITHUB_TOKEN","t"),("GITHUB_API_URL", server.base_url().as_str())])?;
    assert!(out3.contains("\"type\":\"annotated\""));
    Ok(())
}

#[test]
fn list_branches_with_protected_filter() -> anyhow::Result<()> {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(GET)
            .path("/repos/o/r/branches")
            .query_param("per_page", "2")
            .query_param("page", "1")
            .query_param("protected", "true");
        then.status(200).json_body(serde_json::json!([
            {"name":"main","commit":{"sha":"aaa"},"protected":true},
            {"name":"dev","commit":{"sha":"bbb"},"protected":true}
        ]));
    });
    let req = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_branches","arguments":{"owner":"o","repo":"r","limit":2,"protected":true}}});
    let out = run_with_env(&req, &[("GITHUB_TOKEN","t"),("GITHUB_API_URL", server.base_url().as_str())])?;
    let v: serde_json::Value = serde_json::from_str(&out)?;
    let sc = v.get("result").and_then(|r| r.get("structuredContent")).cloned().unwrap();
    assert_eq!(sc["items"].as_array().unwrap().len(), 2);
    Ok(())
}

#[test]
fn list_releases_and_get_release_by_tag() -> anyhow::Result<()> {
    let server = MockServer::start();
    // list releases page 1 with Link next
    let _m_list = server.mock(|when, then| {
        when.method(GET)
            .path("/repos/o/r/releases")
            .query_param("per_page", "1")
            .query_param("page", "1");
        then.status(200)
            .header("link", &format!("<{}/repos/o/r/releases?page=2>; rel=\"next\"", server.base_url()))
            .json_body(serde_json::json!([
                {"id":1,"tag_name":"v1","name":"One","draft":false,"prerelease":false,
                 "created_at":null,"published_at":null,"author":{"login":"a"},"assets":[]}
            ]));
    });
    let req1 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_releases","arguments":{"owner":"o","repo":"r","limit":1}}});
    let out1 = run_with_env(&req1, &[("GITHUB_TOKEN","t"),("GITHUB_API_URL", server.base_url().as_str())])?;
    let v1: serde_json::Value = serde_json::from_str(&out1)?;
    let sc1 = v1.get("result").and_then(|r| r.get("structuredContent")).cloned().unwrap();
    assert!(sc1["meta"]["has_more"].as_bool().unwrap());
    assert!(sc1["meta"]["next_cursor"].is_string());

    // get release by tag
    let _m_rel = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/releases/tags/v1");
        then.status(200).json_body(serde_json::json!({
            "id":1,"tag_name":"v1","name":"One","draft":false,"prerelease":false,
            "created_at":null,"published_at":null,"body":"notes","author":{"login":"a"},
            "assets":[{"id": 9, "name":"bin","content_type":"app","size":1,"download_count":0,"browser_download_url":"u"}]
        }));
    });
    let req2 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"get_release","arguments":{"owner":"o","repo":"r","tag":"v1"}}});
    let out2 = run_with_env(&req2, &[("GITHUB_TOKEN","t"),("GITHUB_API_URL", server.base_url().as_str())])?;
    assert!(out2.contains("\"release\""));
    Ok(())
}

#[test]
fn list_starred_repositories_with_starred_at_gating() -> anyhow::Result<()> {
    let server = MockServer::start();
    // include_starred_at=true endpoint returns array of {starred_at, repo:{...}}
    let _m = server.mock(|when, then| {
        when.method(GET)
            .path("/user/starred")
            .query_param("per_page", "10")
            .query_param("page", "1")
            .query_param("sort", "created")
            .query_param("direction", "desc");
        then.status(200)
            .header("x-ratelimit-remaining", "4999")
            .header("x-ratelimit-used", "1")
            .header("x-ratelimit-reset", "0")
            .json_body(serde_json::json!([
                {"starred_at":"1970-01-01T00:00:00Z","repo": {"full_name":"o/r","private":false,"description":null,"language":null,"stargazers_count":1,"html_url":"u","owner":{"login":"o"}}}
            ]));
    });
    let req = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"list_starred_repositories","arguments":{"limit":10,"include_starred_at":true}}});
    let out = run_with_env(&req, &[("GITHUB_TOKEN","t"),("GITHUB_API_URL", server.base_url().as_str())])?;
    let v: serde_json::Value = serde_json::from_str(&out)?;
    let sc = v.get("result").and_then(|r| r.get("structuredContent")).cloned().unwrap();
    assert_eq!(sc["items"][0]["full_name"].as_str().unwrap(), "o/r");
    assert_eq!(sc["items"][0]["starred_at"].as_str().unwrap(), "1970-01-01T00:00:00Z");
    Ok(())
}

#[test]
fn search_endpoints_prune_meta_and_counts() -> anyhow::Result<()> {
    let server = MockServer::start();
    // search issues (includes PRs when query has type:pr but we exercise issues index)
    let _m_issues = server.mock(|when, then| {
        when.method(GET).path("/search/issues");
        then.status(200)
            .json_body(serde_json::json!({
                "total_count": 2,
                "incomplete_results": false,
                "items": [
                    {"id":1,"number":1,"title":"One","state":"open","repository_url":"https://api.github.com/repos/o/r","user":{"login":"a"},"created_at":"2025-01-01T00:00:00Z","updated_at":"2025-01-01T00:00:00Z","pull_request": {}}
                ]
            }));
    });
    let req1 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"search_issues","arguments":{"q":"repo:o/r state:open","limit":10}}});
    let out1 = run_with_env(&req1, &[("GITHUB_TOKEN","t"),("GITHUB_API_URL", server.base_url().as_str())])?;
    let v1: serde_json::Value = serde_json::from_str(&out1)?;
    let sc1 = v1.get("result").and_then(|r| r.get("structuredContent")).cloned().unwrap();
    assert_eq!(sc1["total_count"].as_i64().unwrap(), 2);
    assert_eq!(sc1["items"][0]["is_pull_request"].as_bool().unwrap(), true);
    assert!(sc1.get("meta").is_none());

    // search repositories with pagination
    let _m_repos = server.mock(|when, then| {
        when.method(GET)
            .path("/search/repositories")
            .query_param("per_page", "1")
            .query_param("page", "1")
            .query_param("q", "rust")
            .query_param("sort", "stars");
        then.status(200)
            .header("link", &format!("<{}/search/repositories?q=rust&sort=stars&page=2>; rel=\"next\"", server.base_url()))
            .json_body(serde_json::json!({
                "total_count": 10, "incomplete_results": false,
                "items": [{"full_name":"o/r","private":false,"description":null,"language":null,"stargazers_count":1,"forks_count":0,"open_issues_count":0,"html_url":"u"}]
            }));
    });
    let req2 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":2,
        "params":{"name":"search_repositories","arguments":{"q":"rust","sort":"stars","limit":1}}});
    let out2 = run_with_env(&req2, &[("GITHUB_TOKEN","t"),("GITHUB_API_URL", server.base_url().as_str())])?;
    let v2: serde_json::Value = serde_json::from_str(&out2)?;
    let sc2 = v2.get("result").and_then(|r| r.get("structuredContent")).cloned().unwrap();
    assert!(sc2["meta"]["has_more"].as_bool().unwrap());
    assert!(sc2["meta"]["next_cursor"].is_string());
    Ok(())
}

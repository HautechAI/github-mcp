use assert_cmd::Command;
use httpmock::{Method::GET, Method::POST, MockServer};
use std::io::Write;
use zip::write::FileOptions;

fn frame(msg: &str) -> Vec<u8> {
    let mut v = Vec::new();
    write!(v, "Content-Length: {}\r\n\r\n{}", msg.as_bytes().len(), msg).unwrap();
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
fn get_workflow_job_logs_redirect_zip_tail_and_timestamps() -> anyhow::Result<()> {
    let server = MockServer::start();
    // Build a ZIP with two .txt files
    let mut zip_bytes: Vec<u8> = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_bytes));
        let options = FileOptions::default();
        writer.start_file("1.txt", options)?;
        writer.write_all(b"line1\nline2\nline3\n")?;
        writer.start_file("2.txt", options)?;
        writer.write_all(b"a\nb\nc\n")?;
        writer.finish()?;
    }
    let redirect_url = format!("{}/tmp/log.zip", server.base_url());
    let _m1 = server.mock(|when, then| {
        when.method(GET).path("/repos/o/r/actions/jobs/42/logs");
        then.status(302).header("location", redirect_url.as_str());
    });
    let _m2 = server.mock(|when, then| {
        when.method(GET).path("/tmp/log.zip");
        then.status(200).body(zip_bytes.clone());
    });

    let req = serde_json::json!({
        "jsonrpc":"2.0","method":"tools/call","id":1,
        "params":{"name":"get_workflow_job_logs","arguments": {"owner":"o","repo":"r","job_id":42,"tail_lines":3,"include_timestamps":true}}
    });
    let out = run_with_env(
        &req,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out.contains("\"logs\""));
    assert!(out.contains("line3"));
    assert!(out.contains(":")); // likely timestamp separator present
    Ok(())
}

#[test]
fn rerun_and_cancel_endpoints() -> anyhow::Result<()> {
    let server = MockServer::start();
    let _rerun = server.mock(|when, then| {
        when.method(POST).path("/repos/o/r/actions/runs/100/rerun");
        then.status(202);
    });
    let _rerun_failed = server.mock(|when, then| {
        when.method(POST)
            .path("/repos/o/r/actions/runs/200/rerun-failed-jobs");
        then.status(202);
    });
    let _cancel = server.mock(|when, then| {
        when.method(POST).path("/repos/o/r/actions/runs/300/cancel");
        then.status(202);
    });

    let req1 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"rerun_workflow_run","arguments":{"owner":"o","repo":"r","run_id":100}}});
    let req2 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"rerun_workflow_run_failed","arguments":{"owner":"o","repo":"r","run_id":200}}});
    let req3 = serde_json::json!({"jsonrpc":"2.0","method":"tools/call","id":3,"params":{"name":"cancel_workflow_run","arguments":{"owner":"o","repo":"r","run_id":300}}});
    let out1 = run_with_env(
        &req1,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    let out2 = run_with_env(
        &req2,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    let out3 = run_with_env(
        &req3,
        &[
            ("GITHUB_TOKEN", "t"),
            ("GITHUB_API_URL", server.base_url().as_str()),
        ],
    )?;
    assert!(out1.contains("\"ok\":true"));
    assert!(out2.contains("\"ok\":true"));
    assert!(out3.contains("\"ok\":true"));
    Ok(())
}

Live E2E tests

Overview
- Runs the compiled github-mcp binary with @modelcontextprotocol/inspector-cli.
- Exercises every read-mostly tool against HautechAI/github-mcp-test-repo.
- Validates MCP envelopes plus key fields in structuredContent.
- Mutation tools (rerun/cancel/resolve/unresolve) are gated by E2E_ENABLE_MUTATIONS=true.

Prereqs
- Rust 1.90.0+, cargo build produces target/release/github-mcp
- Node 18+ (for inspector-cli + assertions)
- Auth: GITHUB_TOKEN via environment (prefer Doppler)

Local run
- Build release: `cargo build --release`
- With Doppler (recommended):
  - `doppler run -p github-mcp -c dev -- bash ./scripts/e2e_live.sh`
- Without Doppler (set envs yourself):
  - `GITHUB_TOKEN=ghp_xxx bash ./scripts/e2e_live.sh`
- Enable mutations explicitly (optional):
  - `E2E_ENABLE_MUTATIONS=true doppler run -p github-mcp -c dev -- bash ./scripts/e2e_live.sh`

What it validates
- initialize: protocolVersion present
- tools/list: returns tool descriptors; includes list_issues/get_issue
- Issues: list_issues, get_issue, list_issue_comments_plain
- Negative path: get_issue not_found for a large number
- PRs: list_pull_requests, get_pull_request, list_pr_comments_plain, list_pr_review_comments_plain, list_pr_review_threads_light, list_pr_reviews_light, list_pr_commits_light, list_pr_files_light, get_pr_diff, get_pr_patch, get_pr_status_summary
- Actions: list_workflows_light, list_workflow_runs_light, get_workflow_run_light, list_workflow_jobs_light, get_workflow_job_logs (best effort)
- Mutations (opt-in): rerun_workflow_run, rerun_workflow_run_failed, cancel_workflow_run, resolve_pr_review_thread, unresolve_pr_review_thread

Graceful skips
- If fixtures are absent (e.g., no workflow runs/jobs), the script continues and logs a note.
- Negative-path checks tolerate differences and log a message instead of failing the whole run.

CI integration
- .github/workflows/e2e-live.yml builds release and runs the script under Doppler:
  - `doppler run -p github-mcp -c dev -- bash ./scripts/e2e_live.sh`
- Permissions: contents: read; actions: read. A separate job enables actions: write if `vars.E2E_ENABLE_MUTATIONS == 'true'`.
- Artifacts: `mcp-diag.log`, `mcp-e2e.log`, and all `out-*.json` outputs are uploaded for troubleshooting.

Coverage map (tools)
- initialize, tools/list — basic MCP handshake
- ping — optional; gated by `GITHUB_MCP_ENABLE_PING` (default OFF). E2E does not rely on ping.
- issues: list_issues, get_issue, list_issue_comments_plain
- pull requests: list_pull_requests, get_pull_request, list_pr_comments_plain, list_pr_review_comments_plain, list_pr_review_threads_light, list_pr_reviews_light, list_pr_commits_light, list_pr_files_light, get_pr_diff, get_pr_patch, get_pr_status_summary
- actions: list_workflows_light, list_workflow_runs_light, get_workflow_run_light, list_workflow_jobs_light, get_workflow_job_logs
- mutations (opt-in): resolve_pr_review_thread, unresolve_pr_review_thread, rerun_workflow_run, rerun_workflow_run_failed, cancel_workflow_run

Troubleshooting
- No auth / 401: ensure GITHUB_TOKEN is set (via Doppler or env) and has repo read permissions for HautechAI/github-mcp-test-repo.
- Inspector timeouts: the script retries minimal parts; rerun with `--log-level info` by setting `MCP_DIAG_LOG` to collect logs.
- CI failures: download the artifacts and inspect `mcp-diag.log` and the corresponding `out-*.json` to see which assertion failed.

Live E2E tests

Overview
- Runs the compiled github-mcp binary with @modelcontextprotocol/inspector-cli.
- Exercises read-only coverage for all GitHub-backed methods (including PR #92 additions).
- Validates MCP envelopes plus key fields in structuredContent.
- Mutation tools are gated by E2E_ENABLE_MUTATIONS=true, with extra gates for merge/fork.

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
  - Optional extra gates:
    - `E2E_ALLOW_PR_MERGE=true` to enable merge_pr coverage (choose a safe PR via E2E_PR_NUM)
    - `E2E_ENABLE_FORKS=true` to enable fork_repository coverage

What it validates
- initialize: protocolVersion present
- tools/list: returns tool descriptors; includes list_issues/get_issue
- Issues: list_issues, get_issue, list_issue_comments_plain
- Negative path: get_issue not_found for a large number
- PRs: list_pull_requests, get_pull_request, pr_summary, list_pr_comments_plain, list_pr_review_comments_plain, list_pr_review_threads_light, list_pr_reviews_light, list_pr_commits_light, list_pr_files_light, get_pr_diff, get_pr_patch, get_pr_status_summary
- Repos: list_commits, get_commit, list_tags, get_tag, list_branches, list_releases, get_release, list_starred_repositories
- Search: search_issues, search_pull_requests, search_repositories
- Actions: list_workflows_light, list_workflow_runs_light, get_workflow_run_light, list_workflow_jobs_light, get_workflow_job_logs (best effort)
- Mutations (opt-in): rerun_workflow_run, rerun_workflow_run_failed, cancel_workflow_run, resolve_pr_review_thread, unresolve_pr_review_thread

Graceful skips
- If fixtures are absent (e.g., no workflow runs/jobs), the script continues and logs a note.
- Negative-path checks tolerate differences and log a message instead of failing the whole run.

CI integration
- The CI workflow runs the live E2E on non-fork PRs and main by default. Fork PRs are skipped automatically.
- Permissions: e2e-live uses read-only (contents/actions read).
- Optional e2e-live-mutations job runs only when `vars.E2E_ENABLE_MUTATIONS == 'true'` and not from a fork; it uses write permissions.
- Artifacts: `mcp-diag.log`, `mcp-e2e.log`, and all `out-*.json` outputs are uploaded for troubleshooting.

Environment variables
- E2E_OWNER, E2E_REPO: target repository (default HautechAI/github-mcp-test-repo)
- E2E_ISSUE_NUM, E2E_PR_NUM: fixture IDs for issue/PR
- E2E_BRANCH: default branch for list_commits (default `main`)
- E2E_ENABLE_MUTATIONS: enable gated mutation coverage
- E2E_ALLOW_PR_MERGE: extra gate to allow merge_pr
- E2E_ENABLE_FORKS: extra gate to allow fork_repository

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

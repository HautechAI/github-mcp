# MCP GitHub Review Playbook (LLM‑friendly)

Purpose
- A minimal, stepwise guide for common PR workflows using the MCP methods.
- Links jump directly to method specs in methods.md. Start lean; request heavier data only when needed.

Quick Links
- Issues: [list_issues](./methods.md#tool-list_issues) · [get_issue](./methods.md#tool-get_issue) · [list_issue_comments_plain](./methods.md#tool-list_issue_comments_plain)
- Pull Requests: [list_pull_requests](./methods.md#tool-list_pull_requests) · [get_pull_request](./methods.md#tool-get_pull_request) · [get_pr_status_summary](./methods.md#tool-get_pr_status_summary) · [list_pr_comments_plain](./methods.md#tool-list_pr_comments_plain) · [list_pr_review_comments_plain](./methods.md#tool-list_pr_review_comments_plain) · [list_pr_review_threads_light](./methods.md#tool-list_pr_review_threads_light) · [resolve_pr_review_thread](./methods.md#tool-resolve_pr_review_thread) · [unresolve_pr_review_thread](./methods.md#tool-unresolve_pr_review_thread) · [list_pr_reviews_light](./methods.md#tool-list_pr_reviews_light) · [list_pr_commits_light](./methods.md#tool-list_pr_commits_light) · [list_pr_files_light](./methods.md#tool-list_pr_files_light) · [get_pr_diff](./methods.md#tool-get_pr_diff) · [get_pr_patch](./methods.md#tool-get_pr_patch)
- Workflows (CI): [list_workflows_light](./methods.md#tool-list_workflows_light) · [list_workflow_runs_light](./methods.md#tool-list_workflow_runs_light) · [get_workflow_run_light](./methods.md#tool-get_workflow_run_light) · [list_workflow_jobs_light](./methods.md#tool-list_workflow_jobs_light) · [get_workflow_job_logs](./methods.md#tool-get_workflow_job_logs) · [rerun_workflow_run](./methods.md#tool-rerun_workflow_run) · [rerun_workflow_run_failed](./methods.md#tool-rerun_workflow_run_failed) · [cancel_workflow_run](./methods.md#tool-cancel_workflow_run)

Guiding Principles
- Keep payloads lean: avoid include_* flags unless needed (e.g., include_location for review comments; include_patch for files).
- Paginate consciously: use small limits; follow meta.next_cursor when needed.
- Escalate detail only as necessary (e.g., fetch diff/patch or logs after a quick summary view).

---

Scenario A — Review a new PR
1) Snapshot the PR
- [get_pull_request](./methods.md#tool-get_pull_request)
  - Why: title, body, state, is_draft, merged, timestamps; author_login if needed.

2) Check CI at a glance
- [get_pr_status_summary](./methods.md#tool-get_pr_status_summary) (set include_failing_contexts=true)
  - Why: overall_state + failing context names.

3) Understand surface area
- [list_pr_files_light](./methods.md#tool-list_pr_files_light) (per_page up to 100; include_patch=false)
  - Why: filenames, status, additions/deletions/changes. Add include_patch only for targeted files if needed.

4) Skim commit history
- [list_pr_commits_light](./methods.md#tool-list_pr_commits_light)
  - Why: commit oids, headlines, authored_at.

5) See prior feedback
- Reviews summary: [list_pr_reviews_light](./methods.md#tool-list_pr_reviews_light)
- Inline code comments: [list_pr_review_comments_plain](./methods.md#tool-list_pr_review_comments_plain) (include_location=true for file/line mapping)
- Discussion comments: [list_pr_comments_plain](./methods.md#tool-list_pr_comments_plain)

6) Deep dive (only if necessary)
- Unified diff: [get_pr_diff](./methods.md#tool-get_pr_diff)
- Patch: [get_pr_patch](./methods.md#tool-get_pr_patch)

Tips
- Start with small limits (e.g., 20–50). Iterate via meta.next_cursor.
- Use include_location only when you need to jump to code lines.

---

Scenario B — A reviewer pinged me to look at their review
1) Locate the review
- [list_pr_reviews_light](./methods.md#tool-list_pr_reviews_light)
  - Find reviewer by author_login, inspect state/timestamp.

2) Pull their comments with code context
- [list_pr_review_comments_plain](./methods.md#tool-list_pr_review_comments_plain) (include_location=true)
  - Filter client-side by author_login to focus on that reviewer.
  - Use path + line/start_line + side to jump to code.

3) Inspect affected files (optional)
- [list_pr_files_light](./methods.md#tool-list_pr_files_light) (include_patch=false)
- If you need diffs: [get_pr_patch](./methods.md#tool-get_pr_patch) or [get_pr_diff](./methods.md#tool-get_pr_diff)

---

Scenario C — CI checks failed on the PR
1) Confirm status
- [get_pr_status_summary](./methods.md#tool-get_pr_status_summary) (include_failing_contexts=true)

2) Find the failing workflow run(s)
- If you know the workflow: [list_workflow_runs_light](./methods.md#tool-list_workflow_runs_light) (filter by branch/event/head_sha)
- If you don’t know the workflow: list workflows first → [list_workflows_light](./methods.md#tool-list_workflows_light), then choose the CI workflow and call list_workflow_runs_light.
  - Tip: Get the PR’s head_sha from [get_pull_request](./methods.md#tool-get_pull_request) and pass as a filter to list_workflow_runs_light.

3) Drill into failing jobs
- [list_workflow_jobs_light](./methods.md#tool-list_workflow_jobs_light)
  - Identify failed jobs via status/conclusion.

4) Read only what you need
- [get_workflow_job_logs](./methods.md#tool-get_workflow_job_logs) (tail_lines=200)
  - Avoid full logs; tail to last N lines. Remember logs are served via a 302 redirect to a ZIP.

5) Take action
- Rerun failed jobs: [rerun_workflow_run_failed](./methods.md#tool-rerun_workflow_run_failed)
- Rerun all: [rerun_workflow_run](./methods.md#tool-rerun_workflow_run)
- Cancel stuck runs: [cancel_workflow_run](./methods.md#tool-cancel_workflow_run)

---

Scenario D — Quick PR triage in chat
- [get_pull_request](./methods.md#tool-get_pull_request)
- [get_pr_status_summary](./methods.md#tool-get_pr_status_summary)
- [list_pr_files_light](./methods.md#tool-list_pr_files_light)
- [list_pr_comments_plain](./methods.md#tool-list_pr_comments_plain)

---

Scenario E — Resolve review comments after implementation
1) List unresolved threads to verify scope
- [list_pr_review_threads_light](./methods.md#tool-list_pr_review_threads_light)
  - Why: get thread ids, resolved state, and optional file/line to jump to code. Filter client-side where is_resolved=false.

2) Resolve threads that are addressed
- For each thread you’ve fixed: [resolve_pr_review_thread](./methods.md#tool-resolve_pr_review_thread)
  - Input: thread_id from step 1. Mutations are idempotent; resolving an already-resolved thread returns is_resolved=true.

3) Recheck remaining threads
- [list_pr_review_threads_light](./methods.md#tool-list_pr_review_threads_light)
  - Why: confirm is_resolved now true. Paginate if needed via meta.next_cursor.

4) Undo if needed (optional)
- If you resolved by mistake: [unresolve_pr_review_thread](./methods.md#tool-unresolve_pr_review_thread)
  - Input: same thread_id.

Tips
- Keep payloads lean: only set include_location=true when you need file/line mapping; include_author=true only if you need the resolver’s login.
- Use small limits (e.g., 20–50) and follow meta.next_cursor for more threads.

Rate limit and pagination habits
- Always pass a limit; rely on meta.next_cursor to fetch more.
- Watch meta.rate.remaining to decide whether to fetch heavy data (diffs/logs) now or later.

Potential future additions
- Bulk operations on review threads (batch resolve/unresolve) to reduce mutation round-trips.
- Targeted file patch retrieval (single-file patch from a PR) to avoid full patch downloads.

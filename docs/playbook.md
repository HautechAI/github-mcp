# MCP GitHub Review Playbook (LLM‑friendly)

Purpose
- A minimal, stepwise guide for common PR workflows using the MCP methods.
- Links jump directly to method specs in methods.md. Start lean; request heavier data only when needed.

Quick Links
- Issues: [list_issues](./methods.md#tool-list_issues) · [get_issue](./methods.md#tool-get_issue) · [list_issue_comments_plain](./methods.md#tool-list_issue_comments_plain)
- Pull Requests: [list_pull_requests](./methods.md#tool-list_pull_requests) · [search_pull_requests](./methods.md#tool-search_pull_requests) · [get_pull_request](./methods.md#tool-get_pull_request) · [get_pr_status_summary](./methods.md#tool-get_pr_status_summary) · [list_pr_comments_plain](./methods.md#tool-list_pr_comments_plain) · [list_pr_review_comments_plain](./methods.md#tool-list_pr_review_comments_plain) · [list_pr_review_threads_light](./methods.md#tool-list_pr_review_threads_light) · [resolve_pr_review_thread](./methods.md#tool-resolve_pr_review_thread) · [unresolve_pr_review_thread](./methods.md#tool-unresolve_pr_review_thread) · [list_pr_reviews_light](./methods.md#tool-list_pr_reviews_light) · [list_pr_commits_light](./methods.md#tool-list_pr_commits_light) · [list_pr_files_light](./methods.md#tool-list_pr_files_light) · [get_pr_diff](./methods.md#tool-get_pr_diff) · [get_pr_patch](./methods.md#tool-get_pr_patch) · [update_pull_request_branch](./methods.md#tool-update_pull_request_branch) · [pull_request_toggle_draft](./methods.md#tool-pull_request_toggle_draft)
- Reviews (write): [create_or_submit_review](./methods.md#tool-create_or_submit_review) · [add_review_comment](./methods.md#tool-add_review_comment)
- Triage helpers: [issues_add_labels](./methods.md#tool-issues_add_labels) · [issues_set_labels](./methods.md#tool-issues_set_labels) · [issues_remove_label](./methods.md#tool-issues_remove_label) · [pulls_request_reviewers](./methods.md#tool-pulls_request_reviewers) · [pulls_remove_requested_reviewers](./methods.md#tool-pulls_remove_requested_reviewers) · [issues_add_assignees](./methods.md#tool-issues_add_assignees) · [issues_remove_assignees](./methods.md#tool-issues_remove_assignees)
- Workflows (CI): [list_workflows_light](./methods.md#tool-list_workflows_light) · [list_workflow_runs_light](./methods.md#tool-list_workflow_runs_light) · [get_workflow_run_light](./methods.md#tool-get_workflow_run_light) · [list_workflow_jobs_light](./methods.md#tool-list_workflow_jobs_light) · [get_workflow_job_logs](./methods.md#tool-get_workflow_job_logs) · [rerun_workflow_run](./methods.md#tool-rerun_workflow_run) · [rerun_workflow_run_failed](./methods.md#tool-rerun_workflow_run_failed) · [cancel_workflow_run](./methods.md#tool-cancel_workflow_run) · [actions_list_run_artifacts](./methods.md#tool-actions_list_run_artifacts) · [actions_download_artifact](./methods.md#tool-actions_download_artifact)

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
  - Tip: Get the PR’s head_sha from [get_pull_request](./methods.md#tool-get_pull_request) (set include_head_sha=true) or from commits(last:1) via GraphQL, and pass as a filter to list_workflow_runs_light.

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

---

Scenario F — Merge readiness triage
1) Snapshot merge readiness
- [get_pull_request](./methods.md#tool-get_pull_request) (include_merge_readiness=true, include_author=true)
  - Why: review_decision, mergeable, merge_state_status, merge queue/auto-merge hints.

2) Confirm CI rollup
- [get_pr_status_summary](./methods.md#tool-get_pr_status_summary) (include_failing_contexts=true)
  - Why: ensure overall_state is SUCCESS before attempting merge.

3) Check unresolved review threads
- [list_pr_review_threads_light](./methods.md#tool-list_pr_review_threads_light)
  - Filter is_resolved=false; optionally include_location to jump to code.

Tips
- REVIEW_REQUIRED or failing CI usually blocks merge; CHANGES_REQUESTED suggests action on feedback.
- If merge_state_status is BEHIND, consider Scenario G to update the branch.

---

Scenario G — Update PR branch from base
1) Confirm head SHA to avoid races
- [get_pull_request](./methods.md#tool-get_pull_request) (include_head_sha=true)

2) Request update from base
- [update_pull_request_branch](./methods.md#tool-update_pull_request_branch) (expected_head_sha=from step 1)
  - Why: queue a server-side merge from base into the PR branch; safe-guard with expected_head_sha.

3) Recheck CI after update
- Repeat Scenario C to monitor runs triggered by the update.

---

Scenario H — Triage labels, reviewers, and assignees
1) Labels
- Add: [issues_add_labels](./methods.md#tool-issues_add_labels)
- Replace all: [issues_set_labels](./methods.md#tool-issues_set_labels)
- Remove one: [issues_remove_label](./methods.md#tool-issues_remove_label)

2) Reviewers
- Request: [pulls_request_reviewers](./methods.md#tool-pulls_request_reviewers)
- Remove requests: [pulls_remove_requested_reviewers](./methods.md#tool-pulls_remove_requested_reviewers)

3) Assignees
- Add: [issues_add_assignees](./methods.md#tool-issues_add_assignees)
- Remove: [issues_remove_assignees](./methods.md#tool-issues_remove_assignees)

---

Scenario I — Toggle draft/ready for review
1) Fetch PR id and state
- [get_pull_request](./methods.md#tool-get_pull_request)
  - Keep the GraphQL id for the next step.

2) Toggle state
- [pull_request_toggle_draft](./methods.md#tool-pull_request_toggle_draft) (action=to_draft | ready_for_review)
  - Why: communicate readiness without closing the PR.

---

Scenario J — Download CI artifacts
1) Locate the run
- Use Scenario C steps 1–3 to find the workflow run id.

2) List artifacts for the run
- [actions_list_run_artifacts](./methods.md#tool-actions_list_run_artifacts)
  - Choose by name or size; note expiration.

3) Download the desired artifact
- [actions_download_artifact](./methods.md#tool-actions_download_artifact)
  - Server returns base64-encoded ZIP bytes; save locally if needed.

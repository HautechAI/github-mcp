Title: Minimal MCP Server Interface for GitHub (LLM-friendly)

Design goals
- Lean payloads by default; expand via flags only.
- Prefer GraphQL for selective fields and cursor pagination; use REST where required (diff/patch, Actions).
- Consistent error meta and pagination across tools.

Shared conventions
- Pagination (inputs): cursor (string, optional), limit (int, default 30, max 100). For REST fallbacks, server maps cursor to page/per_page.
- Pagination (outputs): meta.next_cursor (string | null), meta.has_more (bool).
- Rate limit meta (outputs): meta.rate { remaining (int), used (int), reset_at (iso8601, optional) }.
- Error shape: { error: { code (string), message (string), retriable (bool) } } — omitted on success.
- Auth: PAT/token via server config; tools only take owner, repo, ids.
- Ids: Prefer GraphQL node id when using GraphQL; otherwise REST id. Always include number for issues/PRs.
- Timestamps: iso8601.
- Users: not expanded by default; author_login optional via include_author.

ISSUES

Tool: list_issues
- Input
  { owner (string), repo (string), state (enum: open|closed|all, optional), labels (string[], optional), creator (string, optional), assignee (string|"*", optional), mentions (string, optional), since (iso8601, optional), sort (enum: created|updated|comments, optional), direction (enum: asc|desc, optional), cursor (string, optional), limit (int, optional), include_author (bool, optional, default false) }
- Output
  { items: [ { id (string), number (int), title (string), state (string), created_at (string), updated_at (string), author_login (string, optional) } ], meta: { next_cursor (string|null), has_more (bool), rate { remaining, used, reset_at? } }, error? }
- API
  Preferred: GraphQL (fallback: REST)
  GraphQL: repository -> issues(after: $cursor, first: $limit, states, labels, filterBy)
    Fields: id, number, title, state, createdAt, updatedAt, author { login }
  REST: GET /repos/{owner}/{repo}/issues?state=&labels=&creator=&assignee=&since=&sort=&direction=&per_page=&page

Tool: get_issue
- Input
  { owner (string), repo (string), number (int), include_author (bool, optional, default false) }
- Output
  { item: { id (string), number (int), title (string), body (string, optional), state (string), created_at (string), updated_at (string), author_login (string, optional) }, meta: { rate { remaining, used, reset_at? } }, error? }
- API
  Preferred: GraphQL (fallback: REST)
  GraphQL: repository { issue(number: $n) { id, number, title, state, createdAt, updatedAt, author { login }, body } }
  REST: GET /repos/{owner}/{repo}/issues/{number}

Tool: list_issue_comments_plain
- Input
  { owner (string), repo (string), number (int), cursor (string, optional), limit (int, optional), include_author (bool, optional, default false) }
- Output
  { items: [ { id (string), body (string), author_login (string, optional), created_at (string), updated_at (string) } ], meta: { next_cursor, has_more, rate { remaining, used, reset_at? } }, error? }
- API
  Preferred: GraphQL (fallback: REST)
  GraphQL: repository { issue(number: $n) { comments(first: $limit, after: $cursor) { nodes { id, body, createdAt, updatedAt, author { login } } } } }
  REST: GET /repos/{owner}/{repo}/issues/{number}/comments?per_page=&page

PULL REQUESTS

Tool: list_pull_requests
- Input
  { owner (string), repo (string), state (enum: open|closed|all, optional), base (string, optional), head (string, optional), cursor (string, optional), limit (int, optional), include_author (bool, optional, default false) }
- Output
  { items: [ { id (string), number (int), title (string), state (string), created_at (string), updated_at (string), author_login (string, optional) } ], meta: { next_cursor, has_more, rate { remaining, used, reset_at? } }, error? }
- API
  Preferred: GraphQL (fallback: REST)
  GraphQL: repository { pullRequests(first: $limit, after: $cursor, states: [...], baseRefName: $base, headRefName: $head, orderBy: {field: UPDATED_AT, direction: DESC}) { nodes { id number title state createdAt updatedAt author { login } } pageInfo { hasNextPage endCursor } } }
  REST: GET /repos/{owner}/{repo}/pulls?state=&head=&base=&per_page=&page

Tool: get_pull_request
- Input
  { owner (string), repo (string), number (int), include_author (bool, optional, default false) }
- Output
  { item: { id (string), number (int), title (string), body (string, optional), state (string), is_draft (bool), created_at (string), updated_at (string), merged (bool), merged_at (string|null), author_login (string, optional) }, meta: { rate { remaining, used, reset_at? } }, error? }
- API
  Preferred: GraphQL (fallback: REST)
  GraphQL: repository { pullRequest(number: $n) { id number title state isDraft createdAt updatedAt merged mergedAt author { login } body } }
  REST: GET /repos/{owner}/{repo}/pulls/{number}

Tool: get_pr_status_summary
- Input
  { owner (string), repo (string), number (int), include_failing_contexts (bool, optional, default false), limit_contexts (int, optional, default 10) }
- Output
  { item: { overall_state (string: SUCCESS|PENDING|FAILURE), counts: { success (int), pending (int), failure (int) }, failing_contexts (string[], optional) }, meta: { rate { remaining, used, reset_at? } }, error? }
- API
  Preferred: GraphQL (fallback: REST)
  GraphQL: repository { pullRequest(number: $n) { commits(last: 1) { nodes { commit { oid statusCheckRollup { state contexts(first: $limit) { nodes { ... on CheckRun { name conclusion } ... on StatusContext { context state } } } } } } } } }
   - Map state/conclusion to success/pending/failure. For failing_contexts, use CheckRun.name or StatusContext.context where failure indicated.
  REST fallback:
   - GET /repos/{owner}/{repo}/pulls/{number} -> head.sha
   - GET /repos/{owner}/{repo}/commits/{sha}/status
   - GET /repos/{owner}/{repo}/commits/{sha}/check-runs

Tool: list_pr_comments_plain (PR issue comments)
- Input
  { owner (string), repo (string), number (int), cursor (string, optional), limit (int, optional), include_author (bool, optional, default false) }
- Output
  { items: [ { id (string), body (string), author_login (string, optional), created_at (string), updated_at (string) } ], meta: { next_cursor, has_more, rate { remaining, used, reset_at? } }, error? }
- API
  Preferred: GraphQL (fallback: REST)
  GraphQL: repository { pullRequest(number: $n) { comments(first: $limit, after: $cursor) { nodes { id body createdAt updatedAt author { login } } } } }
  REST: GET /repos/{owner}/{repo}/issues/{number}/comments

Tool: list_pr_review_comments_plain (code review comments)
- Input
  { owner (string), repo (string), number (int), cursor (string, optional), limit (int, optional), include_author (bool, optional, default false) }
- Output
  { items: [ { id (string), body (string), author_login (string, optional), created_at (string), updated_at (string) } ], meta: { next_cursor, has_more, rate { remaining, used, reset_at? } }, error? }
- API
  Preferred: GraphQL (fallback: REST)
  GraphQL: repository { pullRequest(number: $n) { reviewComments(first: $limit, after: $cursor) { nodes { id body createdAt updatedAt author { login } } } } }
  REST: GET /repos/{owner}/{repo}/pulls/{number}/comments

Tool: list_pr_reviews_light (review summaries)
- Input
  { owner (string), repo (string), number (int), cursor (string, optional), limit (int, optional), include_author (bool, optional, default false) }
- Output
  { items: [ { id (string), state (string), submitted_at (string|null), author_login (string, optional) } ], meta: { next_cursor, has_more, rate { remaining, used, reset_at? } }, error? }
- API
  Preferred: GraphQL (fallback: REST)
  GraphQL: repository { pullRequest(number: $n) { reviews(first: $limit, after: $cursor) { nodes { id state submittedAt author { login } } } } }
  REST: GET /repos/{owner}/{repo}/pulls/{number}/reviews

Tool: list_pr_commits_light
- Input
  { owner (string), repo (string), number (int), cursor (string, optional), limit (int, optional), include_author (bool, optional, default false) }
- Output
  { items: [ { sha (string), title (string), authored_at (string), author_login (string, optional) } ], meta: { next_cursor, has_more, rate { remaining, used, reset_at? } }, error? }
- API
  Preferred: GraphQL (fallback: REST)
  GraphQL: repository { pullRequest(number: $n) { commits(first: $limit, after: $cursor) { nodes { commit { oid messageHeadline authoredDate author { user { login } } } } } } }
  REST: GET /repos/{owner}/{repo}/pulls/{number}/commits

Tool: list_pr_files_light
- Input
  { owner (string), repo (string), number (int), page (int, optional), per_page (int, optional), include_patch (bool, optional, default false) }
- Output
  { items: [ { filename (string), status (string), additions (int), deletions (int), changes (int), sha (string), patch (string, optional) } ], meta: { next_cursor (string|null), has_more (bool), rate { remaining, used, reset_at? } }, error? }
- API
  Required: REST
  REST: GET /repos/{owner}/{repo}/pulls/{number}/files?per_page=&page (omit patch unless include_patch=true)

Tool: get_pr_diff
- Input
  { owner (string), repo (string), number (int) }
- Output
  { diff (string), meta: { rate { remaining, used, reset_at? } }, error? }
- API
  Required: REST
  REST: GET /repos/{owner}/{repo}/pulls/{number} with Accept: application/vnd.github.v3.diff

Tool: get_pr_patch
- Input
  { owner (string), repo (string), number (int) }
- Output
  { patch (string), meta: { rate { remaining, used, reset_at? } }, error? }
- API
  Required: REST
  REST: GET /repos/{owner}/{repo}/pulls/{number} with Accept: application/vnd.github.v3.patch

WORKFLOWS (GitHub Actions)

Tool: list_workflows_light
- Input
  { owner (string), repo (string), page (int, optional), per_page (int, optional) }
- Output
  { items: [ { id (int), name (string), path (string), state (string) } ], meta: { next_cursor (string|null), has_more (bool), rate { remaining, used, reset_at? } }, error? }
- API
  Required: REST
  REST: GET /repos/{owner}/{repo}/actions/workflows?per_page=&page

Tool: list_workflow_runs_light
- Input
  { owner (string), repo (string), workflow_id (int|string), status (string, optional), branch (string, optional), actor (string, optional), event (string, optional), created (string, optional), head_sha (string, optional), page (int, optional), per_page (int, optional) }
- Output
  { items: [ { id (int), run_number (int), event (string), status (string), conclusion (string|null), head_sha (string), created_at (string), updated_at (string) } ], meta: { next_cursor, has_more, rate { remaining, used, reset_at? } }, error? }
- API
  Required: REST
  REST: GET /repos/{owner}/{repo}/actions/workflows/{workflow_id}/runs?status=&branch=&actor=&event=&created=&head_sha=&per_page=&page

Tool: get_workflow_run_light
- Input
  { owner (string), repo (string), run_id (int), exclude_pull_requests (bool, optional) }
- Output
  { item: { id (int), run_number (int), event (string), status (string), conclusion (string|null), head_sha (string), created_at (string), updated_at (string) }, meta: { rate { remaining, used, reset_at? } }, error? }
- API
  Required: REST
  REST: GET /repos/{owner}/{repo}/actions/runs/{run_id}?exclude_pull_requests=true|false

Tool: list_workflow_jobs_light
- Input
  { owner (string), repo (string), run_id (int), filter (enum: latest|all, optional), page (int, optional), per_page (int, optional) }
- Output
  { items: [ { id (int), name (string), status (string), conclusion (string|null), started_at (string|null), completed_at (string|null) } ], meta: { next_cursor, has_more, rate { remaining, used, reset_at? } }, error? }
- API
  Required: REST
  REST: GET /repos/{owner}/{repo}/actions/runs/{run_id}/jobs?filter=&per_page=&page

Tool: get_workflow_job_logs
- Input
  { owner (string), repo (string), job_id (int), tail_lines (int, optional), include_timestamps (bool, optional, default false) }
- Output
  { logs (string), truncated (bool), meta: { rate { remaining, used, reset_at? } }, error? }
- API
  Required: REST
  REST: GET /repos/{owner}/{repo}/actions/jobs/{job_id}/logs
  Notes: API returns a 302 redirect to a temporary ZIP of logs. Server should fetch, extract, and optionally return last N lines (tail_lines) to avoid bloat.

Tool: rerun_workflow_run
- Input
  { owner (string), repo (string), run_id (int) }
- Output
  { ok (bool), queued_run_id (int|null), meta: { rate { remaining, used, reset_at? } }, error? }
- API
  Required: REST
  REST: POST /repos/{owner}/{repo}/actions/runs/{run_id}/rerun

Tool: rerun_workflow_run_failed
- Input
  { owner (string), repo (string), run_id (int) }
- Output
  { ok (bool), queued_run_id (int|null), meta: { rate { remaining, used, reset_at? } }, error? }
- API
  Required: REST
  REST: POST /repos/{owner}/{repo}/actions/runs/{run_id}/rerun-failed-jobs

Tool: cancel_workflow_run
- Input
  { owner (string), repo (string), run_id (int) }
- Output
  { ok (bool), meta: { rate { remaining, used, reset_at? } }, error? }
- API
  Required: REST
  REST: POST /repos/{owner}/{repo}/actions/runs/{run_id}/cancel

CROSS-CUTTING NOTES
- Pagination model
  - GraphQL tools: use cursor/limit; output meta.next_cursor from endCursor; has_more from pageInfo.hasNextPage.
  - REST tools: accept page/per_page; also output meta.next_cursor encoding next page (e.g., "page:2"). MCP clients should prefer cursor when present.
- Rate limit meta
  - Populate meta.rate from REST response headers (X-RateLimit-Remaining/Used/Reset) and GraphQL rateLimit where available.
- Error shape
  - On any failure, return { error: { code, message, retriable } } with no items/item payload. Map 429/5xx as retriable=true; 4xx as retriable=false.
- Authentication
  - Token managed by server; tools only require owner/repo and identifiers. No URLs or web links returned by default.
- API choices rationale
  - GraphQL preferred for list/get of issues/PRs/comments/reviews/commits due to selective fields and cursor pagination.
  - REST required for: diffs/patches (media types), PR files (patch access, stable REST pagination), and all Actions workflow operations and logs.
- Comment payload discipline
  - "Plain" variants only include: id, body, author_login (optional), created_at, updated_at. No reactions, URLs, or user objects.
- PR status summary
  - Favor GraphQL statusCheckRollup for holistic state; fallback composes REST statuses and checks. Only expose minimal counts and optional failing contexts.
- Diffs
  - Provide unified diff or patch as raw strings via REST Accept headers. Never embed file blobs or binary content.

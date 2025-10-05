Title: Minimal MCP Server Interface for GitHub (LLM-friendly)

Design goals
- Lean payloads by default; expand via flags only.
- Prefer GraphQL for selective fields and cursor pagination; use REST where required (diff/patch, Actions).
- Consistent error meta and pagination across tools.

Tools Index
- Issues: [list_issues](#tool-list_issues), [get_issue](#tool-get_issue), [list_issue_comments_plain](#tool-list_issue_comments_plain)
- Pull Requests: [list_pull_requests](#tool-list_pull_requests), [get_pull_request](#tool-get_pull_request), [get_pr_status_summary](#tool-get_pr_status_summary), [list_pr_comments_plain](#tool-list_pr_comments_plain), [list_pr_review_comments_plain](#tool-list_pr_review_comments_plain), [list_pr_reviews_light](#tool-list_pr_reviews_light), [list_pr_commits_light](#tool-list_pr_commits_light), [list_pr_files_light](#tool-list_pr_files_light), [get_pr_diff](#tool-get_pr_diff), [get_pr_patch](#tool-get_pr_patch)
- Workflows: [list_workflows_light](#tool-list_workflows_light), [list_workflow_runs_light](#tool-list_workflow_runs_light), [get_workflow_run_light](#tool-get_workflow_run_light), [list_workflow_jobs_light](#tool-list_workflow_jobs_light), [get_workflow_job_logs](#tool-get_workflow_job_logs), [rerun_workflow_run](#tool-rerun_workflow_run), [rerun_workflow_run_failed](#tool-rerun_workflow_run_failed), [cancel_workflow_run](#tool-cancel_workflow_run)

Shared conventions
- Pagination (inputs): cursor (string, optional), limit (int, default 30, max 100). For REST fallbacks, server maps cursor to page/per_page.
- Pagination (outputs): meta.next_cursor (string or null), meta.has_more (bool).
- Rate limit meta (outputs): meta.rate { remaining (int), used (int), reset_at (iso8601, optional) }.
- Error shape: omitted on success; present as below.
- Auth: PAT/token via server config; tools only take owner, repo, ids.
- Ids: Prefer GraphQL node id when using GraphQL; otherwise REST id. Always include number for issues/PRs.
- Timestamps: iso8601.
- Users: not expanded by default; author_login optional via include_author.

Common shapes

- Pagination params (inputs)

| param | type | default | notes |
| --- | --- | --- | --- |
| cursor | string |  | GraphQL cursor; for REST tools, server encodes next page (e.g., "page:2") |
| limit | int | 30 | max 100; server maps to per_page for REST |
| page | int |  | REST only; accepted by some tools |
| per_page | int |  | REST only; accepted by some tools |

- Meta fields (outputs)

| key | type | notes |
| --- | --- | --- |
| meta.next_cursor | string or null | opaque cursor; for REST tools encodes next page (e.g., "page:2") |
| meta.has_more | bool | true if additional pages exist |
| meta.rate.remaining | int | remaining requests in window |
| meta.rate.used | int | used requests in window |
| meta.rate.reset_at | iso8601 (optional) | reset time; populated when available |

- Error shape

| key | type | notes |
| --- | --- | --- |
| error.code | string | short machine code |
| error.message | string | human-readable message |
| error.retriable | bool | true for 429/5xx, false for 4xx |

- Rate limit sources

| API | fields |
| --- | --- |
| REST | X-RateLimit-Remaining, X-RateLimit-Used, X-RateLimit-Reset |
| GraphQL | rateLimit { remaining, used, resetAt } |

ISSUES

## Tool: list_issues
Purpose: List issues with optional filters and minimal fields.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| state | enum | no |  | open, closed, all |  |
| labels | string[] | no |  |  | comma-joined for REST fallback |
| creator | string | no |  |  |  |
| assignee | string | no |  |  | supports "*" in REST fallback |
| mentions | string | no |  |  |  |
| since | iso8601 | no |  |  |  |
| sort | enum | no |  | created, updated, comments |  |
| direction | enum | no |  | asc, desc |  |
| cursor | string | no |  |  | GraphQL cursor; server maps to page/per_page for REST |
| limit | int | no | 30 |  | max 100 |
| include_author | bool | no | false |  | adds author_login when true |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| items[].id | string | always | GraphQL node id or REST id |
| items[].number | int | always |  |
| items[].title | string | always |  |
| items[].state | string | always |  |
| items[].created_at | string | always | iso8601 |
| items[].updated_at | string | always | iso8601 |
| items[].author_login | string | optional | present when include_author=true |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Preferred: GraphQL (fallback: REST)
- GraphQL: repository -> issues(after: $cursor, first: $limit, states, labels, filterBy) fields: id, number, title, state, createdAt, updatedAt, author { login }
- REST: GET /repos/{owner}/{repo}/issues?state=&labels=&creator=&assignee=&since=&sort=&direction=&per_page=&page

## Tool: get_issue
Purpose: Get a single issue with minimal fields.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | issue number |
| include_author | bool | no | false |  | adds author_login when true |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| item.id | string | always |  |
| item.number | int | always |  |
| item.title | string | always |  |
| item.body | string | optional |  |
| item.state | string | always |  |
| item.created_at | string | always | iso8601 |
| item.updated_at | string | always | iso8601 |
| item.author_login | string | optional | present when include_author=true |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Preferred: GraphQL (fallback: REST)
- GraphQL: repository { issue(number: $n) { id number title state createdAt updatedAt author { login } body } }
- REST: GET /repos/{owner}/{repo}/issues/{number}

## Tool: list_issue_comments_plain
Purpose: List issue comments (plain) with minimal fields.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | issue number |
| cursor | string | no |  |  | GraphQL cursor |
| limit | int | no | 30 |  | max 100 |
| include_author | bool | no | false |  | adds author_login when true |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| items[].id | string | always |  |
| items[].body | string | always |  |
| items[].author_login | string | optional | present when include_author=true |
| items[].created_at | string | always | iso8601 |
| items[].updated_at | string | always | iso8601 |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Preferred: GraphQL (fallback: REST)
- GraphQL: repository { issue(number: $n) { comments(first: $limit, after: $cursor) { nodes { id body createdAt updatedAt author { login } } } } }
- REST: GET /repos/{owner}/{repo}/issues/{number}/comments?per_page=&page

PULL REQUESTS

## Tool: list_pull_requests
Purpose: List pull requests with optional filters and minimal fields.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| state | enum | no |  | open, closed, all |  |
| base | string | no |  |  | base branch name |
| head | string | no |  |  | head ref (owner:branch allowed by REST) |
| cursor | string | no |  |  | GraphQL cursor |
| limit | int | no | 30 |  | max 100 |
| include_author | bool | no | false |  | adds author_login when true |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| items[].id | string | always |  |
| items[].number | int | always |  |
| items[].title | string | always |  |
| items[].state | string | always |  |
| items[].created_at | string | always | iso8601 |
| items[].updated_at | string | always | iso8601 |
| items[].author_login | string | optional | present when include_author=true |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Preferred: GraphQL (fallback: REST)
- GraphQL: repository { pullRequests(first: $limit, after: $cursor, states, baseRefName: $base, headRefName: $head, orderBy: {field: UPDATED_AT, direction: DESC}) { nodes { id number title state createdAt updatedAt author { login } } pageInfo { hasNextPage endCursor } } }
- REST: GET /repos/{owner}/{repo}/pulls?state=&head=&base=&per_page=&page

## Tool: get_pull_request
Purpose: Get a single pull request with minimal fields.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |
| include_author | bool | no | false |  | adds author_login when true |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| item.id | string | always |  |
| item.number | int | always |  |
| item.title | string | always |  |
| item.body | string | optional |  |
| item.state | string | always |  |
| item.is_draft | bool | always |  |
| item.created_at | string | always | iso8601 |
| item.updated_at | string | always | iso8601 |
| item.merged | bool | always |  |
| item.merged_at | string or null | always | iso8601 or null |
| item.author_login | string | optional | present when include_author=true |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Preferred: GraphQL (fallback: REST)
- GraphQL: repository { pullRequest(number: $n) { id number title state isDraft createdAt updatedAt merged mergedAt author { login } body } }
- REST: GET /repos/{owner}/{repo}/pulls/{number}

## Tool: get_pr_status_summary
Purpose: Summarize the latest commit status/checks for a PR.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |
| include_failing_contexts | bool | no | false |  | include names of failing contexts |
| limit_contexts | int | no | 10 |  | maximum contexts fetched via GraphQL |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| item.overall_state | string | always | SUCCESS, PENDING, or FAILURE |
| item.counts.success | int | always |  |
| item.counts.pending | int | always |  |
| item.counts.failure | int | always |  |
| item.failing_contexts | string[] | optional | present when include_failing_contexts=true |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Preferred: GraphQL (fallback: REST)
- GraphQL: repository { pullRequest(number: $n) { commits(last: 1) { nodes { commit { oid statusCheckRollup { state contexts(first: $limit) { nodes { ... on CheckRun { name conclusion } ... on StatusContext { context state } } } } } } } } }
- Notes: GraphQL returns a union of CheckRun and StatusContext. Map state/conclusion to SUCCESS/PENDING/FAILURE. failing_contexts use CheckRun.name or StatusContext.context where failure is indicated.
- REST fallback:
  - GET /repos/{owner}/{repo}/pulls/{number} -> head.sha
  - GET /repos/{owner}/{repo}/commits/{sha}/status
  - GET /repos/{owner}/{repo}/commits/{sha}/check-runs

## Tool: list_pr_comments_plain
Purpose: List PR issue comments (not code review comments).

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |
| cursor | string | no |  |  | GraphQL cursor |
| limit | int | no | 30 |  | max 100 |
| include_author | bool | no | false |  | adds author_login when true |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| items[].id | string | always |  |
| items[].body | string | always |  |
| items[].author_login | string | optional | present when include_author=true |
| items[].created_at | string | always | iso8601 |
| items[].updated_at | string | always | iso8601 |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Preferred: GraphQL (fallback: REST)
- GraphQL: repository { pullRequest(number: $n) { comments(first: $limit, after: $cursor) { nodes { id body createdAt updatedAt author { login } } } } }
- REST: GET /repos/{owner}/{repo}/issues/{number}/comments

## Tool: list_pr_review_comments_plain
Purpose: List PR code review comments (inline comments) with minimal fields.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |
| cursor | string | no |  |  | GraphQL cursor |
| limit | int | no | 30 |  | max 100 |
| include_author | bool | no | false |  | adds author_login when true |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| items[].id | string | always |  |
| items[].body | string | always |  |
| items[].author_login | string | optional | present when include_author=true |
| items[].created_at | string | always | iso8601 |
| items[].updated_at | string | always | iso8601 |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Preferred: GraphQL (fallback: REST)
- GraphQL: repository { pullRequest(number: $n) { reviewComments(first: $limit, after: $cursor) { nodes { id body createdAt updatedAt author { login } } } } }
- REST: GET /repos/{owner}/{repo}/pulls/{number}/comments

## Tool: list_pr_reviews_light
Purpose: List PR review summaries.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |
| cursor | string | no |  |  | GraphQL cursor |
| limit | int | no | 30 |  | max 100 |
| include_author | bool | no | false |  | adds author_login when true |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| items[].id | string | always |  |
| items[].state | string | always |  |
| items[].submitted_at | string or null | always | iso8601 or null |
| items[].author_login | string | optional | present when include_author=true |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Preferred: GraphQL (fallback: REST)
- GraphQL: repository { pullRequest(number: $n) { reviews(first: $limit, after: $cursor) { nodes { id state submittedAt author { login } } } } }
- REST: GET /repos/{owner}/{repo}/pulls/{number}/reviews

## Tool: list_pr_commits_light
Purpose: List commits of a PR with minimal fields.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |
| cursor | string | no |  |  | GraphQL cursor |
| limit | int | no | 30 |  | max 100 |
| include_author | bool | no | false |  | adds author_login when true |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| items[].sha | string | always | commit oid |
| items[].title | string | always | message headline |
| items[].authored_at | string | always | iso8601 |
| items[].author_login | string | optional | present when include_author=true |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Preferred: GraphQL (fallback: REST)
- GraphQL: repository { pullRequest(number: $n) { commits(first: $limit, after: $cursor) { nodes { commit { oid messageHeadline authoredDate author { user { login } } } } } } }
- REST: GET /repos/{owner}/{repo}/pulls/{number}/commits

## Tool: list_pr_files_light
Purpose: List files changed in a PR with optional patch inclusion.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |
| page | int | no |  |  | REST pagination |
| per_page | int | no |  |  | REST pagination |
| include_patch | bool | no | false |  | include file patch text when true |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| items[].filename | string | always |  |
| items[].status | string | always |  |
| items[].additions | int | always |  |
| items[].deletions | int | always |  |
| items[].changes | int | always |  |
| items[].sha | string | always |  |
| items[].patch | string | optional | present when include_patch=true |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Required: REST
- REST: GET /repos/{owner}/{repo}/pulls/{number}/files?per_page=&page (omit patch unless include_patch=true)

## Tool: get_pr_diff
Purpose: Get unified diff for a PR.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| diff | string | always | unified diff text |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Required: REST
- REST: GET /repos/{owner}/{repo}/pulls/{number} with Accept: application/vnd.github.v3.diff

## Tool: get_pr_patch
Purpose: Get patch for a PR.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| patch | string | always | patch text |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Required: REST
- REST: GET /repos/{owner}/{repo}/pulls/{number} with Accept: application/vnd.github.v3.patch

WORKFLOWS (GitHub Actions)

## Tool: list_workflows_light
Purpose: List workflows for a repository.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| page | int | no |  |  | REST pagination |
| per_page | int | no |  |  | REST pagination |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| items[].id | int | always |  |
| items[].name | string | always |  |
| items[].path | string | always |  |
| items[].state | string | always |  |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Required: REST
- REST: GET /repos/{owner}/{repo}/actions/workflows?per_page=&page

## Tool: list_workflow_runs_light
Purpose: List workflow runs for a workflow id with minimal fields.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| workflow_id | int or string | yes |  |  | id or filename |
| status | string | no |  |  | REST filter |
| branch | string | no |  |  |  |
| actor | string | no |  |  |  |
| event | string | no |  |  | optional filter |
| created | string | no |  |  | REST created filter syntax |
| head_sha | string | no |  |  |  |
| page | int | no |  |  | REST pagination |
| per_page | int | no |  |  | REST pagination |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| items[].id | int | always |  |
| items[].run_number | int | always |  |
| items[].event | string | always |  |
| items[].status | string | always |  |
| items[].conclusion | string or null | always |  |
| items[].head_sha | string | always |  |
| items[].created_at | string | always | iso8601 |
| items[].updated_at | string | always | iso8601 |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Required: REST
- REST: GET /repos/{owner}/{repo}/actions/workflows/{workflow_id}/runs?status=&branch=&actor=&event=&created=&head_sha=&per_page=&page

## Tool: get_workflow_run_light
Purpose: Get a single workflow run with minimal fields.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| run_id | int | yes |  |  |  |
| exclude_pull_requests | bool | no |  |  | when true, omits PRs in the run payload (server passes REST query) |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| item.id | int | always |  |
| item.run_number | int | always |  |
| item.event | string | always |  |
| item.status | string | always |  |
| item.conclusion | string or null | always |  |
| item.head_sha | string | always |  |
| item.created_at | string | always | iso8601 |
| item.updated_at | string | always | iso8601 |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Required: REST
- REST: GET /repos/{owner}/{repo}/actions/runs/{run_id}?exclude_pull_requests=true|false

## Tool: list_workflow_jobs_light
Purpose: List jobs for a workflow run.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| run_id | int | yes |  |  |  |
| filter | enum | no |  | latest, all | REST filter controls matrix duplication |
| page | int | no |  |  | REST pagination |
| per_page | int | no |  |  | REST pagination |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| items[].id | int | always |  |
| items[].name | string | always |  |
| items[].status | string | always |  |
| items[].conclusion | string or null | always |  |
| items[].started_at | string or null | always | iso8601 or null |
| items[].completed_at | string or null | always | iso8601 or null |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Required: REST
- REST: GET /repos/{owner}/{repo}/actions/runs/{run_id}/jobs?filter=&per_page=&page

## Tool: get_workflow_job_logs
Purpose: Fetch logs for a workflow job, optionally tailing locally.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| job_id | int | yes |  |  |  |
| tail_lines | int | no |  |  | server truncates to last N lines; not sent to GitHub API |
| include_timestamps | bool | no | false |  | server post-processes lines |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| logs | string | always | aggregated plain text |
| truncated | bool | always | true if server tailed the content |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Required: REST
- REST: GET /repos/{owner}/{repo}/actions/jobs/{job_id}/logs
- Notes: GitHub returns HTTP 302 to a temporary ZIP of logs. Server follows redirect, downloads ZIP, extracts text, and may tail locally. Tail and timestamp inclusion are server behaviors.

## Tool: rerun_workflow_run
Purpose: Rerun a workflow run.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| run_id | int | yes |  |  |  |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always |  |
| queued_run_id | int or null | always |  |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Required: REST
- REST: POST /repos/{owner}/{repo}/actions/runs/{run_id}/rerun

## Tool: rerun_workflow_run_failed
Purpose: Rerun only failed jobs of a workflow run.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| run_id | int | yes |  |  |  |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always |  |
| queued_run_id | int or null | always |  |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Required: REST
- REST: POST /repos/{owner}/{repo}/actions/runs/{run_id}/rerun-failed-jobs

## Tool: cancel_workflow_run
Purpose: Cancel a workflow run.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| run_id | int | yes |  |  |  |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always |  |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- Required: REST
- REST: POST /repos/{owner}/{repo}/actions/runs/{run_id}/cancel

Cross-cutting notes
- Pagination model
  - GraphQL tools: use cursor/limit; output meta.next_cursor from endCursor; has_more from pageInfo.hasNextPage.
  - REST tools: accept page/per_page; also output meta.next_cursor encoding next page (e.g., "page:2"). MCP clients should prefer cursor when present.
- Rate limit meta
  - Populate meta.rate from REST response headers (X-RateLimit-Remaining/Used/Reset) and GraphQL rateLimit where available.
- Error shape
  - On any failure, return only { error } and meta; omit items/item. Map 429/5xx as retriable=true; 4xx as retriable=false.
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

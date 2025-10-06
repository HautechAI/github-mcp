Title: Minimal MCP Server Interface for GitHub (LLM-friendly)

Design goals
- Lean payloads by default; expand via flags only.
- Prefer GraphQL for selective fields and cursor pagination; use REST where required (diff/patch, Actions).
- Consistent error meta and pagination across tools.

Tools Index
- Issues: [list_issues](#tool-list_issues), [get_issue](#tool-get_issue), [list_issue_comments_plain](#tool-list_issue_comments_plain)
- Pull Requests: [list_pull_requests](#tool-list_pull_requests), [search_pull_requests](#tool-search_pull_requests), [get_pull_request](#tool-get_pull_request), [get_pr_status_summary](#tool-get_pr_status_summary), [list_pr_comments_plain](#tool-list_pr_comments_plain), [list_pr_review_comments_plain](#tool-list_pr_review_comments_plain), [list_pr_review_threads_light](#tool-list_pr_review_threads_light), [resolve_pr_review_thread](#tool-resolve_pr_review_thread), [unresolve_pr_review_thread](#tool-unresolve_pr_review_thread), [list_pr_reviews_light](#tool-list_pr_reviews_light), [list_pr_commits_light](#tool-list_pr_commits_light), [list_pr_files_light](#tool-list_pr_files_light), [get_pr_diff](#tool-get_pr_diff), [get_pr_patch](#tool-get_pr_patch), [update_pull_request_branch](#tool-update_pull_request_branch), [pull_request_toggle_draft](#tool-pull_request_toggle_draft)
- Reviews (write): [create_or_submit_review](#tool-create_or_submit_review), [add_review_comment](#tool-add_review_comment)
- Triage helpers: [issues_add_labels](#tool-issues_add_labels), [issues_set_labels](#tool-issues_set_labels), [issues_remove_label](#tool-issues_remove_label), [pulls_request_reviewers](#tool-pulls_request_reviewers), [pulls_remove_requested_reviewers](#tool-pulls_remove_requested_reviewers), [issues_add_assignees](#tool-issues_add_assignees), [issues_remove_assignees](#tool-issues_remove_assignees)
- Workflows: [list_workflows_light](#tool-list_workflows_light), [list_workflow_runs_light](#tool-list_workflow_runs_light), [get_workflow_run_light](#tool-get_workflow_run_light), [list_workflow_jobs_light](#tool-list_workflow_jobs_light), [get_workflow_job_logs](#tool-get_workflow_job_logs), [rerun_workflow_run](#tool-rerun_workflow_run), [rerun_workflow_run_failed](#tool-rerun_workflow_run_failed), [cancel_workflow_run](#tool-cancel_workflow_run), [actions_list_run_artifacts](#tool-actions_list_run_artifacts), [actions_download_artifact](#tool-actions_download_artifact)

Shared conventions
- Pagination (inputs): cursor (string, optional), limit (int, default 30, max 100). For REST tools, server maps cursor to page/per_page.
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
- GraphQL only
- Query

```graphql
query ListIssues(
  $owner: String!, $repo: String!,
  $first: Int = 30, $after: String,
  $states: [IssueState!], $filterBy: IssueFilters
) {
  repository(owner: $owner, name: $repo) {
    issues(first: $first, after: $after, states: $states, filterBy: $filterBy) {
      nodes { id number title state createdAt updatedAt author { login } }
      pageInfo { hasNextPage endCursor }
    }
  }
}
```

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
- GraphQL only
- Query

```graphql
query GetIssue($owner: String!, $repo: String!, $number: Int!) {
  repository(owner: $owner, name: $repo) {
    issue(number: $number) {
      id number title body state createdAt updatedAt author { login }
    }
  }
}
```

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
- GraphQL only
- Query

```graphql
query ListIssueComments($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
  repository(owner: $owner, name: $repo) {
    issue(number: $number) {
      comments(first: $first, after: $after) {
        nodes { id body createdAt updatedAt author { login } }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
```

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
- GraphQL only
- Query

```graphql
query ListPullRequests(
  $owner: String!, $repo: String!,
  $first: Int = 30, $after: String,
  $states: [PullRequestState!], $base: String, $head: String
) {
  repository(owner: $owner, name: $repo) {
    pullRequests(
      first: $first, after: $after,
      states: $states, baseRefName: $base, headRefName: $head,
      orderBy: { field: UPDATED_AT, direction: DESC }
    ) {
      nodes { id number title state createdAt updatedAt author { login } }
      pageInfo { hasNextPage endCursor }
    }
  }
}
```

## Tool: search_pull_requests
Purpose: Search pull requests via GitHub search with minimal fields.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  | repository owner (for scoping) |
| repo | string | yes |  |  | repository name (for scoping) |
| q | string | yes |  |  | search qualifiers, server prefixes with repo:owner/repo is:pr |
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
| items[].is_draft | bool | always |  |
| items[].created_at | string | always | iso8601 |
| items[].updated_at | string | always | iso8601 |
| items[].author_login | string | optional | present when include_author=true |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- GraphQL preferred (search API)
- Query

```graphql
query SearchPullRequests($q: String!, $first: Int = 30, $after: String) {
  search(type: ISSUE, query: $q, first: $first, after: $after) {
    nodes {
      ... on PullRequest {
        id number title state isDraft createdAt updatedAt author { login }
      }
    }
    pageInfo { hasNextPage endCursor }
  }
  rateLimit { remaining used resetAt }
}
```

- Notes: Server constructs `$q` as `repo:OWNER/REPO is:pr <user q>`. If GraphQL search is unavailable, server MAY provide a REST fallback `search_pull_requests_rest` using `GET /search/issues` (not recommended due to differing shapes/pagination).

## Tool: get_pull_request
Purpose: Get a single pull request with minimal fields.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |
| include_author | bool | no | false |  | adds author_login when true |
| include_head_sha | bool | no | false |  | adds head_sha (latest commit OID) when true |
| include_merge_readiness | bool | no | false |  | adds merge readiness fields when true |

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
| item.head_sha | string | optional | present when include_head_sha=true |
| item.merge_readiness.review_decision | string | optional | APPROVED, CHANGES_REQUESTED, REVIEW_REQUIRED; present when include_merge_readiness=true |
| item.merge_readiness.mergeable | string | optional | MERGEABLE, CONFLICTING, UNKNOWN; present when include_merge_readiness=true |
| item.merge_readiness.merge_state_status | string | optional | CLEAN, BEHIND, BLOCKED, DRAFT, UNKNOWN, etc.; present when include_merge_readiness=true |
| item.merge_readiness.merge_queue.is_in_queue | bool | optional | present when include_merge_readiness=true and data available |
| item.merge_readiness.merge_queue.position | int or null | optional | present when available |
| item.merge_readiness.auto_merge.enabled | bool | optional | true when auto-merge is enabled |
| item.merge_readiness.auto_merge.merge_method | string or null | optional | SQUASH, MERGE, REBASE when enabled |
| item.merge_readiness.auto_merge.enabled_by_login | string or null | optional | who enabled auto-merge |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- GraphQL only
- Query

```graphql
query GetPullRequest($owner: String!, $repo: String!, $number: Int!) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      id number title body state isDraft merged mergedAt createdAt updatedAt author { login }
      # include when include_head_sha=true
      headRefOid
      # include when include_merge_readiness=true
      reviewDecision
      mergeable
      mergeStateStatus
      isInMergeQueue
      mergeQueueEntry { position }
      autoMergeRequest { mergeMethod enabledAt enabledBy { login } }
    }
  }
  rateLimit { remaining used resetAt }
}
```

Notes
- Only include headRefOid and merge readiness fields when the respective flags are true.
- merge_queue fields are returned when the repository has merge queue enabled; fields may be null otherwise.
- auto_merge fields are returned via `autoMergeRequest`; when null, treat as disabled.

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
- GraphQL only
- Query (demonstrates union handling via inline fragments)

```graphql
query GetPrStatusSummary($owner: String!, $repo: String!, $number: Int!, $limit_contexts: Int = 10) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      commits(last: 1) {
        nodes {
          commit {
            oid
            statusCheckRollup {
              state
              contexts(first: $limit_contexts) {
                nodes {
                  __typename
                  ... on CheckRun { name conclusion }
                  ... on StatusContext { context state }
                }
              }
            }
          }
        }
      }
    }
  }
}
```

- Notes: GraphQL returns a union of CheckRun and StatusContext. Map state/conclusion to SUCCESS/PENDING/FAILURE. failing_contexts derive from CheckRun.name or StatusContext.context where a failure is indicated.

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
- GraphQL only
- Query

```graphql
query ListPrComments($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      comments(first: $first, after: $after) {
        nodes { id body createdAt updatedAt author { login } }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
```

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
| include_location | bool | no | false |  | when true, includes file/line mapping |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| items[].id | string | always |  |
| items[].body | string | always |  |
| items[].author_login | string | optional | present when include_author=true |
| items[].created_at | string | always | iso8601 |
| items[].updated_at | string | always | iso8601 |
| items[].path | string | optional | present when include_location=true |
| items[].line | int | optional | present when include_location=true |
| items[].start_line | int | optional | present when include_location=true |
| items[].side | string | optional | LEFT or RIGHT; present when include_location=true |
| items[].start_side | string | optional | LEFT or RIGHT; present when include_location=true |
| items[].original_line | int | optional | present when include_location=true |
| items[].original_start_line | int | optional | present when include_location=true |
| items[].diff_hunk | string | optional | present when include_location=true |
| items[].commit_sha | string | optional | present when include_location=true |
| items[].original_commit_sha | string | optional | present when include_location=true |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- GraphQL only
- Query

```graphql
query ListPrReviewComments(
  $owner: String!, $repo: String!, $number: Int!,
  $first: Int = 30, $after: String
) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      reviewComments(first: $first, after: $after) {
        nodes {
          id
          body
          createdAt
          updatedAt
          author { login }
          # Location from PullRequestReviewComment
          path
          diffHunk
          line
          startLine
          side
          startSide
          originalLine
          originalStartLine
          commit { oid }
          originalCommit { oid }
          # Thread location from PullRequestReviewThread (current PR mapping)
          pullRequestReviewThread {
            path
            line
            startLine
            side
            startSide
          }
        }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
```

- Notes: Location fields are populated from PullRequestReviewComment and PullRequestReviewThread location fields. Thread-level grouping could be provided by a future `list_pr_review_threads_light` if needed.
- Notes: Location fields are populated from PullRequestReviewComment and PullRequestReviewThread location fields. For thread-level grouping and resolution state, use [list_pr_review_threads_light](#tool-list_pr_review_threads_light).

## Tool: list_pr_review_threads_light
Purpose: List PR review threads (grouped inline discussions) with minimal fields.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |
| cursor | string | no |  |  | GraphQL cursor |
| limit | int | no | 30 |  | max 100 |
| include_author | bool | no | false |  | adds resolved_by_login when true |
| include_location | bool | no | false |  | when true, includes file/line mapping |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| items[].id | string | always | thread node id |
| items[].is_resolved | bool | always |  |
| items[].is_outdated | bool | always |  |
| items[].comments_count | int | always | total comments in thread |
| items[].resolved_by_login | string | optional | present when include_author=true and thread is resolved |
| items[].path | string | optional | present when include_location=true |
| items[].line | int | optional | present when include_location=true |
| items[].start_line | int | optional | present when include_location=true |
| items[].side | string | optional | LEFT or RIGHT; present when include_location=true |
| items[].start_side | string | optional | LEFT or RIGHT; present when include_location=true |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- GraphQL only
- Query

```graphql
query ListPrReviewThreads(
  $owner: String!, $repo: String!, $number: Int!,
  $first: Int = 30, $after: String
) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      reviewThreads(first: $first, after: $after) {
        nodes {
          id
          isResolved
          isOutdated
          comments { totalCount }
          resolvedBy { login }
          path
          line
          startLine
          side
          startSide
        }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
```

## Tool: resolve_pr_review_thread
Purpose: Resolve a single review thread on a PR.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| thread_id | string | yes |  |  | GraphQL node id of the thread |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always | true when mutation succeeds |
| thread_id | string | always | id of the thread mutated |
| is_resolved | bool | always | resolved state after mutation |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- GraphQL only
- Mutation

```graphql
mutation ResolvePrReviewThread($thread_id: ID!) {
  resolveReviewThread(input: { threadId: $thread_id }) {
    thread { id isResolved }
  }
  rateLimit { remaining used resetAt }
}
```

## Tool: unresolve_pr_review_thread
Purpose: Unresolve a single review thread on a PR.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| thread_id | string | yes |  |  | GraphQL node id of the thread |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always | true when mutation succeeds |
| thread_id | string | always | id of the thread mutated |
| is_resolved | bool | always | resolved state after mutation |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- GraphQL only
- Mutation

```graphql
mutation UnresolvePrReviewThread($thread_id: ID!) {
  unresolveReviewThread(input: { threadId: $thread_id }) {
    thread { id isResolved }
  }
  rateLimit { remaining used resetAt }
}
```

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
- GraphQL only
- Query

```graphql
query ListPrReviews($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      reviews(first: $first, after: $after) {
        nodes { id state submittedAt author { login } }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
```

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
- GraphQL only
- Query

```graphql
query ListPrCommits($owner: String!, $repo: String!, $number: Int!, $first: Int = 30, $after: String) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      commits(first: $first, after: $after) {
        nodes {
          commit { oid messageHeadline authoredDate author { user { login } } }
        }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
```

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
- REST only
- Method: GET
- Path: /repos/{owner}/{repo}/pulls/{number}/files?per_page=&page
- Accept: application/vnd.github+json
- Notes: Omit `patch` unless `include_patch=true`. Include header `X-GitHub-Api-Version: 2022-11-28`.

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
- REST only
- Method: GET
- Path: /repos/{owner}/{repo}/pulls/{number}
- Accept: application/vnd.github.v3.diff
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

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
- REST only
- Method: GET
- Path: /repos/{owner}/{repo}/pulls/{number}
- Accept: application/vnd.github.v3.patch
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

## Tool: update_pull_request_branch
Purpose: Update a pull request branch from its base branch.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |
| expected_head_sha | string | no |  |  | if provided, update only when the PR head matches this SHA |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always | true when an update was queued or completed |
| update_queued | bool | always | true when GitHub accepted the update task |
| skipped_due_to_head_sha_mismatch | bool | always | true when expected_head_sha did not match |
| message | string | optional | server-provided message from GitHub |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- REST only
- Method: PUT
- Path: /repos/{owner}/{repo}/pulls/{number}/update-branch
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`. The operation is asynchronous; GitHub typically responds 202 with a message. Server enforces `expected_head_sha` precondition when provided.

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
- REST only
- Method: GET
- Path: /repos/{owner}/{repo}/actions/workflows?per_page=&page
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

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
- REST only
- Method: GET
- Path: /repos/{owner}/{repo}/actions/workflows/{workflow_id}/runs?status=&branch=&actor=&event=&created=&head_sha=&per_page=&page
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

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
- REST only
- Method: GET
- Path: /repos/{owner}/{repo}/actions/runs/{run_id}?exclude_pull_requests=true|false
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

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
- REST only
- Method: GET
- Path: /repos/{owner}/{repo}/actions/runs/{run_id}/jobs?filter=&per_page=&page
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

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
- REST only
- Method: GET
- Path: /repos/{owner}/{repo}/actions/jobs/{job_id}/logs
- Accept: application/vnd.github+json
- Notes: GitHub returns HTTP 302 to a temporary ZIP of logs. Server follows redirect, downloads ZIP, extracts text, and may tail locally. Tail and timestamp inclusion are server behaviors. Include header `X-GitHub-Api-Version: 2022-11-28`.

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
- REST only
- Method: POST
- Path: /repos/{owner}/{repo}/actions/runs/{run_id}/rerun
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

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
- REST only
- Method: POST
- Path: /repos/{owner}/{repo}/actions/runs/{run_id}/rerun-failed-jobs
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

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
- REST only
- Method: POST
- Path: /repos/{owner}/{repo}/actions/runs/{run_id}/cancel
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

## Tool: actions_list_run_artifacts
Purpose: List artifacts produced by a workflow run.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| run_id | int | yes |  |  |  |
| page | int | no |  |  | REST pagination |
| per_page | int | no |  |  | REST pagination |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| items[].id | int | always | artifact id |
| items[].name | string | always |  |
| items[].size_in_bytes | int | always |  |
| items[].expired | bool | always |  |
| items[].created_at | string | always | iso8601 |
| items[].expires_at | string or null | always | iso8601 or null |
| meta | object | always | next_cursor, has_more, rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- REST only
- Method: GET
- Path: /repos/{owner}/{repo}/actions/runs/{run_id}/artifacts?per_page=&page
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

## Tool: actions_download_artifact
Purpose: Download a workflow artifact as a ZIP archive.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| artifact_id | int | yes |  |  | id from actions_list_run_artifacts |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| filename | string | always | suggested filename for the ZIP |
| zip_bytes_base64 | string | always | base64-encoded ZIP content |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- REST only
- Method: GET
- Path: /repos/{owner}/{repo}/actions/artifacts/{artifact_id}/zip
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`. GitHub responds with a redirect or raw ZIP; server returns base64-encoded bytes to clients.

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
  - GraphQL is used for list/get of issues/PRs/comments/reviews/commits due to selective fields and cursor pagination.
  - REST is used for: diffs/patches (media types), PR files (patch access, stable REST pagination), and all Actions workflow operations and logs.
- Comment payload discipline
  - "Plain" variants only include: id, body, author_login (optional), created_at, updated_at. No reactions, URLs, or user objects.
- PR status summary
  - Favor GraphQL statusCheckRollup for holistic state. Only expose minimal counts and optional failing contexts.
- Diffs
  - Provide unified diff or patch as raw strings via REST Accept headers. Never embed file blobs or binary content.

REVIEWS (Write)

## Tool: create_or_submit_review
Purpose: Create and optionally submit a PR review (approve, request changes, or comment).

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |
| event | enum | yes |  | APPROVE, REQUEST_CHANGES, COMMENT | review action |
| body | string | no |  |  | optional summary body |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always |  |
| review_id | int | always | REST review id |
| submitted | bool | always | true when event resulted in a submitted review |
| state | string | always | review state after submission |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- REST only
- Method: POST
- Path: /repos/{owner}/{repo}/pulls/{number}/reviews
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

## Tool: add_review_comment
Purpose: Add a single inline code review comment to a PR (diff view).

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |
| body | string | yes |  |  | comment text |
| commit_id | string | yes |  |  | SHA identifying the commit to comment on |
| path | string | yes |  |  | file path in the repo |
| side | enum | no | RIGHT | LEFT, RIGHT | diff side |
| line | int | yes |  |  | line number in the diff to comment on |
| start_line | int | no |  |  | for multi-line comments |
| start_side | enum | no |  | LEFT, RIGHT | side for start_line |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always |  |
| comment_id | int | always | REST review comment id |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- REST only
- Method: POST
- Path: /repos/{owner}/{repo}/pulls/{number}/comments
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`. Use `commit_id` matching the PR head to avoid outdated mapping.

TRIAGE HELPERS

## Tool: issues_add_labels
Purpose: Add labels to an issue or PR (issues API).

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | issue or PR number |
| labels | string[] | yes |  |  | label names to add |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always |  |
| added | string[] | always | labels that were added |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- REST only
- Method: POST
- Path: /repos/{owner}/{repo}/issues/{number}/labels
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

## Tool: issues_set_labels
Purpose: Replace all labels on an issue or PR.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | issue or PR number |
| labels | string[] | yes |  |  | new full set of labels |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always |  |
| labels | string[] | always | resulting labels on the item |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- REST only
- Method: PUT
- Path: /repos/{owner}/{repo}/issues/{number}/labels
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

## Tool: issues_remove_label
Purpose: Remove a single label from an issue or PR.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | issue or PR number |
| name | string | yes |  |  | label name to remove |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always |  |
| removed | string | always | the label that was removed |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- REST only
- Method: DELETE
- Path: /repos/{owner}/{repo}/issues/{number}/labels/{name}
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

## Tool: pulls_request_reviewers
Purpose: Request reviewers on a pull request.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |
| reviewers | string[] | no |  |  | GitHub usernames |
| team_reviewers | string[] | no |  |  | org/team slugs |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always |  |
| requested | object | always | { reviewers: string[], team_reviewers: string[] } |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- REST only
- Method: POST
- Path: /repos/{owner}/{repo}/pulls/{number}/requested_reviewers
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

## Tool: pulls_remove_requested_reviewers
Purpose: Remove requested reviewers from a pull request.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | PR number |
| reviewers | string[] | no |  |  | GitHub usernames |
| team_reviewers | string[] | no |  |  | org/team slugs |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always |  |
| removed | object | always | { reviewers: string[], team_reviewers: string[] } |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- REST only
- Method: DELETE
- Path: /repos/{owner}/{repo}/pulls/{number}/requested_reviewers
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

## Tool: issues_add_assignees
Purpose: Add assignees to an issue or PR.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | issue or PR number |
| assignees | string[] | yes |  |  | GitHub usernames |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always |  |
| added | string[] | always | usernames added |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- REST only
- Method: POST
- Path: /repos/{owner}/{repo}/issues/{number}/assignees
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

## Tool: issues_remove_assignees
Purpose: Remove assignees from an issue or PR.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| owner | string | yes |  |  |  |
| repo | string | yes |  |  |  |
| number | int | yes |  |  | issue or PR number |
| assignees | string[] | yes |  |  | GitHub usernames to remove |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always |  |
| removed | string[] | always | usernames removed |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- REST only
- Method: DELETE
- Path: /repos/{owner}/{repo}/issues/{number}/assignees
- Accept: application/vnd.github+json
- Notes: Include header `X-GitHub-Api-Version: 2022-11-28`.

## Tool: pull_request_toggle_draft
Purpose: Convert a pull request to draft or mark ready for review.

Inputs

| name | type | required | default | allowed | notes |
| --- | --- | --- | --- | --- | --- |
| pull_request_id | string | yes |  |  | GraphQL node id of the PR |
| action | enum | yes |  | to_draft, ready_for_review |  |

Outputs

| field | type | presence | notes |
| --- | --- | --- | --- |
| ok | bool | always |  |
| is_draft | bool | always | resulting draft state |
| meta | object | always | rate.remaining, rate.used, rate.reset_at? |
| error | object | optional | see Error shape |

API
- GraphQL only
- Mutations

```graphql
mutation ConvertToDraft($pr_id: ID!) {
  convertPullRequestToDraft(input: { pullRequestId: $pr_id }) {
    pullRequest { id isDraft }
  }
  rateLimit { remaining used resetAt }
}

mutation MarkReadyForReview($pr_id: ID!) {
  markPullRequestReadyForReview(input: { pullRequestId: $pr_id }) {
    pullRequest { id isDraft }
  }
  rateLimit { remaining used resetAt }
}
```

Notes
- Server chooses the appropriate mutation based on `action`.

# Comparison: newgithub_* vs github_* tool outputs

Scope
- Repo: HautechAI/github-mcp
- Targets: Issue #39 and PR #40
- Sizes shown as chars=…; keys=… (keys=N/A for diff/patch). Method names shown without MCP prefixes.
- Non-destructive calls only. Destructive/stateful tools marked N/A.

| Our method | Our response size | Official method | Official response size | Note |
|---|---|---|---|---|
| get_issue | chars=ERR; keys=ERR | get_issue | chars=6554; keys=68 | newgithub not available; ours flattens issue fields |
| list_issue_comments | chars=ERR; keys=ERR | get_issue_comments | chars=8517; keys=156 | newgithub not available; ours returns minimal comment items |
| list_issues | chars=ERR; keys=ERR | list_issues | chars=ERR; keys=ERR | official expects OPEN/CLOSED; earlier schema error on state=all; ours has has_more,next_cursor=null |
| get_pull_request | chars=ERR; keys=ERR | get_pull_request | chars=16847; keys=318 | newgithub not available; ours omits heavy nested user/repo fields |
| list_pr_review_comments | chars=ERR; keys=ERR | get_pull_request_review_comments | chars=2; keys=0 | newgithub not available; both empty for PR #40 |
| list_pr_reviews | chars=ERR; keys=ERR | get_pull_request_reviews | chars=21212; keys=280 | newgithub not available; ours emits light review events |
| list_pr_files | chars=ERR; keys=ERR | get_pull_request_files | chars=14050; keys=40 | newgithub not available; ours returns filename/status/additions/deletions |
| list_pr_commits | chars=ERR; keys=ERR | N/A | N/A | official lacks PR-commits tool; closest is list_commits on head SHA |
| get_pr_diff | chars=ERR; keys=N/A | get_pull_request_diff | chars=6482; keys=N/A | diff is text; newgithub not available in this run |
| get_pr_patch | chars=ERR; keys=N/A | N/A | N/A | official patch endpoint not exposed; newgithub returned error in this run |
| list_pull_requests | chars=ERR; keys=ERR | list_pull_requests | chars=ERR; keys=ERR | output too long / truncated on both; our variant filters/normalizes aggressively |
| resolve_pr_review_thread | N/A | resolve_pr_review_thread | N/A | destructive; skipped |
| unresolve_pr_review_thread | N/A | unresolve_pr_review_thread | N/A | destructive; skipped |

Notes and findings
- Multiple newgithub_* list endpoints intermittently failed with generic agent error; re-run yielded the same on list_issues, list_pull_requests, list_pr_review_comments.
- Official github_list_issues earlier failed with schema mismatch when passing state=all; works with state=OPEN or CLOSED only.
- Cursor behavior: newgithub list tools expose has_more and next_cursor; when no next page, next_cursor returned null (Issue #48 proposes omitting the field entirely).
- CI artifacts from PR #40 confirm NDJSON stdio only (no Content-Length headers); initialize result includes protocolVersion and capabilities.tools.
- For large collections, official responses include deeply nested user/repo objects and pagination metadata; newgithub variants flatten and omit unused fields, leading to materially smaller payloads.

Provenance
- Paired calls executed against Issue #39 and PR #40. Where tools errored or returned empty sets, sizes are marked ERR or 2 chars (empty array), respectively.
- Tracking: Issue #49. See also Issue #48 regarding next_cursor omission.

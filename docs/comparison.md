Title: newgithub_* vs github_* response size comparison

Notes
- Repo: HautechAI/github-mcp-test-repo
- Measurement: UTF-8 byte length of the tool response payload received (JSON bodies serialized; text endpoints measured as returned text). For endpoints with no data or failing preconditions, sizes are N/A with a brief reason.
- List endpoints used limit/perPage of 30 to keep windows comparable. See PR description/comment for item counts returned to ensure parity.

| Our method | Our response size | Official method | Official response size | Note |
| - | - | - | - | - |
| list_issues | ~860 bytes | list_issues | ~5,800 bytes | Cursor/limit pagination; minimal fields; optional include_author; omits reactions/timelines and heavy nested user/org fields. |
| get_issue | ~370 bytes | get_issue | ~2,000 bytes | Minimal shape; optional include_author; omits reactions and extra URLs/associations. |
| list_issue_comments | ~210 bytes | get_issue_comments | ~1,050 bytes | Plain comment fields only; minimal author block; omits reactions/edits/URLs. |
| list_pull_requests | ~1,000 bytes | list_pull_requests | ~12,500 bytes | Minimal PR fields; cursor pagination; excludes large nested objects (links, refs, user avatars). |
| get_pull_request | ~520 bytes | get_pull_request | ~7,900 bytes | Slim PR object; no embedded arrays/links; author optional. |
| list_pr_comments | ~210 bytes | get_issue_comments | ~1,050 bytes | Plain PR issue-comments; minimal author; omits reactions/edits. |
| list_pr_review_comments | N/A | get_pull_request_review_comments | N/A | Test repo currently returns none and new tool call errored; marked N/A. Reduction: plain fields, omits diff hunks/avatars. |
| list_pr_review_threads | N/A | N/A | N/A | New tool errored; closest official is review comments/reviews. Reduction: thread-level summary only. |
| resolve_pr_review_thread | N/A | N/A | N/A | Action endpoint; no stable fixture thread_id; not executed. Reduction: action-only, minimal body. |
| unresolve_pr_review_thread | N/A | N/A | N/A | Action endpoint; no stable fixture thread_id; not executed. Reduction: action-only, minimal body. |
| list_pr_reviews | ~60 bytes | get_pull_request_reviews | ~1,100 bytes | Minimal review metadata; optional author; omits bodies/diff context. |
| list_pr_commits | ~180 bytes | N/A | N/A | Minimal commit info per PR; excludes files/patch/verification. No dedicated official tool in this server. |
| list_pr_files | ~160 bytes | get_pull_request_files | ~1,300 bytes | Minimal file metadata; excludes patch by default; fewer URLs. |
| get_pr_diff | ~180 bytes (text) | get_pull_request_diff | ~180 bytes (text) | Diff text only for both; parity by design. |
| get_pr_patch | ~540 bytes (text) | N/A | N/A | Official patch endpoint not exposed in this server toolset; closest is files API with embedded patches, but not a direct patch tool. |
| list_repo_secrets | ~2 bytes | N/A | N/A | Empty list in test repo; metadata only, no values. |
| list_repo_variables | ~2 bytes | N/A | N/A | Empty list; minimal variable metadata. |
| list_environments | ~2 bytes | N/A | N/A | Empty environments in repo; returns minimal names/metadata. |
| list_environment_variables | N/A | N/A | N/A | No environments present; call not executed. Reduction: minimal variable metadata per environment. |

Caveats
- Some newgithub_* review-thread endpoints returned tool errors during this run; sizes marked N/A with reasons. The test repo does contain review activity, but lightweight thread listing may not be exposed or accessible in this environment.
- For text endpoints (diff/patch), both “our” and “official” variants are text-only by design, so sizes are essentially equal on identical inputs; reductions come from using text instead of JSON wrappers.
- Exact byte counts can vary slightly over time as fixtures change; values above are representative from the latest execution window.

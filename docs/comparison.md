Our vs Official GitHub API responses (against HautechAI/github-mcp-test-repo)

Notes
- All calls used the parameters from issue #66. For official endpoints we captured the raw tool responses, minified to JSON, and measured UTF-8 byte lengths.
- Our newgithub_* calls could not be executed in this environment due to a missing GITHUB_TOKEN/GH_TOKEN, so “Our response size” is N/A. The Notes column summarizes expected reduction strategies of the newgithub_* variants.
- When an official endpoint returned no items, its size still reflects the empty payload (e.g., [] -> 2 bytes).

| Our method | Our response size | Official method | Official response size | Note |
| --- | --- | --- | --- | --- |
| list_issues | N/A | list_issues | 3704 | newgithub trims *_url fields, returns compact user (login only), reduces label/user payloads, and simplifies pagination cursors. |
| get_issue | N/A | get_issue | 2429 | newgithub omits many link fields and returns compact user/label objects. |
| list_issue_comments_plain | N/A | get_issue_comments | 1622 | “plain” reduces comment fields (no nested user details beyond login), drops reaction URLs and misc metadata. |
| list_pull_requests | N/A | list_pull_requests | 9498 | newgithub removes many repo/link subobjects and returns compact user; fewer nested *_url fields. |
| get_pull_request | N/A | get_pull_request | 3443 | newgithub keeps core PR fields, drops most *_url/link fields and repo/user subtrees, compacts booleans/strings only. |
| list_pr_comments_plain | N/A | get_issue_comments | 1622 | maps to PR issue comments; “plain” strips reaction URLs and rich user data. |
| list_pr_review_comments_plain | N/A | get_pull_request_review_comments | 2 | Empty for PR #9; “plain” would keep only essential fields per comment. |
| list_pr_review_threads_light | N/A | get_pull_request_review_comments | 2 | Official has no thread view; newgithub groups comments into lightweight threads; empty for PR #9. |
| resolve_pr_review_thread | N/A | N/A | N/A | Action endpoint; newgithub returns concise status only (no official counterpart). |
| unresolve_pr_review_thread | N/A | N/A | N/A | Action endpoint; newgithub returns concise status only (no official counterpart). |
| list_pr_reviews_light | N/A | get_pull_request_reviews | 2 | Empty for PR #9; newgithub returns minimal per-review fields. |
| list_pr_commits_light | N/A | N/A | N/A | No official counterpart; newgithub returns commit SHAs/authors minimally. |
| list_pr_files_light | N/A | get_pull_request_files | 582 | newgithub removes blob/raw/contents URLs and leaves filename, status, and minimal patch stats. |
| get_pr_diff | N/A | get_pull_request_diff | 147 | Official is raw diff text; newgithub returns the same content without extra wrappers. |
| get_pr_patch | N/A | N/A | N/A | No official counterpart; newgithub returns raw patch text. |
| list_repo_secrets_light | N/A | N/A | N/A | No official counterpart; newgithub returns only names/metadata. |
| list_repo_variables_light | N/A | N/A | N/A | No official counterpart; newgithub returns only names/values (where permitted) or metadata. |
| list_environments_light | N/A | N/A | N/A | No official counterpart; newgithub returns minimal environment summaries. |
| list_environment_variables_light | N/A | N/A | N/A | No official counterpart; newgithub returns minimal variable metadata for the environment. |

References
- Issue: #66
- Repo under test: HautechAI/github-mcp-test-repo

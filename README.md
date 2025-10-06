# github-mcp


GitHub MCP server (Rust, stdio JSON-RPC).

Quickstart
- Prereqs: Rust 1.90.0+, cargo; set `GITHUB_TOKEN` or `GH_TOKEN`.
- Build: `cargo build`
- Run (stdio JSON-RPC):
  - Initialize
    - `echo '{"jsonrpc":"2.0","method":"initialize","id":1}' | cargo run -- --log-level warn`
  - Tools list
    - `echo '{"jsonrpc":"2.0","method":"tools/list","id":2}' | cargo run -- --log-level warn`
  - Call ping
    - `echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"ping","arguments":{"message":"hello"}},"id":3}' | cargo run -- --log-level warn`

Configuration
- Token: `GITHUB_TOKEN` (fallback `GH_TOKEN`).
- Endpoints: `GITHUB_API_URL` (default https://api.github.com), `GITHUB_GRAPHQL_URL` (default https://api.github.com/graphql).
- API version header: `GITHUB_API_VERSION` (default 2022-11-28).
- HTTP timeout: `GITHUB_HTTP_TIMEOUT_SECS` (default 30).
- User-Agent: `github-mcp/<version>` (set automatically).

Usage examples
- Issues → list_issues
  - `echo '{"jsonrpc":"2.0","method":"tools/call","id":11,"params":{"name":"list_issues","arguments":{"owner":"octo","repo":"hello","limit":10,"include_author":true}}}' | cargo run -- --log-level warn`

- Pull Requests → get_pull_request
  - `echo '{"jsonrpc":"2.0","method":"tools/call","id":12,"params":{"name":"get_pull_request","arguments":{"owner":"octo","repo":"hello","number":1}}}' | cargo run -- --log-level warn`

- Pull Requests → list_pr_files_light (include_patch)
  - `echo '{"jsonrpc":"2.0","method":"tools/call","id":13,"params":{"name":"list_pr_files_light","arguments":{"owner":"octo","repo":"hello","number":1,"per_page":50,"page":1,"include_patch":true}}}' | cargo run -- --log-level warn`

- Actions → get_workflow_job_logs (tail + timestamps)
  - `echo '{"jsonrpc":"2.0","method":"tools/call","id":14,"params":{"name":"get_workflow_job_logs","arguments":{"owner":"octo","repo":"hello","job_id":123456,"tail_lines":200,"include_timestamps":true}}}' | cargo run -- --log-level warn`

Pagination
- GraphQL: use `cursor` and `limit` (max 100). Responses include `meta.next_cursor` and `meta.has_more`.
- REST: server uses Link headers to detect `has_more` and returns an opaque `next_cursor` encoding `{page, per_page}` (base64 URL-safe). Clients can pass `cursor` back; `page`/`per_page` are also accepted on some tools.

Error model
- On failure, responses include `error` with fields: `code` (e.g., `bad_request`, `unauthorized`, `forbidden`, `not_found`, `conflict`, `rate_limited`, `upstream_error`, `server_error`) and `retriable` (true for 429/5xx).
- `meta.rate` is populated from REST headers and GraphQL `rateLimit` when present.

Notes
- See docs/methods.md for authoritative tool inputs/outputs; this server implements those shapes.
- TODO:
  - GraphQL `rateLimit` is included where feasible without complicating queries; some queries may still omit it.
  - REST pagination relies on Link headers; when GitHub omits Link for small result sets, `has_more` may be false with no `next_cursor`.

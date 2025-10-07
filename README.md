# github-mcp

GitHub MCP server (Rust, stdio JSON-RPC).

Quickstart:
- Prereqs: Rust 1.90.0+, cargo; set `GITHUB_TOKEN` or `GH_TOKEN`.
- Build: `cargo build`
- Run (stdio JSON-RPC, NDJSON framing):
  - Initialize
    - `echo '{"jsonrpc":"2.0","method":"initialize","id":1}' | cargo run -- --log-level warn`
  - Tools list
    - `echo '{"jsonrpc":"2.0","method":"tools/list","id":2}' | cargo run -- --log-level warn`
  - Call ping
    - `echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"ping","arguments":{"message":"hello"}},"id":3}' | cargo run -- --log-level warn`
      - Note: responses now follow MCP tool result envelope with `content` and `structuredContent`.

Inspector CLI
- You can validate end-to-end using the MCP Inspector:
  - `npx @modelcontextprotocol/inspector-cli --cli ./target/release/github-mcp --method initialize`
  - `npx @modelcontextprotocol/inspector-cli --cli ./target/release/github-mcp --method tools/list`
  - Expect `result.protocolVersion` and MCP tool result envelopes with `content` and `structuredContent`.

Configuration
- Token: `GITHUB_TOKEN` (fallback `GH_TOKEN`).
- Endpoints: `GITHUB_API_URL` (default https://api.github.com), `GITHUB_GRAPHQL_URL` (default https://api.github.com/graphql).
- API version header: `GITHUB_API_VERSION` (default 2022-11-28).
- HTTP timeout: `GITHUB_HTTP_TIMEOUT_SECS` (default 30).
- User-Agent: `github-mcp/<version>` (set automatically).

Use with MCP Clients

Assumptions
- You built or installed the binary and it is available on PATH as `github-mcp`.
- Set an auth token via env: `GITHUB_TOKEN` (or `GH_TOKEN`).
- The server uses stdio by default; no subcommand is required.

Claude Desktop
- Config file path
  - macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
  - Windows: `%APPDATA%\Claude\claude_desktop_config.json`
  - Linux (community builds): `~/.config/Claude/claude_desktop_config.json`
- Config snippet (paste into `claude_desktop_config.json`)
```json
{
  "mcpServers": {
    "github-mcp": {
      "command": "github-mcp",
      "args": [],
      "env": {
        "GITHUB_TOKEN": "${env:GITHUB_TOKEN}"
      }
    }
  }
}
```
- Reload: Fully quit Claude Desktop (Cmd+Q/Alt+F4) and relaunch.
- Docs: https://modelcontextprotocol.io/docs/develop/connect-local-servers
- Notes: On macOS, you may need to allow unsigned binaries in Privacy & Security.

Cursor
- Config file paths
  - Project: `<project>/.cursor/mcp.json`
  - Global: `~/.cursor/mcp.json` (Windows: `%USERPROFILE%\\.cursor\\mcp.json`)
- Config snippet (project or global `mcp.json`)
```json
{
  "mcpServers": {
    "github-mcp": {
      "type": "stdio",
      "command": "github-mcp",
      "args": [],
      "env": {
        "GITHUB_TOKEN": "${env:GITHUB_TOKEN}"
      }
    }
  }
}
```
- Reload: Close and reopen Cursor if tools don’t appear.
- Docs: https://cursor.com/docs/context/mcp
- Notes: Prefer PATH-resolved `github-mcp`; if using absolute paths with spaces on Windows, ensure proper quoting.

Continue.dev (VS Code/JetBrains)
- Global config
  - macOS/Linux: `~/.continue/config.yaml`
  - Windows: `%USERPROFILE%\\.continue\\config.yaml`
- Workspace-scoped (recommended): add `.continue/mcpServers/github-mcp.yaml` in your project.
- Workspace YAML (recommended)
```yaml
mcpServers:
  - name: github-mcp
    command: github-mcp
    args: []
    env:
      GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}"
```
- Or add the same block under `mcpServers:` in your global `~/.continue/config.yaml`.
- Reload: Open Continue, switch to Agent mode to use MCP tools; click "Reload config" if needed.
- Docs: https://docs.continue.dev/customize/deep-dives/mcp
- Notes: Store your token in `~/.continue/.env` (or workspace `.continue/.env`) as `GITHUB_TOKEN` to avoid committing secrets.

VS Code Copilot Chat (MCP)
- Config locations
  - Workspace: `.vscode/mcp.json`
  - User: use Command Palette → “MCP: Open User Configuration” (creates `mcp.json` under your VS Code user folder)
    - macOS: `~/Library/Application Support/Code/User/mcp.json`
    - Linux: `~/.config/Code/User/mcp.json`
    - Windows: `%APPDATA%\Code\User\mcp.json`
- Workspace `.vscode/mcp.json` example
```json
{
  "servers": {
    "github-mcp": {
      "command": "github-mcp",
      "args": [],
      "env": {
        "GITHUB_TOKEN": "${input:github_token}"
      }
    }
  },
  "inputs": [
    {
      "type": "promptString",
      "id": "github_token",
      "description": "GitHub token for github-mcp",
      "password": true
    }
  ]
}
```
- Optional: Enable discovery of Claude Desktop config via settings.json: `{ "chat.mcp.discovery.enabled": true }`.
- Reload: Save `mcp.json`, then click the “Start” code lens in the editor or use Command Palette → “MCP: Start Server”. Open Copilot Chat and switch to Agent mode.
- Docs: https://docs.github.com/copilot/customizing-copilot/using-model-context-protocol/extending-copilot-chat-with-mcp

Server invocation and auth (for all clients)
- Command: `github-mcp`
- Args: none required (stdio is default). Optionally add `--log-level warn`.
- Env: set `GITHUB_TOKEN` (fallback: `GH_TOKEN`). Example (macOS/Linux): `export GITHUB_TOKEN=ghp_...`  Example (Windows PowerShell): `setx GITHUB_TOKEN "ghp_..."` and restart your shell.

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

MCP response envelope (breaking change)
- tools/call results are wrapped:
  - `content`: array with one `{type:"text", text:"..."}` block for human-friendly display.
  - `structuredContent`: previous structured JSON payload preserved for programmatic clients.
  - `isError`: present and `true` when a tool-level error is included in `structuredContent.error`.
- Example (ping):
```
{
  "jsonrpc":"2.0",
  "id":3,
  "result":{
    "content":[{"type":"text","text":"hello"}],
    "structuredContent":{"message":"hello"}
  }
}
```
- Programmatic clients should switch to reading `result.structuredContent` and treat `result.content[0].text` as a display hint.

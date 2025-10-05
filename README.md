# GitHub MCP Server (TypeScript)

Minimal MCP server exposing GitHub Issue/PR/Workflow tools over stdio. Lean outputs by default; opt-in flags for heavier data. See `docs/methods.md` and `docs/playbook.md`.

Quick Start
- Prereqs: Node 20+, pnpm
- Setup: copy `.env.example` to `.env` and set `GITHUB_TOKEN` (and `GITHUB_BASE_URL` for GHE if needed)
- Install: `pnpm install`
- Typecheck: `pnpm typecheck`
- Tests: `pnpm test`
- Build: `pnpm build`
- Run MCP server: `pnpm start` (stdio)

Config
- GITHUB_TOKEN: GitHub PAT
- GITHUB_BASE_URL: optional (e.g. https://github.example.com/api/v3)
- LOG_LEVEL: pino level

Transport
- Stdio via `@modelcontextprotocol/sdk`. Tools are registered from `src/server/registry.ts`.

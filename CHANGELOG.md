# Changelog

All notable changes to this project will be documented in this file by Release Please.

## [0.6.0](https://github.com/HautechAI/github-mcp/compare/v0.5.0...v0.6.0) (2025-10-11)


### Features

* **ci:** coverage workflow + PR uncovered-lines table; gated live tests; docs (issue [#86](https://github.com/HautechAI/github-mcp/issues/86)) ([#87](https://github.com/HautechAI/github-mcp/issues/87)) ([2b39e8c](https://github.com/HautechAI/github-mcp/commit/2b39e8c26b2b0a41ced32aff6e58e763b21b5710))

## [0.5.0](https://github.com/HautechAI/github-mcp/compare/v0.4.1...v0.5.0) (2025-10-10)


### Features

* list_pr_review_comments_plain via REST with Link pagination; author/location flags; docs/tests update ([96560fc](https://github.com/HautechAI/github-mcp/commit/96560fc751c7c9c0f606f9b061759de657f130cf))
* list_pr_review_comments_plain via REST with Link-based pagination; include_author/location mapping; docs/tests/e2e updated. Closes [#82](https://github.com/HautechAI/github-mcp/issues/82) ([fa57f7d](https://github.com/HautechAI/github-mcp/commit/fa57f7d24968a683f9d05515e468f253c3f84a24))


### Bug Fixes

* fix:  ([39120c5](https://github.com/HautechAI/github-mcp/commit/39120c5edaa2d3499b4a6b67d43cf2e6b5501a2d))
* clippy cleanups (needless borrow in tests header format!; redundant closure in extract_next_path_from_link) ([5f45adc](https://github.com/HautechAI/github-mcp/commit/5f45adc707c0be29009bbb640d5b3ed75392dd48))

## [0.4.1](https://github.com/HautechAI/github-mcp/compare/v0.4.0...v0.4.1) (2025-10-10)


### Bug Fixes

* 78: GraphQL side fields for review comments/threads (no interface changes) ([#79](https://github.com/HautechAI/github-mcp/issues/79)) ([89ddcde](https://github.com/HautechAI/github-mcp/commit/89ddcde10f804fc9b1121a24dd37f8f319ecdf07))

## [0.4.0](https://github.com/HautechAI/github-mcp/compare/v0.3.0...v0.4.0) (2025-10-09)


### Features

* e2e ([f98faca](https://github.com/HautechAI/github-mcp/commit/f98facacac8f408da6b0a8e54e9b749855e365b5))

## [0.3.0](https://github.com/HautechAI/github-mcp/compare/v0.2.1...v0.3.0) (2025-10-09)


### Features

* **actions:** light tools for secrets/variables/environments ([#60](https://github.com/HautechAI/github-mcp/issues/60)) ([c7b7dd1](https://github.com/HautechAI/github-mcp/commit/c7b7dd16e829c63cb7f5a1a81d24d435b07d179a))

## [0.2.1](https://github.com/HautechAI/github-mcp/compare/v0.2.0...v0.2.1) (2025-10-07)


### Bug Fixes

* tools/list nextCursor omission and CI consolidation (fix [#48](https://github.com/HautechAI/github-mcp/issues/48)) ([0dbc41a](https://github.com/HautechAI/github-mcp/commit/0dbc41a345f5c7ba4a82b4f2ed04594fec84f196))

## [0.2.1](https://github.com/HautechAI/github-mcp/compare/v0.2.0...v0.2.1) (2025-10-07)


### Bug Fixes

* tools/list: omit nextCursor when not paginating to align with MCP Inspector schema; add test asserting nextCursor is absent or a string; consolidate E2E into CI workflow (fix [#48](https://github.com/HautechAI/github-mcp/issues/48))

## [0.2.0](https://github.com/HautechAI/github-mcp/compare/v0.1.5...v0.2.0) (2025-10-07)


### Features

* **mcp:** MCP-compliant tool result envelope with structuredContent + tests/docs (fix [#45](https://github.com/HautechAI/github-mcp/issues/45)) ([42c27da](https://github.com/HautechAI/github-mcp/commit/42c27da82a239771d3ad6c07381aed0a936d8014))
* **mcp:** wrap tools/call results in MCP content envelope with structuredContent; add isError flag; update handlers, tests, README; closes [#45](https://github.com/HautechAI/github-mcp/issues/45) ([37cbad0](https://github.com/HautechAI/github-mcp/commit/37cbad0956490859cf5eca3ae59ca98a7ecf4838))

## [0.1.5](https://github.com/HautechAI/github-mcp/compare/v0.1.4...v0.1.5) (2025-10-07)


### Bug Fixes

* **ci:** rustfmt and release checksum line-ending normalization ([#41](https://github.com/HautechAI/github-mcp/issues/41)) ([fbe1005](https://github.com/HautechAI/github-mcp/commit/fbe1005c144e3b64476bb866bf7feef7c8446a01))

## [0.1.4](https://github.com/HautechAI/github-mcp/compare/v0.1.3...v0.1.4) (2025-10-06)


### Bug Fixes

* **release:** Normalize CRLF in checksum aggregation ([#30](https://github.com/HautechAI/github-mcp/issues/30)) ([e26621f](https://github.com/HautechAI/github-mcp/commit/e26621fe6190003c237044feb54b071f21335ffb))

## [0.1.3](https://github.com/HautechAI/github-mcp/compare/v0.1.2...v0.1.3) (2025-10-06)


### Bug Fixes

* **release:** robust Windows checksum path resolution and pre-hash check ([#24](https://github.com/HautechAI/github-mcp/issues/24), [#25](https://github.com/HautechAI/github-mcp/issues/25)) ([#26](https://github.com/HautechAI/github-mcp/issues/26)) ([a57f278](https://github.com/HautechAI/github-mcp/commit/a57f278d08600326768c4353bacff89cedb57789))

## [0.1.2](https://github.com/HautechAI/github-mcp/compare/v0.1.1...v0.1.2) (2025-10-06)


### Bug Fixes

* README.md ([4b05e6d](https://github.com/HautechAI/github-mcp/commit/4b05e6ddd0687de7a4599791fc711c31443ac464))

## [0.1.1](https://github.com/HautechAI/github-mcp/compare/v0.1.0...v0.1.1) (2025-10-06)


### Bug Fixes

* README.md ([994025f](https://github.com/HautechAI/github-mcp/commit/994025fc0800a3ab5082ca130a1524b71ad917c7))

## 0.1.0 (2025-10-06)


### Features

* **server:** bootstrap Rust crate + stdio JSON-RPC harness (initialize/tools/list/tools/call/ping); add tests; scaffold milestone 1 ([#9](https://github.com/HautechAI/github-mcp/issues/9)) ([3a6e842](https://github.com/HautechAI/github-mcp/commit/3a6e8425df0d1ba7de74eb4c1f849f15bf916d41))

### Unreleased
- feat: Gate built-in `ping` tool behind env flag `GITHUB_MCP_ENABLE_PING` (default OFF). When disabled, `ping` is omitted from `tools/list` and `tools/call ping` returns JSON-RPC `-32601` (Tool not found: ping (disabled)). Update tests, E2E, and docs. (fix [#64](https://github.com/HautechAI/github-mcp/issues/64))
- breaking: Remove direct JSON-RPC method `ping`. Use `tools/call` with `GITHUB_MCP_ENABLE_PING=true` if you need `ping`.

## [Unreleased]
- Initial project bootstrap and CI scaffolding.

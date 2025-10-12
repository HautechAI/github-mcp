Testing

- Unit and mocked integration tests: `cargo test` runs fast with no network using httpmock-based fixtures. CI runs these on all platforms (Linux, macOS, Windows).
- Coverage: GitHub Actions workflow `.github/workflows/coverage.yml` runs `cargo-llvm-cov` with `nextest` to produce `lcov.info` and `coverage.json`. It posts a PR comment summarizing uncovered lines for changed Rust files and appends the same table to the job summary.
  - Tools are cached via `Swatinem/rust-cache@v2`.
  - Tooling pinned: `cargo-llvm-cov@0.6.9`, `nextest@0.9.68`.

Inspector smoke in CI
- Minimal end-to-end smoke is kept using the MCP Inspector:
  - tools/list to validate tool discovery and envelope shape.
  - tools/call get_issue against HautechAI/github-mcp-test-repo #10 (skips on forks or when no token available).
- This replaces the previous live E2E suite; no mutation tools run in CI.

Local development
- Run all tests locally (no network):
  - `cargo test --locked --all-targets --all-features`
- Optional: run Inspector smoke locally if you have a token:
  - Build: `cargo build --release --locked`
  - Tools list: `npx @modelcontextprotocol/inspector-cli --cli ./target/release/github-mcp --method tools/list`
  - Get issue: `GITHUB_TOKEN=ghp_xxx npx @modelcontextprotocol/inspector-cli --cli ./target/release/github-mcp --method tools/call --params {name:get_issue,arguments:{owner:HautechAI,repo:github-mcp-test-repo,number:10}}`
  - If the token is missing or insufficient, the smoke call gracefully fails or can be skipped.

Notes
- Write-path tools (e.g., merge_pr, update_issue, update_pull_request, fork_repository) are covered via mocked integration tests only.
- Some tools are gated by feature flags or Accept headers. Tests do not rely on env flags; gating behavior is validated via default (disabled) paths only. Stars Accept gating is asserted via mocked responses.

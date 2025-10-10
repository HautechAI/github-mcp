Testing

- Unit and mocked integration tests: `cargo test` runs fast, no network. CI runs these on all platforms.
- Coverage: GitHub Actions workflow `.github/workflows/coverage.yml` runs `cargo-llvm-cov` with `nextest` to produce `lcov.info` and `coverage.json`. It posts a PR comment summarizing uncovered lines for changed Rust files and appends the same table to the job summary.
  - Tools are cached via `Swatinem/rust-cache@v2`.
  - Tooling pinned: `cargo-llvm-cov@0.6.9`, `nextest@0.9.68`.

Live API tests (gated)
- Live tests are in `tests/live/` and are marked with `#[ignore]` so they do not run by default.
- To run locally:
  - Set `GITHUB_TOKEN` (or `GH_TOKEN`).
  - Set `LIVE_API_TESTS=1`.
  - Set repository fixtures:
    - `E2E_OWNER` (org/user)
    - `E2E_REPO`
    - Optional: `E2E_ISSUE_NUM`, `E2E_PR_NUM`
  - Command:
    - `cargo test -- --ignored` (or `cargo llvm-cov nextest -- --run-ignored yes` for coverage)

CI behavior
- The coverage workflow runs only unit/mocked tests by default; live tests are enabled when both `LIVE_API_TESTS=1` repository/environment variable is set and the workflow has access to `GITHUB_TOKEN`.
- When enabled, live tests are run via nextest with reduced parallelism (`--jobs=2`).

Notes
- Live tests are read-only and should not mutate repositories. If you add new live tests, keep them read-only.
- If CI secrets are missing, live tests are skipped gracefully.


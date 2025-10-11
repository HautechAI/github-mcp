Testing

- Unit and mocked integration tests: `cargo test` runs fast, no network. CI runs these on all platforms.
- Coverage: GitHub Actions workflow `.github/workflows/coverage.yml` runs `cargo-llvm-cov` with `nextest` to produce `lcov.info` and `coverage.json`. It posts a PR comment summarizing uncovered lines for changed Rust files and appends the same table to the job summary.
  - Tools are cached via `Swatinem/rust-cache@v2`.
  - Tooling pinned: `cargo-llvm-cov@0.6.9`, `nextest@0.9.68`.

Live API tests (ignored by default)
- Live tests are in `tests/live/` and are marked with `#[ignore]` so they do not run by default.
- Prerequisites when running ignored tests:
  - Must run under a Doppler context (environment contains `DOPPLER_*` markers). Recommended command below.
  - A GitHub token must be present via env: `GITHUB_TOKEN` or `GH_TOKEN`.
  - Repository fixtures are required:
    - `E2E_OWNER` (org/user)
    - `E2E_REPO`
    - `E2E_ISSUE_NUM` (number)
    - Optional: `E2E_PR_NUM`
- If any prerequisite is missing and you run with `--ignored`, tests will panic fast with guidance (they no longer skip silently).
- Local run example (Doppler):
  - `doppler run -p github-mcp -c dev -- env E2E_OWNER=HautechAI E2E_REPO=github-mcp-test-repo E2E_ISSUE_NUM=10 cargo test -- --ignored`
  - For coverage with ignored tests: `doppler run -p github-mcp -c dev -- env E2E_OWNER=... E2E_REPO=... E2E_ISSUE_NUM=... cargo llvm-cov nextest -- --run-ignored yes`

CI behavior
- The main CI runs unit/mocked tests on all platforms.
- Live tests run in a dedicated job under Doppler, with required fixtures set, using: `doppler run -p github-mcp -c dev -- env E2E_OWNER=... E2E_REPO=... E2E_ISSUE_NUM=... cargo test -- --ignored`.
- The E2E harness (`scripts/e2e_live.sh`) is also executed under Doppler in CI.
- If Doppler or required env is missing, the job fails by design.

Notes
- Live tests are read-only and should not mutate repositories. If you add new live tests, keep them read-only.
- The E2E harness enforces Doppler and token presence and will fail fast if not provided.

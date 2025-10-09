Progress updates for E2E live tests (Issue #58)

- Added scripts/e2e_live.sh: harness using inspector-cli; read-mostly by default, mutations gated.
- Added .github/workflows/e2e-live.yml: build release, run harness under Doppler, upload artifacts; nightly cron.
- Added docs/e2e/README.md and linked guidance.

Next steps
- Monitor CI runs on the PR and nightly; adjust assertions if flakiness appears (GitHub API variability).
- Consider adding targeted retries for transient 502/503/timeout.


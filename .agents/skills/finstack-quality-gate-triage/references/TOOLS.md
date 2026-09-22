# Finstack Quant Quality Gate Tools

Use repo-native checks first. External scanners are optional only when installed and relevant.

## Rust

- **Iteration (fast):**
  - Lint one crate: `mise run rust-lint-crate -- <package>`
  - Test one crate: `mise run rust-test-crate -- <package>`
  - One integration binary: `mise run rust-test-integration -- <package> <test>`
  - Filter by name: `mise run rust-test-filter -- <package> <filter>`
  - One bench: `mise run rust-bench-crate -- <package> <bench>`
- **Full workspace (CI / final verification):**
  - Lint: `mise run rust-lint`
  - Tests: `mise run rust-test`
  - Benches: `mise run rust-bench`
- Focused crate checks: use the closest crate-specific command available in `mise.toml`
- Supply chain: `cargo deny check`

## Python

- Build extension: `mise run python-build`
- Release-profile runtime build: `mise run python-build -- --release`
- Lint: `mise run python-lint`
- Typecheck: `mise run python-typecheck`
- Tests: `mise run python-test`

## WASM

- Build: `mise run wasm-build`
- Lint: `mise run wasm-lint`
- Tests: `mise run wasm-test`

## UI

- Format: `mise run ui-fmt`
- Typecheck: `mise run ui-typecheck`
- Tests: `mise run ui-test`
- Combined gate: `mise run ui-check`

## Broad Gates

- Format: `mise run all-fmt`
- Lint: `mise run all-lint`
- Tests: `mise run all-test`
- CI-style gate: `mise run all-ci`
- Pre-commit: `mise pre-commit-run`

## Triage Tips

- Start from the first real error, not the final cascade.
- Prefer targeted checks while iterating.
- Run broad gates only when the change crosses many components or before final release readiness.
- Do not add `allow` or ignore suppressions until the root cause is understood and a real fix is not appropriate.

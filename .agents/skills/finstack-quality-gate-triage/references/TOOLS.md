# Finstack Quality Gate Tools

Use repo-native checks first. External scanners are optional only when installed and relevant.

## Rust

- Lint: `mise run rust-lint`
- Tests: `mise run rust-test`
- Focused crate checks: use the closest crate-specific command available in `mise.toml`
- Supply chain: `cargo deny check`

## Python

- Build extension: `mise run python-build`
- Release-profile runtime build: `mise run python-build -- --release`
- Lint: `mise run python-lint`
- Typecheck: `mise run python-typecheck`
- Tests: `mise run python-test`
- Verify types when bindings change: `mise run python-verifytypes` if configured

## WASM

- Build: `mise run wasm-build`
- Lint: `mise run wasm-lint`
- Tests: `mise run wasm-test`

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

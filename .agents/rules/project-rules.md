---
trigger: always_on
description:
globs:
---
- For rust code changes, run scoped checks for the affected crate(s) after each set of changes:
  - Lint: `mise run rust-lint-crate -- <package>` (add `--all-features` when touching feature-gated code)
  - Tests: `mise run rust-test-crate`, `rust-test-integration`, or `rust-test-filter` for the same package
  - Benchmarks: `mise run rust-bench-crate -- <package> <bench>` — never `rust-bench` unless validating performance broadly
  - See `.agents/rules/selective-test-running.mdc` for path → package mapping and examples
- Run full-workspace gates only at the end of a plan or before commit/CI:
  - `mise run rust-lint` (not after every edit — full-workspace clippy is still slow)
  - `mise run rust-test`, `mise run all-lint`, `mise run all-test`
- For wasm code, run `mise run wasm-lint` after each set of changes (WASM surface is small enough to lint whole).
- For python code changes, run `mise run python-lint` after each set of changes; run scoped pytest (`uv run pytest path -k filter`) rather than `python-test --all` while iterating.
- Fix any errors that are present from lints and tests before moving on to next task.
- If you change the rust library, you will need to rebuild the python and wasm bindings before using in python/wasm. Use `mise run python-build` for python and `mise run wasm-build` for typescript/js/wasm
- DO NOT OVER-ENGINEER THE SOLUTIONS. AIM FOR SIMPLICITY.
- DO NOT RUN `cargo test` or `cargo clippy` DIRECTLY; use the scoped `mise run rust-*` tasks (they use nextest and skip rust doc tests).
- Public APIs follow the result-return contract in
  `.claude/skills/finstack-consistency-reviewer/conventions.md`. In short:
  computation entry points return typed results (Rust struct / `Py*` wrapper /
  plain JS object), not JSON strings; only `_json`/`*Json`-suffixed wire
  surfaces return JSON, and each must have a public typed twin. Every result
  wrapper carries typed getters + `to_json` + `from_json` (`#[staticmethod]`),
  plus `to_dataframe()` in Python (and `to_series()` for 1-D labeled vectors).
- In WASM, never call `serde_wasm_bindgen::to_value` directly — always
  `crate::utils::to_js_value`. The raw serializer emits ES `Map`s that
  `JSON.stringify` drops silently.

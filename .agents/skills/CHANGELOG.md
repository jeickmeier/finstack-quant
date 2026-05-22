# Skills Refactor Changelog

## Active Catalog

Final active skill count: 12.

Active skills:

- `binding-parity-reviewer`
- `consistency-reviewer`
- `documentation-maintainer`
- `finstack-simplify`
- `performance-reviewer`
- `production-release-prep`
- `quality-gate-triage`
- `quant-finance-review`
- `refactor`
- `rust-architecture-review`
- `rust-library-architecture-docs`
- `senior-code-review`

## Renamed Or Merged

- `python-binding-reviewer` -> `binding-parity-reviewer`
- `bug_hunting` -> `quality-gate-triage`
- `documentation-cleanup` + `documentation-reviewer` -> `documentation-maintainer`

## Retired Or Demoted

Moved to `.agents/retired-skills/`:

- `code-simplifier`
- `simplicity-auditor`
- `dead-code-removal`

Their active responsibilities now live in `refactor`, `finstack-simplify`, `quality-gate-triage`, and `production-release-prep`.

## Coverage Added

- Binding parity across Rust, PyO3, WASM, stubs, exports, parity contract, tests, and examples.
- Numerical regression and market-convention modes in `quant-finance-review`.
- Benchmark regression mode in `performance-reviewer`.
- Quality-gate and CI/pre-commit triage in `quality-gate-triage`.
- Combined API documentation and stale-doc cleanup in `documentation-maintainer`.
- Golden output examples and eval prompts for each active skill.

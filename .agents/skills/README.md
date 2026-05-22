# Finstack Skill Catalog

This directory contains the active project skills for maintaining the finstack Rust/Python/WASM quant library.

## Active Skills

| Skill | Use For | Prefer Another Skill When |
| --- | --- | --- |
| `quant-finance-review` | Pricing, risk, calibration, market conventions, numerical regression | Pure architecture or binding-shape review |
| `rust-architecture-review` | Crate/module boundaries, ownership, errors, async/concurrency, public API shape | Writing architecture docs |
| `rust-library-architecture-docs` | Source-backed Rust architecture documentation | Critiquing architecture quality |
| `binding-parity-reviewer` | Rust/PyO3/WASM/stub/export/parity-contract drift | The main issue is quant correctness |
| `finstack-simplify` | Finstack-specific slop, dedupe, wrapper bloat, public API consolidation | Small mechanical refactor with known scope |
| `refactor` | Behavior-preserving structural edits after scope is clear | Broad finstack simplification audit |
| `performance-reviewer` | Hot paths, allocations, concurrency, benchmark regression | Formula/convention correctness |
| `documentation-maintainer` | API docs, stale docs, README/spec/changelog cleanup, examples | Release-wide readiness |
| `production-release-prep` | Release orchestration, semver, docs, audit, final gates | One failing check or narrow cleanup |
| `quality-gate-triage` | Pasted lint/test/pre-commit/CI failures, bug-fix loops | Read-only review |
| `senior-code-review` | Broad fallback review when no specialist applies | Any specialist skill fits |
| `consistency-reviewer` | Naming, convention inventory, pattern drift | Dedupe/API-surface consolidation |

## Retired Or Demoted Skills

The previous `code-simplifier`, `simplicity-auditor`, and `dead-code-removal` skills are kept under `.agents/retired-skills/` for reference. Their active responsibilities moved into `refactor`, `finstack-simplify`, `quality-gate-triage`, and `production-release-prep`.

The old `documentation-cleanup` and `documentation-reviewer` split is now `documentation-maintainer`.

The old `python-binding-reviewer` is now `binding-parity-reviewer`.

The old `bug_hunting` skill is now `quality-gate-triage`.

## Catalog Rule

Add a new top-level skill only when the trigger, workflow, and output are distinct. Otherwise add a reference, example, output, or eval to an existing skill.

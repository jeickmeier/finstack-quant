# Finstack Skill Catalog

This directory contains the active project skills for maintaining the finstack Rust/Python/WASM quant library.

## Agent Compatibility

These skills use the shared Agent Skills layout: one folder per skill, each with a `SKILL.md` file containing `name` and `description` frontmatter. Keep `.agents/skills` as the source of truth.

- Cursor discovers `.agents/skills` and exposes skills by name.
- Codex discovers `.agents/skills`; invoke explicitly via the skill selector or `$skill-name`.
- GitHub Copilot discovers `.agents/skills` for Copilot agent mode, Copilot CLI, and cloud agents.
- Claude Code discovers `.claude/skills`; this repo exposes that path as a symlink to `.agents/skills`.

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

## Catalog Rule

Add a new top-level skill only when the trigger, workflow, and output are distinct. Otherwise add a reference, example, output, or eval to an existing skill.

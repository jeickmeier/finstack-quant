# Finstack Skill Catalog

Project skills for maintaining the finstack-quant Rust/Python/WASM workspace.
Each skill packages a review or refactor workflow that would otherwise have to
be re-explained every session: what to look at, in what order, against which
invariants, and what the output should contain.

`.agents/skills` is the source of truth. Its sibling
[`.agents/rules`](../rules/) holds the always-on standards that skills assume
are already loaded: `project-description.md`, `project-rules.md`, the
per-language `rust/`, `python/`, `wasm/` directories, and two Cursor-format
files (`selective-test-running.mdc`, `source-backed-golden-tests.mdc`).

## Agent compatibility

Skills use the shared Agent Skills layout: one directory per skill, each with a
`SKILL.md` carrying `name` and `description` frontmatter.

- Cursor discovers `.agents/skills` and exposes skills by name.
- Codex discovers `.agents/skills`; invoke explicitly via the skill selector or
  `$skill-name`.
- GitHub Copilot discovers `.agents/skills` for agent mode, the CLI, and cloud
  agents.
- Claude Code discovers `.claude/skills`, which is a symlink to this directory.
  Project instructions load via `CLAUDE.md` (`@AGENTS.md`).

Three tracked symlinks give the same trees their host-specific spellings:

| Symlink | Target |
| --- | --- |
| `.claude/skills` | `.agents/skills` |
| `.claude/rules` | `.agents/rules` |
| `.cursor/rules` | `.agents/rules` |

So a rule or skill referenced as `.claude/skills/<name>/...`,
`.cursor/rules/<name>` or `.agents/skills/<name>/...` is the same file. Edit
under `.agents/`; never through a symlinked path.

## Active skills

| Skill | Use for | Prefer another skill when |
| --- | --- | --- |
| [`finstack-quant-finance-review`](finstack-quant-finance-review/) | Pricing, risk, calibration, market conventions, numerical regression | Pure architecture or binding-shape review |
| [`finstack-stepwise-quant-audit`](finstack-stepwise-quant-audit/) | Walking one pricing path, bootstrap, schedule builder, or calibration stage by stage with per-step evidence | A broad review is wanted, not a sequential walkthrough |
| [`finstack-rust-architecture-review`](finstack-rust-architecture-review/) | Crate/module boundaries, ownership, errors, concurrency, public API shape | Writing architecture docs |
| [`finstack-rust-library-architecture-docs`](finstack-rust-library-architecture-docs/) | Source-backed Rust architecture documentation | Critiquing architecture quality |
| [`finstack-binding-parity-reviewer`](finstack-binding-parity-reviewer/) | Rust/PyO3/WASM/stub/export/`parity_contract.toml` drift | The main issue is quant correctness |
| [`finstack-simplify`](finstack-simplify/) | Slop, dedupe, wrapper bloat, public API consolidation | Small mechanical refactor with known scope |
| [`finstack-refactor`](finstack-refactor/) | Behavior-preserving structural edits after scope is clear | Broad simplification audit |
| [`finstack-performance-reviewer`](finstack-performance-reviewer/) | Hot paths, allocations, concurrency, benchmark regression | Formula or convention correctness |
| [`finstack-documentation-maintainer`](finstack-documentation-maintainer/) | API docs, stale docs, README/spec/changelog cleanup, examples | Release-wide readiness |
| [`finstack-production-release-prep`](finstack-production-release-prep/) | Release orchestration, semver, docs, audits, final gates | One failing check or narrow cleanup |
| [`finstack-quality-gate-triage`](finstack-quality-gate-triage/) | Pasted lint/test/pre-commit/CI failures, bug-fix loops | Read-only review |
| [`finstack-senior-code-review`](finstack-senior-code-review/) | Broad fallback review when no specialist applies | Any specialist skill fits |
| [`finstack-consistency-reviewer`](finstack-consistency-reviewer/) | Naming, convention inventory, pattern drift | Dedupe or API-surface consolidation |

## Skill anatomy

Every skill has `SKILL.md` (the workflow) and `evals/evals.json` (prompts with
expected-output descriptions, keyed by `skill_name`). Supporting material is
loaded on demand rather than inlined, so `SKILL.md` stays short:

- `references/` — the common case: checklists, patterns, workspace maps.
  `finstack-quant-finance-review` additionally carries `market-standards/`
  split by asset class (rates, fixed income, equity, FX, cross-asset) and by
  algorithm.
- `examples/` or `examples.md` — golden outputs showing the expected report
  shape.
- `finstack-consistency-reviewer/conventions.md` is not just reference material:
  [`.agents/rules/project-rules.md`](../rules/project-rules.md) cites it as the
  normative home of the public-API result-return contract.
- `finstack-quant-finance-review/agents/openai.yaml` supplies display metadata
  for hosts that render a skill picker.

Layout under a skill is not uniform, and that is tolerated — some use a
`references/` directory, others a flat `reference.md`. Match the skill you are
editing rather than reorganizing it.

## Catalog rule

Add a new top-level skill only when the trigger, workflow, and output are all
distinct. Otherwise add a reference, example, output, or eval to an existing
skill. When adding one, update this table and
[`changelog/unreleased.md`](../../changelog/unreleased.md) in the same change.

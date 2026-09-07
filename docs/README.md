# docs

Repository documentation that is not rustdoc, not a crate README, and not a
notebook. [`index.md`](index.md) is the reader-facing map — start there if you
want to *learn* the library. This file is the directory inventory: what
physically lives here and who owns it.

## Contents

| Path | What it is |
| --- | --- |
| [`index.md`](index.md) | Public documentation map: getting started, concepts, notebook curriculum, package surfaces. |
| [`REFERENCES.md`](REFERENCES.md) | Canonical citation anchors for formulas, market conventions, and standards. Rustdoc `# References` sections link into it by anchor. |
| [`CONTRACTS.md`](CONTRACTS.md) | The persisted-contract matrix: which Rust serde types are wire contracts, their generated schemas, strict loaders, and Rust/Python/WASM API map. |
| [`SERDE_STABILITY.md`](SERDE_STABILITY.md) | The serde contract policy: source-of-truth rules, v1-only scope, deterministic generation requirements. |
| [`audits/`](audits/) | Dated deep-audit reports. One file per audit, named `YYYY-MM-DD-<scope>.md`. Historical records — do not rewrite them to match later code. |
| [`superpowers/specs/`](superpowers/specs/) | Design specs for planned or recently landed work. |
| [`superpowers/plans/`](superpowers/plans/) | Implementation plans of record. Do not edit an active plan unless the current task is that plan. |

## Documentation that lives elsewhere

Not everything doc-shaped is under `docs/`:

- [`../INVARIANTS.md`](../INVARIANTS.md) — cross-crate numerical and API
  invariants. Normative; cite it rather than restating it.
- [`../AGENTS.md`](../AGENTS.md) — engineering rules, project structure, the
  public-API documentation contract, clippy strictness. Claude Code loads the
  same file through [`../CLAUDE.md`](../CLAUDE.md) (`@AGENTS.md`).
- [`../CONTRIBUTING.md`](../CONTRIBUTING.md) — contribution workflow and the
  binding-change checklist.
- [`../.agents/rules/`](../.agents/rules/) — per-language standards
  (`rust/`, `python/`, `wasm/`), `project-description.md`, `project-rules.md`,
  and two Cursor-format `.mdc` rules. `.claude/rules` and `.cursor/rules` are
  both symlinks to this directory.
- [`../benchmarks/`](../benchmarks/) — the materialization benchmark record and
  its checked-in baselines.
- [`../finstack-quant-py/README.md`](../finstack-quant-py/README.md) and
  [`../finstack-quant-wasm/README.md`](../finstack-quant-wasm/README.md) —
  binding packages. Per-crate READMEs live next to each crate under
  `../finstack-quant/`.
- Rustdoc is the API reference. Build it with `mise run rust-doc`; nothing in
  `docs/` duplicates it.

## Conventions

- Markdown is linted by `markdownlint-fix` under `pre-commit`; run
  `mise run pre-commit-run` before committing doc changes.
- Prose wraps at roughly 80 columns.
- No README or Markdown file in this repository is included into rustdoc via
  `#![doc = include_str!(...)]`, so Markdown code blocks are **never**
  compile-checked. Verify every symbol you write against source.
- Audit and plan documents are dated and append-only in spirit. Supersede them
  with a new dated file instead of editing history.

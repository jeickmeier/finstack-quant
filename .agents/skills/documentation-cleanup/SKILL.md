---
name: documentation-cleanup
description: "Use whenever documentation needs cleanup, stale-reference audits, concise rewrites, removal of AI/process/meta language, README/API/spec/changelog accuracy checks, or validation of examples, commands, links, and code references against current source and tests."
---

# Documentation Cleanup

Use this skill to turn noisy or AI-shaped documentation into concise, accurate docs about the current code. The finished documentation should describe the software as it exists, not the process used to write, review, or generate it.

The goal is not to make docs shorter at any cost. The goal is to make them more useful, more factual, and easier for the intended reader to act on.

## Core Standard

Good documentation is:

- Accurate to the referenced code and tests.
- Concise enough that every paragraph earns its place.
- Written for the reader using the API, module, workflow, or system.
- Free of process commentary, implementation-history chatter, and AI phrasing.
- Specific about behavior, constraints, inputs, outputs, errors, and invariants.

Remove or rewrite language like:

- "This document explains..."
- "We added..."
- "The implementation now..."
- "This section was generated..."
- "As part of this change..."
- "Comprehensive", "robust", "seamless", "powerful", or similar filler unless technically precise.
- Hedge phrases that hide responsibility: "could", "may want to", "if desired", "optionally" when the behavior or recommendation is actually fixed.
- Stale status language: "currently being implemented", "future work", "TODO", "temporary", unless the document is explicitly a plan or roadmap and the status is still true.
- Implementation details that do not change reader behavior.
- Repeated setup, architecture, or API explanations already covered by a nearer canonical document.

Treat these documents carefully:

- Changelogs and migration guides may need historical wording. Keep versioned facts and migration rationale, but remove meta commentary about writing or reviewing the change.
- Design specs, plans, and ADRs can discuss decisions and alternatives. Make their status explicit and remove claims that pretend planned behavior already exists.
- Generated docs should normally be fixed at the source or regeneration step when the generated output is stale.
- Public API docs need the highest verification standard because examples and names become compatibility promises.

## Workflow

Track the cleanup with this checklist:

```markdown
Documentation Cleanup:
- [ ] Identify documentation targets and intended audience
- [ ] Inventory every referenced code symbol, file, command, config, and test
- [ ] Verify each reference against current source
- [ ] Decide the verification depth for each claim and example
- [ ] Rewrite documentation to describe current behavior only
- [ ] Remove process language, hype, duplication, and stale caveats
- [ ] Validate links, examples, commands, and public API names
- [ ] Run the smallest relevant formatting, linting, or docs checks
- [ ] Summarize changed scope and any unchecked risk
```

### 1. Identify Targets

Find the docs the user asked to clean. If the scope is broad, start with the files explicitly named by the user or the files most directly tied to the current change. Do not rewrite unrelated docs just because they are nearby.

For each target, determine:

- Who reads it.
- What decision or task it supports.
- Which code, commands, configs, tests, or APIs it references.
- Whether it is public-facing, contributor-facing, or internal planning material.
- Whether the file is hand-written, generated, or derived from another contract/source file.

Keep the scope tight. If you discover unrelated stale docs, note them separately unless they block the requested cleanup. If the documentation exposes a likely code bug, report it; do not change code just to make the docs true unless the user asked for that.

### 2. Verify Referenced Code

Before editing, inspect every referenced item. Follow references until the documented behavior is proven from source, tests, schemas, generated bindings, examples, or checked command output.

Verify:

- Symbol names, paths, exports, and module locations.
- Function signatures, argument names, return values, and error behavior.
- CLI commands, task names, environment variables, and config keys.
- Examples and snippets compile or match the current API shape.
- Claims about performance, stability, compatibility, or scope are supported.

If a claim cannot be verified, remove it or rewrite it as a narrow, factual statement.

Use this calibration for verification depth:

| Documentation type | Minimum verification |
| --- | --- |
| README, tutorial, notebook, public API docs | Check symbols, examples, imports, and commands against source and the smallest runnable validation available. |
| Contributor or architecture docs | Check paths, task names, config keys, invariants, and cited tests. |
| Internal notes, plans, ADRs, specs | Check whether statements describe implemented behavior, planned behavior, or historical context; label status accurately. |
| Generated or derived docs | Check the source contract/generator and avoid hand-editing generated output unless that is the repo convention. |
| Changelogs and migration notes | Check version names, removed/renamed APIs, replacement paths, and any migration examples. |

For behavioral claims, prefer a source-and-test pair: source proves what the code does, tests prove the intended public behavior. If only one is practical, say so in the final summary.

### 3. Rewrite For The Current Code

Write in present tense. Describe what the code does now, how to use it, and what constraints matter. Prefer short sections with concrete nouns and verbs.

Keep:

- Purpose and reader outcome.
- Required inputs and valid values.
- Observable behavior and outputs.
- Important invariants, edge cases, and error conditions.
- Minimal examples that match current code.
- Canonical links to deeper docs instead of duplicated explanations.

Remove:

- Implementation history unless it is essential migration guidance.
- Meta commentary about authors, agents, reviews, or generation.
- Apologies, caveats, and speculative roadmap language.
- Repeated explanations already covered by nearby docs.
- Marketing adjectives that do not change reader behavior.

When adding missing information, keep it proportional to the document's job. It is in scope to add a missing constraint, argument, error case, or example needed for accuracy. It is not in scope to turn a cleanup into a broad documentation expansion unless the user asked for that.

### 4. Check Examples And Commands

Examples are documentation promises. Update or delete examples that do not match the current code.

When practical, run the smallest command that validates the edited surface:

- Markdown formatting or linting if the repo has it.
- Targeted unit tests for referenced code.
- Doctests, notebook checks, binding parity checks, or API generation checks when those are the documented surface.
- Build or type checks only when the edited examples or generated docs depend on them.

Prefer focused checks while iterating. Reserve broad checks for broad documentation rewrites or when project rules require them.

If validation is too expensive or unavailable, still check syntax and names by inspection, then clearly report what was not run and why.

### 5. Guard Against Common Failures

Avoid these cleanup mistakes:

- Deleting real constraints because they sound negative or messy.
- Keeping confident claims that were not checked.
- Updating examples by guesswork instead of tracing the current API.
- Rewriting a plan, changelog, or migration guide as if it were current-state reference documentation.
- Expanding the document with broad background material that the target reader does not need.
- Fixing one mention of a renamed symbol while leaving the same stale name elsewhere in the same document.

### 6. Final Review

Read the final document once as the target reader and once as a skeptic.

Confirm:

- No text refers to the act of writing, editing, generating, reviewing, or implementing unless the document is explicitly a plan or changelog.
- Every code reference was checked.
- Every claim is either verified or removed.
- The document is shorter or clearer than before.
- The result preserves useful warnings and constraints instead of deleting them for neatness.
- Generated or derived docs were handled through their source path where appropriate.

## Output Style

When reporting back:

- State which docs were cleaned.
- Mention the code references checked.
- List validation commands run and their outcome.
- Call out any references that could not be verified.

Keep the summary brief. The cleaned documentation is the deliverable.

Example final summary:

```markdown
Cleaned docs/api.md and finstack-py/README.md. Verified the public names against the Rust exports, PyO3 bindings, stubs, and the targeted parity test. Removed stale implementation-history wording and updated two examples. Validation: `mise run test-python -- tests/parity/test_core.py` passed. One benchmark claim was removed because it was not supported by current tests.
```

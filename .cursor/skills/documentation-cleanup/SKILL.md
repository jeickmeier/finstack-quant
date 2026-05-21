---
name: documentation-cleanup
description: Removes AI-generated documentation slop and rewrites concise documentation that reflects the code as it exists. Use when cleaning docs, removing process/meta language, auditing code references in docs, or making documentation accurate against source and tests.
---

# Documentation Cleanup

Use this skill to turn noisy or AI-shaped documentation into concise, accurate docs about the current code. The finished documentation should describe the software as it exists, not the process used to write, review, or generate it.

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

## Workflow

Track the cleanup with this checklist:

```markdown
Documentation Cleanup:
- [ ] Identify documentation targets and intended audience
- [ ] Inventory every referenced code symbol, file, command, config, and test
- [ ] Verify each reference against current source
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

### 2. Verify Referenced Code

Before editing, inspect every referenced item. Follow references until the documented behavior is proven from source, tests, schemas, generated bindings, examples, or checked command output.

Verify:

- Symbol names, paths, exports, and module locations.
- Function signatures, argument names, return values, and error behavior.
- CLI commands, task names, environment variables, and config keys.
- Examples and snippets compile or match the current API shape.
- Claims about performance, stability, compatibility, or scope are supported.

If a claim cannot be verified, remove it or rewrite it as a narrow, factual statement.

### 3. Rewrite For The Current Code

Write in present tense. Describe what the code does now, how to use it, and what constraints matter. Prefer short sections with concrete nouns and verbs.

Keep:

- Purpose and reader outcome.
- Required inputs and valid values.
- Observable behavior and outputs.
- Important invariants, edge cases, and error conditions.
- Minimal examples that match current code.

Remove:

- Implementation history unless it is essential migration guidance.
- Meta commentary about authors, agents, reviews, or generation.
- Apologies, caveats, and speculative roadmap language.
- Repeated explanations already covered by nearby docs.
- Marketing adjectives that do not change reader behavior.

### 4. Check Examples And Commands

Examples are documentation promises. Update or delete examples that do not match the current code.

When practical, run the smallest command that validates the edited surface:

- Markdown formatting or linting if the repo has it.
- Targeted unit tests for referenced code.
- Doctests, notebook checks, binding parity checks, or API generation checks when those are the documented surface.
- Build or type checks only when the edited examples or generated docs depend on them.

Prefer focused checks while iterating. Reserve broad checks for broad documentation rewrites or when project rules require them.

### 5. Final Review

Read the final document once as the target reader and once as a skeptic.

Confirm:

- No text refers to the act of writing, editing, generating, reviewing, or implementing unless the document is explicitly a plan or changelog.
- Every code reference was checked.
- Every claim is either verified or removed.
- The document is shorter or clearer than before.
- The result preserves useful warnings and constraints instead of deleting them for neatness.

## Output Style

When reporting back:

- State which docs were cleaned.
- Mention the code references checked.
- List validation commands run and their outcome.
- Call out any references that could not be verified.

Keep the summary brief. The cleaned documentation is the deliverable.

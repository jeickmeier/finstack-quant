---
name: simplicity-auditor
description: Identifies opportunities to simplify a codebase by removing duplication, DRYing repeated logic, converging multiple pathways into one canonical path, and reducing overly complicated public APIs. Use when refactoring for simplicity, deduplicating implementations, auditing API surface area, removing wrappers, or when the user asks to "simplify", "DRY", "dedupe", "reduce API surface", or "make one obvious way".
---

# Simplicity Auditor

## Quick start

When asked to simplify a codebase (or to reduce API surface / DRY / remove duplicates), produce an audit with:

1. **Summary**: what areas are complex and why; expected risk level.
2. **Simplicity principles**: the few rules you will apply (e.g., “one obvious way”, “bias to deletion”).
3. **Redundancy clusters**: groups of duplicates / parallel pathways, each with a canonical target.
4. **API simplifications**: concrete proposals to make APIs smaller and more obvious.
5. **Consolidation plan**: a sequence of small refactors (PR-sized) with migration notes.
6. **Action items**: checklist of the highest-impact changes.

After each cycle, apply the changes and re-audit. Continue until there are no remaining action items.

## Principles (default)

- **One obvious way**: For each capability, there should be one canonical entry point.
- **Bias to deletion**: Prefer removing code to adding abstractions.
- **Private complexity, public simplicity**: Helpers are fine internally; public APIs should be minimal and unsurprising.
- **Wrappers must earn their keep**: Keep wrappers only if they add meaningful semantics (not just renaming/reordering/defaulting).
- **No parallel universes**: Avoid `_v2`, `_ex`, multiple builders, and “try_* everywhere” alternate pathways unless semantics truly differ.
- **Prefer `new` over `try_new` when converging constructors**: When collapsing multiple construction pathways, bias toward a single canonical `new` constructor (even if it returns `Result`), and route parsing/builders into it. Only keep `try_new` if you *also* have a clearly infallible `new` (e.g., takes validated types) and the distinct naming is necessary to prevent ambiguity.
- **Consistency beats flexibility**: Prefer fewer knobs with clear defaults over hyper-configurable APIs.

## What to look for

### Duplication & near-duplication (DRY)

- Copy/paste blocks with small variations (same algorithm repeated with minor parameter differences).
- Repeated validation/parsing/normalization logic across modules.
- Similar types/structs/enums that differ only in naming or a small field set.
- Multiple “helper” functions that do the same thing with slightly different naming.

### Multiple pathways for the same capability

- Parallel APIs: `do_x()`, `do_x_with_config()`, `do_x_ex()`, `do_x_v2()`, `do_x_advanced()`.
- Wrapper-only functions that forward arguments unchanged (or near-unchanged).
- Safety variants that are mostly input validation or error translation.
- Multiple construction pathways (builder + constructor + free function + macro) for the same type.

### Overly complicated public APIs

- Too many configuration structs or nested options for common use cases.
- Boolean flags that materially change semantics (a “mode switch” in disguise).
- Over-exposed generics/trait bounds that complicate downstream usage.
- Many public types that are only used to support internal implementation details.
- Multiple error-handling strategies for the same operation (panic vs `Option` vs `Result`).

## Workflow

1. **Define scope**: identify modules/crates/packages and the intended user-facing “top-level” API.
2. **Inventory public surface**:
   - Rust: `pub` fns/methods/types, re-exports, feature-gated exports.
   - Python/JS bindings: exported symbols and wrappers that shadow Rust behavior.
3. **Group by capability**: map each public entry point to a single capability bucket.
4. **Cluster redundancies**:
   - Create clusters for “same capability / similar semantics”.
   - For each cluster, pick (or propose) a single canonical API.
5. **Decide keep vs merge vs delete**:
   - Merge near-duplicates into internal helpers.
   - Collapse public entry points into the canonical path.
   - Keep only semantically distinct APIs; justify in one sentence.
6. **Plan migration**:
   - Provide before → after call-site examples.
   - If this is a public library: deprecate first, remove later.
7. **Risk & verification**:
   - Identify the top likely behavior breaks (edge cases, error behavior, precision).
   - Add minimal tests (golden/property tests) to lock behavior before deleting pathways.

## Severity rubric

- **Blocker**: Two pathways claim same semantics but produce different results; consolidation would silently change behavior; public API is ambiguous (“which one should I use?”) for core capabilities.
- **Major**: Significant duplication or wrapper layers that materially increase maintenance burden; inconsistent error strategies for the same capability; config explosion for common usage.
- **Minor**: Small redundancies, naming inconsistencies, thin convenience wrappers of low value.
- **Nit**: Style/readability opportunities that don’t meaningfully reduce surface area or duplication.

## Output format (strict)

```markdown
## Summary
<1–3 bullets: what’s complicated, quantitative/behavioral risk level if relevant>

## Simplicity principles applied
- <principle 1>
- <principle 2>

## Surface area inventory
<capability> → <public entry points>

## Redundancy clusters
### Cluster: <capability>
**Canonical API**: `<symbol>` (<path>)

**Redundant pathways**
- `<symbol>` (<path>) — why redundant; recommendation (delete/merge/make-private)

**Migration notes**
- Before: `<callsite>`
- After: `<callsite>`

## Consolidation plan (PR-sized)
- PR 1: add tests / golden cases; internal helpers
- PR 2: collapse public APIs; add deprecations/migrations
- PR 3: remove dead code; docs/examples cleanup

## Scorecard (1–5)
- API simplicity: <n>
- Redundancy level: <n>
- Consistency: <n>
- Maintainability: <n>
- Ergonomics: <n>

## Action items
- [ ] <highest-impact change>
- [ ] <test to add to protect behavior>
```

## Additional resources

- Single-pathway and wrapper-reduction guidance: `.cursor/commands/multi-pathways.md`
- Heuristics and patterns: [reference.md](reference.md)
- Example outputs: [examples.md](examples.md)

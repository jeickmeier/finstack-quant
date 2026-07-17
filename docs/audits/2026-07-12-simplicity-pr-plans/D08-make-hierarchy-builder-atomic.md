# Consolidation Plan: D08 — Make HierarchyBuilder node creation atomic

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12
**User priorities:** complete all five clusters through PR-sized, independently green slices
**Plan date:** 2026-07-12
**Status:** planned
**Suggested branch:** `codex/simplify-d08-make-hierarchy-builder-atomic`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Make HierarchyBuilder node creation atomic

**Tier:** 3 (public surface simplification)
**Estimated net LOC:** −20 to −60
**Addresses:** F29; H11
**Depends on:** None

**Files/filesets:**
- `finstack-quant/core/src/market_data/hierarchy/builder.rs`
- `finstack-quant/core/src/market_data/hierarchy/mod.rs`
- `finstack-quant/core/tests/market_data/hierarchy/**`

**Scope:** Replace current-path temporal state with `add_node(path, curve_ids, tags)` or a node sub-builder that cannot silently discard tags/curve IDs.

**Non-goals:** Do not change hierarchy resolution, path syntax, serialization, or curve targeting.

**Invariants touched:** Hierarchy serde, deterministic node order, and targeting semantics.

## Implementation

1. Add regression tests for the previously silent call-order cases.
2. Choose one atomic node-construction signature and migrate all workspace callers.
3. Remove deferred current-path state and sticky builder errors.
4. Update examples/rustdoc to show only the atomic form.

## Tests to add or update

- Hierarchy builder, resolution, and scenario-targeting tests.

## Verify

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
```

**Bindings/parity/serde impact:** None unless the builder is bound; if discovered, update both hosts and parity.

**Parallel and merge safety:** Safe with most plans; avoid simultaneous scenario hierarchy edits.

**Rollback:** Revert public signature and callers atomically.

## Done when

- No builder method can silently no-op based on call order.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.

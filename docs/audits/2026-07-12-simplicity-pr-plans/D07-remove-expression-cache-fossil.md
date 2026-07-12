# Consolidation Plan: D07 — Move expression-cache compatibility to the wire boundary

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12  
**User priorities:** complete all five clusters through PR-sized, independently green slices  
**Plan date:** 2026-07-12  
**Status:** planned  
**Suggested branch:** `codex/simplify-d07-remove-expression-cache-fossil`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Move expression-cache compatibility to the wire boundary

**Tier:** 4 (serde-sensitive)  
**Estimated net LOC:** −100 to −180  
**Addresses:** F21  
**Depends on:** None

**Files/filesets:**
- `finstack-quant/core/src/expr/eval.rs`
- `finstack-quant/core/src/expr/dag.rs`
- `finstack-quant/core/src/expr/README.md`
- `finstack-quant/core/tests/expr/**`

**Scope:** Remove no-op `with_cache`, `has_cache`, runtime cache budget, plan cache strategy, and unused recommendations; accept legacy cache fields only through private deserialization compatibility.

**Non-goals:** Do not reintroduce caching or change expression evaluation order/results.

**Invariants touched:** Old-payload deserialization and expression outputs.

## Implementation

1. Add legacy-payload fixtures proving old cache fields deserialize and are ignored.
2. Introduce private wire compatibility where needed and stop serializing cache fields.
3. Delete runtime methods/fields, strategy generation, and cache-only tests.
4. Update expression documentation to describe the one current execution model.

## Tests to add or update

- Legacy serde fixture, DAG plan round-trip, expression numerical evaluation tests.

## Verify

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
```

**Bindings/parity/serde impact:** Update stubs/parity only if cache methods were exposed; otherwise Rust-only.

**Parallel and merge safety:** Safe with all planned clusters except work touching expression serde.

**Rollback:** Revert as one serde-compatibility commit.

## Done when

- No public/runtime cache API or emitted cache metadata remains; legacy fixtures still load.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.

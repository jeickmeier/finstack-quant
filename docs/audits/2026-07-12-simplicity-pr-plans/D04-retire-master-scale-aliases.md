# Consolidation Plan: D04 — Retire legacy MasterScale constructor aliases

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12  
**User priorities:** complete all five clusters through PR-sized, independently green slices  
**Plan date:** 2026-07-12  
**Status:** planned  
**Suggested branch:** `codex/simplify-d04-retire-master-scale-aliases`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Retire legacy MasterScale constructor aliases

**Tier:** 3 (public surface simplification)  
**Estimated net LOC:** −20 to −50  
**Addresses:** F32  
**Depends on:** None

**Files/filesets:**
- `finstack-quant/core/src/credit/pd/master_scale.rs`
- `finstack-quant/core/src/credit/registry.rs`
- `finstack-quant/core/data/credit/credit_assumptions.v1.json`
- `finstack-quant/core/src/credit/pd/tests.rs`
- `finstack-quant/core/tests/credit.rs`

**Scope:** Delete `sp_empirical` and `moodys_empirical` constructor aliases while preserving legacy registry input IDs as wire-only aliases to the canonical named scales.

**Non-goals:** Do not change empirical probability tables, rating mappings, or accepted persisted registry IDs.

**Invariants touched:** Credit PD tables and persisted configuration IDs.

## Implementation

1. Add explicit tests showing legacy registry IDs resolve to canonical constructors.
2. Migrate Rust tests/callers to canonical constructor names.
3. Delete legacy methods and stop advertising them in rustdoc.
4. Keep compatibility in registry data/parsing only.

## Tests to add or update

- Master-scale registry alias tests and full core credit tests.

## Verify

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
```

**Bindings/parity/serde impact:** None unless aliases are unexpectedly exported; if found, remove triplets atomically.

**Parallel and merge safety:** Safe with other clusters; avoid concurrent edits to the credit registry.

**Rollback:** Revert constructor and registry mapping changes together.

## Done when

- Legacy names exist only as inbound data aliases; no duplicate runtime constructor remains.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.

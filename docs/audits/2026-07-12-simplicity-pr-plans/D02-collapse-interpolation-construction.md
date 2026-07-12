# Consolidation Plan: D02 — Collapse interpolation to the enum-backed construction path

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12  
**User priorities:** complete all five clusters through PR-sized, independently green slices  
**Plan date:** 2026-07-12  
**Status:** planned  
**Suggested branch:** `codex/simplify-d02-collapse-interpolation-construction`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Collapse interpolation to the enum-backed construction path

**Tier:** 3 (public surface simplification)  
**Estimated net LOC:** −100 to −180  
**Addresses:** F8  
**Depends on:** None

**Files/filesets:**
- `finstack-quant/core/src/math/interp/types.rs`
- `finstack-quant/core/src/math/interp/traits.rs`
- `finstack-quant/core/src/math/interp/generic.rs`
- `finstack-quant/core/src/math/interp/mod.rs`
- `finstack-quant/core/benches/interpolation.rs`
- `finstack-quant/core/tests/math/interp.rs`

**Scope:** Make the static `InterpolationStrategy`/enum factory canonical; remove the boxed `InterpStyle::build -> Box<dyn InterpFn>` pathway and its public dynamic trait if no downstream contract requires it.

**Non-goals:** Do not change interpolation formulas, extrapolation policy, duplicate-x validation, or monotonicity behavior.

**Invariants touched:** Floating-point outputs, extrapolation, monotonicity, and duplicate-x errors.

## Implementation

1. Pin all five interpolation variants against current outputs and extrapolation cases.
2. Redirect the benchmark and remaining tests to the enum-backed factory.
3. Delete the boxed factory, unused dynamic trait surface, and stale re-exports/docs.
4. Search the workspace for `InterpFn` and boxed factory references before merge.

## Tests to add or update

- Core interpolation unit/property tests and the interpolation benchmark compile check.

## Verify

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
```

**Bindings/parity/serde impact:** No direct bindings; public Rust surface changes are intentional.

**Parallel and merge safety:** Safe with A/B/C/E unless another PR edits `core/src/math/mod.rs` or interpolation exports.

**Rollback:** Atomic revert restores the boxed compatibility surface.

## Done when

- Exactly one construction match over interpolation styles remains; no production `Box<dyn InterpFn>` path remains.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.

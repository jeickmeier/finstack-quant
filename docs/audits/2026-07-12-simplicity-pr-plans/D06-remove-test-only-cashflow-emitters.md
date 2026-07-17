# Consolidation Plan: D06 — Remove test-only default and prepayment emitters from the public API

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12
**User priorities:** complete all five clusters through PR-sized, independently green slices
**Plan date:** 2026-07-12
**Status:** planned
**Suggested branch:** `codex/simplify-d06-remove-test-only-cashflow-emitters`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Remove test-only default and prepayment emitters from the public API

**Tier:** 3 (public surface simplification)
**Estimated net LOC:** −60 to −140
**Addresses:** F32
**Depends on:** Before A03, or after A12; do not overlap Cluster A schedule work

**Files/filesets:**
- `finstack-quant/cashflows/src/builder/emission/credit.rs`
- `finstack-quant/cashflows/src/builder/emission/mod.rs`
- `finstack-quant/cashflows/src/builder/mod.rs`
- `finstack-quant/cashflows/tests/cashflows/builder/credit_models.rs`

**Scope:** Move `emit_default_on` and `emit_prepayment_on` behavior into test support or private helpers and remove their public/doc-hidden exports.

**Non-goals:** Do not alter production default/prepayment schedule emission or the genuine inflation/revolving-fee integration adapters.

**Invariants touched:** Default/prepayment balance transitions and event ordering.

## Implementation

1. Reconfirm no production caller uses either function.
2. Move only needed test setup into a test helper.
3. Delete public exports and doc examples that promote the test plumbing.
4. Run stale-reference and public-api checks.

## Tests to add or update

- Cashflows credit-model and schedule tests.

## Verify

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
```

**Bindings/parity/serde impact:** None.

**Parallel and merge safety:** Must be serialized against Cluster A because `builder/mod.rs` and emission files overlap.

**Rollback:** Straight revert.

## Done when

- No doc-hidden test-only emitter remains in the public builder namespace.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.

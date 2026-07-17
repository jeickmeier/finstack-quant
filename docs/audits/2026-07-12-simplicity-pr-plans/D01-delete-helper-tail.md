# Consolidation Plan: D01 — Delete the low-risk helper tail

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12
**User priorities:** complete all five clusters through PR-sized, independently green slices
**Plan date:** 2026-07-12
**Status:** planned
**Suggested branch:** `codex/simplify-d01-delete-helper-tail`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Delete the low-risk helper tail

**Tier:** 1 (delete-only)
**Estimated net LOC:** −60 to −100
**Addresses:** F33
**Depends on:** None

**Files/filesets:**
- `finstack-quant/valuations/src/instruments/fixed_income/tba/pricer.rs`
- `finstack-quant/valuations/src/instruments/rates/cms_option/replication_pricer.rs`
- `finstack-quant/valuations/src/instruments/equity/equity_option/rough_heston_market.rs`
- `finstack-quant/valuations/src/instruments/common_impl/helpers.rs`

**Scope:** Delete `estimate_fail_cost`, the unused CMS replication `compute_pv` wrapper, test-only rough-Heston fallback/default helpers from production scope, and the one-caller Black-Scholes tuple wrapper.

**Non-goals:** Do not alter TBA fail-cost economics, CMS replication formulas, or strict rough-Heston market resolution.

**Invariants touched:** Numerical pricing behavior must remain byte-for-byte unchanged.

## Implementation

1. Reconfirm each symbol has no production caller outside its defining file.
2. Inline the one real helper call where necessary, then delete the wrappers and stale re-exports.
3. Move any genuinely useful test fixture into test support rather than retaining production API.
4. Run a stale-symbol search and keep the diff deletion-dominant.

## Tests to add or update

- Existing valuations unit and integration tests; add no replacement tests for unreachable helpers.

## Verify

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
```

**Bindings/parity/serde impact:** None.

**Parallel and merge safety:** Safe with Clusters A/C/E. Avoid parallel implementation with B01/B14/B17 because `common_impl/helpers.rs` overlaps.

**Rollback:** Straight revert; no public wire or parity changes.

## Done when

- No production definition or reference remains for the four audited helpers, and a targeted simplicity pass reports no wrapper-only helper in these files.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.

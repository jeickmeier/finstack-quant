# Consolidation Plan A05: Unify Balance Replay and Weighted Average Life

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- Audit: 2026-07-12 simplicity audit, Cluster A.
- Findings and hazards: F24.
- Risk tier: Tier 4 — outstanding balances and WAL are numerical outputs.
- Estimated net change: -140 to +40 LOC.
- Dependencies: A04.
- Suggested branch: `codex/a05-unify-balance-replay-wal`.
- Parallel and merge safety: safe beside A01, A06, A08, A10, A11, and A12 after A04. Conflicts with any schedule/DataFrame or structured-credit WAL work.
- Atomicity: tightly coupled seven-file consolidation. It is not a public-type compile-atomic exception, but deleting the legacy helpers and moving all consumers together avoids a temporary second policy.

## Exact Files

- `finstack-quant/cashflows/src/builder/schedule.rs`
- `finstack-quant/cashflows/src/builder/dataframe.rs`
- `finstack-quant/cashflows/README.md`
- `finstack-quant/cashflows/tests/cashflows/builder/schedule.rs`
- `finstack-quant/cashflows/tests/cashflows/builder/amortization.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/structured_credit/metrics/pricing/wal.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/structured_credit/types/pool.rs`

## Scope

- Define one internal chronological balance-replay primitive with the canonical principal-event sign and kind policy.
- Make `outstanding_by_date` and DataFrame drawn/undrawn columns consume that primitive.
- Delete `outstanding_path_per_flow`, its simplified event policy, its tests, and README references.
- Define one reusable WAL calculation over dated principal reductions and make both `CashFlowSchedule::weighted_average_life` and structured-credit callers delegate to it.
- Preserve existing public schedule methods that remain semantically distinct; remove only duplicate implementations.

## Non-Goals

- No cashflow kind reordering; A02 owns ordering.
- No per-flow accrual/rate metadata changes; A03/A04 own those.
- No change to pool collateral generation or waterfall allocation.
- No new balance cache or precomputed balance state.

## Implementation Steps

1. Extract the current complete `outstanding_by_date` event semantics into a private streaming replay helper.
2. Delegate schedule balance views and DataFrame balance columns to that helper and delete local replay loops.
3. Remove `outstanding_path_per_flow` and convert any valid caller to the canonical balance view.
4. Extract a currency-checked `weighted_average_life_from_principal` helper with one date-weighting and zero-balance policy.
5. Delegate schedule and structured-credit/pool WAL paths to it; delete local numerator/denominator loops.
6. Update README and regression tests to advertise only the surviving APIs.

## Tests to Add or Update

- One mixed principal sequence covering initial notional, draw, repayment, amortization, prepayment, PIK, and defaulted notional.
- Assert schedule balance view and DataFrame columns are identical at each date.
- WAL parity test using identical dated reductions through schedule and structured-credit entry points.
- Currency mismatch, no-principal, fully repaid, and same-date event cases.

## Full Verification

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
rtk mise run goldens-test
```

## Binding, Parity, and Serde Impact

- Python/WASM: no symbol or JSON shape change.
- Parity contract: no change.
- Serde/schema: no change.
- Numerical impact: duplicate paths must converge; any changed golden requires an identified prior policy mismatch.

## Rollback

Revert the slice as one commit. No serialized state or migration is introduced.

## Done Criteria

- `outstanding_path_per_flow` is absent from code, tests, and README.
- Exactly one implementation interprets principal event kinds into balances.
- Exactly one implementation computes WAL date weights and normalization.
- DataFrame, schedule, and structured-credit regression fixtures agree.

## Targeted Re-Audit Acceptance

Run `finstack-simplify` for `outstanding`, `balance`, and `weighted_average_life` across cashflows and valuations. Accept only when it finds one balance replay policy, one WAL kernel, thin delegates, and no copied kind/sign/date loops.

# Consolidation Plan A06: Migrate Valuations to build_periods

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- Audit: 2026-07-12 simplicity audit, Cluster A.
- Findings and hazards: F14, production-caller migration.
- Risk tier: Tier 2 — internal behavior-preserving call-site consolidation.
- Estimated net change: -40 to +80 LOC.
- Dependencies: none.
- Suggested branch: `codex/a06-valuations-build-periods`.
- Parallel and merge safety: safe beside A01-A05 and A08-A12. Must merge before A07. Conflicts only with active work in the eight listed pricing/calibration files.
- Atomicity: eight-file mechanical caller batch; no public compile-atomic exception because `build_dates` remains available until A07.

## Exact Files

- `finstack-quant/valuations/src/calibration/targets/forward.rs`
- `finstack-quant/valuations/src/calibration/targets/swaption.rs`
- `finstack-quant/valuations/src/instruments/rates/exotics_shared/forward_swap_rate.rs`
- `finstack-quant/valuations/src/instruments/rates/irs/metrics/par_rate.rs`
- `finstack-quant/valuations/src/instruments/rates/swaption/hw_pricer.rs`
- `finstack-quant/valuations/src/instruments/rates/swaption/types/definitions.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/inflation_linked_bond/types.rs`
- `finstack-quant/valuations/src/instruments/credit_derivatives/cds_tranche/pricer/sensitivities.rs`

## Scope

- Replace every production `build_dates(...)` call in valuations with `build_periods(BuildPeriodsParams { ... })`.
- Consume explicit accrual start/end and payment dates from the canonical period result instead of rebuilding adjacent date pairs.
- Preserve stub, end-of-month, calendar, business-day adjustment, payment-lag, and adjusted-accrual behavior exactly.
- Remove production imports and helper adaptations that exist only for `PeriodSchedule`/`SchedulePeriod`.

## Non-Goals

- Do not remove or change the `build_dates` API; A07 owns that deletion.
- Do not refactor pricing formulas or calibration objective construction.
- Do not alter coupon spec representation; A08 owns it.
- Do not broaden the migration to tests/support in this slice.

## Implementation Steps

1. Translate each caller's arguments into `BuildPeriodsParams` without changing convention values.
2. Replace adjacent-date or `unadjusted` reconstruction with the canonical period fields.
3. Preserve callers that need adjusted versus unadjusted dates by selecting the corresponding explicit field, not by applying a second adjustment.
4. Remove obsolete imports and local adapters.
5. Add inline assertions or focused unit cases where a caller previously depended on a stub/payment-lag edge.

## Tests to Add or Update

- Calibration target cases with front/back stub and end-of-month schedules.
- Swaption/IRS cases with adjusted accrual boundaries and payment lag.
- Inflation-linked bond and CDS-tranche cases that pin contractual versus payment dates.
- Existing golden/pricing tests must remain unchanged.

## Full Verification

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
rtk mise run goldens-test
```

## Binding, Parity, and Serde Impact

- Python/WASM: none.
- Parity contract: no change.
- Serde/schema: no change.
- Numerical impact: none intended; zero unexplained golden diff is required.

## Rollback

Revert the call-site migration. `build_dates` remains available until A07, so rollback is self-contained.

## Done Criteria

- No production file under `valuations/src` calls or imports `build_dates`.
- No migrated caller reconstructs periods with `windows(2)` when canonical periods already exist.
- All pricing/calibration goldens are unchanged.

## Targeted Re-Audit Acceptance

Run `finstack-simplify` over valuations date-generation call sites. Accept only when production callers use `build_periods`, explicit period fields carry date semantics, and there is no local adapter recreating the old `PeriodSchedule` API.

# Consolidation Plan A04: Remove Schedule Metadata Sidecars

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- Audit: 2026-07-12 simplicity audit, Cluster A.
- Findings and hazards: F12; H2, H3, H6.
- Risk tier: Tier 4 — accrual, projection decomposition, schedule serde, and term-loan cashflows.
- Estimated net change: -80 to +180 LOC.
- Dependencies: A03.
- Suggested branch: `codex/a04-remove-schedule-sidecars`.
- Parallel and merge safety: safe beside A01, A05, A06, and A10 after A03 merges. Conflicts with A02 in schedule sorting and with A08/A09 in coupon emission/serde; merge A02 first and A04 before A08/A09.
- Atomicity: seven-file semantic cutover exception. Producers, consumers, legacy deserialization, and term-loan merging must switch together; a partial merge would create two authoritative metadata paths.

## Exact Files

- `finstack-quant/cashflows/src/builder/schedule.rs`
- `finstack-quant/cashflows/src/builder/emission/coupons.rs`
- `finstack-quant/cashflows/src/builder/rate_helpers.rs`
- `finstack-quant/cashflows/src/builder/dataframe.rs`
- `finstack-quant/cashflows/src/accrual.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/term_loan/cashflows.rs`
- `finstack-quant/valuations/tests/instruments/term_loan/cashflows.rs`

## Scope

- Populate `CashFlow.accrual` at fixed, floating, stub, PIK, and applicable fee emission sites.
- Store the actual projected index rate at projection time; retain `CashFlow.rate` as the all-in rate.
- Remove `CashFlowMeta.accrual_periods` and `accrual_day_counts` plus all alignment, sorting, filtering, merging, and repair choreography.
- Add a private legacy schedule deserializer that accepts old sidecar arrays, zips them onto flows once, validates their lengths, and serializes only the canonical per-flow form.
- Group accrued-interest buckets by full accrual identity rather than payment date alone so same-date legs do not erase conflicting day-count or period data.
- Make term-loan fee/coupon schedule composition use canonical merge helpers and preserve each flow's metadata.
- Make DataFrame base-rate/spread columns use stored projected index rate; do not reconstruct a fixed-tenor rate from reset/payment dates.

## Non-Goals

- No balance replay or WAL consolidation; that is A05.
- No coupon-spec shape changes; that is A08.
- No removal of legacy input compatibility in schema version 1.
- No recomputation of missing historical index-rate metadata; missing values remain `None`.

## Implementation Steps

1. Populate canonical per-flow accrual metadata in coupon projection/emission, including projected index rate before caps/spread are combined.
2. Replace schedule sort/filter/merge records with direct operations on self-contained flows and delete both sidecar vectors from `CashFlowMeta`.
3. Implement a private v1 wire adapter that translates legacy arrays into the new field and rejects nonempty misaligned arrays instead of replacing them with `None`.
4. Change accrual bucketing to preserve separate same-date accrual identities and sum their independently computed accrued amounts.
5. Replace DataFrame forward-curve reconstruction with stored `index_rate`; derive spread only as `all_in_rate - index_rate`.
6. Build the term-loan fee leg as a schedule and merge canonically instead of extending/sorting raw flows.

## Tests to Add or Update

- Legacy sidecar JSON translates losslessly; mismatched nonempty sidecars fail validation.
- Sorting, retaining, and merging schedules preserve metadata without alignment logic.
- Same payment date with different accrual periods/day counts produces two correct accrual contributions.
- Term-loan fees do not erase coupon metadata.
- DataFrame index rate equals the projection rate for nonstandard index tenor and differs from the previously reconstructed fixed-tenor value where expected.

## Full Verification

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
rtk mise run python-build -- --release
rtk mise run python-lint
rtk mise run python-test
rtk mise run wasm-build
rtk mise run wasm-lint
rtk mise run wasm-test
rtk env UV_CACHE_DIR=/private/tmp/finstack-uv-cache uv run pytest finstack-quant-py/tests/parity -x
rtk mise run gen-check
rtk mise run rust-check-schemas
rtk mise run goldens-test
```

## Binding, Parity, and Serde Impact

- Python/WASM callable surfaces do not change; raw schedule JSON moves from metadata sidecars to optional per-flow accrual metadata.
- TypeScript generated schedule types must match A03's canonical nested field.
- Parity topology does not change.
- Serde accepts legacy v1 input but emits only the canonical representation; no dual public structs or fields remain.

## Rollback

Revert A04 as one unit. The A03 optional field can remain unused, but removing A03 requires reverting A04 first.

## Done Criteria

- `CashFlowMeta` has no per-flow vectors.
- No sort/filter/merge function has metadata-length or index-alignment branches.
- No DataFrame path calls a curve to reconstruct an already projected flow's base rate.
- Same-date accrual conflicts are preserved or explicitly rejected, never silently converted to `None`.
- Term-loan schedule composition preserves all flow metadata.

## Targeted Re-Audit Acceptance

Run `finstack-simplify` on schedule, accrual, DataFrame, coupon emission, and term-loan cashflows. Accept only when each flow owns its metadata, legacy arrays exist only in a private deserialization adapter, same-date grouping is metadata-safe, and there is no second rate-reconstruction policy.

# Consolidation Plan A08: Make ScheduleParams Canonical Across Coupon Specs

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- Audit: 2026-07-12 simplicity audit, Cluster A.
- Findings and hazards: F13.
- Risk tier: Tier 4 — public Rust structs, serde/schema, and accrual-date convention propagation.
- Estimated net change: -150 to +100 LOC.
- Dependencies: A01 and A07.
- Suggested branch: `codex/a08-canonical-schedule-params`.
- Parallel and merge safety: safe beside A02, A05, A10, A11, and A12 after dependencies. High conflict risk with A09 and any bond/floating-spec construction work; merge A08 before A09.
- Atomicity: **compile-atomic exception**. Removing duplicated public struct fields requires every direct `FixedCouponSpec`, `FloatingCouponSpec`, `FloatingRateSpec`, and `StepUpCouponSpec` literal to migrate in one commit. `serde(flatten)` preserves the existing JSON shape, so no temporary duplicate fields or accessor API should be introduced.

## Exact Files and Filesets

- `finstack-quant/cashflows/src/builder/specs/schedule.rs`
- `finstack-quant/cashflows/src/builder/specs/coupon.rs`
- `finstack-quant/cashflows/src/builder/coupon_api.rs`
- `finstack-quant/cashflows/src/builder/compiler.rs`
- `finstack-quant/cashflows/src/builder/rate_helpers.rs`
- `finstack-quant/cashflows/src/builder/emission/coupons.rs`
- `finstack-quant/cashflows/src/json.rs`
- Every direct literal/construction file in the exact inventories:

```bash
rtk rg -l '(FixedCouponSpec|FloatingCouponSpec|FloatingRateSpec|StepUpCouponSpec) \{' finstack-quant finstack-quant-py finstack-quant-wasm
rtk rg -l '\.(freq|dc|bdc|calendar_id|stub|end_of_month|payment_lag_days|adjust_accrual_dates)' finstack-quant/cashflows/src/builder finstack-quant/valuations/src
```

- `finstack-quant/cashflows/tests/cashflows/examples/fixed_coupon_spec.example.json`
- `finstack-quant/cashflows/tests/cashflows/examples/floating_coupon_spec.example.json`
- `finstack-quant/cashflows/tests/cashflows/schema_roundtrip.rs`

## Scope

- Store `#[serde(flatten)] pub schedule: ScheduleParams` in fixed, floating, and step-up coupon specs.
- Keep `FloatingRateSpec` limited to index/rate mechanics; remove schedule-owned day count, BDC, calendar, EOM, and payment-lag fields from it.
- Delete `from_parts`, `schedule_params`, and `schedule_from_floating_spec` copy/repack helpers.
- Preserve the existing flat JSON keys through `serde(flatten)` while allowing `adjust_accrual_dates=true` to survive every Rust and JSON path.
- Update all direct construction and field access to use the canonical nested Rust value.

## Non-Goals

- No JSON coupon-program expansion; A09 owns it.
- No date-generation API work; A06/A07 own it.
- No legacy duplicated Rust fields or deprecated getters.
- No semantic change to preset convention values.

## Implementation Steps

1. Embed flattened `ScheduleParams` in the three coupon specs and strip schedule concerns from `FloatingRateSpec`.
2. Replace compiler/builder conversions with direct borrowing/cloning of the canonical schedule value.
3. Migrate every literal and field access in the captured inventory, grouped mechanically but committed atomically.
4. Verify fixed, floating, step-up, fixed-to-float, and explicit-window paths retain `adjust_accrual_dates` rather than resetting it to false.
5. Regenerate schemas/types and update serde examples without changing their flat external key layout.
6. Delete every old copy/repack helper and duplicated field reference.

## Tests to Add or Update

- Round-trip each coupon spec with default and `adjust_accrual_dates=true` schedules.
- Assert flat legacy JSON still deserializes and reserializes with the same schedule keys.
- Builder equivalence tests for fixed, floating, step-up, and fixed-to-float specs before/after consolidation.
- Schema tests asserting one definition/source for schedule fields.

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

- Python/WASM callable signatures remain JSON-string based and unchanged.
- Parity topology remains unchanged.
- Rust construction changes intentionally.
- JSON remains flat and backward compatible through `serde(flatten)`; schema descriptions/references are regenerated from one canonical `ScheduleParams`.

## Rollback

Revert the full compile-atomic commit. No mixed old/new spec shape may be cherry-picked.

## Done Criteria

- Fixed, floating, and step-up specs each contain one `ScheduleParams` value.
- `FloatingRateSpec` contains no schedule-generation fields.
- No copy/repack helper reconstructs `ScheduleParams` field by field.
- `adjust_accrual_dates=true` survives all builder and serde paths.
- External flat JSON examples remain accepted.

## Targeted Re-Audit Acceptance

Run `finstack-simplify` across coupon specs, builder/compiler, and valuation construction sites. Accept only when schedule conventions have one owner, no field-by-field repacking remains, no compatibility struct duplicates the old layout, and the generated schema references one schedule definition.

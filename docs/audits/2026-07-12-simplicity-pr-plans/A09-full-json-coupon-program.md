# Consolidation Plan A09: Give JSON the Full Canonical Coupon Program

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- Audit: 2026-07-12 simplicity audit, Cluster A.
- Findings and hazards: F15.
- Risk tier: Tier 4 — public JSON/schema contract and Python/WASM behavior.
- Estimated net change: +180 to +280 LOC.
- Dependencies: A01 and A08.
- Suggested branch: `codex/a09-full-json-coupon-program`.
- Parallel and merge safety: safe beside A02, A05, A06/A07, A10, and A12 after dependencies. Conflicts with A11 in every JSON binding test and with coupon-builder work; merge A09 before A11.
- Atomicity: cross-surface wire-contract slice. It exceeds five files because Rust, Python, WASM, schema fixtures, and parity notes must remain independently green; no second public build-spec type is allowed.

## Exact Files

- `finstack-quant/cashflows/src/json.rs`
- `finstack-quant/cashflows/src/lib.rs`
- `finstack-quant/cashflows/tests/cashflows/schema_roundtrip.rs`
- `finstack-quant-py/src/bindings/cashflows/mod.rs`
- `finstack-quant-py/finstack_quant/cashflows/__init__.pyi`
- `finstack-quant-py/tests/test_cashflows.py`
- `finstack-quant-py/parity_contract.toml`
- `finstack-quant-wasm/src/api/cashflows/mod.rs`
- `finstack-quant-wasm/types/generated/CashflowSchedule.ts`
- `finstack-quant-wasm/tests/wasm_cashflows.rs`
- `finstack-quant-wasm/tests/facade/cashflows.test.mjs`

## Scope

- Replace the narrow `fixed_coupons`/`floating_coupons` build-spec model with one canonical serialized coupon-program enum that maps one-to-one to supported builder instructions.
- Cover fixed, floating, step-up, fixed-to-float, explicit fixed/floating windows, fixed margin/rate step programs, and payment-split windows/programs.
- Keep fees, notional/amortization, and principal events in the same build spec.
- Accept the old fixed/floating array form only through a private deserialization adapter; serialize and generate schema for the canonical form only.
- Route every variant into the existing Rust builder methods rather than duplicating compilation logic in `json.rs`.

## Non-Goals

- No typed Python classes or WASM objects; bindings remain JSON-string bridges.
- No second `CashflowScheduleBuildSpecV2` public type.
- No envelope deletion; A11 owns wire-format consolidation.
- No bond namespace move; A12 owns it.

## Implementation Steps

1. Define a serde/schemars `CouponLegSpec` and, if needed, a small `PaymentProgramSpec` whose variants exactly cover the public builder instructions.
2. Replace public narrow vectors in `CashflowScheduleBuildSpec` with canonical program vectors.
3. Implement private legacy input structs that translate old fixed/floating arrays once; never expose or serialize those structs.
4. Dispatch each canonical variant to the existing builder and rely on A01's deferred horizon semantics.
5. Update Rust/Python/WASM fixtures to exercise at least one step-up, one fixed-to-float, one explicit window, and one payment split.
6. Regenerate schema/type artifacts and update parity notes to describe the full program rather than a narrow first slice.

## Tests to Add or Update

- Legacy fixed/floating JSON input builds the same schedule as before.
- Canonical JSON covers every enum variant and round-trips without legacy fields.
- Equivalent Rust-builder and JSON-builder programs produce identical schedules.
- Python and WASM smoke tests exercise nontrivial step-up/fixed-to-float/payment-split programs.
- Unknown/overlapping/invalid windows return the same Rust validation error through all surfaces.

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

- Python/WASM functions keep the same names and string signatures but accept the richer canonical input.
- Parity contract notes and generated JSON types change; symbol topology does not.
- Serde accepts legacy narrow input privately and emits only the canonical coupon program.
- Schema change is intentional and must be generated from Rust, not hand-maintained.

## Rollback

Revert the complete cross-surface slice. Legacy input remains available only while the new canonical build spec exists; do not retain the adapter alone.

## Done Criteria

- Every supported Rust coupon/payment-program operation has one JSON representation.
- `CashflowScheduleBuildSpec` has no public `fixed_coupons` or `floating_coupons` parallel fields.
- JSON dispatch calls the canonical builder rather than implementing schedule logic.
- Python/WASM tests prove at least one operation previously unavailable through bindings.

## Targeted Re-Audit Acceptance

Run `finstack-simplify` on `cashflows/src/json.rs`, builder methods, binding registrations, generated types, and parity contract. Accept only when one build-spec model covers the Rust builder, legacy input translation is private and one-way, and no binding-owned financial or schedule logic exists.

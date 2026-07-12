# Consolidation Plan A03: Move Accrual Metadata onto CashFlow

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- Audit: 2026-07-12 simplicity audit, Cluster A.
- Findings and hazards: F12 groundwork; H6 groundwork.
- Risk tier: Tier 4 — public core type, serde contract, and downstream numerical metadata.
- Estimated net change: +80 to +180 LOC.
- Dependencies: none.
- Suggested branch: `codex/a03-cashflow-owned-metadata`.
- Parallel and merge safety: safe beside A01, A02, A05, A06, and A10. Merge immediately before A04. High conflict risk with any branch adding `CashFlow` fields or literals.
- Atomicity: **compile-atomic exception**. Adding a field to the public `CashFlow` struct requires every workspace struct literal to migrate in the same commit. Splitting the literal migration would require a temporary duplicate constructor/state API and would leave intermediate commits uncompilable.

## Exact Files and Filesets

- `finstack-quant/core/src/cashflow/primitives.rs`
- `finstack-quant/core/tests/cashflow/primitives.rs`
- Every Rust file in the following exact inventory that contains a `CashFlow { ... }` literal at the start of the slice:

```bash
rtk rg -l 'CashFlow \{' finstack-quant/core finstack-quant/cashflows finstack-quant/margin finstack-quant/statements finstack-quant/valuations
```

- The inventory includes builder/emission, accrual, aggregation, DataFrame, margin repo, statement waterfall, credit/rates/fixed-income/equity valuation, structured-credit, tests, and benchmark literal sites; no file without a literal is in scope.
- `finstack-quant-wasm/types/generated/CashflowSchedule.ts`

## Scope

- Add one optional, copyable `CashFlowAccrual` value owned by each `CashFlow`, containing contractual accrual start/end, day-count convention, and optional projected index rate.
- Add a canonical `CashFlow::new(...)` constructor that defaults the optional accrual value to `None` and a single fluent/setter path for attaching it.
- Replace all workspace struct literals with the constructor so future optional metadata does not trigger another workspace-wide literal rewrite.
- Keep old serialized schedules byte-shape compatible when accrual metadata is absent by using `default` plus `skip_serializing_if`.
- Leave existing schedule sidecars operational until A04 performs the semantic cutover.

## Non-Goals

- Do not populate the new metadata in production yet.
- Do not remove `CashFlowMeta.accrual_periods` or `accrual_day_counts`; that is A04.
- Do not change accrual grouping, DataFrame reconstruction, or term-loan merging yet.
- Do not introduce a second wrapper such as `ScheduledCashFlow`.

## Implementation Steps

1. Define `CashFlowAccrual` beside `CashFlow` with serde/schemars defaults and exact field documentation.
2. Add the optional `accrual` field and canonical constructor/attachment method while retaining `Copy` where possible.
3. Mechanically migrate every literal in the captured fileset to the constructor; do not alter amounts, kinds, dates, rates, or factors.
4. Update core round-trip tests for missing and present optional metadata.
5. Update the generated TypeScript schedule shape to expose the optional nested metadata using Rust's actual enum/date spelling.

## Tests to Add or Update

- Core constructor equality test against the former literal shape.
- Deserialize a pre-change schedule without `accrual` and reserialize without adding the field.
- Round-trip a flow with accrual dates, day count, and projected index rate.
- Existing crate tests serve as mechanical migration coverage for all literal sites.

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

- Python: JSON schedules accept the optional field automatically; no callable signature changes.
- WASM: generated `CashFlowJson` type gains one optional nested field; callable signatures remain unchanged.
- Parity contract: no symbol change.
- Serde: old payloads remain accepted and canonical old flows omit the new field; present metadata is strictly typed.

## Rollback

Revert A03 before A04. Once A04 lands, rollback must revert A04 first because it removes the legacy sidecars.

## Done Criteria

- `CashFlow` has exactly one optional accrual metadata value.
- All workspace `CashFlow` construction uses the canonical constructor; the literal inventory is empty outside `primitives.rs` tests explicitly exercising serde/type construction.
- Old schedule JSON round-trips without a shape change.
- No production code reads the new field before A04.

## Targeted Re-Audit Acceptance

Run `finstack-simplify` on core cashflow primitives and the workspace construction inventory. Accept only when there is one constructor, one optional accrual metadata type, no `ScheduledCashFlow` wrapper, and no second set of newly introduced per-flow fields.

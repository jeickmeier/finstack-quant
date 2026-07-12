# Consolidation Plan A07: Remove the Parallel build_dates API

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- Audit: 2026-07-12 simplicity audit, Cluster A.
- Findings and hazards: F14, final API consolidation.
- Risk tier: Tier 3 — public Rust API deletion.
- Estimated net change: -120 to -40 LOC.
- Dependencies: A06.
- Suggested branch: `codex/a07-remove-build-dates`.
- Parallel and merge safety: safe beside A01-A05 and A08-A12 after A06. Conflicts with date-generation tests/support and any new `build_dates` caller.
- Atomicity: **compile-atomic exception**. The public re-export, internal bridge, and every remaining workspace test/support caller must move together or the workspace will not compile.

## Exact Files

- `finstack-quant/cashflows/src/builder/date_generation.rs`
- `finstack-quant/cashflows/src/builder/periods.rs`
- `finstack-quant/cashflows/src/builder/compiler.rs`
- `finstack-quant/cashflows/src/builder/mod.rs`
- `finstack-quant/cashflows/tests/cashflows/builder/conventions.rs`
- `finstack-quant/cashflows/tests/cashflows/builder/schedule.rs`
- `finstack-quant/valuations/tests/cashflows/bridge_smoke.rs`
- `finstack-quant/valuations/tests/support/cashflow_emission.rs`
- `finstack-quant/valuations/tests/instruments/cap_floor/cashflows.rs`
- `finstack-quant/valuations/tests/instruments/irs/cashflows.rs`

## Scope

- Move the low-level date-generation algorithm behind `build_periods` as a private helper.
- Migrate all remaining test/support callers to `build_periods`.
- Remove the public `build_dates` function and root re-export.
- Remove `PeriodSchedule`/`SchedulePeriod` if no independent public role remains after migration.
- Make `build_periods` the only supported schedule-construction API.

## Non-Goals

- No convention or algorithm changes.
- No renaming of `build_periods`.
- No new compatibility alias, deprecated wrapper, or facade.
- No valuations production changes beyond A06.

## Implementation Steps

1. Convert `periods.rs` to call a private low-level generator rather than the public `build_dates` wrapper.
2. Migrate every listed test/support caller and its expected-field access.
3. Delete the public wrapper, obsolete types, docs, examples, and re-export.
4. Remove imports and dead conversion helpers in compiler/tests.
5. Run an inventory asserting `build_dates` remains only in historical audit/plan text, not code or API docs.

## Tests to Add or Update

- Preserve direct convention coverage through `build_periods` for unknown calendars, stubs, EOM, BDC, payment lag, and adjusted accruals.
- Preserve valuations bridge-smoke coverage against the canonical API.
- Keep cap/floor and IRS expected schedule tests unchanged except for result access syntax.

## Full Verification

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
rtk mise run gen-check
rtk mise run rust-check-schemas
rtk mise run goldens-test
```

## Binding, Parity, and Serde Impact

- Python/WASM: none; `build_dates` is not part of their public binding surface.
- Parity contract: no change.
- Serde/schema: no change.
- Rust API: intentional deletion with no deprecated alias.

## Rollback

Revert A07; A06 callers can continue using `build_periods`, so no A06 rollback is required.

## Done Criteria

- `build_dates` and its root re-export are absent.
- No workspace Rust source/test/support file calls it.
- `build_periods` owns the only public schedule-generation contract.
- No conversion wrapper recreates `PeriodSchedule` under another name.

## Targeted Re-Audit Acceptance

Run `finstack-simplify` on the builder date-generation modules and workspace call sites. Accept only when one public period API remains, the low-level generator is private, and there is no compatibility alias or duplicate result type.

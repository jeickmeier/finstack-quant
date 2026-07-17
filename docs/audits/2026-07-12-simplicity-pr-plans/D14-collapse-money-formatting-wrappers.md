# Consolidation Plan: D14 — Collapse Money formatting wrappers into FormatOpts

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12
**User priorities:** complete all five clusters through PR-sized, independently green slices
**Plan date:** 2026-07-12
**Status:** planned
**Suggested branch:** `codex/simplify-d14-collapse-money-formatting-wrappers`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Collapse Money formatting wrappers into FormatOpts

**Tier:** 3 (public surface simplification)
**Estimated net LOC:** −40 to −80
**Addresses:** F33
**Depends on:** D12

**Files/filesets:**
- `finstack-quant/core/src/money/types.rs`
- `finstack-quant/core/src/money/format.rs`
- `finstack-quant-py/src/bindings/core/money.rs`
- `finstack-quant-py/finstack_quant/core/money.pyi`
- `finstack-quant/core/tests/money/rounding.rs`

**Scope:** Retain `Display` and `Money::format_with(FormatOpts)`; delete compact/accounting/symbol-placement wrappers and have bindings construct options explicitly.

**Non-goals:** Do not change separators, negative/accounting layout, symbol placement, rounding, or locale-independent output.

**Invariants touched:** Exact formatting strings, rounding and currency placement.

## Implementation

1. Snapshot every wrapper output as `format_with` equivalence tests.
2. Migrate the Python method(s) to construct the corresponding `FormatOpts`.
3. Delete wrapper methods and update stubs/docs.
4. Search for stale wrapper names across examples and notebooks.

## Tests to add or update

- Money formatting/rounding goldens and Python formatting tests.

## Verify

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
rtk uv run pytest finstack-quant-py/tests/parity -x
```

**Bindings/parity/serde impact:** Python touched; verify WASM has no shadow methods and update parity if necessary.

**Parallel and merge safety:** Run after D12; otherwise safe.

**Rollback:** Revert wrapper deletion and stubs together.

## Done when

- One configurable formatting method plus `Display` remains.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.

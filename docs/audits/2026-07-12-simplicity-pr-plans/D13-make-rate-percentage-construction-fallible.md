# Consolidation Plan: D13 — Make Rate and Percentage construction uniformly fallible

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12
**User priorities:** complete all five clusters through PR-sized, independently green slices
**Plan date:** 2026-07-12
**Status:** planned
**Suggested branch:** `codex/simplify-d13-make-rate-percentage-construction-fallible`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Make Rate and Percentage construction uniformly fallible

**Tier:** 4 (numeric/public API-sensitive)
**Estimated net LOC:** −30 to −90 net; high mechanical churn
**Addresses:** F22
**Depends on:** D12 recommended

**Files/filesets:**
- `finstack-quant/core/src/types/rates.rs`
- `finstack-quant/core/src/types/mod.rs`
- `finstack-quant-py/src/bindings/core/types.rs`
- `finstack-quant-wasm/src/api/core/types.rs`
- `Workspace call sites of`Rate::from_decimal`,`Rate::from(f64)`,`Percentage::new`, and`Percentage::from(f64)``

**Scope:** Keep one checked constructor per type, delete panicking `From<f64>` and constructor shadows, and use explicit private `_unchecked` construction only for trusted constants.

**Non-goals:** Do not change rate units, basis-point semantics, percentage scaling, arithmetic, or serde labels.

**Invariants touched:** Rate units, finite-value validation, Decimal conversions, serde/parity.

## Implementation

1. Pin finite, NaN, infinity, negative, and unit-conversion behavior.
2. Select canonical checked constructors and migrate bindings first.
3. Mechanically migrate workspace call sites with explicit error propagation or trusted private helpers.
4. Delete panicking conversions and stale docs.
5. Run rate/percentage and financial model goldens.

## Tests to add or update

- Core rate/percentage tests; cashflow rate specs; valuation rate-boundary tests; binding error parity.

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

**Bindings/parity/serde impact:** Python and WASM both touched; full stack required.

**Parallel and merge safety:** Avoid C03-C07 and A08-A09 because they edit rate construction call sites.

**Rollback:** Atomic full-stack revert.

## Done when

- No public f64 conversion can panic and no `new`/`try_new` shadow pair remains.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.

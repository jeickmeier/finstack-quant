# Consolidation Plan: D11 — Replace zero-state calendar wrappers with free resolution APIs

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12  
**User priorities:** complete all five clusters through PR-sized, independently green slices  
**Plan date:** 2026-07-12  
**Status:** planned  
**Suggested branch:** `codex/simplify-d11-remove-calendar-registry-wrapper`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Replace zero-state calendar wrappers with free resolution APIs

**Tier:** 4 (calendar/serde-sensitive)  
**Estimated net LOC:** −100 to −250 net; high mechanical churn  
**Addresses:** F10; H9  
**Depends on:** A07 recommended before cashflow schedule consolidation

**Files/filesets:**
- `finstack-quant/core/src/dates/calendar/{registry.rs,mod.rs}`
- `finstack-quant/core/src/dates/{daycount.rs,fx.rs,mod.rs}`
- `Core/cashflows/valuations callers of `CalendarRegistry` and `CalendarWrapper``
- `Python/WASM core dates and analytics performance bindings`
- `Calendar/day-count docs, tests, stubs, and parity entries`

**Scope:** Expose `calendar_by_id`, `available_calendars`, and a strict multi-ID resolver; use one static weekends-only calendar; remove `CalendarRegistry`, its lifetime plumbing, `CalendarWrapper`, and silent unknown-ID dropping.

**Non-goals:** Do not alter holiday data, composite-calendar rules, weekends-only opt-in behavior, BDCs, or day-count formulas.

**Invariants touched:** ISDA day counts, holiday resolution, composite semantics, serde IDs, host parity.

## Implementation

1. Add regression tests for unknown IDs, composite order, weekends-only, and day-count state hydration.
2. Introduce strict free functions and migrate core day-count/FX consumers.
3. Migrate cashflows, valuations, Python, WASM, and analytics consumers.
4. Delete registry/wrapper types, lifetimes, re-exports, and stale documentation.
5. Run ISDA/calendar vectors and parity.

## Tests to add or update

- Core calendar/day-count/FX tests; cashflow schedule conventions; affected valuations calendar tests; Python/WASM dates tests.

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

**Bindings/parity/serde impact:** Python and WASM both touched. Compile-atomic large-PR exception: removing the lifetime-bearing type must migrate all callers together.

**Parallel and merge safety:** Serialize with A06-A10 and B18 because they touch schedule/calendar semantics.

**Rollback:** Atomic full-stack revert.

## Done when

- No zero-state registry/wrapper remains; all unknown calendar IDs fail explicitly.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.

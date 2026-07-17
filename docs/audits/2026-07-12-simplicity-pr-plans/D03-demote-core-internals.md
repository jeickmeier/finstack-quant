# Consolidation Plan: D03 — Demote FX bump and arbitrage strategy implementation types

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12
**User priorities:** complete all five clusters through PR-sized, independently green slices
**Plan date:** 2026-07-12
**Status:** planned
**Suggested branch:** `codex/simplify-d03-demote-core-internals`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Demote FX bump and arbitrage strategy implementation types

**Tier:** 3 (public surface simplification)
**Estimated net LOC:** −40 to −100
**Addresses:** F32
**Depends on:** None

**Files/filesets:**
- `finstack-quant/core/src/money/fx/providers.rs`
- `finstack-quant/core/src/money/fx/matrix.rs`
- `finstack-quant/core/src/money/fx/mod.rs`
- `finstack-quant/core/src/money/README.md`
- `finstack-quant/core/src/market_data/arbitrage/mod.rs`
- `finstack-quant/core/src/market_data/arbitrage/checks/mod.rs`

**Scope:** Make `BumpedFxProvider` and concrete arbitrage check strategies crate-private while retaining `FxMatrix::with_bumped_rate`, configs, reports, and one orchestration surface as public API.

**Non-goals:** Do not change FX quote orientation, bump semantics, arbitrage formulas, tolerances, or report schemas.

**Invariants touched:** FX orientation and metadata; arbitrage numerical results and report serde.

## Implementation

1. Add/retain behavior tests through the canonical matrix and arbitrage orchestration APIs.
2. Remove public re-exports and narrow visibility of implementation traits/types.
3. Update internal imports and documentation to teach only the canonical entry points.
4. Verify no external workspace production code names the demoted types.

## Tests to add or update

- Core FX bump tests; butterfly/calendar/local-vol arbitrage report tests.

## Verify

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
```

**Bindings/parity/serde impact:** No binding shape should change; bindings already use orchestration APIs.

**Parallel and merge safety:** Safe with most plans; conflicts with C08/C09 only if `market_data/surfaces/mod.rs` is also edited.

**Rollback:** Revert visibility/re-export changes as one commit.

## Done when

- The internal provider/check types disappear from rustdoc and prelude exports; canonical operations remain unchanged.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.

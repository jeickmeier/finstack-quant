# Consolidation Plan: D10 — Give MarketContext one canonical mutation API

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

**Based on:** [Core, cashflows, and valuations simplicity audit](../2026-07-12-core-cashflows-valuations-simplicity-audit.md) dated 2026-07-12  
**User priorities:** complete all five clusters through PR-sized, independently green slices  
**Plan date:** 2026-07-12  
**Status:** planned  
**Suggested branch:** `codex/simplify-d10-canonicalize-market-context-mutation`

## Slicing principles applied

- One theme and one commit/PR.
- The tree must compile and all listed gates must pass before merge.
- Rust remains canonical; binding triplets move together.
- Compatibility parsing may survive privately; parallel runtime APIs may not.

## Slice 1 — Give MarketContext one canonical mutation API

**Tier:** 3/4 (public API and FX-sensitive)  
**Estimated net LOC:** −150 to −350 net; high mechanical churn  
**Addresses:** F6  
**Depends on:** C09 recommended; no Cluster B market-context edits in flight

**Files/filesets:**
- `finstack-quant/core/src/market_data/context/mod.rs`
- `finstack-quant/core/src/market_data/context/**`
- `finstack-quant-py/src/bindings/core/market_data/context.rs`
- `Workspace call sites returned by `rg` for consuming `insert*`, `clear_fx`, and `map_collateral` methods`
- `Affected notebooks, benches, rustdoc, stubs, and parity entries`

**Scope:** Make `insert*`, FX, collateral, and index operations canonical `&mut self -> &mut Self` methods. Retain only explicitly named `with_*` consuming conveniences for heavily used fluent construction, all delegating to the mutable kernels.

**Non-goals:** Do not change insertion order, credit-index rebinding, FX state, collateral mapping, context versioning, or cache invalidation.

**Invariants touched:** FX policy/state, credit-index rebinding, insertion order, version/cache invalidation.

## Implementation

1. Characterize every receiver-family side effect and rebind/version behavior.
2. Create the final naming split: mutable base names and deliberate consuming `with_*` adapters.
3. Migrate Python off `mem::take` and mechanically migrate Rust fluent call sites.
4. Delete `_mut` shadows and unearned consuming variants; update docs/stubs/parity.
5. Run a final workspace search proving no old receiver family remains.

## Tests to add or update

- Context insertion/replacement/version tests; FX and credit-index rebinding goldens; Python context mutation tests.

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

**Bindings/parity/serde impact:** Python directly touched; run full stack. This is a compile-atomic large-PR exception because receiver renames span the workspace.

**Parallel and merge safety:** Do not implement in parallel with C09, B09-B17, or other context/binding work. Rebase immediately before merge.

**Rollback:** One atomic revert including all call sites and parity metadata.

## Done when

- One mutation kernel per operation; any remaining `with_*` wrapper is documented as fluent-only and contains no logic.
- The diff contains no unrelated cleanup and `rtk git diff --check` is clean.
- Actual final status lines from every verification command are recorded in the PR.
- A targeted `finstack-simplify` re-audit reports this finding closed and no replacement parallel path introduced.

# Consolidation Plan B10: Migrate metric dependency consumers

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F11 consumer half-migration
- **Risk tier:** Tier 3 — risk-factor selection behavior
- **Estimated net LOC:** -20 to +100
- **Dependencies:** B09
- **Branch:** `codex/simplicity-b10-metric-market-dependencies`
- **Commit subject:** `refactor(valuations): use canonical metric dependencies`
- **Parallel / merge safety:** May run in parallel with B14 after B09. Must precede B11/B12 completion and B13 deletion. Conflicts with unrelated sensitivity work in the listed files.

## Scope

Make DV01, finite-difference Greeks, vega, cross-factor sensitivity, and cashflow export consume only canonical `MarketDependencies`. Remove consumer-side fallback and branching over legacy curve/equity dependency traits.

### Exact files

- `finstack-quant/valuations/src/metrics/sensitivities/dv01.rs`
- `finstack-quant/valuations/src/metrics/sensitivities/fd_greeks.rs`
- `finstack-quant/valuations/src/metrics/sensitivities/vega.rs`
- `finstack-quant/valuations/src/metrics/sensitivities/cross_factor.rs`
- `finstack-quant/valuations/src/instruments/common_impl/cashflow_export.rs`

### Non-goals

- No provider migration or legacy-trait deletion.
- No bump-size, metric-unit, metric-key, or finite-difference formula change.
- No new dependency inference inside metrics.

## Invariants

- The same market factors are bumped exactly once.
- Volatility bumps retain surface IDs and reference strikes.
- Existing fully qualified metric keys and units are unchanged.
- Instruments without a dependency remain excluded as before.

## Implementation steps

1. Replace legacy trait queries with `market_dependencies()` in each consumer.
2. Centralize any repeated filtering of curves, spots, FX, volatility, and correlation factors.
3. Remove consumer compatibility branches.
4. Add snapshot/set-equality tests for representative rates, equity, FX, commodity, exotics, fixed-income, and credit-derivative instruments.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations dv01 --lib
rtk cargo test -p finstack-quant-valuations vega --lib
rtk cargo test -p finstack-quant-valuations fd_greeks --lib
rtk cargo test -p finstack-quant-valuations cross_factor --lib
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

No API or wire change is intended. Binding behavioral tests must retain metric keys and values; any changed factor set is a correctness regression unless explicitly traced to the F11 strike-loss bug and approved.

## Rollback

Revert before B13. The temporary adapters from B09 keep the legacy consumer path available until deletion.

## Done criteria

- Listed consumers query only canonical dependencies.
- No consumer-side legacy fallback remains.
- Factor-set and metric regression tests pass.
- Full verification is green.

## Targeted re-audit acceptance

`rtk rg -n 'curve_dependencies|equity_dependencies' finstack-quant/valuations/src/metrics finstack-quant/valuations/src/instruments/common_impl/cashflow_export.rs` returns no production uses.

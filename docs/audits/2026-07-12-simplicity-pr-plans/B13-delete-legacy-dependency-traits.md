# Consolidation Plan B13: Delete legacy dependency traits and adapters

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F11 final deletion
- **Risk tier:** Tier 3 — public Rust trait deletion
- **Estimated net LOC:** -250 to -450
- **Dependencies:** B11, B12
- **Branch:** `codex/simplicity-b13-delete-legacy-dependencies`
- **Commit subject:** `refactor(valuations): delete legacy dependency traits`
- **Parallel / merge safety:** Must land after both complete provider waves. Conflicts with B14/B17 in `instrument.rs` and with option-trait work; serialize those edits or rebase immediately before implementation.

## Scope

Delete `CurveDependencies`, `EquityDependencies`, their builder/data wrappers that are no longer canonical, the option macro branches that generate them, temporary B09 adapters, and root exports. `MarketDependencies` becomes the only dependency contract.

### Exact files

- `finstack-quant/valuations/src/instruments/common_impl/traits/curve_dependencies.rs` (delete)
- `finstack-quant/valuations/src/instruments/common_impl/traits/equity_dependencies.rs` (delete)
- `finstack-quant/valuations/src/instruments/common_impl/traits/mod.rs`
- `finstack-quant/valuations/src/instruments/common_impl/traits/option_greeks.rs`
- `finstack-quant/valuations/src/instruments/common_impl/dependencies.rs`
- `finstack-quant/valuations/src/instruments/common_impl/traits/instrument.rs`
- `finstack-quant/valuations/src/instruments/mod.rs`

This deletion slice is an atomic exception to the 1–5-file target: deleting only part of the old contract would leave an unusable public half-surface.

### Non-goals

- No further provider migration; B11/B12 must already be complete.
- No change to dependency meaning, risk metrics, or pricing.
- No deprecated aliases that indefinitely preserve the removed traits.

## Invariants

- Every instrument needed by metrics implements complete canonical dependencies before deletion.
- Canonical curve-role and volatility-strike metadata remains unchanged.
- No binding layer reconstructs dependencies independently.

## Implementation steps

1. Prove B11/B12 acceptance searches are clean.
2. Remove legacy macro branches and temporary conversion constructors.
3. Delete both trait modules and obsolete builder/data types.
4. Remove trait methods, imports, exports, docs, and examples.
5. Resolve all compiler failures by canonical imports only; do not add compatibility traits.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations --lib
rtk cargo test -p finstack-quant-valuations --tests
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

This is a Rust API break only. Python/WASM must continue to derive behavior from canonical Rust dependencies and require no duplicate implementation. Any generated-doc or stub reference to deleted traits is removed in the same PR.

## Rollback

Revert B13 alone while B09–B12 remain landed; the temporary adapters and traits are restored without reverting provider implementations.

## Done criteria

- Legacy trait modules, builders, adapters, and exports are gone.
- `MarketDependencies` is the sole dependency contract.
- Risk-factor regression tests and full verification are green.

## Targeted re-audit acceptance

```sh
rtk rg -n 'CurveDependencies|EquityDependencies|InstrumentCurvesBuilder|EquityInstrumentDepsBuilder|fn (curve_dependencies|equity_dependencies)' \
  finstack-quant/valuations/src
```

The command returns no production matches; references in historical audit documents are excluded.

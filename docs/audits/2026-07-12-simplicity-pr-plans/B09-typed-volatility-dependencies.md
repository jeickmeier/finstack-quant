# Consolidation Plan B09: Add typed volatility dependencies

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F11 foundation
- **Risk tier:** Tier 2 — additive dependency-model API
- **Estimated net LOC:** +60 to +140
- **Dependencies:** B01
- **Branch:** `codex/simplicity-b09-typed-vol-dependencies`
- **Commit subject:** `refactor(valuations): type volatility dependencies`
- **Parallel / merge safety:** Must precede B10–B13. Conflicts with B01 in `instrument.rs`; otherwise safe beside B02–B08 and B14.

## Scope

Extend canonical `MarketDependencies` with typed volatility dependencies that preserve the data currently lost when legacy equity dependencies are reconstructed, including reference strike where required. Provide temporary lossless adapters from legacy dependency traits so consumers can migrate before provider traits are deleted.

### Exact files

- `finstack-quant/valuations/src/instruments/common_impl/dependencies.rs`
- `finstack-quant/valuations/src/instruments/common_impl/traits/instrument.rs`
- `finstack-quant/valuations/src/instruments/common_impl/mod.rs`
- `finstack-quant/valuations/src/instruments/mod.rs`

### Non-goals

- No provider migration; that belongs to B11/B12.
- No deletion of `CurveDependencies` or `EquityDependencies`; that belongs to B13.
- No market-data lookup or pricing-formula change.

## Invariants

- Dependency conversion is lossless, especially volatility surface ID, underlying ID, and reference strike.
- Existing curve, spot, FX, and correlation dependency semantics remain stable.
- Ordering and deduplication remain deterministic.

## Implementation steps

1. Define the smallest typed volatility-dependency representation inside canonical dependencies.
2. Add it to `MarketDependencies` with deterministic merge/deduplication behavior.
3. Replace the `reference_strike: None` reconstruction with a lossless legacy adapter.
4. Add focused unit tests for roundtrip conversion, equality, ordering, and deduplication.
5. Document the adapter as temporary and internal to the B09–B13 migration.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations common_impl::dependencies --lib
rtk cargo test -p finstack-quant-valuations equity_option --lib
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

Dependencies are Rust-side metadata, not an instrument wire-schema redesign. Do not expose temporary adapters through Python/WASM. If `MarketDependencies` is bound, add the typed field consistently across Rust/Python/WASM and update the parity contract in this PR.

## Rollback

Revert before B10–B13. After consumer/provider migrations land, rollback requires reverting the dependent sequence in reverse order.

## Done criteria

- Canonical dependencies can represent every legacy equity/volatility dependency losslessly.
- Reference strike survives conversion.
- Temporary adapters are documented and tested.
- Full verification is green.

## Targeted re-audit acceptance

No construction in `common_impl/dependencies.rs` silently writes `reference_strike: None` when the source carries a strike; unit tests demonstrate exact preservation.

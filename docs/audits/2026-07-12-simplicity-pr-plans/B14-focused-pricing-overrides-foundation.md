# Consolidation Plan B14: Add wire-only override adapter and focused accessors

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F17 foundation
- **Risk tier:** Tier 4 — serde and override-routing foundation
- **Estimated net LOC:** +80 to +180
- **Dependencies:** B01
- **Branch:** `codex/simplicity-b14-focused-overrides-foundation`
- **Commit subject:** `refactor(valuations): separate override wire format`
- **Parallel / merge safety:** May be developed beside B09–B12 but conflicts with B13 in `instrument.rs`; rebase after whichever lands first. Must precede B15–B17.

## Scope

Separate the stable legacy JSON shape from runtime storage. Add a private wire adapter that accepts existing flat/nested `pricing_overrides` payloads and converts them into focused `InstrumentPricingOverrides`, `MetricPricingOverrides`, and `ScenarioPricingOverrides`. Add focused `Instrument` accessors and make common helpers prefer them while temporarily retaining full-bag hooks as migration adapters.

### Exact files

- `finstack-quant/valuations/src/instruments/pricing_overrides.rs`
- `finstack-quant/valuations/src/instruments/common_impl/traits/instrument.rs`
- `finstack-quant/valuations/src/instruments/common_impl/helpers.rs`
- `finstack-quant/valuations/src/metrics/core/traits.rs`
- `finstack-quant/valuations/src/instruments/mod.rs`

### Non-goals

- No instrument field/provider migration; that belongs to B15/B16.
- No deletion of `PricingOverrides` or mirrored builders; that belongs to B17.
- No override-name, default, validation-rule, or pricing-policy change.

## Invariants

- Existing JSON payloads deserialize identically, including flat legacy fields.
- Canonical serialization remains stable.
- Each focused category has one owner; helpers do not reconstruct policy from unrelated fields.
- Scenario adjustments remain applied exactly once.

## Implementation steps

1. Isolate custom deserialization into a private wire representation with explicit conversion to focused runtime values.
2. Add focused immutable/mutable trait accessors with temporary defaults backed by the full bag.
3. Update common helpers and metric context construction to use focused accessors.
4. Add exhaustive old-payload/new-runtime and runtime/canonical-output tests.
5. Mark full-bag accessors as temporary migration-only API for B15–B17.

## Targeted tests

```sh
rtk cargo test -p finstack-quant-valuations pricing_overrides --lib
rtk cargo test -p finstack-quant-valuations metrics::core --lib
rtk mise run rust-check-schemas
```

## Full verification

```sh
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Bindings, parity, and serde

Wire compatibility is mandatory. The adapter remains in Rust; Python/WASM must not duplicate parsing or default logic. If focused override objects are bound, names must follow Rust/Python/WASM triplet conventions and parity metadata changes land here.

## Rollback

Revert before B15/B16. After provider waves land, rollback the sequence B17, B16/B15, then B14.

## Done criteria

- Stable wire parsing is isolated from runtime storage.
- Common helpers consume focused accessors.
- Old payload fixtures roundtrip to canonical output.
- Full verification is green.

## Targeted re-audit acceptance

`helpers.rs` and metric-context construction contain no direct conversion from an all-fields `PricingOverrides` bag except the explicitly documented temporary adapter; wire parsing exists in one Rust location.

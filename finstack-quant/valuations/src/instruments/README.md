# Instruments

The instrument families, their `Family` registration statics, and the JSON
loading contract for `finstack-quant-valuations`. The `Instrument` trait, the
shared parameter types and the pricing kernels are defined in the framework
crate [`finstack-quant-valuations-core`](../../core/src/instruments/README.md)
and re-exported here unchanged.

The canonical overview and quick-start example live in [`mod.rs`](mod.rs)
rustdoc. This file covers layout, the family contract, and extension points.

## Layout

Instruments are grouped by asset class. Each leaf directory is one instrument.

```
instruments/
├── commodity/            # asian option, forward, listed commodity future, option, swap
├── credit_derivatives/   # cds, cds_index, cds_option, cds_tranche
├── equity/               # spot, option, TRS, variance swap, autocallable, cliquet,
│                         # DCF, listed equity/TRF futures, PE fund, real estate, vol future
├── exotics/              # asian, barrier, basket, lookback, range accrual,
│                         # callable range accrual, snowball, tarn
├── fixed_income/         # bond, bond future, convertible, ILB, term loan,
│                         # revolving credit, structured credit, MBS/TBA/CMO,
│                         # dollar roll, FI index TRS
├── fx/                   # spot, forward, listed future, swap, NDF, option, barrier,
│                         # digital, touch, variance swap, quanto
└── rates/                # irs, basis swap, xccy swap, cap/floor, swaption,
                          # deposit, fra, repo, IR future, CMS family,
                          # inflation swap / cap-floor
```

A typical instrument directory contains `types.rs` (or `types/`), pricing logic
(`pricer.rs` or `pricing/`), optional `cashflows.rs`, `json.rs`, and a
`metrics/` module holding its metric calculators.

Files at this level:

| File | Role |
|------|------|
| `mod.rs` | `pub use finstack_quant_valuations_core::instruments::*`, the asset-class module declarations and the flat public re-export list |
| `json_loader.rs` | `InstrumentEnvelope`, `InstrumentJson`, and the registry macro (the closed world) |
| `cashflow_export.rs` | `instrument_cashflows_json` and `instrument_cashflows(&ParsedInstrument, …)` over the core exporter (`export_cashflows`); re-exports the envelope and row types |
| `composite/` | `CompositeInstrument` and its legs; it holds `InstrumentJson`, so it belongs to the closed world |
| `<family>/family.rs` | The family's `Family` static: its pricer table and metric table |

The framework surface (`Instrument`, `PricingOptions`, parameter types, the
`common_impl` kernels, `instruments::pricing`) is re-exported flat from
`instruments::*` exactly as the core crate exports it.

## Core trait

Every instrument implements `Instrument`, defined in the framework crate
([`core/src/instruments/README.md`](../../core/src/instruments/README.md) lists
its entry points and hooks) and re-exported as
`finstack_quant_valuations::instruments::Instrument`. A family instrument names
its `Family` through `impl_instrument_base!(InstrumentType::X, FAMILY)`, so
pricing with no explicit registry resolves the family's own pricers and
metrics; `STANDARD` merges every family for the closed-world entry points.

## JSON contract

Instruments load through `InstrumentEnvelope` in
[`json_loader.rs`](json_loader.rs) under schema `finstack_quant.instrument/1`
(`InstrumentEnvelope::CURRENT_SCHEMA`). Unknown fields are rejected at
deserialize time. `InstrumentJson::into_boxed()` produces a
`Box<dyn Instrument>`.

The `with_instrument_json_registry!` macro is the single source of truth for the
registry: the `InstrumentJson` enum definition, the deserialize tag map,
`into_boxed`, `registry_tags`, and the schema-parity check are all generated
from it.

## Adding an instrument

1. Add the type under the appropriate asset-class directory, following an
   existing neighbour (for example `fixed_income/bond/` or `rates/irs/`).
2. Implement `Instrument` (`impl_instrument_base!` supplies `id`, `key`,
   `attributes` and `family`).
3. Add an `InstrumentType` variant in `core/src/pricer/keys.rs`.
4. Add one line to `with_instrument_json_registry!` in `json_loader.rs`. No
   per-site hand edits are needed — everything else is generated.
5. Add a `metrics/` module with a `register_<name>_metrics(&mut MetricRegistry)`
   function and call it from `register_metrics` in the family's `family.rs`;
   register the pricer in that file's `register_pricers` and name the family
   as the second argument of `impl_instrument_base!`.
6. Add tests under `../../tests/instruments/<name>/`, then regenerate schemas
   when the public JSON shape changes: `mise run rust-gen-schemas`
   (verify with `mise run rust-check-schemas`).

## Related

- [`../../core/src/instruments/README.md`](../../core/src/instruments/README.md) — the framework: trait, hooks, parameters, kernels
- [`../../core/src/metrics/README.md`](../../core/src/metrics/README.md) — `MetricId`, calculators, registry
- [`../../core/src/results/README.md`](../../core/src/results/README.md) — `ValuationResult` envelope
- [`../../core/src/market/README.md`](../../core/src/market/README.md) — quotes and conventions
- [`../../core/src/pricer/README.md`](../../core/src/pricer/README.md) — dispatch keys, `PricerRegistry`, `Family`
- [`../pricer/`](../pricer/) — `FAMILIES`, `STANDARD` and the JSON pricing entry points (rustdoc only)
- Per-instrument READMEs:
  [bond](fixed_income/bond/README.md),
  [structured_credit](../../structured-credit/src/structured_credit/README.md),
  [revolving_credit](fixed_income/revolving_credit/README.md),
  [term_loan](fixed_income/term_loan/README.md),
  [irs](rates/irs/README.md),
  [cds_option](credit_derivatives/cds_option/README.md),
  [cds_tranche](credit_derivatives/cds_tranche/README.md),
  [bond_future](fixed_income/bond_future/README.md),
  [listed instrument mechanics](../../core/src/instruments/common_impl/listed/README.md),
  [real_estate](equity/real_estate/README.md),
  [range_accrual](exotics/range_accrual/README.md)

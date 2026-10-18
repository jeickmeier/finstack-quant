# Instruments

Instrument definitions, the `Instrument` trait, per-instrument metric
registration, and the JSON loading contract for `finstack-quant-valuations`.

The canonical overview and quick-start example live in [`mod.rs`](mod.rs)
rustdoc. This file covers layout, the trait contract, and extension points.

## Layout

Instruments are grouped by asset class. Each leaf directory is one instrument.

```
instruments/
├── common_impl/          # crate-private: traits, shared listed terms/future-option mechanics,
│                         # parameters and pricing helpers
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
| `mod.rs` | Asset-class module declarations and the flat public re-export list |
| `json_loader.rs` | `InstrumentEnvelope`, `InstrumentJson`, and the registry macro |
| `pricing_overrides.rs` | `InstrumentPricingOverrides`, `MetricPricingOverrides`, `ScenarioPricingOverrides`, `ModelConfig`, `BumpConfig` |
| `marginable.rs` | private `finstack_quant_margin::Marginable` impls, reached only through `Instrument::as_marginable` |
| `model_params.rs` | `ModelParamsSnapshot` used by attribution |
| `position.rs` | `Position` (long/short direction) |
| `breakeven.rs` | `BreakevenConfig`, `BreakevenMode`, `BreakevenTarget` |

`common_impl` is `pub(crate)`. Its supported surface is re-exported flat from
`instruments::*`, plus two public sub-namespaces: `instruments::pricing`
(schedules, generic pricers, `TrsEngine`) and `instruments::cashflow_export`
(per-flow DF / survival / PV columns used by the bindings).

## Core trait

Every instrument implements `Instrument`, defined in
[`common_impl/traits/instrument.rs`](common_impl/traits/instrument.rs) and
re-exported as `finstack_quant_valuations::instruments::Instrument`. It requires
`CashflowProvider` (from `finstack-quant-cashflows`) and `Send + Sync`.

Main entry points:

| Method | Returns |
|--------|---------|
| `id()`, `key()` | Instrument identifier and `InstrumentType` for pricer dispatch |
| `value(market, as_of)` | `Money` PV only, in the instrument's native currency |
| `price_with_metrics(market, as_of, metrics, options)` | `ValuationResult` — PV plus every requested `MetricId`; pass `&[]` for PV only |
| `cashflow_schedule(market, as_of)` | Canonical signed, future-filtered `CashFlowSchedule` |
| `dated_cashflows(market, as_of)` | Flattened `(Date, Money)` view of the same schedule |
| `market_dependencies()` | Declared curve/surface/FX requirements |
| `default_model()` | `ModelKey` used when `PricingOptions::model` is `None` |
| `attributes()` / `attributes_mut()` | Tags for scenario selection and reporting |
| `as_marginable()` | Optional `finstack_quant_margin::Marginable` view |

`PricingOptions` carries the optional config, model override, pricer registry,
market history, and quote-recalibration provider; `PricingOptions::default()`
uses the shared standard registry and each instrument's `default_model()`.

Cashflow policy is universal. Deterministic products emit contractual or
projected schedules; contingent or exhausted products still return an explicit
empty schedule tagged so `Placeholder` is distinguishable from `NoResidual`.

`Instrument` is a stable compatibility surface used behind
`Arc<dyn Instrument>` across portfolio and binding code. New optional
capabilities belong in focused provider traits (follow `OptionGreeksProvider`),
not as new required methods.

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
2. Implement `Instrument` and register a pricer in the matching
   `src/pricer/<asset_class>.rs` shard.
3. Add an `InstrumentType` variant in `src/pricer/keys.rs`.
4. Add one line to `with_instrument_json_registry!` in `json_loader.rs`. No
   per-site hand edits are needed — everything else is generated.
5. Add a `metrics/` module with a `register_<name>_metrics(&mut MetricRegistry)`
   function and call it from the matching
   `register_*_instrument_metrics` in `src/metrics/core/standard_registry.rs`.
6. Add tests under `../../tests/instruments/<name>/`, then regenerate schemas
   when the public JSON shape changes: `mise run rust-gen-schemas`
   (verify with `mise run rust-check-schemas`).

## Related

- [`../metrics/README.md`](../metrics/README.md) — `MetricId`, calculators, registry
- [`../results/README.md`](../results/README.md) — `ValuationResult` envelope
- [`../calibration/README.md`](../calibration/README.md) — building a `MarketContext`
- [`../market/README.md`](../market/README.md) — quotes and conventions
- [`../pricer/`](../pricer/) — dispatch keys and `PricerRegistry` (rustdoc only)
- Per-instrument READMEs:
  [bond](fixed_income/bond/README.md),
  [structured_credit](fixed_income/structured_credit/README.md),
  [revolving_credit](fixed_income/revolving_credit/README.md),
  [term_loan](fixed_income/term_loan/README.md),
  [irs](rates/irs/README.md),
  [cds_option](credit_derivatives/cds_option/README.md),
  [cds_tranche](credit_derivatives/cds_tranche/README.md),
  [bond_future](fixed_income/bond_future/README.md),
  [listed instrument mechanics](common_impl/listed/README.md),
  [real_estate](equity/real_estate/README.md),
  [range_accrual](exotics/range_accrual/README.md)

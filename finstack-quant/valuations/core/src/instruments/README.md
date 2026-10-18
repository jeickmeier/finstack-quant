# Instruments (framework)

The `Instrument` trait, its pricing hooks, the shared parameter types and the
pricing kernels every instrument family builds on. No instrument type lives
here: the families (`rates`, `fixed_income`, `credit_derivatives`, `equity`,
`fx`, `commodity`, `exotics`) are in the umbrella crate
[`finstack-quant-valuations`](../../../src/instruments/README.md), which
re-exports this module under the same `instruments::*` paths.

The canonical overview lives in [`mod.rs`](mod.rs) rustdoc; the trait's own
documentation implements it for a one-payment note.

## Layout

| Path | Role |
|------|------|
| `mod.rs` | Module declarations and the flat re-export list of the framework surface |
| [`common_impl/`](common_impl/README.md) | The `Instrument` trait and provider traits, `PricingOptions`, parameter types (legs, conventions, convertible vocabulary, Merton MC config, bond quotes), the generic pricer, shared pricing kernels, listed-contract mechanics, the `Marginable` helpers. Public but hidden from the rendered docs; the families reach it by path |
| `cashflow_export.rs` | `export_cashflows`, `InstrumentCashflowEnvelope`, `CashflowRow`, `CashflowExportTerms` — the per-flow DF / survival / PV exporter behind the umbrella's JSON entry points and the explain trace; it decomposes an instrument through `Instrument::cashflow_export_terms` and `Instrument::validate_cashflow_export` |
| `pricing_overrides.rs` | `InstrumentPricingOverrides`, `MetricPricingOverrides`, `ScenarioPricingOverrides`, `ModelConfig`, `BumpConfig` |
| `model_params.rs` | `ModelParamsSnapshot` used by attribution |
| `position.rs` | `Position` (long/short direction) |
| `breakeven.rs` | `BreakevenConfig`, `BreakevenMode`, `BreakevenTarget` |

`instruments::pricing` is the public sub-namespace for the shared pricing
infrastructure the calibration crate also uses (`swap_legs`, overnight
conventions, `hw1f_params`, the generic pricer).

## Core trait

Every instrument implements `Instrument`, defined in
[`common_impl/traits/instrument.rs`](common_impl/traits/instrument.rs) and
re-exported as `instruments::Instrument` (and unchanged through the umbrella).
It requires `CashflowProvider` (from `finstack-quant-cashflows`) and
`Send + Sync`.

Main entry points:

| Method | Returns |
|--------|---------|
| `id()`, `key()` | Instrument identifier and `InstrumentType` for pricer dispatch |
| `family()` | The `pricer::Family` whose registries price the instrument when `PricingOptions` names none |
| `value(market, as_of)` | `Money` PV only, in the instrument's native currency |
| `price_with_metrics(market, as_of, metrics, options)` | `ValuationResult` — PV plus every requested `MetricId`; pass `&[]` for PV only |
| `cashflow_schedule(market, as_of)` | Canonical signed, future-filtered `CashFlowSchedule` |
| `dated_cashflows(market, as_of)` | Flattened `(Date, Money)` view of the same schedule |
| `market_dependencies()` | Declared curve/surface/FX requirements |
| `default_model()` | `ModelKey` used when `PricingOptions::model` is `None` |
| `attributes()` / `attributes_mut()` | Tags for scenario selection and reporting |
| `as_marginable()` | Optional `finstack_quant_margin::Marginable` view |

Optional hooks with framework defaults: `notional_draw_date`,
`with_bumped_fx_quote` (FX01), `composite_valuation`,
`validate_cashflow_export`, `cashflow_export_terms`.

`PricingOptions` carries the optional config, model override, pricer registry,
market history, and quote-recalibration provider; `PricingOptions::default()`
resolves the instrument's family registries and its `default_model()`.

Cashflow policy is universal. Deterministic products emit contractual or
projected schedules; contingent or exhausted products still return an explicit
empty schedule tagged so `Placeholder` is distinguishable from `NoResidual`.

`Instrument` is a stable compatibility surface used behind
`Arc<dyn Instrument>` across portfolio and binding code. New optional
capabilities belong in focused provider traits (follow `OptionGreeksProvider`),
not as new required methods.

## Related

- [`../metrics/README.md`](../metrics/README.md) — `MetricId`, calculators, registry
- [`../results/README.md`](../results/README.md) — `ValuationResult` envelope
- [`../market/README.md`](../market/README.md) — quotes and conventions
- [`../pricer/README.md`](../pricer/README.md) — dispatch keys, `PricerRegistry`, `Family`
- [listed instrument mechanics](common_impl/listed/README.md)

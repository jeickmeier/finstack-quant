# finstack-quant-valuations-core

The pricing framework of the valuations stack, without the instruments. This
crate defines what an instrument *is* and how it is priced, risked and
reported; the instrument families (rates, fixed income, credit, equity, FX,
exotics, structured credit) implement it and the umbrella crate
[`finstack-quant-valuations`](../README.md) assembles everything into the
closed world the bindings and the portfolio layer use.

## Where it sits

Depends on [`finstack-quant-core`](../../core/README.md) (money, dates,
market data), [`finstack-quant-cashflows`](../../cashflows/README.md) (the
`CashflowProvider` supertrait of `Instrument`),
[`finstack-quant-covenants`](../../covenants/README.md),
[`finstack-quant-margin`](../../margin/README.md) (the `Marginable` bridge)
and [`finstack-quant-models`](../../models/README.md) (model kernels), plus the
`FinancialBuilder` derive in [`macros/`](../macros). Nothing in this crate
names a concrete instrument type.

## Crate map

| Path | Role |
|------|------|
| `src/instruments/` | The `Instrument` trait and its hooks, `PricingOptions`, parameter and override types, the generic pricer, shared pricing kernels (`common_impl`), the per-flow cashflow exporter |
| `src/pricer/` | `InstrumentType`/`ModelKey`/`PricerKey`, `Pricer`, `PricerRegistry`, `PricingDispatch`, `Family` (the registration unit), metric enrichment, product defaults |
| `src/metrics/` | `MetricId`, `MetricCalculator`, `MetricContext`, `MetricRegistry`, the universal calculators, generic sensitivities, historical VaR |
| `src/market/` | Market quotes, convention registries, listed-product catalog |
| `src/results/` | `ValuationResult`, `ValuationDetails`, provenance, row export, the composite and structured-credit result DTOs |
| `src/contract_specs.rs` | Embedded exchange contract specs (hidden; surfaced through the instruments that use them) |
| `src/recalibration.rs` | Recalibration contract implemented by the calibration crate |
| `data/` | Conventions, contract specs, product defaults and the listed-product catalog compiled in with `include_str!` |

## Usage

Downstream code imports through the umbrella crate
(`finstack_quant_valuations::instruments::Instrument` is this crate's trait,
re-exported); only instrument-family crates and the umbrella depend on this
crate directly.

```rust
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_core::pricer::{InstrumentType, ModelKey, PricerKey};

let key = PricerKey::new(InstrumentType::Bond, ModelKey::Discounting);
assert_eq!(key.model, ModelKey::Discounting);
assert_eq!(MetricId::Dv01.as_str(), "dv01");
```

## Verification

- `mise run rust-lint-crate -- finstack-quant-valuations-core`
- `mise run rust-test-crate -- finstack-quant-valuations-core` (unit tests;
  the framework tests that need instruments run in the umbrella's
  `core_framework` integration module)
- `mise run rust-doc-crate -- finstack-quant-valuations-core`

## License

Same as the workspace.

# Finstack Quant Valuations

`finstack-quant-valuations` prices instruments, computes risk metrics, and calibrates market structures. Results use deterministic numerics and schema-versioned JSON where serialization applies.

## Layout

| Path | Role |
|------|------|
| `src/instruments/` | Instrument types, pricing, JSON loading |
| `src/pricer/` | Pricing dispatch and registry infrastructure |
| `src/models/` | Pricing models and numerical methods (analytical, tree, PDE, Monte Carlo) |
| `src/metrics/` | `MetricId`, registries, sensitivity calculators |
| `src/calibration/` | Calibration plans, solvers, validation |
| `src/market/` | Quotes, conventions, quote-to-instrument builders |
| `src/results/` | `ValuationResult` and export helpers |
| `src/correlation/` | Correlation, copula, and factor models for credit structures |
| `schemas/` | Generated JSON Schema artifacts |

P&L attribution and covenant evaluation live in the separate `finstack-quant-attribution`
and `finstack-quant-covenants` crates (the latter is a dependency of this crate).

## Dependencies

- `finstack-quant-core` — dates, money, market data, math
- `finstack-quant-cashflows` — schedules and accrual
- `finstack-quant-analytics` — shared analytics helpers
- `finstack-quant-margin` — margin and XVA integrations
- `finstack-quant-monte-carlo` — simulation paths (used by selected pricers)

Bindings: umbrella `finstack-quant` (`valuations` feature), `finstack-quant-py`, `finstack-quant-wasm`.

## Features

| Feature | Default | Purpose |
|---------|---------|---------|
| `ts_export` | off | TypeScript export for selected schema types |

## Usage

```toml
[dependencies]
finstack-quant-valuations = { path = "../finstack-quant/valuations" }
```

Or via the umbrella crate:

```toml
[dependencies]
finstack-quant = { path = "../finstack-quant", features = ["valuations"] }
```

Crate API docs: `cargo doc -p finstack-quant-valuations --open`.

Module READMEs under `src/` cover calibration, metrics, instruments, market, and results.

## Verification

```bash
mise run rust-fmt
mise run rust-clippy
mise run rust-test
```

## License

MIT OR Apache-2.0

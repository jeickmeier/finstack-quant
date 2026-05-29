# Finstack Valuations

`finstack-valuations` prices instruments, computes risk metrics, and calibrates market structures. Results use deterministic numerics and schema-versioned JSON where serialization applies.

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

P&L attribution and covenant evaluation live in the separate `finstack-attribution`
and `finstack-covenants` crates (the latter is a dependency of this crate).

## Dependencies

- `finstack-core` — dates, money, market data, math
- `finstack-cashflows` — schedules and accrual
- `finstack-analytics` — shared analytics helpers
- `finstack-margin` — margin and XVA integrations
- `finstack-monte-carlo` — simulation paths (used by selected pricers)

Bindings: umbrella `finstack` (`valuations` feature), `finstack-py`, `finstack-wasm`.

## Features

| Feature | Default | Purpose |
|---------|---------|---------|
| `ts_export` | off | TypeScript export for selected schema types |
| `fx-vanna-volga` | off | FX barrier Vanna–Volga smile correction (research; not registered in the default pricer registry) |

## Usage

```toml
[dependencies]
finstack-valuations = { path = "../finstack/valuations" }
```

Or via the umbrella crate:

```toml
[dependencies]
finstack = { path = "../finstack", features = ["valuations"] }
```

Crate API docs: `cargo doc -p finstack-valuations --open`.

Module READMEs under `src/` cover calibration, metrics, instruments, market, and results.

## Verification

```bash
mise run rust-fmt
mise run rust-clippy
mise run rust-test
```

## License

MIT OR Apache-2.0

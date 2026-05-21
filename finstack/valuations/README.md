# Finstack Valuations

`finstack-valuations` prices instruments, computes risk metrics, calibrates market structures, attributes P&L, and evaluates covenants. Results use deterministic numerics and schema-versioned JSON where serialization applies.

## Layout

| Path | Role |
|------|------|
| `src/instruments/` | Instrument types, pricing, JSON loading |
| `src/metrics/` | `MetricId`, registries, sensitivity calculators |
| `src/calibration/` | Calibration plans, solvers, validation |
| `src/market/` | Quotes, conventions, quote-to-instrument builders |
| `src/attribution/` | Parallel, waterfall, and metrics-based P&L attribution |
| `src/covenants/` | Covenant evaluation and forward projection |
| `src/results/` | `ValuationResult` and export helpers |
| `src/correlation/` | Correlation, copula, and factor models for credit structures |
| `schemas/` | Generated JSON Schema artifacts |

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

Module READMEs under `src/` cover calibration, metrics, instruments, attribution, and related areas.

## Verification

```bash
mise run rust-fmt
mise run rust-clippy
mise run rust-test
```

## License

MIT OR Apache-2.0

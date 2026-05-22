# Finstack Quant Module Index

Use this map to connect quant review concerns to source locations.

## Core Market Infrastructure

- Dates, calendars, day counts: `finstack/core/src/dates/`
- Market data and curves: `finstack/core/src/market_data/`
- Volatility and implied vol: `finstack/core/src/math/volatility/`
- Money, currency, FX policy: `finstack/core/src/money/`, `finstack/core/src/currency/`

## Valuation And Risk

- Valuation integrations: `finstack/valuations/src/`
- Attribution: `finstack/valuations/src/attribution/`
- Fixed income instruments: `finstack/valuations/src/instruments/fixed_income/`
- Structured credit metrics: `finstack/valuations/src/instruments/fixed_income/structured_credit/metrics/`
- Convertible metrics: `finstack/valuations/src/instruments/fixed_income/convertible/metrics/`

## Portfolio, Scenarios, And Margin

- Portfolio attribution and aggregation: `finstack/portfolio/src/`
- Scenario specs and engines: `finstack/scenarios/src/`
- SIMM and margin calculators: `finstack/margin/src/`
- Monte Carlo engines and payoffs: `finstack/monte_carlo/src/`

## Binding Exposure

- Python bindings: `finstack-py/src/bindings/`
- WASM bindings: `finstack-wasm/src/api/`
- Parity contract: `finstack-py/parity_contract.toml`

When a quant finding affects an exposed user workflow, trace Rust implementation -> Rust tests -> binding wrappers -> stubs/types -> parity contract -> examples/notebooks.

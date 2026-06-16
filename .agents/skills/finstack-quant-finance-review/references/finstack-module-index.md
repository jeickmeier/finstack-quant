# Finstack Quant Module Index

Use this map to connect quant review concerns to source locations.

## Core Market Infrastructure

- Dates, calendars, day counts: `finstack-quant/core/src/dates/`
- Market data and curves: `finstack-quant/core/src/market_data/`
- Volatility and implied vol: `finstack-quant/core/src/math/volatility/`
- Money, currency, FX policy: `finstack-quant/core/src/money/`, `finstack-quant/core/src/currency/`

## Valuation And Risk

- Valuation integrations: `finstack-quant/valuations/src/`
- Attribution: `finstack-quant/valuations/src/attribution/`
- Fixed income instruments: `finstack-quant/valuations/src/instruments/fixed_income/`
- Structured credit metrics: `finstack-quant/valuations/src/instruments/fixed_income/structured_credit/metrics/`
- Convertible metrics: `finstack-quant/valuations/src/instruments/fixed_income/convertible/metrics/`

## Portfolio, Scenarios, And Margin

- Portfolio attribution and aggregation: `finstack-quant/portfolio/src/`
- Scenario specs and engines: `finstack-quant/scenarios/src/`
- SIMM and margin calculators: `finstack-quant/margin/src/`
- Monte Carlo engines and payoffs: `finstack-quant/monte_carlo/src/`

## Binding Exposure

- Python bindings: `finstack-quant-py/src/bindings/`
- WASM bindings: `finstack-quant-wasm/src/api/`
- Parity contract: `finstack-quant-py/parity_contract.toml`

When a quant finding affects an exposed user workflow, trace Rust implementation -> Rust tests -> binding wrappers -> stubs/types -> parity contract -> examples/notebooks.

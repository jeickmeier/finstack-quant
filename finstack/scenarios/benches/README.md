# Scenarios benchmarks

Criterion benchmarks in `scenarios.rs` for composition, market shocks, statement ops,
serde round-trips, and credit stress paths.

## Run

```bash
cargo bench -p finstack-scenarios
cargo bench -p finstack-scenarios --bench scenarios -- curve_parallel_shock
cargo bench -p finstack-scenarios -- --save-baseline my_baseline
```

## Benchmark groups

| Group | What it measures |
|-------|------------------|
| `scenario_composition` | Priority merge of 2–20 specs |
| `curve_parallel_shock` | Parallel discount-curve bp shift |
| `curve_node_shock` | Key-rate bumps (2–10 nodes) |
| `hazard_curve_shock` | Parallel and node hazard shifts |
| `fx_shock` | FX quote percent change |
| `equity_shock` | 1–5 equity price shocks |
| `vol_surface_shock` | Equity vol parallel and bucket |
| `credit_vol_shock` | Credit vol parallel and bucket |
| `base_correlation_shock` | Parallel and bucket base corr |
| `instrument_spread_shock` | Spread shock by instrument type |
| `statement_operations` | Forecast percent and assign |
| `complex_multi_operation` | Mixed 5–20 operation scenarios |
| `comprehensive_credit_scenario` | Multi-leg credit stress |
| `serde_roundtrip` | JSON serialize/deserialize |
| `rate_bindings` | Curve-to-statement sync after shocks |

HTML reports land under `target/criterion/`. Release builds only.

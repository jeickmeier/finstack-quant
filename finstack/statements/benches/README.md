# finstack-statements benchmarks

Criterion benchmarks for statement modeling hot paths.

## Running

```bash
# Correctness-sized models (4–24 periods, ≤50 nodes)
cargo bench -p finstack-statements --bench statements_operations

# Production-scale workloads (Monte Carlo, rolling windows, large LBO)
cargo bench -p finstack-statements --bench statements_scale

# Filter by group or function name
cargo bench -p finstack-statements -- model_building
cargo bench -p finstack-statements -- evaluate_with_calculations

# Faster iteration
cargo bench -p finstack-statements -- --quick

# Baseline comparison
cargo bench -p finstack-statements -- --save-baseline my_baseline
cargo bench -p finstack-statements -- --baseline my_baseline
```

HTML reports land under `target/criterion/<group>/report/index.html`.

## `statements_operations` groups

| Group | Benchmarks |
|-------|------------|
| `model_building` | `simple_value_model`, `computed_nodes_model`, `large_model_50_nodes` |
| `model_evaluation` | `evaluate_value_only`, `evaluate_with_calculations`, `evaluate_with_timeseries`, `evaluate_50_nodes`, `evaluate_24_periods` |
| `dsl_operations` | `parse_simple_formula`, `parse_complex_formula`, `parse_timeseries_formula`, `compile_simple_ast`, `compile_complex_ast` |
| `forecast_methods` | `forecast_forward_fill`, `forecast_growth_rate`, `forecast_seasonal`, `forecast_lognormal` |
| `registry_operations` | `create_empty_registry`, `load_builtin_metrics`, `lookup_metric`, `check_metric_exists` |
| `results_export` | `export_to_long_table`, `export_to_wide_table`, `export_large_to_*` |
| `serialization` | `serialize_model_to_json`, `deserialize_model_from_json` |
| `end_to_end` | `simple_pl_model`, `complex_financial_model` |

Export benchmarks use `StatementResult::to_table_*` envelope APIs.

## `statements_scale` groups

| Group | Purpose |
|-------|---------|
| `monte_carlo_scaling` | Path-count sweeps for Monte Carlo overhead |
| `rolling_window_scaling` | Many rolling-aggregate formulas on one node |
| `large_lbo_model` | 100 nodes × 60 monthly periods |

Re-run `statements_scale` after changes to the evaluator hot path, Monte Carlo loop, or capital-structure waterfall. See `BENCHMARKS.md` for details.

## Regression tracking

Flag for investigation when end-to-end latency grows more than ~10%, any single benchmark more than ~20%, or scaling becomes clearly non-linear.

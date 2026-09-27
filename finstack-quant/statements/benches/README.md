# finstack-quant-statements benchmarks

Criterion benchmarks for statement modeling hot paths. Two targets are declared
explicitly in `Cargo.toml` (`autobenches = false`), so nothing here is picked up
by filename convention:

| Target | Scope |
|--------|-------|
| `statements_operations` | Correctness-sized models — 4–24 periods, ≤50 nodes |
| `statements_scale` | Production-sized workloads — Monte Carlo, rolling windows, 100×60 LBO |

## Running

```bash
# One target
cargo bench -p finstack-quant-statements --bench statements_operations
cargo bench -p finstack-quant-statements --bench statements_scale

# Filter by group or benchmark name (runs both targets, keeps matches)
cargo bench -p finstack-quant-statements -- model_building
cargo bench -p finstack-quant-statements -- evaluate_with_calculations

# Faster iteration
cargo bench -p finstack-quant-statements -- --quick

# Baseline comparison
cargo bench -p finstack-quant-statements -- --save-baseline my_baseline
cargo bench -p finstack-quant-statements -- --baseline my_baseline
```

HTML reports land under `target/criterion/<group>/report/index.html`.

`mise run rust-lint` and `mise run rust-fix` do not compile Criterion targets.
Workspace-wide runs and regression gating use `mise run rust-bench`,
`mise run rust-bench-baseline`, and `mise run rust-bench-compare` (the last
fails above a 10% median regression).

## `statements_operations` groups

| Group | Benchmarks |
|-------|------------|
| `model_building` | `simple_value_model`, `computed_nodes_model`, `large_model_50_nodes` |
| `model_evaluation` | `evaluate_value_only`, `evaluate_with_calculations`, `evaluate_with_timeseries`, `evaluate_50_nodes`, `evaluate_24_periods` |
| `dsl_operations` | `parse_simple_formula`, `parse_complex_formula`, `parse_timeseries_formula`, `compile_simple_ast`, `compile_complex_ast` |
| `forecast_methods` | `forecast_forward_fill`, `forecast_growth_rate`, `forecast_seasonal`, `forecast_lognormal` |
| `registry_operations` | `create_empty_registry`, `load_builtin_metrics`, `lookup_metric`, `check_metric_exists` |
| `results_export` | `export_to_long_table`, `export_to_wide_table`, `export_large_to_long_table`, `export_large_to_wide_table` |
| `serialization` | `serialize_model_to_json`, `deserialize_model_from_json` |
| `end_to_end` | `simple_pl_model`, `complex_financial_model` |

Export benchmarks exercise the `StatementResult::to_table_long` /
`to_table_wide` envelope APIs, not an ad-hoc serializer.

## `statements_scale` groups

Each group currently pins one production-representative size rather than
sweeping a range; `Throughput` is reported so the per-unit cost is comparable
if the size is changed.

| Group | Benchmark id | Workload |
|-------|--------------|----------|
| `monte_carlo_scaling` | `1000` | 1,000 Monte Carlo paths over a `Normal`-forecast model, surfacing per-path overhead (forecast-cache rebuilds, accumulator merges) |
| `rolling_window_scaling` | `rolling_count/25` | 25 rolling-aggregate formulas over one node across 24 periods, guarding the historical-value memoization in `evaluator::formula_helpers` |
| `large_lbo_model` | `evaluate/100x60` | 100 nodes × 60 monthly periods, checking that the period × node loop stays roughly linear |

Both `monte_carlo_scaling` and `large_lbo_model` set `sample_size(10)` because a
single iteration is long.

Re-run `statements_scale` after changes to the evaluator hot path
(`evaluator/{engine,formula,formula_dispatch,formula_aggregates,formula_helpers}`),
the historical cache (`evaluator/context.rs`), the Monte Carlo loop
(`evaluator/monte_carlo.rs`), or the capital-structure waterfall. See
[`BENCHMARKS.md`](BENCHMARKS.md) for the same guidance in the target's own terms.

## Regression tracking

Investigate when end-to-end latency grows more than ~10%, any single benchmark
more than ~20%, or a `statements_scale` group stops scaling roughly linearly
with its `Throughput` element count. Keep machine-specific absolute timings in
Criterion output or CI artifacts, not in this file.

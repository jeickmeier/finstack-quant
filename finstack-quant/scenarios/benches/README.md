# Scenarios benchmarks

Two Criterion targets. `scenarios.rs` covers composition, market and
instrument shocks, statement operations, rate bindings, serde round-trips, and
credit stress paths at one size. `scenarios_scaling.rs` measures how those
costs grow with operation count, curve count, and book size. The crate sets
`autobenches = false`; both targets are registered as `[[bench]]`.

## Run

```bash
mise run rust-bench                                        # whole workspace, reduced timing
cargo bench -p finstack-quant-scenarios                    # this crate, full Criterion timing
cargo bench -p finstack-quant-scenarios -- curve_parallel_shock   # filter by group name
cargo bench -p finstack-quant-scenarios -- --quick
cargo bench -p finstack-quant-scenarios -- --save-baseline my_baseline
cargo bench -p finstack-quant-scenarios -- --baseline my_baseline
```

`mise run rust-bench` overrides timing via `FQ_BENCH_SAMPLE_SIZE`,
`FQ_BENCH_WARM_UP_TIME`, `FQ_BENCH_MEASUREMENT_TIME`, and
`FQ_BENCH_NRESAMPLES`. `mise run rust-bench-baseline` / `rust-bench-compare`
save and diff a `main` baseline, failing above a 10% median regression.

## Groups

| Group | Cases |
|-------|-------|
| `scenario_composition` | `compose` over 10 specs |
| `curve_parallel_shock` | `single_curve` — 50 bp discount-curve shift |
| `curve_node_shock` | `5_nodes` key-rate bumps |
| `hazard_curve_shock` | `parallel_ig`, `node_hy` par-CDS shifts |
| `fx_shock` | `single_pair` percent move |
| `equity_shock` | `3_equities` in one `EquityPricePct` |
| `vol_surface_shock` | `parallel`, `bucket` equity vol |
| `credit_vol_shock` | `parallel`, `bucket` credit vol |
| `base_correlation_shock` | `parallel`, `bucket` base correlation |
| `instrument_spread_shock` | `by_type` spread shock, no instruments in context (measures dispatch, not mutation) |
| `statement_operations` | `forecast_percent`, `forecast_assign` |
| `complex_multi_operation` | `10_operations` mixed scenario |
| `comprehensive_credit_scenario` | `credit_stress` multi-leg |
| `serde_roundtrip` | `serialize`, `deserialize`, `roundtrip` |
| `rate_bindings` | `with_rate_bindings` curve-to-statement sync after shocks |

### `scenarios_scaling`

| Group | Sizes | What it stresses |
|-------|-------|------------------|
| `scaling_same_curve_ops` | 1 / 8 / 24 / 48 sequential discount bumps on one curve | Sequential flush-before-next-op |
| `scaling_hierarchy_curves` | 16 / 64 / 128 curves under one `HierarchyCurveParallelBp` | Expansion + N synthetic discount rebuilds |
| `scaling_hierarchy_par_cds` | 2 / 4 / 8 hazard curves under one ParCDS hierarchy shock | Expansion + N explicit first-order hazard shifts |
| `historical_credit_quote_replay_10bp` | One calibrated credit curve, 10bp pillar spread shock | Exact quote replay through the cached recalibration provider; initial calibration is outside timing |
| `scaling_instrument_spread` | 50 / 200 / 500 bonds | Instrument-spread dispatch |
| `scaling_time_roll_instruments` | 10 / 40 / 80 bonds | Time-roll carry (Rayon above 64) |
| `scaling_compose` | 10 / 50 / 200 specs | `compose` |

Most groups rebuild the market context inside `b.iter` so the measurement
includes the engine's own clone/bump path; `scenario_composition` deliberately
pre-builds its specs so it measures composition only.

## Results

HTML reports: `target/criterion/<group>/report/index.html`. Release builds only;
timings vary by hardware. Use `--quick` while iterating.

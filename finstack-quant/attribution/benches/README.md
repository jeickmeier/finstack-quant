# finstack-quant-attribution benchmarks

Criterion benchmarks for the P&L attribution entry points. The crate sets
`autobenches = false` in [`../Cargo.toml`](../Cargo.toml), so a new file here does
nothing until it is added as a `[[bench]]` target. Two targets are registered, both
`harness = false`:

| Target | Scope |
|--------|-------|
| `attribution` | Fixed-size cost of every public hot path at one representative size |
| `attribution_scale` | How cost grows with book size, curve count, and methodology |

The split matters. `attribution` answers "how expensive is this call"; only
`attribution_scale` can catch a super-linear term, and it does so by reporting
`Throughput::Elements` so ns-per-instrument (or ns-per-curve) is comparable
across sizes. Shared fixtures live in [`support/fixtures.rs`](support/fixtures.rs)
and are built outside `b.iter`.

Both targets attribute across the same shape of market move unless a case
says otherwise: a flat `USD-OIS` `DiscountCurve` at 4%, shifted between
`market_t0` and `market_t1`. `attribution.rs` uses a 5 bp shift;
`attribution_scale.rs` uses 1 bp. Lean-market instruments are vanilla
fixed-coupon `Bond`s (1M notional, 5% coupon). Fat-market cases add unused
hazard, inflation, FX, and spot families so extract/restore cost is visible.

## `attribution` benchmarks

Legacy top-level ids (no `benchmark_group`, preserved for `--baseline`
compare) plus the `attribution_hot_paths` group:

| Id | Measures |
|----|----------|
| `parallel_1_bond` | `attribute_pnl` (`Parallel`) with `ExecutionPolicy::Parallel`, single 5y bond |
| `waterfall_1_bond` | `attribute_pnl` (`Waterfall`) over `default_waterfall_order()`, single 5y bond |
| `parallel_5_bonds` | The parallel path looped over 5 bonds with maturities spread 3y–11y |
| `simple_bridge_1_bond` | Two-reprice baseline |
| `metrics_based_precomputed_1_bond` | Linear decomposition with `price_with_metrics` outside the timer |
| `taylor_1_bond` / `taylor_gamma_1_bond` | First-order vs `include_gamma` Taylor |
| `parallel_serial_1_bond` | Same bond under `ExecutionPolicy::Serial` |
| `parallel_fat_market_1_bond` / `waterfall_fat_market_1_bond` | Lean bond against a multi-family book market |
| `equity_parallel_1` | Spot equity (scalars + carry, no YTM flat-curve work) |
| `fx_translate_1_bond` | `translate_to_target_currency` on a precomputed EUR attribution |
| `long_rows_1_bond` | `pnl_attribution_long_rows` |
| `snapshot_extract_restore_rates` / `snapshot_extract_restore_all_fat` | `MarketSnapshot::extract` + `restore_market` |
| `return_contribution_1k` / `_brinson_1k` / `_json_1k` | Weight × return contribution |
| `spec_execute_1_bond` | `AttributionEnvelope::execute` reconstruction + parallel |

## `attribution_scale` benchmarks

| Group | Ids | Measures |
|-------|-----|----------|
| `attribution` | `<method>/{10,100,1000}` for `simple_bridge`, `metrics_based`, `metrics_based_precomputed`, `parallel`, `parallel_serial`, `waterfall`, `taylor` | Per-instrument cost at three book sizes |
| `attribution_credit` | `parallel_with_credit_model/200` | `AttributionEnvelope::execute` over 200 specs carrying a `CreditFactorModel` |
| `return_contribution` | `gross/{100,1000,10000}`, `brinson/{100,1000,10000}` | Weight × return and Brinson-Fachler roll-up |
| `snapshot_extract_restore` | `rates/{1,10,50}` | Extract + restore vs curve count |

`pnl_bridge` is the intended baseline: two reprices, no factor loop.
`metrics_based` still includes two `price_with_metrics` calls inside the
timer; `metrics_based_precomputed` isolates the linear decomposition.

Both original groups set `sample_size(10)` because N = 1000 waterfall /
parallel would take minutes at Criterion's default of 100 samples. The
return-contribution and snapshot groups use 20 samples. Treat single-run
deltas with suspicion and use `--save-baseline` / `--baseline`.

## Run

```bash
cargo bench -p finstack-quant-attribution --bench attribution
cargo bench -p finstack-quant-attribution --bench attribution_scale
cargo bench -p finstack-quant-attribution -- --quick
cargo bench -p finstack-quant-attribution -- taylor            # filter by id substring
cargo bench -p finstack-quant-attribution -- --save-baseline before
cargo bench -p finstack-quant-attribution -- --baseline before
```

Benchmarks are measurement tasks, not gates: they are not run by `mise run rust-test`
(nextest), not by `mise run all-test`, and not by PR CI. `mise run rust-lint` and
`mise run rust-fix` do not compile Criterion targets. Workspace-wide measurement goes through `mise run rust-bench` (reduced
sampling, tunable via `FQ_BENCH_SAMPLE_SIZE`, `FQ_BENCH_WARM_UP_TIME`,
`FQ_BENCH_MEASUREMENT_TIME`, `FQ_BENCH_NRESAMPLES`), with
`mise run rust-bench-baseline` and `mise run rust-bench-compare` (fails above a 10%
median regression).

Criterion writes to `target/criterion/<group>/<id>/report/index.html` for
grouped benches. Legacy `attribution` ids sit at the top level:
`target/criterion/<id>/report/index.html`. The `mise run rust-bench*` tasks pass
`--noplot`.

## Conventions when adding a case

- Put a fixed-size case in `attribution` and a size sweep in `attribution_scale`.
  Do not add a sweep to the hot-paths target — it is read as absolute cost.
- Build fixtures outside `b.iter` via [`support/fixtures.rs`](support/fixtures.rs).
- Set `group.throughput(Throughput::Elements(n))` on any size sweep.
- Register the function in the target's `criterion_group!`. A new function
  added to neither runs silently as a no-op.
- Keep the three legacy ids (`parallel_1_bond`, `waterfall_1_bond`,
  `parallel_5_bonds`) stable so `--baseline` compares stay valid.

## See also

- [`../README.md`](../README.md) — crate overview, methodology table, and the test tree
  under [`../tests/`](../tests)
- [`../../portfolio/benches/README.md`](../../portfolio/benches/README.md) — book-level
  attribution, including the method-owned controls and Rayon thresholds
- [`../../valuations/benches/README.md`](../../valuations/benches/README.md) — the
  underlying instrument pricing these benchmarks call twice per attribution

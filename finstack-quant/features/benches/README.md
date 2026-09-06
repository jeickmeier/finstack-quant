# finstack-quant-features benchmarks

Criterion benchmarks for the `finstack-quant-features` hot paths. The crate sets
`autobenches = false` in [`../Cargo.toml`](../Cargo.toml), so a new file here is inert
until it is added as a `[[bench]]` target. Two targets are registered, both
`harness = false`:

| Target | Scope |
|--------|-------|
| `features_hot_paths` | Absolute cost of each public entry point at one representative panel |
| `features_scaling` | How cost grows with row count, window, cross-section width, and factor count |

The split matches the analytics / cashflows suites. `features_hot_paths` answers
"how expensive is this call"; only `features_scaling` can catch a super-linear
term coming back, and it does so by reporting `Throughput::Elements` so
ns-per-row is comparable across sizes. Flat ns-per-row is linear; rising
ns-per-row is the regression signal.

Every public transform is on a measured path. Ops that share a kernel
(`rolling_sum` with `rolling_mean`, `rolling_max` with `rolling_min`,
`ewma_vol` with `ewma_zscore`) are represented by one case.

## `features_hot_paths` cases

The default panel is 100 names × 252 days (25,200 rows), date-major, ~5%
missing. Rolling windows default to 63; neutralize uses 3 factors.

| Criterion fn | Benchmark ids |
|--------------|---------------|
| `bench_timeseries_linear` | `returns`, `log_returns`, `lag`, `drawdown`, `ewma_mean`, `ewma_zscore` |
| `bench_timeseries_rolling` | `rolling_mean` at w=21 and w=63, `rolling_std`, `rolling_zscore`, `rolling_min` |
| `bench_timeseries_advanced` | `rolling_rank`, `rolling_quantile`, `rolling_skew`, `rolling_sharpe`, `rolling_winsorize`, `hampel_filter`, `exp_decay_weights` |
| `bench_cross_sectional` | `zscore`, `rank`, `winsorize`, `robust_zscore`, `long_short_weights`, `normal_score` |
| `bench_multi` | `rolling_corr`, `rolling_beta`, `grouped zscore`, `neutralize`, `neutralize_and_zscore`, `rolling_regression_residual`, `rank_to_weights`, `risk_scaled_weights`, `cross_sectional winsorize`, `cross_sectional zscore` |
| `bench_panel_pipeline` | typed `transform_panel` and JSON `transform_panel_json` for returns + rolling std + rank |

`lag` is the cheap O(n) baseline after the shared entity/order sort. Compare it
to `rolling_*` to isolate window work from grouping.

## `features_scaling` groups

| Group | Sizes | Measures |
|-------|-------|----------|
| `scaling_returns` | 50 / 100 / 200 / 400 names × 252 days | Sort + O(n) shift must stay linear in rows |
| `scaling_rolling_mean_rows` | 50 / 100 / 200 names × 252, w=63 | Per-row cost of the naive window collect |
| `scaling_rolling_mean_window` | w = 21 / 63 / 126 / 252 at 100 × 252 | Must stay O(n·w) today; incremental mean should flatten this |
| `scaling_rolling_rank_window` | w = 21 / 63 / 126 at 100 × 252 | Sort-per-window; ns/row should grow ~w log w |
| `scaling_hampel_window` | w = 21 / 63 / 126 at 100 × 252 | Two sorts plus a MAD buffer per row |
| `scaling_zscore_names` | 50 / 100 / 200 / 400 names × 252 | Cross-section width |
| `scaling_grouped_zscore` | 50 / 100 / 200 names × 252 | String-key rewrite + BTreeMap regroup |
| `scaling_neutralize_factors` | k = 1 / 2 / 3 / 5 at 100 × 252 | Per-date Cholesky |
| `scaling_pairwise_corr_window` | w = 21 / 63 / 126 at 100 × 252 | Two Vecs collected per row |
| `scaling_rolling_regression_window` | w = 21 / 63 at 50 × 126 | Refit OLS every row |
| `scaling_exp_decay_window` | w = 21 / 63 / 126 / 252 at 100 × 252 | Geometric sum rebuilt per row |

## Fixtures

Panel inputs live in [`support/fixtures.rs`](support/fixtures.rs) and are
deterministic — no RNG crate, no clock:

- `synthetic_returns(n, seed)` — splitmix64-style iteration mapped into
  `(-0.02, 0.02)`.
- `iso_dates(n)` — consecutive calendar days from 2020-01-01 as ISO-8601
  strings (the crate's lexicographic clock).
- `feature_panel(n_entities, n_obs, n_factors, seed)` — date-major panel
  (`values`, `levels`, `other`, `volatility`, `entity`, `order`, `time_key`,
  `groups`, `exposures`) with ~5% missing rows so pandas-`skipna` paths run.
- `hot_panel()` — 100 × 252 × 3 factors.

Keep new fixtures deterministic. A seeded generator is a hard requirement here:
benchmark inputs feed the same code paths the correctness tests pin.

## Run

```bash
cargo bench -p finstack-quant-features --bench features_hot_paths
cargo bench -p finstack-quant-features --bench features_scaling
cargo bench -p finstack-quant-features --bench features_hot_paths --bench features_scaling -- --quick
cargo bench -p finstack-quant-features --bench features_hot_paths -- rolling_mean
cargo bench -p finstack-quant-features --bench features_hot_paths --bench features_scaling -- --save-baseline before
cargo bench -p finstack-quant-features --bench features_hot_paths --bench features_scaling -- --baseline before
```

Pass `--bench` explicitly. `cargo bench -p finstack-quant-features -- --sample-size 10`
also launches the lib test harness, which rejects Criterion flags.

Benchmarks are measurement tasks, not gates: they are not run by `mise run rust-test`
(nextest), not by `mise run all-test`, and not by PR CI. `mise run rust-fmt` and
`mise run rust-lint` also skip Criterion targets. Workspace-wide measurement goes through `mise run rust-bench` (reduced
sampling, tunable via `FQ_BENCH_SAMPLE_SIZE`, `FQ_BENCH_WARM_UP_TIME`,
`FQ_BENCH_MEASUREMENT_TIME`, `FQ_BENCH_NRESAMPLES`), with
`mise run rust-bench-baseline` and `mise run rust-bench-compare` for regression gating
(the compare task fails above a 10% median regression).

Criterion writes to `target/criterion/<id>/report/index.html`; the `mise run rust-bench*`
tasks pass `--noplot`, so run `cargo bench` directly if you want the HTML.

## Conventions when adding a case

- Put a fixed-size case in `features_hot_paths` and a size sweep in
  `features_scaling` when the algorithm can go super-linear (window collect,
  per-row sort, OLS refit, string-key regroup).
- Any sweep must set `group.throughput(Throughput::Elements(n_rows))`.
- Build the panel outside `b.iter`; `black_box` the result so the call is not
  hoisted.
- Both targets set `#![allow(clippy::unwrap_used)]` and `#![allow(clippy::expect_used)]`
  at file scope. Those lints are denied by an inner attribute in
  [`../src/lib.rs`](../src/lib.rs), which covers the library crate only.

## See also

- [`../README.md`](../README.md) — crate overview, public surface, and the
  correctness tests under [`../tests/`](../tests)
- [`../../analytics/benches/README.md`](../../analytics/benches/README.md) —
  the hot-path / scaling split this suite follows

# finstack-quant-analytics benchmarks

Criterion benchmarks for the `finstack-quant-analytics` hot paths. The crate sets
`autobenches = false` in [`../Cargo.toml`](../Cargo.toml), so a new file here is inert
until it is added as a `[[bench]]` target. Two targets are registered
(`harness = false`, Criterion owns `main`): `analytics_hot_paths` (absolute
cost at one size) and `analytics_scaling` (how cost grows with series length
or matrix dimension).

Most cases are driven through [`Performance`](../src/performance/mod.rs), the crate's
canonical public entry point. The scaling target also directly benchmarks the
intentionally public `correlation::nearest_correlation_matrix` repair helper and
`regression::constrained_least_squares`. Almost every per-metric building block remains
`pub(crate)` and is not a benchmark surface.

## Cases

`analytics_hot_paths.rs` uses top-level `c.bench_function` calls rather than
`benchmark_group`, so each id below is its own Criterion directory.

| Criterion fn | Benchmark ids |
|--------------|---------------|
| `bench_tail_risk` | `Performance::value_at_risk 2.5k`, `Performance::expected_shortfall 2.5k`, `Performance::value_at_risk 100k`, `Performance::expected_shortfall 100k` |
| `bench_return_based` | `Performance::volatility 2.5k`, `Performance::sharpe 2.5k` |
| `bench_drawdown` | `Performance::drawdown_series 10k` |
| `bench_performance` | `Performance::new 750x2 daily` (construction from price levels), `Performance::sharpe 750x2`, `Performance::value_at_risk 750x2` |
| `bench_rolling_greeks` | `Performance::rolling_greeks 2.5k window=63` |
| `bench_multi_factor_greeks` | `Performance::multi_factor_greeks 2.5k k=3` — OLS against three factor columns |
| `bench_correlation_matrix` | `Performance::correlation_matrix 1k x 50` |
| `bench_period_stats` | `Performance::period_stats 2.5k monthly` |

The 2.5k / 100k pair in `bench_tail_risk` is the only size sweep; it exists because the
tail metrics sort, so per-observation cost is expected to grow with `n log n`.

## Fixtures

Both benchmark targets share deterministic inputs from
[`support/fixtures.rs`](support/fixtures.rs) — no RNG crate, no clock:

- `synthetic_returns(n, seed)` — a splitmix64-style iteration mapped into
  `(-0.02, 0.02)`.
- `synthetic_dates(n)` — consecutive calendar days from 2020-01-01 via
  `Date::next_day`.
- `perf_from_returns(n, seed)` — single-ticker `Performance::from_returns` at
  `PeriodKind::Daily`.
- `perf_panel(n_obs, n_tickers, seed)` — multi-ticker panel where column 0 (`T0`) is the
  benchmark ticker; used by the rolling-greeks, multi-factor, and correlation cases.
- `near_correlation_needs_repair(n)` — an indefinite correlation-shaped matrix used by
  the scaling target's repair benchmark.
- `constrained_ls_inputs(n_assets, n_factors)` — full-rank exposures, returns, and
  weights used by the scaling target's constrained-regression benchmark.

Keep new fixtures deterministic. A seeded generator is a hard requirement here, not a
style preference: benchmark inputs feed the same code paths the correctness tests pin.

## Run

```bash
cargo bench -p finstack-quant-analytics --bench analytics_hot_paths
cargo bench -p finstack-quant-analytics --bench analytics_scaling
cargo bench -p finstack-quant-analytics --bench analytics_hot_paths -- --quick
cargo bench -p finstack-quant-analytics -- value_at_risk        # filter by id substring
cargo bench -p finstack-quant-analytics -- --save-baseline before
cargo bench -p finstack-quant-analytics -- --baseline before
```

Benchmarks are measurement tasks, not gates: they are not run by `mise run rust-test`
(nextest), not by `mise run all-test`, and not by PR CI. `mise run rust-lint` and
`mise run rust-fix` do not compile Criterion targets. Workspace-wide measurement goes through `mise run rust-bench` (reduced
sampling, tunable via `FQ_BENCH_SAMPLE_SIZE`, `FQ_BENCH_WARM_UP_TIME`,
`FQ_BENCH_MEASUREMENT_TIME`, `FQ_BENCH_NRESAMPLES`), with
`mise run rust-bench-baseline` and `mise run rust-bench-compare` for regression gating
(the compare task fails above a 10% median regression).

Criterion writes to `target/criterion/<id>/report/index.html`; the `mise run rust-bench*`
tasks pass `--noplot`, so run `cargo bench` directly if you want the HTML.

## See also

- [`../README.md`](../README.md) — crate overview, public surface, and the correctness
  tests under [`../tests/`](../tests)
- [`../../core/benches/README.md`](../../core/benches/README.md) — core numerics
  benchmarks. Note that the core primitives analytics actually calls
  (`math::stats::quantile`, `math::summation`, `math::special_functions`,
  `math::linalg`) have no dedicated core bench target, so their cost surfaces here
  through `Performance` rather than there.

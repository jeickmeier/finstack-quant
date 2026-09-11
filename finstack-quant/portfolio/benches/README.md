# Portfolio benchmarks

Criterion benchmarks for the maintained portfolio hot paths: valuation,
selective repricing, cashflows, metrics, attribution, scenario/replay
workflows, sensitivity engines, Rayon thresholds, materialization, book
rollup, reporting analytics, margin aggregation, and LP optimization.

The suite is manifest-driven (`autobenches = false` in
[`../Cargo.toml`](../Cargo.toml)), so a new file under `benches/` does not
expand benchmark runtime unless a matching `[[bench]]` entry is added.

## Layout

| File | Registered bench? | Contents |
|------|-------------------|----------|
| `portfolio_valuation.rs` | yes | Full valuation, entity/multicurrency aggregation, filtering, PV scaling, selective-repricing shapes, `revalue_affected` |
| `portfolio_cashflows.rs` | yes | `aggregate_full_cashflows` ladder (63 / 64 / 250; 3,000 with `FINSTACK_PORTFOLIO_BENCH_XL`) plus `collapse_to_base_by_date_kind` |
| `portfolio_metrics.rs` | yes | `aggregate_metrics` scaling (63 / 64 / 250 / 3,000), combined valuation, and table export |
| `portfolio_attribution.rs` | yes | Parallel, metrics-based, and method-owned attribution controls |
| `portfolio_workflows.rs` | yes | `scenario_pnl` / `scenario_pnl_batch` reuse and `replay_portfolio` |
| `portfolio_analytics.rs` | yes | Book rollup, primitive exposure, Brinson / Campisi / grid / factor-Brinson / excess-return |
| `portfolio_margin.rs` | yes | `PortfolioMarginAggregator::calculate` over 256 / 1,024 mocked-marginable positions |
| `portfolio_optimization.rs` | yes | `DefaultLpOptimizer::optimize` on 32 / 64-bond books |
| `sensitivity_simulation.rs` | yes | Full-repricing grids, delta-based sensitivities, `FactorModel::analyze` / `factor_stress`, parametric and Monte Carlo decomposition |
| `parallel_thresholds.rs` | yes | Historical tail-risk decomposer serial fold at 400 / 500 / 600 positions |
| `materialization.rs` | yes | `Portfolio::from_materialization` / `validate_materialization` plus absolute latency gates |
| `bench_common.rs` | no | Shared fixture builders, pulled in with `#[path = "bench_common.rs"] mod bench_common;` |
| `materialization_fixtures.rs`, `materialization_gate.rs` | no | Fixture builders and percentile/gate logic used only by `materialization.rs` |
| `support/rates.rs` | no | IRS fixture helper |

## Run

```bash
mise run rust-bench                                   # whole workspace, reduced timing
cargo bench -p finstack-quant-portfolio               # this crate, full Criterion timing
cargo bench -p finstack-quant-portfolio --bench portfolio_valuation
cargo bench -p finstack-quant-portfolio -- --quick
cargo bench -p finstack-quant-portfolio -- --save-baseline my_baseline
cargo bench -p finstack-quant-portfolio -- --baseline my_baseline
```

`mise run rust-bench` overrides Criterion sample size and timing via
`FQ_BENCH_SAMPLE_SIZE`, `FQ_BENCH_WARM_UP_TIME`, `FQ_BENCH_MEASUREMENT_TIME`,
and `FQ_BENCH_NRESAMPLES`. `mise run rust-bench-baseline` and
`mise run rust-bench-compare` save and diff a `main` baseline; the compare task
fails above a 10% median regression.

## Default matrix and opt-in extensions

The default matrix keeps the large workflows bounded. The shared market supplies
synthetic published CPI history separately from its projection curve. Its TIPS
use an unlagged, linearly interpolated index; inflation swaps elect their own
three-month lag and step interpolation.

- `portfolio_valuation` PV scaling: 63 / 64 / 250 / 3,000 positions. The 63/64
  pair straddles `POSITION_PARALLEL_MIN_POSITIONS = 64`, the Rayon cut-over in
  `src/evaluation/executor.rs`.
- `portfolio_valuation` selective repricing: a 3,000-position uniform-cost
  fixture measured at 3% / 25% / 50% / 100% dirty sets.
- `portfolio_attribution`: 40 / 120 / 250 positions (40 / 120 for the
  method-owned controls).
- `portfolio_workflows` scenario P&L: 120 positions, 1 / 10 / 100 scenarios.
- `portfolio_workflows` replay: 40-position book, 20 and 250 snapshots.

Two environment flags widen it:

```bash
FINSTACK_PORTFOLIO_BENCH_FULL=1 \
FINSTACK_PORTFOLIO_BENCH_XL=1 \
cargo bench -p finstack-quant-portfolio
```

- `FINSTACK_PORTFOLIO_BENCH_FULL=1` (read by `portfolio_workflows.rs`) adds the
  3,000-position scenario-P&L case and the 300-position replay book.
- `FINSTACK_PORTFOLIO_BENCH_XL=1` (read by `portfolio_valuation.rs`) adds the
  25,000-position PV control, which materially increases Criterion setup and
  measurement time.

`bench_common::create_institutional_portfolio` builds the multi-entity fixture
used by most groups: deposits, bonds, interest-rate swaps, inflation-linked
bonds and inflation swaps, convertibles, repos, swaptions, equities and equity
options, variance swaps, FX spot and options, CDS, CDS options, CDS tranches,
and structured credit (CLO). Read
`bench_common.rs` for the exact composition before quoting a number from these
benchmarks.

## Materialization bench

`materialization.rs` is not a pure Criterion bench: before the Criterion groups
run, it takes p95 acceptance samples and **asserts** that cold fixture A stays
under a 1,000 ms hard gate (`COLD_HARD_LIMIT_MILLIS` in
`materialization_gate.rs`, alongside the 500 ms cold and 250 ms warm
engineering targets). Fixture A is 5,000 unique instrument artifacts over 5,000
positions; fixture B is 5,000 positions sharing 50 artifacts, which is the
cache-hit case.

Environment variables it reads:

| Variable | Effect |
|----------|--------|
| `FQ_MATERIALIZATION_P95_SAMPLES` | Acceptance sample count (minimum 100 unless smoke mode) |
| `FQ_MATERIALIZATION_SMOKE=1` | Permit a sample count below the acceptance minimum |
| `FQ_MATERIALIZATION_RAW_OUTPUT` | Write raw per-sample timings and phase counters to this JSON path |
| `FQ_MATERIALIZATION_CRITERION_DIR` | Criterion output directory (default `target/materialization-criterion/criterion`) |

The release workflow drives it through mise rather than by hand:

```bash
mise run materialization-benchmark-fixtures      # regenerate deterministic fixtures
mise run materialization-rust-bench-compare      # compare against the checked-in baseline
mise run materialization-rust-bench-baseline     # re-establish the baseline (guarded)
mise run materialization-benchmark-doc-check     # verify tracked digests and baselines
```

The tracked record lives in
[`benchmarks/MATERIALIZATION_BENCHMARKS.md`](../../../benchmarks/MATERIALIZATION_BENCHMARKS.md).

## Results

HTML reports: `target/criterion/<group>/report/index.html`.

```bash
open target/criterion/portfolio_pv_scaling/report/index.html
```

`materialization` is the exception — it writes to
`target/materialization-criterion/criterion` unless
`FQ_MATERIALIZATION_CRITERION_DIR` says otherwise.

Release builds only; timings vary by hardware. Use `--quick` while iterating.

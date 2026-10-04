# Python binding benchmarks

Micro- and workflow-level timings for the PyO3 bindings. Everything here measures
the *binding* cost — argument conversion, GIL handling, wrapper construction — on
top of the Rust work, so a number from this directory is only meaningful next to
the corresponding Criterion number from `finstack-quant/*/benches/`.

Two harnesses live here: a pytest-benchmark suite for discovery, and a standalone
CLI that emits raw latency samples for the one benchmark path that is gated
against a checked-in baseline (portfolio materialization).

## Layout

| Path | Role |
|------|------|
| `__init__.py` | Package marker only. Makes the directory importable; holds no code. |
| `bench_bindings.py` | The pytest-benchmark suite. 13 `@pytest.mark.perf` classes covering 9 of the 14 binding domains. |
| `materialization_measure.py` | Standalone CLI (not a pytest module). Writes one raw JSON fragment of release-Python materialization latencies for the regression gate. |

Nothing in this directory is importable public API. `bench_bindings.py` is
collected by pytest; `materialization_measure.py` is invoked by path with two
positional arguments (fixture directory, output path) and is only ever run by
the `python-bench-portfolio-*` tasks.

## Release build is mandatory

`maturin develop` without `--release` produces a debug extension. Timings from a
debug build are off by an order of magnitude and are not comparable to anything.

`mise run python-bench` handles this — it runs `mise run python-build -- --release`
(that is, `uv run maturin develop --release`) before invoking pytest. So do
`python-bench-portfolio`, `python-bench-portfolio-baseline`, and
`python-bench-portfolio-compare`.

The trap is the ordinary test loop: `mise run python-test` builds the *dev*
extension. If you then run pytest against `bench_bindings.py` by hand, you are
measuring that debug build. Re-run `mise run python-build -- --release` first, and
remember that the release extension stays installed afterwards — rebuild dev
before going back to test work if compile time matters to you.

## What the suite measures

Every class is marked `perf`. Selection within the suite is by class or by the
`slow` marker.

| Class | Covers |
|-------|--------|
| `TestCoreBenchmarks` | `Currency`, `Money` arithmetic, `DayCount.year_fraction`, `DiscountCurve.df`, `ForwardCurve.rate`, `FxMatrix.rate`, `Tenor.parse`, `Rate` conversions, `linalg.cholesky_decomposition`, `stats.mean`/`variance` |
| `TestAnalyticsBenchmarks` | `Performance` over a 10,000-point two-series panel: `sharpe`, `sortino`, `calmar`, `volatility`, `returns`, `cumulative_returns`, drawdown series and details, VaR/ES, moments, `beta`, `tracking_error`, `rolling_sharpe`, `period_stats`, `longest_positive_run`. Construction (`Performance.from_arrays`) is benched separately on a 252-point single-series panel |
| `TestCorrelationBenchmarks` | `models.correlation`: `CopulaSpec`, `CorrelatedBernoulli`, `RecoverySpec`, `LatentFactorSpec`/`LatentSingleFactor`, `correlation_bounds`, `validate_correlation_matrix` |
| `TestMonteCarloBenchmarks` | Closed-form `models.bs_price`, plus `EuropeanPricer`, `LsmcPricer`, and `PathDependentPricer` at 5,000–10,000 paths |
| `TestMarginBenchmarks` | `CsaSpec.usd_regulatory`, `VmCalculator.calculate`, `NettingSetId`, `XvaConfig`, `FundingConfig`, `MarginUtilization` |
| `TestStatementsBenchmarks` | `FinancialModelSpec.from_json`, `ModelBuilder`, `Evaluator.evaluate`, `parse_formula`, `parse_and_compile`, `normalize` |
| `TestStatementsAnalyticsBenchmarks` | Sensitivity, variance, scenario sets, goal seek, dependency tracing, `explain_formula` — each benched **twice**, once on the JSON path and once on the typed path, so the serialization overhead of the wire surface is directly visible. `backtest_forecast` is the one unpaired case |
| `TestPortfolioBenchmarks` | `Portfolio.from_materialization` (cold-unique, cold-dedup, warm-dedup) and `parse_portfolio_spec_json` / `Portfolio.from_spec` |
| `TestPortfolioCompoundWorkflow` | The realistic calling pattern (value + metrics + cashflows) over 500 positions, JSON path vs. typed `Portfolio`/`MarketContext` path |
| `TestPortfolioReleaseControls` | Release-scale controls: metrics attribution (40/120 positions), `scenario_pnl_batch` vs. repeated `scenario_pnl` (10/100 scenarios), 20-snapshot `replay_portfolio`, standard-risk valuation at 3,000 positions, PV-only valuation at 3,000 and 25,000 positions |
| `TestPortfolioRiskInputBenchmarks` | `parametric_var_decomposition` (256×256), `historical_var_decomposition` and `build_stress_attribution` (200×1,000), each with `list` and contiguous NumPy inputs — this pair exists to measure the zero-copy path, so keep both |
| `TestValuationsBenchmarks` | `validate_instrument_json`, `list_standard_metrics` |
| `TestScenariosBenchmarks` | Template registry, `parse_scenario_spec`, `validate_scenario_spec`, `build_scenario_spec`, `compose_scenarios` |

Uncovered domains: `attribution`, `cashflows`, `covenants`, `factor_model`,
`features`. The module docstring's claim of full domain coverage is stale.

## Running

From the repository root.

```bash
mise run python-bench                     # release build, then the whole perf suite
mise run python-bench-portfolio           # fixtures + release build, then -k Portfolio
```

Direct pytest, once a release extension is installed:

```bash
uv run pytest finstack-quant-py/benchmarks/bench_bindings.py -m "perf and not slow" --benchmark-only
uv run pytest finstack-quant-py/benchmarks/bench_bindings.py -m perf --benchmark-only -v
```

`testpaths` in `pyproject.toml` is `finstack-quant-py/tests`, and `python_files`
is `test_*.py` / `*_test.py`, so `bench_bindings.py` matches neither — it is
collected only when named explicitly on the command line. It never runs as part
of `mise run python-test`.

Import of `bench_bindings.py` is not free: module scope builds the 10,000-point
`Performance` panel, the 500-position spec, the market contexts, and two
evaluated statement models. Collection alone therefore does real work.

## The materialization path is the only gated benchmark

Three `TestPortfolioBenchmarks` cases and all of `materialization_measure.py`
measure `Portfolio.from_materialization` against two deterministic fixtures:

- `materialization-a-5000-unique.json` — 5,000 positions over 5,000 unique
  instrument artifacts (cold, no cache reuse).
- `materialization-b-5000-50.json` — 5,000 positions over 50 artifacts
  (measured cold and warm; the cache-hit case).

Neither fixture is checked in. Both are regenerated from
`cargo run --release -p finstack-quant-portfolio --example materialization_fixtures`,
which `mise run materialization-benchmark-fixtures` wraps. `bench_bindings.py`
shells out to that same command itself the first time a materialization fixture
is requested (`_regenerate_materialization_fixtures`, `lru_cache`d), so cargo must
be on `PATH` even for a pytest-only invocation.

Cache construction is deliberately outside every timer. In the pytest cases that
is `benchmark.pedantic(..., setup=...)`; in `materialization_measure.py` it is the
pre-built `caches` list. Do not move it inside.

Two environment variables gate the sample count, read identically by the pytest
suite, the CLI, the WASM bench, and the Rust bench:

| Variable | Effect |
|----------|--------|
| `FQ_MATERIALIZATION_P95_SAMPLES` | Sample/round count. Default 100. Must be a finite integer at or above the minimum, or import fails. |
| `FQ_MATERIALIZATION_SMOKE=1` | Lowers the minimum from 100 to 1. Short-run override for tests only — a record produced under it is invalid. |

## What counts as a regression

Only for materialization, and only through the gate tasks:

```bash
mise run python-bench-portfolio-baseline   # establish + seal (refuses if a baseline exists)
mise run python-bench-portfolio-compare    # fresh measurement vs. the sealed baseline
```

`compare` fails when a case's **median** regresses by more than 10% against
`benchmarks/materialization/materialization-python-baseline.json`. It also
rejects the run outright — before any timing comparison — on a wrong tree
revision, a mismatched fixture digest, a changed case set, or a stale
measurement file. The three compared cases are `cold_a_5000_unique`,
`cold_b_5000_50`, `warm_b_5000_50`.

Baselines are immutable. Replacing one needs
`FQ_REPLACE_MATERIALIZATION_BASELINE=1`.

Every other benchmark in this file is discovery-oriented: run it, read the
numbers, commit nothing. There is no gate and no tracked history for them.
Benchmarks do not run in PR CI.

## Adding a benchmark

- Put it in the class for its domain, or add a new `@pytest.mark.perf` class.
  Class names are `Test*`; pytest's `python_classes` requires the prefix.
- Mark anything over roughly a second `@pytest.mark.slow` so the
  `-m "perf and not slow"` selection stays usable.
- Build fixtures at module scope or in the test body, never inside the timed
  callable. Use `benchmark.pedantic(setup=...)` when per-round state is needed.
- Prefer benching the typed path and the JSON path as a pair when a binding
  offers both — that delta is the reason this suite exists.
- Ruff runs over this directory under `mise run python-lint`, with `T201`,
  `D100`–`D103`, `ANN`, and `S101` waived by a per-file ignore in
  `pyproject.toml`. Everything else applies.

## Related

- [`../README.md`](../README.md) — the Python package overview
- [`../../benchmarks/README.md`](../../benchmarks/README.md) — the checked-in
  performance records; only the materialization path is tracked there
- [`../../benchmarks/MATERIALIZATION_BENCHMARKS.md`](../../benchmarks/MATERIALIZATION_BENCHMARKS.md)
  — fixture definitions, timing boundaries, gates, hardware provenance
- [`../../scripts/README.md`](../../scripts/README.md) — the baseline, gating,
  and result-collection scripts behind the tasks above
- [`../../finstack-quant/portfolio/benches/README.md`](../../finstack-quant/portfolio/benches/README.md)
  — the Rust side of the same materialization measurement
- [`../../finstack-quant-wasm/benchmarks/README.md`](../../finstack-quant-wasm/benchmarks/README.md)
  — the Node/WASM sibling

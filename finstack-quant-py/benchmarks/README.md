# Python binding benchmarks

Micro- and workflow-level timings through the PyO3 bindings. These include Rust
calculation work plus argument conversion, GIL handling and wrapper construction.
They do not isolate binding overhead; compare with a matching Criterion workload
from `finstack-quant/*/benches/` only when fixtures and timing boundaries agree.

Two harnesses live here: a pytest-benchmark suite for discovery, and a standalone
CLI that emits raw latency samples for the one benchmark path that is gated
against a checked-in baseline (portfolio materialization).

## Layout

| Path                         | Role                                                                                                                                               |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `__init__.py`                | Package marker only. Makes the directory importable; holds no code.                                                                                |
| `bench_bindings.py`          | The pytest-benchmark suite. 13 `@pytest.mark.perf` classes covering 9 of the 14 binding domains.                                                   |
| `_ccm.py`                    | Private value checks, fixed batch sizes and provenance for the 22 core/correlation/model controls. No Finstack runtime is imported by this helper. |
| `materialization_measure.py` | Standalone CLI (not a pytest module). Writes one raw JSON fragment of release-Python materialization latencies for the regression gate.            |

Nothing in this directory is importable public API. `bench_bindings.py` is
collected by pytest; `materialization_measure.py` is invoked by path with two
positional arguments (fixture directory, output path) and is only ever run by
the `python`, `python-baseline` and `python-compare` steps of
`scripts/materialization_bench.sh`.

## Release build is mandatory

`maturin develop` without `--release` produces a debug extension. Timings from a
debug build are off by an order of magnitude and are not comparable to anything.

`mise run python-bench` handles this — it runs `mise run python-build -- --release`
(that is, `uv run maturin develop --release`) before invoking pytest. So do the
`python`, `python-baseline` and `python-compare` steps of
`scripts/materialization_bench.sh`.

The trap is the ordinary test loop: `mise run python-test` builds the _dev_
extension. If you then run pytest against `bench_bindings.py` by hand, you are
measuring that debug build. Re-run `mise run python-build -- --release` first, and
remember that the release extension stays installed afterwards — rebuild dev
before going back to test work if compile time matters to you.

## What the suite measures

Every class is marked `perf`. Selection within the suite is by class or by the
`slow` marker.

| Class                               | Covers                                                                                                                                                                                                                                                                                                                                       |
| ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `TestCoreBenchmarks`                | `Currency`, `Money` arithmetic, `DayCount.year_fraction`, `DiscountCurve.df`, `ForwardCurve.rate`, `FxMatrix.rate`, `Tenor.parse`, `Rate` conversions, `linalg.cholesky_decomposition`, `stats.mean`/`variance`                                                                                                                              |
| `TestAnalyticsBenchmarks`           | `Performance` over a 10,000-point two-series panel: `sharpe`, `sortino`, `calmar`, `volatility`, `returns`, `cumulative_returns`, drawdown series and details, VaR/ES, moments, `beta`, `tracking_error`, `rolling_sharpe`, `period_stats`. Construction (`Performance.from_panel`) is benched separately on a 252-point single-series panel |
| `TestCorrelationBenchmarks`         | `models.correlation`: `CopulaSpec`, `CorrelatedBernoulli`, `RecoverySpec`, `LatentFactorSpec`/`LatentSingleFactor`, `correlation_bounds`, `validate_correlation_matrix`                                                                                                                                                                      |
| `TestMonteCarloBenchmarks`          | Closed-form `models.bs_price`, European exact GBM at 10,000 independent/physical paths and 252 steps, arithmetic Asian at 5,000 independent/physical paths and 50 fixings, and Bermudan in-sample LSMC at 5,000 independent estimators / 10,000 antithetic physical paths and 50 exercise steps. Seed 42 and serial execution are explicit.  |
| `TestMarginBenchmarks`              | `CsaSpec.usd_regulatory`, `VmCalculator.calculate`, `NettingSetId`, `XvaConfig`, `FundingConfig`, `MarginUtilization`                                                                                                                                                                                                                        |
| `TestStatementsBenchmarks`          | `FinancialModelSpec.from_json`, `ModelBuilder`, `Evaluator.evaluate`, `parse_formula`, `parse_and_compile`, `normalize`                                                                                                                                                                                                                      |
| `TestStatementsAnalyticsBenchmarks` | Sensitivity, variance, scenario sets, goal seek, dependency tracing, `explain_formula` — each benched **twice**, once on the JSON path and once on the typed path, so the serialization overhead of the wire surface is directly visible. `backtest_forecast` is the one unpaired case                                                       |
| `TestPortfolioBenchmarks`           | `Portfolio.from_materialization` (cold-unique, cold-dedup, warm-dedup) and `parse_portfolio_spec_json` / `Portfolio.from_spec`                                                                                                                                                                                                               |
| `TestPortfolioCompoundWorkflow`     | Deposit-only value + metrics + cashflows over 500 positions, JSON inputs vs. typed `Portfolio`/`MarketContext` inputs; both return typed results                                                                                                                                                                                             |
| `TestPortfolioReleaseControls`      | Deposit-only controls: metrics attribution (40/120 deposits), `scenario_pnl_batch` vs. repeated `scenario_pnl` (10/100 scenarios over 500 deposits), 20-snapshot `replay_portfolio`, strict standard-risk valuation at 3,000 deposits, PV-only valuation at 3,000 and 25,000 deposits                                                        |
| `TestPortfolioRiskInputBenchmarks`  | `position_risk_report` over a parametric (256×256) and a historical (200×1,000) source, and `build_stress_attribution` (200×1,000) with `list` and contiguous NumPy inputs — that pair exists to measure the zero-copy path, so keep both                                                                                                    |
| `TestValuationsBenchmarks`          | Deposit JSON validation, `list_standard_metrics`, and serialize/deserialize round-trip of a successfully priced deposit `ValuationResult` with DV01                                                                                                                                                                                          |
| `TestScenariosBenchmarks`           | Template registry, `ScenarioSpec.from_json`, `validate_scenario_spec`, `ScenarioSpec` construction, `compose_scenarios`                                                                                                                                                                                                                      |

Uncovered domains: `attribution`, `cashflows`, `covenants`, `factor_model`,
`features`. The deposit portfolio controls do not establish mixed-instrument
valuation performance or cross-language workload parity.

Valuation and compound-workflow controls check complete position counts, finite
prices and requested DV01, `risk_metrics_complete`, and the absence of risk
errors or degraded positions before timing and on the returned measured result.
Portfolio risk requests explicitly use strict risk. Scenario and replay controls
check their requested result counts and finite values. Failures abort the case.
The deposit result round-trip prices once outside timing; each measured call
serializes that result and deserializes it through `ValuationResult.from_json`.
The JSON-validation row is validation only, with no pricing in its timer.

The valuation/portfolio release controls use five measured rounds and one warmup round, replacing
the former single-call observations. These are discovery timings, not enough
samples to claim stable tail latency. Short rows outside the checked core,
correlation and model classes use pytest-benchmark's
automatic repeated sampling. Changed case names and repaired workloads are new
baseline identities; old validation-only “valuation-result” timings are invalid.

## Checked core, correlation and model controls

The three selected classes contain exactly 22 required rows: ten core, seven
correlation and five model rows. Each performs an untimed preflight, validates
the complete final returned result, and requires the deterministic results to
match. Checks include independently calculated scalar references, full matrix
reconstruction, probability shapes/marginals, USD currencies and both independent
estimator and physical-path counts. Failures propagate; an unsuccessful or
missing row is not an accepted observation. Direct cashflows host coverage is
absent; the portfolio cashflow controls do not replace it.

Sampling is explicit for these rows only. `FQ_CCM_PY_ROUNDS` defaults to 10 and
must be a positive integer; `FQ_CCM_PY_WARMUP_ROUNDS` defaults to 1 and must be a
nonnegative integer. `FQ_BENCH_*` configures Criterion and has no effect here.
Fixed per-round iteration counts are frozen in `_ccm.py`: 512 for tiny scalar
queries, 256 for Money/FX/tenor/rate workflows, 128 for model construction and
queries, 64 for correlation-matrix validation, 16 for Cholesky, 8 for the
10,000-value statistics pair, and 1 for each MC workflow. Pytest-benchmark
normalizes its raw observations to seconds per operation. When iterations exceed
one it makes one extra untimed call to obtain the returned result. Warmup uses
the same operation and batch size. These short observations do not establish
tail latency or cross-language binding overhead.

Every selected row's `extra_info` contains the exact parameters and their hash,
batch/round counts, numerical bounds, timing/validation boundaries, full preflight
and final output, clocks and runtime/artifact identity. General scalar checks use
absolute and relative bounds of `1e-12`; Cholesky reconstruction uses `1e-11`
absolute, the deterministic statistics pair `1e-10` absolute, and Black-Scholes
against independent `math.erfc` arithmetic `1e-10` absolute. The fixed Gaussian
conditional-PD fixture uses `1e-11` absolute against `math.erfc`, matching the
owning CDF precision gate for the current `statrs` approximation. MC sanity guards use
six reported standard errors alongside exact counts/currencies and finite
confidence bands. These guards do not replace the owning model accuracy tests
or relax their financial tolerances.

Use `--benchmark-json` for complete raw observations; it includes all samples,
the normalized statistics and `extra_info`. Run three independent processes
against one proven release extension, using distinct output paths. For example,
after preparing the output directory and verifying the release build receipt:

```bash
RAYON_NUM_THREADS=8 FQ_CCM_PY_ROUNDS=10 FQ_CCM_PY_WARMUP_ROUNDS=1 uv run --no-sync pytest finstack-quant-py/benchmarks/bench_bindings.py -m perf -k 'CoreBenchmarks or CorrelationBenchmarks or MonteCarloBenchmarks' --benchmark-only --benchmark-json=target/core-cashflow-model-benchmarks/CCM-B2/python-r1.json
```

Repeat as separate `r2` and `r3` processes. Use `--benchmark-disable` for checked
execution without a timing baseline; the metadata explicitly labels that mode.
Existing valuation and materialization rows retain their previous protocols.

`FQ_CCM_SOURCE_IDENTITY` and `FQ_CCM_BUILD_RECEIPT` optionally point to the root
campaign's JSON records; their paths, hashes and payloads are retained. The
actual already-loaded `finstack_quant.finstack_quant` extension is hashed once
per process, together with the harness/helper and runtime versions. These hashes
prove identity, not optimization profile: an external successful release-build
receipt and matching Rust/runtime input hashes supply release provenance. The
harness does not infer release from filenames, modification times or speed, and
records `release_profile_verified_by_harness: false`. Reuse a proven unchanged
release artifact across processes; rebuild when its actual inputs change.

## Running

From the repository root.

```bash
mise run python-bench                    # release build, then the whole perf suite
scripts/materialization_bench.sh python  # fixtures + release build, then -k Portfolio
```

Direct pytest, once a release extension is installed:

```bash
uv run pytest finstack-quant-py/benchmarks/bench_bindings.py -m "perf and not slow" --benchmark-only
uv run pytest finstack-quant-py/benchmarks/bench_bindings.py -m perf --benchmark-only -v
```

After the release build, run only the valuation/deposit workflow validity checks
without collecting a timing baseline:

```bash
uv run pytest finstack-quant-py/benchmarks/bench_bindings.py -m perf -k "ValuationsBenchmarks or PortfolioCompoundWorkflow or PortfolioReleaseControls" --benchmark-disable
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
which `scripts/materialization_bench.sh fixtures` wraps. `bench_bindings.py`
shells out to that same command itself the first time a materialization fixture
is requested (`_regenerate_materialization_fixtures`, `lru_cache`d), so cargo must
be on `PATH` even for a pytest-only invocation.

Cache construction is deliberately outside every timer. In the pytest cases that
is `benchmark.pedantic(..., setup=...)`; in `materialization_measure.py` it is the
pre-built `caches` list. Do not move it inside.

Two environment variables gate the sample count, read identically by the pytest
suite, the CLI, the WASM bench, and the Rust bench:

| Variable                         | Effect                                                                                                       |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `FQ_MATERIALIZATION_P95_SAMPLES` | Sample/round count. Default 100. Must be a finite integer at or above the minimum, or import fails.          |
| `FQ_MATERIALIZATION_SMOKE=1`     | Lowers the minimum from 100 to 1. Short-run override for tests only — a record produced under it is invalid. |

## What counts as a regression

Only for materialization, and only through the gate steps:

```bash
scripts/materialization_bench.sh python-baseline  # establish + seal (refuses if a baseline exists)
scripts/materialization_bench.sh python-compare   # fresh measurement vs. the sealed baseline
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
  and result-collection scripts behind the steps above
- [`../../finstack-quant/portfolio/benches/README.md`](../../finstack-quant/portfolio/benches/README.md)
  — the Rust side of the same materialization measurement
- [`../../finstack-quant-wasm/benchmarks/README.md`](../../finstack-quant-wasm/benchmarks/README.md)
  — the Node/WASM sibling

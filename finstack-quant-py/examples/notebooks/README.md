# finstack-quant-py notebook examples

93 layered Jupyter notebooks covering the `finstack_quant` Python API, from core
types through pricing, analytics, statement modeling, portfolio and scenario
workflows, advanced quant methods, and reporting. They double as the executable
regression suite for the Python surface: `mise run python-examples` runs every
one of them.

## Prerequisites

- Python 3.12+
- `finstack_quant` built and installed (`mise run python-build` from the
  repository root; add `-- --release` before running the heavier levels)
- Dev dependencies synced (`mise run python-sync`), which brings in `jupyter`
  (and therefore `nbclient`/`nbformat`), `pyarrow`, and `polars`; `numpy` and
  `pandas` are runtime dependencies of the package itself

## Running

Interactive:

```bash
uv run jupyter lab finstack-quant-py/examples/notebooks
```

Batch, from the repository root:

```bash
mise run python-examples
# equivalently:
uv run python finstack-quant-py/examples/notebooks/run_all_notebooks.py
```

`run_all_notebooks.py` executes each notebook with `nbclient` over an IPC
kernel transport, with the notebook's own directory as the working directory,
and prints a per-notebook PASS/FAIL summary. It exits non-zero if any notebook
fails.

| Flag | Effect |
|------|--------|
| `--directory PATH` | Restrict to one subdirectory (recursively) or a single `.ipynb` file, resolved relative to the notebooks root |
| `--timeout N` | Per-notebook timeout in seconds (default 300) |
| `--fail-fast` | Stop after the first failure |
| `--save-outputs` | Write each *successful* notebook back with fresh outputs; failed notebooks are never written |
| `--verbose` | Dump full per-notebook output at the end |

```bash
uv run python finstack-quant-py/examples/notebooks/run_all_notebooks.py --directory 05_portfolio
uv run python finstack-quant-py/examples/notebooks/run_all_notebooks.py \
    --directory 02_pricing/pricing_fundamentals.ipynb --verbose
```

The runner prepends the notebooks root, `finstack-quant-py/`, and the
repository root to `PYTHONPATH` so `_shared` resolves without any per-notebook
setup. `mise run python-sync` and `mise run python-build` write the same
notebooks root into the virtualenv as `finstack_notebooks.pth`, so an
interactive kernel can `from _shared import ...` from any notebook directory.
Restart the kernel after installing or moving the checkout. A relative
`sys.path` insert (`".."` from a level directory, `"../.."` from a deep-dive
subdirectory) still works and is unnecessary once that path file is present.

## Curriculum structure

Two tiers:

- **Overview notebooks** sit directly in each numbered level directory and are
  meant to be read in order.
- **Deep-dive notebooks** sit in subdirectories and can be read on demand. They
  repeat enough setup to open standalone.

Prerequisites are stated inside each notebook as paths
(`01_foundations/core_types_and_money.ipynb`), not global numbers. The numbered
directories are the reading order.

## Shared helpers and data

Bulk payloads live in `data/<notebook_name>.json` next to the notebook that
uses them, loaded with a cwd-relative path (the runner sets cwd to the
notebook's directory):

```python
from pathlib import Path
import json

specs = json.loads(Path("data/portfolio_construction_and_valuation.json").read_text())
```

Keep one representative spec inline when the JSON shape is part of the lesson;
move catalogs and repeated payloads into the data file.

Cross-level reuse goes through the example-only `_shared` package, which sits
deliberately outside the public `finstack_quant` API:

| Module | Exports |
|--------|---------|
| `_shared.paths` | `NOTEBOOKS_ROOT`, `REPOSITORY_ROOT` (and `PYTHON_PACKAGE_ROOT`) |
| `_shared.market` | `DEMO_AS_OF`, `build_demo_market()`, `usd_ois_curve`, `usd_sofr_curve`, `usd_sofr_fixings`, `usd_ois_2026` — the canonical deterministic cross-asset `MarketContext`, including the SOFR historical fixings floating-rate examples need |
| `_shared.instrument_fixtures` | `acme_bond`, `instrument_envelope`, `instrument_envelope_json`, plus parameterized per-asset-class factories used by the scale lab |
| `_shared.synthetic` | `random_walk_panel`, `demo_pl_builder`, `demo_pl_model` — deterministic synthetic panels and statement models |
| `_shared.notebook_helpers` | `banner`, `print_metrics`, `series` — presentation only; they never wrap `finstack_quant` logic |

Import via the package root: `from _shared import build_demo_market`.

`finstack-quant-py/tests/test_notebook_instrument_fixtures.py` deserializes
every instrument factory in `_shared.instrument_fixtures` through the canonical
serde path, so a fixture that drifts from the wire contract fails in CI rather
than in a notebook.

## Notebook hygiene rules

`finstack-quant-py/tests/test_notebook_hygiene.py` parses every code cell and
fails the build on constructs that hide API breakage. It also validates
nbformat and compiles every code cell.

| Rule | What it rejects |
|------|-----------------|
| `broad-catch` | `except:` / `except Exception:` / `except BaseException:` (including `except*`). Catch the exact documented exception instead. |
| `first-party-import-soft-fail` | `try: import finstack_quant / except ImportError: ...`. First-party imports must fail visibly. |
| `public-api-probe` | `hasattr(<finstack object>, ...)` or 3-argument `getattr(<finstack object>, ..., default)`. Call the API directly. |

The scanner tracks name bindings across cells, so aliasing (`CatchAll =
Exception`) does not evade it. The only exemption is a deliberately failing
teaching cell tagged `intentional-negative`, recorded in the test's
`ALLOWLIST`, which is currently empty.

## Level 1 — Foundations (`01_foundations/`)

| Path | Topics |
|------|--------|
| `core_types_and_money.ipynb` | `Currency`, `Money`, `Rate`, `Bps`, `Percentage`, `CreditRating` |
| `dates_calendars_schedules.ipynb` | `DayCount`, `Tenor`, `PeriodId`, `HolidayCalendar`, `ScheduleBuilder` |
| `market_data_and_curves.ipynb` | `DiscountCurve`, `ForwardCurve`, `HazardCurve`, `FxMatrix`, `MarketContext` |
| `math_toolkit.ipynb` | Linear algebra, statistics, special functions, compensated summation |
| `registry_defaults_and_overrides.ipynb` | `FinstackConfig` extensions, registry override payloads, JSON round-tripping |
| `market_bootstrap_tour.ipynb` | Building a `MarketContext` from raw quotes: calibration envelopes, `calibrate`, diagnostics |

Deep dives:

- `dates/` (3): day-count conventions, holiday calendars and business-day
  adjustment, schedule building.
- `market_data/` (10): discount, forward, hazard, inflation, price and
  volatility-index curves; FX matrix; volatility surfaces; SABR smiles and
  calibration; dynamic term structure (`models.rates.dtsm`).

## Level 2 — Instrument pricing (`02_pricing/`)

| Path | Topics |
|------|--------|
| `pricing_fundamentals.ipynb` | Instrument JSON envelope, `MarketContext`, `ValuationResult`, model keys, metrics, valuation caching |
| `pricing_across_asset_classes.ipynb` | Deposits, swaps, CDS, equity options, FX options, exotics |
| `pnl_attribution.ipynb` | Decomposing daily MTM changes by risk factor |

Deep dives — `instruments/` (15): complex cashflows, bonds and fixed income,
rates derivatives (deposits / FRA / IRS), FX spot and forwards, equity options,
credit derivatives (CDS), credit events and restructuring, structured credit,
inflation-linked bonds, loans and credit facilities, repo and secured
financing, convertible bonds, Fourier pricing, exotic rates, total-return and
variance swaps.

## Level 3 — Performance and risk analytics (`03_analytics/`)

| Path | Topics |
|------|--------|
| `performance_analytics.ipynb` | `Performance`, CAGR, Sharpe, drawdowns, rolling metrics |
| `risk_and_factor_analytics.ipynb` | VaR, factor regression, capture ratios, ruin estimation |
| `factor_sensitivity.ipynb` | Factor sensitivities and risk decomposition for portfolio positions |
| `feature_transforms.ipynb` | Cross-sectional ranking, time-series transforms, grouped normalization |
| `breakeven_analysis.ipynb` | Spread and carry breakevens from `cs01`, `dv01`, and carry metrics |
| `return_contribution.ipynb` | Single-period return contribution, group/factor decomposition, Brinson-Fachler, weighting modes |
| `portfolio_returns_and_attribution.ipynb` | TWRR/MWRR, multi-period Brinson-Fachler, Carino linking |

Tear sheets that render these analytics live in `09_reporting/`.

## Level 4 — Financial statement modeling (`04_statement_modeling/`)

| Path | Topics |
|------|--------|
| `statement_modeling.ipynb` | `ModelBuilder`, `Evaluator`, DSL formulas, DataFrame export |
| `statement_analytics.ipynb` | Sensitivity, tornado, variance, goal-seek, dependency tracing |

Deep dives — `models/` (11): three-statement linked model, DCF from statement
UFCF, debt waterfall and cash sweep, LBO sources and uses, credit ratios,
covenant monitoring, IFRS 9 / CECL ECL, credit scoring and PD calibration,
normalization and adjusted EBITDA, real-estate and roll-forward templates,
comparable-company analysis.

## Level 5 — Portfolio (`05_portfolio/`)

| Path | Topics |
|------|--------|
| `portfolio_construction_and_valuation.ipynb` | Portfolio spec, valuation, aggregation, cashflow ladder |
| `portfolio_optimization.ipynb` | JSON and typed optimization specs, objectives, constraints, trade universes |
| `horizon_total_return.ipynb` | Carry plus scenario P&L composition, factor-decomposed total return |
| `historical_replay.ipynb` | Replay a portfolio through dated market snapshots; P&L and attribution |
| `liquidity_risk.ipynb` | Roll spread, Amihud illiquidity, days-to-liquidate, Bangia LVaR, Almgren-Chriss impact |
| `portfolio_risk_decomposition.ipynb` | Euler VaR/ES decomposition, risk budgeting, capital allocation, factor engines |
| `credit_factor_hierarchy.ipynb` | Credit factor levels, hierarchy assignments, period decomposition |
| `multi_asset_portfolio_at_scale.ipynb` | Optional scale/throughput lab for larger multi-asset books |

## Level 6 — Scenarios (`06_scenarios/`)

Scenarios are a first-class layer: build market shocks once, then apply them to
markets, portfolios, reporting, or the capstone workflow.

| Path | Topics |
|------|--------|
| `scenarios_and_stress_testing.ipynb` | Templates, composition, market application, portfolio revaluation |
| `rate_scenarios.ipynb` | Rate-curve parallel, node, and composed shocks |
| `credit_scenarios.ipynb` | Credit-spread, hazard, and default-stress examples |
| `composite_stress_tests.ipynb` | Multi-factor macro and market stress packages |
| `scenario_impact_analysis.ipynb` | Impact analysis and ranking across scenario outputs |

## Level 7 — Advanced quantitative methods (`07_advanced_quant/`)

| Path | Topics |
|------|--------|
| `monte_carlo_simulation.ipynb` | Canonical Monte Carlo pricers and Black-Scholes benchmarks |
| `correlation_and_credit_models.ipynb` | Copulas, recovery models, factor models, correlated Bernoulli |
| `margin_collateral_and_xva.ipynb` | CSA specs, VM/IM, XVA, collateral analytics |
| `regulatory_capital.ipynb` | FRTB SA, SA-CCR, initial-margin methodologies |

Deep dives:

- `monte_carlo/` (4): Black-Scholes benchmarks vs Monte Carlo, stochastic
  processes for equity and rates, discretization schemes (Euler, Milstein,
  exact GBM), exotic payoffs (European, Asian, American via LSMC).
- `correlation/` (4): portfolio default simulation, recovery modeling and tail
  risk, CLO tranche modeling, structural credit models (Merton equity ↔
  credit).

## Level 8 — Capstone (`08_capstone/`)

| Path | Topics |
|------|--------|
| `end_to_end_credit_portfolio_workflow.ipynb` | One linear pass over the whole stack: market setup, credit instrument pricing, risk metrics, portfolio aggregation, scenarios, P&L attribution, statement modeling, and JSON / dependency-trace export |

## Level 9 — Reporting (`09_reporting/`)

Rendering demos for `finstack_quant.reporting`, the pure-Python presentation
layer. Read the corresponding analytics or modeling notebook first, then use
these to learn how to package results for review.

| Path | API |
|------|-----|
| `reporting_statement_tearsheet.ipynb` | `statement_tearsheet` — P&L summary, margins, variance vs plan |
| `reporting_credit_tearsheet.ipynb` | `credit_tearsheet` — leverage/coverage trend, per-instrument coverage, covenant compliance |
| `reporting_dcf_tearsheet.ipynb` | `dcf_tearsheet` — EV-to-equity bridge, UFCF projection, WACC / terminal-growth sensitivity |
| `reporting_scenario_tearsheet.ipynb` | `scenario_tearsheet` — driver tornado, scenario comparison, Monte Carlo fan, variance |
| `reporting_portfolio_tearsheet.ipynb` | `portfolio_tearsheet` — holdings, exposure by entity, aggregated sensitivities, cashflow ladder |
| `reporting_portfolio_risk_tearsheet.ipynb` | `portfolio_risk_tearsheet` — Euler VaR/ES contributions and risk budget |
| `reporting_benchmark_tearsheet.ipynb` | `benchmark_tearsheet` — alpha/beta, capture, rolling greeks, relative series, multi-factor |
| `reporting_instrument_tearsheet.ipynb` | `instrument_tearsheet` — instrument valuation tear sheet |
| `reporting_performance_tearsheet.ipynb` | `performance_tearsheet` — performance analytics tear sheet |
| `reporting_attribution_tearsheet.ipynb` | `attribution_tearsheet` — return and P&L attribution tear sheet |

## Related

- [`../README.md`](../README.md) — the rest of `examples/`: standalone scripts
  and the reports they produce
- [`../../README.md`](../../README.md) — `finstack-quant-py` package overview,
  namespaces, and conventions
- [`../../DOCS_STYLE.md`](../../DOCS_STYLE.md) — docstring and stub standards

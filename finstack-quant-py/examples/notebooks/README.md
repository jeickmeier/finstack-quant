# finstack-quant-py Notebook Examples

Layered Jupyter notebooks for learning quantitative finance workflows and the
`finstack_quant` Python API, from core data types through pricing, analytics,
portfolio/scenario workflows, advanced quant methods, and reporting.

## Prerequisites

- Python 3.12+
- `finstack_quant` built and installed (`mise run python-build`)
- Project dependencies installed from the repository root `pyproject.toml`

## How To Run

Interactive use:

```bash
uv run jupyter lab finstack-quant-py/examples/notebooks
```

Notebooks that need shared helpers add the notebooks root with a relative
`sys.path` insert (`".."` or `"../.."` depending on depth), then
`from _shared import ...`. The batch runner also puts that directory on
`PYTHONPATH`.

Batch execution from the repository root:

```bash
mise run python-examples
# or:
uv run python finstack-quant-py/examples/notebooks/run_all_notebooks.py
```

One section:

```bash
uv run python finstack-quant-py/examples/notebooks/run_all_notebooks.py --directory 01_foundations
```

## How The Curriculum Is Organized

The curriculum has two tiers:

- **Overview notebooks** live directly in each numbered level directory and are
  intended to be read in the order listed below.
- **Deep-dive notebooks** live in subdirectories and can be read when you need
  the topic. They repeat some setup from the overview notebooks so they can also
  be opened independently.

Prerequisites are written as notebook paths, not global notebook numbers. The
numbered directories are the reading order.

## Notebook Data Files

Large example payloads live beside notebooks in `data/*.json` files, or in
`_shared/` when reused across levels. Notebook code loads these files with
cwd-relative paths such as:

```python
from pathlib import Path
import json

specs = json.loads(Path("data/example_specs.json").read_text())
```

Keep one representative spec visible in the notebook when the JSON shape is part
of the lesson; move bulk catalogs and repeated payloads into data files.

Reusable examples live in the example-only `_shared` package (it is deliberately
outside the public `finstack_quant` API):

- `_shared.paths` provides stable repository, package, notebook, and fixture
  paths.
- `_shared.market.build_demo_market()` returns the canonical typed market,
  including SOFR historical fixings used by floating-rate examples.
- `_shared.fixtures` loads independent copies of named instrument and portfolio
  payloads from versioned `_shared/data/v1/` catalogs.
- `_shared.instrument_fixtures` contains the larger, parameterized instrument
  factories used by the multi-asset scale lab.

Import shared examples with `from _shared import ...`. New fixture schemas must
use a new version directory rather than silently changing an existing wire
contract.

## Level 1 -- Foundations (`01_foundations/`)

| Path | Topics |
|------|--------|
| `01_foundations/core_types_and_money.ipynb` | Currency, Money, Rate, Bps, Percentage, CreditRating, Attributes |
| `01_foundations/dates_calendars_schedules.ipynb` | DayCount, Tenor, PeriodId, HolidayCalendar, ScheduleBuilder |
| `01_foundations/market_data_and_curves.ipynb` | DiscountCurve, ForwardCurve, HazardCurve, FxMatrix, MarketContext |
| `01_foundations/math_toolkit.ipynb` | Linear algebra, statistics, special functions, compensated summation |
| `01_foundations/registry_defaults_and_overrides.ipynb` | FinstackConfig extensions, registry override payloads, JSON round-tripping |
| `01_foundations/market_bootstrap_tour.ipynb` | Calibration envelopes, `calibrate`, market snapshots, diagnostics |

Deep dives:

- `01_foundations/dates/` (3 notebooks): day counts, holiday calendars, schedule building.
- `01_foundations/market_data/` (10 notebooks): discount/forward/hazard/inflation/price curves, FX matrices, vol surfaces/cubes, SABR, dynamic term structure.

## Level 2 -- Instrument Pricing (`02_pricing/`)

| Path | Topics |
|------|--------|
| `02_pricing/pricing_fundamentals.ipynb` | Instrument JSON, ValuationResult, model keys, metrics, report components, valuation caching |
| `02_pricing/pricing_across_asset_classes.ipynb` | Deposits, swaps, CDS, equity options, FX options, exotics |
| `02_pricing/pnl_attribution.ipynb` | Pricing-based P&L attribution and explanation workflows |

Deep dives:

- `02_pricing/instruments/`: bonds, rates derivatives, FX, equities/options, credit derivatives, structured credit, inflation-linked products, loans, repos, cashflows, convertibles, Fourier pricing, exotic rates, credit events, and total-return/variance swaps.

## Level 3 -- Performance And Risk Analytics (`03_analytics/`)

| Path | Topics |
|------|--------|
| `03_analytics/performance_analytics.ipynb` | Performance class, CAGR, Sharpe, drawdowns, rolling metrics |
| `03_analytics/risk_and_factor_analytics.ipynb` | VaR, factor regression, capture ratios, ruin estimation |
| `03_analytics/factor_sensitivity.ipynb` | Factor sensitivities and risk decomposition for portfolio positions |
| `03_analytics/feature_transforms.ipynb` | Cross-sectional ranking, time-series transforms, grouped normalization |
| `03_analytics/breakeven_analysis.ipynb` | Spread/carry breakevens from cs01, dv01, and carry metrics |
| `03_analytics/return_contribution.ipynb` | Single-period return contribution, group/factor decomposition, Brinson-Fachler, weighting modes |
| `03_analytics/portfolio_returns_and_attribution.ipynb` | TWRR/MWRR, multi-period Brinson-Fachler, Carino linking |

Reporting tear sheets that render many of these analytics live in
`09_reporting/`.

## Level 4 -- Financial Statement Modeling (`04_statement_modeling/`)

| Path | Topics |
|------|--------|
| `04_statement_modeling/statement_modeling.ipynb` | ModelBuilder, Evaluator, DSL formulas, Polars export |
| `04_statement_modeling/statement_analytics.ipynb` | Sensitivity, tornado, variance, goal-seek, dependency tracing |

Deep dives:

- `04_statement_modeling/models/` (11 notebooks): three-statement models, DCF, debt waterfalls, LBO, credit analysis, covenant monitoring, IFRS 9 / CECL ECL, credit scoring / PD, normalization, real-estate and roll-forward templates, comparable-company analysis.

## Level 5 -- Portfolio (`05_portfolio/`)

| Path | Topics |
|------|--------|
| `05_portfolio/portfolio_construction_and_valuation.ipynb` | Portfolio spec, valuation, aggregation, cashflow ladder |
| `05_portfolio/portfolio_optimization.ipynb` | JSON and typed optimization specs, objectives, constraints, trade universes |
| `05_portfolio/horizon_total_return.ipynb` | Carry plus scenario P&L composition, factor-decomposed total return |
| `05_portfolio/historical_replay.ipynb` | Replay portfolio through dated market snapshots, P&L, attribution |
| `05_portfolio/liquidity_risk.ipynb` | Roll spread, Amihud illiquidity, days-to-liquidate, Bangia LVaR, Almgren-Chriss impact |
| `05_portfolio/portfolio_risk_decomposition.ipynb` | Euler VaR/ES decomposition, risk budgeting, capital allocation |
| `05_portfolio/credit_factor_hierarchy.ipynb` | Credit factor levels, hierarchy assignments, period decomposition |
| `05_portfolio/multi_asset_portfolio_at_scale.ipynb` | Optional scale/throughput lab for larger multi-asset books |

## Level 6 -- Scenarios (`06_scenarios/`)

Scenarios are a first-class layer: build market shocks once, then apply them to
markets, portfolios, reporting, or capstone workflows.

| Path | Topics |
|------|--------|
| `06_scenarios/scenarios_and_stress_testing.ipynb` | Templates, composition, market application, portfolio revaluation |
| `06_scenarios/rate_scenarios.ipynb` | Rate-curve parallel, node, and composed rate shocks |
| `06_scenarios/credit_scenarios.ipynb` | Credit-spread, hazard, and default-stress examples |
| `06_scenarios/composite_stress_tests.ipynb` | Multi-factor macro and market stress packages |
| `06_scenarios/scenario_impact_analysis.ipynb` | Impact analysis and ranking across scenario outputs |

## Level 7 -- Advanced Quantitative Methods (`07_advanced_quant/`)

| Path | Topics |
|------|--------|
| `07_advanced_quant/monte_carlo_simulation.ipynb` | TimeGrid, McEngine, EuropeanPricer, Black-Scholes benchmarks |
| `07_advanced_quant/correlation_and_credit_models.ipynb` | Copulas, recovery models, factor models, correlated Bernoulli |
| `07_advanced_quant/margin_collateral_and_xva.ipynb` | CSA, VM/IM, XVA, collateral analytics |
| `07_advanced_quant/regulatory_capital.ipynb` | FRTB SBA, SA-CCR, initial-margin methodologies |

Deep dives:

- `07_advanced_quant/monte_carlo/` (4 notebooks): Black-Scholes benchmarks, stochastic processes, discretization schemes, exotic payoffs and pricers.
- `07_advanced_quant/correlation/` (4 notebooks): portfolio default simulation, recovery modeling, CLO tranche modeling, structural credit models.

## Level 8 -- Capstone (`08_capstone/`)

| Path | Topics |
|------|--------|
| `08_capstone/end_to_end_credit_portfolio_workflow.ipynb` | Integrated credit portfolio workflow across statements, pricing, scenarios, analytics, and reporting inputs |

## Level 9 -- Reporting (`09_reporting/`)

These notebooks are rendering demos for the reporting API. Read the source
analytics/modeling notebooks first, then use the reporting notebooks to learn
how to package results for review.

| Path | Topics |
|------|--------|
| `09_reporting/reporting_statement_tearsheet.ipynb` | P&L summary, margins, variance-vs-plan (`statement_tearsheet`) |
| `09_reporting/reporting_credit_tearsheet.ipynb` | Leverage/coverage trend, per-instrument coverage, covenant compliance (`credit_tearsheet`) |
| `09_reporting/reporting_dcf_tearsheet.ipynb` | EV-to-equity bridge, UFCF projection, WACC/terminal-growth sensitivity (`dcf_tearsheet`) |
| `09_reporting/reporting_scenario_tearsheet.ipynb` | Driver tornado, scenario comparison, Monte Carlo fan, variance (`scenario_tearsheet`) |
| `09_reporting/reporting_portfolio_tearsheet.ipynb` | Holdings, exposure by entity, aggregated sensitivities, cashflow ladder (`portfolio_tearsheet`) |
| `09_reporting/reporting_portfolio_risk_tearsheet.ipynb` | Euler VaR/ES contributions and risk budget (`portfolio_risk_tearsheet`) |
| `09_reporting/reporting_benchmark_tearsheet.ipynb` | Alpha/beta, capture, rolling greeks, relative series, multi-factor (`benchmark_tearsheet`) |
| `09_reporting/reporting_instrument_tearsheet.ipynb` | Generic instrument valuation tear sheet |
| `09_reporting/reporting_performance_tearsheet.ipynb` | Performance analytics tear sheet |
| `09_reporting/reporting_attribution_tearsheet.ipynb` | Return and P&L attribution tear sheet |

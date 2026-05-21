# finstack Python notebook examples

Layered Jupyter notebooks for the `finstack` package: core types through portfolio,
scenarios, and advanced credit/Monte Carlo workflows.

## Prerequisites

- Python 3.12+
- `finstack` built and installed (`mise run python-build`)
- Project dependencies installed from the repository root `pyproject.toml`

## How to Run

**Interactive** -- open individual notebooks in JupyterLab:

```bash
uv run jupyter lab
```

**Batch** — execute all notebooks and report pass/fail (from repo root):

```bash
mise run python-examples
# or:
uv run python finstack-py/examples/notebooks/run_all_notebooks.py
```

One section:

```bash
uv run python finstack-py/examples/notebooks/run_all_notebooks.py --directory 01_foundations
```

## Curriculum Structure

The curriculum has two tiers: **section overview notebooks** (read in the order
listed below) and **deep-dive sub-directories** (jump to as needed).

### Level 1 -- Foundations (`01_foundations/`)

| Notebook | Topics |
|----------|--------|
| Core Types and Money | Currency, Money, Rate, Bps, Percentage, CreditRating, Attributes |
| Dates, Calendars, Schedules | DayCount, Tenor, PeriodId, HolidayCalendar, ScheduleBuilder |
| Market Data and Curves | DiscountCurve, ForwardCurve, HazardCurve, FxMatrix, MarketContext |
| Math Toolkit | Linear algebra, statistics, special functions, compensated summation |
| Registry Defaults and Overrides | FinstackConfig extensions, registry override payloads, JSON round-tripping |

Deep dives: `market_data/` (9 notebooks, including dynamic term structure), `dates/` (3 notebooks)

### Level 2 -- Instrument Pricing (`02_pricing/`)

| Notebook | Topics |
|----------|--------|
| Pricing Fundamentals | Instrument JSON, ValuationResult, model keys, metrics, report components, valuation caching |
| Pricing Across Asset Classes | Deposit, IRS, CDS, equity option, FX option, exotic |
| PnL Attribution | Attribution workflows and explain |

Deep dives: `instruments/` (15 notebooks, including credit events, Fourier pricing, and exotic rates)

### Level 3 -- Performance and Risk Analytics (`03_analytics/`)

| Notebook | Topics |
|----------|--------|
| Performance Analytics | Performance class, CAGR, Sharpe, drawdowns, rolling metrics |
| Risk and Factor Analytics | VaR, factor regression, capture ratios, ruin estimation |
| GARCH Volatility | GARCH, EGARCH, GJR-GARCH fitting, variance forecasts, residual diagnostics |

### Level 4 -- Financial Statement Modeling (`04_statement_modeling/`)

| Notebook | Topics |
|----------|--------|
| Statement Modeling | ModelBuilder, Evaluator, DSL formulas, Polars export |
| Statement Analytics | Sensitivity, tornado, variance, goal-seek, dependency tracing |

Deep dives: `models/` (10 notebooks, including IFRS 9 / CECL ECL, credit scoring / PD, and comparable-company analysis)

### Level 5 -- Portfolio and Scenarios (`05_portfolio_and_scenarios/`)

| Notebook | Topics |
|----------|--------|
| Portfolio Construction and Valuation | Portfolio spec, valuation, aggregation, cashflow ladder |
| Scenarios and Stress Testing | Templates, composition, application, revaluation |
| Horizon Total Return | Carry + scenario P&L composition, factor-decomposed total return |
| Historical Replay | Replay portfolio through dated market snapshots, P&L, attribution |
| Liquidity Risk | Roll spread, Amihud illiquidity, days-to-liquidate, Bangia LVaR, Almgren-Chriss impact |

Deep dives: `scenarios/` (4 notebooks)

### Level 6 -- Advanced Quantitative Methods (`06_advanced_quant/`)

| Notebook | Topics |
|----------|--------|
| Monte Carlo Simulation | TimeGrid, McEngine, EuropeanPricer, Black-Scholes benchmarks |
| Correlation and Credit Models | Copulas, recovery models, factor models, correlated Bernoulli |
| Margin, Collateral, and XVA | CSA, VM/IM, XVA, collateral analytics |

Deep dives: `monte_carlo/` (4 notebooks), `correlation/` (3 notebooks)

### Level 7 -- Capstone (`07_capstone/`)

| Notebook | Topics |
|----------|--------|
| End-to-End Credit Portfolio Workflow | Integrates all modules into a realistic workflow |

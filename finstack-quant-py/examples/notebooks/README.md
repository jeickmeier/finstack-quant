# finstack-quant-py Python notebook examples

Layered Jupyter notebooks for the `finstack_quant` package: core types through portfolio,
scenarios, and advanced credit/Monte Carlo workflows.

## Prerequisites

- Python 3.12+
- `finstack_quant` built and installed (`mise run python-build`)
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
uv run python finstack-quant-py/examples/notebooks/run_all_notebooks.py
```

One section:

```bash
uv run python finstack-quant-py/examples/notebooks/run_all_notebooks.py --directory 01_foundations
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

Deep dives: `market_data/` (10 notebooks, including SABR volatility smiles and dynamic term structure), `dates/` (3 notebooks)

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
| Factor Sensitivity | Factor sensitivities and risk decomposition for portfolio positions |
| Feature Transforms | Cross-sectional ranking, time-series transforms, grouped normalization |
| Breakeven Analysis | Spread / carry breakevens from cs01, dv01, and carry metrics |
| Return-Contribution Attribution | Single-period return contribution, group/factor decomposition, Brinson-Fachler, weighting modes |
| Portfolio Returns and Attribution | Time- and money-weighted returns (TWRR/MWRR), multi-period Brinson-Fachler, Carino linking |
| Reporting: Statement Tear Sheet | P&L summary, margins, variance-vs-plan (`statement_tearsheet`) |
| Reporting: Credit Tear Sheet | Leverage/coverage trend, per-instrument coverage, covenant compliance (`credit_tearsheet`) |
| Reporting: DCF Tear Sheet | EV→equity bridge, UFCF projection, WACC/terminal-growth sensitivity (`dcf_tearsheet`) |
| Reporting: Scenario Tear Sheet | Driver tornado, scenario comparison, Monte-Carlo fan, variance (`scenario_tearsheet`) |
| Reporting: Portfolio Tear Sheet | Holdings, exposure by entity, aggregated sensitivities, cashflow ladder (`portfolio_tearsheet`) |
| Reporting: Portfolio Risk Tear Sheet | Euler VaR/ES contributions and risk budget (`portfolio_risk_tearsheet`) |

### Level 4 -- Financial Statement Modeling (`04_statement_modeling/`)

| Notebook | Topics |
|----------|--------|
| Statement Modeling | ModelBuilder, Evaluator, DSL formulas, Polars export |
| Statement Analytics | Sensitivity, tornado, variance, goal-seek, dependency tracing |

Deep dives: `models/` (11 notebooks, including real-estate and roll-forward templates, IFRS 9 / CECL ECL, credit scoring / PD, and comparable-company analysis)

### Level 5 -- Portfolio and Scenarios (`05_portfolio_and_scenarios/`)

| Notebook | Topics |
|----------|--------|
| Portfolio Construction and Valuation | Portfolio spec, valuation, aggregation, cashflow ladder |
| Scenarios and Stress Testing | Templates, composition, application, revaluation |
| Horizon Total Return | Carry + scenario P&L composition, factor-decomposed total return |
| Historical Replay | Replay portfolio through dated market snapshots, P&L, attribution |
| Liquidity Risk | Roll spread, Amihud illiquidity, days-to-liquidate, Bangia LVaR, Almgren-Chriss impact |
| Portfolio Risk Decomposition | Euler VaR/ES decomposition (parametric & historical), risk budgeting, capital allocation |

Deep dives: `scenarios/` (4 notebooks)

### Level 6 -- Advanced Quantitative Methods (`06_advanced_quant/`)

| Notebook | Topics |
|----------|--------|
| Monte Carlo Simulation | TimeGrid, McEngine, EuropeanPricer, Black-Scholes benchmarks |
| Correlation and Credit Models | Copulas, recovery models, factor models, correlated Bernoulli |
| Margin, Collateral, and XVA | CSA, VM/IM, XVA, collateral analytics |
| Regulatory Capital | FRTB SBA market-risk capital, SA-CCR counterparty EAD, initial-margin methodology |

Deep dives: `monte_carlo/` (4 notebooks), `correlation/` (3 notebooks)

### Level 7 -- Capstone (`07_capstone/`)

| Notebook | Topics |
|----------|--------|
| End-to-End Credit Portfolio Workflow | Integrates all modules into a realistic workflow |

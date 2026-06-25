# Finstack Quant

![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)
![Rust](https://img.shields.io/badge/rust-1.90%2B-orange)
![Python](https://img.shields.io/badge/python-3.12%2B-blue)
![WASM](https://img.shields.io/badge/wasm-ready-purple)
![Status](https://img.shields.io/badge/status-alpha-yellow)

High-performance quantitative finance primitives, pricing, portfolio analytics,
and scenario tooling written in Rust, with Python and WebAssembly bindings.

Finstack Quant is for developers, researchers, and investment teams who want one
deterministic financial computation engine that can run in Rust services,
Python notebooks, and browser or Node applications.

## What You Can Do With It

- Build currency-safe financial models using Rust `Decimal`-backed primitives.
- Price instruments across rates, credit, FX, equity options, structured credit,
  private markets, and structured products.
- Run portfolio analytics, risk decomposition, attribution, stress testing, and
  liquidity-aware scenario analysis.
- Model financial statements, forecasts, sensitivities, covenants, and credit
  workflows.
- Reuse the same core logic from Rust, Python, or WebAssembly.
- Teach, prototype, and validate workflows through the included notebook
  curriculum.

## Why Finstack Quant?

Most financial analytics code ends up split across Python notebooks, backend
services, spreadsheets, and web applications. Finstack Quant keeps the financial
logic in one Rust core and exposes it through Python and WebAssembly so the same
calculations can run in research, production, and interactive applications.

Design goals:

- Deterministic financial calculations.
- Currency-safe and accounting-aware primitives.
- Strongly typed APIs with documented public surfaces.
- Cross-platform reuse across Rust, Python, and JavaScript.
- Useful coverage for public markets, private credit, portfolio construction,
  scenario analysis, and risk.

## Quick Start: Python

```bash
git clone https://github.com/jeickmeier/finstack-quant.git
cd finstack-quant
mise install
mise run python-build
uv run python
```

```python
from datetime import date

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import BusinessDayConvention, HolidayCalendar, adjust
from finstack_quant.core.money import Money

usd = Currency("USD")
amount = Money(1_000_000, usd)

settle = adjust(
    date(2025, 1, 4),
    BusinessDayConvention.FOLLOWING,
    HolidayCalendar("usny"),
)

print(amount.format())
print(settle)
```

## Learn By Example

The Python notebook curriculum walks through:

1. Foundations: money, dates, curves, market data, math, registry defaults.
2. Pricing: deposits, swaps, CDS, equity options, FX options, exotics,
   attribution.
3. Analytics: performance, VaR, factor regression, return attribution, reporting
   tear sheets.
4. Statement modeling: formulas, forecasts, sensitivities, covenants, credit
   scoring.
5. Portfolio and scenarios: aggregation, stress testing, liquidity, risk
   decomposition.
6. Advanced quant: Monte Carlo, correlation, margin, XVA, regulatory capital.
7. Capstone: an end-to-end credit portfolio workflow.

Start with
[`finstack-quant-py/examples/notebooks/README.md`](finstack-quant-py/examples/notebooks/README.md).
Run every notebook with:

```bash
mise run python-examples
```

## Architecture

Finstack Quant is a Rust-first quantitative finance workspace with Python and
WebAssembly bindings. The repository is organized around reusable financial
primitives, pricing and risk engines, statement modeling, deterministic scenario
tooling, portfolio analytics, and thin binding layers that keep business logic
in Rust.

## Workspace Layout

```text
finstack-quant/
├── finstack-quant/
│   ├── Cargo.toml                 # `finstack-quant` umbrella crate
│   ├── src/                       # Feature-gated re-exports
│   ├── core/                      # Dates, money, market data, math, expressions
│   ├── cashflows/                 # Schedule construction and cashflow aggregation
│   ├── analytics/                 # Return-series performance and risk analytics
│   ├── monte_carlo/               # Simulation engine, processes, payoffs, pricers
│   ├── margin/                    # Margin, collateral, and XVA primitives
│   ├── statements/                # Financial statement modeling and evaluation
│   ├── statements-analytics/      # Higher-level statement analytics and reporting
│   ├── valuations/                # Instruments, pricing, metrics, calibration
│   ├── portfolio/                 # Portfolio valuation, grouping, optimization
│   └── scenarios/                 # Scenario composition and application
├── finstack-quant-py/             # PyO3 bindings packaged as `finstack-quant-py`
├── finstack-quant-wasm/           # wasm-bindgen bindings packaged as `finstack-quant-wasm`
├── docs/                          # References, standards, reviews, and design notes
├── pyproject.toml                 # Python packaging and tooling
├── Cargo.toml                     # Rust workspace manifest
└── mise.toml                      # Toolchain versions and dev tasks
```

## Library Map

- `finstack-quant-core`: currencies, money, rates, dates, calendars, market data,
  cashflow primitives, math utilities, and the expression engine.
- `finstack-quant-cashflows`: schedule construction, accrual logic, and
  currency-preserving cashflow aggregation for bonds, loans, swaps, and
  structured products.
- `finstack-quant-analytics`: return-series performance analytics, drawdown
  analysis, tail risk, benchmark-relative metrics, and rolling statistics.
- `finstack-quant-monte-carlo`: generic Monte Carlo engine, stochastic processes,
  discretizations, payoffs, variance reduction, and result types.
- `finstack-quant-margin`: CSA and repo margin specs, VM/IM engines, SIMM
  helpers, collateral eligibility, and XVA primitives.
- `finstack-quant-statements`: period-based financial statement modeling,
  forecasting, formula evaluation, and extension hooks.
- `finstack-quant-statements-analytics`: higher-level analysis on top of
  `finstack-quant-statements`, including scenarios, variance tooling, templates,
  reporting, and covenant-oriented workflows.
- `finstack-quant-valuations`: instrument coverage across rates, credit, equity,
  FX, structured products, and private markets, plus pricing, metrics,
  attribution, covenants, and calibration.
- `finstack-quant-portfolio`: entity and position containers, aggregation,
  grouping, selective repricing, factor decomposition, optimization, and
  scenario-aware workflows.
- `finstack-quant-scenarios`: deterministic scenario composition, market-data
  and statement shocks, instrument shocks, and time roll-forward workflows.

## Packages

### Rust

The top-level Rust crate is `finstack-quant`, imported in Rust as
`finstack_quant`, and re-exports every sub-crate so downstream consumers reach
the full API through a single dependency.

```toml
[dependencies]
finstack-quant = { path = "finstack-quant" }
```

### Python

`finstack-quant-py` builds the Python package `finstack_quant`. Top-level
subpackages are lazy-loaded:

- `analytics`, `cashflows`, `core`, `margin`, `monte_carlo`, `portfolio`,
  `scenarios`, `statements`, `statements_analytics`, `valuations`.

Nested modules under `finstack_quant.valuations` mirror the Rust crate layout.
See [`finstack-quant-py/README.md`](finstack-quant-py/README.md).

### WebAssembly

`finstack-quant-wasm` builds the `finstack-quant-wasm` package for browser and
Node.js consumers. The public facade lives in `finstack-quant-wasm/index.js`,
TypeScript declarations live in `finstack-quant-wasm/index.d.ts`, and namespace
shims live in `finstack-quant-wasm/exports/`.

## Documentation

- [`docs/index.md`](docs/index.md) for the public documentation map.
- [`docs/REFERENCES.md`](docs/REFERENCES.md) for formulas, conventions, and
  market references.
- [`finstack-quant-py/README.md`](finstack-quant-py/README.md) for Python
  bindings.
- [`finstack-quant-py/examples/notebooks/README.md`](finstack-quant-py/examples/notebooks/README.md)
  for the notebook curriculum.

## Development Setup

The repository uses [mise](https://mise.jdx.dev/) as the single source of truth
for toolchain versions: Rust, Python, Node, `uv`, `wasm-pack`, `cargo-nextest`,
`cargo-deny`, `cargo-llvm-cov`, and `maturin`.

```bash
# Install mise on macOS or Linux
curl https://mise.run | sh

# Provision every pinned tool listed in mise.toml
mise install
```

Windows users should run `mise run <task>` from a POSIX shell such as Git Bash,
MSYS2, or WSL. mise itself works natively on Windows, and every task in
`mise.toml` is cross-platform except `docs-all`, which shells out to a bash
script under `scripts/`.

## Common Commands

| Command | Purpose |
|---|---|
| `mise run rust-build` | Build the Rust workspace excluding binding crates |
| `mise run all-test` | Run Rust, Python, and WASM tests |
| `mise run all-fmt` | Format Rust, Python, and WASM code |
| `mise run all-lint` | Run the fast lint pass across Rust, Python, and WASM |
| `mise run python-sync` | Sync Python dev dependencies with `uv sync --group dev` |
| `mise run python-build` | Build the Python extension in-place with the dev profile |
| `mise run python-build -- --release` | Build the Python extension in release mode |
| `mise run wasm-gen-bindings` | Export TypeScript types from Rust |
| `mise run wasm-pkg` | Build the web and Node WASM packages |
| `mise run rust-test` | Run Rust tests with `cargo nextest` |
| `mise run python-test` | Build the release Python extension, then run Python tests |
| `mise run wasm-test` | Run WASM package tests |
| `mise run rust-test-cov` | Run Rust tests with HTML coverage report |
| `mise run python-test-cov` | Build the release Python extension, then run Python tests with HTML coverage report |
| `mise run wasm-test-cov` | Run WASM binding tests with HTML coverage report |
| `mise run rust-check-schemas` | Verify JSON schemas match Rust types |
| `mise run wheel-local` | Build a Python wheel for the current platform |

Run `mise tasks` to list every available task.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the development setup, project
principles, and starter contribution areas.

## License

MIT OR Apache-2.0

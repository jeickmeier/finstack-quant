# Finstack Quant

Finstack Quant is a Rust-first quantitative finance workspace with Python and
WebAssembly bindings. The repository is organized around reusable financial
primitives, pricing and risk engines, statement modeling, deterministic
scenario tooling, portfolio analytics, and thin binding layers that keep the
business logic in Rust.

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
├── finstack-quant-py/                   # PyO3 bindings packaged as `finstack-quant-py`
├── finstack-quant-wasm/                 # wasm-bindgen bindings packaged as `finstack-quant-wasm`
├── docs/                          # Shared references and project documentation
├── pyproject.toml                 # Python packaging and tooling
├── Cargo.toml                     # Workspace manifest
└── mise.toml                      # Toolchain versions and dev tasks (build/test/lint/...)
```

## Library Map

- `finstack-quant-core`: currencies, money, rates, dates, calendars, market data,
  cashflow primitives, math utilities, and the expression engine.
- `finstack-quant-cashflows`: schedule construction, accrual logic, and
  currency-preserving cashflow aggregation for bonds, loans, swaps, and
  structured products.
- `finstack-quant-analytics`: return-series performance analytics, drawdown analysis,
  tail risk, benchmark-relative metrics, and rolling statistics.
- `finstack-quant-monte-carlo`: generic Monte Carlo engine, stochastic processes,
  discretizations, payoffs, variance reduction, and result types.
- `finstack-quant-margin`: CSA and repo margin specs, VM/IM engines, SIMM helpers,
  collateral eligibility, and XVA primitives.
- `finstack-quant-statements`: period-based financial statement modeling,
  forecasting, formula evaluation, and extension hooks.
- `finstack-quant-statements-analytics`: higher-level analysis on top of
  `finstack-quant-statements`, including scenarios, variance tooling, templates,
  reporting, and covenant-oriented workflows.
- `finstack-quant-valuations`: instrument coverage across rates, credit, equity, FX,
  structured products, and private markets, plus pricing, metrics,
  attribution, covenants, and calibration.
- `finstack-quant-portfolio`: entity and position containers, aggregation, grouping,
  selective repricing, factor decomposition, optimization, and scenario-aware
  workflows.
- `finstack-quant-scenarios`: deterministic scenario composition, market-data and
  statement shocks, instrument shocks, and time roll-forward workflows.

## Umbrella Crate

The top-level Rust crate is `finstack-quant`, imported in Rust as `finstack_quant`,
which re-exports every sub-crate so
downstream consumers reach the full API through a single dependency.

```toml
[dependencies]
finstack-quant = { path = "finstack-quant" }
```

`finstack-quant-cashflows` is a standalone workspace crate and a direct dependency of
`finstack-quant-valuations`.

## Python Bindings

`finstack-quant-py` builds the Python package `finstack_quant`. Top-level subpackages (lazy-loaded):

- `analytics`, `cashflows`, `core`, `margin`, `monte_carlo`, `portfolio`,
  `scenarios`, `statements`, `statements_analytics`, `valuations`

Nested modules under `finstack_quant.valuations` (for example `correlation`, `instruments`)
mirror the Rust crate layout. See [`finstack-quant-py/README.md`](finstack-quant-py/README.md).

Notebooks live under `finstack-quant-py/examples/notebooks/`. Run them with
`mise run python-examples` or
`uv run python finstack-quant-py/examples/notebooks/run_all_notebooks.py`.

## WASM Bindings

`finstack-quant-wasm` builds the `finstack-quant-wasm` package for browser and Node.js
consumers. It exposes namespaced modules that mirror the Rust workspace:

- `core`
- `analytics`
- `margin`
- `monte_carlo`
- `portfolio`
- `scenarios`
- `statements`
- `statements_analytics`
- `valuations`

The package facade lives in `finstack-quant-wasm/index.js`, TypeScript declarations
live in `finstack-quant-wasm/index.d.ts`, and the namespace shims live in
`finstack-quant-wasm/exports/`.

## Development Setup

### Prerequisites

The repository uses [mise](https://mise.jdx.dev/) as the single source of truth for
toolchain versions (Rust, Python, Node, uv, wasm-pack, cargo-nextest, cargo-deny,
cargo-llvm-cov, maturin) and developer tasks.

```bash
# Install mise (macOS / Linux)
curl https://mise.run | sh

# Provision every pinned tool listed in mise.toml
mise install
```

> **Windows users:** run `mise run <task>` from a POSIX shell such as Git Bash,
> MSYS2, or WSL. mise itself works natively on Windows, and every task in
> `mise.toml` is cross-platform except `docs-all` (which shells out to a bash
> script under `scripts/`).

### Quick Start

```bash
git clone https://github.com/jeickmeier/finstack-quant.git
cd finstack-quant
mise install

mise run rust-build
mise run all-test
mise run python-build
mise run wasm-pkg
```

Run `mise tasks` to list every available task.

### Common Commands

| Command | Purpose |
|---|---|
| `mise run rust-build` | Build the Rust workspace excluding binding crates |
| `mise run all-test` | Run Rust, Python, and WASM tests |
| `mise run all-fmt` | Format Rust, Python, and WASM code |
| `mise run all-lint` | Run the fast lint pass across Rust, Python, and WASM |
| `mise run python-sync` | Sync Python dev dependencies (`uv sync --group dev`) |
| `mise run python-build` | Build the Python extension in-place (dev profile) |
| `mise run python-build -- --release` | Build the Python extension in release mode |
| `mise run wasm-gen-bindings` | Export TypeScript types from Rust |
| `mise run wasm-pkg` | Build the web and node WASM packages |
| `mise run rust-test` | Run Rust tests with `cargo nextest` |
| `mise run python-test` | Build the release Python extension, then run Python tests |
| `mise run wasm-test` | Run WASM package tests |
| `mise run rust-test-cov` | Run Rust tests with HTML coverage report |
| `mise run python-test-cov` | Build the release Python extension, then run Python tests with HTML coverage report |
| `mise run wasm-test-cov` | Run WASM binding tests with HTML coverage report |
| `mise run rust-check-schemas` | Verify JSON schemas match Rust types |
| `mise run wheel-local` | Build a Python wheel for the current platform |

## Documentation

- `docs/` for shared references, standards, and design notes.
- [`finstack-quant-py/README.md`](finstack-quant-py/README.md) and
  [`finstack-quant-py/examples/notebooks/README.md`](finstack-quant-py/examples/notebooks/README.md)
  for Python bindings and the notebook curriculum.

## License

MIT OR Apache-2.0

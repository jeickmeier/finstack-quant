# finstack Python bindings

`finstack-py` builds the Python package `finstack`: thin PyO3 wrappers over the Rust
Finstack workspace. Pricing and analytics logic stay in Rust. Top-level subpackages
load lazily so `import finstack` does not import every domain.

## Top-level modules

| Module | Rust crate domain |
|--------|-------------------|
| `finstack.analytics` | `finstack-analytics` |
| `finstack.cashflows` | `finstack-cashflows` (schedule JSON helpers) |
| `finstack.core` | `finstack-core` |
| `finstack.margin` | `finstack-margin` |
| `finstack.monte_carlo` | `finstack-monte-carlo` |
| `finstack.portfolio` | `finstack-portfolio` |
| `finstack.scenarios` | `finstack-scenarios` |
| `finstack.statements` | `finstack-statements` |
| `finstack.statements_analytics` | `finstack-statements-analytics` |
| `finstack.valuations` | `finstack-valuations` |

`finstack.valuations` also exposes nested subpackages (`instruments`, `correlation`,
`credit`, `credit_derivatives`, `fx`, `exotics`, …) that mirror Rust module layout.

## Build and install

From the repository root:

```bash
mise run python-build
```

Installs dependencies from the root `pyproject.toml` and builds the extension with
the Rust **dev** profile (fast compile).

Release build (slower compile, faster runtime — large portfolios, batch notebooks):

```bash
mise run python-build -- --release
```

Direct maturin develop (from `finstack-py/`):

```bash
uv run python -m maturin develop
uv run python -m maturin develop --release
```

## Quick start

```python
from datetime import date

from finstack.core.currency import Currency
from finstack.core.dates import BusinessDayConvention, HolidayCalendar, adjust
from finstack.core.money import Money

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

## Package layout

| Path | Role |
|------|------|
| `finstack-py/finstack/` | Python package, lazy `__init__.py`, `.pyi` stubs |
| `finstack-py/src/bindings/` | PyO3 registration by domain |
| `finstack-py/parity_contract.toml` | Parity-tested public API |
| `finstack-py/tests/` | Runtime and behavioral tests |
| `finstack-py/tests/parity/` | Structural import/name parity |

## Examples and notebooks

Notebook curriculum under `finstack-py/examples/notebooks/`:

- `01_foundations` — core types, dates, market data, math, registry
- `02_pricing` — instruments, attribution
- `03_analytics` — performance and risk analytics
- `04_statement_modeling` — statements and statement analytics
- `05_portfolio_and_scenarios` — portfolio, scenarios, liquidity
- `06_advanced_quant` — Monte Carlo, correlation, margin/XVA
- `07_capstone` — end-to-end workflow

Index: [`examples/notebooks/README.md`](examples/notebooks/README.md).

Run all notebooks from the repo root:

```bash
mise run python-examples
# or:
uv run python finstack-py/examples/notebooks/run_all_notebooks.py
```

One section:

```bash
uv run python finstack-py/examples/notebooks/run_all_notebooks.py --directory 05_portfolio_and_scenarios
```

## Stubs, parity, and tests

`.pyi` stubs live under `finstack-py/finstack/`. When you add or rename a binding in the
parity-tested surface, update `parity_contract.toml` in the same change.

| Check | Command |
|-------|---------|
| Python tests | `mise run python-test` |
| Parity only | `uv run pytest finstack-py/tests/parity` |
| Type check | `mise run python-typecheck` |
| Stub completeness | `mise run python-verifytypes` |

Structural parity (`finstack-py/tests/parity/`):

- Every contract entry imports.
- Names match Rust `snake_case` 1:1 (see `AGENTS.md`).
- Modules marked `exists` / `flattened` import; `missing` stay absent until the contract changes.

Behavioral parity (e.g. `tests/test_core_parity.py`) compares Rust-backed results.

## Type discovery

| Area | Module | Entry points |
|------|--------|--------------|
| Money / currency | `finstack.core.money`, `finstack.core.currency` | `Money`, `Currency` |
| Rates | `finstack.core.types` | `Rate`, `Bps`, `Percentage` |
| Credit ratings | `finstack.core.types` | `CreditRating` |
| Dates | `finstack.core.dates` | `Tenor`, `DayCount`, `Schedule`, `ScheduleBuilder`, `HolidayCalendar`, `adjust` |
| Config | `finstack.core.config` | `FinstackConfig`, `RoundingMode`, `ToleranceConfig` |
| Curves / context | `finstack.core.market_data` | `DiscountCurve`, `ForwardCurve`, `MarketContext` |
| Credit scoring | `finstack.core.credit.scoring` | `altman_z_score`, `ohlson_o_score`, … (tuple results) |
| Cashflow schedules | `finstack.cashflows` | `build_cashflow_schedule`, `validate_cashflow_schedule` |
| Pricing | `finstack.valuations` | `price_instrument`, instrument types under `valuations.instruments` |
| Performance / risk | `finstack.analytics` | `Performance` (methods: `value_at_risk`, drawdowns, rolling metrics, …) |

Full surface: `finstack-py/finstack/**/*.pyi`.

## Common pitfalls

### Decimal vs `float`

Per `INVARIANTS.md` §1, Rust uses `Decimal` at the money/accounting boundary and `f64`
elsewhere. Bindings expose `f64` for interop; `Money` also accepts `decimal.Decimal` on
construction. Convert at your boundary if downstream code needs exact decimals:

```python
from decimal import Decimal
from finstack.core.money import Money

m = Money(123.45, "USD")
d = Decimal(m.format(decimals=2, show_currency=False))
```

### Builders mutate in place

Rust builders chain (`builder.frequency(x).stub_rule(y).build()`). Python builder
methods return `None` — call setters on the same instance, then `.build()`:

```python
from finstack.core.dates import ScheduleBuilder, StubKind

b = ScheduleBuilder(start, end)
b.frequency("3M")
b.stub_rule(StubKind.SHORT_FRONT)
schedule = b.build()
```

### Errors

Most fallible bindings raise `ValueError` with the Rust error chain in the message
(`finstack-py/src/errors.rs`: `core_to_py`, `display_to_py`).

Domain-specific types (all subclass `ValueError` unless noted):

| Exception | Module | Use |
|-----------|--------|-----|
| `AnalyticsError` | `finstack.analytics` | Analytics validation / calculation |
| `PortfolioError` | `finstack.portfolio` | General portfolio errors |
| `FinstackValuationError` | `finstack.portfolio` | Valuation failures |
| `FinstackFxError` | `finstack.portfolio` | FX / missing market data |
| `FinstackOptimizationError` | `finstack.portfolio` | Optimization failures |
| `CholeskyError` | `finstack.core.math.linalg` | Cholesky decomposition |
| `CalibrationEnvelopeError` | `finstack.valuations` | Calibration envelope (`RuntimeError` subclass) |

Catching `ValueError` still covers analytics and portfolio subclasses.

### Naming matches Rust

Rust `snake_case` ↔ Python `snake_case`, identical. Search the Rust crate if a symbol
is missing from stubs — the name is usually the same.

## Documentation style

Contributors: [`DOCS_STYLE.md`](DOCS_STYLE.md) (PyO3 `///` comments, `.pyi` NumPy-style
docstrings, financial conventions, in-place builders).

## Rust and WASM

Same Rust workspace as the repo root `README.md`. Browser/Node bindings:
`finstack-wasm` (subset of `finstack-core` on WASM; see `parity_contract.toml`
`[wasm_core_subset]`).

## License

MIT OR Apache-2.0

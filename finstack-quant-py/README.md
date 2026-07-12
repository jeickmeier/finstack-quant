# finstack-quant-py Python bindings

`finstack-quant-py` builds the Python package `finstack_quant`: thin PyO3 wrappers over the Rust
Finstack Quant workspace. Pricing and analytics logic stay in Rust. Top-level subpackages
load lazily so `import finstack_quant` does not import every domain.

## Top-level modules

| Module | Rust crate domain |
|--------|-------------------|
| `finstack_quant.analytics` | `finstack-quant-analytics` |
| `finstack_quant.cashflows` | `finstack-quant-cashflows` (schedule JSON helpers) |
| `finstack_quant.core` | `finstack-quant-core` |
| `finstack_quant.margin` | `finstack-quant-margin` |
| `finstack_quant.monte_carlo` | `finstack-quant-monte-carlo` |
| `finstack_quant.portfolio` | `finstack-quant-portfolio` |
| `finstack_quant.scenarios` | `finstack-quant-scenarios` |
| `finstack_quant.statements` | `finstack-quant-statements` |
| `finstack_quant.statements_analytics` | `finstack-quant-statements-analytics` |
| `finstack_quant.valuations` | `finstack-quant-valuations` |

`finstack_quant.valuations` also exposes nested subpackages (`instruments`, `correlation`,
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

Python test tasks build the extension in release mode before invoking `pytest`:

```bash
mise run python-test
```

Direct maturin develop (from the repository root):

```bash
uv run python -m maturin develop
uv run python -m maturin develop --release
```

## Quick start

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

## Package layout

| Path | Role |
|------|------|
| `finstack-quant-py/finstack_quant/` | Python package, lazy `__init__.py`, `.pyi` stubs |
| `finstack-quant-py/src/bindings/` | PyO3 registration by domain |
| `finstack-quant-py/parity_contract.toml` | Parity-tested public API |
| `finstack-quant-py/tests/` | Runtime and behavioral tests |
| `finstack-quant-py/tests/parity/` | Structural import/name parity |

## Examples and notebooks

Notebook curriculum under `finstack-quant-py/examples/notebooks/`:

- `01_foundations` — core types, dates, market data, math, registry
- `02_pricing` — instruments, attribution
- `03_analytics` — performance and risk analytics
- `04_statement_modeling` — statements and statement analytics
- `05_portfolio` — portfolio construction, optimization, risk, liquidity
- `06_scenarios` — scenario authoring, stress tests, impact analysis
- `07_advanced_quant` — Monte Carlo, correlation, margin/XVA
- `08_capstone` — end-to-end workflow
- `09_reporting` — reporting tear sheets

Index: [`examples/notebooks/README.md`](examples/notebooks/README.md).

Run all notebooks from the repo root:

```bash
mise run python-examples
# or:
uv run python finstack-quant-py/examples/notebooks/run_all_notebooks.py
```

One section:

```bash
uv run python finstack-quant-py/examples/notebooks/run_all_notebooks.py --directory 05_portfolio
```

## Stubs, parity, and tests

`.pyi` stubs live under `finstack-quant-py/finstack_quant/`. When you add or rename a binding in the
parity-tested surface, update `parity_contract.toml` in the same change.

| Check | Command |
|-------|---------|
| Python tests | `mise run python-test` (release build, then pytest) |
| Parity only | `uv run pytest finstack-quant-py/tests/parity` |
| Type check | `mise run python-typecheck` |

Structural parity (`finstack-quant-py/tests/parity/`):

- Every contract entry imports.
- Names match Rust `snake_case` 1:1 (see `AGENTS.md`).
- Modules marked `exists` / `flattened` import; `missing` stay absent until the contract changes.

Behavioral parity (e.g. `tests/test_core_parity.py`) compares Rust-backed results.

## Type discovery

| Area | Module | Entry points |
|------|--------|--------------|
| Money / currency | `finstack_quant.core.money`, `finstack_quant.core.currency` | `Money`, `Currency` |
| Rates | `finstack_quant.core.types` | `Rate`, `Bps`, `Percentage` |
| Credit ratings | `finstack_quant.core.types` | `CreditRating` |
| Dates | `finstack_quant.core.dates` | `Tenor`, `DayCount`, `Schedule`, `ScheduleBuilder`, `HolidayCalendar`, `adjust` |
| Config | `finstack_quant.core.config` | `FinstackConfig`, `RoundingMode`, `ToleranceConfig` |
| Curves / context | `finstack_quant.core.market_data` | `DiscountCurve`, `ForwardCurve`, `MarketContext` |
| Credit scoring | `finstack_quant.core.credit.scoring` | `altman_z_score`, `ohlson_o_score`, … (tuple results) |
| Cashflow schedules | `finstack_quant.cashflows` | `build_cashflow_schedule_json`, `validate_cashflow_schedule_json` |
| Pricing | `finstack_quant.valuations` | `price_instrument`, instrument types under `valuations.instruments` |
| Performance / risk | `finstack_quant.analytics` | `Performance` (methods: `value_at_risk`, drawdowns, rolling metrics, …) |

Full surface: `finstack-quant-py/finstack_quant/**/*.pyi`.

## Common pitfalls

### Decimal vs `float`

Per `INVARIANTS.md` §1, `Money` stores its amount as Rust `Decimal`. Python
construction accepts `decimal.Decimal`, `float`, or `int`: `Decimal` inputs
preserve full decimal precision, while `float`/`int` inputs are converted
through Python's finite floating-point value. Use `amount_decimal` for the
lossless stored value; `amount` is the interoperable `float` view:

```python
from decimal import Decimal
from finstack_quant.core.money import Money

m = Money(Decimal("123.4500000000000000001"), "USD")
d = m.amount_decimal
```

### Builders are fluent and mutate in place

Schedule builders match Rust's fluent chaining while preserving Python's
in-place mutation: each setter returns the same instance.

```python
from finstack_quant.core.dates import ScheduleBuilder, StubKind

schedule = (
    ScheduleBuilder(start, end)
    .frequency("3M")
    .stub_rule(StubKind.SHORT_FRONT)
    .build()
)
```

### Errors

Most fallible bindings raise `ValueError` with the Rust error chain in the message
(`finstack-quant-py/src/errors.rs`: `core_to_py`, `display_to_py`).

Domain-specific types (all subclass `ValueError` unless noted):

| Exception | Module | Use |
|-----------|--------|-----|
| `AnalyticsError` | `finstack_quant.analytics` | Analytics validation / calculation |
| `PortfolioError` | `finstack_quant.portfolio` | General portfolio errors |
| `FinstackValuationError` | `finstack_quant.portfolio` | Valuation failures |
| `FinstackFxError` | `finstack_quant.portfolio` | FX / missing market data |
| `FinstackOptimizationError` | `finstack_quant.portfolio` | Optimization failures |
| `CholeskyError` | `finstack_quant.core.math.linalg` | Cholesky decomposition |
| `CalibrationEnvelopeError` | `finstack_quant.valuations` | Calibration envelope (`RuntimeError` subclass) |

Catching `ValueError` still covers analytics and portfolio subclasses.

### Naming matches Rust

Rust `snake_case` ↔ Python `snake_case`, identical. Search the Rust crate if a symbol
is missing from stubs — the name is usually the same.

## Documentation style

Contributors: [`DOCS_STYLE.md`](DOCS_STYLE.md) (PyO3 `///` comments, `.pyi` NumPy-style
docstrings, financial conventions, in-place builders).

## Rust and WASM

Same Rust workspace as the repo root `README.md`. Browser/Node bindings:
`finstack-quant-wasm` (subset of `finstack-quant-core` on WASM; see `parity_contract.toml`
`[wasm_core_subset]`).

## License

MIT OR Apache-2.0

# finstack-quant-py

`finstack-quant-py` is the PyO3 crate that builds the Python package
`finstack_quant`: thin wrappers over the Rust Finstack Quant workspace. All
pricing, analytics, and modeling logic lives in Rust; the bindings only
translate types, map errors, and hand results back as typed Python objects.

## Where this sits

The crate depends on all 14 domain crates plus the umbrella crate
`finstack-quant` (for `finstack_quant::schema`) and the supporting crate
`finstack-quant-arrow` (for `core.table`). Nothing in the Rust workspace
depends on the bindings. Build artifacts land as a single compiled extension
module, `finstack_quant.finstack_quant`, which the pure-Python shim packages
under `finstack_quant/` re-export by domain.

`pyproject.toml` at the repository root is the maturin project file. It sets
`module-name` to `finstack_quant.finstack_quant`, `python-source` to
`finstack-quant-py`, and builds with the `extension-module` feature. Requires
Python 3.12+.

## Namespaces

`import finstack_quant` binds nothing but a lazy `__getattr__`; each domain is
imported on first attribute access, so cold start in a CLI or notebook does not
pay for every domain's registration.

| Python namespace | Rust crate |
|------------------|------------|
| `finstack_quant.analytics` | `finstack-quant-analytics` |
| `finstack_quant.attribution` | `finstack-quant-attribution` |
| `finstack_quant.cashflows` | `finstack-quant-cashflows` |
| `finstack_quant.core` | `finstack-quant-core` |
| `finstack_quant.covenants` | `finstack-quant-covenants` |
| `finstack_quant.features` | `finstack-quant-features` |
| `finstack_quant.margin` | `finstack-quant-margin` |
| `finstack_quant.models` (including `.factor`) | `finstack-quant-models` |
| `finstack_quant.portfolio` | `finstack-quant-portfolio` |
| `finstack_quant.scenarios` | `finstack-quant-scenarios` |
| `finstack_quant.statements` | `finstack-quant-statements` |
| `finstack_quant.statements_analytics` | `finstack-quant-statements-analytics` |
| `finstack_quant.valuations` | `finstack-quant-valuations` |

Two more names are exported from the package root and are not domain crates:

- `finstack_quant.schema` — a compiled submodule bound from the umbrella
  crate's own `schema` module. `index()`, `get(selector, profile=...)`,
  `validate(selector, payload)`, `domains()`. It merges the per-domain registries
  (`domains()` lists the nine that publish schemas) into one domain-labelled
  index, so a caller does not have to hard-code the domain list.
- `finstack_quant.reporting` — a **pure-Python** presentation layer (tear
  sheets, tables, charts, themes) with no Rust counterpart. It is explicitly
  exempt from crate mirroring and has no WASM twin.

`finstack_quant.__version__` mirrors the workspace version. It is also lazy;
reading it is what first loads the compiled extension. Record it next to
results so a notebook stays reproducible.

### Nested namespaces

Domains that mirror a nested Rust module tree expose it as nested packages:

- `finstack_quant.core.{config, currency, dates, market_data, math,
  money, rating_scales, schema, table, types}`, with
  `core.market_data.{arbitrage, context, curves, fx, scalars}`,
  and `core.math.{linalg, special_functions, stats,
  summation}`.
- `finstack_quant.models.{credit, correlation, factor, liquidity, monte_carlo,
  rates, volatility}`, with `models.credit.{scoring, pd, lgd, migration,
  recovery_waterfall, liability_management}` and `models.rates.dtsm`.
- `finstack_quant.valuations.{instruments, credit_derivatives, composite,
  market, envelope, schema}`.
- `finstack_quant.cashflows.{accrual, aggregation, builder, primitives,
  schema}`.
- `finstack_quant.models.factor.{credit, risk, schema}`,
  `finstack_quant.features.dataframe`.

Each of the ten schema-publishing registry domains also has a Python schema
namespace. The factor registry lives at `finstack_quant.models.factor.schema`;
the others use the corresponding domain's `.schema` module.
`finstack_quant.schema` is the merged view over all ten.

## Build and install

From the repository root:

```bash
mise run python-sync              # uv sync --group dev
mise run python-build             # uv run maturin develop  (dev profile)
mise run python-build -- --release
```

`python-sync` still installs the union `dev` group (the default). Split
groups are available when you want a slimmer environment:

```bash
uv sync --group test              # pytest, hypothesis, jsonschema
uv sync --group lint              # ruff, ty, bandit, pre-commit
uv sync --group goldens           # QuantLib, for golden regeneration only
uv sync --group notebooks         # IPython / Jupyter
uv sync --group arrow             # pyarrow / polars interchange tests
```

Pip extras mirror the same sets: `pip install .[test]`, `.[goldens]`,
`.[notebooks]`, `.[arrow]`, `.[lint]`, `.[build]`.

The dev profile compiles fast and runs slowly; use `--release` for large
portfolios, Monte Carlo work, and batch notebook runs. `mise run wheel-local`
builds a release wheel for the current interpreter into `target/wheels`;
`mise run wheel-all` does the same for every locally discoverable interpreter.

The Python test tasks rebuild the extension with the dev profile first, so
`mise run python-test` is safe to run directly after a Rust change.

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

print(amount.format_with())   # 'USD 1000000.00'
print(settle)            # 2025-01-06
```

Pricing an instrument against a market context:

```python
import datetime

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import StubKind
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Rate
from finstack_quant.valuations.instruments import Bond, price_instrument

as_of = datetime.date(2024, 1, 1)
bond = Bond.fixed(
    "B",
    Money(1000.0, Currency("USD")),
    Rate(0.05),
    as_of,
    datetime.date(2026, 1, 1),
    StubKind.NONE,
    "USD-OIS",
)
market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))

result = price_instrument(bond, market, as_of, metrics=["dv01"])
print(result.instrument_id, round(result.price, 2), result.currency)  # B 1017.07 USD
frame = result.to_dataframe()
```

## Result-return contract

Computation entry points return **typed results**, never JSON strings. The
contract, pinned by `tests/parity/test_return_shapes.py` and mirrored by
`finstack-quant-wasm/tests/return_shapes.rs`:

| Shape | Meaning |
|-------|---------|
| `wrapper` | A typed `Py*` class (`ValuationResult`, `PeriodStats`, …) |
| `frame` | `pandas.DataFrame` |
| `series` | `pandas.Series` |
| `json` | A JSON `str` — legal **only** on a `*_json`-suffixed wire surface, and every such surface must have a typed twin |
| `scalar` / `list` / `dict` | Plain Python values |

Every result wrapper carries:

- typed getters for its headline fields (a class whose only accessor is
  `to_json` is rejected),
- `to_json()` and `from_json(...)`, where `from_json` is a `@staticmethod`,
- `__reduce__` via that same JSON path, so results pickle and therefore survive
  `multiprocessing` / `joblib` / `dask` fan-out,
- `to_dataframe()` where the result is tabular, and `to_series()` for 1-D
  labeled vectors. Orientation is a parameter (`to_dataframe(orient=...)`),
  not a separate method name.

Non-finite metrics (`+inf` profit factor, `NaN` placeholders) round-trip
through JSON and pickle intact; `serde_json`'s null-for-non-finite behavior is
handled inside the bindings.

## Conventions that bite

### Decimal vs float

Per [`INVARIANTS.md`](../INVARIANTS.md) §1, `Money` stores a Rust `Decimal`
plus a `Currency`. Python construction accepts `decimal.Decimal`, `float`, or
`int`. `Decimal` inputs keep full precision; `float`/`int` inputs are limited
to what the float held. `amount_decimal` is the lossless view; `amount` is the
interoperable `float` view.

```python
from decimal import Decimal
from finstack_quant.core.money import Money

m = Money(Decimal("123.4500000000000000001"), "USD")
m.amount_decimal  # Decimal('123.4500000000000000001')
m.amount          # 123.45000000000002
```

Curves, rates, vols, correlations, greeks, and Monte Carlo paths are `f64` on
both sides of the boundary.

### Currency safety

There is no implicit cross-currency arithmetic anywhere, including in the
bindings. Cross-currency work goes through an explicit FX provider or matrix,
and the applied policy is stamped into the result envelope.

### Rate units

Rates are decimals (`Rate(0.05)` is 5%). Basis-point inputs are named with a
`_bp` / `Bps` suffix and are never mixed into a plain rate argument. Curve IDs
(`"USD-OIS"`) are required by name in `MarketContext`; a missing ID raises
rather than falling back.

### Builders mutate in place and return `self`

Python builders match Rust's fluent chaining by returning the same instance
from every setter — they are not copy-on-write unless the method is explicitly
named `with_*`.

```python
from datetime import date
from finstack_quant.core.dates import Schedule, StubKind

schedule = (
    Schedule.builder(date(2025, 1, 15), date(2030, 1, 15))
    .frequency("3M")
    .stub_rule(StubKind.SHORT_FRONT)
    .build()
)
```

### Errors

Fallible bindings raise through `src/errors.rs` (`core_to_py`,
`display_to_py`): missing ids become `KeyError`, validation and argument
failures `ValueError`, calibration and operational failures `RuntimeError`. The
Rust error chain is preserved in the message. Named exceptions inherit
`FinstackError`, which inherits `ValueError`, so `except ValueError` still
catches them.

| Exception | Module | Base |
|-----------|--------|------|
| `FinstackError` | `finstack_quant.core` | `ValueError` |
| `AnalyticsError` | `finstack_quant.analytics` | `FinstackError` |
| `CholeskyError` | `finstack_quant.core.math.linalg` | `FinstackError` |
| `PortfolioError` | `finstack_quant.portfolio` | `FinstackError` |
| `ValuationError` | `finstack_quant.portfolio` | `PortfolioError` |
| `FxError` | `finstack_quant.portfolio` | `PortfolioError` |
| `ContractValidationError` | `finstack_quant.portfolio` | `FinstackError` |
| `ContractLimitExceededError` | `finstack_quant.portfolio` | `ContractValidationError` |
| `MalformedContractSchemaError` | `finstack_quant.portfolio` | `ContractValidationError` |
| `MissingContractVersionError` | `finstack_quant.portfolio` | `ContractValidationError` |
| `UnsupportedContractVersionError` | `finstack_quant.portfolio` | `ContractValidationError` |
| `CalibrationEnvelopeError` | `finstack_quant.valuations` | `RuntimeError` (deliberately outside the `FinstackError` tree) |

### Determinism and the GIL

Stochastic APIs take an explicit seed; none of them read a thread-local RNG.
The reproducibility tier is per-API and is documented on the Rust side — see
[`INVARIANTS.md`](../INVARIANTS.md) §2, which distinguishes bit-reproducible
(identical bits across serial/parallel modes and thread counts) from
seed-reproducible and statistically reproducible. Do not assume the strongest
tier without checking the API's own docs.

CPU-heavy entry points release the GIL inside Rust, so they parallelize under
`concurrent.futures` threads; `tests/test_portfolio_gil_release.py` pins that
for the portfolio surface.

### Serde strictness

Input and configuration structs carry `#[serde(deny_unknown_fields)]`, so a
payload with a typo'd key fails loudly rather than silently dropping the
field — the same behavior as the Rust surface. See
[`docs/SERDE_STABILITY.md`](../docs/SERDE_STABILITY.md).

## Naming and parity

Rust is the source of truth for topology and naming: the binding module tree
under `src/bindings/` mirrors the Rust umbrella crate, and Python names match
Rust names character for character (`sharpe` stays `sharpe`, `Date` stays
`Date`). No convenience re-exports at `finstack_quant.*`, no legacy aliases. If
a symbol seems missing from the stubs, search the Rust crate — the name is
almost always identical.

`parity_contract.toml` (in this directory) is the authoritative Rust↔Python↔WASM
map. It records per-crate module status (`exists` / `flattened` / `missing`,
each `missing` entry with a reason), symbol-level pins, the exact top-level
names the compiled stub may declare, and the WASM namespace subsets. Three
documented deviations from strict crate mirroring live there:

- `finstack_quant.models.correlation` is a **merged** namespace. Most of it
  mirrors `finstack_quant_models::correlation` (copulas, `CreditExposure`,
  portfolio-loss simulation); the shared correlation-matrix helpers
  (`validate_correlation_matrix`, `nearest_correlation`) are canonically owned
  by `finstack_quant_analytics::correlation` and re-exported through
  `finstack_quant_models::correlation`. `nearest_correlation` is the one
  documented rename (Rust: `nearest_correlation_matrix`).
- `reporting` is pure Python with no Rust crate and no WASM parity.
- `core.table` is a binding-level host-interop surface backed by
  `finstack-quant-arrow`, which the umbrella crate does not re-export. It has no
  WASM twin because arrow-rs is not built for wasm32.

When you add or rename anything in the parity-tested surface, update
`parity_contract.toml` in the same change.

See [`.agents/rules/python/code-standards.md`](../.agents/rules/python/code-standards.md)
for the binding-authoring rules (registration pattern, `__all__` handling, type
wrapping, error mapping).

## Type discovery

`.pyi` stubs live beside the shim packages under `finstack_quant/` and are the
primary IntelliSense surface; `py.typed` marks the package as typed. Runtime
`help()` text comes from the PyO3 `///` comments in `src/bindings/`.

| Area | Module | Entry points |
|------|--------|--------------|
| Money / currency | `core.money`, `core.currency` | `Money`, `Currency` |
| Rates and ratings | `core.types` | `Rate`, `Bps`, `Percentage`, `CreditRating` |
| Dates | `core.dates` | `Tenor`, `DayCount`, `PeriodId`, `Schedule`, `ScheduleBuilder`, `HolidayCalendar`, `BusinessDayConvention`, `StubKind`, `adjust` |
| Config | `core.config` | `FinstackConfig`, `RoundingMode`, `ToleranceConfig` |
| Curves / context | `core.market_data` | `DiscountCurve`, `ForwardCurve`, `HazardCurve`, `FxMatrix`, `ScalarTimeSeries`, `MarketContext` |
| Credit models | `models.credit` | Structural-credit models, `moodys_warf_factor`, and nested `scoring`, `pd`, `lgd`, `migration`, `recovery_waterfall`, and `liability_management` modules |
| Arrow interchange | `core.table` | `ArrowTable` (the module's only export). Instances come from result wrappers elsewhere — `StatementResult.to_arrow_long` / `.to_arrow_wide`, `PortfolioValuation.to_arrow_positions`; consume with `pyarrow.table(...)`, `polars.DataFrame(...)` via the `__arrow_c_stream__` PyCapsule protocol |
| Cashflow schedules | `cashflows.builder` | `CashFlowBuilder`, `ScheduleParams`, `FixedCouponSpec`, `FloatingCouponSpec`, `AmortizationSpec`, … |
| Pricing | `valuations.instruments` | `price_instrument`, `Bond`, `TermLoan`, `InterestRateSwap`, `Swaption`, `CapFloor`, `CreditDefaultSwap`, `FxForward`, `FxOption`, `EquityOption`, `StructuredCredit`, … |
| Valuation envelope | `valuations` | `ValuationResult`, `CalibrationEnvelope`, `calibrate` |
| Performance / risk | `analytics` | `Performance`, `PeriodStats`, `BetaResult`, `DrawdownEpisode`, `RollingGreeks` |
| Portfolio | `portfolio` | `Portfolio`, `value_portfolio`, `optimize_portfolio`, `brinson_fachler`, `replay_portfolio`, … |
| Schemas | `schema` | `index`, `get`, `validate`, `domains` |
| Tear sheets | `reporting` | `statement_tearsheet`, `credit_tearsheet`, `dcf_tearsheet`, `scenario_tearsheet`, `portfolio_tearsheet`, `portfolio_risk_tearsheet`, `benchmark_tearsheet`, `instrument_tearsheet`, `performance_tearsheet`, `attribution_tearsheet`, `Theme`, `INSTITUTIONAL` |

Full surface: `finstack_quant/**/*.pyi`.

## Layout

| Path | Role |
|------|------|
| `finstack_quant/` | Python package: lazy `__init__.py`, per-domain shims, `.pyi` stubs, `py.typed` |
| `src/lib.rs` | Entry point; delegates to `bindings::register_root` |
| `src/bindings/` | PyO3 registration, one directory per crate domain |
| `src/errors.rs` | Centralized Rust→Python error mapping |
| `parity_contract.toml` | Authoritative Rust↔Python↔WASM API map |
| `tests/` | Runtime and behavioral tests |
| `tests/parity/` | Contract topology, return shapes, covenant bindings |
| `tests/golden/` | Per-instrument pricing goldens; fixtures are the Rust crate's own JSON under `finstack-quant/valuations/tests/golden/data/pricing/{regression_goldens,quantlib,bloomberg}` |
| `benchmarks/` | `pytest-benchmark` suites (marked `perf`) |
| `examples/notebooks/` | Layered notebook curriculum ([index](examples/notebooks/README.md)) |
| `DOCS_STYLE.md` | Docstring/stub requirements for contributors |

## Verification

Run from the repository root.

| Check | Command |
|-------|---------|
| Fast tests (rebuilds dev extension) | `mise run python-test` |
| Slow tests only | `mise run python-test-slow` |
| Full suite | `mise run python-test-all` |
| Coverage (HTML into `target/python-cov`) | `mise run python-test-cov` |
| Parity only | `uv run pytest finstack-quant-py/tests/parity` |
| Lint (ruff + doc checkers) | `mise run python-lint` |
| Format / autofix | `mise run python-fmt` |
| Type check (`ty`) | `mise run python-typecheck` |
| Stub + PyO3 doc completeness | `mise run python-doc` |
| Stub doctests | `mise run python-doctest` |
| Notebooks | `mise run python-examples` |
| Benchmarks (release build) | `mise run python-bench` |

Structural parity in `tests/parity/test_contract_topology.py` asserts that
every contract entry imports, that `exists`/`flattened` modules resolve while
`missing` ones stay absent, that the compiled stub declares exactly the
contracted top-level names, and that the WASM facade exports match the same
contract. Behavioral parity (for example `tests/test_core_parity.py`) compares
Rust-backed results directly.

## Related

- [`../README.md`](../README.md) — workspace overview
- [`../INVARIANTS.md`](../INVARIANTS.md) — cross-crate numerical and financial
  contracts
- [`../docs/CONTRACTS.md`](../docs/CONTRACTS.md) and
  [`../docs/SERDE_STABILITY.md`](../docs/SERDE_STABILITY.md) — wire-format policy
- [`../finstack-quant-wasm/README.md`](../finstack-quant-wasm/README.md) —
  browser/Node bindings, a contracted subset of this surface

## License

MIT OR Apache-2.0

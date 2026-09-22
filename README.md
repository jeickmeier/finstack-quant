# finstack-quant

![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)
![Rust](https://img.shields.io/badge/rust-1.90%2B-orange)
![Python](https://img.shields.io/badge/python-3.12%2B-blue)
![WASM](https://img.shields.io/badge/wasm-ready-purple)
![Status](https://img.shields.io/badge/status-alpha-yellow)

A deterministic financial computation workspace. Fourteen Rust domain crates
cover market data, cashflows, instrument pricing, risk, factor models,
financial-statement modeling, scenarios, margin/XVA, and portfolio aggregation.
PyO3 and wasm-bindgen binding crates expose the same APIs to Python and
JavaScript.

Financial logic lives in Rust. Bindings do type conversion, error mapping, and
module registration only, so the same calculation produces the same answer from
a Rust service, a Python notebook, and a browser.

## Scope

- Currency-safe monetary primitives (`Money` is Decimal-backed and refuses to
  mix currencies), ISO-4217 currencies, ISDA day counts, holiday calendars,
  schedule generation.
- Term structures and market data: discount/forward/hazard/inflation/price
  curves, FX matrices, vol surfaces and cubes, bootstrap and calibration.
- Instrument pricing and risk across rates, credit, FX, equity, inflation,
  commodities, convertibles, structured credit, and private markets, with
  closed-form, tree, PDE, Fourier, and Monte Carlo models.
- Performance and risk analytics, factor models, P&L attribution, and
  panel feature transforms.
- Financial-statement modeling with `Value > Forecast > Formula` precedence,
  DCF, sensitivity, covenants, and ECL workflows.
- Deterministic scenario shocks and roll-forward, portfolio aggregation to a
  base currency with an explicit FX policy, margin/collateral/XVA, and
  regulatory capital (FRTB-SBA, SA-CCR, ISDA SIMM).
- Published JSON Schemas for the persisted wire contracts, indexed and
  validatable from Rust and Python.

## Repository layout

```text
finstack-quant/
├── finstack-quant/               # Rust workspace crates
│   ├── Cargo.toml                # `finstack-quant` umbrella crate manifest
│   ├── src/                      # umbrella re-exports + the `schema` registry
│   ├── core/                     # money/FX, dates, market data, math, expressions
│   ├── analytics/                # return-series performance and risk statistics
│   ├── attribution/              # multi-period P&L attribution
│   ├── calibration/              # quote ingestion, market construction, calibration, replay caches
│   ├── cashflows/                # schedule construction, accrual, dated flows
│   ├── covenants/                # covenant specs, evaluation, forecasting
│   ├── features/                 # vectorized panel feature transforms
│   ├── margin/                   # CSA/VM/IM, SIMM, FRTB-SBA, SA-CCR, XVA
│   ├── models/                   # analytical, numerical, factor, credit, correlation, stochastic models
│   ├── valuations/               # instruments, pricing, recalibration port, metrics, results
│   │   └── macros/               # `FinancialBuilder` derive used by valuations
│   ├── statements/               # statement model graph and period evaluation
│   ├── statements-analytics/     # DCF, scenario sets, sensitivity, ECL, backtesting
│   ├── portfolio/                # positions/books, base-currency rollups
│   ├── scenarios/                # deterministic shock/roll DSL and engine
│   ├── arrow-interchange/        # `finstack-quant-arrow`: TableEnvelope -> RecordBatch
│   ├── test-utils/               # golden-test helpers (dev-dependency only)
│   └── tests/                    # umbrella-level integration tests
├── finstack-quant-py/            # PyO3 bindings; builds the `finstack_quant` package
├── finstack-quant-wasm/          # wasm-bindgen bindings + hand-written JS facade
├── benchmarks/                   # materialization benchmark fixtures and notes
├── docs/                         # references, contracts, serde policy, design notes
├── scripts/                      # generation and check scripts driven by mise tasks
├── Cargo.toml                    # Rust workspace manifest
├── pyproject.toml                # Python packaging and tooling
└── mise.toml                     # toolchain pins and dev tasks
```

## Crate map

`finstack-quant` is the umbrella crate. It re-exports all fourteen domain
crates, so one dependency reaches the whole API. Default features enable
JSON Schema generation and validation; they do not change pricing behavior.

| Crate | Umbrella path | Provides |
|---|---|---|
| [`finstack-quant-core`](finstack-quant/core/README.md) | `finstack_quant::core` | `Money`/`Currency`/`Rate`, FX providers, dates and calendars, term structures, math, expression engine, config, `table` envelope |
| [`finstack-quant-analytics`](finstack-quant/analytics/README.md) | `finstack_quant::analytics` | `Performance` entry point: return/risk scalars, drawdowns, rolling windows, alpha/beta, basic factor models |
| [`finstack-quant-attribution`](finstack-quant/attribution/README.md) | `finstack_quant::attribution` | Multi-period P&L attribution: simple bridge, metrics-based, parallel, waterfall, Taylor |
| [`finstack-quant-calibration`](finstack-quant/calibration/README.md) | `finstack_quant::calibration` | Quote ingestion, market construction, calibration engines, explicit model fitting, and cached quote replay |
| [`finstack-quant-cashflows`](finstack-quant/cashflows/README.md) | `finstack_quant::cashflows` | Schedule construction, accrual, currency-preserving aggregation |
| [`finstack-quant-covenants`](finstack-quant/covenants/README.md) | `finstack_quant::covenants` | Covenant specs, evaluation engine, threshold schedules, forecasting, standard packages |
| [`finstack-quant-features`](finstack-quant/features/README.md) | `finstack_quant::features` | Time-series, cross-sectional, and panel feature transforms over `Option<f64>` columns |
| [`finstack-quant-margin`](finstack-quant/margin/README.md) | `finstack_quant::margin` | CSA/repo terms, VM and IM engines (SIMM, schedule, haircut, CCP), collateral metrics, XVA config, regulatory capital |
| [`finstack-quant-models`](finstack-quant/models/README.md) | `finstack_quant::models` | Closed-form/Fourier formulas, volatility, rates/DTSM, credit and structured-credit pool models, factor risk, liquidity, PDE/tree engines, and Monte Carlo |
| [`finstack-quant-valuations`](finstack-quant/valuations/README.md) | `finstack_quant::valuations` | Instruments, market resolution, pricing registries, recalibration provider contract, metrics, and result envelopes |
| [`finstack-quant-statements`](finstack-quant/statements/README.md) | `finstack_quant::statements` | Statement model graph, DSL formulas, forecasting, corkscrews, deterministic period evaluation |
| [`finstack-quant-statements-analytics`](finstack-quant/statements-analytics/README.md) | `finstack_quant::statements_analytics` | Sensitivity, scenario sets, variance, DCF, goal seek, covenant forecasting, backtesting, templates |
| [`finstack-quant-portfolio`](finstack-quant/portfolio/README.md) | `finstack_quant::portfolio` | Entities and positions, valuation and metric aggregation, grouping, optimization, risk decomposition, materialization |
| [`finstack-quant-scenarios`](finstack-quant/scenarios/README.md) | `finstack_quant::scenarios` | Deterministic market/instrument/statement shocks and time rolls as serde-stable specs, template registry, composition, phase-ordered apply, horizon P&L |

### Crates that are not re-exported

Depend on these directly when you need them:

| Crate | Path | Purpose |
|---|---|---|
| [`finstack-quant-arrow`](finstack-quant/arrow-interchange/README.md) | `finstack-quant/arrow-interchange/` | Export a `core::table::TableEnvelope` as an Arrow `RecordBatch` |
| [`finstack-quant-test-utils`](finstack-quant/test-utils/README.md) | `finstack-quant/test-utils/` | Golden-test framework shared across crates; dev-dependency only |
| `finstack-quant-valuations-macros` | `finstack-quant/valuations/macros/` | `FinancialBuilder` derive used inside `valuations` |

### Dependency direction

Every domain crate depends on `core`, as does `arrow-interchange`; `test-utils`
and `valuations-macros` have no in-workspace dependencies. The rest of the
edges, read off the manifests:

| Crate | Also depends on |
|---|---|
| `analytics`, `cashflows`, `covenants`, `features`, `margin` | nothing else in-workspace |
| `models` | `analytics`, `cashflows` |
| `valuations` | `analytics`, `cashflows`, `covenants`, `margin`, `models`, `valuations-macros` |
| `calibration` | `cashflows`, `models`, `valuations` |
| `attribution` | `calibration`, `cashflows`, `models`, `valuations` |
| `statements` | `cashflows`, `valuations` |
| `statements-analytics` | `covenants`, `statements`, `valuations` |
| `scenarios` | `attribution`, `calibration`, `statements`, `valuations` |
| `portfolio` | `attribution`, `calibration`, `cashflows`, `margin`, `models`, `scenarios`, `valuations` |

`valuations` is the mid-stack hub and `portfolio` is the top. No Rust crate
depends on a binding crate, and `core` never depends on `models`.

No cargo feature selects financial behavior. Workspace features are
`json-schema` and `jsonschema-validate` (default on most crates, including
the umbrella), `ts_export` (TypeScript declaration generation on `core`,
`models`, `valuations`, `portfolio`, `calibration`, and the WASM crate),
`extension-module` (PyO3 linking), and `console_panic_hook` (WASM panic
hook).

## Quick start

### Rust

```toml
[dependencies]
finstack-quant = { path = "finstack-quant" }
```

```rust
use finstack_quant::core::currency::Currency;
use finstack_quant::core::money::Money;

fn main() -> finstack_quant::core::Result<()> {
    // Parse ISO-4217 codes (case-insensitive).
    let _eur = "eur".parse::<Currency>().expect("valid ISO-4217 currency");

    // Arithmetic refuses to mix currencies.
    let subtotal = Money::new(49.50, Currency::EUR)?;
    let tax = Money::new(9.90, Currency::EUR)?;
    let total = subtotal.checked_add(tax)?;
    assert_eq!(format!("{total}"), "EUR 59.40");
    Ok(())
}
```

### Python

```bash
git clone https://github.com/jeickmeier/finstack-quant.git
cd finstack-quant
mise install
mise run python-setup
mise run python-sync
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

print(amount.format())  # 'USD 1000000.00'
print(settle)           # 2025-01-06
```

`finstack-quant-py` builds the Python package `finstack_quant`. It exposes the
same thirteen domains — `analytics`, `attribution`, `cashflows`, `core`,
`covenants`, `features`, `margin`, `models`, `portfolio`,
`scenarios`, `statements`, `statements_analytics`, `valuations` — plus
`reporting` (a pure-Python presentation layer with no Rust crate) and `schema`
(a compiled submodule). Submodules load lazily, and `finstack_quant.__version__`
reports the installed version. See
[`finstack-quant-py/README.md`](finstack-quant-py/README.md).

### WebAssembly

```bash
mise run wasm-pkg
```

```javascript
import init, { core } from 'finstack-quant-wasm';

await init();

const usd = new core.Currency('USD');
const amount = new core.Money(1000.0, usd);
console.log(amount.toString());
```

The published entry point is
[`finstack-quant-wasm/index.js`](finstack-quant-wasm/index.js), which re-exports
the thirteen namespaces assembled in `finstack-quant-wasm/exports/`. TypeScript
declarations live in
[`finstack-quant-wasm/index.d.ts`](finstack-quant-wasm/index.d.ts). The
`wasm-pack` output under `pkg/` and `pkg-node/` is generated build output, not
the public API. See
[`finstack-quant-wasm/README.md`](finstack-quant-wasm/README.md).

## Conventions

These bite callers who assume otherwise. [`INVARIANTS.md`](INVARIANTS.md) is
authoritative; this is the short form.

- **Numerics.** `Money` stores `rust_decimal::Decimal` plus a `Currency`.
  `Money::new` / `amount()` take and return `f64`; `Money::from_decimal` /
  `amount_decimal()` are the lossless path. Curves, rates, vols, correlations,
  greeks, optimizers, and Monte Carlo paths use `f64`. There is no `F` type
  alias in this workspace.
- **Currency safety.** `Money` arithmetic is checked and errors on a currency
  mismatch. There is no implicit FX. Cross-currency collapse goes through an
  explicit `FxProvider`, and the applied policy is stamped into the result
  envelope.
- **Rate units.** Rate and coupon fields are decimals (`0.05` means 5%). Fields
  whose name ends in `_bp` are basis points. Ratio metrics are turns (`4.5`
  means 4.5x).
- **Determinism.** Randomized paths take an explicit seed; nothing reads the
  system clock. Every public stochastic API declares a reproducibility tier —
  bit-reproducible, seed-reproducible, or statistically reproducible — and a
  reproducible parallel reduction uses a fixed partition and merge tree, so
  ordering is stable. Parallelism uses Rayon behind no cargo feature; on
  `wasm32` the parallel paths are `cfg`-gated to a serial fallback because
  there is no usable thread pool.
- **Serde strictness.** Inbound types deny unknown fields and use stable field
  names. Wire-format policy is in
  [`docs/SERDE_STABILITY.md`](docs/SERDE_STABILITY.md); the persisted-contract
  matrix is in [`docs/CONTRACTS.md`](docs/CONTRACTS.md).
- **Binding result contract.** Computation entry points return typed results
  (a Rust struct, a `Py*` wrapper, a plain JS object); `_json` / `*Json`
  surfaces are the wire twins that return JSON strings. Python result wrappers
  carry typed getters, `to_json`, `from_json`, and — where the result is
  tabular — `to_dataframe()`, plus `to_series()` for 1-D labeled vectors. On
  the WASM side the rule is pinned per export rather than across the whole
  surface, and a known set of exports still returns a JSON string under an
  unsuffixed name, so read the declared return type in `index.d.ts` before
  assuming an object. See
  [`finstack-quant-py/README.md`](finstack-quant-py/README.md) and
  [`finstack-quant-wasm/README.md`](finstack-quant-wasm/README.md).

## JSON schemas

Each domain crate owns its own schema artifacts. The umbrella crate's
`finstack_quant::schema` module is the only place that sees all of them at once,
which is what a cross-document `$ref` resolver and a whole-corpus index need.

```rust
use serde_json::json;

fn main() -> finstack_quant::core::Result<()> {
    let artifact = finstack_quant::schema::find("bond.schema.json")?;
    let failures = finstack_quant::schema::validate(artifact, &json!({}))?;
    assert!(!failures.is_empty());
    Ok(())
}
```

Union failures are reported at the offending field rather than at the enclosing
`oneOf`, and unit-enum mismatches list the accepted spellings.

The same registry is reachable from Python as `finstack_quant.schema`, with
`index()`, `get(selector, profile="canonical")`, `validate(selector, payload)`,
and `domains()`. These are the schema wire surface: `index`, `get`, and
`validate` return JSON strings, and `validate` takes `payload` as a JSON string
too — pass `json.dumps(payload)`, not a `dict`. Only `domains()` returns a
Python list. Nine registry domains publish schemas today (`attribution`, `cashflows`,
`core`, `factor_model`, `margin`, `portfolio`, `scenarios`, `statements`,
`valuations`). Their Python schema namespaces mirror domain ownership, with the
factor registry at `finstack_quant.models.factor.schema`; there is no WASM
schema namespace.

Regenerate with `mise run rust-gen-schemas`; check for drift with
`mise run rust-check-schemas`.

## Notebook curriculum

The Python notebooks are the main tutorial path, organized in nine levels:

1. `01_foundations` — money, dates and calendars, market data and curves, math,
   registry defaults, market bootstrap.
2. `02_pricing` — instrument JSON, valuation results, per-asset-class pricing
   deep dives, and daily MTM attribution.
3. `03_analytics` — performance, VaR and factor analytics, factor sensitivity,
   feature transforms, breakeven analysis, return contribution, and
   TWRR/MWRR attribution.
4. `04_statement_modeling` — model building, DSL formulas, sensitivity,
   tornado, variance, goal seek, and eleven model deep dives.
5. `05_portfolio` — construction and valuation, optimization, horizon total
   return, historical replay, liquidity risk, risk decomposition, the credit
   factor hierarchy, and an optional multi-asset scale lab.
6. `06_scenarios` — templates and composition, rate/credit/composite stress,
   impact analysis.
7. `07_advanced_quant` — Monte Carlo, correlation and credit models, margin and
   XVA, regulatory capital.
8. `08_capstone` — end-to-end credit portfolio workflow.
9. `09_reporting` — tear-sheet rendering demos for the `reporting` API.

Start at
[`finstack-quant-py/examples/notebooks/README.md`](finstack-quant-py/examples/notebooks/README.md).
Execute the whole curriculum with:

```bash
mise run python-examples
```

## Development setup

[mise](https://mise.jdx.dev/) pins the toolchain in [`mise.toml`](mise.toml): an
exact stable Rust with `clippy`, `rustfmt`, and the `wasm32-unknown-unknown`
target, plus nightly (needed only for the rustdoc JSON that `cargo-public-api`
consumes), Node, `wasm-pack`, `cargo-nextest`, `cargo-llvm-cov`, `cargo-deny`,
`cargo-public-api`, `flamegraph`, `maturin`, and `osv-scanner`. The pinned
toolchain is ahead of the declared MSRV; `mise run rust-msrv` checks production
targets against Rust 1.90.

```bash
# Install mise on macOS or Linux
curl https://mise.run | sh

# Provision every pinned tool listed in mise.toml
mise install
```

Python is not a mise-managed tool here. Install [uv](https://docs.astral.sh/uv/)
separately, then create the virtualenv and sync dev dependencies:

```bash
mise run python-setup   # uv venv --python 3.12
mise run python-sync    # uv sync --group dev
```

Windows users should run `mise run <task>` from a POSIX shell such as Git Bash,
MSYS2, or WSL. mise itself works natively on Windows; the tasks in `mise.toml`
are written for POSIX shells.

## Common commands

`mise.toml` defines 82 tasks named `<domain>-<action>`. `all-*` fans out across
all three languages, `rust-*` / `python-*` / `wasm-*` are per-language, and the
rest are narrower (`goldens-*`, `wheel-*`, `pre-commit-*`, `materialization-*`,
`check-*`). `*-fmt` tasks mutate; `*-lint` tasks are check-only. Run
`mise tasks` for the full list.

| Command | Purpose |
|---|---|
| `mise run all-lint` | Lint Rust, Python, and WASM (check-only) |
| `mise run all-fmt` | Format and auto-fix Rust, Python, and WASM (mutating) |
| `mise run all-test` | Run Rust, Python, and WASM tests |
| `mise run all-ci` | Regenerate derived artifacts, then reproduce the CI job set locally |
| `mise run rust-build` | Build the Rust workspace excluding the binding crates |
| `mise run rust-test` | Run native Rust tests via `cargo nextest` |
| `mise run rust-lint` | `cargo fmt --check` plus clippy with `-D warnings` across the workspace |
| `mise run rust-doc` | Build workspace docs, enforce input docs, and run doctests |
| `mise run rust-msrv` | Check production targets against the declared Rust 1.90 MSRV |
| `mise run rust-bench` | Run Criterion benchmarks with reduced measurement timing |
| `mise run rust-flamegraph` | Generate a CPU flamegraph (`cargo flamegraph --profile bench`; pass extra args after `--`) |
| `mise run gen-write` | Regenerate checked-in schemas, fixtures, and TypeScript bindings |
| `mise run rust-gen-schemas` | Regenerate typed JSON schemas from Rust types |
| `mise run rust-check-schemas` | Verify JSON schemas match Rust types |
| `mise run python-build` | Build the Python extension in place (dev profile) |
| `mise run python-build -- --release` | Build the Python extension in release mode |
| `mise run python-test` | Build the dev extension, then run fast Python tests |
| `mise run python-typecheck` | Type-check the Python bindings with `ty` |
| `mise run python-examples` | Execute every example notebook |
| `mise run python-bench` | Benchmark the Python bindings against a release build |
| `mise run wasm-build` | Build the WASM package (web target) |
| `mise run wasm-pkg` | Build the web and Node WASM packages |
| `mise run wasm-test` | Run wasm-bindgen and Node facade tests |
| `mise run ui-fmt` | Format the UI package with Prettier (mutating) |
| `mise run ui-typecheck` | Type-check the UI package with `tsc` |
| `mise run ui-test` | Run the UI Vitest suite |
| `mise run wasm-gen-bindings` | Export TypeScript types from Rust |
| `mise run goldens-test` | Run the Rust and Python golden-test layers |
| `mise run rust-test-cov` | Rust tests with an HTML coverage report |
| `mise run python-test-cov` | Python tests with an HTML coverage report |
| `mise run wasm-test-cov` | WASM binding tests with an HTML coverage report |
| `mise run wheel-local` | Build a Python wheel for the current platform |

Do not run `cargo test` directly: it pulls in doc tests, which are owned by
`mise run rust-doc`. Use `mise run rust-test` (nextest) for unit and integration
tests.

Benchmarks are measurement tasks and stay outside `all-test`, nextest,
`rust-fmt`, `rust-lint`, and wall-clock-gated PR CI. Run `mise run rust-bench`,
or a specific `cargo bench -p <crate> --bench <target>`, to compile and measure.
`mise run python-bench-portfolio` is the materialization-specific Python
benchmark path; see
[`benchmarks/MATERIALIZATION_BENCHMARKS.md`](benchmarks/MATERIALIZATION_BENCHMARKS.md).

## Documentation

| Document | Contents |
|---|---|
| [`docs/index.md`](docs/index.md) | Public documentation map |
| [`INVARIANTS.md`](INVARIANTS.md) | Cross-crate numerical, convention, and API invariants |
| [`docs/REFERENCES.md`](docs/REFERENCES.md) | Canonical sources for formulas, conventions, and market practice |
| [`docs/CONTRACTS.md`](docs/CONTRACTS.md) | Persisted-contract matrix, strict loaders, generated schemas |
| [`docs/SERDE_STABILITY.md`](docs/SERDE_STABILITY.md) | Wire-format and schema-version policy |
| [`CHANGELOG.md`](CHANGELOG.md) | Release history, including breaking changes |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Development setup, principles, binding-change checklist |
| [`AGENTS.md`](AGENTS.md) | Repository operating rules for automated contributors |
| [`.agents/rules/`](.agents/rules/) | Rust, Python, and WASM code, testing, and documentation standards |
| [`finstack-quant-py/README.md`](finstack-quant-py/README.md) | Python package layout, stubs, parity checks, pitfalls |
| [`finstack-quant-py/parity_contract.toml`](finstack-quant-py/parity_contract.toml) | Python and WASM binding parity contract |
| [`finstack-quant-wasm/README.md`](finstack-quant-wasm/README.md) | WASM namespaces, facade, type-declaration strategy |

Rustdoc is the reference for the Rust API surface; build it with
`mise run rust-doc`. For Python, the `.pyi` stubs shipped in the package are the
IDE-facing API docs. For TypeScript, `index.d.ts` is the authoritative
IntelliSense surface.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

MIT OR Apache-2.0

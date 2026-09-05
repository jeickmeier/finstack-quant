# finstack-quant-core

Foundation crate for the Finstack Quant workspace. It owns the vocabulary every
other crate speaks: currency-tagged money, dates and calendars, market data
containers, numerical routines, a scalar expression engine, and the serde /
schema / canonicalization plumbing that keeps wire formats stable.

Directory: `finstack-quant/core`. Package / import name: `finstack-quant-core` /
`finstack_quant_core`.

## Position in the workspace

`finstack-quant-core` depends on **no** other workspace crate. Every one of the
other 13 domain crates depends on it, as do `finstack-quant-arrow`,
`finstack-quant-py`, and `finstack-quant-wasm`. Adding a dependency here is
therefore a workspace-wide decision.

Reach it either directly or through the umbrella crate, which re-exports it
unconditionally as `finstack_quant::core`:

```toml
[dependencies]
finstack-quant-core = { path = "../finstack-quant/core" }
# or
finstack-quant = { path = "../finstack-quant" }
```

Its only dev-dependency inside the workspace is
[`finstack-quant-test-utils`](../test-utils/README.md), the golden-fixture
harness.

## Public surface

Full item documentation is in the rustdoc (`cargo doc -p finstack-quant-core
--open`). This is the map of where to look.

### Money, currency, and FX

| Path | What it is |
|------|------------|
| `currency::Currency` | ISO 4217 deliverable currencies, generated from `data/iso_4217.csv`; 2 bytes, numeric discriminants, case-insensitive parsing. `XXX`, `XAU`/`XAG`/`XPT`/`XPD`, and `XDR` are deliberately excluded. |
| `money::Money` | Decimal-backed, currency-tagged amount. `checked_add`/`checked_sub` refuse to mix currencies. |
| `money::FormatOpts` | Formatting options behind `Money::format` / `format_with`. |
| `money::fx::FxProvider` | Conversion interface. |
| `money::fx::{FxMatrix, SimpleFxProvider, BumpedFxProvider}` | Concrete providers; `FxMatrix` also carries `FxMatrixState` for serialization. |
| `money::fx::{FxConfig, FxConversionPolicy, FxPolicyMeta, FxQuery, FxRateResult}` | Policy selection and the stamped result of a conversion. |
| `decimal::{f64_to_decimal, decimal_to_f64}` | The only sanctioned `f64 ↔ Decimal` bridge; both return `Result` instead of collapsing non-finite input to zero. |

### Dates, calendars, and schedules

| Path | What it is |
|------|------------|
| `dates::{Date, Duration, Month, OffsetDateTime, PrimitiveDateTime}` | Re-exports of the `time` crate types used throughout. |
| `dates::{create_date, parse_iso_date}` | Fallible constructors (no `unwrap` on `from_calendar_date`). |
| `dates::DayCount` | Day-count conventions, plus `DayCountContext` for conventions (Act/Act ICMA, Bus/252) that need a coupon period or calendar. |
| `dates::{adjust, BusinessDayConvention, HolidayCalendar, CalendarMetadata}` | Business-day rolling. |
| `dates::{calendar_by_id, calendars_by_ids, available_calendars, WEEKENDS_ONLY}` | Calendar registry; the 26 built-in calendars are generated from `data/calendars/*.json` at build time. |
| `dates::{Rule, Direction, Observed}` | Declarative holiday-rule primitives (fixed dates, nth weekday, Easter, lunar festivals, spans). |
| `dates::{CompositeCalendar, CompositeMode}` | Union and intersection of calendars. |
| `dates::{Schedule, ScheduleBuilder, ScheduleSpec, StubKind, ScheduleWarning, ScheduleErrorPolicy}` | Schedule generation with stub handling and EOM rules. |
| `dates::{Tenor, TenorUnit}` | Tenor arithmetic. |
| IMM / roll helpers on `dates` | `next_imm`, `third_wednesday`, `third_friday`, `is_cds_date`, `next_cds_date`, `sifma_settlement_date`, `SifmaSettlementClass`, equity and IMM option expiries. |
| `dates::{Period, PeriodId, PeriodKind, PeriodPlan, build_periods, build_fiscal_periods, FiscalConfig}` | Calendar and fiscal period identifiers and planning. |
| `dates::DateExt` | `quarter()`, fiscal helpers, weekday predicates. |

### Market data

| Path | What it is |
|------|------------|
| `market_data::context::MarketContext` | Immutable container keyed by curve id, with typed getters (`get_discount`, `get_forward`, …) returning `Arc<T>`. Bumping returns a new context. |
| `market_data::term_structures` | `DiscountCurve`, `ForwardCurve`, `HazardCurve`, `InflationCurve`, `BaseCorrelationCurve`, `BasisSpreadCurve`, `PriceCurve` (signed prices or, via `PriceCurveKind::VolIndex`, non-negative volatility-index levels), `ParametricCurve` (Nelson-Siegel / Svensson), `ForwardVarianceCurve`, `CreditIndexData`, plus builders and rate-calibration helpers. |
| `market_data::surfaces` | `VolSurface`, `VolCube`, `FxDeltaVolSurface` (ATM / risk-reversal / butterfly quoting converted to strikes). |
| `market_data::scalars` | `MarketScalar`, `ScalarTimeSeries`, `SeriesInterpolation`, plus `InflationIndex` with its `InflationLag`/`InflationInterpolation` conventions. |
| `market_data::dividends` | `DividendEvent`, `DividendKind`, `DividendSchedule`. |
| `market_data::bumps` | `BumpSpec`, `BumpType`, `BumpUnits`, `BumpMode`, `MarketBump`, and the `Bumpable` trait — the scenario/greeks perturbation vocabulary. |
| `market_data::diff` | Curve and scalar shift measurements between two contexts; volatility-surface evaluation and shift measurement live in models. |
| `market_data::hierarchy` | Tree of tagged nodes referencing `CurveId`s, for scenario targeting and factor scoping. |
| `market_data::fixings` | The `FIXING:{forward_curve_id}` lookup convention. |
| `market_data::traits` | `TermStructure`, `Discounting`, `Forward`, `Survival` — the minimal trait surface for polymorphic curve code; concrete curve types carry the rest. |

### Math

`math` holds anything numerically general enough that more than one domain could
want it; domain-specific numerics (stochastic processes, payoffs) belong in
`monte_carlo` or `valuations`.

| Path | What it is |
|------|------------|
| `math::stats` | `mean`, `variance`, `correlation`, `covariance`, `quantile`, NaN-tolerant `*_or_nan` variants, streaming `OnlineStats`/`OnlineCovariance`, realized-variance estimators. |
| `math::summation` | `kahan_sum`, `neumaier_sum`, `NeumaierAccumulator`. |
| `math::special_functions` | `norm_cdf`, `norm_pdf`, `erf`, `ln_gamma`, `standard_normal_inv_cdf`, Student-t CDF/inverse. |
| `math::linalg` | Cholesky (`cholesky_decomposition`, `cholesky_correlation`), `symmetric_eigen`, correlation-matrix construction and validation. |
| `math::interp` | `Interpolator`, `InterpStyle`, `ExtrapolationPolicy`, `ValidationPolicy` and the strategies (`LinearStrategy`, `LogLinearStrategy`, `CubicHermiteStrategy`, `MonotoneConvexStrategy`, `PiecewiseQuadraticForwardStrategy`). |
| `math::solver` / `math::solver_multi` | `NewtonSolver`, `BrentSolver`, `Solver` trait; `LevenbergMarquardtSolver` and `AnalyticalDerivatives` for systems. |
| `math::integration` | Gauss-Legendre (fixed, composite, adaptive), Gauss-Hermite, Gauss-Laguerre. |
| `math::random` | `Pcg64Rng`, `RandomNumberGenerator`, `SobolRng` (up to `MAX_SOBOL_DIMENSION` = 40), `BrownianBridge`, Poisson inversion, `box_muller_transform`. |
| `math::distributions` / `math::probability` | Binomial and chi-squared helpers, `CorrelatedBernoulli`, `correlation_bounds`. |
| `math::compounding` | `Compounding` conversions between simple, periodic, and continuous rates. |
| `math::{time_grid, piecewise, fractional, consecutive}` | `TimeGrid` with `map_date_to_step`/`map_exercise_dates_to_steps`; validated left-continuous piecewise-constant curves; fractional Brownian motion kernels, fBM covariance, and Mittag-Leffler for rough-vol models; `count_consecutive` streaks. |

### Types, errors, and configuration

| Path | What it is |
|------|------------|
| `types::{Rate, Bps, Percentage}` | Rate wrappers. See "Rate units" below. |
| `types::{Id, CurveId, InstrumentId, IssuerId, IndexId, DealId, PoolId, PriceId, CalendarId, UnderlyingId}` | Phantom-typed identifiers; `#[serde(transparent)]`, so they stay plain strings on the wire. |
| `types::{CreditRating, RatingLabel}` | Neutral rating enum, parsing, ordering, and stable labels. Rating-factor calculations live in `finstack-quant-models`. |
| `types::{Attributes, BarrierType}` | Attribute bags for matching/metadata; barrier taxonomy. |
| `error::{Error, InputError, NonFiniteKind, Result}` | The unified error type, re-exported at the crate root. `Error` and `InputError` are `#[non_exhaustive]`; match with a wildcard arm. |
| `config::{FinstackConfig, RoundingMode, RoundingPolicy, CurrencyScalePolicy, ToleranceConfig, ConfigExtensions}` | Explicit, caller-supplied configuration — there is no global state. |
| `config::{RoundingContext, ResultsMeta, NumericMode, results_meta, rounding_context_from}` | Policy stamps attached to result envelopes. |
| `validation::{require, require_or, require_with}` | Convention-agnostic invariant checks returning `Result`. |
| `explain::{ExplainOpts, ExplanationTrace, TraceEntry}` | Opt-in computation tracing, off and zero-cost by default. |
| `prelude` | `Currency`, `Money`, FX traits, common date types, the main curve types, `FinstackConfig`, rate types, `Error`/`Result`. |

`rating_scales` holds the neutral shared scorecard-scale registry
(`data/rating_scales/`). Product-independent credit engines and assumptions
live in `finstack-quant-models::credit`.

### Expression engine

`expr` is a scalar DAG engine over `&[f64]` columns, used by `statements` for
formula evaluation. `Expr`/`ExprNode` build the AST, `Function` enumerates the
supported operators (lags, diffs, cumulatives, rolling windows, EWM, reducers),
`SimpleContext` maps column names to positions, `CompiledExpr::eval` plans and
runs it, and `EvaluationResult` carries values plus metadata. Windows are
row-count windows, not calendar-time windows.

### Serialization, schema, and contracts

| Path | What it is |
|------|------------|
| `table::{TableEnvelope, TableColumn, TableColumnData, TableColumnRole}` | The canonical columnar interchange type. |
| `canonical::{to_canonical_bytes, canonical_bytes_of_value, content_hash, CANONICAL_VERSION}` | Deterministic JSON bytes (recursively key-sorted, array order preserved, non-finite floats rejected) and versioned content hashes. |
| `contract::{ContractDescriptor, LoadLimits, Diagnostic, Severity, LoadPhase, ValidationReport, ContractError}` | Shared vocabulary for loading persisted artifacts under resource limits. |
| `schema` | Deterministic JSON Schema assembly: `SerdeSchema`, `SchemaArtifact`, `ARTIFACTS`, `COMMON_SCHEMA_DEFINITIONS`, `run_schema_generator`, `run_schema_index_generator`, `project_llm`. |
| `wire` | Canonical serde representations for types whose storage form cannot describe its own JSON contract to `schemars`. |
| `serde_guard::UnknownFieldGuard` | Restores `deny_unknown_fields` strictness for structs that use `#[serde(flatten)]`, which serde cannot do natively. |
| `versions` | Centralized model-version strings stamped into calibration reports. |

Note that `finstack_quant::schema` (on the umbrella crate) is a different,
higher-level module that indexes artifacts across all domains;
`finstack_quant_core::schema` is the assembly machinery those generators use.

`HashMap`/`HashSet` are re-exported at the crate root as `rustc_hash` aliases so
iteration order is deterministic across the workspace.

## Example

`dates` re-exports the `time` types rather than wrapping them, so a caller that
writes date literals needs `time = "0.3"` (with the `macros` feature) as its own
dependency; `dates::create_date` avoids that.

```rust
use finstack_quant_core::cashflow::npv;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use time::macros::date;

fn main() -> finstack_quant_core::Result<()> {
    let base = date!(2025 - 01 - 01);

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.952), (5.0, 0.78)])
        .build()?;

    // MarketContext is immutable; `insert` returns a new context.
    let market = MarketContext::new().insert(curve);
    let usd_ois = market.get_discount("USD-OIS")?; // Arc<DiscountCurve>

    let flows = [
        (date!(2025 - 07 - 01), Money::new(25_000.0, Currency::USD)?),
        (date!(2026 - 01 - 01), Money::new(1_025_000.0, Currency::USD)?),
    ];

    // Flows dated on or before `base` are excluded (pricing-standard cutoff).
    let pv = npv(usd_ois.as_ref(), base, &flows)?;
    assert_eq!(pv.currency(), Currency::USD);

    // Currency safety is enforced, not converted away.
    assert!(pv.checked_add(Money::new(1.0, Currency::EUR)?).is_err());
    Ok(())
}
```

## Conventions that will bite you

**Decimal vs f64.** `Money` stores `rust_decimal::Decimal` plus a `Currency`.
`Money::new` accepts `f64` and `amount()` returns `f64`;
`Money::from_decimal`/`amount_decimal()` are the lossless path. Curves, rates,
vols, correlations, greeks, and solver internals use `f64`. Wrapping an `f64`
result in `Money` gives it currency semantics and Decimal storage — it does not
make the preceding arithmetic Decimal-exact. See
[INVARIANTS.md §1](../../INVARIANTS.md).

**Money construction does not round.** `Money::new` preserves the raw
finite amount; ISO 4217 minor-unit quantization must be asked for explicitly via
`Money::new_with_config` or a `RoundingContext`.
Construction returns `Result`; non-finite and unrepresentable amounts are rejected.

**Currency safety.** `Money` arithmetic is fallible and refuses mixed
currencies. Cross-currency work goes through an `FxProvider`, and the applied
`FxConversionPolicy` is recorded in the result rather than left implicit.

**Rate units.** `Rate` holds decimals (5% is `0.05`), `Percentage` holds whole
percent (25% is `25.0`), and `Bps` holds basis points (25 bp is `25.0`). Curve
knot values are decimals. Fields and parameters named `*_bp`/`*_bps` are the
exception and carry basis points.

**Day-count context.** Some conventions cannot be evaluated from two dates
alone. `DayCount::ActActIsma` needs the coupon period and `DayCount::Bus252`
needs a calendar; both are supplied through `DayCountContext`, and the
convention returns an error rather than guessing when it is missing.

**Determinism.** Core introduces no parallelism of its own — it has no `rayon`
dependency, so there is no parallel/serial split to reconcile here. (Its types
are still `Send + Sync`; the FX providers guard their caches with `std::sync`
locks.) Randomness is explicitly seeded: `Pcg64Rng` and `SobolRng` are
constructed with a seed and never read the system clock or a thread-local
generator. Hash containers use `rustc_hash` aliases for stable iteration order.
Canonical JSON sorts keys recursively and rejects non-finite floats instead of
emitting `null`.

**Serde strictness.** Inbound types carry `#[serde(deny_unknown_fields)]`;
structs that use `#[serde(flatten)]` cannot, so they end with a
`serde_guard::UnknownFieldGuard` field that restores the same behavior. Field
names are stable wire contract, not an
implementation detail — see [docs/SERDE_STABILITY.md](../../docs/SERDE_STABILITY.md)
and [docs/CONTRACTS.md](../../docs/CONTRACTS.md).

**No DataFrame dependency.** There is no Polars (or pandas-equivalent) type in
this crate. `core::table::TableEnvelope` is the canonical serializable columnar
surface; bindings convert it to a host table type at the boundary, and
[`finstack-quant-arrow`](../arrow-interchange/README.md) converts it to an Arrow
`RecordBatch`. Do not introduce ad-hoc series types.

## Generated assets

Generated Rust reaches this crate two ways, and the difference matters when you
change a data file.

**Derived by `build.rs` into `OUT_DIR`.** These are read on every build, so
editing the input changes the next build:

| Input | Generated into `OUT_DIR` | Included by |
|-------|--------------------------|-------------|
| `data/calendars/*.json` (26 calendars) | `calendars.rs` — per-calendar `Rule` tables and the registry, evaluated at runtime (there are no precomputed holiday bitsets) | `src/dates/calendar/mod.rs` |
| `data/sifma_settlements.csv` | `sifma_settlements_generated.rs` | `src/dates/imm.rs` |

**Committed under `src/generated/`.** `build.rs` does not derive these from the
data files, so editing the CSV alone changes nothing:

| File | Regenerated by | Paired data |
|------|----------------|-------------|
| `currency_generated.rs` (`Currency` enum, `MINOR_UNITS`) | no in-repo generator; `build.rs` only copies the file into `OUT_DIR` for `src/currency.rs` to `include!` | `data/iso_4217.csv` |
| `cny_generated.rs` (Chinese New Year dates) | no in-repo generator | `data/chinese_new_year.csv` |
| `festivals_generated.rs` (Dragon Boat, Mid-Autumn) | `data/gen_lunar_festivals.py`, which rewrites the Rust table and both CSVs | `data/dragon_boat.csv`, `data/mid_autumn.csv` |

The last two are `include!`d straight from `src/dates/calendar/algo.rs`. Change a
committed table only alongside its paired data file.

JSON Schemas for the market-data contracts are checked in under `schemas/` and
produced by the `gen_core_schemas` binary. Regenerate with
`mise run rust-gen-schemas`; verify with `mise run rust-check-schemas`.

## Cargo features

| Feature | Default | Effect |
|---------|---------|--------|
| `json-schema` | on | Optional `schemars` derives and the `schema` generation module. WASM builds this crate with `default-features = false` so the schema stack stays off that graph. |
| `ts_export` | off | Derives `ts_rs::TS` on the `contract::diagnostics` types for TypeScript declaration export. |

Serde and tracing hooks compile unconditionally; there is no `std`/`no_std`
split (the crate uses the standard library). Golden-test helpers live in
`finstack-quant-test-utils`, not behind a feature here.

## Bindings

- **Python**: `finstack_quant.core`, with submodules `config`, `currency`,
  `dates`, `market_data`, `math`, `money`, `rating_scales`,
  `schema`, `table`, `types`, plus the `FinstackError` exception class. Binding
  source is `finstack-quant-py/src/bindings/core/`.
- **WASM**: the `core` namespace exported from `finstack-quant-wasm/index.js`
  (`core.Currency`, `core.Money`, `core.DayCount`, `core.createDate`,
  `core.DiscountCurve`, …). Binding source is
  `finstack-quant-wasm/src/api/core/`.

Not every Rust item is bound. `finstack-quant-py/parity_contract.toml` is the
authority on what the namespaces must contain.

## Testing and benchmarks

- Integration tests: [`tests/README.md`](tests/README.md)
- Golden fixtures: [`tests/golden/README.md`](tests/golden/README.md)
- Criterion suites: [`benches/README.md`](benches/README.md)

## Verification

```bash
cargo nextest run -p finstack-quant-core
cargo fmt -p finstack-quant-core -- --check
cargo clippy -p finstack-quant-core --lib --bins --tests --examples --all-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p finstack-quant-core --no-deps
cargo test -p finstack-quant-core --doc
```

Workspace-wide equivalents: `mise run rust-test`, `mise run rust-lint`,
`mise run rust-doc`. Do not run a bare `cargo test`; it pulls in doc tests
across the workspace. `cargo test --doc` above is the deliberate exception.

## References

Quantitative and standards references: [`docs/REFERENCES.md`](../../docs/REFERENCES.md).

## License

MIT OR Apache-2.0

## Market Data Module (core)

The `market_data` module in `finstack-core` provides yield curves, credit
curves, volatility surfaces, FX, and scalar market data used across valuations,
scenarios, and portfolios.

- **Term structures**: one-dimensional curves for discount factors, forward rates, credit hazard rates, and inflation.
- **Surfaces**: two-dimensional volatility surfaces indexed by expiry and strike.
- **Scalars and time series**: spot prices, FX rates, indices, and generic scalar time series.
- **Market context**: `MarketContext` stores curves, surfaces, scalars, histories, FX, and collateral mappings.
- **Scenario/risk utilities**: bumping APIs (`bumps.rs`) and shift measurement utilities (`diff.rs`).
- **Dividends**: shared dividend schedules for equity and ETF valuations.

The module uses deterministic data structures, typed IDs, and serde-friendly
state objects. Higher-level `valuations`, `scenarios`, and `portfolio` crates
consume these types directly.

Convention notes:

- Hazard curves require strictly positive knot times; use the first positive pillar
  as the start of the published term structure instead of encoding a synthetic `t=0`
  hazard node.
- Parallel inflation bumps are applied in zero-inflation-rate space so a `+x%` shift
  moves annualized inflation rates consistently across tenors instead of scaling CPI
  levels directly.

---

## Module Structure

- **`mod.rs`**
  - Public entrypoint for the market data module.
  - Documents high-level concepts (discount/forward/hazard/inflation curves, vol surfaces, scalars, `MarketContext`).
  - Re-exports:
    - Submodules: `bumps`, `context`, `diff`, `dividends`, `scalars`, `surfaces`, `term_structures`, `traits`.
    - Helpers: `math::interp::utils::validate_knots`.
    - Dividend schedule types for ergonomic access.

- **`context.rs`**
  - Defines:
    - `MarketContext`: registry for market data; cheap to clone (Arc-based), builder-style insert APIs (`insert_discount`, `insert_forward`, `insert_surface`, `insert_price`, `insert_fx`, etc.), and type-safe getters (`get_discount`, `surface`, `price`, `series`, `collateral`, …).
    - `CurveStorage`: enum wrapper for heterogeneous curve storage (`Discount`, `Forward`, `Hazard`, `Inflation`, `BaseCorrelation`) with helpers like `curve_type()` and type filters.
    - `ContextStats`: lightweight statistics struct returned by `MarketContext::stats()`.
  - Scenario helpers:
    - `MarketContext::bump` is the single entry point for applying [`MarketBump`] lists (curves, FX, vol buckets, base correlation buckets).
    - `MarketContext::roll_forward` implements constant-curve roll-down scenarios.
  - Serialization:
    - `CurveState`: tagged enum for serializing any curve type.
    - `CreditIndexState` and `MarketContextState`: DTOs for persisting complete context snapshots.
    - `Serialize`/`Deserialize` implementations for `CurveStorage` and `MarketContext` round-trip through the `*State` DTOs.
  - **Public surface**:
    - Use `new`, `insert_*`, typed getters, scenario helper (`bump`), stats, and serde state types as the stable surface.
    - Treat internal storage details (HashMaps, instrument registry, `market_history`) as private; they may change.

- **`term_structures/`**
  - `mod.rs`: documentation and re-exports for all curve types.
  - `discount_curve.rs`: discount factor curves (`DiscountCurve`) implementing:
    - `TermStructure` + `Discounting` traits.
    - Builder pattern with `base_date`, `day_count`, `knots`, `interp`, and extrapolation controls.
    - Optional `fx_policy` stamp when bootstrap used cross-currency assumptions.
  - `forward_curve.rs`: forward-rate curves (`ForwardCurve`) with tenor-aware builders (e.g., 3M forward) and knot-based interpolation; optional `fx_policy` stamp.
  - `hazard_curve.rs`: credit hazard/survival curves (`HazardCurve`) with survival/probability helpers; optional `fx_policy` stamp.
  - `inflation.rs`: real/breakeven inflation term structures (`InflationCurve`) built from CPI levels.
  - `credit_index.rs`: credit index aggregates (`CreditIndexData`) referencing component hazard and base correlation curves.
  - `base_correlation.rs`: base correlation curves for tranche pricing (`BaseCorrelationCurve`).
  - All curve types:
    - Use validated knot sets (via `validate_knots`) and pluggable interpolation (`InterpStyle`).
    - Implement `TermStructure` and domain-specific traits from `traits.rs` where appropriate.
    - Support serde via `*State` DTOs when runtime types need explicit wire shapes.

- **`surfaces/`**
  - `mod.rs`: documentation and re-exports for surface types.
  - `vol_surface.rs`: bilinear volatility surface on a strike grid (`VolSurface`).
  - `fx_delta_vol_surface.rs`: FX smiles quoted in delta space (`FxDeltaVolSurface`);
    ATM / 25-delta RR/BF (optional 10-delta wings), forward-delta convention,
    Garman-Kohlhagen delta-to-strike conversion, and materialization to `VolSurface`.

- **`scalars/`**
  - `mod.rs`: documentation and re-exports.
  - `primitives.rs`:
    - `MarketScalar`: enum for single-value market observables (e.g., equity spot, FX rate, generic scalar).
    - `ScalarTimeSeries`: generic `(Date, f64)` time series with optional interpolation (`SeriesInterpolation`) and metadata.
  - `inflation_index.rs`:
    - `InflationIndex`: CPI/RPI time series with lag/interpolation support.
  - `storage.rs`:
    - Internal storage for time series; not typically used directly by consumers.

- **`bumps.rs`**
- Scenario bump specification types:
  - `BumpMode` (additive vs multiplicative).
  - `BumpUnits` (basis points, percent, fraction, factor).
  - `BumpType` (`Parallel`, `TriangularKeyRate` with explicit bucket neighbors).
  - `BumpSpec`: unified bump description (mode, units, value, type) with helpers like:
    - `BumpSpec::parallel_bp`, `BumpSpec::triangular_key_rate_bp`.
    - Domain-specific helpers (`inflation_shift_pct`, `correlation_shift_pct`, `multiplier`).
  - `MarketBump`: heterogeneous bump enum for curves, FX, volatility buckets, and base correlation buckets.
  - Integrates with curve/surface/scalar types via internal `Bumpable` traits.

- **`diff.rs`**
  - Market shift measurement helpers between two `MarketContext` instances:
    - `TenorSamplingMethod` (`Standard`, `Dynamic`, `Custom`) controls sampling points along a curve.
  - `measure_discount_curve_shift` for rate shifts in basis points.
  - Additional helpers for hazard spreads and volatility surfaces.
  - Used for P&L attribution, DV01/CS01-style risk reports, and calibration diagnostics.

- **`dividends.rs`**
  - Shared dividend schedule types (`DividendSchedule`, cash/yield/stock events) keyed by `CurveId`.
  - Integrated with `MarketContext` via `insert_dividends` / `dividend_schedule` helpers.

- **`traits.rs`**
  - Minimal trait surface for polymorphism:
    - `TermStructure`: base trait with `id() -> &CurveId`.
    - `Discounting`: discount curve abstraction with `base_date`, `df(t)`, and a default `day_count`.
    - `Forward`: forward-rate abstraction with `rate(t)` and `rate_period(t1, t2)`.
    - `Survival`: hazard/survival abstraction with `sp(t)` for survival probabilities.
  - The traits stay small; most functionality lives on concrete curve types.

---

## Core Concepts and Types

### Term Structures and Surfaces

- **Discount curves (`DiscountCurve`)**
  - Map year fractions from a base date to discount factors.
  - Provide helpers for zero rates, forwards, and rolling.
  - Implement `TermStructure` + `Discounting` traits.
- **Forward curves (`ForwardCurve`)**
  - Represent simple or period-averaged forward rates over a tenor.
  - Builder specifies base date, tenor, day count, and knot values.
- **Hazard curves (`HazardCurve`)**
  - Encode credit hazard rates and survival probabilities.
  - Used by credit pricers in `valuations`.
- **Inflation curves (`InflationCurve`)**
  - Built from CPI levels and base CPI, enabling real/nominal conversions.
- **Base correlation and credit index data**
  - `BaseCorrelationCurve` plus `CreditIndexData` model tranche correlation and index-level credit data.
- **Volatility surfaces (`VolSurface`)**
  - Two-dimensional matrices of implied vols by expiry and strike.
  - Builder validates grid dimensions and supports bilinear interpolation and bucket-level bumps.
- **FX delta vol surfaces (`FxDeltaVolSurface`)**
  - FX option vols quoted as ATM DNS, 25-delta risk reversal, and 25-delta butterfly per expiry.
  - Optional 10-delta wings; forward delta (premium-unadjusted) convention.
  - Converts to strike-axis `VolSurface` via Garman-Kohlhagen for existing pricing engines.

All curve and surface types:

- Use year-fraction time coordinates backed by `dates::DayCount`.
- Validate knots and grid structure up-front.
- Support serde via `*State` DTOs or direct derives with stable field names.

### Scalars and Time Series

- **`MarketScalar`**
  - Enum wrapper for single-value market observables (e.g., equity spot, FX rate, index level).
  - Carries `Money` and `Currency` where spot values need currency tags.
- **`ScalarTimeSeries`**
  - Generic `(Date, f64)` time series with interpolation (`SeriesInterpolation`) and optional metadata.
  - Used for things like historical vol, macro series, and generic market history.
- **`InflationIndex`**
  - CPI/RPI-style index with:
    - Observations as `(Date, level)` pairs.
    - Configurable interpolation (e.g., linear).
    - Currency tagging and lag/seasonality support.

These types are stored inside `MarketContext` under `prices`, `series`, and `inflation_indices`.

### MarketContext

`MarketContext` stores the market data used in a valuation run:

- **Builder-style inserts**
  - Curves: `insert_discount`, `insert_forward`, `insert_hazard`, `insert_inflation`, `insert_base_correlation`.
  - Surfaces: `insert_surface`, `insert_fx_delta_vol_surface`.
  - Scalars & time series: `insert_price`, `insert_series`.
  - Inflation indices: `insert_inflation_index`.
  - Credit indices: `insert_credit_index`.
  - FX: `insert_fx`.
  - Collateral: `map_collateral` (CSA code → discount curve ID).
  - Dividends and market history: `insert_dividends`, `insert_market_history`.
- **Type-safe getters**
  - Curves: `get_discount`, `get_forward`, `get_hazard`, `get_inflation_curve`, `get_base_correlation` (and `_ref` borrowing variants).
  - Surfaces and indices: `get_surface`, `get_fx_delta_vol_surface`, `get_price`, `get_series`, `get_inflation_index`, `dividend_schedule`, `credit_index`, `collateral`.
  - Introspection: `curve_ids`, `curves_of_type`, `count_by_type`, `stats`, `is_empty`, `total_objects`.
- **Scenario support**
  - `bump` for curve/surface/price/time-series bumps keyed by `CurveId`.
  - `bump` is also the heterogeneous entry point for `MarketBump` lists (including FX and bucket-level shifts).
  - `roll_forward(days)` for constant-curve roll-down scenarios.
  - `bump_fx_spot` for FX-specific percentage bumps (via `FxMatrix`).
- **Serialization**
  - `MarketContext` serializes via `MarketContextState` with stable field names:
    `curves`, `surfaces`, `fx_delta_vol_surfaces`, `prices`, `series`,
    `inflation_indices`, `credit_indices`, and `collateral`.
  - `MarketContextState` is the wire shape for Python/WASM bindings and long-lived storage.

---

## Usage Examples

### Build a Simple MarketContext with Curves

```rust
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{
    DiscountCurve,
    ForwardCurve,
    HazardCurve,
};
use finstack_core::math::interp::InterpStyle;
use time::macros::date;

let base = date!(2025 - 01 - 01);

let disc = DiscountCurve::builder("USD-OIS")
    .base_date(base)
    .knots([(0.0, 1.0), (5.0, 0.88)])
    .interp(InterpStyle::MonotoneConvex)
    .build()
    ?;

let fwd3m = ForwardCurve::builder("USD-SOFR3M", 0.25)
    .base_date(base)
    .knots([(0.0, 0.03), (5.0, 0.04)])
    .interp(InterpStyle::Linear)
    .build()
    ?;

let hazard = HazardCurve::builder("USD-CRED")
    .base_date(base)
    .knots([(1.0, 0.01), (10.0, 0.015)])
    .build()
    ?;

let ctx = MarketContext::new()
    .insert(disc)
    .insert(fwd3m)
    .insert(hazard);

assert!(ctx.get_discount("USD-OIS").is_ok());
assert!(ctx.get_forward("USD-SOFR3M").is_ok());
assert!(ctx.get_hazard("USD-CRED").is_ok());
# Ok::<(), finstack_core::Error>(())
```

### Add Scalars, Time Series, and Inflation Indices

```rust
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::{
    MarketScalar,
    ScalarTimeSeries,
    SeriesInterpolation,
    InflationIndex,
};
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use time::macros::date;

let spot = MarketScalar::Price(Money::new(101.5, Currency::USD));

let ts = ScalarTimeSeries::new(
    "US-CPI-TS",
    vec![
        (date!(2024 - 01 - 31), 100.0),
        (date!(2024 - 02 - 29), 101.0),
    ],
    None,
)?
.with_interpolation(SeriesInterpolation::Linear);

let index = InflationIndex::new(
    "US-CPI",
    vec![
        (date!(2024 - 01 - 31), 100.0),
        (date!(2024 - 02 - 29), 101.0),
    ],
    Currency::USD,
)?
// configure interpolation/lag as needed
;

let ctx = MarketContext::new()
    .insert_price("AAPL", spot)
    .insert_series(ts)
    .insert_inflation_index("US-CPI", index);

// Lookups are type-safe and validated
let price = ctx.get_price("AAPL")?;
let series = ctx.get_series("US-CPI-TS")?;
let cpi = ctx
    .get_inflation_index("US-CPI")
    .expect("Inflation index present");

assert!(matches!(price, MarketScalar::Price(_)));
assert_eq!(series.id().as_str(), "US-CPI-TS");
assert_eq!(cpi.id, "US-CPI");
# Ok::<(), finstack_core::Error>(())
```

### Apply Parallel and Key-Rate Bumps

```rust
use finstack_core::collections::HashMap;
use finstack_core::market_data::context::{MarketContext, BumpSpec};
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::types::CurveId;
use time::macros::date;

let base = date!(2025 - 01 - 01);
let curve = DiscountCurve::builder("USD-OIS")
    .base_date(base)
    .knots([(0.0, 1.0), (5.0, 0.9)])
    .build()
    ?;

let ctx = MarketContext::new().insert(curve);

// 100bp parallel bump
let mut bumps = HashMap::default();
bumps.insert(CurveId::from("USD-OIS"), BumpSpec::parallel_bp(100.0));

let bumped = ctx.bump(bumps)?;
let bumped_curve = bumped.get_discount("USD-OIS")?;

assert_eq!(bumped_curve.id(), &CurveId::from("USD-OIS"));
# Ok::<(), finstack_core::Error>(())
```

For heterogeneous scenarios (curves, FX, vol buckets, base correlation), build a list of `MarketBump` and call `MarketContext::bump`.

### Measure Market Shifts Between Contexts

```rust
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::diff::{measure_discount_curve_shift, TenorSamplingMethod};
use finstack_core::types::CurveId;

fn measure_shift(market_t0: MarketContext, market_t1: MarketContext) -> finstack_core::Result<f64> {
    let shift_bp = measure_discount_curve_shift(
        &CurveId::from("USD-OIS"),
        &market_t0,
        &market_t1,
        TenorSamplingMethod::Standard,
    )?;

    println!("USD-OIS moved {shift_bp} basis points");
    Ok(shift_bp)
}
```

Use `TenorSamplingMethod::Dynamic` or `Custom` when you need knot-aware or instrument-specific bucket definitions.

### Serialize and Deserialize a MarketContext

```rust
use finstack_core::market_data::context::MarketContext;
use serde_json;

// Build or obtain a MarketContext
let ctx = MarketContext::new();

// Serialize to JSON (using MarketContextState under the hood)
let json = serde_json::to_string_pretty(&ctx)?;

// Deserialize back
let round_tripped: MarketContext = serde_json::from_str(&json)?;

assert_eq!(ctx.stats().total_curves, round_tripped.stats().total_curves);
```

---

## Extending

New term structures go under `term_structures/` with a builder, validated knots,
`TermStructure` trait impls, and optional `*State` DTOs. Wire into `MarketContext`
when the type is first-class. New surfaces follow the same pattern under
`surfaces/`. Preserve stable serde field names and add tests under
`finstack/core/tests/market_data/`.

# Range Accrual

Range accrual notes: a coupon that accrues only for observations where the
underlying stays inside `[lower_bound, upper_bound]`. Priced by static
replication with digital call spreads (the default) or by GBM Monte Carlo.

```text
coupon = coupon_rate × accrual_year_fraction × (observations in range / total observations)
```

## Public surface

Import path: `finstack_quant_valuations::instruments::exotics::range_accrual`
(`RangeAccrual` is also re-exported at `finstack_quant_valuations::instruments`).

| Item | Purpose |
|------|---------|
| `RangeAccrual` | The instrument. `RangeAccrual::builder()`, `example()` (relative bounds), `example_absolute_bounds()`. |
| `BoundsType` | `Absolute` (default) or `RelativeToInitialSpot`. |
| `monte_carlo::RangeAccrualPayoff` | The MC payoff, for direct use with the Monte Carlo engine. |

Useful methods: `validate()`, `accrual_year_fraction()`,
`effective_lower_bound(initial_spot)`, `effective_upper_bound(initial_spot)`.

The `pricer` submodule is `pub(crate)`: `npv_analytic`,
`RangeAccrualStaticReplicationPricer` and `RangeAccrualMcPricer` are named below
to explain the pricing paths, but they cannot be called from outside the crate.
Price through `Instrument::value` / `price_with_metrics`, or through the
registry with an explicit `ModelKey`.

## Module layout

```
range_accrual/
├── mod.rs          # re-exports and module overview
├── types.rs        # RangeAccrual, BoundsType, builder, examples, validate
├── pricer.rs       # RangeAccrualStaticReplicationPricer, RangeAccrualMcPricer, npv_analytic
├── monte_carlo.rs  # RangeAccrualPayoff for the shared MC path-dependent pricer
└── metrics/        # rho.rs plus generic FD greeks registered in mod.rs
```

## Pricing

Registered in [`src/pricer/exotics.rs`](../../../pricer/exotics.rs):

| `ModelKey` | Pricer |
|-----------|--------|
| `StaticReplication` | `RangeAccrualStaticReplicationPricer` — **the instrument's `default_model()`** |
| `MonteCarloGBM` | `RangeAccrualMcPricer` |

### Static replication (default)

Replicates the payoff as a portfolio of digitals (finite-width binary call
spreads) at each future observation date, so volatility skew, smile and term
structure come straight off the surface. No simulation variance, and generally
more accurate than GBM Monte Carlo for anything with meaningful smile exposure.

The forward is written as `S / DF_asset(as_of → t_obs) · exp(−(q + quanto_drift)·t_obs)`,
which keeps the carry exact on the model/volatility clock instead of
annualizing a curve-native zero rate when the curve and instrument day counts
differ.

### Monte Carlo

Selected **only** when the caller explicitly asks for `ModelKey::MonteCarloGBM`.
GBM paths with discrete observations; the accrual fraction is the proportion of
simulated fixings inside the effective bounds.

The `mc_seed_scenario` entry on `metric_pricing_overrides` controls the
deterministic random stream *after* Monte Carlo has been selected — it never
selects the model. Absent, the seed derives from the instrument id and the
scenario label `"base"`.

Both paths discount the payment-date cashflow back to `as_of`, apply
`BoundsType` to get effective bounds, and fold in historical fixings for
mid-life valuations. When every observation is in the past, both fall through to
the known-value computation.

**Rate-linked notes are not priced here.** If `rate_index_id` is set, both the
static-replication and the GBM Monte Carlo pricer return a validation error
rather than treating a rate as an equity spot.

## Bounds

| `BoundsType` | Interpretation | Example |
|--------------|----------------|---------|
| `Absolute` (default) | Absolute price or rate levels | `lower = 0.04`, `upper = 0.06` for a SOFR range |
| `RelativeToInitialSpot` | Multipliers of the initial spot | `lower = 0.95`, `upper = 1.05` for a ±5% equity range |

## Construction

```rust
use finstack_quant_valuations::instruments::exotics::range_accrual::{BoundsType, RangeAccrual};

// Equity-linked, 95%–105% of initial spot, monthly observations.
let equity_range = RangeAccrual::example();

// Rate-linked bounds (4%–6%), absolute.
let rate_range = RangeAccrual::example_absolute_bounds();
```

Building one explicitly — the entry point is `RangeAccrual::builder()`. The
derived `RangeAccrualBuilder` type is not re-exported from the module, so
`RangeAccrualBuilder::new()` cannot be named from outside the crate even though
the derive generates it:

```rust
use finstack_quant_valuations::instruments::exotics::range_accrual::{BoundsType, RangeAccrual};
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use time::macros::date;

let note = RangeAccrual::builder()
    .id(InstrumentId::new("RANGE-SPX-1Y"))
    .underlying_ticker("SPX".to_string())
    .observation_dates(vec![
        date!(2024 - 01 - 31),
        date!(2024 - 02 - 29),
        date!(2024 - 03 - 31),
    ])
    .accrual_start_date(date!(2023 - 12 - 31))
    .lower_bound(0.95)
    .upper_bound(1.05)
    .bounds_type(BoundsType::RelativeToInitialSpot)
    .coupon_rate(0.08)
    .notional(Money::new(100_000.0, Currency::USD)?)
    .day_count(DayCount::Act365F)
    .discount_curve_id(CurveId::new("USD-OIS"))
    .spot_id("SPX-SPOT".into())
    .vol_surface_id(CurveId::new("SPX-VOL"))
    .div_yield_id_opt(Some(PriceId::new("SPX-DIV")))
    .attributes(Attributes::new())
    // Mid-life: 3 of 6 past observations were in range.
    .past_fixings_in_range_opt(Some(3))
    .total_past_observations_opt(Some(6))
    .build()?;
```

## Quanto

Quanto configuration is a single nested `quanto: Option<QuantoSpec>` field, not
loose `quanto_correlation` / `fx_vol_surface_id` fields. `QuantoSpec` is defined
at `instruments::QuantoSpec`:

| Field | Meaning |
|-------|---------|
| `asset_currency` | Currency of the asset price and financing |
| `asset_discount_curve_id` | Asset-currency financing curve |
| `correlation` | Asset correlation to payoff-currency units per asset-currency unit, in `[-1, 1]` |
| `fx_vol_surface_id` | FX volatility surface on that same quote orientation |
| `fx_spot_id` | Required positive FX spot scalar, in payoff currency per asset currency |

The asset forward is `S / DF_asset · exp(-(q + ρ σ_asset σ_FX) t)`.
Coupon cashflows use `DF_payoff`. The FX volatility is sampled at the forward
FX rate `FX_spot · DF_asset / DF_payoff`. Both financing curves and the FX
spot/surface must resolve. Monetary asset and FX scalars must carry the asset
and payoff currencies, respectively. GBM retains its documented flat-volatility
approximation; static replication resolves these inputs for each observation.
The active asset-volatility override drives both the asset distribution and
the covariance adjustment. Without an override, the asset surface supplies
that volatility. Asset and FX sources must be compatible with unshifted Black
pricing; normal or displaced surface quotes produce a validation error.

## Validation

`RangeAccrual::validate()` checks:

- at least one observation date, sorted strictly ascending;
- `lower_bound`, `upper_bound`, `coupon_rate` and `notional` are finite, with
  `notional > 0`, `lower_bound < upper_bound`, `coupon_rate >= 0`;
- `past_fixings_in_range` and `total_past_observations` are both set or both
  unset, with `in_range <= total`;
- `payment_date` (when set) is on or after the final observation date;
- the accrual factor from `accrual_start_date` to the final observation is
  finite and positive;
- `rate_index_id`, `projection_curve_id` and `reference_tenor` are supplied
  together (all three or none).

## Metrics

Registered for `InstrumentType::RangeAccrual` in `metrics/mod.rs`, all
finite-difference:

| `MetricId` | Calculator |
|-----------|-----------|
| `Delta`, `Gamma` | `GenericFdDelta` / `GenericFdGamma` |
| `Vega`, `Vanna`, `Volga` | `GenericFdVega` / `GenericFdVanna` / `GenericFdVolga` |
| `Rho` | `rho::RhoCalculator` |
| `Dv01`, `BucketedDv01` | Parallel and triangular key-rate curve risk |

`Theta` is registered universally by `metrics::standard_registry()`.

## Market dependencies

Declared by `Instrument::market_dependencies`: the discount curve, the
projection curve when `projection_curve_id` is set, the `spot_id` scalar, the
volatility surface at both `lower_bound` and `upper_bound` strikes, and the
dividend-yield scalar when `div_yield_id` is set.

## Bindings

Reachable from Python and WASM through the JSON envelope
(`InstrumentJson::RangeAccrual` inside `finstack_quant.instrument/1`):

- **Python**: `finstack_quant.valuations.instruments.price_instrument(...)`.
- **WASM**: `valuations.instruments.priceInstrument`.

A related standalone helper for the callable variant is exposed as
`finstack_quant.valuations.callable_range_accrual_accrued` (Python) and
`valuations.callableRangeAccrualAccrued` (WASM).

## Limitations

- GBM dynamics only: no stochastic volatility, no jumps.
- Discrete observation only; no continuous-monitoring adjustment.
- Rate-linked range accruals (`rate_index_id` set) are rejected by both
  registered pricers.
- Quanto handling uses correlation and vol inputs; there is no full
  multi-currency simulation.

## Verification

Range accrual has no dedicated `tests/instruments/` directory; coverage lives in
the in-module `#[cfg(test)]` suites in `pricer.rs` and `monte_carlo.rs`, plus
the shared JSON-example and serde-contract suites
(`tests/instruments/json_examples/range_accrual.json` and
`callable_range_accrual.json`).

```bash
# In-module unit tests
cargo nextest run -p finstack-quant-valuations --lib range_accrual

# Serde and JSON-example contracts
cargo nextest run -p finstack-quant-valuations --test instruments range_accrual

# Whole workspace (never `cargo test` — it runs doctests)
mise run rust-test

# Lints
mise run rust-lint
```

## See also

- [`../../README.md`](../../README.md) — instrument module map and how to add one
- [`../tarn/`](../tarn/) and [`../snowball/`](../snowball/) — sibling exotic coupon structures
- [`INVARIANTS.md`](../../../../../../INVARIANTS.md) — Decimal/f64, determinism and serde invariants

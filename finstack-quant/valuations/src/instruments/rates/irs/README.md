# Interest Rate Swap (IRS)

Plain-vanilla fixed/float and OIS-style interest rate swaps under ISDA leg
conventions. Basis swaps live in [`../basis_swap/`](../basis_swap/) and
cross-currency swaps in [`../xccy_swap/`](../xccy_swap/).

## Public surface

Import path: `finstack_quant_valuations::instruments::rates::irs`
(`InterestRateSwap` and the leg specs are also re-exported at
`finstack_quant_valuations::instruments`).

| Item | Purpose |
|------|---------|
| `InterestRateSwap` | The instrument. `builder()`, `validate()`, `example_standard()`, `from_conventions(..)`. |
| `InterestRateSwapBuilder` | Fluent builder produced by `InterestRateSwap::builder()`. |
| `PayReceive` | `Pay` (pay fixed) or `Receive` (receive fixed). |
| `FixedLegSpec`, `FloatLegSpec` | Leg schedules, day counts, calendars, spreads, reset/payment lags. |
| `FloatingLegCompounding` | Term (`Simple`) vs compounded-in-arrears / observation-shift / rate-cutoff RFR coupons. |
| `IrsLegConventions` | Per-index conventions resolved from the `ConventionRegistry`. |
| `ConventionSwapParams` | Argument struct for `InterestRateSwap::from_conventions`. |
| `ParRateMethod` | `ForwardBased` (market standard) or `DiscountRatio` (bootstrapping). |

## Module layout

```
irs/
├── mod.rs         # re-exports + module-level pricing and convention rustdoc
├── types.rs       # InterestRateSwap, builder, IrsLegConventions, ConventionSwapParams, validate
├── pricer.rs      # leg PVs and NPV (compute_pv, compute_pv_raw)
├── cashflow.rs    # fixed and floating leg schedule generation
├── compounding.rs # FloatingLegCompounding + daily RFR compounding with lookback/shift
└── metrics/
    ├── par_rate.rs             # ParRate
    ├── annuity.rs              # Annuity
    ├── pv_fixed.rs pv_float.rs # leg PVs
    ├── dv01.rs                 # IrsDv01Calculator
    ├── ir_convexity.rs         # IrConvexity, IrCrossGamma
    └── schedule_diagnostics.rs # payment counts, first/last payment dates, first accrual factors
```

## Pricing

Leg PVs and the sign convention (`pricer.rs`):

```text
PV_fixed = N · K · Σ τᵢ · DF(Tᵢ)
PV_float = N · Σ τᵢ · Fwd(Tᵢ) · DF(Tᵢ)

PayReceive::Pay     (pay fixed)     => PV = PV_float − PV_fixed
PayReceive::Receive (receive fixed) => PV = PV_fixed − PV_float
```

Par rate is the fixed rate that zeroes the NPV:

```text
ParRate = PV_float / Annuity
```

with a near-zero-annuity guard (`ANNUITY_EPSILON`) that returns a validation
error rather than dividing. `ParRateMethod::DiscountRatio` uses
`(DF(start) − DF(end)) / annuity` instead, but only when the swap is unseasoned
(`as_of <= start`), single-curve (forward id == discount id), term-style
(`Simple`) with no spread, and has no payment delay on either leg. When any of
those preconditions fails the calculator **silently falls back** to the
forward-based par rate rather than erroring, so requesting `DiscountRatio` never
guarantees you got it.

Leg PV accumulation uses compensated summation (Kahan / Neumaier), which
matters on 30Y+ schedules with 60+ periods.

### OIS coupons

`FloatingLegCompounding::CompoundedInArrears { lookback_days }` (and the
observation-shift and rate-cutoff variants) accrue a compounded overnight
coupon:

```text
coupon = ∏(1 + rᵢ · dcfᵢ) − 1
```

over daily observations in the accrual period.

**Fast path.** When the swap is unseasoned (`as_of <= accrual_start`), has no
lookback or observation shift, and the forward curve id equals the discount
curve id, the exact identity `∏(1 + rᵢ · dcfᵢ) = DF(start) / DF(end)` is used
instead of iterating daily observations. Any lookback or shift disables the fast
path and the engine performs full daily compounding with shifted observation
dates. The crate-internal single-curve-OIS classification being true does
**not** imply the fast path applies — lookback and shift still force the long
path.

**Seasoned swaps.** When `as_of` falls inside an accrual period, historical
fixings are required for observation dates before `as_of`. Supply them as a
`ScalarTimeSeries` with id `FIXING:{forward_curve_id}`; the remaining days
project off the forward (or discount) curve. Missing fixings are an error, not
a silent projection.

RFR presets (cleared-OIS conventions — plain in-arrears, no lookback):
`FloatingLegCompounding::{sofr, fedfunds, sonia, estr, tona, saron}()`. For
FRN-style legs there are `sofr_observation_shift()` and
`sonia_observation_shift()` (no €STR/TONA/SARON shift preset), plus
`rate_cutoff(cutoff_days)`.

**A preset sets the compounding method only — never the day count.** The leg's
`day_count` must be set separately to the index's own basis: ACT/360 for SOFR,
EFFR, €STR and SARON, but **ACT/365F for SONIA and TONA**. Pairing `sonia()` or
`tona()` with an ACT/360 leg misstates every accrual by ≈365/360 (about 1.4% of
the coupon). `from_conventions` gets this right; hand-built legs must set it.

## Construction

```rust
use finstack_quant_valuations::instruments::rates::irs::{InterestRateSwap, PayReceive};
use finstack_quant_valuations::instruments::{FixedLegSpec, FloatLegSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use rust_decimal::Decimal;
use time::macros::date;

let start = date!(2024 - 01 - 02);
let end = date!(2029 - 01 - 02);

let swap = InterestRateSwap::builder()
    .id(InstrumentId::new("IRS-5Y-USD"))
    .notional(Money::new(10_000_000.0, Currency::USD)?)
    .side(PayReceive::Pay)
    .fixed(FixedLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        rate: Decimal::try_from(0.04)?,
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Thirty360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".to_string()),
        stub: StubKind::ShortFront,
        start,
        end,
        par_method: None,
        compounding_simple: true,
        payment_lag_days: 0,
        end_of_month: false,
    })
    .float(FloatLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        spread_bp: Decimal::ZERO,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".to_string()),
        stub: StubKind::ShortFront,
        reset_lag_days: 2,
        fixing_calendar_id: Some("usny".to_string()),
        start,
        end,
        compounding: Default::default(),
        payment_lag_days: 0,
        end_of_month: false,
    })
    .build()?;

swap.validate()?;
```

`InterestRateSwap::example_standard()` returns exactly this swap. To resolve
conventions from a rate index instead of spelling out both legs, use
`InterestRateSwap::from_conventions(ConventionSwapParams { .. })`, which reads
`IrsLegConventions::from_rate_index(index_id)` out of the global
`ConventionRegistry`.

`validate()` checks currency and schedule consistency, rejects `Simple`
compounding on a registered overnight-RFR index, and warns (does not fail)
when the two legs have mismatched date ranges, since that can be intentional in
bespoke structures.

## Metrics

Registered for `InstrumentType::Irs` in `metrics/mod.rs`:

| `MetricId` | Meaning |
|-----------|---------|
| `ParRate` | Fixed rate that zeroes the NPV |
| `Annuity` | Fixed-leg annuity, `Σ τᵢ DF(Tᵢ)` |
| `PvFixed`, `PvFloat` | Individual leg PVs |
| `Dv01` | Parallel curve DV01 (`IrsDv01Calculator`) |
| `Pv01` | Per-curve parallel bump, stored as `pv01::{curve}` |
| `BucketedDv01` | Triangular key-rate DV01 |
| `IrConvexity` | Second-order parallel rate sensitivity |
| `IrCrossGamma` | Mixed second derivative, discount vs forward curve |
| `FixedLegPaymentCount`, `FloatingLegPaymentCount` | Schedule diagnostics |
| `FixedFirstPaymentDate`, `FixedLastPaymentDate`, `FloatingFirstPaymentDate`, `FloatingLastPaymentDate` | Schedule diagnostics |
| `FixedFirstAccrualFactor`, `FloatingFirstAccrualFactor` | Schedule diagnostics |

`Theta` is registered universally by `metrics::standard_registry()`.
Request metrics via `Instrument::price_with_metrics(market, as_of, &[..], PricingOptions::default())`.

## Conventions

Standard fixed/float conventions by currency (from `mod.rs`):

| Currency | Float day count | Fixed day count (OIS) | Index |
|----------|-----------------|-----------------------|-------|
| USD | ACT/360 | ACT/360 | SOFR |
| EUR | ACT/360 | ACT/360 | €STR |
| GBP | ACT/365F | ACT/365F | SONIA |
| JPY | ACT/365F | ACT/365F | TONA |
| CHF | ACT/360 | ACT/360 | SARON |
| CAD | ACT/365F | ACT/365F | CORRA |
| AUD | ACT/365F | ACT/365F | AONIA / BBSW |
| NZD | ACT/365F | ACT/365F | BKBM |
| CNY | ACT/365F | ACT/365F | Shibor |

Accrual day count may legitimately differ from the discount curve's day count —
that is standard in USD swap markets, not a bug.

## Margin

`InterestRateSwap` carries an optional `margin_spec: Option<OtcMarginSpec>`
(from `finstack-quant-margin`) for CSA and cleared workflows, including
`OtcMarginSpec::cleared()`. SIMM sensitivities and VM/IM metrics live in the
margin crate.

## Bindings

Reachable from Python and WASM through the JSON envelope
(`InstrumentJson::Irs` inside `finstack_quant.instrument/1`):

- **Python**: typed leg specs in `finstack_quant.valuations.instruments`, plus
  `price_instrument(...)` and `instrument_cashflows_json(...)` in the same
  namespace.
- **WASM**: `valuations.instruments.priceInstrument` and
  `valuations.instruments.instrumentCashflowsJson`.

## Limitations

- Deterministic curves in the default pricer; no embedded stochastic short-rate
  model (that is `ModelKey::HullWhite1F` on swaptions and cap/floors).
- CMS, callable and cross-currency structures live in sibling modules.
- No embedded FVA/CVA/DVA; funding is expressed through curve choice.

## Verification

```bash
# IRS construction, cashflows, pricing, compounding, metrics and validation
cargo nextest run -p finstack-quant-valuations --test instruments irs::

# Whole workspace (never `cargo test` — it runs doctests)
mise run rust-test

# Lints
mise run rust-lint
```

Tests live in [`tests/instruments/irs/`](../../../../tests/instruments/irs/):
`construction.rs`, `cashflows.rs`, `pricing.rs`, `proptests.rs`,
`test_compounding_accuracy.rs`, `test_swap_pricing.rs`, `test_swap_symmetry.rs`,
plus `metrics/`, `integration/` and `validation/` subdirectories.

## References

- ISDA 2021 Definitions (current term-rate and compounded-RFR framework) —
  [`docs/REFERENCES.md#isda-2021-definitions`](../../../../../../docs/REFERENCES.md#isda-2021-definitions)
- ISDA 2006 Definitions (legacy transactions) —
  [`docs/REFERENCES.md#isda-2006-definitions`](../../../../../../docs/REFERENCES.md#isda-2006-definitions)
- ARRC SOFR conventions —
  [`docs/REFERENCES.md#arrc-sofr-users-guide`](../../../../../../docs/REFERENCES.md#arrc-sofr-users-guide)
- Bank of England SONIA key features —
  [`docs/REFERENCES.md#boe-sonia-key-features`](../../../../../../docs/REFERENCES.md#boe-sonia-key-features)
- Practitioner swap mechanics (Sadr) —
  [`docs/REFERENCES.md#sadr-2009-irs`](../../../../../../docs/REFERENCES.md#sadr-2009-irs)
- Bloomberg SWPM screen conventions —
  [`docs/REFERENCES.md#bloomberg-swpm`](../../../../../../docs/REFERENCES.md#bloomberg-swpm)

## See also

- [`../../README.md`](../../README.md) — instrument module map and how to add one
- [`../../../metrics/README.md`](../../../metrics/README.md) — metric ids and calculators
- [`INVARIANTS.md`](../../../../../../INVARIANTS.md) — Decimal/f64, determinism and serde invariants

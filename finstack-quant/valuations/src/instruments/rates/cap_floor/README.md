# Interest Rate Cap/Floor

## Features

- Supports caps, floors, caplets, and floorlets via `RateOptionType` with configurable schedule (freq/day-count/BDC/stub).
- Uses explicit discount, forward, and volatility curve IDs for market data alignment; settlement and exercise style fields follow standard conventions.
- Helper constructors (`new_cap`, `new_floor`, and `CapFloorParams`) simplify building standard structures.

## Methodology & References

- Black (1976) lognormal model and Bachelier normal model for caplet/floorlet pricing (`pricing/black.rs`, `pricing/normal.rs`).
- Deterministic projection of forward rates with discounting from the chosen curve; no stochastic rates beyond the supplied curves.
- Conventions follow ISDA interest-rate option market standards (Act/360, modified following, IMM-style stubs).
- Hull–White 1F pricing supports exact bond-option term caplets and a documented
  moment-matched normal approximation for compounded-in-arrears RFR coupons.
  Compounded-RFR validation uses an independent seeded pathwise HW benchmark,
  not QuantLib's generic analytic cap engine.
- Fixed-kappa normal-surface calibration is price-consistent on the contractual
  schedule. A piecewise `hw1f_sigma_schedule` may be supplied with
  `hw1f_mean_reversion`; scalar `hw1f_sigma` remains supported for compatibility.

## Usage Example

```rust
use finstack_quant_valuations::instruments::rates::cap_floor::CapFloor;
use finstack_quant_core::{currency::Currency, dates::*, money::Money, types::CurveId};
use time::Month;

let cap = CapFloor::new_cap(
    "CAP-1Y",
    Money::new(10_000_000.0, Currency::USD),
    0.035,
    Date::from_calendar_date(2024, Month::January, 3)?,
    Date::from_calendar_date(2025, Month::January, 3)?,
    Tenor::quarterly(),
    DayCount::Act360,
    CurveId::new("USD-OIS"),
    CurveId::new("USD-SOFR-3M"),
    CurveId::new("USD-CAP-VOL"),
)?;
let pv = cap.value(&market_context, Date::from_calendar_date(2024, Month::January, 3)?)?;
```

## Limitations / Known Issues

- Pricing assumes European exercise; displaced-diffusion and SABR-local-vol hybrid dynamics are not included.
- Volatility smile handled only through the supplied surface; no stochastic volatility or SABR inside the pricer.
- Does not include convexity adjustments for futures-style margined underlyings.
- Market caplet stripping remains source-dependent. A supplied normal optionlet
  surface can be fitted, but a vendor's proprietary quote-to-strip Jacobian is
  not inferred from screen values.

## Metrics

- PV plus cap/floor par strike (implied volatility to match price), delta/vega/theta via bump-and-revalue.
- DV01 on discount curve and forward-curve sensitivities (parallel/key-rate) through generic calculators.
- Bucketed caplet contributions for attribution.
- Under HW1F, `vega` bumps normal market quotes and recalibrates the model;
  `hw_sigma_vega` is the distinct direct short-rate-volatility sensitivity
  (a parallel bump across every segment for a scheduled sigma).

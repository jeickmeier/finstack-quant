# CMS Option

## Features

- Options on constant-maturity swap (CMS) rates with configurable tenor, fixing/payment dates, and accrual fractions.
- Supports fixed/floating leg conventions for the underlying swap (frequencies, fixed and optional float day-count) and optional forward curve override.
- Pricing overrides and optional CMS volatility surface for market alignment.

## Methodology & References

- Convexity-adjusted Black–76 pricer per Hagan (2003) with Hull-style adjustment; forward CMS rate computed from discount/forward curves.
- Deterministic annuity and discounting; SABR/local-vol not modeled beyond supplied surface values.
- European payoff on each fixing; summed across accrual periods.

## Usage Example

```rust
use finstack_quant_valuations::instruments::rates::cms_option::CmsOption;
use finstack_quant_core::dates::Date;
use time::Month;

let as_of = Date::from_calendar_date(2024, Month::January, 5)?;
let opt = CmsOption::example();
let pv = opt.value(&market_context, as_of)?;
```

## Limitations / Known Issues

- Relies on convexity approximation; no full LMM/SABR path modeling.
- Volatility surface optional but required for market-consistent pricing; fallback uses a flat 20% if absent.
- European style only; Bermudan CMS caps/floors are not modeled.

## Metrics

- PV plus strike/vol sensitivity (delta/vega) via bump-and-revalue; forward CMS rate and annuity reported per period for attribution.
- DV01 on discount/forward curves using generic calculators; bucketed contributions per fixing.
- Par strike solving support (implied strike for zero PV) through solver hooks.

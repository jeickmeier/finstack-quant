# Deposit

## Features

- Single-period deposit with explicit start/end, ACT/360-style accrual, and optional quoted simple rate.
- Deterministic cashflow schedule (principal out, principal + interest back) using the shared cashflow helpers.
- Discount curve driven valuation with serde-stable shape for JSON pipelines.

## Methodology & References

- Simple interest accrual on chosen day-count; PV computed via discount curve day-count to align with par-rate conventions.
- When spot lag and business-day conventions are provided, `start` is treated as trade date and accrual uses the effective start/end dates.
- No optionality or convexity; mirrors standard money-market deposit pricing.
- Integration with DV01 metrics through canonical market dependencies.

## Usage Example

```rust
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_core::dates::Date;
use time::Month;

let as_of = Date::from_calendar_date(2024, Month::January, 2)?;
let dep = Deposit::example().unwrap();
let pv = dep.value(&market_context, as_of)?;
```

## Limitations / Known Issues

- Assumes single deterministic rate; no compounding beyond simple accrual.
- Does not model credit/funding adjustments; relies solely on the provided discount curve.
- No support for callable or extendable deposits.

## Metrics

- PV and par/forward deposit rate solved from discount curve.
- DV01 on discount curve via generic bump calculators.
- Accrued interest as-of valuation date when within deposit period.

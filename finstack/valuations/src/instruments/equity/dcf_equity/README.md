# Discounted Cash Flow (DCF)

## Features

- Corporate valuation instrument with explicit projected free-cash-flow schedule and terminal value specification (Gordon Growth, Exit Multiple, or H-Model).
- Structured equity bridge (total debt, cash, preferred equity, minority interest, non-operating assets, named adjustments) or flat net-debt scalar.
- Mid-year discounting convention (standard IB/PE practice).
- Per-share equity value with diluted share count via treasury stock method.
- Private company valuation discounts (DLOM, DLOC).
- Builder pattern via `DiscountedCashFlowBuilder` with serde-stable fields for storage/pipeline use.

## Methodology & References

- Standard DCF formula: present value of projected FCFs plus discounted terminal value minus equity bridge adjustments.
- Terminal value options: Gordon Growth, Exit Multiple, and H-Model (Damodaran linear growth fade).
- Mid-year convention: discounts flows at `(t - 0.5)` instead of `t`, reflecting mid-period cash arrival.
- Year fractions use an ACT/365.25 basis; discounting always uses the deterministic WACC (the named curve is for risk attribution only).
- Valuation discounts applied multiplicatively: `FMV = Equity × (1 - DLOC) × (1 - DLOM)`.
- Dilution via treasury stock method: in-the-money securities increase diluted share count.

## Usage Example

### Basic DCF (public company intrinsic value)

```rust
use finstack_valuations::instruments::equity::dcf_equity::{DiscountedCashFlow, TerminalValueSpec};
use finstack_core::{currency::Currency, dates::Date, money::Money, types::{CurveId, InstrumentId}};
use time::Month;

let flows = vec![
    (Date::from_calendar_date(2025, Month::December, 31)?, 12.0),
    (Date::from_calendar_date(2026, Month::December, 31)?, 14.0),
];

let dcf = DiscountedCashFlow::builder()
    .id(InstrumentId::new("DCF-ACME"))
    .currency(Currency::USD)
    .flows(flows)
    .wacc(0.09)
    .terminal_value(TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
    .net_debt(50.0)
    .valuation_date(Date::from_calendar_date(2024, Month::December, 31)?)
    .discount_curve_id(CurveId::new("USD-OIS"))
    .build()?;

let equity_value = dcf.value(&market_context, dcf.valuation_date)?;
```

### Private company valuation (with mid-year, bridge, discounts)

```rust
use finstack_valuations::instruments::equity::dcf_equity::*;

let dcf = DiscountedCashFlow::builder()
    .id(InstrumentId::new("DCF-PRIVATE"))
    .currency(Currency::USD)
    .flows(projected_fcf_schedule)
    .wacc(0.12)
    .terminal_value(TerminalValueSpec::ExitMultiple {
        terminal_metric: ebitda_year_5,
        multiple: 8.0,
    })
    .net_debt(0.0) // ignored when equity_bridge is set
    .valuation_date(valuation_date)
    .discount_curve_id(CurveId::new("USD-DISCOUNT"))
    .mid_year_convention(true)
    .equity_bridge(EquityBridge {
        total_debt: 25_000_000.0,
        cash: 5_000_000.0,
        preferred_equity: 10_000_000.0,
        minority_interest: 0.0,
        non_operating_assets: 2_000_000.0,
        other_adjustments: vec![("unfunded_pension".into(), -1_500_000.0)],
    })
    .shares_outstanding(1_000_000.0)
    .valuation_discounts(ValuationDiscounts {
        dlom: Some(0.25),
        dloc: Some(0.15),
        other_discount: None,
    })
    .build()?;

let fmv = dcf.value(&market_context, valuation_date)?;
let per_share = dcf.equity_value_per_share(fmv.amount());
```

### H-Model (two-stage growth fade for public company)

```rust
let dcf = DiscountedCashFlow::builder()
    // ... standard fields ...
    .terminal_value(TerminalValueSpec::HModel {
        high_growth_rate: 0.15,
        stable_growth_rate: 0.03,
        half_life_years: 5.0,
    })
    .build()?;
```

## Terminal Value Methods

### Gordon Growth Model

```text
TV = FCF_terminal × (1 + g) / (WACC - g)
```

### Exit Multiple

```text
TV = Terminal_Metric × Multiple
(e.g., EBITDA × 10x)
```

### H-Model (Damodaran)

```text
TV = FCF_T × (1+g_s)/(WACC-g_s) + FCF_T × H × (g_h-g_s)/(WACC-g_s)
```

Where `H` is the half-life of the linear growth fade from `g_h` to `g_s`.

## Equity Bridge

When `equity_bridge` is set, it takes precedence over the flat `net_debt` field:

```text
Equity = EV - Total Debt + Cash - Preferred Equity
         - Minority Interest + Non-Operating Assets + Σ(adjustments)
```

## Metrics

- Enterprise value, equity value, terminal value PV.
- Equity price per share (diluted), diluted share count.
- DV01 (parallel and bucketed) for rate sensitivity.
- Theta (time decay via date roll).
- Duration-like sensitivity to WACC and growth via finite-difference bump scripts.

## Pricing Methodology

- Discounts explicit free cashflows and terminal value at WACC on ACT/365.25 basis; DV01 bumps the risk-free component inside the WACC.
- Mid-year convention (optional): discounts at `(t - 0.5)` years.
- Terminal value via Gordon Growth, Exit Multiple, or H-Model, discounted to valuation date.
- Equity value = Enterprise Value − Bridge Amount; then DLOC/DLOM discounts applied.
- Deterministic single-scenario valuation (compose multiple instances for scenario weighting).

## Optional Fields

All new fields are optional with serde defaults:
- `mid_year_convention` defaults to `false`.
- `equity_bridge`, `shares_outstanding`, `valuation_discounts` default to `None`.
- `dilution_securities` defaults to empty.
- Old serialized JSON (without new fields) deserializes correctly.

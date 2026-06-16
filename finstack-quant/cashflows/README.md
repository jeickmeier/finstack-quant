# finstack-quant-cashflows

Cashflow schedule construction, accrual, and period aggregation for bonds,
loans, swaps, and structured products.

## Overview

`finstack-quant-cashflows` builds dated, currency-tagged cashflow schedules and
aggregates them for reporting and PV:

- `CashFlowSchedule::builder()` for coupons, principal, fees, and credit legs
- specification types for coupons, amortization, fees, default, prepayment,
  and recovery
- schedule-driven accrued interest
- currency-preserving aggregation and period PV helpers with explicit
  `DayCountContext`

Conventions:

- amounts use `Money` with an explicit currency
- coupon rates are decimals; spreads and periodic fee quotes are often basis
  points on the spec types
- payment and reset lags are business-day based when a calendar ID is set
- day-count and calendar behavior come from the builder specs, not from examples

## Import path

```rust
use finstack_quant_cashflows::builder::CashFlowSchedule;
```

`finstack-quant-valuations` depends on this crate internally. For application code,
add `finstack-quant-cashflows` as a direct dependency rather than reaching through
valuations.

## Modules

| Module | Role |
| --- | --- |
| [`builder`](https://docs.rs/finstack-quant-cashflows/latest/finstack_quant_cashflows/builder/index.html) | `CashFlowSchedule`, specs, schedule inspection, period PV |
| [`aggregation`](https://docs.rs/finstack-quant-cashflows/latest/finstack_quant_cashflows/aggregation/index.html) | Period rollups; [`RecoveryTiming`](https://docs.rs/finstack-quant-cashflows/latest/finstack_quant_cashflows/aggregation/enum.RecoveryTiming.html) for recovery placement |
| [`accrual`](https://docs.rs/finstack-quant-cashflows/latest/finstack_quant_cashflows/accrual/index.html) | `accrued_interest_amount`, `AccrualConfig` |
| [`traits`](https://docs.rs/finstack-quant-cashflows/latest/finstack_quant_cashflows/traits/index.html) | `CashflowProvider`, `schedule_from_dated_flows`, `schedule_from_classified_flows` |
| [`primitives`](https://docs.rs/finstack-quant-cashflows/latest/finstack_quant_cashflows/primitives/index.html) | Re-exports `CashFlow` and `CFKind` from `finstack-quant-core` |
| [`json`](https://docs.rs/finstack-quant-cashflows/latest/finstack_quant_cashflows/json/index.html) | Serde-first JSON bridge for building/validating schedules (`build_cashflow_schedule_json`, `validate_cashflow_schedule_json`, `accrued_interest_json`); binding surface |

`ScheduleParams` ships ten presets: `quarterly_act360`, `semiannual_30360`,
`annual_actact`, and seven regional templates (`usd_sofr_swap`,
`usd_corporate_bond`, `usd_treasury`, `eur_estr_swap`, `eur_gov_bond`,
`gbp_sonia_swap`, `jpy_tona_swap`).

Schedule helpers include `weighted_average_life`, `coupons`,
`outstanding_path_per_flow`, `outstanding_by_date`, `merge_cashflow_schedules`,
and `normalize_public`.

## Quick start

### Fixed-rate schedule

```rust
use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType, FixedCouponSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use rust_decimal_macros::dec;
use time::Month;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let issue = Date::from_calendar_date(2025, Month::January, 15)?;
let maturity = Date::from_calendar_date(2030, Month::January, 15)?;

let fixed_spec = FixedCouponSpec {
    coupon_type: CouponType::Cash,
    rate: dec!(0.05),
    freq: Tenor::semi_annual(),
    dc: DayCount::Thirty360,
    bdc: BusinessDayConvention::Following,
    calendar_id: "weekends_only".to_string(),
    stub: StubKind::None,
    end_of_month: false,
    payment_lag_days: 0,
};

let schedule = CashFlowSchedule::builder()
    .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
    .fixed_cf(fixed_spec)
    .build_with_curves(None)?;

assert!(!schedule.flows.is_empty());
# Ok(())
# }
```

### Amortization and fees

```rust
use finstack_quant_cashflows::builder::{
    AmortizationSpec, CashFlowSchedule, CouponType, FeeBase, FeeSpec, FixedCouponSpec,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use rust_decimal_macros::dec;
use time::Month;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let issue = Date::from_calendar_date(2025, Month::January, 1)?;
let maturity = Date::from_calendar_date(2028, Month::January, 1)?;

let fee = FeeSpec::PeriodicBps {
    base: FeeBase::Drawn,
    bps: dec!(25),
    freq: Tenor::quarterly(),
    dc: DayCount::Act360,
    bdc: BusinessDayConvention::ModifiedFollowing,
    calendar_id: "weekends_only".to_string(),
    stub: StubKind::None,
    accrual_basis: Default::default(),
};

let coupon = FixedCouponSpec {
    coupon_type: CouponType::Cash,
    rate: dec!(0.06),
    freq: Tenor::quarterly(),
    dc: DayCount::Act360,
    bdc: BusinessDayConvention::ModifiedFollowing,
    calendar_id: "weekends_only".to_string(),
    stub: StubKind::None,
    end_of_month: false,
    payment_lag_days: 0,
};

let schedule = CashFlowSchedule::builder()
    .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
    .amortization(AmortizationSpec::LinearTo {
        final_notional: Money::new(0.0, Currency::USD),
    })
    .fee(fee)
    .fixed_cf(coupon)
    .build_with_curves(None)?;

let balances = schedule.outstanding_by_date()?;
assert!(!balances.is_empty());
# Ok(())
# }
```

### Floating-rate schedule

```rust
use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType, FloatingCouponSpec, FloatingRateSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use rust_decimal_macros::dec;
use time::Month;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let issue = Date::from_calendar_date(2025, Month::January, 15)?;
let maturity = Date::from_calendar_date(2027, Month::January, 15)?;

let float_spec = FloatingCouponSpec {
    coupon_type: CouponType::Cash,
    rate_spec: FloatingRateSpec {
        index_id: CurveId::new("USD-SOFR-3M"),
        spread_bp: dec!(200),
        gearing: dec!(1),
        gearing_includes_spread: true,
        index_floor_bp: Some(dec!(0)),
        all_in_floor_bp: None,
        all_in_cap_bp: None,
        index_cap_bp: None,
        reset_freq: Tenor::quarterly(),
        reset_lag_days: 2,
        dc: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: "weekends_only".to_string(),
        fixing_calendar_id: None,
        end_of_month: false,
        payment_lag_days: 0,
        overnight_compounding: None,
        overnight_basis: None,
        fallback: Default::default(),
    },
    freq: Tenor::quarterly(),
    stub: StubKind::None,
};

let schedule = CashFlowSchedule::builder()
    .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
    .floating_cf(float_spec)
    .build_with_curves(None)?;

assert!(!schedule.flows.is_empty());
# Ok(())
# }
```

### Accrued interest

```rust
use finstack_quant_cashflows::{accrued_interest_amount, AccrualConfig, AccrualMethod, ExCouponRule};

# fn demo(schedule: &finstack_quant_cashflows::builder::CashFlowSchedule, as_of: finstack_quant_core::dates::Date) -> finstack_quant_core::Result<f64> {
let config = AccrualConfig {
    method: AccrualMethod::Compounded,
    ex_coupon: Some(ExCouponRule {
        days_before_coupon: 5,
        calendar_id: Some("usny".to_string()),
    }),
    include_pik: false,
    frequency: None,
};

accrued_interest_amount(schedule, as_of, &config)
# }
```

The result is a scalar in the schedule amount space; use the schedule currency
when reporting it.

## Workflows

### Aggregate by reporting period

```rust
use finstack_quant_cashflows::aggregation::aggregate_by_period;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Period, PeriodId};
use finstack_quant_core::money::Money;
use time::Month;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let flows = vec![
    (
        Date::from_calendar_date(2025, Month::March, 15)?,
        Money::new(100_000.0, Currency::USD),
    ),
    (
        Date::from_calendar_date(2025, Month::March, 20)?,
        Money::new(50_000.0, Currency::EUR),
    ),
];

let periods = vec![Period {
    id: PeriodId::quarter(2025, 1),
    start: Date::from_calendar_date(2025, Month::January, 1)?,
    end: Date::from_calendar_date(2025, Month::April, 1)?,
    is_actual: true,
}];

let aggregated = aggregate_by_period(&flows, &periods);
assert!(aggregated.contains_key(&PeriodId::quarter(2025, 1)));
# Ok(())
# }
```

### Periodized PV

```rust,no_run
use finstack_quant_cashflows::aggregation::DateContext;
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_cashflows::builder::{PvCreditAdjustment, PvDiscountSource};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Period};
use finstack_quant_core::market_data::traits::{Discounting, Survival};

fn periodized_pv(
    schedule: &CashFlowSchedule,
    periods: &[Period],
    disc: &dyn Discounting,
    base: Date,
) -> finstack_quant_core::Result<()> {
    let pv_map = schedule.pv_by_period(
        periods,
        PvDiscountSource::Discount { disc, credit: None },
        DateContext::new(base, DayCount::Act365F, DayCountContext::default()),
    )?;
    let _ = pv_map;
    Ok(())
}

fn credit_adjusted_periodized_pv(
    schedule: &CashFlowSchedule,
    periods: &[Period],
    disc: &dyn Discounting,
    hazard: &dyn Survival,
    base: Date,
) -> finstack_quant_core::Result<()> {
    let pv_map = schedule.pv_by_period(
        periods,
        PvDiscountSource::Discount {
            disc,
            credit: Some(PvCreditAdjustment {
                hazard: Some(hazard),
                recovery_rate: Some(0.40),
            }),
        },
        DateContext::new(base, DayCount::Act365F, DayCountContext::default()),
    )?;
    let _ = pv_map;
    Ok(())
}
```

### `CashflowProvider`

```rust,no_run
use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType, FixedCouponSpec};
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use rust_decimal_macros::dec;

struct FixedBondLike {
    notional: Money,
    issue: Date,
    maturity: Date,
}

impl CashflowProvider for FixedBondLike {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        CashFlowSchedule::builder()
            .principal(self.notional, self.issue, self.maturity)
            .fixed_cf(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: dec!(0.05),
                freq: Tenor::semi_annual(),
                dc: DayCount::Thirty360,
                bdc: BusinessDayConvention::Following,
                calendar_id: "weekends_only".to_string(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
            })
            .build_with_curves(None)
    }
}
```

Implement `cashflow_schedule`; `dated_cashflows` derives holder-view `(Date, Money)`
pairs from that schedule.

### Inspect and merge schedules

```rust,no_run
use finstack_quant_cashflows::builder::{CashFlowSchedule, merge_cashflow_schedules, Notional};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};

fn inspect(schedule: &CashFlowSchedule, as_of: Date) -> finstack_quant_core::Result<()> {
    let _wal = schedule.weighted_average_life(as_of)?;
    let _coupons = schedule.coupons().count();
    let _per_flow = schedule.outstanding_path_per_flow()?;
    let _by_date = schedule.outstanding_by_date()?;
    Ok(())
}

fn compose(legs: Vec<CashFlowSchedule>) -> CashFlowSchedule {
    merge_cashflow_schedules(legs, Notional::par(0.0, Currency::USD), DayCount::Act365F)
}
```

`normalize_public` filters to future flows, omits pure PIK accretion, re-sorts,
and tags the schedule as `Projected` for downstream consumers.

## Internal emission helpers

`builder` re-exports `#[doc(hidden)]` `emit_*` helpers for tests and internal
callers. Prefer `CashFlowSchedule::builder()` and the public spec types.

## `CFKind`

`CFKind` lives in `finstack_quant_core::cashflow` and is `#[non_exhaustive]`. Match on
the core enum in downstream code; the schedule builder uses it for ordering,
accrual, and credit-adjusted PV.

## Tests

```bash
cargo test -p finstack-quant-cashflows
cargo test -p finstack-quant-cashflows --doc
RUSTDOCFLAGS='-D warnings' cargo doc -p finstack-quant-cashflows --no-deps --all-features
```

## References

- Day count and business days: `docs/REFERENCES.md#isda-2006-definitions`
- Bond accrued interest: `docs/REFERENCES.md#icma-rule-book`
- Discounting: `docs/REFERENCES.md#hull-options-futures`
- Multi-curve rates: `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`

## See also

- `finstack_quant_core::cashflow`, `finstack_quant_core::money`, `finstack_quant_core::dates`

//! Shared schedule and market fixtures for cashflow Criterion targets.
#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use finstack_quant_cashflows::builder::{
    AmortizationSpec, CashFlowMeta, CashFlowSchedule, CouponType, FeeBase, FeeSpec,
    FixedCouponSpec, FloatingCouponSpec, FloatingRateFallback, FloatingRateSpec, Notional,
    OvernightCompoundingMethod, OvernightIndexConstraintApplication, ScheduleParams,
};
use finstack_quant_cashflows::primitives::{CFKind, CashFlow};
use finstack_quant_cashflows::{schedule_from_classified_flows, ScheduleBuildOpts};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, Period, PeriodId, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use time::Month;

pub const INDEX_ID: &str = "USD-SOFR-3M";

pub fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).unwrap()
}

pub fn maturity_of(base: Date, years: i32) -> Date {
    Date::from_calendar_date(base.year() + years, base.month(), base.day()).unwrap()
}

pub fn weekends_params(frequency: Tenor) -> ScheduleParams {
    ScheduleParams {
        frequency,
        day_count: DayCount::Act365F,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
        roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
    }
}

pub fn float_params(frequency: Tenor) -> ScheduleParams {
    ScheduleParams {
        frequency,
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
        roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
    }
}

pub fn make_discount_market(base: Date) -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.951),
            (3.0, 0.865),
            (5.0, 0.790),
            (10.0, 0.640),
            (30.0, 0.375),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("USD-CREDIT")
        .base_date(base)
        .recovery_rate(0.40)
        .knots([(0.0, 0.015), (5.0, 0.015), (10.0, 0.015)])
        .build()
        .unwrap();

    MarketContext::new().insert(disc).insert(hazard)
}

pub fn make_forward_market(base: Date) -> MarketContext {
    // Curve origin sits before issue so T-2 term resets and 5-business-day
    // overnight lookback observations project instead of requiring fixings.
    let curve_base = base + time::Duration::days(-45);
    let fwd = ForwardCurve::builder(INDEX_ID, 0.25)
        .base_date(curve_base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.045), (10.0, 0.045), (40.0, 0.045)])
        .build()
        .unwrap();
    MarketContext::new().insert(fwd)
}

pub fn make_fixed_schedule(base: Date, years: i32, frequency: Tenor) -> CashFlowSchedule {
    CashFlowSchedule::builder()
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            base,
            maturity_of(base, years),
        )
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.06),
            schedule: weekends_params(frequency),
        })
        .build(None)
        .unwrap()
}

pub fn build_monthly(base: Date, years: i32) -> CashFlowSchedule {
    let maturity = maturity_of(base, years);
    CashFlowSchedule::builder()
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            base,
            maturity,
        )
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.06),
            schedule: ScheduleParams {
                frequency: Tenor::monthly(),
                day_count: DayCount::Act365F,
                business_day_convention: BusinessDayConvention::Unadjusted,
                calendar_id: "weekends_only".to_string(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
                roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
            },
        })
        .build(None)
        .unwrap()
}

pub fn build_adjusted(
    base: Date,
    years: i32,
    adjust_accrual_dates: bool,
    lag: i32,
) -> CashFlowSchedule {
    let maturity = maturity_of(base, years);
    CashFlowSchedule::builder()
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            base,
            maturity,
        )
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.06),
            schedule: ScheduleParams {
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act365F,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: "usny".to_string(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: lag,
                adjust_accrual_dates,
                roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
            },
        })
        .build(None)
        .unwrap()
}

pub fn term_float_spec(frequency: Tenor) -> FloatingCouponSpec {
    FloatingCouponSpec {
        rate_spec: FloatingRateSpec {
            index_id: INDEX_ID.into(),
            spread_bp: dec!(200.0),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
            reset_frequency: frequency,
            index_tenor: None,
            reset_lag_days: 2,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: FloatingRateFallback::Error,
        },
        coupon_type: CouponType::Cash,
        schedule: float_params(frequency),
    }
}

pub fn overnight_float_spec(
    frequency: Tenor,
    method: OvernightCompoundingMethod,
) -> FloatingCouponSpec {
    FloatingCouponSpec {
        rate_spec: FloatingRateSpec {
            index_id: INDEX_ID.into(),
            spread_bp: dec!(200.0),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
            reset_frequency: frequency,
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: Some(method),
            overnight_basis: Some(DayCount::Act360),
            fallback: FloatingRateFallback::Error,
        },
        coupon_type: CouponType::Cash,
        schedule: float_params(frequency),
    }
}

pub fn build_floating_term(base: Date, years: i32, market: &MarketContext) -> CashFlowSchedule {
    CashFlowSchedule::builder()
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            base,
            maturity_of(base, years),
        )
        .floating_cf(term_float_spec(Tenor::quarterly()))
        .build(Some(market))
        .unwrap()
}

pub fn build_overnight(
    base: Date,
    years: i32,
    method: OvernightCompoundingMethod,
    market: &MarketContext,
) -> CashFlowSchedule {
    CashFlowSchedule::builder()
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            base,
            maturity_of(base, years),
        )
        .floating_cf(overnight_float_spec(Tenor::quarterly(), method))
        .build(Some(market))
        .unwrap()
}

pub fn build_amortizing_linear(base: Date, years: i32) -> CashFlowSchedule {
    CashFlowSchedule::builder()
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            base,
            maturity_of(base, years),
        )
        .amortization(AmortizationSpec::LinearTo {
            final_notional: Money::new(0.0, Currency::USD).expect("valid money fixture"),
        })
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.06),
            schedule: weekends_params(Tenor::quarterly()),
        })
        .build(None)
        .unwrap()
}

pub fn build_fixed_with_periodic_fee(base: Date, years: i32) -> CashFlowSchedule {
    CashFlowSchedule::builder()
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            base,
            maturity_of(base, years),
        )
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.06),
            schedule: weekends_params(Tenor::quarterly()),
        })
        .fee(FeeSpec::PeriodicBp {
            base: FeeBase::Drawn,
            bp: dec!(25),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            accrual_basis: Default::default(),
        })
        .build(None)
        .unwrap()
}

pub fn make_quarterly_periods(base: Date, n_quarters: u32) -> Vec<Period> {
    let mut periods = Vec::with_capacity(n_quarters as usize);
    let mut year = base.year();
    let mut q = ((base.month() as u8 - 1) / 3) + 1;

    for _ in 0..n_quarters {
        let start_month = (q - 1) * 3 + 1;
        let end_month = q * 3;
        let end_year = if end_month == 12 { year + 1 } else { year };
        let end_m = if end_month == 12 { 1 } else { end_month + 1 };

        let start =
            Date::from_calendar_date(year, Month::try_from(start_month).unwrap(), 1).unwrap();
        let end = Date::from_calendar_date(end_year, Month::try_from(end_m).unwrap(), 1).unwrap();

        periods.push(Period {
            id: PeriodId::quarter(year, q).expect("valid period fixture"),
            start,
            end,
            is_actual: true,
        });

        q += 1;
        if q > 4 {
            q = 1;
            year += 1;
        }
    }
    periods
}

pub fn make_dated_flows(n: usize, base: Date) -> finstack_quant_cashflows::DatedFlows {
    (0..n)
        .map(|i| {
            let days = (i as i64) * 90 + 90;
            let d = base + time::Duration::days(days);
            (
                d,
                Money::new(10_000.0, Currency::USD).expect("valid money fixture"),
            )
        })
        .collect()
}

pub fn make_amortizing_schedule(base: Date, n_periods: usize) -> CashFlowSchedule {
    let per = 1_000_000.0 / n_periods as f64;
    let flows: Vec<CashFlow> = (0..n_periods)
        .map(|i| {
            let days = ((i + 1) as i64) * 90;
            CashFlow::new(
                base + time::Duration::days(days),
                None,
                Money::new(per, Currency::USD).expect("valid money fixture"),
                CFKind::Amortization,
                0.25,
                None,
            )
        })
        .collect();

    schedule_from_classified_flows(
        flows,
        DayCount::Act365F,
        ScheduleBuildOpts {
            notional_hint: Some(
                Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            ),
            meta: CashFlowMeta {
                issue_date: Some(base),
                ..Default::default()
            },
        },
    )
}

pub fn five_year_fixed_json() -> &'static str {
    r#"{
      "notional": {
        "initial": { "amount": "1000000", "currency": "USD" },
        "amort": "none"
      },
      "issue": "2025-01-15",
      "maturity": "2030-01-15",
      "coupon_program": [{
        "kind": "fixed",
        "spec": {
          "coupon_type": "cash",
          "rate": "0.06",
          "frequency": { "count": 3, "unit": "months" },
          "day_count": "act_365f",
          "business_day_convention": "modified_following",
          "calendar_id": "weekends_only",
          "stub": "none",
          "end_of_month": false,
          "payment_lag_days": 0
        }
      }]
    }"#
}

pub fn notional(k: f64) -> Notional {
    Notional::par(k, Currency::USD).expect("valid notional fixture")
}

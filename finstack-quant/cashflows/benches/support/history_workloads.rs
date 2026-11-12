//! Fixed-economic history/piece fixtures and independent arithmetic for CCM-I2.

use finstack_quant_cashflows::builder::{
    CashFlowBuilder, CashFlowSchedule, CouponType, FloatingCouponSpec, FloatingLegCompounding,
    FloatingRateFallback, FloatingRateSpec, OvernightIndexConstraintApplication, RollRule,
    ScheduleParams,
};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{ScalarTimeSeries, SeriesInterpolation};
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use rust_decimal_macros::dec;
use std::collections::{BTreeMap, BTreeSet};
use time::Month;

pub(super) const COUPONS: usize = 64;
pub(super) const TERM_AMOUNT_BOUND: f64 = 1e-7;
pub(super) const OVERNIGHT_AMOUNT_BOUND: f64 = 1e-5;
pub(super) const TERM_RATE_BOUND: f64 = 1e-11;
pub(super) const RATE_BOUND: f64 = 1e-10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Leg {
    Term,
    Overnight,
}

impl Leg {
    pub(super) fn group(self) -> &'static str {
        match self {
            Self::Term => "ccm_build_term_history",
            Self::Overnight => "ccm_build_overnight_history",
        }
    }

    pub(super) fn index(self) -> &'static str {
        match self {
            Self::Term => "CCM-TERM",
            Self::Overnight => "CCM-ON",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Case {
    pub(super) leg: Leg,
    pub(super) pieces: usize,
    pub(super) extra_history: usize,
}

impl Case {
    pub(super) fn suffix(self) -> String {
        format!("mixed_p{}_extra{}", self.pieces, self.extra_history)
    }
}

pub(super) fn cases(leg: Leg) -> [Case; 6] {
    [
        (16, 0),
        (16, 250),
        (16, 2_500),
        (16, 10_000),
        (1, 2_500),
        (64, 2_500),
    ]
    .map(|(pieces, extra_history)| Case {
        leg,
        pieces,
        extra_history,
    })
}

pub(super) fn issue() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).unwrap()
}

pub(super) fn curve_base() -> Date {
    Date::from_calendar_date(2024, Month::December, 1).unwrap()
}

pub(super) fn mixed_cutoff() -> Date {
    Date::from_calendar_date(2025, Month::July, 16).unwrap()
}

pub(super) fn historical_rate(date: Date) -> f64 {
    0.020 + f64::from(date.to_julian_day().rem_euclid(17)) * 0.0001
}

fn back_business_days(mut date: Date, count: usize) -> Date {
    for _ in 0..count {
        date -= time::Duration::days(1);
        while date.is_weekend() {
            date -= time::Duration::days(1);
        }
    }
    date
}

fn payment(mut date: Date) -> Date {
    // Every contractual anchor is the fifteenth, so following cannot cross month end.
    while date.is_weekend() {
        date += time::Duration::days(1);
    }
    date
}

/// Independent weekday carry slices, including a leading partial weekend.
fn slices(start: Date, end: Date) -> Vec<(Date, i64)> {
    let mut result = Vec::new();
    let mut cursor = start;
    while cursor < end {
        let mut observation = cursor;
        while observation.is_weekend() {
            observation -= time::Duration::days(1);
        }
        let mut next = cursor + time::Duration::days(1);
        while next.is_weekend() {
            next += time::Duration::days(1);
        }
        next = next.min(end);
        result.push((observation, (next - cursor).whole_days()));
        cursor = next;
    }
    result
}

fn observation_dates(leg: Leg) -> Vec<Date> {
    let mut dates = BTreeSet::new();
    for quarter in 0..COUPONS {
        let start = issue().add_months(quarter as i32 * 3).unwrap();
        let end = issue().add_months((quarter as i32 + 1) * 3).unwrap();
        match leg {
            Leg::Term => {
                dates.insert(back_business_days(start, 2));
            }
            Leg::Overnight => dates.extend(slices(start, end).into_iter().map(|(date, _)| date)),
        }
    }
    dates.into_iter().collect()
}

pub(super) struct Fixture {
    pub(super) case: Case,
    pub(super) as_of: Date,
    pub(super) builder: CashFlowBuilder,
    pub(super) market: MarketContext,
    pub(super) required_history: Vec<(Date, f64)>,
    pub(super) history: Vec<(Date, f64)>,
}

pub(super) fn fixture(case: Case, as_of: Date) -> Fixture {
    assert!([1, 16, 64].contains(&case.pieces));
    let all_dates = observation_dates(case.leg);
    let required_history: Vec<_> = all_dates
        .iter()
        .copied()
        .filter(|&date| date < as_of)
        .map(|date| (date, historical_rate(date)))
        .collect();
    let earliest = all_dates[0];
    let mut date = earliest;
    let mut history = Vec::with_capacity(required_history.len() + case.extra_history);
    for _ in 0..case.extra_history {
        date = back_business_days(date, 1);
        history.push((date, historical_rate(date)));
    }
    history.reverse();
    history.extend(required_history.iter().copied());
    let tenor = if case.leg == Leg::Term {
        0.25
    } else {
        1.0 / 360.0
    };
    let forward = ForwardCurve::builder(case.leg.index(), tenor)
        .base_date(curve_base())
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (40.0, 0.05)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let mut market = MarketContext::new().insert(forward);
    if !history.is_empty() {
        let series = ScalarTimeSeries::new(
            format!("FIXING:{}", case.leg.index()),
            history.clone(),
            Some(Currency::USD),
        )
        .unwrap()
        .with_interpolation(SeriesInterpolation::Linear);
        market = market.insert_series(series);
    }
    let spec = FloatingCouponSpec {
        coupon_type: CouponType::Cash,
        rate_spec: FloatingRateSpec {
            forward_curve_id: case.leg.index().into(),
            spread_bp: dec!(200),
            gearing: dec!(1),
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: if case.leg == Leg::Term { 2 } else { 0 },
            fixing_calendar_id: None,
            compounding: if case.leg == Leg::Term {
                None
            } else {
                Some(FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 })
            },
            overnight_basis: if case.leg == Leg::Term {
                None
            } else {
                Some(DayCount::Act360)
            },
            fallback: FloatingRateFallback::Error,
        },
        schedule: ScheduleParams {
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".into(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: RollRule::None,
        },
    };
    let mut builder = CashFlowSchedule::builder();
    let _ = builder.principal(
        Money::new(1_000_000.0, Currency::USD).unwrap(),
        issue(),
        issue().add_months(192).unwrap(),
    );
    for piece in 0..case.pieces {
        let start = issue()
            .add_months(192 * piece as i32 / case.pieces as i32)
            .unwrap();
        let end = issue()
            .add_months(192 * (piece as i32 + 1) / case.pieces as i32)
            .unwrap();
        let _ = builder.add_floating_window(start, end, spec.clone());
    }
    Fixture {
        case,
        as_of,
        builder,
        market,
        required_history,
        history,
    }
}

fn expected_raw(fixture: &Fixture, date: Date) -> f64 {
    if date < fixture.as_of {
        historical_rate(date)
    } else {
        let time = (date - curve_base()).whole_days() as f64 / 360.0;
        if fixture.case.leg == Leg::Term {
            0.03 + 0.0005 * time
        } else {
            // Exact average of the globally linear one-day forward curve.
            0.03 + 0.0005 * (time + 0.5 / 360.0)
        }
    }
}

#[derive(serde::Serialize)]
pub(super) struct CouponReference {
    pub(super) start: Date,
    pub(super) end: Date,
    pub(super) payment: Date,
    pub(super) reset: Date,
    pub(super) accrual_factor: f64,
    pub(super) index_rate: f64,
    pub(super) all_in_rate: f64,
    pub(super) amount: f64,
}

#[derive(serde::Serialize)]
pub(super) struct Reference {
    pub(super) coupons: Vec<CouponReference>,
    pub(super) raw_fixings: BTreeMap<Date, f64>,
}

fn close(actual: f64, expected: f64, bound: f64) {
    assert!(actual.is_finite() && expected.is_finite());
    assert!(
        (actual - expected).abs() <= bound,
        "actual {actual}, expected {expected}, bound {bound}"
    );
}

pub(super) fn check(fixture: &Fixture, schedule: &CashFlowSchedule) -> Reference {
    schedule.validate().unwrap();
    assert_eq!(schedule.get_flows().len(), COUPONS + 2);
    assert_eq!(schedule.coupons().count(), COUPONS);
    assert_eq!(schedule.get_meta().issue_date, Some(issue()));
    assert_eq!(
        schedule.get_meta().maturity,
        Some(issue().add_months(192).unwrap())
    );
    assert_eq!(
        fixture.history.len(),
        fixture.required_history.len() + fixture.case.extra_history
    );
    let mut reference = Reference {
        coupons: Vec::with_capacity(COUPONS),
        raw_fixings: BTreeMap::new(),
    };
    for (quarter, flow) in schedule.coupons().enumerate() {
        assert_eq!(flow.get_kind(), CFKind::Floating);
        assert_eq!(flow.get_amount().currency(), Currency::USD);
        let start = issue().add_months(quarter as i32 * 3).unwrap();
        let end = issue().add_months((quarter as i32 + 1) * 3).unwrap();
        let accrual_factor = (end - start).whole_days() as f64 / 360.0;
        let reset = if fixture.case.leg == Leg::Term {
            back_business_days(start, 2)
        } else {
            start
        };
        let index_rate = if fixture.case.leg == Leg::Term {
            let raw = expected_raw(fixture, reset);
            reference.raw_fixings.insert(reset, raw);
            raw
        } else {
            let mut product = 1.0;
            for (date, weight) in slices(start, end) {
                let raw = expected_raw(fixture, date);
                reference.raw_fixings.insert(date, raw);
                product *= 1.0 + raw * weight as f64 / 360.0;
            }
            (product - 1.0) / accrual_factor
        };
        let all_in_rate = index_rate + 0.02;
        let amount = 1_000_000.0 * all_in_rate * accrual_factor;
        let accrual = flow
            .get_accrual()
            .as_ref()
            .expect("explicit floating accrual");
        assert_eq!((accrual.get_start(), accrual.get_end()), (start, end));
        assert_eq!(accrual.get_day_count(), DayCount::Act360);
        assert_eq!(
            accrual.get_context().calendar_id.as_deref(),
            Some("weekends_only")
        );
        assert_eq!(
            accrual.get_context().end_is_termination_date,
            Some(quarter + 1 == COUPONS)
        );
        assert_eq!(flow.get_date(), payment(end));
        assert_eq!(flow.get_reset_date(), Some(reset));
        close(flow.get_accrual_factor(), accrual_factor, 1e-12);
        let rate_bound = if fixture.case.leg == Leg::Term {
            TERM_RATE_BOUND
        } else {
            RATE_BOUND
        };
        close(
            accrual.get_projected_index_rate().unwrap(),
            index_rate,
            rate_bound,
        );
        close(flow.get_rate().unwrap(), all_in_rate, rate_bound);
        let amount_bound = if fixture.case.leg == Leg::Term {
            TERM_AMOUNT_BOUND
        } else {
            OVERNIGHT_AMOUNT_BOUND
        };
        close(flow.get_amount().amount(), amount, amount_bound);
        reference.coupons.push(CouponReference {
            start,
            end,
            payment: payment(end),
            reset,
            accrual_factor,
            index_rate,
            all_in_rate,
            amount,
        });
    }
    let principal: Vec<_> = schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.get_kind() == CFKind::Notional)
        .collect();
    assert_eq!(principal.len(), 2);
    assert_eq!(principal[0].get_date(), issue());
    close(principal[0].get_amount().amount(), -1_000_000.0, 0.0);
    assert_eq!(
        principal[1].get_date(),
        payment(issue().add_months(192).unwrap())
    );
    close(principal[1].get_amount().amount(), 1_000_000.0, 0.0);
    assert_eq!(
        schedule.get_meta().projected_fixings.len(),
        reference.raw_fixings.len()
    );
    for (actual, (date, raw)) in schedule
        .get_meta()
        .projected_fixings
        .iter()
        .zip(&reference.raw_fixings)
    {
        assert_eq!(actual.date, *date);
        assert_eq!(
            actual.series_id,
            format!("FIXING:{}", fixture.case.leg.index())
        );
        close(actual.value.expect("successful raw fixing"), *raw, 1e-11);
    }
    if !fixture.history.is_empty() {
        let series = fixture
            .market
            .get_series(format!("FIXING:{}", fixture.case.leg.index()))
            .unwrap();
        assert_eq!(series.observations(), fixture.history);
        assert_eq!(series.currency(), Some(Currency::USD));
        assert_eq!(series.interpolation(), SeriesInterpolation::Linear);
    }
    reference
}

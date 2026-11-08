//! Test-only contractual fixtures and independent references shared with Criterion.

use finstack_quant_cashflows::accrual::{AccrualConfig, AccrualIndex, accrued_interest_amount};
use finstack_quant_cashflows::builder::{
    CashFlowSchedule, CouponType, FixedCouponSpec, RollRule, ScheduleParams,
};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use rust_decimal_macros::dec;
use time::Month;

pub(super) const SCALAR_BOUND: f64 = 1e-7;
pub(super) const TOTAL_BOUND: f64 = 1e-6;

#[derive(Clone, Copy, Debug)]
pub(super) enum QueryShape {
    Synthetic,
    Early,
    Middle,
    Late,
}

impl QueryShape {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Synthetic => "synthetic",
            Self::Early => "early",
            Self::Middle => "middle",
            Self::Late => "late",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Case {
    pub(super) periods: usize,
    pub(super) shape: QueryShape,
    pub(super) rebuild: bool,
}

pub(super) fn cases() -> Vec<Case> {
    let mut result = Vec::with_capacity(20);
    for periods in [60, 120, 240, 480] {
        for rebuild in [true, false] {
            result.push(Case {
                periods,
                shape: QueryShape::Synthetic,
                rebuild,
            });
        }
        for shape in [QueryShape::Early, QueryShape::Middle, QueryShape::Late] {
            result.push(Case {
                periods,
                shape,
                rebuild: false,
            });
        }
    }
    result
}

pub(super) struct Fixture {
    pub(super) schedule: CashFlowSchedule,
    pub(super) config: AccrualConfig,
    pub(super) dates: Vec<Date>,
    pub(super) expected: Vec<f64>,
}

pub(super) fn issue() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).expect("fixture issue")
}

/// Independent reference from contractual monthly anchors and actual elapsed days.
pub(super) fn expected_accrued(periods: usize, date: Date) -> f64 {
    let issue = issue();
    for period in 0..periods {
        let start = issue
            .add_months(period as i32)
            .expect("monthly reference start");
        let end = issue
            .add_months((period + 1) as i32)
            .expect("monthly reference end");
        if start <= date && date < end {
            return 1_000_000.0 * 0.06 * (date - start).whole_days() as f64 / 365.0;
        }
    }
    0.0
}

pub(super) fn fixture(case: Case) -> Fixture {
    assert!([60, 120, 240, 480].contains(&case.periods));
    assert!(!case.rebuild || matches!(case.shape, QueryShape::Synthetic));
    let base = issue();
    let maturity = base
        .add_months(case.periods as i32)
        .expect("fixture maturity");
    let schedule = CashFlowSchedule::builder()
        .principal(Money::from((1_000_000_i64, Currency::USD)), base, maturity)
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.06),
            schedule: ScheduleParams {
                frequency: Tenor::monthly(),
                day_count: DayCount::Act365F,
                business_day_convention: BusinessDayConvention::Unadjusted,
                calendar_id: "weekends_only".into(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
                roll_rule: RollRule::None,
            },
        })
        .build(None)
        .expect("monthly accrual fixture");
    schedule.validate().expect("valid monthly fixture");
    assert_eq!(schedule.get_flows().len(), case.periods + 2);
    assert_eq!(schedule.coupons().count(), case.periods);
    assert_eq!(schedule.get_meta().issue_date, Some(base));
    assert_eq!(schedule.get_meta().maturity, Some(maturity));
    for flow in schedule.get_flows() {
        assert_eq!(flow.amount.currency(), Currency::USD);
        assert!(flow.amount.amount().is_finite());
    }
    let principal: Vec<_> = schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.kind == CFKind::Notional)
        .collect();
    assert_eq!(principal.len(), 2);
    assert_eq!((principal[0].date, principal[1].date), (base, maturity));
    close(principal[0].amount.amount(), -1_000_000.0, 0.0);
    close(principal[1].amount.amount(), 1_000_000.0, 0.0);
    for (period, coupon) in schedule.coupons().enumerate() {
        let start = base
            .add_months(period as i32)
            .expect("reference coupon start");
        let end = base
            .add_months((period + 1) as i32)
            .expect("reference coupon end");
        let accrual = coupon.accrual.as_ref().expect("coupon metadata");
        assert_eq!(coupon.kind, CFKind::Fixed);
        assert_eq!(coupon.amount.currency(), Currency::USD);
        assert_eq!((accrual.start, accrual.end, coupon.date), (start, end, end));
        assert_eq!(accrual.day_count, DayCount::Act365F);
        close(
            coupon.accrual_factor,
            (end - start).whole_days() as f64 / 365.0,
            1e-12,
        );
        close(coupon.rate.expect("monthly fixed rate"), 0.06, 1e-14);
        close(
            coupon.amount.amount(),
            1_000_000.0 * 0.06 * (end - start).whole_days() as f64 / 365.0,
            SCALAR_BOUND,
        );
    }
    let dates: Vec<_> = (0..case.periods)
        .map(|i| match case.shape {
            QueryShape::Synthetic => base + time::Duration::days(i as i64 * 30 + 15),
            shape => {
                let span = case.periods / 4;
                let offset = match shape {
                    QueryShape::Early | QueryShape::Synthetic => 0,
                    QueryShape::Middle => case.periods / 2,
                    QueryShape::Late => 3 * case.periods / 4,
                };
                let period = offset + i * span / case.periods;
                base.add_months(period as i32)
                    .expect("contractual query anchor")
                    + time::Duration::days(15)
            }
        })
        .collect();
    let expected = dates
        .iter()
        .map(|&date| expected_accrued(case.periods, date))
        .collect();
    Fixture {
        schedule,
        config: AccrualConfig::default(),
        dates,
        expected,
    }
}

pub(super) fn close(actual: f64, expected: f64, bound: f64) {
    assert!(
        actual.is_finite() && expected.is_finite(),
        "non-finite accrued value"
    );
    assert!(
        (actual - expected).abs() <= bound,
        "actual {actual}, expected {expected}, bound {bound}"
    );
}

pub(super) fn validate(case: Case, fixture: &Fixture) {
    assert_eq!(fixture.dates.len(), case.periods);
    assert_eq!(fixture.expected.len(), case.periods);
    assert!(fixture.dates.windows(2).all(|pair| pair[0] <= pair[1]));
    let index = AccrualIndex::build(&fixture.schedule, &fixture.config).expect("accrual index");
    let mut total = 0.0;
    let mut expected_total = 0.0;
    for (&date, &expected) in fixture.dates.iter().zip(&fixture.expected) {
        let actual = if case.rebuild {
            accrued_interest_amount(&fixture.schedule, date, &fixture.config).expect("direct query")
        } else {
            index.accrued_at(date).expect("indexed query")
        };
        close(actual, expected, SCALAR_BOUND);
        total += actual;
        expected_total += expected;
    }
    close(total, expected_total, TOTAL_BOUND);
}

//! Roll-rule schedule tests: standard IMM and post-Big-Bang CDS IMM grids.
//!
//! Pins the date grids produced when `ScheduleParams::roll_rule` /
//! `BuildPeriodsParams::roll_rule` selects the core `ScheduleBuilder` IMM
//! modes:
//!
//! - `RollRule::Imm`: quarterly third Wednesdays (CME IMM futures dates)
//! - `RollRule::CdsImm`: 20th of Mar/Jun/Sep/Dec with the post-Big-Bang
//!   (2009) front accrual anchored at the roll date preceding the start
//!
//! References: ISDA CDS Standard Model conventions (Big Bang Protocol,
//! April 2009); CME IMM date rules.

use finstack_quant_cashflows::builder::periods::{build_periods, BuildPeriodsParams};
use finstack_quant_cashflows::builder::specs::{CouponType, FixedCouponSpec, RollRule};
use finstack_quant_cashflows::builder::{CashFlowSchedule, ScheduleParams};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("valid month"), day)
        .expect("valid test date")
}

fn roll_params(start: Date, end: Date, roll_rule: RollRule) -> BuildPeriodsParams<'static> {
    BuildPeriodsParams {
        start,
        end,
        frequency: Tenor::quarterly(),
        stub: StubKind::ShortBack,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: "weekends_only",
        end_of_month: false,
        day_count: DayCount::Act360,
        payment_lag_days: 0,
        reset_lag_days: None,
        adjust_accrual_dates: false,
        roll_rule,
    }
}

#[test]
fn cds_imm_roll_rule_pins_post_big_bang_grid_with_front_accrual() {
    // Start is not a roll date: the first period must accrue from the
    // PRECEDING roll date (2023-12-20), per post-Big-Bang convention.
    let periods = build_periods(roll_params(
        d(2024, 1, 10),
        d(2024, 12, 20),
        RollRule::CdsImm,
    ))
    .expect("CDS IMM schedule builds");

    let starts: Vec<Date> = periods.iter().map(|p| p.accrual_start).collect();
    let ends: Vec<Date> = periods.iter().map(|p| p.accrual_end).collect();
    assert_eq!(
        starts,
        vec![
            d(2023, 12, 20),
            d(2024, 3, 20),
            d(2024, 6, 20),
            d(2024, 9, 20)
        ],
        "front accrual must anchor at the preceding CDS roll date"
    );
    assert_eq!(
        ends,
        vec![
            d(2024, 3, 20),
            d(2024, 6, 20),
            d(2024, 9, 20),
            d(2024, 12, 20)
        ]
    );
    // All 2024 roll dates are business days: payment dates are unadjusted.
    for period in &periods {
        assert_eq!(period.payment_date, period.accrual_end);
    }
}

#[test]
fn cds_imm_roll_rule_start_on_roll_date_has_no_front_accrual() {
    let periods = build_periods(roll_params(
        d(2024, 3, 20),
        d(2024, 12, 20),
        RollRule::CdsImm,
    ))
    .expect("CDS IMM schedule builds");

    let starts: Vec<Date> = periods.iter().map(|p| p.accrual_start).collect();
    assert_eq!(
        starts,
        vec![d(2024, 3, 20), d(2024, 6, 20), d(2024, 9, 20)],
        "a start on a roll date is its own anchor (no front accrual)"
    );
    assert_eq!(
        periods.last().expect("periods").accrual_end,
        d(2024, 12, 20)
    );
}

#[test]
fn imm_roll_rule_pins_third_wednesday_grid() {
    // 2025 third Wednesdays: Mar-19, Jun-18, Sep-17, Dec-17.
    let periods = build_periods(roll_params(d(2025, 1, 15), d(2025, 12, 31), RollRule::Imm))
        .expect("IMM schedule builds");

    let starts: Vec<Date> = periods.iter().map(|p| p.accrual_start).collect();
    let ends: Vec<Date> = periods.iter().map(|p| p.accrual_end).collect();
    assert_eq!(
        starts,
        vec![
            d(2025, 1, 15),
            d(2025, 3, 19),
            d(2025, 6, 18),
            d(2025, 9, 17)
        ],
        "IMM grid starts with the contractual start then third Wednesdays"
    );
    assert_eq!(
        ends,
        vec![
            d(2025, 3, 19),
            d(2025, 6, 18),
            d(2025, 9, 17),
            d(2025, 12, 17)
        ]
    );
    for period in &periods {
        assert_eq!(period.payment_date, period.accrual_end);
    }
}

#[test]
fn roll_rule_none_preserves_plain_tenor_grid() {
    let periods = build_periods(roll_params(d(2024, 1, 10), d(2024, 7, 10), RollRule::None))
        .expect("plain schedule builds");
    let ends: Vec<Date> = periods.iter().map(|p| p.accrual_end).collect();
    assert_eq!(ends, vec![d(2024, 4, 10), d(2024, 7, 10)]);
}

#[test]
fn schedule_params_roll_rule_defaults_to_none_and_is_omitted_on_wire() {
    let json = r#"{
        "frequency": {"count": 3, "unit": "months"},
        "day_count": "act_360",
        "calendar_id": "weekends_only"
    }"#;
    let params: ScheduleParams = serde_json::from_str(json).expect("deserializes without field");
    assert_eq!(params.roll_rule, RollRule::None);

    let wire = serde_json::to_value(&params).expect("serializes");
    assert!(
        wire.get("roll_rule").is_none(),
        "RollRule::None must not appear on the wire: {wire}"
    );
}

#[test]
fn schedule_params_roll_rule_roundtrips_cds_imm() {
    let mut params = ScheduleParams::quarterly_act360();
    params.roll_rule = RollRule::CdsImm;

    let wire = serde_json::to_value(&params).expect("serializes");
    assert_eq!(wire["roll_rule"], serde_json::json!("cds_imm"));

    let back: ScheduleParams = serde_json::from_value(wire).expect("roundtrips");
    assert_eq!(back.roll_rule, RollRule::CdsImm);
}

#[test]
fn schedule_params_still_denies_unknown_fields() {
    let json = r#"{
        "frequency": {"count": 3, "unit": "months"},
        "day_count": "act_360",
        "calendar_id": "weekends_only",
        "roll_rulez": "CdsImm"
    }"#;
    assert!(serde_json::from_str::<ScheduleParams>(json).is_err());
}

#[test]
fn cds_imm_schedule_params_reach_the_cashflow_compiler() {
    // Reachability through the canonical builder: coupons must land on the
    // CDS roll grid, not the plain quarterly-from-issue grid.
    let mut schedule = ScheduleParams::quarterly_act360();
    schedule.stub = StubKind::ShortBack;
    schedule.roll_rule = RollRule::CdsImm;

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            d(2024, 3, 20),
            d(2024, 12, 20),
        )
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: Decimal::new(5, 2),
            schedule,
        });
    let flows = builder.build(None).expect("cashflow schedule builds");

    let coupon_dates: Vec<Date> = flows
        .get_flows()
        .iter()
        .filter(|flow| flow.kind == CFKind::Fixed)
        .map(|flow| flow.date)
        .collect();
    assert_eq!(
        coupon_dates,
        vec![d(2024, 6, 20), d(2024, 9, 20), d(2024, 12, 20)],
        "coupons must follow the CDS IMM roll grid"
    );
}

//! Roll-rule schedule tests for the standard IMM grid.
//!
//! Pins the date grids produced when `ScheduleParams::roll_rule` /
//! `BuildPeriodsParams::roll_rule` selects `RollRule::Imm`: the IMM roll
//! convention (third Wednesdays of Mar/Jun/Sep/Dec at the leg frequency,
//! contractual ends kept). There is no generic CDS grid; CDS products select
//! their own date regime (record CALIBRATION-FRAMEWORK-007, O7).
//!
//! Reference: CME IMM date rules.

use finstack_quant_cashflows::builder::{BuildPeriodsParams, build_periods};
use finstack_quant_cashflows::builder::{CashFlowSchedule, ScheduleParams};
use finstack_quant_cashflows::builder::{CouponType, FixedCouponSpec, RollRule};
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
fn imm_roll_rule_pins_third_wednesday_grid() {
    // 2025 third Wednesdays: Mar-19, Jun-18, Sep-17, Dec-17. A back-stub IMM
    // leg is anchored on its on-cycle start (record CORE-CONVENTIONS-008 #6).
    assert!(
        build_periods(roll_params(d(2025, 1, 15), d(2025, 12, 31), RollRule::Imm)).is_err(),
        "an off-cycle start cannot anchor the IMM grid"
    );
    let periods = build_periods(roll_params(d(2025, 3, 19), d(2025, 12, 31), RollRule::Imm))
        .expect("IMM schedule builds");

    let starts: Vec<Date> = periods.iter().map(|p| p.accrual_start).collect();
    let ends: Vec<Date> = periods.iter().map(|p| p.accrual_end).collect();
    assert_eq!(
        starts,
        vec![
            d(2025, 3, 19),
            d(2025, 6, 18),
            d(2025, 9, 17),
            d(2025, 12, 17)
        ],
        "IMM grid rolls on third Wednesdays from the on-cycle start"
    );
    assert_eq!(
        ends,
        vec![
            d(2025, 6, 18),
            d(2025, 9, 17),
            d(2025, 12, 17),
            d(2025, 12, 31)
        ]
    );
    for period in &periods {
        assert_eq!(period.payment_date, period.accrual_end);
    }
}

fn fixed_schedule(
    start: Date,
    end: Date,
    params: ScheduleParams,
    payment_window: Option<(Date, CouponType)>,
) -> CashFlowSchedule {
    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("money"),
            start,
            end,
        )
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: Decimal::new(5, 2),
            schedule: params,
        });
    if let Some((window_start, split)) = payment_window {
        let _ = builder.add_payment_window(window_start, end, split);
    }
    builder.build(None).expect("coupon schedule")
}

fn coupon_total(schedule: &CashFlowSchedule) -> f64 {
    schedule
        .get_flows()
        .iter()
        .filter(|flow| matches!(flow.get_kind(), CFKind::Fixed | CFKind::Stub))
        .map(|flow| flow.get_amount().amount())
        .sum()
}

#[test]
fn imm_coupons_accrue_through_off_grid_maturity() {
    // A back-stub IMM leg starts on the IMM lattice; its maturity may be
    // off-grid (the declared stub side).
    let mut params = ScheduleParams::quarterly_act360();
    params.roll_rule = RollRule::Imm;
    params.stub = StubKind::ShortBack;
    params.business_day_convention = BusinessDayConvention::Unadjusted;
    for (start, end) in [
        (d(2025, 3, 19), d(2025, 12, 31)),
        // A coupon horizon need not contain another futures roll at all.
        (d(2025, 9, 17), d(2025, 11, 1)),
    ] {
        let schedule = fixed_schedule(start, end, params.clone(), None);
        let expected = 1_000_000.0 * 0.05 * (end - start).whole_days() as f64 / 360.0;
        assert!((coupon_total(&schedule) - expected).abs() < 1e-8);
        let coupons: Vec<_> = schedule
            .get_flows()
            .iter()
            .filter(|flow| matches!(flow.get_kind(), CFKind::Fixed | CFKind::Stub))
            .collect();
        assert_eq!(
            coupons
                .last()
                .expect("terminal coupon")
                .get_accrual()
                .as_ref()
                .expect("accrual")
                .get_end(),
            end
        );
        if coupons.len() > 2 {
            assert!(
                coupons[1..coupons.len() - 1]
                    .iter()
                    .all(|flow| flow.get_kind() == CFKind::Fixed)
            );
        }
    }
}

#[test]
fn imm_roll_rule_honours_the_leg_frequency_and_stub() {
    // A semi-annual leg rolled on IMM dates, generated backward from its
    // on-cycle maturity: regular periods run IMM to IMM six months apart and
    // only the declared front stub opens an irregular period.
    let mut params = ScheduleParams::quarterly_act360();
    params.frequency = Tenor::semi_annual();
    params.stub = StubKind::ShortFront;
    params.roll_rule = RollRule::Imm;
    params.business_day_convention = BusinessDayConvention::Unadjusted;
    let start = d(2025, 1, 15);
    let end = d(2026, 3, 18);

    let periods = build_periods(BuildPeriodsParams::from_schedule(&params, start, end, None))
        .expect("semi-annual IMM periods");
    let bounds: Vec<(Date, Date)> = periods
        .iter()
        .map(|period| (period.accrual_start, period.accrual_end))
        .collect();
    assert_eq!(
        bounds,
        vec![
            (start, d(2025, 3, 19)),
            (d(2025, 3, 19), d(2025, 9, 17)),
            (d(2025, 9, 17), end),
        ]
    );

    let schedule = fixed_schedule(start, end, params, None);
    let kinds: Vec<CFKind> = schedule
        .get_flows()
        .iter()
        .filter(|flow| matches!(flow.get_kind(), CFKind::Fixed | CFKind::Stub))
        .map(|flow| flow.get_kind())
        .collect();
    assert_eq!(kinds, vec![CFKind::Stub, CFKind::Fixed, CFKind::Fixed]);
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
        "business_day_convention": "modified_following",
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
fn schedule_params_have_no_generic_cds_grid() {
    // Record CALIBRATION-FRAMEWORK-007 (O7): a CDS product selects its own
    // date regime; the cashflows roll rule has no CDS grid.
    let json = r#"{
        "frequency": {"count": 3, "unit": "months"},
        "day_count": "act_360",
        "business_day_convention": "modified_following",
        "calendar_id": "weekends_only",
        "roll_rule": "cds_imm"
    }"#;
    assert!(serde_json::from_str::<ScheduleParams>(json).is_err());
}

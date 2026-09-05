//! Economic regression cases from the cashflow pipeline audit.
use cf::builder::{
    CashFlowSchedule, OvernightCompoundingMethod, OvernightObservationSchedule,
    OvernightRateConstraints,
};
use cf::primitives::CFKind;
use finstack_quant_cashflows as cf;
use serde_json::{json, Value};

fn spec(issue: &str, maturity: &str) -> Value {
    json!({"notional":{"initial":{"amount":"1000000","currency":"USD"},"amort":"none"},
    "issue":issue,"maturity":maturity,"coupon_program":[{"kind":"fixed","spec":{
        "coupon_type":"cash","rate":"0.1","frequency":{"count":3,"unit":"months"},
        "day_count":"act_360","business_day_convention":"unadjusted",
        "calendar_id":"weekends_only","stub":"short_back","payment_lag_days":0}}]})
}
fn build(v: Value) -> CashFlowSchedule {
    serde_json::from_value::<cf::CashflowScheduleBuildSpec>(v)
        .expect("valid spec")
        .build(None)
        .expect("valid economics")
}
fn date(s: &str) -> cf::Date {
    serde_json::from_value::<cf::CashflowScheduleBuildSpec>(spec(s, "2030-01-01"))
        .expect("date")
        .issue
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-8,
        "actual {actual}, expected {expected}"
    );
}

#[test]
fn amortization_spans_all_step_up_segments() {
    for amort in [
        json!({"linear_to":{"final_notional":{"amount":"0","currency":"USD"}}}),
        json!({"percent_of_original_per_period":{"pct":0.25}}),
    ] {
        let mut v = spec("2025-01-01", "2026-01-01");
        v["notional"]["initial"]["amount"] = json!("1000");
        v["notional"]["amort"] = amort;
        v["coupon_program"][0]["kind"] = json!("step_up");
        let coupon = v["coupon_program"][0]["spec"]
            .as_object_mut()
            .expect("coupon");
        coupon.remove("rate");
        coupon.insert("initial_rate".into(), json!("0.1"));
        coupon.insert("step_schedule".into(), json!([["2025-07-01", "0.2"]]));
        let s = build(v);
        let repayments: Vec<_> = s
            .get_flows()
            .iter()
            .filter(|f| f.kind == CFKind::Amortization)
            .collect();
        assert_eq!(repayments.len(), 4);
        for f in repayments {
            close(f.amount.amount(), 250.0);
        }
        assert!(s.coupons().all(|f| f.amount.amount() > 0.0));
    }
}
#[test]
fn discounted_draw_roundtrips_principal_independently_of_cash() {
    let mut v = spec("2025-01-01", "2026-01-01");
    v["notional"]["initial"]["amount"] = json!("1000");
    v["coupon_program"] = json!([]);
    v["principal_events"] = json!([{"date":"2025-07-01","delta":{"amount":"100","currency":"USD"},"cash":{"amount":"98","currency":"USD"},"kind":"notional"}]);
    let s = build(v);
    let s: CashFlowSchedule =
        serde_json::from_str(&serde_json::to_string(&s).expect("encode")).expect("decode");
    let path = s.outstanding_by_date().expect("balances");
    close(path[1].1.amount(), 1100.0);
    close(path.last().expect("maturity").1.amount(), 0.0);
    s.validate().expect("valid balance conservation");
}
#[test]
fn initialization_rejects_over_repayment() {
    for day in ["2024-12-31", "2025-01-01", "2025-01-02"] {
        let mut v = spec("2025-01-01", "2026-01-01");
        v["notional"]["initial"]["amount"] = json!("100");
        v["principal_events"] =
            json!([{"date":day,"delta":{"amount":"-150","currency":"USD"},"kind":"amortization"}]);
        let spec: cf::CashflowScheduleBuildSpec = serde_json::from_value(v).expect("spec");
        assert!(spec.build(None).is_err(), "over-repayment on {day}");
    }
}
#[test]
fn short_back_icma_uses_forward_quasi_coupon() {
    let mut v = spec("2025-01-31", "2025-04-15");
    v["coupon_program"][0]["spec"]["day_count"] = json!("act_act_isma");
    let s = build(v);
    let coupon = s.coupons().next().expect("stub");
    close(coupon.accrual_factor, 74.0 / 89.0 / 4.0);
    close(coupon.amount.amount(), 100000.0 * 74.0 / 89.0 / 4.0);
}
#[test]
fn accrued_and_ex_coupon_follow_payment_date() {
    let mut v = spec("2025-01-01", "2025-07-01");
    v["coupon_program"][0]["spec"]["frequency"]["count"] = json!(6);
    v["coupon_program"][0]["spec"]["payment_lag_days"] = json!(2);
    let raw = serde_json::to_string(&build(v)).expect("serialize");
    close(
        cf::accrued_interest(&raw, "2025-07-02", None).expect("AI"),
        100000.0 * 181.0 / 360.0,
    );
    close(
        cf::accrued_interest(&raw, "2025-07-03", None).expect("AI"),
        0.0,
    );
    let ex=json!({"method":"linear","ex_coupon":{"days_before_coupon":7,"calendar_id":null},"include_pik":true,"frequency":null}).to_string();
    close(
        cf::accrued_interest(&raw, "2025-06-25", Some(&ex)).expect("cum"),
        100000.0 * 175.0 / 360.0,
    );
    assert!(cf::accrued_interest(&raw, "2025-06-26", Some(&ex)).expect("ex") < 0.0);
}
#[test]
fn bus252_accrual_retains_calendar() {
    let mut v = spec("2025-01-01", "2025-04-01");
    v["coupon_program"][0]["spec"]["day_count"] = json!("bus_252");
    let raw = serde_json::to_string(&build(v)).expect("serialize");
    // January 2025 has 23 weekdays under the explicitly weekends-only calendar.
    close(
        cf::accrued_interest(&raw, "2025-02-01", None).expect("calendar retained"),
        100000.0 * 23.0 / 252.0,
    );
}
#[test]
fn pik_funded_repayment_validates() {
    let mut v = spec("2025-01-01", "2026-01-01");
    v["notional"]["initial"]["amount"] = json!("100");
    v["coupon_program"][0]["spec"]["coupon_type"] = json!("pik");
    v["principal_events"] = json!([{"date":"2025-04-01","delta":{"amount":"-102.5","currency":"USD"},"kind":"amortization"}]);
    let s = build(v);
    s.validate().expect("PIK is repayable principal");
    close(
        s.outstanding_by_date()
            .expect("balances")
            .last()
            .expect("last")
            .1
            .amount(),
        0.0,
    );
}
#[test]
fn funding_outflow_has_no_default_recovery_payment() {
    let mut v = spec("2025-01-01", "2026-01-01");
    v["coupon_program"] = json!([]);
    v["principal_events"] =
        json!([{"date":"2025-07-01","delta":{"amount":"100","currency":"USD"},"kind":"notional"}]);
    let s = build(v);
    let draw = s
        .get_flows()
        .iter()
        .find(|f| f.date == date("2025-07-01"))
        .expect("draw");
    close(
        cf::aggregation::credit_adjusted_cashflow_pv(draw, 1.0, 0.8, Some(0.4), date("2025-01-01"))
            .expect("PV"),
        -80.0,
    );
}
#[test]
fn issue_amortization_updates_fee_history() {
    for basis in ["point_in_time", "time_weighted_average"] {
        let mut v = spec("2025-01-01", "2025-04-01");
        v["notional"]["initial"]["amount"] = json!("1000");
        v["notional"]["amort"] = json!({"custom_principal":{"items":[["2025-01-01",{"amount":"200","currency":"USD"}]]}});
        v["fees"] = json!([{"periodic_bp":{"base":"drawn","bp":"100","frequency":{"count":3,"unit":"months"},"day_count":"act_360","business_day_convention":"unadjusted","calendar_id":"weekends_only","stub":"short_back","accrual_basis":basis}}]);
        let s = build(v);
        close(
            s.get_flows()
                .iter()
                .find(|f| f.kind == CFKind::Fee)
                .expect("fee")
                .amount
                .amount(),
            2.0,
        );
    }
}
#[test]
fn overnight_replay_is_independent_of_weekend_checkpoints() {
    let cal = cf::builder::calendar::resolve_calendar_strict("weekends_only").expect("calendar");
    let on = OvernightObservationSchedule::compile(
        date("2025-01-03"),
        date("2025-01-07"),
        OvernightCompoundingMethod::CompoundedInArrears,
        cal,
    )
    .expect("schedule");
    let direct = on
        .replay(
            date("2025-01-07"),
            360.0,
            OvernightRateConstraints::default(),
            |_| Ok(0.05),
        )
        .expect("direct");
    let mut a = on
        .accumulator(360.0, OvernightRateConstraints::default())
        .expect("accumulator");
    for checkpoint in ["2025-01-04", "2025-01-05", "2025-01-06"] {
        on.advance(&mut a, date(checkpoint), |_| Ok(0.05))
            .expect("checkpoint");
    }
    let replay = on
        .advance(&mut a, date("2025-01-07"), |_| Ok(0.05))
        .expect("final");
    assert!((direct.projected_rate - replay.projected_rate).abs() < 1e-13);
}

#[test]
fn lockout_requires_a_preceding_fixing() {
    let cal = cf::builder::calendar::resolve_calendar_strict("weekends_only").expect("calendar");
    for lockout_days in [5, 6, u32::MAX] {
        assert!(OvernightObservationSchedule::compile(
            date("2025-01-06"),
            date("2025-01-13"),
            OvernightCompoundingMethod::CompoundedWithLockout { lockout_days },
            cal
        )
        .is_err());
    }
    let schedule = OvernightObservationSchedule::compile(
        date("2025-01-06"),
        date("2025-01-13"),
        OvernightCompoundingMethod::CompoundedWithLockout { lockout_days: 1 },
        cal,
    )
    .expect("one-day lockout");
    assert_eq!(
        schedule
            .observations()
            .iter()
            .map(|o| o.observation_date.day())
            .collect::<Vec<_>>(),
        vec![6, 7, 8, 9, 9]
    );
}

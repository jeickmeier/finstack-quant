//! Economic regression cases from the cashflow pipeline audit.
use cf::builder::{
    CashFlowSchedule, FloatingLegCompounding, OvernightObservationSchedule,
    OvernightRateConstraints,
};
use finstack_quant_cashflows as cf;
use finstack_quant_core::cashflow::CFKind;
use serde_json::{Value, json};

fn spec(issue: &str, maturity: &str) -> Value {
    json!({"notional":{"initial":{"amount":"1000000","currency":"USD"},"amort":"none"},
    "issue_date":issue,"maturity":maturity,"coupon_program":[{"kind":"fixed","spec":{
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
fn date(s: &str) -> finstack_quant_core::dates::Date {
    serde_json::from_value::<cf::CashflowScheduleBuildSpec>(spec(s, "2030-01-01"))
        .expect("date")
        .issue_date
}
fn accrued_json(raw: &str, as_of: &str, config: Option<&str>) -> finstack_quant_core::Result<f64> {
    let schedule: CashFlowSchedule = serde_json::from_str(raw).expect("schedule JSON");
    let config = config.map_or_else(cf::accrual::AccrualConfig::default, |json| {
        serde_json::from_str(json).expect("accrual config JSON")
    });
    cf::accrual::accrued_interest_amount(&schedule, date(as_of), &config)
        .map(|money| money.amount())
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-8,
        "actual {actual}, expected {expected}"
    );
}

#[test]
fn act365l_balance_segments_preserve_the_full_coupon_denominator() {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::{context::MarketContext, term_structures::ForwardCurve};
    for (issue, maturity, balance_change, frequency, first_days, last_days) in [
        (
            "2023-03-01",
            "2024-03-01",
            "2023-06-01",
            json!({"count":1,"unit":"years"}),
            92.0,
            274.0,
        ),
        (
            "2023-12-01",
            "2024-03-01",
            "2023-12-15",
            json!({"count":3,"unit":"months"}),
            14.0,
            77.0,
        ),
    ] {
        for floating in [false, true] {
            let mut value = spec(issue, maturity);
            value["coupon_program"][0]["spec"]["frequency"] = frequency.clone();
            value["coupon_program"][0]["spec"]["day_count"] = json!("act_365l");
            value["principal_events"] = json!([{
                "date":balance_change, "payment_date":balance_change, "kind":"amortization",
                "delta":{"amount":"-500000","currency":"USD"}
            }]);
            // Both contractual coupons use ACT/365L's 366-day denominator.
            // Express the intended 10% coupon on the curve's ACT/365F basis:
            // r_curve * days/365 = 10% * days/366.
            let curve_rate = 0.1 * 365.0 / 366.0;
            let curve = ForwardCurve::builder("TERM", if first_days == 92.0 { 1.0 } else { 0.25 })
                .day_count(DayCount::Act365F)
                .base_date(date(issue))
                .knots([(0.0, curve_rate), (2.0, curve_rate)])
                .build()
                .expect("flat term curve");
            let market = MarketContext::new().insert(curve);
            if floating {
                value["coupon_program"][0]["kind"] = json!("floating");
                value["coupon_program"][0]["spec"]
                    .as_object_mut()
                    .expect("coupon")
                    .remove("rate");
                value["coupon_program"][0]["spec"]["rate_spec"] = json!({
                    "forward_curve_id":"TERM", "spread_bp":"0", "reset_frequency":frequency,
                    "reset_lag_days":0
                });
            }
            let schedule = serde_json::from_value::<cf::CashflowScheduleBuildSpec>(value)
                .expect("spec")
                .build(Some(&market))
                .expect("coupon schedule");
            let coupons: Vec<_> = schedule.coupons().collect();
            assert_eq!(coupons.len(), 2);
            close(coupons[0].get_accrual_factor(), first_days / 366.0);
            close(coupons[1].get_accrual_factor(), last_days / 366.0);
            close(
                coupons.iter().map(|flow| flow.get_amount().amount()).sum(),
                1_000_000.0 * 0.1 * first_days / 366.0 + 500_000.0 * 0.1 * last_days / 366.0,
            );
        }
    }
}

#[test]
fn act365l_periodic_fees_use_actual_stub_boundaries_for_every_balance_segment() {
    use finstack_quant_core::dates::{DayCount, DayCountContext, Tenor};
    for (issue, maturity, change, stub, frequency, denominator) in [
        // The ICMA reference includes Feb 29, but this short coupon does not.
        (
            "2024-03-01",
            "2025-01-01",
            "2024-06-01",
            "short_front",
            Tenor::annual(),
            365.0,
        ),
        // A long first coupon begins before its adjacent ICMA reference.
        (
            "2023-12-01",
            "2025-01-01",
            "2024-01-15",
            "long_front",
            Tenor::annual(),
            365.0,
        ),
        // The first balance segment ends before the full coupon's leap day.
        (
            "2023-03-01",
            "2024-03-01",
            "2023-06-01",
            "short_front",
            Tenor::annual(),
            366.0,
        ),
        // A non-annual coupon crosses into a leap year after the first segment.
        (
            "2023-12-01",
            "2024-03-01",
            "2023-12-15",
            "short_front",
            Tenor::quarterly(),
            366.0,
        ),
    ] {
        for basis in ["point_in_time", "time_weighted_average"] {
            let mut value = spec(issue, maturity);
            value["coupon_program"] = json!([]);
            value["fees"] = json!([{"periodic_bp":{
                "base":"drawn", "bp":"100", "frequency":frequency,
                "day_count":"act_365l", "business_day_convention":"unadjusted",
                "calendar_id":"weekends_only", "stub":stub, "accrual_basis":basis
            }}]);
            value["principal_events"] = json!([{
                "date":change, "payment_date":change, "kind":"amortization",
                "delta":{"amount":"-500000","currency":"USD"}
            }]);
            let schedule = build(value);
            let fees: Vec<_> = schedule
                .get_flows()
                .iter()
                .filter(|flow| flow.get_kind() == CFKind::Fee)
                .collect();
            assert_eq!(fees.len(), if basis == "point_in_time" { 1 } else { 2 });
            let full_days = (date(maturity) - date(issue)).whole_days() as f64;
            let first_days = (date(change) - date(issue)).whole_days() as f64;
            let expected_amount = if basis == "point_in_time" {
                10_000.0 * full_days / denominator
            } else {
                10_000.0 * first_days / denominator
                    + 5_000.0 * (full_days - first_days) / denominator
            };
            close(
                fees.iter().map(|flow| flow.get_amount().amount()).sum(),
                expected_amount,
            );
            close(
                fees.iter().map(|flow| flow.get_accrual_factor()).sum(),
                full_days / denominator,
            );
            for flow in fees {
                let accrual = flow.get_accrual().as_ref().expect("fee accrual metadata");
                assert_eq!(
                    accrual.get_context().coupon_period,
                    Some((date(issue), date(maturity)))
                );
                let context = DayCountContext {
                    frequency: Some(frequency),
                    coupon_period: accrual.get_context().coupon_period,
                    ..Default::default()
                };
                close(
                    flow.get_accrual_factor(),
                    DayCount::Act365L
                        .year_fraction(accrual.get_start(), accrual.get_end(), context)
                        .expect("segment retains its actual enclosing coupon"),
                );
            }
        }
    }
}

#[test]
fn b13_adjusted_payment_does_not_delay_amortization_interest_base() {
    let mut v = spec("2025-02-15", "2026-02-15");
    v["notional"]["amort"] =
        json!({"linear_to":{"final_notional":{"amount":"0","currency":"USD"}}});
    v["coupon_program"][0]["spec"]["rate"] = json!("0.05");
    v["coupon_program"][0]["spec"]["day_count"] = json!("act_365f");
    v["coupon_program"][0]["spec"]["business_day_convention"] = json!("following");
    let schedule = build(v);
    let last_coupon = schedule.coupons().last().expect("last quarterly coupon");
    close(
        last_coupon.get_amount().amount(),
        250_000.0 * 0.05 * 92.0 / 365.0,
    );
    let replay: CashFlowSchedule =
        serde_json::from_str(&serde_json::to_string(&schedule).expect("serialize schedule"))
            .expect("restore schedule");
    let balances = replay.outstanding_by_date().expect("economic balances");
    close(
        balances
            .iter()
            .find(|(day, _)| *day == date("2025-11-15"))
            .expect("Saturday amortization boundary")
            .1
            .amount(),
        250_000.0,
    );
    assert!(
        replay
            .get_flows()
            .iter()
            .any(|flow| flow.get_kind() == CFKind::Amortization
                && flow.get_date() == date("2025-11-17")
                && flow.get_balance_date() == Some(date("2025-11-15")))
    );
}

#[test]
fn b14_midperiod_draw_accrues_from_its_event_date() {
    let mut v = spec("2025-01-01", "2025-04-01");
    v["notional"]["initial"]["amount"] = json!("0");
    v["principal_events"] = json!([{
        "date":"2025-02-01", "payment_date": "2025-02-03", "delta":{"amount":"1000000","currency":"USD"},
        "cash":{"amount":"1000000","currency":"USD"}, "kind":"notional"
    }]);
    let schedule = build(v);
    let interest: f64 = schedule
        .coupons()
        .map(|flow| flow.get_amount().amount())
        .sum();
    close(interest, 1_000_000.0 * 0.1 * 59.0 / 360.0);
    let draw = schedule
        .get_flows()
        .iter()
        .find(|flow| flow.get_amount().amount() < 0.0)
        .expect("draw cashflow");
    assert_eq!(draw.get_date(), date("2025-02-03"));
    assert_eq!(
        draw.get_balance_date().expect("principal row balance date"),
        date("2025-02-01")
    );
}

#[test]
fn b13_preceding_final_payment_includes_contractual_pik() {
    let mut v = spec("2024-01-01", "2024-03-31");
    v["coupon_program"][0]["spec"]["coupon_type"] = json!("pik");
    v["coupon_program"][0]["spec"]["business_day_convention"] = json!("preceding");
    let schedule = build(v);
    let redemption = schedule
        .get_flows()
        .iter()
        .find(|flow| flow.get_kind() == CFKind::Notional && flow.get_amount().amount() > 0.0)
        .expect("final redemption");
    assert!(redemption.get_date() < date("2024-03-31"));
    close(
        redemption.get_amount().amount(),
        1_000_000.0 * (1.0 + 0.1 * 90.0 / 360.0),
    );
}

#[test]
fn b14_overnight_principal_events_preserve_cumulative_rate_clock() {
    use finstack_quant_core::market_data::{context::MarketContext, term_structures::ForwardCurve};
    let mut v = spec("2025-01-06", "2025-01-08");
    v["coupon_program"][0]["kind"] = json!("floating");
    v["coupon_program"][0]["spec"]
        .as_object_mut()
        .expect("spec")
        .remove("rate");
    v["coupon_program"][0]["spec"]["rate_spec"] = json!({
        "forward_curve_id":"RFR", "spread_bp":"0", "reset_frequency":{"count":3,"unit":"months"},
        "reset_lag_days":0, "compounding":{"compounded_in_arrears":{"lookback_days":0}}
    });
    v["principal_events"] = json!([{
        "date":"2025-01-07", "payment_date":"2025-01-07", "kind":"amortization",
        "delta":{"amount":"-500000","currency":"USD"}
    }]);
    let curve = ForwardCurve::builder("RFR", 0.25)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .base_date(date("2025-01-06"))
        .knots([(0.0, 0.1), (1.0, 0.1)])
        .build()
        .expect("constant overnight rate");
    let schedule = serde_json::from_value::<cf::CashflowScheduleBuildSpec>(v)
        .expect("spec")
        .build(Some(&MarketContext::new().insert(curve)))
        .expect("coupon");
    // Sterling RFR working-group NCCR convention: weight each increment of
    // the original period's cumulative compounded rate by that day's balance.
    let first = 0.1 / 360.0;
    let second = first * (1.0 + first);
    let expected = 1_000_000.0 * first + 500_000.0 * second;
    let amount: f64 = schedule
        .coupons()
        .map(|flow| flow.get_amount().amount())
        .sum();
    close(amount, expected);
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
            .filter(|f| f.get_kind() == CFKind::Amortization)
            .collect();
        assert_eq!(repayments.len(), 4);
        for f in repayments {
            close(f.get_amount().amount(), 250.0);
        }
        assert!(s.coupons().all(|f| f.get_amount().amount() > 0.0));
    }
}
#[test]
fn discounted_draw_roundtrips_principal_independently_of_cash() {
    let mut v = spec("2025-01-01", "2026-01-01");
    v["notional"]["initial"]["amount"] = json!("1000");
    v["coupon_program"] = json!([]);
    v["principal_events"] = json!([{"date":"2025-07-01", "payment_date": "2025-07-01","delta":{"amount":"100","currency":"USD"},"cash":{"amount":"98","currency":"USD"},"kind":"notional"}]);
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
        v["principal_events"] = json!([{"date":day,"payment_date":day,"delta":{"amount":"-150","currency":"USD"},"kind":"amortization"}]);
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
    close(coupon.get_accrual_factor(), 74.0 / 89.0 / 4.0);
    close(coupon.get_amount().amount(), 100000.0 * 74.0 / 89.0 / 4.0);
}
#[test]
fn accrued_and_ex_coupon_follow_payment_date() {
    let mut v = spec("2025-01-01", "2025-07-01");
    v["coupon_program"][0]["spec"]["frequency"]["count"] = json!(6);
    v["coupon_program"][0]["spec"]["payment_lag_days"] = json!(2);
    let raw = serde_json::to_string(&build(v)).expect("serialize");
    close(
        accrued_json(&raw, "2025-07-02", None).expect("AI"),
        100000.0 * 181.0 / 360.0,
    );
    close(accrued_json(&raw, "2025-07-03", None).expect("AI"), 0.0);
    let ex=json!({"method":"linear","ex_coupon":{"days_before_coupon":7,"calendar_id":null},"include_pik":true,"frequency":null}).to_string();
    close(
        accrued_json(&raw, "2025-06-25", Some(&ex)).expect("cum"),
        100000.0 * 175.0 / 360.0,
    );
    assert!(accrued_json(&raw, "2025-06-26", Some(&ex)).expect("ex-date") < 0.0);
    assert!(accrued_json(&raw, "2025-06-27", Some(&ex)).expect("ex") < 0.0);
}
#[test]
fn bus252_accrual_retains_calendar() {
    let mut v = spec("2025-01-01", "2025-04-01");
    v["coupon_program"][0]["spec"]["day_count"] = json!("bus_252");
    let raw = serde_json::to_string(&build(v)).expect("serialize");
    // January 2025 has 23 weekdays under the explicitly weekends-only calendar.
    close(
        accrued_json(&raw, "2025-02-01", None).expect("calendar retained"),
        100000.0 * 23.0 / 252.0,
    );
}
#[test]
fn pik_funded_repayment_validates() {
    let mut v = spec("2025-01-01", "2026-01-01");
    v["notional"]["initial"]["amount"] = json!("100");
    v["coupon_program"][0]["spec"]["coupon_type"] = json!("pik");
    v["principal_events"] = json!([{"date":"2025-04-01", "payment_date": "2025-04-01","delta":{"amount":"-102.5","currency":"USD"},"kind":"amortization"}]);
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
    let hazard =
        finstack_quant_core::market_data::term_structures::HazardCurve::from_survival_probs(
            "test-survival",
            date("2025-01-01"),
            &[(181.0 / 365.0, 0.8), (1.0, 0.64)],
            0.4,
        )
        .expect("valid hazard curve");
    let mut v = spec("2025-01-01", "2026-01-01");
    v["coupon_program"] = json!([]);
    v["principal_events"] = json!([{"date":"2025-07-01", "payment_date": "2025-07-01","delta":{"amount":"100","currency":"USD"},"kind":"notional"}]);
    let s = build(v);
    let draw_index = s
        .get_flows()
        .iter()
        .position(|f| f.get_date() == date("2025-07-01"))
        .expect("draw");
    let as_of = date("2025-01-01");
    let flat_disc =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("FLAT")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 1.0)])
            .build()
            .expect("valid flat discount curve");
    let curves: Vec<&dyn finstack_quant_core::market_data::traits::Discounting> =
        vec![&flat_disc; s.get_flows().len()];
    let pvs = cf::aggregation::credit_adjusted_cashflow_pvs(
        s.get_flows(),
        &curves,
        Some(&hazard),
        Some(0.4),
        as_of,
    )
    .expect("PV");
    close(pvs[draw_index], -80.0);
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
                .find(|f| f.get_kind() == CFKind::Fee)
                .expect("fee")
                .get_amount()
                .amount(),
            2.0,
        );
    }
}
#[test]
fn overnight_replay_is_independent_of_weekend_checkpoints() {
    let cal = finstack_quant_core::dates::calendar_by_id_strict("weekends_only").expect("calendar");
    let on = OvernightObservationSchedule::compile(
        date("2025-01-03"),
        date("2025-01-07"),
        FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
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
    let cal = finstack_quant_core::dates::calendar_by_id_strict("weekends_only").expect("calendar");
    for lockout_days in [5, 6, u32::MAX] {
        assert!(
            OvernightObservationSchedule::compile(
                date("2025-01-06"),
                date("2025-01-13"),
                FloatingLegCompounding::CompoundedWithRateCutoff {
                    cutoff_days: lockout_days
                },
                cal
            )
            .is_err()
        );
    }
    let schedule = OvernightObservationSchedule::compile(
        date("2025-01-06"),
        date("2025-01-13"),
        FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 },
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

#[test]
fn terminal_window_payment_lag_redeems_final_pik() {
    let mut v = spec("2025-01-01", "2025-07-01");
    let mut first = v["coupon_program"][0].clone();
    first["kind"] = json!("fixed_window");
    first["start"] = json!("2025-01-01");
    first["end"] = json!("2025-04-01");
    let mut last = first.clone();
    last["start"] = json!("2025-04-01");
    last["end"] = json!("2025-07-01");
    last["spec"]["coupon_type"] = json!("pik");
    last["spec"]["payment_lag_days"] = json!(2);
    v["coupon_program"] = json!([first, last]);
    for floating in [false, true] {
        let mut v = v.clone();
        if floating {
            let leg = &mut v["coupon_program"][1];
            leg["kind"] = json!("floating_window");
            let coupon = leg["spec"].as_object_mut().expect("coupon");
            coupon.remove("rate");
            coupon.insert(
                "rate_spec".into(),
                json!({
                    "forward_curve_id":"TEST-3M", "spread_bp":"0", "gearing":"1",
                    "reset_frequency":{"count":3,"unit":"months"},
                    "reset_lag_days":0, "fallback":{"fixed_rate":"0.1"}
                }),
            );
        }
        let s = build(v);
        let redemption = s
            .get_flows()
            .iter()
            .find(|f| f.get_kind() == CFKind::Notional && f.get_amount().amount() > 0.0)
            .expect("redemption");
        assert_eq!(redemption.get_date(), date("2025-07-03"));
        close(
            redemption.get_amount().amount(),
            1_000_000.0 * (1.0 + 0.1 * 91.0 / 360.0),
        );
        close(
            s.outstanding_by_date()
                .expect("balances")
                .last()
                .expect("final")
                .1
                .amount(),
            0.0,
        );
    }
}

#[test]
fn february_rate_and_payment_boundaries_are_not_termination() {
    for payment_window in [false, true] {
        let mut v = spec("2024-08-31", "2025-08-31");
        v["coupon_program"][0]["spec"]["frequency"]["count"] = json!(6);
        v["coupon_program"][0]["spec"]["day_count"] = json!("30e_360_isda");
        v["coupon_program"][0]["spec"]["end_of_month"] = json!(true);
        if payment_window {
            v["payment_program"] =
                json!([{"kind":"window", "start":"2024-08-31", "end":"2025-02-28", "split":"pik"}]);
        } else {
            v["coupon_program"][0]["kind"] = json!("step_up");
            let coupon = v["coupon_program"][0]["spec"]
                .as_object_mut()
                .expect("coupon");
            coupon.remove("rate");
            coupon.insert("initial_rate".into(), json!("0.1"));
            coupon.insert("step_schedule".into(), json!([["2025-02-28", "0.2"]]));
        }
        let s = build(v);
        let coupon = s
            .get_flows()
            .iter()
            .find(|f| f.get_date() == date("2025-02-28"))
            .expect("February coupon");
        // A PIK coupon's value is its explicit capitalization, not cash.
        let value = if coupon.get_kind() == finstack_quant_core::cashflow::CFKind::Pik {
            coupon.get_principal_delta().expect("PIK delta").amount()
        } else {
            coupon.get_amount().amount()
        };
        close(value, 50_000.0);
        assert_eq!(
            coupon
                .get_accrual()
                .as_ref()
                .expect("metadata")
                .get_context()
                .end_is_termination_date,
            Some(false)
        );
    }
}

#[test]
fn accrued_interest_preserves_icma_long_stub_reference_on_roundtrip() {
    for (stub, issue, maturity, as_of) in [
        ("long_back", "2025-01-15", "2025-10-15", "2025-07-15"),
        ("long_front", "2025-01-15", "2025-10-15", "2025-04-15"),
    ] {
        let mut v = spec(issue, maturity);
        v["coupon_program"][0]["spec"]["frequency"]["count"] = json!(6);
        v["coupon_program"][0]["spec"]["day_count"] = json!("act_act_isma");
        v["coupon_program"][0]["spec"]["stub"] = json!(stub);
        let raw = serde_json::to_string(&build(v)).expect("serialize");
        let cfg = json!({"method":"linear", "ex_coupon":null, "include_pik":true, "frequency":{"count":6,"unit":"months"}}).to_string();
        let expected = if stub == "long_back" {
            50_000.0
        } else {
            100_000.0 * 90.0 / 182.0 / 2.0
        };
        close(
            accrued_json(&raw, as_of, Some(&cfg)).expect("accrued"),
            expected,
        );
    }
}

#[test]
fn accrued_interest_preserves_february_termination_on_roundtrip() {
    let mut v = spec("2024-08-31", "2025-02-28");
    v["coupon_program"][0]["spec"]["frequency"]["count"] = json!(6);
    v["coupon_program"][0]["spec"]["day_count"] = json!("30e_360_isda");
    v["coupon_program"][0]["spec"]["payment_lag_days"] = json!(2);
    let raw = serde_json::to_string(&build(v)).expect("serialize");
    close(
        accrued_json(&raw, "2025-01-31", None).expect("accrued"),
        100_000.0 * 150.0 / 360.0,
    );
    close(
        accrued_json(&raw, "2025-02-28", None).expect("unpaid coupon"),
        100_000.0 * 178.0 / 360.0,
    );
    close(
        accrued_json(&raw, "2025-03-04", None).expect("paid coupon"),
        0.0,
    );
}

#[test]
fn abs_to_smm_follows_the_absolute_prepayment_ramp() {
    close(cf::builder::abs_to_smm(0.015, 1).expect("month 1"), 0.015);
    close(
        cf::builder::abs_to_smm(0.015, 11).expect("month 11"),
        0.015 / (1.0 - 0.015 * 10.0),
    );
}

mod slice1_regressions {
    use finstack_quant_cashflows::builder::{
        CashFlowSchedule, CouponType, FeeSpec, FixedCouponSpec, ScheduleParams,
    };
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
    use finstack_quant_core::money::Money;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use time::Month;

    fn schedule_params(day_count: DayCount, frequency: Tenor) -> ScheduleParams {
        let mut params = ScheduleParams::semiannual_30360();
        params.frequency = frequency;
        params.day_count = day_count;
        params.business_day_convention = BusinessDayConvention::Unadjusted;
        params.calendar_id = "weekends_only".into();
        params.stub = StubKind::None;
        params
    }

    #[test]
    fn mid_period_balance_split_preserves_contractual_accrual_sum_across_day_counts() {
        let cases = [
            (
                DayCount::OneOne,
                Date::from_calendar_date(2025, Month::January, 1).unwrap(),
                Date::from_calendar_date(2025, Month::February, 15).unwrap(),
                Date::from_calendar_date(2025, Month::July, 1).unwrap(),
                Tenor::semi_annual(),
                1.0_f64,
            ),
            (
                DayCount::Thirty360,
                Date::from_calendar_date(2025, Month::January, 31).unwrap(),
                Date::from_calendar_date(2025, Month::February, 15).unwrap(),
                Date::from_calendar_date(2025, Month::February, 28).unwrap(),
                Tenor::monthly(),
                28.0 / 360.0,
            ),
            (
                DayCount::ThirtyE360Isda,
                Date::from_calendar_date(2025, Month::January, 31).unwrap(),
                Date::from_calendar_date(2025, Month::February, 15).unwrap(),
                Date::from_calendar_date(2025, Month::February, 28).unwrap(),
                Tenor::monthly(),
                28.0 / 360.0,
            ),
            (
                DayCount::Act365L,
                Date::from_calendar_date(2024, Month::January, 15).unwrap(),
                Date::from_calendar_date(2024, Month::February, 10).unwrap(),
                Date::from_calendar_date(2024, Month::April, 15).unwrap(),
                Tenor::quarterly(),
                91.0 / 366.0,
            ),
            (
                DayCount::ActActIsma,
                Date::from_calendar_date(2025, Month::January, 15).unwrap(),
                Date::from_calendar_date(2025, Month::March, 1).unwrap(),
                Date::from_calendar_date(2025, Month::July, 15).unwrap(),
                Tenor::semi_annual(),
                0.5_f64,
            ),
            (
                DayCount::Bus252,
                Date::from_calendar_date(2025, Month::January, 2).unwrap(),
                Date::from_calendar_date(2025, Month::February, 12).unwrap(),
                Date::from_calendar_date(2025, Month::April, 2).unwrap(),
                Tenor::quarterly(),
                64.0 / 252.0,
            ),
        ];

        for (day_count, issue, mid_draw, maturity, frequency, expected_full_yf) in cases {
            let mut params = schedule_params(day_count, frequency);
            if matches!(day_count, DayCount::Thirty360 | DayCount::ThirtyE360Isda) {
                params.stub = StubKind::ShortFront;
            }
            let bullet = CashFlowSchedule::builder()
                .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
                .fixed_cf(FixedCouponSpec {
                    coupon_type: CouponType::Cash,
                    rate: dec!(0.06),
                    schedule: params.clone(),
                })
                .build(None)
                .unwrap();
            let full_yf: f64 = bullet
                .get_flows()
                .iter()
                .filter(|f| matches!(f.get_kind(), CFKind::Fixed | CFKind::Stub))
                .map(|f| f.get_accrual_factor())
                .sum();
            assert!(
                (full_yf - expected_full_yf).abs() < 1e-12,
                "{day_count:?} bullet yf {full_yf} vs {expected_full_yf}"
            );

            let split = CashFlowSchedule::builder()
                .principal(Money::from((500_000_i64, Currency::USD)), issue, maturity)
                .add_principal_event(
                    mid_draw,
                    mid_draw,
                    Money::from((500_000_i64, Currency::USD)),
                    None,
                    CFKind::Notional,
                )
                .fixed_cf(FixedCouponSpec {
                    coupon_type: CouponType::Cash,
                    rate: dec!(0.06),
                    schedule: params,
                })
                .build(None)
                .unwrap();

            let split_yf: f64 = split
                .get_flows()
                .iter()
                .filter(|f| matches!(f.get_kind(), CFKind::Fixed | CFKind::Stub))
                .map(|f| f.get_accrual_factor())
                .sum();
            assert!(
                (split_yf - full_yf).abs() < 1e-12,
                "{day_count:?} subinterval sum {split_yf} != full {full_yf}"
            );
        }
    }

    #[test]
    fn split_coupon_conserves_exact_decimal_total_and_rejects_inexact_weights() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let maturity = Date::from_calendar_date(2025, Month::July, 15).unwrap();
        let schedule_base = schedule_params(DayCount::Act360, Tenor::semi_annual());

        let near_one = CouponType::Split {
            cash_fraction: dec!(0.5),
            pik_fraction: dec!(0.4999999995),
        };
        assert!(
            CashFlowSchedule::builder()
                .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
                .fixed_cf(FixedCouponSpec {
                    coupon_type: near_one,
                    rate: dec!(0.05),
                    schedule: schedule_base.clone(),
                })
                .build(None)
                .is_err()
        );

        let notional = Money::from_decimal(dec!(123456.789), Currency::USD);
        let rate = dec!(0.07375);

        let all_cash = CashFlowSchedule::builder()
            .principal(notional, issue, maturity)
            .fixed_cf(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate,
                schedule: schedule_base.clone(),
            })
            .build(None)
            .unwrap();
        let split_sched = CashFlowSchedule::builder()
            .principal(notional, issue, maturity)
            .fixed_cf(FixedCouponSpec {
                coupon_type: CouponType::Split {
                    cash_fraction: dec!(0.333),
                    pik_fraction: dec!(0.667),
                },
                rate,
                schedule: schedule_base,
            })
            .build(None)
            .unwrap();

        let total_cash_only = all_cash
            .get_flows()
            .iter()
            .find(|f| f.get_kind() == CFKind::Fixed)
            .unwrap()
            .get_amount()
            .amount_decimal();
        let cash_part = split_sched
            .get_flows()
            .iter()
            .find(|f| f.get_kind() == CFKind::Fixed)
            .unwrap()
            .get_amount()
            .amount_decimal();
        let pik_part = split_sched
            .get_flows()
            .iter()
            .find(|f| f.get_kind() == CFKind::Pik)
            .unwrap()
            .get_principal_delta()
            .unwrap()
            .amount_decimal();
        assert_eq!(cash_part + pik_part, total_cash_only);
        assert_ne!(cash_part, Decimal::ZERO);
        assert_ne!(pik_part, Decimal::ZERO);
    }

    #[test]
    fn payment_split_program_inherits_base_pik_duplicate_windows_error_and_fees_preserve_post_maturity_and_zero()
     {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let step1 = Date::from_calendar_date(2025, Month::July, 15).unwrap();
        let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
        let exit_fee_date = Date::from_calendar_date(2026, Month::January, 30).unwrap();

        let sched = CashFlowSchedule::builder()
            .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
            .fixed_cf(FixedCouponSpec {
                coupon_type: CouponType::Pik,
                rate: dec!(0.08),
                schedule: ScheduleParams::semiannual_30360(),
            })
            .payment_split_program(&[(step1, CouponType::Cash)])
            .fee(FeeSpec::Fixed {
                date: step1,
                amount: Money::zero(Currency::USD),
            })
            .fee(FeeSpec::Fixed {
                date: exit_fee_date,
                amount: Money::from((2_500_i64, Currency::USD)),
            })
            .build(None)
            .unwrap();

        assert!(
            sched
                .get_flows()
                .iter()
                .any(|f| f.get_date() == step1 && f.get_kind() == CFKind::Fixed),
            "first period should use program Cash split"
        );
        assert!(
            sched
                .get_flows()
                .iter()
                .any(|f| f.get_date() == maturity && f.get_kind() == CFKind::Pik),
            "trailing uncovered period should inherit base CouponType::Pik"
        );
        assert!(
            sched.get_flows().iter().any(|f| {
                f.get_date() == step1
                    && f.get_kind() == CFKind::Fee
                    && f.get_amount().amount() == 0.0
            }),
            "zero-amount fixed fee should be preserved"
        );
        assert!(
            sched.get_flows().iter().any(|f| {
                f.get_date() == exit_fee_date
                    && f.get_kind() == CFKind::Fee
                    && f.get_amount().amount() == 2_500.0
            }),
            "post-maturity fixed fee should be permitted"
        );

        let dup = CashFlowSchedule::builder()
            .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
            .fixed_cf(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: dec!(0.08),
                schedule: ScheduleParams::semiannual_30360(),
            })
            .add_payment_window(issue, step1, CouponType::Cash)
            .add_payment_window(issue, step1, CouponType::Pik)
            .build(None);
        assert!(
            dup.is_err(),
            "duplicate identical payment windows must fail"
        );
    }
}

mod slice2_regressions {
    use finstack_quant_cashflows::builder::{
        AmortizationSpec, CashFlowSchedule, CouponType, FixedCouponSpec, Notional, ScheduleParams,
    };
    use finstack_quant_core::cashflow::{CFKind, CashFlowParams};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
    use finstack_quant_core::money::Money;
    use rust_decimal_macros::dec;
    use time::Month;

    #[test]
    fn daily_unadjusted_coupons_across_weekend_preserve_all_periods() {
        let friday = Date::from_calendar_date(2025, Month::January, 3).unwrap();
        let monday = Date::from_calendar_date(2025, Month::January, 6).unwrap();
        let mut sched_params = ScheduleParams::semiannual_30360();
        sched_params.frequency = Tenor::daily();
        sched_params.day_count = DayCount::Act360;
        sched_params.business_day_convention = BusinessDayConvention::Following;
        sched_params.calendar_id = "weekends_only".into();
        sched_params.stub = StubKind::None;

        let sched = CashFlowSchedule::builder()
            .principal(Money::from((1_000_000_i64, Currency::USD)), friday, monday)
            .fixed_cf(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: dec!(0.036),
                schedule: sched_params,
            })
            .build(None)
            .unwrap();

        let coupon_rows: Vec<_> = sched
            .get_flows()
            .iter()
            .filter(|f| f.get_kind() == CFKind::Fixed)
            .collect();
        assert_eq!(coupon_rows.len(), 3);
        assert!(coupon_rows.iter().all(|f| f.get_date() == monday));
    }

    #[test]
    fn linear_to_protects_final_notional_floor_and_rejects_excess_or_breach() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let q1 = Date::from_calendar_date(2025, Month::April, 15).unwrap();
        let prepay = Date::from_calendar_date(2025, Month::June, 1).unwrap();
        let q2 = Date::from_calendar_date(2025, Month::July, 15).unwrap();
        let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

        let mut sched_params = ScheduleParams::semiannual_30360();
        sched_params.frequency = Tenor::quarterly();

        let invalid = CashFlowSchedule::builder()
            .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
            .amortization(AmortizationSpec::LinearTo {
                final_notional: Money::from((1_200_000_i64, Currency::USD)),
            })
            .fixed_cf(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: dec!(0.05),
                schedule: sched_params.clone(),
            })
            .build(None);
        assert!(invalid.is_err());

        // Mid-life prepayment leaves 450k before Q2 vs target 400k: Q2 amort is capped at 50k.
        let floored = CashFlowSchedule::builder()
            .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
            .amortization(AmortizationSpec::LinearTo {
                final_notional: Money::from((400_000_i64, Currency::USD)),
            })
            .add_principal_event(
                prepay,
                prepay,
                Money::from((-400_000_i64, Currency::USD)),
                None,
                CFKind::Amortization,
            )
            .fixed_cf(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: dec!(0.05),
                schedule: sched_params.clone(),
            })
            .build(None)
            .unwrap();

        let q2_amort: f64 = floored
            .get_flows()
            .iter()
            .filter(|f| f.get_date() == q2 && f.get_kind() == CFKind::Amortization)
            .map(|f| f.get_amount().amount())
            .sum();
        assert!((q2_amort - 50_000.0).abs() < 1e-9);
        assert!(floored.get_flows().iter().any(|f| f.get_date() == maturity
            && f.get_kind() == CFKind::Notional
            && (f.get_amount().amount() - 400_000.0).abs() < 1e-9));

        let breached = CashFlowSchedule::builder()
            .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
            .amortization(AmortizationSpec::LinearTo {
                final_notional: Money::from((400_000_i64, Currency::USD)),
            })
            .add_principal_event(
                q1,
                q1,
                Money::from((-700_000_i64, Currency::USD)),
                None,
                CFKind::Amortization,
            )
            .fixed_cf(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: dec!(0.05),
                schedule: sched_params,
            })
            .build(None);
        assert!(breached.is_err());
    }

    #[test]
    fn linear_between_recalibrates_after_prepayment_and_pik() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let q1 = Date::from_calendar_date(2025, Month::April, 15).unwrap();
        let mid_prepay = Date::from_calendar_date(2025, Month::June, 1).unwrap();
        let q2 = Date::from_calendar_date(2025, Month::July, 15).unwrap();
        let q3 = Date::from_calendar_date(2025, Month::October, 15).unwrap();
        let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

        let mut sched_params = ScheduleParams::semiannual_30360();
        sched_params.frequency = Tenor::quarterly();

        let sched = CashFlowSchedule::builder()
            .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
            .amortization(AmortizationSpec::LinearBetween {
                start: issue,
                end: maturity,
            })
            .add_principal_event(
                mid_prepay,
                mid_prepay,
                Money::from((-300_000_i64, Currency::USD)),
                None,
                CFKind::Amortization,
            )
            .fixed_cf(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: dec!(0.04),
                schedule: sched_params,
            })
            .build(None)
            .unwrap();

        let amort_on = |date: Date| -> f64 {
            sched
                .get_flows()
                .iter()
                .filter(|f| f.get_date() == date && f.get_kind() == CFKind::Amortization)
                .map(|f| f.get_amount().amount())
                .sum()
        };
        assert!((amort_on(q1) - 250_000.0).abs() < 1e-6);
        // After Q1 (250k) and mid_prepay (300k), 450k remains over 3 boundaries [q2, q3, maturity].
        assert!((amort_on(q2) - 150_000.0).abs() < 1e-6);
        assert!((amort_on(q3) - 150_000.0).abs() < 1e-6);
        assert!((amort_on(maturity) - 150_000.0).abs() < 1e-6);
    }

    #[test]
    fn same_day_repay_and_draw_rollover_nets_atomically_regardless_of_order() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let roll_date = Date::from_calendar_date(2025, Month::April, 15).unwrap();
        let maturity = Date::from_calendar_date(2025, Month::July, 15).unwrap();

        let sched = CashFlowSchedule::builder()
            .principal(Money::from((100_000_i64, Currency::USD)), issue, maturity)
            // Repay 150k first on roll_date, draw 200k second on same roll_date -> net +50k.
            .add_principal_event(
                roll_date,
                roll_date,
                Money::from((-150_000_i64, Currency::USD)),
                None,
                CFKind::Amortization,
            )
            .add_principal_event(
                roll_date,
                roll_date,
                Money::from((200_000_i64, Currency::USD)),
                None,
                CFKind::Notional,
            )
            .fixed_cf(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: dec!(0.05),
                schedule: ScheduleParams::semiannual_30360(),
            })
            .build(None)
            .unwrap();

        let balances = sched.outstanding_by_date().unwrap();
        let after_roll = balances
            .iter()
            .find(|(d, _)| *d == roll_date)
            .unwrap()
            .1
            .amount();
        assert!((after_roll - 150_000.0).abs() < 1e-9);
    }

    #[test]
    fn replay_and_balances_reject_negative_end_of_day_principal() {
        let overpay_date = Date::from_calendar_date(2025, Month::April, 15).unwrap();

        let flow = CashFlowParams::new(
            overpay_date,
            None,
            Money::from((150_000_i64, Currency::USD)),
            CFKind::Amortization,
            0.0,
            None,
        )
        .with_principal_delta(Money::from((-150_000_i64, Currency::USD)))
        .with_principal_date(overpay_date)
        .build()
        .unwrap();

        let sched = CashFlowSchedule::from_flows(
            vec![flow],
            Notional::par(100_000.0, Currency::USD).unwrap(),
            DayCount::Act360,
            Default::default(),
        );

        assert!(sched.outstanding_by_date().is_err());
        assert!(sched.balances_by_flow().is_err());
    }
}

mod slice3_regressions {
    use super::cf;
    use cf::builder::{
        CashFlowMeta, CashFlowSchedule, CouponType, FloatingCouponSpec, FloatingLegCompounding,
        FloatingRateFallback, FloatingRateSpec, Notional, RateSpec, ScheduleParams,
    };
    use cf::fixings::{ProjectedFixing, materialize_fixings};
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use rust_decimal_macros::dec;
    use serde_json::json;
    use time::Month;

    fn d(y: i32, m: Month, day: u8) -> Date {
        Date::from_calendar_date(y, m, day).expect("valid test date")
    }

    fn base_term_spec(
        index_tenor: Option<Tenor>,
        compounding: Option<FloatingLegCompounding>,
        fallback: FloatingRateFallback,
    ) -> FloatingRateSpec {
        FloatingRateSpec {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            spread_bp: dec!(100),
            gearing: dec!(1),
            gearing_includes_spread: true,
            index_floor_bp: None,
            index_cap_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_frequency: Tenor::quarterly(),
            index_tenor,
            reset_lag_days: 2,
            fixing_calendar_id: Some("weekends_only".into()),
            compounding,
            overnight_basis: None,
            fallback,
        }
    }

    #[test]
    fn non_overnight_index_tenor_mismatch_fails_instead_of_warning() {
        let issue = d(2025, Month::January, 15);
        let maturity = d(2025, Month::July, 15);
        let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(d(2025, Month::January, 10))
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.04), (2.0, 0.04)])
            .build()
            .unwrap();
        let market = MarketContext::new().insert(fwd);

        let mut mismatch = base_term_spec(
            Some(Tenor::semi_annual()),
            Some(FloatingLegCompounding::Simple),
            FloatingRateFallback::Error,
        );
        let mut b = CashFlowSchedule::builder();
        let _ = b
            .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
            .floating_cf(FloatingCouponSpec {
                rate_spec: mismatch.clone(),
                coupon_type: CouponType::Cash,
                schedule: ScheduleParams::quarterly_act360(),
            });
        let err = b
            .build(Some(&market))
            .expect_err("index_tenor mismatch >10% must fail loudly");
        assert!(err.to_string().contains("USD-SOFR-3M"));
        assert!(err.to_string().contains("disagrees"));

        mismatch.index_tenor = Some(Tenor::quarterly());
        let mut ok_builder = CashFlowSchedule::builder();
        let _ = ok_builder
            .principal(Money::from((1_000_000_i64, Currency::USD)), issue, maturity)
            .floating_cf(FloatingCouponSpec {
                rate_spec: mismatch,
                coupon_type: CouponType::Cash,
                schedule: ScheduleParams::quarterly_act360(),
            });
        assert!(ok_builder.build(Some(&market)).is_ok());
    }

    #[test]
    fn spread_only_fallback_does_not_mask_supplied_curve_or_series_errors() {
        // Case 1: Curve is present and covers all observations (`>= curve.base_date()`),
        // but 30/360 forward clock on 2025-03-30 -> 2025-03-31 has 0 curve year fraction.
        // Fallback must NOT mask that curve-projection failure.
        let base = d(2025, Month::March, 28);
        let end = d(2025, Month::April, 2);
        let fwd_30360 = ForwardCurve::builder("USD-SOFR-3M", 1.0 / 360.0)
            .base_date(base)
            .day_count(DayCount::Thirty360)
            .knots([(0.0, 0.04), (1.0, 0.04)])
            .build()
            .unwrap();
        let market = MarketContext::new().insert(fwd_30360);
        let mut schedule_params = ScheduleParams::quarterly_act360();
        schedule_params.stub = finstack_quant_core::dates::StubKind::ShortBack;
        let on_spec = base_term_spec(
            None,
            Some(FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 }),
            FloatingRateFallback::SpreadOnly,
        );
        let mut b1 = CashFlowSchedule::builder();
        let _ = b1
            .principal(Money::from((1_000_000_i64, Currency::USD)), base, end)
            .floating_cf(FloatingCouponSpec {
                rate_spec: on_spec,
                coupon_type: CouponType::Cash,
                schedule: schedule_params,
            });
        assert!(
            b1.build(Some(&market)).is_err(),
            "computational curve projection error must not be hidden by SpreadOnly"
        );

        // Case 2: Curve omitted (`None`), fixing series supplied with a gap before `as_of`:
        // `SpreadOnly` must not mask missing observations in a supplied fixing series.
        let issue = d(2025, Month::January, 15);
        let maturity = d(2025, Month::July, 15);
        let as_of = d(2025, Month::June, 1);
        let gap_fixings = ScalarTimeSeries::new(
            "FIXING:USD-SOFR-3M",
            vec![(d(2025, Month::January, 13), 0.04)],
            Some(Currency::USD),
        )
        .unwrap();
        let fixings_only = MarketContext::new().insert_series(gap_fixings);
        let spec_json = json!({
            "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
            "issue_date": "2025-01-15",
            "maturity": "2025-07-15",
            "as_of": "2025-06-01",
            "coupon_program": [{
                "kind": "floating",
                "spec": FloatingCouponSpec {
                    rate_spec: base_term_spec(None, None, FloatingRateFallback::SpreadOnly),
                    coupon_type: CouponType::Cash,
                    schedule: ScheduleParams::quarterly_act360(),
                }
            }]
        });
        let spec: cf::CashflowScheduleBuildSpec = serde_json::from_value(spec_json).unwrap();
        let err = spec
            .build_as_of(Some(&fixings_only), Some(as_of))
            .expect_err("incomplete supplied fixing series must fail under SpreadOnly");
        assert!(err.to_string().contains("2025-04-11"));
        assert!(issue < maturity);
    }

    #[test]
    fn rate_spec_aligns_simple_tenor_and_missing_market_fallback() {
        let base = d(2025, Month::January, 2);
        let start = d(2025, Month::April, 4);
        let end = d(2025, Month::July, 4);
        let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(base)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.03), (1.0, 0.05)])
            .build()
            .unwrap();
        let market = MarketContext::new().insert(fwd);

        // `Simple` compounding matches `None`.
        let r_none = RateSpec::Floating(base_term_spec(
            Some(Tenor::quarterly()),
            None,
            FloatingRateFallback::Error,
        ))
        .try_rate_for_period(start, end, 91.0 / 360.0, base, &market)
        .unwrap();
        let r_simple = RateSpec::Floating(base_term_spec(
            Some(Tenor::quarterly()),
            Some(FloatingLegCompounding::Simple),
            FloatingRateFallback::Error,
        ))
        .try_rate_for_period(start, end, 91.0 / 360.0, base, &market)
        .unwrap();
        assert!((r_none - r_simple).abs() < 1e-12);

        // Mismatched `index_tenor` fails.
        assert!(
            RateSpec::Floating(base_term_spec(
                Some(Tenor::semi_annual()),
                Some(FloatingLegCompounding::Simple),
                FloatingRateFallback::Error,
            ))
            .try_rate_for_period(start, end, 91.0 / 360.0, base, &market)
            .is_err()
        );

        // Absent curve or absent fixing series honors fallback (`FixedRate(0.045)` + 100bp spread = 0.055).
        let empty = MarketContext::new();
        let r_missing_curve = RateSpec::Floating(base_term_spec(
            None,
            Some(FloatingLegCompounding::Simple),
            FloatingRateFallback::FixedRate(dec!(0.045)),
        ))
        .try_rate_for_period(start, end, 91.0 / 360.0, base, &empty)
        .unwrap();
        assert!((r_missing_curve - 0.055).abs() < 1e-12);

        let r_missing_series = RateSpec::Floating(base_term_spec(
            None,
            None,
            FloatingRateFallback::FixedRate(dec!(0.045)),
        ))
        .try_rate_for_period(start, end, 91.0 / 360.0, start, &market)
        .unwrap();
        assert!((r_missing_series - 0.055).abs() < 1e-12);
    }

    #[test]
    fn materialize_fixings_rejects_tiny_projection_conflicts() {
        let s = |v: f64| {
            CashFlowSchedule::from_flows(
                vec![],
                Notional::par(1.0, Currency::USD).unwrap(),
                DayCount::Act360,
                CashFlowMeta {
                    projected_fixings: vec![ProjectedFixing {
                        series_id: "FIXING:USD-SOFR".into(),
                        date: d(2025, Month::January, 3),
                        value: Some(v),
                    }],
                    ..Default::default()
                },
            )
        };
        let s1 = s(0.03);
        let s2 = s(0.03 + 1e-13);
        let err = materialize_fixings(
            &MarketContext::new(),
            [&s1, &s2],
            d(2025, Month::January, 2),
            d(2025, Month::January, 6),
        )
        .expect_err("1e-13 disagreement must fail without epsilon tolerance");
        assert!(err.to_string().contains("conflicting pre-roll projections"));
    }

    #[test]
    fn json_schedule_honors_spec_and_override_as_of_cutoff() {
        let curve_base = d(2025, Month::April, 15);
        let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(curve_base)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.05), (2.0, 0.05)])
            .build()
            .unwrap();
        let fixings = ScalarTimeSeries::new(
            "FIXING:USD-SOFR-3M",
            vec![(d(2025, Month::April, 11), 0.032)],
            Some(Currency::USD),
        )
        .unwrap();
        let market_json =
            serde_json::to_string(&MarketContext::new().insert(fwd).insert_series(fixings))
                .unwrap();

        let float_spec = |as_of_date: &str, issue_str: &str, mat_str: &str| {
            json!({
                "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
                "issue_date": issue_str,
                "maturity": mat_str,
                "as_of": as_of_date,
                "coupon_program": [{
                    "kind": "floating",
                    "spec": {
                        "coupon_type": "cash",
                        "rate_spec": {
                            "forward_curve_id": "USD-SOFR-3M",
                            "spread_bp": "0",
                            "reset_frequency": {"count": 3, "unit": "months"},
                            "fixing_calendar_id": "weekends_only"
                        },
                        "frequency": {"count": 3, "unit": "months"},
                        "day_count": "act_360",
                        "business_day_convention": "unadjusted",
                        "calendar_id": "weekends_only",
                        "stub": "none",
                        "end_of_month": false,
                        "payment_lag_days": 0
                    }
                }]
            })
        };

        // Jan-15..Apr-15 reset is 2025-01-13 (< as_of 2025-04-15), missing from fixings -> fails.
        let spec = float_spec("2025-04-15", "2025-01-15", "2025-07-15");
        let err = cf::build_cashflow_schedule_json(&spec.to_string(), Some(&market_json), None)
            .expect_err("spec.as_of requires Jan-13 historical fixing");
        assert!(err.to_string().contains("2025-01-13"));

        // Overriding `as_of` on a schedule whose only past reset IS supplied (Apr-15..Jul-15):
        let apr_spec = float_spec("2025-01-01", "2025-04-15", "2025-10-15");
        // Explicit `as_of = 2025-05-01` overrides `spec.as_of = 2025-01-01` and selects the 0.032 fixing!
        let sched_json = cf::build_cashflow_schedule_json(
            &apr_spec.to_string(),
            Some(&market_json),
            Some(d(2025, Month::May, 1)),
        )
        .unwrap();
        let sched: CashFlowSchedule = serde_json::from_str(&sched_json).unwrap();
        let first_float = sched
            .get_flows()
            .iter()
            .find(|cf_row| cf_row.get_kind() == CFKind::Floating)
            .unwrap();
        assert!((first_float.get_rate().unwrap() - 0.032).abs() < 1e-12);

        // `as_of` with `market = None` on floating schedule fails; on fixed schedule succeeds.
        assert!(
            cf::build_cashflow_schedule_json(
                &apr_spec.to_string(),
                None,
                Some(d(2025, Month::May, 1)),
            )
            .is_err()
        );
        assert!(
            cf::build_cashflow_schedule_json(
                &super::spec("2025-01-15", "2025-07-15").to_string(),
                None,
                Some(d(2025, Month::May, 1)),
            )
            .is_ok()
        );
    }

    #[test]
    fn cashflow_schedule_normalization_rejects_pre_as_of_overpay_but_allows_fx_and_inflation() {
        use cf::{CashflowProvider, CashflowScheduleSource};
        use finstack_quant_core::cashflow::CashFlowParams;
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::{Date, DayCount};
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::money::Money;

        struct StaticSource(CashFlowSchedule);
        impl CashflowScheduleSource for StaticSource {
            fn raw_cashflow_schedule(
                &self,
                _curves: &MarketContext,
                _as_of: Date,
            ) -> finstack_quant_core::Result<CashFlowSchedule> {
                Ok(self.0.clone())
            }
        }

        let d1 = d(2025, Month::February, 1);
        let d2 = d(2025, Month::April, 1);
        let as_of = d(2025, Month::May, 1);
        let d3 = d(2026, Month::January, 15);
        let ctx = MarketContext::default();

        // 1. Pre-as_of intermediate over-repayment in initial currency (even if followed by a draw) rejects.
        let overpay = CashFlowParams::new(
            d1,
            None,
            Money::from((150_i64, Currency::USD)),
            CFKind::Amortization,
            0.0,
            None,
        )
        .with_principal_delta(Money::from((-150_i64, Currency::USD)))
        .with_principal_date(d1)
        .build()
        .unwrap();
        let subsequent_draw = CashFlowParams::new(
            d2,
            None,
            Money::from((-100_i64, Currency::USD)),
            CFKind::Notional,
            0.0,
            None,
        )
        .with_principal_delta(Money::from((100_i64, Currency::USD)))
        .with_principal_date(d2)
        .build()
        .unwrap();
        let overpay_src = StaticSource(CashFlowSchedule::from_flows(
            vec![overpay, subsequent_draw],
            Notional::par(100.0, Currency::USD).unwrap(),
            DayCount::Act360,
            CashFlowMeta::default(),
        ));
        assert!(overpay_src.cashflow_schedule(&ctx, as_of).is_err());

        // 2. Multi-currency FX schedule (EUR initial notional + USD CFKind::Notional principal_delta) succeeds.
        let fx_eur = CashFlowParams::new(
            d3,
            None,
            Money::from((100_i64, Currency::EUR)),
            CFKind::Notional,
            0.0,
            None,
        )
        .with_principal_delta(Money::from((-100_i64, Currency::EUR)))
        .with_principal_date(d3)
        .build()
        .unwrap();
        let fx_usd = CashFlowParams::new(
            d3,
            None,
            Money::from((-110_i64, Currency::USD)),
            CFKind::Notional,
            0.0,
            None,
        )
        .with_principal_delta(Money::from((-110_i64, Currency::USD)))
        .with_principal_date(d3)
        .build()
        .unwrap();
        let fx_src = StaticSource(CashFlowSchedule::from_flows(
            vec![fx_eur, fx_usd],
            Notional::par(100.0, Currency::EUR).unwrap(),
            DayCount::Act360,
            CashFlowMeta::default(),
        ));
        assert_eq!(
            fx_src
                .cashflow_schedule(&ctx, as_of)
                .unwrap()
                .get_flows()
                .len(),
            2
        );

        // 3. Inflation-indexed redemption exceeding real initial par succeeds on CashflowProvider.
        let tips_redemption = CashFlowParams::new(
            d3,
            None,
            Money::from((125_i64, Currency::USD)),
            CFKind::Notional,
            0.0,
            None,
        )
        .with_principal_delta(Money::from((-125_i64, Currency::USD)))
        .with_principal_date(d3)
        .build()
        .unwrap();
        let tips_src = StaticSource(CashFlowSchedule::from_flows(
            vec![tips_redemption],
            Notional::par(100.0, Currency::USD).unwrap(),
            DayCount::Act360,
            CashFlowMeta::default(),
        ));
        assert_eq!(
            tips_src.cashflow_schedule(&ctx, as_of).unwrap().get_flows()[0]
                .get_amount()
                .amount(),
            125.0
        );
    }
}

mod slice4_regressions {
    use super::close;

    #[test]
    fn month_zero_rejected_across_credit_rate_specs() {
        use finstack_quant_cashflows::builder::{
            DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec, abs_to_smm, psa_cpr,
        };

        assert!(psa_cpr(1.0, 0).is_err());
        assert!(abs_to_smm(0.015, 0).is_err());
        assert!(PrepaymentModelSpec::constant_cpr(0.1).smm(0).is_err());
        assert!(DefaultModelSpec::constant_cdr(0.1).mdr(0).is_err());
        assert!(
            DefaultModelSpec::constant_cdr(0.1)
                .mdr_with_survival(0, 1.0)
                .is_err()
        );
        assert!(
            RecoveryModelSpec::with_lag(0.4, 12)
                .recovery_rate(0)
                .is_err()
        );
    }

    #[test]
    fn psa_high_multiplier_caps_at_unit_cpr() {
        use finstack_quant_cashflows::builder::{PrepaymentModelSpec, psa_cpr};

        assert_eq!(psa_cpr(20.0, 30).unwrap(), 1.0);
        let spec = PrepaymentModelSpec::psa(20.0);
        spec.validate().unwrap();
        assert_eq!(spec.smm(30).unwrap(), 1.0);
    }

    #[test]
    fn mdr_with_survival_rejects_exhausted_pool_and_handles_zero_default() {
        use finstack_quant_cashflows::builder::DefaultModelSpec;

        let spec = DefaultModelSpec::cumulative_loss(vec![1.0, 2.0], 0.5);
        // Month 1 has cumulative loss 1% / severity 0.5 = 0.02 defaulted fraction.
        assert!(spec.mdr_with_survival(1, 0.0).is_err());
        assert!(spec.mdr_with_survival(1, 0.01).is_err());
        // Beyond the curve plateau (month 3), incremental default is 0.0 -> Ok(0.0) even at zero survival.
        assert_eq!(spec.mdr_with_survival(3, 0.0).unwrap(), 0.0);
        // Intrinsic survival at month 2: cum_def(1) = 0.02, cum_def(2) = 0.04 => 0.02 / (1.0 - 0.02).
        close(spec.mdr(2).unwrap(), 0.02 / 0.98);
    }

    #[test]
    fn recovery_rate_validates_spec_and_evaluates_severity_vector() {
        use finstack_quant_cashflows::builder::RecoveryModelSpec;

        let invalid = RecoveryModelSpec::with_lag(-0.1, 12);
        assert!(invalid.recovery_rate(1).is_err());

        let vectored = RecoveryModelSpec::with_lag(0.40, 12).with_severity_vector(vec![0.7, 0.6]);
        close(vectored.recovery_rate(1).unwrap(), 0.3);
        close(vectored.recovery_rate(2).unwrap(), 0.4);
    }
}

mod slice5_regressions {
    use super::close;
    use finstack_quant_cashflows::accrual::{
        AccrualConfig, AccrualIndex, ExCouponRule, accrued_interest_amount,
    };
    use finstack_quant_cashflows::builder::{
        CashFlowMeta, CashFlowSchedule, Notional, merge_cashflow_schedules,
    };
    use finstack_quant_core::Error;
    use finstack_quant_core::cashflow::{CFKind, CashFlowAccrual, CashFlowParams};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, DayCountContextState};
    use finstack_quant_core::money::Money;
    use time::Month;

    fn d(y: i32, m: Month, day: u8) -> Date {
        Date::from_calendar_date(y, m, day).expect("valid date")
    }

    #[test]
    fn accrual_config_defaults_to_excluding_pik_flows() {
        let cfg = AccrualConfig::default();
        assert!(!cfg.include_pik);

        let issue = d(2025, Month::January, 1);
        let maturity = d(2026, Month::January, 1);
        let pik = CashFlowParams::new(
            maturity,
            None,
            Money::from((0_i64, Currency::USD)),
            CFKind::Pik,
            1.0,
            Some(0.06),
        )
        .with_principal_delta(Money::from((60_i64, Currency::USD)))
        .with_principal_date(maturity)
        .with_accrual(
            CashFlowAccrual::new(
                issue,
                maturity,
                DayCount::Act365F,
                None,
                DayCountContextState::default(),
            )
            .expect("accrual"),
        )
        .build()
        .expect("pik flow");

        let schedule = CashFlowSchedule::from_flows(
            vec![pik],
            Notional::par(1_000.0, Currency::USD).expect("notional"),
            DayCount::Act365F,
            CashFlowMeta {
                issue_date: Some(issue),
                ..Default::default()
            },
        );
        let mid = d(2025, Month::July, 2);
        let default_ai = accrued_interest_amount(&schedule, mid, &cfg).expect("default AI");
        assert_eq!(default_ai.currency(), Currency::USD);
        assert_eq!(default_ai.amount(), 0.0);

        let with_pik = accrued_interest_amount(
            &schedule,
            mid,
            &AccrualConfig {
                include_pik: true,
                ..cfg
            },
        )
        .expect("pik AI");
        close(with_pik.amount(), 60.0 * 182.0 / 365.0);
    }

    #[test]
    fn ex_coupon_rule_treats_ex_date_as_inclusive() {
        let payment = d(2025, Month::July, 15);
        let rule = ExCouponRule {
            days_before_coupon: 7,
            calendar_id: None,
        };
        let ex_date = rule.ex_date(payment).expect("ex_date");
        assert_eq!(ex_date, d(2025, Month::July, 8));
        assert!(!rule.is_ex_coupon(payment, d(2025, Month::July, 7)).unwrap());
        assert!(rule.is_ex_coupon(payment, ex_date).unwrap());
        assert!(!rule.is_ex_coupon(payment, payment).unwrap());
    }

    #[test]
    fn accrual_index_derives_coupon_currency_and_rejects_mixed_selected_coupons() {
        let issue = d(2025, Month::January, 1);
        let payment = d(2025, Month::July, 1);
        let eur_coupon = CashFlowParams::new(
            payment,
            None,
            Money::from((180_i64, Currency::EUR)),
            CFKind::Fixed,
            0.5,
            Some(0.04),
        )
        .with_accrual(
            CashFlowAccrual::new(
                issue,
                payment,
                DayCount::Thirty360,
                None,
                DayCountContextState::default(),
            )
            .expect("accrual"),
        )
        .build()
        .expect("eur flow");

        let eur_sched = CashFlowSchedule::from_flows(
            vec![eur_coupon],
            Notional::par(10_000.0, Currency::EUR).expect("eur notional"),
            DayCount::Thirty360,
            CashFlowMeta {
                issue_date: Some(issue),
                ..Default::default()
            },
        );
        // Merged schedule with USD notional header but single-currency EUR coupons.
        let merged = merge_cashflow_schedules(
            [eur_sched],
            Notional::par(10_000.0, Currency::USD).expect("usd notional header"),
            DayCount::Thirty360,
        );
        let cfg = AccrualConfig::default();
        let idx = AccrualIndex::build(&merged, &cfg).expect("EUR coupon schedule builds");
        assert_eq!(idx.get_currency(), Currency::EUR);
        let mid_ai = idx.accrued_at(d(2025, Month::April, 1)).expect("EUR AI");
        assert_eq!(mid_ai.currency(), Currency::EUR);
        close(mid_ai.amount(), 90.0);

        // Adding a mixed-currency selected coupon fails closed with CurrencyMismatch.
        let usd_coupon = CashFlowParams::new(
            payment,
            None,
            Money::from((50_i64, Currency::USD)),
            CFKind::Fixed,
            0.5,
            Some(0.02),
        )
        .build()
        .expect("usd flow");
        let mixed = CashFlowSchedule::from_flows(
            vec![merged.get_flows()[0].clone(), usd_coupon],
            Notional::par(10_000.0, Currency::EUR).expect("notional"),
            DayCount::Thirty360,
            CashFlowMeta {
                issue_date: Some(issue),
                ..Default::default()
            },
        );
        assert!(matches!(
            AccrualIndex::build(&mixed, &cfg),
            Err(Error::CurrencyMismatch { .. })
        ));
    }

    #[test]
    fn one_one_day_count_prorates_mid_period_accrued_interest() {
        let start = d(2024, Month::January, 1);
        let end = d(2024, Month::July, 1); // 182 days (leap year Feb=29)
        let mid = d(2024, Month::April, 1); // 91 days elapsed
        let coupon = CashFlowParams::new(
            end,
            None,
            Money::from((100_i64, Currency::USD)),
            CFKind::Fixed,
            1.0,
            Some(0.10),
        )
        .with_accrual(
            CashFlowAccrual::new(
                start,
                end,
                DayCount::OneOne,
                None,
                DayCountContextState::default(),
            )
            .expect("one/one accrual"),
        )
        .build()
        .expect("flow");
        let schedule = CashFlowSchedule::from_flows(
            vec![coupon],
            Notional::par(1_000.0, Currency::USD).expect("notional"),
            DayCount::OneOne,
            CashFlowMeta {
                issue_date: Some(start),
                ..Default::default()
            },
        );
        let ai = accrued_interest_amount(&schedule, mid, &AccrualConfig::default())
            .expect("OneOne mid-period AI");
        close(ai.amount(), 50.0);
    }
}

mod slice6_regressions {
    use super::date;
    use finstack_quant_cashflows as cf;
    use finstack_quant_core::cashflow::{CFKind, CashFlowParams};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, DayCountContext, Period, PeriodId};
    use finstack_quant_core::market_data::term_structures::HazardCurve;
    use finstack_quant_core::market_data::traits::Discounting;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use rust_decimal_macros::dec;

    #[test]
    fn nominal_aggregation_preserves_exact_decimal_cents() {
        let mut flows = Vec::with_capacity(2_000);
        for _ in 0..1_000 {
            flows.push((
                date("2025-03-15"),
                Money::from_decimal(dec!(0.10), Currency::USD),
            ));
            flows.push((
                date("2025-06-15"),
                Money::from_decimal(dec!(0.20), Currency::USD),
            ));
        }

        let checked_total = cf::aggregation::aggregate_cashflows_checked(&flows, Currency::USD)
            .expect("checked total");
        assert_eq!(checked_total.amount_decimal(), dec!(300.00));

        let periods = vec![Period {
            id: PeriodId::annual(2025).expect("valid period"),
            start: date("2025-01-01"),
            end: date("2026-01-01"),
            is_actual: true,
        }];
        let by_period =
            cf::aggregation::aggregate_by_period(&flows, &periods).expect("period rollup");
        assert_eq!(
            by_period[&periods[0].id][&Currency::USD].amount_decimal(),
            dec!(300.00)
        );
    }

    #[test]
    fn ten_year_default_time_recovery_matches_continuous_integral() {
        struct FlatRateDiscount {
            base: finstack_quant_core::dates::Date,
            rate: f64,
        }

        impl Discounting for FlatRateDiscount {
            fn id(&self) -> &CurveId {
                static ID: std::sync::LazyLock<CurveId> =
                    std::sync::LazyLock::new(|| "flat-5pct".into());
                &ID
            }

            fn base_date(&self) -> finstack_quant_core::dates::Date {
                self.base
            }

            fn df(&self, t: f64) -> f64 {
                (-self.rate * t).exp()
            }
        }

        let as_of = date("2025-01-01");
        let maturity = date("2035-01-01");
        let r = 0.05_f64;
        let lambda = 0.03_f64;
        let recovery_rate = 0.40_f64;
        let notional = 1_000_000.0_f64;

        let disc = FlatRateDiscount {
            base: as_of,
            rate: r,
        };
        let hazard = HazardCurve::flat("flat-3pct", as_of, lambda, recovery_rate)
            .expect("valid hazard curve");

        let repayment = CashFlowParams::new(
            maturity,
            None,
            Money::from_decimal(dec!(1_000_000), Currency::USD),
            CFKind::Notional,
            0.0,
            None,
        )
        .with_principal_delta(Money::from_decimal(dec!(-1_000_000), Currency::USD))
        .with_principal_date(maturity)
        .build()
        .expect("repayment flow");

        let flows = [repayment];
        let curves: [&dyn Discounting; 1] = [&disc];
        let pvs = cf::aggregation::credit_adjusted_cashflow_pvs(
            &flows,
            &curves,
            Some(&hazard),
            Some(recovery_rate),
            as_of,
        )
        .expect("credit-adjusted PVs");

        let t_end = DayCount::Act365F
            .year_fraction(as_of, maturity, DayCountContext::default())
            .expect("valid Act365F fraction");
        let df_end = disc.df(t_end);
        let sp_end = hazard.sp_on_date(maturity).expect("valid survival");
        let settlement_pv = notional * df_end * sp_end;
        let actual_recovery_pv = pvs[0] - settlement_pv;

        let analytic_recovery_pv = recovery_rate
            * notional
            * (lambda / (r + lambda))
            * (1.0 - (-(r + lambda) * t_end).exp());
        let terminal_recovery_pv = recovery_rate * notional * df_end * (1.0 - sp_end);

        assert!(
            (actual_recovery_pv - analytic_recovery_pv).abs() < 1.0,
            "actual recovery {actual_recovery_pv} differed from analytic {analytic_recovery_pv}"
        );
        assert!(
            actual_recovery_pv - terminal_recovery_pv > 19_000.0,
            "expected >$19k improvement over terminal discounting: actual={actual_recovery_pv}, terminal={terminal_recovery_pv}"
        );
    }
}

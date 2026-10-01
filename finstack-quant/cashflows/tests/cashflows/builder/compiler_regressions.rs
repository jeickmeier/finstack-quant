//! Economic regressions for compiled coupon and principal programs.

use finstack_quant_cashflows::{
    builder::CashFlowSchedule, primitives::CFKind, CashflowScheduleBuildSpec,
};
use finstack_quant_core::{
    dates::{parse_iso_date, DayCount},
    market_data::{context::MarketContext, term_structures::ForwardCurve},
};
use serde_json::{json, Value};

fn date(value: &str) -> finstack_quant_core::dates::Date {
    parse_iso_date(value).expect("valid date")
}

fn spec(issue: &str, maturity: &str) -> Value {
    json!({
        "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
        "issue_date": issue,
        "maturity": maturity,
        "coupon_program": [{"kind": "fixed", "spec": {
            "coupon_type": "cash", "rate": "0.04",
            "frequency": {"count": 3, "unit": "months"},
            "day_count": "act_360", "business_day_convention": "unadjusted",
            "calendar_id": "weekends_only", "stub": "short_back", "payment_lag_days": 0
        }}]
    })
}

fn build(value: Value, market: Option<&MarketContext>) -> CashFlowSchedule {
    serde_json::from_value::<CashflowScheduleBuildSpec>(value)
        .expect("valid build spec")
        .build(market)
        .expect("valid schedule")
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-8,
        "actual {actual}, expected {expected}"
    );
}

#[test]
fn principal_draw_overflow_returns_an_error_at_issue_and_later() {
    for event_date in ["2025-01-01", "2025-04-01"] {
        let mut value = spec("2025-01-01", "2026-01-01");
        value["notional"]["initial"]["amount"] = json!("50000000000000000000000000000");
        value["coupon_program"] = json!([]);
        value["principal_events"] = json!([{
            "date": event_date, "payment_date": event_date, "kind": "notional",
            "delta": {"amount": "50000000000000000000000000000", "currency": "USD"}
        }]);
        let error = serde_json::from_value::<CashflowScheduleBuildSpec>(value)
            .expect("individually representable principal inputs")
            .build(None)
            .expect_err("sum exceeds the decimal balance range");
        assert!(error.to_string().contains("outstanding principal exceeds"));
    }
}

#[test]
fn pik_balance_overflow_returns_an_error() {
    let mut value = spec("2025-01-01", "2026-01-01");
    value["notional"]["initial"]["amount"] = json!("70000000000000000000000000000");
    value["coupon_program"][0]["spec"]["coupon_type"] = json!("pik");
    value["coupon_program"][0]["spec"]["rate"] = json!("1.0");
    value["coupon_program"][0]["spec"]["frequency"]["count"] = json!(12);
    let error = serde_json::from_value::<CashflowScheduleBuildSpec>(value)
        .expect("individually representable principal and coupon")
        .build(None)
        .expect_err("PIK capitalization exceeds the decimal balance range");
    assert!(error.to_string().contains("outstanding principal exceeds"));
}

#[test]
fn fixed_coupon_decimal_overflow_returns_an_error() {
    let mut value = spec("2025-01-01", "2026-01-01");
    value["notional"]["initial"]["amount"] = json!("50000000000000000000000000000");
    value["coupon_program"][0]["spec"]["rate"] = json!("2.0");
    value["coupon_program"][0]["spec"]["frequency"]["count"] = json!(12);
    let error = serde_json::from_value::<CashflowScheduleBuildSpec>(value)
        .expect("representable principal and decimal rate")
        .build(None)
        .expect_err("coupon product exceeds the decimal amount range");
    assert!(error
        .to_string()
        .contains("fixed coupon amount exceeds Decimal range"));
}

#[test]
fn ex_coupon_rebates_all_principal_segments_of_the_forfeited_payment() {
    use finstack_quant_cashflows::{AccrualConfig, AccrualIndex, AccrualMethod, ExCouponRule};

    let mut value = spec("2025-01-01", "2025-07-01");
    value["notional"]["initial"]["amount"] = json!("100");
    value["coupon_program"][0]["spec"]["rate"] = json!("0.12");
    value["coupon_program"][0]["spec"]["frequency"]["count"] = json!(6);
    value["principal_events"] = json!([{
        "date": "2025-06-28", "payment_date": "2025-06-28", "kind": "notional",
        "delta": {"amount": "100", "currency": "USD"}
    }]);
    let schedule = build(value, None);
    for method in [AccrualMethod::Linear, AccrualMethod::Compounded] {
        let config = AccrualConfig {
            method: method.clone(),
            ex_coupon: Some(ExCouponRule {
                days_before_coupon: 7,
                calendar_id: None,
            }),
            ..Default::default()
        };
        let index = AccrualIndex::build(&schedule, &config).expect("segmented coupon index");
        for (as_of, expected) in [
            (
                "2025-06-25",
                if matches!(method, AccrualMethod::Linear) {
                    -0.3
                } else {
                    -100.0 * ((1.0_f64 + 0.12 * 178.0 / 360.0).powf(3.0 / 178.0) - 1.0) - 0.2
                },
            ),
            (
                "2025-06-29",
                if matches!(method, AccrualMethod::Linear) {
                    -0.12 * 200.0 * 2.0 / 360.0
                } else {
                    -200.0 * ((1.0_f64 + 0.12 * 3.0 / 360.0).powf(2.0 / 3.0) - 1.0)
                },
            ),
        ] {
            close(
                index
                    .accrued_at(date(as_of))
                    .expect("valid ex-coupon rebate"),
                expected,
            );
            close(
                finstack_quant_cashflows::accrued_interest_amount(&schedule, date(as_of), &config)
                    .expect("direct segmented rebate"),
                expected,
            );
        }
    }
}

#[test]
fn ex_coupon_future_segment_keeps_the_zero_balance_prefix_in_its_window() {
    let mut value = spec("2025-01-01", "2025-07-01");
    value["notional"]["initial"]["amount"] = json!("0");
    value["coupon_program"][0]["spec"]["rate"] = json!("0.12");
    value["coupon_program"][0]["spec"]["frequency"]["count"] = json!(6);
    value["principal_events"] = json!([{
        "date": "2025-06-28", "payment_date": "2025-06-28", "kind": "notional",
        "delta": {"amount": "100", "currency": "USD"}
    }]);
    let schedule = build(value, None);
    let config = finstack_quant_cashflows::AccrualConfig {
        ex_coupon: Some(finstack_quant_cashflows::ExCouponRule {
            days_before_coupon: 7,
            calendar_id: None,
        }),
        ..Default::default()
    };
    close(
        finstack_quant_cashflows::accrued_interest_amount(&schedule, date("2025-06-25"), &config)
            .expect("future coupon was forfeited despite the zero-balance prefix"),
        -0.1,
    );
}

#[test]
fn schedule_act365l_accrual_preserves_the_contractual_leap_denominator() {
    let mut value = spec("2023-03-01", "2024-03-01");
    value["notional"]["initial"]["amount"] = json!("100");
    value["coupon_program"][0]["spec"]["rate"] = json!("0.12");
    value["coupon_program"][0]["spec"]["day_count"] = json!("act_365l");
    value["coupon_program"][0]["spec"]["frequency"]["count"] = json!(12);
    let schedule = build(value, None);
    close(
        finstack_quant_cashflows::accrued_interest_amount(
            &schedule,
            date("2023-04-01"),
            &finstack_quant_cashflows::AccrualConfig::default(),
        )
        .expect("elapsed accrual before the contractual leap day"),
        100.0 * 0.12 * 31.0 / 366.0,
    );
}

#[test]
fn step_up_uses_contractual_start_under_both_adjustment_directions() {
    for (issue, maturity, convention, step, expected_rate) in [
        ("2025-01-05", "2025-07-05", "following", "2025-04-06", 0.04),
        ("2025-01-06", "2025-07-06", "preceding", "2025-04-06", 0.08),
    ] {
        let mut value = spec(issue, maturity);
        value["coupon_program"][0]["kind"] = json!("step_up");
        let coupon = value["coupon_program"][0]["spec"]
            .as_object_mut()
            .expect("coupon");
        coupon.remove("rate");
        coupon.insert("initial_rate".into(), json!("0.04"));
        coupon.insert("step_schedule".into(), json!([[step, "0.08"]]));
        coupon.insert("business_day_convention".into(), json!(convention));
        coupon.insert("adjust_accrual_dates".into(), json!(true));
        let schedule = build(value, None);
        let coupon = schedule.coupons().last().expect("last coupon");
        close(coupon.rate.expect("coupon rate"), expected_rate);
        close(
            coupon.amount.amount(),
            1_000_000.0 * coupon.accrual_factor * expected_rate,
        );
    }
}

#[test]
fn principal_events_after_adjusted_terminal_boundary_are_rejected() {
    for (kind, amount) in [("notional", "100000"), ("amortization", "-100000")] {
        let mut value = spec("2024-12-31", "2025-03-30");
        value["coupon_program"][0]["spec"]["business_day_convention"] = json!("preceding");
        value["coupon_program"][0]["spec"]["adjust_accrual_dates"] = json!(true);
        value["principal_events"] = json!([{
            "date": "2025-03-30", "payment_date": "2025-03-30", "kind": kind,
            "delta": {"amount": amount, "currency": "USD"}
        }]);
        let error = serde_json::from_value::<CashflowScheduleBuildSpec>(value.clone())
            .expect("valid spec")
            .build(None)
            .expect_err("event after final economic boundary");
        assert!(error
            .to_string()
            .contains("effective terminal accrual date 2025-03-28"));
        value["coupon_program"][0]["spec"]["adjust_accrual_dates"] = json!(false);
        let schedule = build(value, None);
        close(
            schedule
                .outstanding_by_date()
                .expect("balances")
                .last()
                .expect("terminal balance")
                .1
                .amount(),
            0.0,
        );
    }
}

#[test]
fn pik_and_same_date_draws_are_included_in_remaining_principal_targets() {
    let mut value = spec("2025-01-15", "2026-01-15");
    value["coupon_program"][0]["spec"]["coupon_type"] = json!("pik");
    value["notional"]["amort"] = json!({"step_remaining": {"schedule": [
        ["2025-04-15", {"amount": "800000", "currency": "USD"}]
    ]}});
    value["principal_events"] = json!([{
        "date": "2025-04-15", "payment_date": "2025-04-15", "kind": "notional",
        "delta": {"amount": "50000", "currency": "USD"}
    }]);
    let schedule = build(value, None);
    let remaining = schedule
        .outstanding_by_date()
        .expect("economic balance replay")
        .into_iter()
        .find(|(day, _)| *day == date("2025-04-15"))
        .expect("target boundary")
        .1;
    close(remaining.amount(), 800_000.0);
    let repayment: f64 = schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.kind == CFKind::Amortization && flow.date == date("2025-04-15"))
        .map(|flow| flow.amount.amount())
        .sum();
    close(repayment, 260_000.0);
}

#[test]
fn linear_between_full_payoff_includes_final_pik_and_principal_movements() {
    let mut value = spec("2025-01-15", "2026-01-15");
    value["coupon_program"][0]["spec"]["coupon_type"] = json!("pik");
    value["notional"]["amort"] = json!({"linear_between": {
        "start": "2025-01-15", "end": "2025-07-15"
    }});
    value["principal_events"] = json!([{
        "date": "2025-07-15", "payment_date": "2025-07-15", "kind": "notional",
        "delta": {"amount": "50000", "currency": "USD"}
    }]);
    let schedule = build(value, None);
    let remaining = schedule
        .outstanding_by_date()
        .expect("economic balance replay")
        .into_iter()
        .find(|(day, _)| *day == date("2025-07-15"))
        .expect("full payoff boundary")
        .1;
    close(remaining.amount(), 0.0);
    assert!(!schedule
        .get_flows()
        .iter()
        .any(|flow| flow.kind == CFKind::Pik && flow.date > date("2025-07-15")));
    assert!(!schedule
        .get_flows()
        .iter()
        .any(|flow| flow.kind == CFKind::Notional && flow.amount.amount() > 0.0));
    let principal_paid: f64 = schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.kind == CFKind::Amortization)
        .map(|flow| flow.amount.amount())
        .sum();
    let pik: f64 = schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.kind == CFKind::Pik)
        .map(|flow| flow.amount.amount())
        .sum();
    close(principal_paid, 1_050_000.0 + pik);
}

#[test]
fn linear_between_installments_use_the_balance_entering_the_window() {
    for (event_date, kind, delta, entering_balance) in [
        ("2025-03-15", "notional", "1000000", 2_000_000.0),
        ("2025-03-15", "amortization", "-250000", 750_000.0),
        ("2025-04-16", "notional", "1000000", 2_000_000.0),
        ("2025-01-15", "notional", "1000000", 2_000_000.0),
    ] {
        let mut value = spec("2025-01-15", "2026-01-15");
        value["notional"]["amort"] = json!({"linear_between": {
            "start": "2025-04-16", "end": "2026-01-15"
        }});
        value["principal_events"] = json!([{
            "date": event_date, "payment_date": event_date, "kind": kind,
            "delta": {"amount": delta, "currency": "USD"}
        }]);
        let schedule = build(value, None);
        let installments: Vec<_> = schedule
            .get_flows()
            .iter()
            .filter(|flow| flow.kind == CFKind::Amortization && flow.date > date("2025-04-16"))
            .collect();
        assert_eq!(installments.len(), 3);
        for installment in installments {
            close(installment.amount.amount(), entering_balance / 3.0);
        }
    }
}

#[test]
fn linear_between_uses_effective_boundaries_and_lagged_cash_settlement() {
    let mut value = spec("2025-01-15", "2026-01-15");
    value["notional"]["amort"] = json!({"linear_between": {
        "start": "2025-01-15", "end": "2025-07-15"
    }});
    value["coupon_program"][0]["spec"]["payment_lag_days"] = json!(2);
    let schedule = build(value, None);
    let final_installment = schedule
        .get_flows()
        .iter()
        .rev()
        .find(|flow| flow.kind == CFKind::Amortization)
        .expect("final installment");
    assert_eq!(final_installment.principal_date, Some(date("2025-07-15")));
    assert_eq!(final_installment.date, date("2025-07-17"));
    let balance = schedule
        .outstanding_by_date()
        .expect("balance replay")
        .into_iter()
        .find(|(day, _)| *day == date("2025-07-15"))
        .expect("economic payoff date")
        .1;
    close(balance.amount(), 0.0);
}

#[test]
fn linear_between_rejects_unaligned_or_out_of_horizon_windows() {
    for (start, end, message) in [
        (
            "2025-01-15",
            "2025-07-16",
            "must be a coupon accrual boundary",
        ),
        ("2024-12-15", "2025-07-15", "outside"),
        (
            "2025-01-15",
            "2026-04-15",
            "effective terminal accrual date",
        ),
    ] {
        let mut value = spec("2025-01-15", "2026-01-15");
        value["notional"]["amort"] = json!({"linear_between": {"start": start, "end": end}});
        let error = serde_json::from_value::<CashflowScheduleBuildSpec>(value)
            .expect("valid wire spec")
            .build(None)
            .expect_err("invalid economic amortization window");
        assert!(error.to_string().contains(message), "{error}");
    }
}

#[test]
fn explicit_amortization_after_adjusted_terminal_boundary_is_rejected() {
    for amortization in [
        json!({"custom_principal": {"items": [
            ["2025-03-30", {"amount": "500000", "currency": "USD"}]
        ]}}),
        json!({"step_remaining": {"schedule": [
            ["2025-03-30", {"amount": "500000", "currency": "USD"}]
        ]}}),
    ] {
        let mut value = spec("2024-12-31", "2025-03-30");
        value["notional"]["amort"] = amortization;
        value["coupon_program"][0]["spec"]["business_day_convention"] = json!("preceding");
        value["coupon_program"][0]["spec"]["adjust_accrual_dates"] = json!(true);
        let error = serde_json::from_value::<CashflowScheduleBuildSpec>(value.clone())
            .expect("valid wire spec")
            .build(None)
            .expect_err("amortization after final economic boundary");
        assert!(error.to_string().contains(
            "amortization on 2025-03-30 is after the effective terminal accrual date 2025-03-28"
        ));
        value["coupon_program"][0]["spec"]["adjust_accrual_dates"] = json!(false);
        let schedule = build(value, None);
        assert!(schedule.get_flows().iter().any(|flow| {
            flow.kind == CFKind::Amortization && flow.principal_date == Some(date("2025-03-30"))
        }));
    }
}

fn overnight_spec() -> Value {
    let mut value = spec("2025-01-06", "2025-01-08");
    value["coupon_program"][0]["kind"] = json!("floating");
    value["coupon_program"][0]["spec"]
        .as_object_mut()
        .expect("coupon")
        .remove("rate");
    value["coupon_program"][0]["spec"]["rate_spec"] = json!({
        "forward_curve_id": "RFR", "spread_bp": "0",
        "reset_frequency": {"count": 3, "unit": "months"}, "reset_lag_days": 0,
        "compounding": {"compounded_in_arrears": {"lookback_days": 0}},
        "overnight_index_constraints": "period"
    });
    value["principal_events"] = json!([{
        "date": "2025-01-07", "payment_date": "2025-01-07", "kind": "amortization",
        "delta": {"amount": "-500000", "currency": "USD"}
    }]);
    value
}

fn overnight_market() -> MarketContext {
    let curve = ForwardCurve::builder("RFR", 1.0 / 360.0)
        .base_date(date("2025-01-06"))
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.2), (1.0 / 360.0, 0.0), (1.0, 0.0)])
        .build()
        .expect("valid curve");
    MarketContext::new().insert(curve)
}

#[test]
fn overnight_period_bounds_use_final_rate_and_uniform_accrual_adjustment() {
    let market = overnight_market();
    let unbounded = build(overnight_spec(), Some(&market));
    let raw_interest: f64 = unbounded.coupons().map(|flow| flow.amount.amount()).sum();
    close(raw_interest, 1_000_000.0 * 0.1 / 360.0);
    for (field, bound, annual_adjustment) in [
        ("index_cap_bp", "600", 0.0),
        ("index_floor_bp", "400", 0.0),
        ("index_cap_bp", "300", -0.02),
        ("index_floor_bp", "700", 0.02),
        ("all_in_cap_bp", "300", -0.02),
        ("all_in_floor_bp", "700", 0.02),
    ] {
        let mut value = overnight_spec();
        value["coupon_program"][0]["spec"]["rate_spec"][field] = json!(bound);
        let schedule = build(value, Some(&market));
        let interest: f64 = schedule.coupons().map(|flow| flow.amount.amount()).sum();
        close(
            interest,
            raw_interest + 1_500_000.0 * annual_adjustment / 360.0,
        );
    }
}

#[test]
fn overnight_daily_bounds_are_applied_before_compounding() {
    let market = overnight_market();
    let mut value = overnight_spec();
    let rate = &mut value["coupon_program"][0]["spec"]["rate_spec"];
    rate["overnight_index_constraints"] = json!("daily");
    rate["index_cap_bp"] = json!("600");
    let schedule = build(value, Some(&market));
    let interest: f64 = schedule.coupons().map(|flow| flow.amount.amount()).sum();
    close(interest, 1_000_000.0 * 0.06 / 360.0);
}

#[test]
fn bounded_split_pik_capitalizes_net_coupon_on_one_contractual_boundary() {
    let market = overnight_market();
    for coupon_type in [
        json!("pik"),
        json!({"split": {"cash_fraction": "0.4", "pik_fraction": "0.6"}}),
    ] {
        let mut value = overnight_spec();
        value["coupon_program"][0]["spec"]["coupon_type"] = coupon_type;
        value["coupon_program"][0]["spec"]["rate_spec"]["index_cap_bp"] = json!("300");
        let schedule = build(value, Some(&market));
        let expected_interest = 1_000_000.0 * 0.1 / 360.0 - 1_500_000.0 * 0.02 / 360.0;
        let pik: f64 = schedule
            .get_flows()
            .iter()
            .filter(|flow| flow.kind == CFKind::Pik)
            .map(|flow| flow.amount.amount())
            .sum();
        let cash: f64 = schedule
            .coupons()
            .filter(|flow| flow.kind == CFKind::FloatReset)
            .map(|flow| flow.amount.amount())
            .sum();
        close(cash + pik, expected_interest);
        assert!(schedule
            .get_flows()
            .iter()
            .filter(|flow| flow.kind == CFKind::Pik)
            .all(|flow| flow.principal_date == Some(date("2025-01-08"))));
        assert!(schedule
            .get_flows()
            .iter()
            .any(|flow| flow.kind == CFKind::Pik && flow.amount.amount() < 0.0));
        let repayment = schedule
            .get_flows()
            .iter()
            .find(|flow| flow.kind == CFKind::Notional && flow.amount.amount() > 0.0)
            .expect("redemption");
        close(repayment.amount.amount(), 500_000.0 + pik);
        close(
            schedule
                .outstanding_by_date()
                .expect("balance replay")
                .last()
                .expect("final balance")
                .1
                .amount(),
            0.0,
        );
    }
}

#[test]
fn overnight_zero_daycount_segment_keeps_replay_clock_and_emits_zero() {
    let mut value = overnight_spec();
    value["issue_date"] = json!("2025-01-30");
    value["maturity"] = json!("2025-02-03");
    value["principal_events"][0]["date"] = json!("2025-01-31");
    value["principal_events"][0]["payment_date"] = json!("2025-01-31");
    value["coupon_program"][0]["spec"]["day_count"] = json!("30_360");
    value["coupon_program"][0]["spec"]["rate_spec"]["overnight_basis"] = json!("act_360");
    let curve = ForwardCurve::builder("RFR", 1.0 / 360.0)
        .base_date(date("2025-01-30"))
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.1), (1.0, 0.1)])
        .build()
        .expect("flat curve");
    let schedule = build(value, Some(&MarketContext::new().insert(curve)));
    let zero = schedule
        .coupons()
        .find(|flow| {
            flow.accrual
                .as_ref()
                .is_some_and(|accrual| accrual.end == date("2025-01-31"))
        })
        .expect("zero-daycount segment");
    close(zero.accrual_factor, 0.0);
    close(zero.amount.amount(), 0.0);
    assert!(schedule
        .coupons()
        .all(|flow| flow.amount.amount().is_finite()));
}

#[test]
fn weekend_balance_splits_record_one_raw_fixing_and_materialize_without_conflicts() {
    let mut value = overnight_spec();
    value["issue_date"] = json!("2025-01-04");
    value["maturity"] = json!("2025-01-07");
    value["principal_events"][0]["date"] = json!("2025-01-05");
    value["principal_events"][0]["payment_date"] = json!("2025-01-05");
    let base = date("2025-01-03");
    let curve = ForwardCurve::builder("RFR", 1.0 / 360.0)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.02), (0.1, 0.15), (1.0, 0.15)])
        .build()
        .expect("sloped curve");
    let market = MarketContext::new().insert(curve);
    let schedule = build(value.clone(), Some(&market));
    let known: Vec<_> = schedule
        .get_meta()
        .projected_fixings
        .iter()
        .filter(|fixing| fixing.date == base && fixing.value.is_some())
        .collect();
    assert_eq!(known.len(), 1);
    let expected = market
        .get_forward("RFR")
        .expect("curve")
        .rate_period(0.0, 1.0 / 360.0);
    close(known[0].value.expect("raw fixing"), expected);
    let realized = finstack_quant_cashflows::fixings::materialize_fixings(
        &market,
        [&schedule],
        date("2025-01-02"),
        date("2025-01-07"),
    )
    .expect("raw published observations have no conflicting slice-tenor values")
    .insert(
        ForwardCurve::builder("RFR", 1.0 / 360.0)
            .base_date(date("2025-01-07"))
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.3), (1.0, 0.3)])
            .build()
            .expect("post-roll curve"),
    );
    let replayed = build(value, Some(&realized));
    close(
        replayed.coupons().map(|flow| flow.amount.amount()).sum(),
        schedule.coupons().map(|flow| flow.amount.amount()).sum(),
    );
}

//! Principal history, exact repayment, and periodic fee regression cases.

use finstack_quant_cashflows::CashflowScheduleBuildSpec;
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::cashflow::CFKind;
use serde_json::{Value, json};

fn spec(initial: &str) -> Value {
    json!({
        "notional": {"initial": {"amount": initial, "currency": "USD"}, "amort": "none"},
        "issue_date": "2024-01-10", "maturity": "2024-12-20"
    })
}

fn build(value: Value) -> finstack_quant_core::Result<CashFlowSchedule> {
    serde_json::from_value::<CashflowScheduleBuildSpec>(value)
        .expect("valid build specification")
        .build(None)
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= expected.abs().max(1.0) * 1e-12,
        "actual {actual}, expected {expected}"
    );
}

#[test]
fn cds_roll_program_rejects_an_off_roll_issue() {
    // Record CORE-CONVENTIONS-008: no front accrual from the roll before an
    // off-roll issue date; the schedule builder never substitutes the start.
    let mut value = spec("1000000");
    value["coupon_program"] = json!([{"kind": "fixed", "spec": {
        "coupon_type": "cash", "rate": "0.1", "frequency": {"count": 3, "unit": "months"},
        "day_count": "act_360", "business_day_convention": "unadjusted",
        "calendar_id": "weekends_only", "stub": "short_back", "roll_rule": "cds_imm"
    }}]);
    let error = build(value).expect_err("an off-roll CDS issue is rejected");
    assert!(error.to_string().contains("CDS roll date"), "{error}");
}

#[test]
fn custom_principal_preserves_exact_cents_and_rejects_real_overpayment() {
    let mut value = spec("0.3");
    value["notional"]["amort"] = json!({"custom_principal": {"items": [
        ["2024-04-01", {"amount": "0.1", "currency": "USD"}],
        ["2024-07-01", {"amount": "0.2", "currency": "USD"}]
    ]}});
    let schedule = build(value.clone()).expect("exact decimal principal conservation");
    close(
        schedule
            .outstanding_by_date()
            .expect("balances")
            .last()
            .expect("balance")
            .1
            .amount(),
        0.0,
    );
    value["notional"]["amort"]["custom_principal"]["items"][1][1]["amount"] = json!("0.21");
    assert!(
        build(value)
            .expect_err("one cent excess is not rounding noise")
            .to_string()
            .contains("exceeds outstanding principal")
    );
}

#[test]
fn explicit_amortization_uses_funding_on_its_effective_date() {
    for amort in [
        json!({"custom_principal": {"items": [
            ["2024-07-01", {"amount": "150", "currency": "USD"}]
        ]}}),
        json!({"step_remaining": {"schedule": [
            ["2024-04-01", {"amount": "50", "currency": "USD"}],
            ["2024-07-01", {"amount": "120", "currency": "USD"}]
        ]}}),
    ] {
        let mut value = spec("100");
        value["notional"]["amort"] = amort;
        value["principal_events"] = json!([{
            "date": "2024-07-01", "payment_date": "2024-07-01", "kind": "notional",
            "delta": {"amount": "100", "currency": "USD"}
        }]);
        let schedule = build(value.clone()).expect("repayment funded by same-date draw");
        schedule.validate().expect("valid resolved principal path");
        value["principal_events"] = json!([]);
        assert!(
            build(value)
                .expect_err("repayment must not exceed live principal")
                .to_string()
                .contains("exceeds outstanding principal")
        );
    }
}

#[test]
fn explicit_amortization_can_repay_capitalized_pik_above_initial_principal() {
    let mut value = spec("100");
    value["issue_date"] = json!("2024-01-01");
    value["maturity"] = json!("2024-03-31");
    value["coupon_program"] = json!([{"kind": "fixed", "spec": {
        "coupon_type": "pik", "rate": "0.12", "frequency": {"count": 3, "unit": "months"},
        "day_count": "act_360", "business_day_convention": "unadjusted",
        "calendar_id": "weekends_only", "stub": "short_back"
    }}]);
    value["notional"]["amort"] = json!({"custom_principal": {"items": [
        ["2024-03-31", {"amount": "103", "currency": "USD"}]
    ]}});
    let schedule = build(value).expect("90 days of PIK fund the three-unit excess repayment");
    let amortization: f64 = schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.get_kind() == CFKind::Amortization)
        .map(|flow| flow.get_amount().amount())
        .sum();
    close(amortization, 103.0);
    close(
        schedule
            .outstanding_by_date()
            .expect("balances")
            .last()
            .expect("balance")
            .1
            .amount(),
        0.0,
    );
}

#[test]
fn periodic_fee_normalizes_basis_points_before_large_notional_multiplication() {
    let mut value = spec("100000000000000000000");
    value["issue_date"] = json!("2025-01-01");
    value["maturity"] = json!("2025-04-01");
    value["fees"] = json!([{"periodic_bp": {
        "base": "drawn", "bp": "10000000000", "frequency": {"count": 3, "unit": "months"},
        "day_count": "act_360", "business_day_convention": "unadjusted",
        "calendar_id": "weekends_only", "stub": "short_back"
    }}]);
    let schedule = build(value.clone()).expect("representable fee must not overflow intermediates");
    let fee = schedule
        .get_flows()
        .iter()
        .find(|flow| flow.get_kind() == CFKind::Fee)
        .expect("fee");
    close(fee.get_amount().amount(), 2.5e25);
    value["fees"][0]["periodic_bp"]["bp"] = json!("100000000000000000000");
    assert!(
        build(value)
            .expect_err("unrepresentable final fee must return an error")
            .to_string()
            .contains("periodic fee amount exceeds Decimal range")
    );
}

#[test]
fn periodic_fee_preserves_tiny_quotes_on_large_notionals() {
    for (bp, expected_fee, expected_rate) in [
        ("0.000000000000000000000001", 0.25, 1e-28),
        ("0.0000000000000000000000000001", 0.000025, 1e-32),
        ("-0.000000000000000000000001", -0.25, -1e-28),
    ] {
        let mut value = spec("10000000000000000000000000000");
        value["issue_date"] = json!("2025-01-01");
        value["maturity"] = json!("2025-04-01");
        value["fees"] = json!([{"periodic_bp": {
            "base": "drawn", "bp": bp, "frequency": {"count": 3, "unit": "months"},
            "day_count": "act_360", "business_day_convention": "unadjusted",
            "calendar_id": "weekends_only", "stub": "short_back"
        }}]);
        let schedule = build(value).expect("representable fee from a tiny quote");
        let fee = schedule
            .get_flows()
            .iter()
            .find(|flow| flow.get_kind() == CFKind::Fee)
            .expect("nonzero fee");
        close(fee.get_amount().amount(), expected_fee);
        close(
            fee.get_rate().expect("annual rate metadata") / expected_rate,
            1.0,
        );
    }
}

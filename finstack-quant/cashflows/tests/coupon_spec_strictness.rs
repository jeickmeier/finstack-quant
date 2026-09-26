//! Regression tests for strict coupon-spec deserialization.
//!
//! These specs previously combined `deny_unknown_fields` with `flatten`, making
//! unknown-field rejection a silent no-op.

use finstack_quant_cashflows::builder::{FixedCouponSpec, FloatingCouponSpec, StepUpCouponSpec};

#[test]
fn fixed_coupon_spec_accepts_known_fields() {
    let json = r#"{
        "rate": "0.05",
        "coupon_type": "cash",
        "frequency": {"count": 6, "unit": "months"},
        "day_count": "30_360",
        "business_day_convention": "following",
        "calendar_id": "weekends_only",
        "stub": "none",
        "end_of_month": false,
        "payment_lag_days": 0,
        "adjust_accrual_dates": false
    }"#;
    let spec: FixedCouponSpec = serde_json::from_str(json).expect("known fields must deserialize");
    assert_eq!(spec.schedule.calendar_id, "weekends_only");
}

#[test]
fn fixed_coupon_spec_rejects_unknown_field() {
    let json = r#"{
        "rate": "0.05",
        "frequency": {"count": 6, "unit": "months"},
        "day_count": "30_360",
        "calendar_id": "weekends_only",
        "totally_bogus_field": 42
    }"#;
    let error =
        serde_json::from_str::<FixedCouponSpec>(json).expect_err("unknown fields must be rejected");
    assert!(
        error.to_string().contains("totally_bogus_field"),
        "error must name the offending key: {error}"
    );
}

#[test]
fn floating_coupon_spec_rejects_misspelled_schedule_field() {
    let json = r#"{
        "rate_spec": {
            "forward_curve_id": "SOFR",
            "spread_bp": "10",
            "reset_frequency": {"count": 3, "unit": "months"},
            "reset_lag_days": 0,
            "fallback": "spread_only"
        },
        "frequency": {"count": 3, "unit": "months"},
        "day_count": "act_360",
        "calendar_id": "weekends_only",
        "calender_id": "weekends_only"
    }"#;
    let result = serde_json::from_str::<FloatingCouponSpec>(json);
    assert!(
        result.is_err(),
        "a misspelled schedule field must be rejected"
    );
}

#[test]
fn floating_coupon_spec_rejects_pascal_case_fallback() {
    let json = r#"{
        "rate_spec": {
            "forward_curve_id": "SOFR",
            "spread_bp": "10",
            "reset_frequency": {"count": 3, "unit": "months"},
            "reset_lag_days": 0,
            "fallback": "SpreadOnly"
        },
        "coupon_type": "cash",
        "frequency": {"count": 3, "unit": "months"},
        "day_count": "act_360",
        "business_day_convention": "following",
        "calendar_id": "weekends_only",
        "stub": "none"
    }"#;
    let error = serde_json::from_str::<FloatingCouponSpec>(json)
        .expect_err("legacy PascalCase fallback must be rejected");
    assert!(error.to_string().contains("SpreadOnly"));
}

#[test]
fn step_up_coupon_spec_rejects_unknown_field() {
    let json = r#"{
        "initial_rate": "0.03",
        "step_schedule": [],
        "frequency": {"count": 6, "unit": "months"},
        "day_count": "30_360",
        "calendar_id": "weekends_only",
        "nope": true
    }"#;
    let error = serde_json::from_str::<StepUpCouponSpec>(json)
        .expect_err("unknown fields must be rejected");
    assert!(
        error.to_string().contains("nope"),
        "error must name the offending key: {error}"
    );
}

// Credit-model specs (prepayment / default / recovery) must be closed inbound
// types: a typo'd key silently deserializing to defaults changes cashflows.

#[test]
fn prepayment_model_spec_rejects_unknown_field() {
    use finstack_quant_cashflows::builder::PrepaymentModelSpec;

    let error = serde_json::from_str::<PrepaymentModelSpec>(
        r#"{"cpr": 0.06, "curve": {"curve": "psa", "speed_multiplier": 1.0}, "speed_multipler": 2.0}"#,
    )
    .expect_err("unknown fields must be rejected");
    assert!(
        error.to_string().contains("speed_multipler"),
        "error must name the offending key: {error}"
    );
}

#[test]
fn default_model_spec_rejects_unknown_field() {
    use finstack_quant_cashflows::builder::DefaultModelSpec;

    let error = serde_json::from_str::<DefaultModelSpec>(r#"{"cdr": 0.02, "sda_multiplier": 1.5}"#)
        .expect_err("unknown fields must be rejected");
    assert!(
        error.to_string().contains("sda_multiplier"),
        "error must name the offending key: {error}"
    );
}

#[test]
fn recovery_model_spec_rejects_unknown_field() {
    use finstack_quant_cashflows::builder::RecoveryModelSpec;

    let error = serde_json::from_str::<RecoveryModelSpec>(
        r#"{"rate": 0.40, "recovery_lag": 12, "recovery_lag_months": 12}"#,
    )
    .expect_err("unknown fields must be rejected");
    assert!(
        error.to_string().contains("recovery_lag_months"),
        "error must name the offending key: {error}"
    );
}

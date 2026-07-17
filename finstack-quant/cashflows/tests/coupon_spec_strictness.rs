//! Regression tests for strict coupon-spec deserialization.
//!
//! These specs previously combined `deny_unknown_fields` with `flatten`, making
//! unknown-field rejection a silent no-op.

use finstack_quant_cashflows::builder::{FixedCouponSpec, FloatingCouponSpec, StepUpCouponSpec};

#[test]
fn fixed_coupon_spec_accepts_known_fields() {
    let json = r#"{
        "rate": "0.05",
        "coupon_type": "Cash",
        "freq": {"count": 6, "unit": "months"},
        "dc": "Thirty360",
        "bdc": "following",
        "calendar_id": "weekends_only",
        "stub": "None",
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
        "freq": {"count": 6, "unit": "months"},
        "dc": "Thirty360",
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
            "index_id": "SOFR",
            "spread_bp": "10",
            "reset_freq": {"count": 3, "unit": "months"},
            "reset_lag_days": 0,
            "fallback": "SpreadOnly"
        },
        "freq": {"count": 3, "unit": "months"},
        "dc": "Act360",
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
fn step_up_coupon_spec_rejects_unknown_field() {
    let json = r#"{
        "initial_rate": "0.03",
        "step_schedule": [],
        "freq": {"count": 6, "unit": "months"},
        "dc": "Thirty360",
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

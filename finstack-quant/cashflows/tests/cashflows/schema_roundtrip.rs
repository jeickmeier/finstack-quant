//! Cashflow schema and example parity tests.
//!
//! Validates cashflow JSON examples and the crate-owned schemas under
//! `schemas/cashflow/1/`.
//! can be deserialized into the corresponding Rust types and re-serialized
//! back to equivalent JSON.
//!
//! Also verifies that deserialized values match expected market-standard conventions.

use finstack_quant_cashflows::builder::specs::{
    AmortizationSpec, CouponType, DefaultEvent, DefaultModelSpec, FeeSpec, FeeTier,
    FixedCouponSpec, FloatingCouponSpec, FloatingRateSpec, Notional, PrepaymentCurve,
    PrepaymentModelSpec, RecoveryModelSpec, ScheduleParams, StepUpCouponSpec,
};
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Generic envelope for cashflow specs with schema version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CashflowEnvelope<T> {
    schema: String,
    #[serde(flatten)]
    payload: T,
}

#[test]
fn cashflows_owns_seven_resolvable_schema_resources() {
    let resources = finstack_quant_cashflows::schema::resources()
        .expect("embedded cashflow schemas are valid resources");
    assert_eq!(resources.len(), 7);
    assert!(resources
        .iter()
        .all(|(uri, _)| uri.starts_with(finstack_quant_cashflows::schema::CASHFLOW_SCHEMA_BASE)));
}

// Payload wrapper types for each spec
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct NotionalPayload {
    notional: Notional,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AmortizationSpecPayload {
    amortization_spec: AmortizationSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CouponTypePayload {
    coupon_type: CouponType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FixedCouponSpecPayload {
    fixed_coupon_spec: FixedCouponSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FloatingRateSpecPayload {
    floating_rate_spec: FloatingRateSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FloatingCouponSpecPayload {
    floating_coupon_spec: FloatingCouponSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct PrepaymentModelSpecPayload {
    prepayment_model_spec: PrepaymentModelSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DefaultModelSpecPayload {
    default_model_spec: DefaultModelSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DefaultEventPayload {
    default_event: DefaultEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct RecoveryModelSpecPayload {
    recovery_model_spec: RecoveryModelSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FeeSpecPayload {
    fee_spec: FeeSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FeeTierPayload {
    fee_tier: FeeTier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScheduleParamsPayload {
    schedule_params: ScheduleParams,
}

/// Helper to deserialize, re-serialize, and compare JSON for parity.
fn test_roundtrip<T>(json_str: &str)
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    // Deserialize from example JSON
    let envelope: T = serde_json::from_str(json_str).expect("Failed to deserialize");

    // Re-serialize
    let reserialized = serde_json::to_string(&envelope).expect("Failed to serialize");

    // Parse both to Value for comparison (to ignore whitespace/ordering)
    let original_value: serde_json::Value =
        serde_json::from_str(json_str).expect("Failed to parse original JSON");
    let reserialized_value: serde_json::Value =
        serde_json::from_str(&reserialized).expect("Failed to parse reserialized JSON");

    assert_eq!(
        original_value, reserialized_value,
        "Roundtrip mismatch for envelope"
    );
}

fn fixed_schedule_build_spec() -> serde_json::Value {
    json!({
        "notional": {
            "initial": {"amount": "1000000", "currency": "USD"},
            "amort": "None"
        },
        "issue": "2024-08-31",
        "maturity": "2025-08-31",
        "coupon_program": [{
            "kind": "fixed",
            "spec": {
                "coupon_type": "Cash",
                "rate": "0.06",
                "freq": {"count": 12, "unit": "months"},
                "dc": "Thirty360",
                "bdc": "following",
                "calendar_id": "weekends_only",
                "stub": "None",
                "end_of_month": false,
                "payment_lag_days": 0
            }
        }]
    })
}

fn canonical_schedule_params() -> serde_json::Value {
    json!({
        "freq": {"count": 3, "unit": "months"},
        "dc": "Act360",
        "bdc": "following",
        "calendar_id": "weekends_only",
        "stub": "None",
        "end_of_month": false,
        "payment_lag_days": 0,
        "adjust_accrual_dates": false
    })
}

fn canonical_fixed_coupon(rate: &str) -> serde_json::Value {
    let mut spec = canonical_schedule_params();
    spec["coupon_type"] = json!("Cash");
    spec["rate"] = json!(rate);
    spec
}

fn canonical_floating_coupon(spread_bp: &str) -> serde_json::Value {
    let mut spec = canonical_schedule_params();
    spec["coupon_type"] = json!("Cash");
    spec["rate_spec"] = json!({
        "index_id": "TEST-INDEX",
        "spread_bp": spread_bp,
        "reset_freq": {"count": 3, "unit": "months"},
        "reset_lag_days": 0,
        "fallback": "SpreadOnly"
    });
    spec
}

fn canonical_build_spec(
    coupon_program: Vec<serde_json::Value>,
    payment_program: Vec<serde_json::Value>,
) -> serde_json::Value {
    json!({
        "notional": {
            "initial": {"amount": "1000000", "currency": "USD"},
            "amort": "None"
        },
        "issue": "2025-01-01",
        "maturity": "2027-01-01",
        "coupon_program": coupon_program,
        "payment_program": payment_program
    })
}

#[test]
fn test_json_bridge_build_validate_flows_and_accrual() {
    let spec_json = fixed_schedule_build_spec().to_string();
    let schedule_json = finstack_quant_cashflows::build_cashflow_schedule_json(&spec_json, None)
        .expect("schedule should build from JSON");

    let schedule: finstack_quant_cashflows::builder::CashFlowSchedule =
        serde_json::from_str(&schedule_json).expect("schedule JSON should deserialize");
    assert!(schedule.flows.iter().any(|flow| matches!(
        flow.kind,
        finstack_quant_cashflows::primitives::CFKind::Fixed
            | finstack_quant_cashflows::primitives::CFKind::FloatReset
            | finstack_quant_cashflows::primitives::CFKind::Stub
            | finstack_quant_cashflows::primitives::CFKind::InflationCoupon
    )));
    assert_eq!(
        schedule.meta.issue_date,
        Some(time::macros::date!(2024 - 08 - 31))
    );

    let validated = finstack_quant_cashflows::validate_cashflow_schedule_json(&schedule_json)
        .expect("schedule should validate");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&schedule_json).expect("schedule value"),
        serde_json::from_str::<serde_json::Value>(&validated).expect("validated value")
    );

    let dated = finstack_quant_cashflows::dated_flows_json(&schedule_json).expect("dated flows");
    let dated_flows: Vec<finstack_quant_cashflows::DatedFlowJson> =
        serde_json::from_str(&dated).expect("dated flow JSON");
    assert_eq!(dated_flows.len(), schedule.flows.len());

    let accrued =
        finstack_quant_cashflows::accrued_interest_json(&schedule_json, "2025-02-28", None)
            .expect("accrued interest");
    assert!(accrued > 0.0, "expected positive accrued interest");
}

#[test]
fn test_json_bridge_validates_pre_issue_principal_and_fee_flows() {
    // Delayed-funding structures legitimately carry principal-type flows and
    // fees dated before the issue date; the validator must accept them while
    // still rejecting interest-bearing flows before issue.
    let spec_json = fixed_schedule_build_spec().to_string();
    let schedule_json = finstack_quant_cashflows::build_cashflow_schedule_json(&spec_json, None)
        .expect("schedule should build from JSON");

    let mut schedule_value: serde_json::Value =
        serde_json::from_str(&schedule_json).expect("schedule JSON parses");
    let template_flow = schedule_value["flows"][0].clone();

    // Pre-issue principal event (Notional draw) and an up-front fee.
    let mut pre_issue_notional = template_flow.clone();
    pre_issue_notional["date"] = json!("2024-08-15");
    pre_issue_notional["kind"] = json!("Notional");
    pre_issue_notional["amount"] = json!({"amount": "-100000", "currency": "USD"});
    let mut pre_issue_fee = template_flow.clone();
    pre_issue_fee["date"] = json!("2024-08-15");
    pre_issue_fee["kind"] = json!("Fee");
    pre_issue_fee["amount"] = json!({"amount": "5000", "currency": "USD"});

    let flows = schedule_value["flows"].as_array_mut().expect("flows array");
    flows.insert(0, pre_issue_fee);
    flows.insert(0, pre_issue_notional);

    // Build -> validate round-trip succeeds with pre-issue principal/fee flows.
    let validated =
        finstack_quant_cashflows::validate_cashflow_schedule_json(&schedule_value.to_string())
            .expect("pre-issue principal and fee flows should validate");
    assert!(validated.contains("\"2024-08-15\""));

    // Interest-bearing flows before issue are still rejected.
    let mut bad_schedule: serde_json::Value =
        serde_json::from_str(&schedule_json).expect("schedule JSON parses");
    let mut pre_issue_coupon = template_flow;
    pre_issue_coupon["date"] = json!("2024-08-15");
    pre_issue_coupon["kind"] = json!("Fixed");
    bad_schedule["flows"]
        .as_array_mut()
        .expect("flows array")
        .insert(0, pre_issue_coupon);
    let err = finstack_quant_cashflows::validate_cashflow_schedule_json(&bad_schedule.to_string())
        .expect_err("pre-issue interest-bearing flow must be rejected");
    assert!(
        format!("{err}").contains("before issue date"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_json_bridge_amortizing_schedule_build() {
    let mut spec = fixed_schedule_build_spec();
    spec["maturity"] = json!("2026-08-31");
    spec["notional"]["amort"] = json!({
        "LinearTo": {"final_notional": {"amount": "0", "currency": "USD"}}
    });

    let schedule_json =
        finstack_quant_cashflows::build_cashflow_schedule_json(&spec.to_string(), None)
            .expect("amortizing schedule should build");
    let schedule: finstack_quant_cashflows::builder::CashFlowSchedule =
        serde_json::from_str(&schedule_json).expect("schedule JSON should deserialize");
    assert!(schedule.flows.iter().any(|flow| matches!(
        flow.kind,
        finstack_quant_cashflows::primitives::CFKind::Amortization
    )));
}

fn canonicalize_floating_rate_keys(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(legacy_floor) = map.remove("floor_bp") {
                map.insert("index_floor_bp".to_string(), legacy_floor);
            }
            if let Some(legacy_cap) = map.remove("cap_bp") {
                map.insert("all_in_cap_bp".to_string(), legacy_cap);
            }
            for nested in map.values_mut() {
                canonicalize_floating_rate_keys(nested);
            }
        }
        serde_json::Value::Array(values) => {
            for nested in values {
                canonicalize_floating_rate_keys(nested);
            }
        }
        _ => {}
    }
}

#[test]
fn test_notional_par() {
    let json = include_str!("examples/notional_par.example.json");
    test_roundtrip::<CashflowEnvelope<NotionalPayload>>(json);
}

#[test]
fn test_notional_percent_per_period() {
    let json = include_str!("examples/notional_percent_per_period.example.json");
    test_roundtrip::<CashflowEnvelope<NotionalPayload>>(json);
}

#[test]
fn test_amortization_linear_to() {
    let json = include_str!("examples/amortization_linear_to.example.json");
    test_roundtrip::<CashflowEnvelope<AmortizationSpecPayload>>(json);
}

#[test]
fn test_amortization_step_remaining() {
    let json = include_str!("examples/amortization_step_remaining.example.json");
    test_roundtrip::<CashflowEnvelope<AmortizationSpecPayload>>(json);
}

#[test]
fn test_coupon_type_cash() {
    let json = include_str!("examples/coupon_type_cash.example.json");
    test_roundtrip::<CashflowEnvelope<CouponTypePayload>>(json);
}

#[test]
fn test_coupon_type_split() {
    let json = include_str!("examples/coupon_type_split.example.json");
    test_roundtrip::<CashflowEnvelope<CouponTypePayload>>(json);
}

#[test]
fn test_fixed_coupon_spec() {
    let json = include_str!("examples/fixed_coupon_spec.example.json");

    // Verify deserialized values match market-standard conventions
    let envelope: CashflowEnvelope<FixedCouponSpecPayload> =
        serde_json::from_str(json).expect("Failed to deserialize");
    let spec = &envelope.payload.fixed_coupon_spec;

    // Rate should be 4.25% (expressed as 0.0425)
    assert!(
        (spec.rate.to_f64().unwrap_or(0.0) - 0.0425).abs() < 1e-10,
        "Fixed coupon rate should be 4.25%, got {}",
        spec.rate
    );

    // Day count should be 30/360 (standard for USD corporate bonds)
    assert_eq!(
        spec.schedule.dc,
        DayCount::Thirty360,
        "Fixed coupon day count should be 30/360"
    );

    // Business day convention should be Modified Following (market standard)
    assert_eq!(
        spec.schedule.bdc,
        BusinessDayConvention::ModifiedFollowing,
        "Fixed coupon BDC should be Modified Following"
    );

    // Coupon type should be Cash
    assert!(
        matches!(spec.coupon_type, CouponType::Cash),
        "Coupon type should be Cash"
    );

    test_roundtrip::<CashflowEnvelope<FixedCouponSpecPayload>>(json);

    let mut adjusted: serde_json::Value = serde_json::from_str(json).expect("valid example");
    adjusted["fixed_coupon_spec"]["adjust_accrual_dates"] = json!(true);
    let adjusted: CashflowEnvelope<FixedCouponSpecPayload> =
        serde_json::from_value(adjusted).expect("adjusted schedule deserializes");
    assert!(
        adjusted
            .payload
            .fixed_coupon_spec
            .schedule
            .adjust_accrual_dates
    );
    assert_eq!(
        serde_json::to_value(adjusted).expect("adjusted schedule serializes")["fixed_coupon_spec"]
            ["adjust_accrual_dates"],
        json!(true)
    );
}

#[test]
fn test_floating_rate_spec() {
    let json = include_str!("examples/floating_rate_spec.example.json");
    let envelope: CashflowEnvelope<FloatingRateSpecPayload> =
        serde_json::from_str(json).expect("Failed to deserialize");
    let reserialized = serde_json::to_string(&envelope).expect("Failed to serialize");

    let mut expected_value: serde_json::Value =
        serde_json::from_str(json).expect("Failed to parse original JSON");
    canonicalize_floating_rate_keys(&mut expected_value);
    let reserialized_value: serde_json::Value =
        serde_json::from_str(&reserialized).expect("Failed to parse reserialized JSON");

    assert_eq!(
        expected_value, reserialized_value,
        "Roundtrip mismatch for envelope"
    );
}

#[test]
fn test_floating_coupon_spec() {
    let json = include_str!("examples/floating_coupon_spec.example.json");

    // Verify deserialized values match market-standard conventions
    let envelope: CashflowEnvelope<FloatingCouponSpecPayload> =
        serde_json::from_str(json).expect("Failed to deserialize");
    let spec = &envelope.payload.floating_coupon_spec;

    // Spread should be 150 bps
    assert!(
        (spec.rate_spec.spread_bp.to_f64().unwrap_or(0.0) - 150.0).abs() < 1e-10,
        "Floating rate spread should be 150 bps, got {}",
        spec.rate_spec.spread_bp
    );

    // Day count should be Act/360 (standard for EUR EURIBOR)
    assert_eq!(
        spec.schedule.dc,
        DayCount::Act360,
        "Floating rate day count should be Act/360"
    );

    // Reset lag should be T-2 (standard for EURIBOR)
    assert_eq!(
        spec.rate_spec.reset_lag_days, 2,
        "Reset lag should be 2 days (T-2)"
    );

    // Gearing should be 1.0 (no leverage)
    assert!(
        (spec.rate_spec.gearing.to_f64().unwrap_or(0.0) - 1.0).abs() < 1e-10,
        "Gearing should be 1.0"
    );

    let reserialized = serde_json::to_string(&envelope).expect("Failed to serialize");

    let mut expected_value: serde_json::Value =
        serde_json::from_str(json).expect("Failed to parse original JSON");
    canonicalize_floating_rate_keys(&mut expected_value);
    let reserialized_value: serde_json::Value =
        serde_json::from_str(&reserialized).expect("Failed to parse reserialized JSON");

    assert_eq!(
        expected_value, reserialized_value,
        "Roundtrip mismatch for envelope"
    );

    let mut adjusted: serde_json::Value = serde_json::from_str(json).expect("valid example");
    adjusted["floating_coupon_spec"]["adjust_accrual_dates"] = json!(true);
    let adjusted: CashflowEnvelope<FloatingCouponSpecPayload> =
        serde_json::from_value(adjusted).expect("adjusted schedule deserializes");
    assert!(
        adjusted
            .payload
            .floating_coupon_spec
            .schedule
            .adjust_accrual_dates
    );
    assert_eq!(
        serde_json::to_value(adjusted).expect("adjusted schedule serializes")
            ["floating_coupon_spec"]["adjust_accrual_dates"],
        json!(true)
    );
}

#[test]
fn test_step_up_coupon_preserves_adjusted_accruals() {
    let spec = StepUpCouponSpec {
        coupon_type: CouponType::Cash,
        initial_rate: Decimal::new(5, 2),
        step_schedule: Vec::<(Date, Decimal)>::new(),
        schedule: ScheduleParams {
            adjust_accrual_dates: true,
            ..ScheduleParams::semiannual_30360()
        },
    };

    let encoded = serde_json::to_value(&spec).expect("step-up schedule serializes");
    assert_eq!(encoded["adjust_accrual_dates"], json!(true));
    let decoded: StepUpCouponSpec =
        serde_json::from_value(encoded).expect("step-up schedule deserializes");
    assert!(decoded.schedule.adjust_accrual_dates);
}

#[test]
fn test_prepayment_model_constant() {
    let json = include_str!("examples/prepayment_model_constant.example.json");
    test_roundtrip::<CashflowEnvelope<PrepaymentModelSpecPayload>>(json);
}

#[test]
fn test_prepayment_model_psa_100() {
    let json = include_str!("examples/prepayment_model_psa_100.example.json");

    // Verify deserialized values match PSA standard
    let envelope: CashflowEnvelope<PrepaymentModelSpecPayload> =
        serde_json::from_str(json).expect("Failed to deserialize");
    let spec = &envelope.payload.prepayment_model_spec;

    // CPR should be 6% (100% PSA terminal rate)
    assert!(
        (spec.cpr - 0.06).abs() < 1e-10,
        "PSA 100 CPR should be 6%, got {}",
        spec.cpr
    );

    // Should have PSA curve with 1.0 multiplier
    match &spec.curve {
        Some(PrepaymentCurve::Psa { speed_multiplier }) => {
            assert!(
                (*speed_multiplier - 1.0).abs() < 1e-10,
                "PSA 100 speed multiplier should be 1.0, got {}",
                speed_multiplier
            );
        }
        _ => panic!("PSA 100 should have Psa curve variant"),
    }

    test_roundtrip::<CashflowEnvelope<PrepaymentModelSpecPayload>>(json);
}

#[test]
fn test_default_model_constant() {
    let json = include_str!("examples/default_model_constant.example.json");
    test_roundtrip::<CashflowEnvelope<DefaultModelSpecPayload>>(json);
}

#[test]
fn test_default_model_sda_100() {
    let json = include_str!("examples/default_model_sda_100.example.json");
    test_roundtrip::<CashflowEnvelope<DefaultModelSpecPayload>>(json);
}

#[test]
fn test_default_event() {
    let json = include_str!("examples/default_event.example.json");
    test_roundtrip::<CashflowEnvelope<DefaultEventPayload>>(json);
}

#[test]
fn test_recovery_model_standard() {
    let json = include_str!("examples/recovery_model_standard.example.json");
    test_roundtrip::<CashflowEnvelope<RecoveryModelSpecPayload>>(json);
}

#[test]
fn test_fee_spec_fixed() {
    let json = include_str!("examples/fee_spec_fixed.example.json");
    test_roundtrip::<CashflowEnvelope<FeeSpecPayload>>(json);
}

#[test]
fn test_fee_spec_periodic_bps() {
    let json = include_str!("examples/fee_spec_periodic_bps.example.json");
    test_roundtrip::<CashflowEnvelope<FeeSpecPayload>>(json);
}

#[test]
fn test_fee_tier() {
    let json = include_str!("examples/fee_tier.example.json");
    test_roundtrip::<CashflowEnvelope<FeeTierPayload>>(json);
}

#[test]
fn test_schedule_params_usd_act360() {
    let json = include_str!("examples/schedule_params_usd_act360.example.json");

    // Verify deserialized values match USD market conventions
    let envelope: CashflowEnvelope<ScheduleParamsPayload> =
        serde_json::from_str(json).expect("Failed to deserialize");
    let spec = &envelope.payload.schedule_params;

    // Day count should be Act/360 (USD money market convention)
    assert_eq!(
        spec.dc,
        DayCount::Act360,
        "USD standard day count should be Act/360"
    );

    // Business day convention should be Modified Following
    assert_eq!(
        spec.bdc,
        BusinessDayConvention::ModifiedFollowing,
        "USD standard BDC should be Modified Following"
    );

    // Calendar should be USD
    assert_eq!(spec.calendar_id, "USD", "Calendar should be USD");

    test_roundtrip::<CashflowEnvelope<ScheduleParamsPayload>>(json);
}

/// JSON bridge end-to-end with historical fixings: a `market_json` carrying a
/// `FIXING:{index}` ScalarTimeSeries lets a seasoned floating schedule build,
/// with the first (pre-curve-base) coupon priced off the realized fixing.
#[test]
fn test_json_bridge_seasoned_floating_schedule_with_fixing_series() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount as CoreDayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use time::macros::date;

    // Curve based mid-life (2025-06-15); the Jan-15 and Apr-15 resets are
    // realized history carried by the FIXING:USD-SOFR-3M series.
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(date!(2025 - 06 - 15))
        .day_count(CoreDayCount::Act360)
        .knots([(0.0, 0.03), (5.0, 0.03)])
        .build()
        .expect("flat forward curve builds");
    let series = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![
            (date!(2025 - 01 - 15), 0.040),
            (date!(2025 - 04 - 15), 0.042),
        ],
        None,
    )
    .expect("fixing series builds");
    let market = MarketContext::new().insert(fwd).insert_series(series);
    let market_json = serde_json::to_string(&market).expect("market context serializes");
    assert!(
        market_json.contains("FIXING:USD-SOFR-3M"),
        "market JSON must carry the fixing series with no schema change"
    );

    let spec = json!({
        "notional": {
            "initial": {"amount": "1000000", "currency": "USD"},
            "amort": "None"
        },
        "issue": "2025-01-15",
        "maturity": "2026-01-15",
        "coupon_program": [{
            "kind": "floating",
            "spec": {
              "coupon_type": "Cash",
              "rate_spec": {
                "index_id": "USD-SOFR-3M",
                "spread_bp": "100",
                "reset_freq": {"count": 3, "unit": "months"},
                "reset_lag_days": 0
              },
              "freq": {"count": 3, "unit": "months"},
              "dc": "Act360",
              "bdc": "following",
              "calendar_id": "weekends_only",
              "stub": "None",
              "end_of_month": false,
              "payment_lag_days": 0
            }
        }]
    });

    let schedule_json = finstack_quant_cashflows::build_cashflow_schedule_json(
        &spec.to_string(),
        Some(&market_json),
    )
    .expect("seasoned floating schedule builds from JSON with fixing series");
    let schedule: finstack_quant_cashflows::builder::CashFlowSchedule =
        serde_json::from_str(&schedule_json).expect("schedule JSON deserializes");

    let first_coupon = schedule
        .flows
        .iter()
        .find(|flow| {
            matches!(
                flow.kind,
                finstack_quant_cashflows::primitives::CFKind::FloatReset
            )
        })
        .expect("schedule has a floating coupon");

    // First coupon (accrues 2025-01-15 -> 2025-04-15): 4.0% fixing + 100 bp
    // spread = 5.0% on $1M, Act/360 over 90 days.
    let rate = first_coupon.rate.expect("floating coupon carries a rate");
    assert!(
        (rate - 0.05).abs() < 1e-10,
        "first coupon should price off the historical fixing + spread: {rate}"
    );
    let expected_amount = 1_000_000.0 * 0.05 * (90.0 / 360.0);
    assert!(
        (first_coupon.amount.amount() - expected_amount).abs() < 1e-6,
        "first coupon amount should be {expected_amount}, got {}",
        first_coupon.amount.amount()
    );
    assert_eq!(first_coupon.amount.currency(), Currency::USD);
}

#[test]
fn legacy_coupon_arrays_deserialize_but_serialize_canonically() {
    let legacy = json!({
        "notional": {
            "initial": {"amount": "1000000", "currency": "USD"},
            "amort": "None"
        },
        "issue": "2024-01-01",
        "maturity": "2025-01-01",
        "fixed_coupons": [{
            "coupon_type": "Cash",
            "rate": "0.06",
            "freq": {"count": 12, "unit": "months"},
            "dc": "Thirty360",
            "bdc": "following",
            "calendar_id": "weekends_only",
            "stub": "None"
        }]
    });

    let spec: finstack_quant_cashflows::CashflowScheduleBuildSpec =
        serde_json::from_value(legacy).expect("legacy input remains readable");
    let canonical = serde_json::to_value(spec).expect("canonical spec serializes");
    assert!(canonical.get("coupon_program").is_some());
    assert!(canonical.get("fixed_coupons").is_none());
    assert!(canonical.get("floating_coupons").is_none());
}

#[test]
fn canonical_coupon_and_payment_programs_build_nontrivial_schedule() {
    let spec = json!({
        "notional": {
            "initial": {"amount": "1000000", "currency": "USD"},
            "amort": "None"
        },
        "issue": "2024-01-01",
        "maturity": "2026-01-01",
        "coupon_program": [{
            "kind": "step_up",
            "spec": {
                "coupon_type": "Cash",
                "initial_rate": "0.06",
                "step_schedule": [["2025-01-01", "0.07"]],
                "freq": {"count": 12, "unit": "months"},
                "dc": "Thirty360",
                "bdc": "following",
                "calendar_id": "weekends_only",
                "stub": "None"
            }
        }],
        "payment_program": [{
            "kind": "program",
            "steps": [
                {"date": "2025-01-01", "split": "PIK"},
                {"date": "2026-01-01", "split": "Cash"}
            ]
        }]
    });

    let schedule_json =
        finstack_quant_cashflows::build_cashflow_schedule_json(&spec.to_string(), None)
            .expect("canonical programs build");
    let schedule: finstack_quant_cashflows::builder::CashFlowSchedule =
        serde_json::from_str(&schedule_json).expect("schedule deserializes");
    assert!(schedule
        .flows
        .iter()
        .any(|flow| flow.kind == finstack_quant_cashflows::primitives::CFKind::PIK));
}

#[test]
fn every_canonical_coupon_and_payment_variant_round_trips() {
    let fixed = canonical_fixed_coupon("0.04");
    let floating = canonical_floating_coupon("150");
    let schedule = canonical_schedule_params();
    let coupon_variants = vec![
        json!({"kind": "fixed", "spec": fixed}),
        json!({"kind": "floating", "spec": floating}),
        json!({
            "kind": "step_up",
            "spec": {
                "coupon_type": "Cash",
                "initial_rate": "0.04",
                "step_schedule": [["2026-01-01", "0.05"]],
                "freq": {"count": 3, "unit": "months"},
                "dc": "Act360",
                "bdc": "following",
                "calendar_id": "weekends_only",
                "stub": "None",
                "end_of_month": false,
                "payment_lag_days": 0,
                "adjust_accrual_dates": false
            }
        }),
        json!({
            "kind": "fixed_window",
            "start": "2025-01-01",
            "end": "2026-01-01",
            "spec": canonical_fixed_coupon("0.04")
        }),
        json!({
            "kind": "floating_window",
            "start": "2026-01-01",
            "end": "2027-01-01",
            "spec": canonical_floating_coupon("150")
        }),
        json!({
            "kind": "fixed_to_float",
            "switch": "2026-01-01",
            "fixed": {"rate": "0.04", "schedule": schedule},
            "floating": canonical_floating_coupon("150"),
            "fixed_split": "Cash"
        }),
        json!({
            "kind": "fixed_rate_program",
            "steps": [{"date": "2026-01-01", "rate": "0.04"}],
            "schedule": canonical_schedule_params(),
            "default_split": "Cash"
        }),
        json!({
            "kind": "floating_margin_program",
            "steps": [{"date": "2026-01-01", "rate": "175"}],
            "base": canonical_floating_coupon("150")
        }),
    ];

    for value in coupon_variants {
        let kind = value["kind"].as_str().expect("variant kind").to_string();
        let parsed: finstack_quant_cashflows::CouponLegSpec =
            serde_json::from_value(value).unwrap_or_else(|err| panic!("parse {kind}: {err}"));
        let canonical =
            serde_json::to_value(&parsed).unwrap_or_else(|err| panic!("serialize {kind}: {err}"));
        assert_eq!(canonical["kind"], kind);
        assert!(canonical.get("fixed_coupons").is_none());
        assert!(canonical.get("floating_coupons").is_none());
        serde_json::from_value::<finstack_quant_cashflows::CouponLegSpec>(canonical)
            .unwrap_or_else(|err| panic!("round-trip {kind}: {err}"));
    }

    for value in [
        json!({
            "kind": "window",
            "start": "2025-01-01",
            "end": "2026-01-01",
            "split": "PIK"
        }),
        json!({
            "kind": "program",
            "steps": [
                {"date": "2026-01-01", "split": "PIK"},
                {"date": "2027-01-01", "split": "Cash"}
            ]
        }),
    ] {
        let kind = value["kind"].as_str().expect("variant kind").to_string();
        let parsed: finstack_quant_cashflows::PaymentProgramSpec =
            serde_json::from_value(value).unwrap_or_else(|err| panic!("parse {kind}: {err}"));
        let canonical =
            serde_json::to_value(&parsed).unwrap_or_else(|err| panic!("serialize {kind}: {err}"));
        assert_eq!(canonical["kind"], kind);
        serde_json::from_value::<finstack_quant_cashflows::PaymentProgramSpec>(canonical)
            .unwrap_or_else(|err| panic!("round-trip {kind}: {err}"));
    }
}

#[test]
fn every_canonical_coupon_variant_dispatches_to_the_builder() {
    let programs = vec![
        (
            "fixed",
            vec![json!({"kind": "fixed", "spec": canonical_fixed_coupon("0.04")})],
        ),
        (
            "floating",
            vec![json!({
                "kind": "floating",
                "spec": canonical_floating_coupon("150")
            })],
        ),
        (
            "step_up",
            vec![json!({
                "kind": "step_up",
                "spec": {
                    "coupon_type": "Cash",
                    "initial_rate": "0.04",
                    "step_schedule": [["2026-01-01", "0.05"]],
                    "freq": {"count": 3, "unit": "months"},
                    "dc": "Act360",
                    "bdc": "following",
                    "calendar_id": "weekends_only",
                    "stub": "None"
                }
            })],
        ),
        (
            "explicit_windows",
            vec![
                json!({
                    "kind": "fixed_window",
                    "start": "2025-01-01",
                    "end": "2026-01-01",
                    "spec": canonical_fixed_coupon("0.04")
                }),
                json!({
                    "kind": "floating_window",
                    "start": "2026-01-01",
                    "end": "2027-01-01",
                    "spec": canonical_floating_coupon("150")
                }),
            ],
        ),
        (
            "fixed_to_float",
            vec![json!({
                "kind": "fixed_to_float",
                "switch": "2026-01-01",
                "fixed": {"rate": "0.04", "schedule": canonical_schedule_params()},
                "floating": canonical_floating_coupon("150"),
                "fixed_split": "Cash"
            })],
        ),
        (
            "fixed_rate_program",
            vec![json!({
                "kind": "fixed_rate_program",
                "steps": [{"date": "2026-01-01", "rate": "0.04"}],
                "schedule": canonical_schedule_params(),
                "default_split": "Cash"
            })],
        ),
        (
            "floating_margin_program",
            vec![json!({
                "kind": "floating_margin_program",
                "steps": [{"date": "2026-01-01", "rate": "175"}],
                "base": canonical_floating_coupon("150")
            })],
        ),
    ];

    for (name, coupon_program) in programs {
        let spec = canonical_build_spec(coupon_program, Vec::new());
        let schedule =
            finstack_quant_cashflows::build_cashflow_schedule_json(&spec.to_string(), None)
                .unwrap_or_else(|err| panic!("build {name}: {err}"));
        let schedule: finstack_quant_cashflows::builder::CashFlowSchedule =
            serde_json::from_str(&schedule).unwrap_or_else(|err| panic!("parse {name}: {err}"));
        assert!(schedule.flows.len() > 2, "{name} emitted no coupons");
    }
}

#[test]
fn canonical_payment_window_dispatches_and_overlap_errors() {
    let fixed = vec![json!({
        "kind": "fixed",
        "spec": canonical_fixed_coupon("0.04")
    })];
    let valid = canonical_build_spec(
        fixed.clone(),
        vec![json!({
            "kind": "window",
            "start": "2025-01-01",
            "end": "2026-01-01",
            "split": "PIK"
        })],
    );
    let schedule = finstack_quant_cashflows::build_cashflow_schedule_json(&valid.to_string(), None)
        .expect("payment window builds");
    assert!(schedule.contains("\"kind\":\"PIK\""));

    let overlapping = canonical_build_spec(
        fixed,
        vec![
            json!({
                "kind": "window",
                "start": "2025-01-01",
                "end": "2026-06-01",
                "split": "PIK"
            }),
            json!({
                "kind": "window",
                "start": "2026-01-01",
                "end": "2027-01-01",
                "split": "Cash"
            }),
        ],
    );
    let error =
        finstack_quant_cashflows::build_cashflow_schedule_json(&overlapping.to_string(), None)
            .expect_err("overlapping payment windows fail");
    assert!(error.to_string().contains("overlapping payment windows"));
}

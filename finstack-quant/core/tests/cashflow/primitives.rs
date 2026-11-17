//! Tests for CashFlow struct primitives and validation.
//!
//! These tests verify:
//! - CashFlow construction and field access
//! - Input validation (NaN, Infinity, invalid dates)
//! - CFKind variants and behavior
//!
//! # Reference
//!
//! - ISDA 2006 Definitions (cashflow semantics)

use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowAccrual, CashFlowParams};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::money::Money;
use time::Month;

// Test Helpers

/// Helper to create dates
fn d(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
}

/// Helper to create valid base cashflow inputs for testing
fn valid_params() -> CashFlowParams {
    CashFlowParams {
        date: d(2025, 1, 15),
        reset_date: None,
        amount: Money::new(100.0, Currency::USD).expect("valid money fixture"),
        kind: CFKind::Fixed,
        accrual_factor: 0.5,
        rate: Some(0.05),
        accrual: None,
        principal_delta: None,
        principal_date: None,
    }
}

// Construction and Field Tests

#[test]
fn cashflow_fixed_construction() {
    let cf = CashFlowParams {
        date: d(2025, 1, 15),
        reset_date: None,
        amount: Money::new(1000.0, Currency::USD).expect("valid money fixture"),
        kind: CFKind::Fixed,
        accrual_factor: 0.25,
        rate: Some(0.05),
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };

    let cf = cf.build().expect("valid cashflow");
    assert_eq!(cf.get_date(), d(2025, 1, 15));
    assert_eq!(cf.get_kind(), CFKind::Fixed);
    assert!(cf.get_reset_date().is_none());
    assert!(cf.validate().is_ok());
}

#[test]
fn cashflow_floating_construction() {
    let payment = d(2025, 1, 10);
    let reset = d(2025, 1, 5);
    let amount = Money::new(50.0, Currency::USD).expect("valid money fixture");

    let cf = CashFlowParams {
        date: payment,
        reset_date: Some(reset),
        amount,
        kind: CFKind::Floating,
        accrual_factor: 0.25,
        rate: None,
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };

    let cf = cf.build().expect("valid cashflow");
    assert_eq!(cf.get_kind(), CFKind::Floating);
    assert_eq!(cf.get_reset_date(), Some(reset));
    assert!(cf.validate().is_ok());
}

// Amount Validation Tests

// Money::new validates that amounts are finite and returns an error on NaN/Infinity.
// These tests verify that behavior at the Money level.

#[test]
fn money_new_rejects_nan() {
    assert!(Money::new(f64::NAN, Currency::USD).is_err());
}

#[test]
fn money_new_rejects_positive_infinity() {
    assert!(Money::new(f64::INFINITY, Currency::USD).is_err());
}

#[test]
fn money_new_rejects_negative_infinity() {
    assert!(Money::new(f64::NEG_INFINITY, Currency::USD).is_err());
}

#[test]
fn cashflow_accepts_zero_amount() {
    // Zero amounts are legitimate (e.g. a floating coupon floored at 0% in a
    // negative-rate environment); previously rejected — fixed per the
    // convention.
    let cf = CashFlowParams {
        amount: Money::new(0.0, Currency::USD).expect("valid money fixture"),
        ..valid_params()
    };
    assert!(cf.build().is_ok(), "Zero amount should be accepted");
}

#[test]
fn cashflow_accepts_negative_amount() {
    // Negative amounts are valid (represent outflows)
    let cf = CashFlowParams {
        amount: Money::new(-100.0, Currency::USD).expect("valid money fixture"),
        ..valid_params()
    };
    assert!(cf.build().is_ok(), "Negative amount should be accepted");
}

#[test]
fn cashflow_accepts_large_amount() {
    let cf = CashFlowParams {
        amount: Money::new(1e15, Currency::USD).expect("valid money fixture"),
        ..valid_params()
    };
    assert!(cf.build().is_ok(), "Large amount should be accepted");
}

#[test]
fn cashflow_accepts_small_but_nonzero_amount() {
    // Note: Money rounds to currency decimal places (2 for USD).
    // Very small amounts like 1e-10 round to 0.0 (now still valid).
    // 0.01 is the smallest representable non-zero USD amount.
    let cf = CashFlowParams {
        amount: Money::new(0.01, Currency::USD).expect("valid money fixture"),
        ..valid_params()
    };
    assert!(
        cf.build().is_ok(),
        "Small but non-zero amount should be accepted"
    );
}

// Accrual Factor Validation Tests

#[test]
fn cashflow_rejects_nan_accrual_factor() {
    let cf = CashFlowParams {
        accrual_factor: f64::NAN,
        ..valid_params()
    };
    assert!(cf.build().is_err(), "NaN accrual factor should be rejected");
}

#[test]
fn cashflow_rejects_positive_infinity_accrual_factor() {
    let cf = CashFlowParams {
        accrual_factor: f64::INFINITY,
        ..valid_params()
    };
    assert!(
        cf.build().is_err(),
        "Positive infinity accrual factor should be rejected"
    );
}

#[test]
fn cashflow_rejects_negative_infinity_accrual_factor() {
    let cf = CashFlowParams {
        accrual_factor: f64::NEG_INFINITY,
        ..valid_params()
    };
    assert!(
        cf.build().is_err(),
        "Negative infinity accrual factor should be rejected"
    );
}

#[test]
fn cashflow_accepts_zero_accrual_factor() {
    // Zero accrual factor is valid (e.g., notional flows)
    let cf = CashFlowParams {
        accrual_factor: 0.0,
        ..valid_params()
    };
    assert!(cf.build().is_ok(), "Zero accrual factor should be accepted");
}

#[test]
fn cashflow_rejects_negative_accrual_factor() {
    // Negative accrual factor is invalid
    let cf = CashFlowParams {
        accrual_factor: -0.25,
        ..valid_params()
    };
    assert!(
        cf.build().is_err(),
        "Negative accrual factor should be rejected"
    );
}

// Rate Validation Tests

#[test]
fn cashflow_rejects_nan_rate() {
    let cf = CashFlowParams {
        rate: Some(f64::NAN),
        ..valid_params()
    };
    assert!(cf.build().is_err(), "NaN rate should be rejected");
}

#[test]
fn cashflow_rejects_positive_infinity_rate() {
    let cf = CashFlowParams {
        rate: Some(f64::INFINITY),
        ..valid_params()
    };
    assert!(
        cf.build().is_err(),
        "Positive infinity rate should be rejected"
    );
}

#[test]
fn cashflow_rejects_negative_infinity_rate() {
    let cf = CashFlowParams {
        rate: Some(f64::NEG_INFINITY),
        ..valid_params()
    };
    assert!(
        cf.build().is_err(),
        "Negative infinity rate should be rejected"
    );
}

#[test]
fn cashflow_accepts_none_rate() {
    let cf = CashFlowParams {
        rate: None,
        ..valid_params()
    };
    assert!(cf.build().is_ok(), "None rate should be accepted");
}

#[test]
fn cashflow_accepts_zero_rate() {
    let cf = CashFlowParams {
        rate: Some(0.0),
        ..valid_params()
    };
    assert!(cf.build().is_ok(), "Zero rate should be accepted");
}

#[test]
fn cashflow_accepts_negative_rate() {
    // Negative rates are valid (negative interest rate environment)
    let cf = CashFlowParams {
        rate: Some(-0.005),
        ..valid_params()
    };
    assert!(cf.build().is_ok(), "Negative rate should be accepted");
}

#[test]
fn cashflow_accepts_high_rate() {
    // High rates are valid (distressed debt, VC scenarios)
    let cf = CashFlowParams {
        rate: Some(2.5), // 250%
        ..valid_params()
    };
    assert!(cf.build().is_ok(), "High rate should be accepted");
}

// Reset Date Validation Tests

#[test]
fn cashflow_rejects_reset_date_after_payment() {
    let payment = d(2025, 1, 10);
    let reset = d(2025, 1, 15); // After payment date

    let cf = CashFlowParams {
        date: payment,
        reset_date: Some(reset),
        amount: Money::new(100.0, Currency::USD).expect("valid money fixture"),
        kind: CFKind::Floating,
        accrual_factor: 0.25,
        rate: None,
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert!(
        cf.build().is_err(),
        "Reset date after payment date should be rejected"
    );
}

#[test]
fn cashflow_accepts_reset_date_before_payment() {
    let payment = d(2025, 1, 15);
    let reset = d(2025, 1, 10); // Before payment date

    let cf = CashFlowParams {
        date: payment,
        reset_date: Some(reset),
        amount: Money::new(100.0, Currency::USD).expect("valid money fixture"),
        kind: CFKind::Floating,
        accrual_factor: 0.25,
        rate: None,
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert!(
        cf.build().is_ok(),
        "Reset date before payment date should be accepted"
    );
}

#[test]
fn cashflow_accepts_reset_date_equal_to_payment() {
    let date = d(2025, 1, 15);

    let cf = CashFlowParams {
        date,
        reset_date: Some(date), // Same as payment date
        amount: Money::new(100.0, Currency::USD).expect("valid money fixture"),
        kind: CFKind::Floating,
        accrual_factor: 0.25,
        rate: None,
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert!(
        cf.build().is_ok(),
        "Reset date equal to payment date should be accepted"
    );
}

#[test]
fn cashflow_accepts_no_reset_date() {
    let cf = CashFlowParams {
        date: d(2025, 1, 15),
        reset_date: None,
        amount: Money::new(100.0, Currency::USD).expect("valid money fixture"),
        kind: CFKind::Fixed,
        accrual_factor: 0.25,
        rate: Some(0.05),
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert!(
        cf.build().is_ok(),
        "No reset date should be accepted for fixed cashflows"
    );
}

// Combined Validation Tests

#[test]
fn cashflow_valid_with_all_fields_populated() {
    let cf = CashFlowParams {
        date: d(2025, 6, 15),
        reset_date: Some(d(2025, 6, 1)),
        amount: Money::new(25_000.0, Currency::EUR).expect("valid money fixture"),
        kind: CFKind::Floating,
        accrual_factor: 0.25,
        rate: Some(0.0325),
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert!(
        cf.build().is_ok(),
        "Valid cashflow with all fields should be accepted"
    );
}

#[test]
fn cashflow_multiple_invalid_fields_first_error_wins() {
    // Multiple invalid fields: accrual_factor and rate are invalid
    // Validation should fail on the first non-amount check (accrual_factor)
    // Money rejects non-finite input at construction, before cashflow validation.
    let cf = CashFlowParams {
        date: d(2025, 1, 15),
        reset_date: Some(d(2025, 1, 20)), // After payment - invalid
        amount: Money::new(100.0, Currency::USD).expect("valid money fixture"),
        kind: CFKind::Fixed,
        accrual_factor: f64::INFINITY, // Invalid
        rate: Some(f64::NAN),          // Also invalid
        accrual: None,
        principal_delta: None,
        principal_date: None,
    };
    assert!(
        cf.build().is_err(),
        "Cashflow with multiple invalid fields should be rejected"
    );
}

#[test]
fn constructor_defaults_accrual_to_none() {
    let flow = CashFlowParams::new(
        d(2025, 6, 15),
        None,
        Money::new(25_000.0, Currency::EUR).expect("valid money fixture"),
        CFKind::Fixed,
        0.25,
        Some(0.0325),
    )
    .build()
    .expect("valid cashflow");
    assert!(flow.get_accrual().is_none());
}

#[test]
fn json_round_trips_without_optional_accrual_field() {
    let flow = valid_params().build().expect("valid cashflow");
    let payload = serde_json::to_value(flow).unwrap();
    assert!(payload.get("accrual").is_none());

    let decoded: CashFlow = serde_json::from_value(payload.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), payload);
}

#[test]
fn accrual_metadata_round_trips() {
    let accrual = CashFlowAccrual::new(
        d(2025, 3, 15),
        d(2025, 6, 15),
        DayCount::Act360,
        Some(0.031),
        finstack_quant_core::dates::DayCountContextState::try_new(
            None,
            None,
            None,
            Some((d(2025, 3, 15), d(2025, 9, 15))),
            Some(true),
        )
        .expect("valid context"),
    )
    .expect("valid accrual");
    let flow = CashFlowParams::new(
        d(2025, 6, 15),
        Some(d(2025, 3, 13)),
        Money::new(12_500.0, Currency::USD).expect("valid money fixture"),
        CFKind::Floating,
        0.25,
        Some(0.036),
    )
    .with_accrual(accrual.clone())
    .build()
    .expect("valid cashflow");

    let encoded = serde_json::to_string(&flow).unwrap();
    let decoded: CashFlow = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, flow);
    assert_eq!(decoded.get_accrual(), &Some(accrual));
}

#[test]
fn monetary_representation_is_validated_at_construction() {
    for amount in [
        rust_decimal::Decimal::MIN,
        rust_decimal::Decimal::ZERO,
        rust_decimal::Decimal::MAX,
    ] {
        let flow = CashFlowParams::new(
            d(2025, 6, 15),
            None,
            Money::from_decimal(amount, Currency::USD),
            CFKind::Fixed,
            0.25,
            Some(0.03),
        )
        .build()
        .expect("valid cashflow");
        assert!(flow.validate().is_ok());
        assert_eq!(flow.get_amount().amount_decimal(), amount);
    }
    for amount in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(Money::new(amount, Currency::USD).is_err());
    }
}

#[test]
fn accrual_validate_enforces_day_count_context_without_eager_non_bus252_calendar_lookup() {
    use finstack_quant_core::dates::{DayCountContextState, Tenor};

    let start = d(2025, 3, 15);
    let end = d(2025, 6, 15);

    // Non-Bus252 conventions defer calendar_id lookup to evaluation time.
    let lazy_act365f = CashFlowAccrual::new(
        start,
        end,
        DayCount::Act365F,
        None,
        DayCountContextState::try_new(Some("not_a_calendar".into()), None, None, None, None)
            .expect("state"),
    );
    assert!(lazy_act365f.is_ok());

    // Bus252 requires calendar_id present and valid in the registry.
    assert!(
        CashFlowAccrual::new(
            start,
            end,
            DayCount::Bus252,
            None,
            DayCountContextState::default(),
        )
        .is_err()
    );
    assert!(
        CashFlowAccrual::new(
            start,
            end,
            DayCount::Bus252,
            None,
            DayCountContextState::try_new(Some("not_a_calendar".into()), None, None, None, None)
                .expect("state"),
        )
        .is_err()
    );
    assert!(
        CashFlowAccrual::new(
            start,
            end,
            DayCount::Bus252,
            None,
            DayCountContextState::try_new(Some("usny".into()), None, None, None, None)
                .expect("state"),
        )
        .is_ok()
    );

    // ActActIsma requires frequency, nominal-grid coupon_period, and overlapping period,
    // while admitting multi-period long stubs alongside one regular reference interval.
    assert!(
        CashFlowAccrual::new(
            start,
            end,
            DayCount::ActActIsma,
            None,
            DayCountContextState::try_new(None, None, None, Some((start, end)), None)
                .expect("state"),
        )
        .is_err()
    );
    assert!(
        CashFlowAccrual::new(
            start,
            end,
            DayCount::ActActIsma,
            None,
            DayCountContextState::try_new(None, Some(Tenor::quarterly()), None, None, None)
                .expect("state"),
        )
        .is_err()
    );
    assert!(
        CashFlowAccrual::new(
            start,
            end,
            DayCount::ActActIsma,
            None,
            DayCountContextState::try_new(
                None,
                Some(Tenor::quarterly()),
                None,
                Some((d(2025, 4, 1), end)),
                None,
            )
            .expect("state"),
        )
        .is_err()
    );
    assert!(
        CashFlowAccrual::new(
            start,
            end,
            DayCount::ActActIsma,
            None,
            DayCountContextState::try_new(
                None,
                Some(Tenor::quarterly()),
                None,
                Some((d(2025, 6, 15), d(2025, 9, 15))),
                None,
            )
            .expect("state"),
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<CashFlowAccrual>(
            r#"{
                "start": "2025-03-15",
                "end": "2025-06-15",
                "day_count": "act_act_isma",
                "context": {
                    "frequency": { "count": 3, "unit": "months" },
                    "coupon_period": ["2025-06-15", "2025-09-15"]
                }
            }"#,
        )
        .is_err()
    );
    assert!(
        CashFlowAccrual::new(
            start,
            end,
            DayCount::ActActIsma,
            None,
            DayCountContextState::try_new(
                None,
                Some(Tenor::quarterly()),
                None,
                Some((start, end)),
                None,
            )
            .expect("state"),
        )
        .is_ok()
    );
    assert!(
        CashFlowAccrual::new(
            d(2025, 1, 15),
            d(2025, 10, 15),
            DayCount::ActActIsma,
            None,
            DayCountContextState::try_new(
                None,
                Some(Tenor::semi_annual()),
                None,
                Some((d(2025, 1, 15), d(2025, 7, 15))),
                None,
            )
            .expect("state"),
        )
        .is_ok()
    );

    // Act365L requires enclosing coupon_period and slice containment.
    assert!(
        CashFlowAccrual::new(
            start,
            end,
            DayCount::Act365L,
            None,
            DayCountContextState::default(),
        )
        .is_err()
    );
    assert!(
        CashFlowAccrual::new(
            start,
            end,
            DayCount::Act365L,
            None,
            DayCountContextState::try_new(None, None, None, Some((start, d(2025, 5, 1))), None)
                .expect("state"),
        )
        .is_err()
    );

    // ThirtyE360Isda requires explicit termination status.
    assert!(
        CashFlowAccrual::new(
            start,
            end,
            DayCount::ThirtyE360Isda,
            None,
            DayCountContextState::default(),
        )
        .is_err()
    );
    assert!(
        CashFlowAccrual::new(
            start,
            end,
            DayCount::ThirtyE360Isda,
            None,
            DayCountContextState::try_new(None, None, None, None, Some(false)).expect("state"),
        )
        .is_ok()
    );
}

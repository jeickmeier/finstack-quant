//! Golden value tests for PSA and SDA credit models.
//!
//! These tests verify that prepayment (PSA) and default (SDA) model implementations
//! produce correct values according to industry standards.
//!
//! # PSA (Public Securities Association) Prepayment Model
//!
//! - 100% PSA: Linear ramp from 0% CPR at month 0 to 6% CPR at month 30, then flat
//! - SMM (Single Monthly Mortality) = 1 - (1 - CPR)^(1/12)
//!
//! # SDA (Standard Default Assumption) Model
//!
//! - 100 SDA (PSA/BMA standard): 0.02%/month linear ramp to 0.60% annual CDR
//!   at month 30, flat plateau months 30-60, linear decline to 0.03% annual
//!   CDR by month 120, constant thereafter
//! - MDR (Monthly Default Rate) follows similar conversion to SMM

use crate::helpers::{FACTOR_TOLERANCE, RATE_TOLERANCE};

// =============================================================================
// PSA Golden Values
// =============================================================================

#[test]
fn psa_smm_golden_values() {
    // PSA (Public Securities Association) Prepayment Model Golden Values
    // 100% PSA ramps to 6% CPR over 30 months, then stays flat
    // SMM = 1 - (1 - CPR)^(1/12)
    use finstack_quant_cashflows::builder::PrepaymentModelSpec;
    use finstack_quant_cashflows::builder::{cpr_to_smm, smm_to_cpr};

    let model = PrepaymentModelSpec::psa_100();

    // Month 0: 0% CPR → 0% SMM
    let smm_0 = model.smm(0).unwrap();
    assert!(
        smm_0.abs() < RATE_TOLERANCE,
        "PSA at month 0 should be 0% SMM, got {}",
        smm_0
    );

    // Month 15: 3% CPR (halfway through ramp) → ~0.2536% SMM
    let smm_15 = model.smm(15).unwrap();
    let cpr_15 = smm_to_cpr(smm_15).expect("valid SMM");
    assert!(
        (cpr_15 - 0.03).abs() < RATE_TOLERANCE,
        "PSA at month 15 should be 3% CPR, got {}",
        cpr_15
    );

    // Month 30: 6% CPR (end of ramp) → ~0.5143% SMM
    let smm_30 = model.smm(30).unwrap();
    let expected_smm_30 = cpr_to_smm(0.06).unwrap();
    assert!(
        (smm_30 - expected_smm_30).abs() < RATE_TOLERANCE,
        "PSA at month 30 should be {} SMM, got {}",
        expected_smm_30,
        smm_30
    );

    // Month 60: Still 6% CPR (flat after ramp)
    let smm_60 = model.smm(60).unwrap();
    assert!(
        (smm_60 - expected_smm_30).abs() < RATE_TOLERANCE,
        "PSA at month 60 should still be {} SMM, got {}",
        expected_smm_30,
        smm_60
    );

    // 150% PSA should be 1.5x the base values
    let model_150 = PrepaymentModelSpec::psa(1.5);
    let smm_30_150 = model_150.smm(30).unwrap();
    let cpr_30_150 = smm_to_cpr(smm_30_150).expect("valid SMM");
    assert!(
        (cpr_30_150 - 0.09).abs() < RATE_TOLERANCE,
        "150% PSA at month 30 should be 9% CPR, got {}",
        cpr_30_150
    );
}

// =============================================================================
// SDA Golden Values
// =============================================================================

#[test]
fn sda_mdr_golden_values() {
    // SDA (Standard Default Assumption) Model Golden Values — PSA/BMA 100 SDA:
    // 0.02%/month ramp to 0.60% CDR at month 30, flat plateau months 30-60,
    // linear decline to 0.03% CDR by month 120, flat thereafter.
    use finstack_quant_cashflows::builder::smm_to_cpr;
    use finstack_quant_cashflows::builder::DefaultModelSpec;

    let model = DefaultModelSpec::sda(1.0);

    // Month 0: 0% CDR
    let mdr_0 = model.mdr(0).unwrap();
    assert!(
        mdr_0.abs() < RATE_TOLERANCE,
        "SDA at month 0 should be 0% MDR, got {}",
        mdr_0
    );

    // Month 15: 0.30% CDR (halfway to peak)
    let mdr_15 = model.mdr(15).unwrap();
    let cdr_15 = smm_to_cpr(mdr_15).expect("valid SMM");
    assert!(
        (cdr_15 - 0.003).abs() < RATE_TOLERANCE,
        "SDA at month 15 should be 0.30% CDR, got {}",
        cdr_15
    );

    // Month 30: 0.60% CDR (peak)
    let mdr_30 = model.mdr(30).unwrap();
    let cdr_30 = smm_to_cpr(mdr_30).expect("valid SMM");
    assert!(
        (cdr_30 - 0.006).abs() < RATE_TOLERANCE,
        "SDA at month 30 should be 0.60% CDR (peak), got {}",
        cdr_30
    );

    // Month 60: still 0.60% CDR (end of plateau)
    let mdr_60 = model.mdr(60).unwrap();
    let cdr_60 = smm_to_cpr(mdr_60).expect("valid SMM");
    assert!(
        (cdr_60 - 0.006).abs() < RATE_TOLERANCE,
        "SDA at month 60 should be 0.60% CDR (plateau), got {}",
        cdr_60
    );

    // Month 90: 0.315% CDR (halfway down the 60-month decline)
    let mdr_90 = model.mdr(90).unwrap();
    let cdr_90 = smm_to_cpr(mdr_90).expect("valid SMM");
    assert!(
        (cdr_90 - 0.00315).abs() < RATE_TOLERANCE,
        "SDA at month 90 should be 0.315% CDR (mid-decline), got {}",
        cdr_90
    );

    // Month 120: 0.03% CDR (terminal)
    let mdr_120 = model.mdr(120).unwrap();
    let cdr_120 = smm_to_cpr(mdr_120).expect("valid SMM");
    assert!(
        (cdr_120 - 0.0003).abs() < RATE_TOLERANCE,
        "SDA at month 120 should be 0.03% CDR (terminal), got {}",
        cdr_120
    );

    // Month 200: still 0.03% CDR (flat after terminal)
    let mdr_200 = model.mdr(200).unwrap();
    let cdr_200 = smm_to_cpr(mdr_200).expect("valid SMM");
    assert!(
        (cdr_200 - 0.0003).abs() < RATE_TOLERANCE,
        "SDA at month 200 should still be 0.03% CDR, got {}",
        cdr_200
    );
}

// =============================================================================
// CPR/SMM Conversion Tests
// =============================================================================

#[test]
fn cpr_smm_conversion_roundtrip_precision() {
    // Test that CPR ↔ SMM conversion maintains precision across range
    // Formula: SMM = 1 - (1 - CPR)^(1/12)
    //          CPR = 1 - (1 - SMM)^12
    use finstack_quant_cashflows::builder::{cpr_to_smm, smm_to_cpr};

    let test_cprs = [0.0, 0.01, 0.03, 0.06, 0.10, 0.15, 0.20, 0.50];

    for &cpr in &test_cprs {
        let smm = cpr_to_smm(cpr).unwrap();
        let cpr_back = smm_to_cpr(smm).expect("valid SMM");

        assert!(
            (cpr - cpr_back).abs() < FACTOR_TOLERANCE,
            "CPR {} roundtrip failed: got {}",
            cpr,
            cpr_back
        );

        // SMM should always be less than CPR (except for 0)
        if cpr > 0.0 {
            assert!(smm < cpr, "SMM ({}) should be less than CPR ({})", smm, cpr);
        }
    }

    // Verify specific golden value: 6% CPR ≈ 0.5143% SMM
    // Using exact calculation: SMM = 1 - (1 - 0.06)^(1/12) ≈ 0.005143
    let smm_6pct = cpr_to_smm(0.06).unwrap();
    let expected_smm = 1.0 - (1.0 - 0.06_f64).powf(1.0 / 12.0);
    assert!(
        (smm_6pct - expected_smm).abs() < FACTOR_TOLERANCE,
        "6% CPR should convert to {} SMM, got {}",
        expected_smm,
        smm_6pct
    );
}

#[test]
fn smm_to_cpr_rejects_invalid_smm() {
    use finstack_quant_cashflows::builder::smm_to_cpr;

    assert!(
        smm_to_cpr(-0.01).is_err(),
        "negative SMM should be rejected"
    );
    assert!(
        smm_to_cpr(1.01).is_err(),
        "SMM above 100% should be rejected"
    );
}

// =============================================================================
// PSA Industry Standard Benchmark Tests
// =============================================================================

#[test]
fn psa_matches_industry_standard_ramp() {
    // Reference: Bond Market Association PSA Standard Prepayment Model
    // 100% PSA: Linear ramp from 0% CPR at month 0 to 6% CPR at month 30
    use finstack_quant_cashflows::builder::smm_to_cpr;
    use finstack_quant_cashflows::builder::PrepaymentModelSpec;

    let model = PrepaymentModelSpec::psa_100();

    // Month 1: 0.2% CPR (1/30 * 6%)
    let cpr_1 = smm_to_cpr(model.smm(1).unwrap()).expect("valid SMM");
    assert!(
        (cpr_1 - 0.002).abs() < RATE_TOLERANCE,
        "PSA month 1 should be 0.2% CPR, got {}",
        cpr_1
    );

    // Month 10: 2.0% CPR (10/30 * 6%)
    let cpr_10 = smm_to_cpr(model.smm(10).unwrap()).expect("valid SMM");
    assert!(
        (cpr_10 - 0.02).abs() < RATE_TOLERANCE,
        "PSA month 10 should be 2.0% CPR, got {}",
        cpr_10
    );

    // Month 20: 4.0% CPR (20/30 * 6%)
    let cpr_20 = smm_to_cpr(model.smm(20).unwrap()).expect("valid SMM");
    assert!(
        (cpr_20 - 0.04).abs() < RATE_TOLERANCE,
        "PSA month 20 should be 4.0% CPR, got {}",
        cpr_20
    );

    // Verify ramp is linear for all months 1-30
    for month in 1..=30 {
        let expected_cpr = (month as f64 / 30.0) * 0.06;
        let actual_cpr = smm_to_cpr(model.smm(month).unwrap()).expect("valid SMM");
        assert!(
            (actual_cpr - expected_cpr).abs() < RATE_TOLERANCE,
            "PSA month {} should be {:.4}% CPR, got {:.4}%",
            month,
            expected_cpr * 100.0,
            actual_cpr * 100.0
        );
    }
}

#[test]
fn psa_multiplier_scales_correctly() {
    // Test that PSA multipliers scale linearly
    use finstack_quant_cashflows::builder::smm_to_cpr;
    use finstack_quant_cashflows::builder::PrepaymentModelSpec;

    // 50% PSA, 100% PSA, 200% PSA at month 30
    let psa_50 = PrepaymentModelSpec::psa(0.5);
    let psa_100 = PrepaymentModelSpec::psa_100();
    let psa_200 = PrepaymentModelSpec::psa(2.0);

    let cpr_50 = smm_to_cpr(psa_50.smm(30).unwrap()).expect("valid SMM");
    let cpr_100 = smm_to_cpr(psa_100.smm(30).unwrap()).expect("valid SMM");
    let cpr_200 = smm_to_cpr(psa_200.smm(30).unwrap()).expect("valid SMM");

    assert!(
        (cpr_50 - 0.03).abs() < RATE_TOLERANCE,
        "50% PSA at month 30 should be 3% CPR, got {}",
        cpr_50
    );
    assert!(
        (cpr_100 - 0.06).abs() < RATE_TOLERANCE,
        "100% PSA at month 30 should be 6% CPR, got {}",
        cpr_100
    );
    assert!(
        (cpr_200 - 0.12).abs() < RATE_TOLERANCE,
        "200% PSA at month 30 should be 12% CPR, got {}",
        cpr_200
    );

    // Verify linear scaling relationship
    assert!(
        (cpr_100 - 2.0 * cpr_50).abs() < RATE_TOLERANCE,
        "100% PSA should be 2x 50% PSA"
    );
    assert!(
        (cpr_200 - 2.0 * cpr_100).abs() < RATE_TOLERANCE,
        "200% PSA should be 2x 100% PSA"
    );
}

#[test]
fn psa_terminal_rate_is_flat() {
    // After month 30, PSA should stay flat at terminal rate
    use finstack_quant_cashflows::builder::smm_to_cpr;
    use finstack_quant_cashflows::builder::PrepaymentModelSpec;

    let model = PrepaymentModelSpec::psa_100();
    let terminal_cpr = 0.06;

    // Test various months after the ramp
    for month in [31, 50, 100, 200, 360] {
        let actual_cpr = smm_to_cpr(model.smm(month).unwrap()).expect("valid SMM");
        assert!(
            (actual_cpr - terminal_cpr).abs() < RATE_TOLERANCE,
            "PSA month {} should be terminal 6% CPR, got {}",
            month,
            actual_cpr
        );
    }
}

// =============================================================================
// SDA Industry Standard Benchmark Tests
// =============================================================================

#[test]
fn sda_matches_industry_standard_curve() {
    // Reference: PSA/BMA Standard Default Assumption curve (100 SDA)
    // 0.02%/month ramp to 0.60% CDR at month 30, flat plateau through month
    // 60, linear decline to 0.03% CDR by month 120, flat thereafter.
    use finstack_quant_cashflows::builder::smm_to_cpr;
    use finstack_quant_cashflows::builder::DefaultModelSpec;

    let model = DefaultModelSpec::sda(1.0);

    // Verify ramp phase (months 1-30): 0.02% CDR per month
    for month in 1..=30 {
        let expected_cdr = (month as f64 / 30.0) * 0.006;
        let actual_cdr = smm_to_cpr(model.mdr(month).unwrap()).expect("valid SMM");
        assert!(
            (actual_cdr - expected_cdr).abs() < RATE_TOLERANCE,
            "SDA month {} (ramp) should be {:.4}% CDR, got {:.4}%",
            month,
            expected_cdr * 100.0,
            actual_cdr * 100.0
        );
    }

    // Verify plateau phase (months 31-60): flat 0.60% CDR
    for month in 31..=60 {
        let actual_cdr = smm_to_cpr(model.mdr(month).unwrap()).expect("valid SMM");
        assert!(
            (actual_cdr - 0.006).abs() < RATE_TOLERANCE,
            "SDA month {} (plateau) should be 0.60% CDR, got {:.4}%",
            month,
            actual_cdr * 100.0
        );
    }

    // Verify decline phase (months 61-120): linear to 0.03% CDR
    for month in 61..=120 {
        let months_past_plateau = (month - 60) as f64;
        let expected_cdr = 0.006 - (months_past_plateau / 60.0) * (0.006 - 0.0003);
        let actual_cdr = smm_to_cpr(model.mdr(month).unwrap()).expect("valid SMM");
        assert!(
            (actual_cdr - expected_cdr).abs() < RATE_TOLERANCE,
            "SDA month {} (decline) should be {:.4}% CDR, got {:.4}%",
            month,
            expected_cdr * 100.0,
            actual_cdr * 100.0
        );
    }

    // Verify terminal phase (month 121+): flat 0.03% CDR
    for month in [121, 200, 360] {
        let actual_cdr = smm_to_cpr(model.mdr(month).unwrap()).expect("valid SMM");
        assert!(
            (actual_cdr - 0.0003).abs() < RATE_TOLERANCE,
            "SDA month {} (terminal) should be 0.03% CDR, got {}",
            month,
            actual_cdr
        );
    }
}

#[test]
fn sda_multiplier_scales_correctly() {
    // Test that SDA multipliers scale linearly
    use finstack_quant_cashflows::builder::smm_to_cpr;
    use finstack_quant_cashflows::builder::DefaultModelSpec;

    let sda_100 = DefaultModelSpec::sda(1.0);
    let sda_200 = DefaultModelSpec::sda(2.0);

    // At peak (month 30)
    let cdr_100_peak = smm_to_cpr(sda_100.mdr(30).unwrap()).expect("valid SMM");
    let cdr_200_peak = smm_to_cpr(sda_200.mdr(30).unwrap()).expect("valid SMM");

    assert!(
        (cdr_100_peak - 0.006).abs() < RATE_TOLERANCE,
        "100% SDA peak should be 0.60% CDR"
    );
    assert!(
        (cdr_200_peak - 0.012).abs() < RATE_TOLERANCE,
        "200% SDA peak should be 1.20% CDR"
    );
    assert!(
        (cdr_200_peak - 2.0 * cdr_100_peak).abs() < RATE_TOLERANCE,
        "200% SDA should be 2x 100% SDA at peak"
    );

    // At terminal (month 150)
    let cdr_100_term = smm_to_cpr(sda_100.mdr(150).unwrap()).expect("valid SMM");
    let cdr_200_term = smm_to_cpr(sda_200.mdr(150).unwrap()).expect("valid SMM");

    assert!(
        (cdr_100_term - 0.0003).abs() < RATE_TOLERANCE,
        "100% SDA terminal should be 0.03% CDR"
    );
    assert!(
        (cdr_200_term - 0.0006).abs() < RATE_TOLERANCE,
        "200% SDA terminal should be 0.06% CDR"
    );
}

// =============================================================================
// CMBS Lockout Boundary Tests
// =============================================================================

#[test]
fn cmbs_lockout_boundary_months() {
    // The lockout boundary is inclusive: seasoning == lockout_months is still
    // locked out (SMM = 0); lockout_months + 1 prepays at the configured CPR.
    use finstack_quant_cashflows::builder::{smm_to_cpr, PrepaymentModelSpec};

    let spec = PrepaymentModelSpec::cmbs_with_lockout(60, 0.10);

    let smm_at_lockout = spec.smm(60).unwrap();
    assert!(
        smm_at_lockout.abs() < RATE_TOLERANCE,
        "seasoning == lockout_months should be locked out (0 SMM), got {}",
        smm_at_lockout
    );

    let smm_after_lockout = spec.smm(61).unwrap();
    assert!(
        smm_after_lockout > 0.0,
        "seasoning == lockout_months + 1 should prepay, got {}",
        smm_after_lockout
    );
    let cpr_after = smm_to_cpr(smm_after_lockout).expect("valid SMM");
    assert!(
        (cpr_after - 0.10).abs() < RATE_TOLERANCE,
        "post-lockout CPR should be 10%, got {}",
        cpr_after
    );
}

// =============================================================================
// Over-Unity Multiplier Validation Tests
// =============================================================================

#[test]
fn psa_over_unity_multiplier_gives_descriptive_error() {
    // 6% terminal CPR * 20x multiplier = 120% CPR > 100% — should produce a
    // descriptive Validation error, not an opaque InputError.
    use finstack_quant_cashflows::builder::PrepaymentModelSpec;

    let spec = PrepaymentModelSpec::psa(20.0);
    let err = spec.smm(30).expect_err("CPR above 1.0 should be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("CPR") && msg.contains("exceeds 1.0"),
        "error should describe the over-unity CPR, got: {msg}"
    );
}

#[test]
fn sda_over_unity_multiplier_gives_descriptive_error() {
    // 0.60% peak CDR * 200x multiplier = 120% CDR > 100%.
    use finstack_quant_cashflows::builder::DefaultModelSpec;

    let spec = DefaultModelSpec::sda(200.0);
    let err = spec.mdr(30).expect_err("CDR above 1.0 should be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("CDR") && msg.contains("exceeds 1.0"),
        "error should describe the over-unity CDR, got: {msg}"
    );
}

#[test]
fn non_finite_speed_multiplier_rejected() {
    use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec};

    for mult in [f64::NAN, f64::INFINITY, -1.0] {
        assert!(
            PrepaymentModelSpec::psa(mult).smm(15).is_err(),
            "PSA multiplier {mult} should be rejected"
        );
        assert!(
            DefaultModelSpec::sda(mult).mdr(15).is_err(),
            "SDA multiplier {mult} should be rejected"
        );
    }
}

// =============================================================================
// Non-Finite Default/Prepayment Amount Tests
// =============================================================================

#[test]
fn non_finite_defaulted_amount_rejected_without_mutation() {
    // NaN/∞ defaulted_amount previously slipped past the `< 0.0` check and
    // `nan.min(outstanding)` returned `outstanding`, silently liquidating the
    // full balance. Must now be a Validation error with no partial mutation.
    use finstack_quant_cashflows::builder::{emit_default_on, DefaultEvent};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use time::Month;

    let d = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
    for bad in [f64::NAN, f64::INFINITY] {
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: bad,
            recovery_rate: 0.40,
            recovery_lag: 6,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: None,
        };
        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        let err = emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
            .expect_err("non-finite defaulted_amount should be rejected");
        assert!(
            err.to_string().contains("finite"),
            "error should mention finiteness, got: {err}"
        );
        assert_eq!(outstanding, 1_000_000.0, "no mutation on rejection");
        assert!(flows.is_empty(), "no flows on rejection");
    }
}

#[test]
fn non_finite_accrued_on_default_rejected() {
    use finstack_quant_cashflows::builder::{emit_default_on, DefaultEvent};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use time::Month;

    let d = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
    for bad in [f64::NAN, f64::INFINITY] {
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: 100_000.0,
            recovery_rate: 0.40,
            recovery_lag: 6,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: Some(bad),
        };
        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        let result = emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows);
        assert!(
            result.is_err(),
            "non-finite accrued_on_default should be rejected"
        );
        assert_eq!(outstanding, 1_000_000.0, "no mutation on rejection");
        assert!(flows.is_empty(), "no flows on rejection");
    }
}

#[test]
fn non_finite_prepayment_amount_rejected_without_mutation() {
    use finstack_quant_cashflows::builder::emit_prepayment_on;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use time::Month;

    let d = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
    for bad in [f64::NAN, f64::INFINITY] {
        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();
        let err = emit_prepayment_on(d, bad, &mut outstanding, Currency::USD, &mut flows)
            .expect_err("non-finite prepayment amount should be rejected");
        assert!(
            err.to_string().contains("finite"),
            "error should mention finiteness, got: {err}"
        );
        assert_eq!(outstanding, 1_000_000.0, "no mutation on rejection");
        assert!(flows.is_empty(), "no flows on rejection");
    }
}

// =============================================================================
// Multi-Event Atomicity Tests
// =============================================================================

#[test]
fn multi_event_default_emission_is_atomic() {
    // A validation failure on the second event must leave the first event
    // unapplied: no flows emitted, outstanding unchanged.
    use finstack_quant_cashflows::builder::{emit_default_on, DefaultEvent};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use time::Month;

    let d = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
    let valid = DefaultEvent {
        default_date: d,
        defaulted_amount: 100_000.0,
        recovery_rate: 0.40,
        recovery_lag: 6,
        recovery_bdc: None,
        recovery_calendar_id: None,
        accrued_on_default: None,
    };
    let invalid = DefaultEvent {
        default_date: d,
        defaulted_amount: 50_000.0,
        recovery_rate: 1.5, // out of [0, 1]
        recovery_lag: 6,
        recovery_bdc: None,
        recovery_calendar_id: None,
        accrued_on_default: None,
    };

    let mut outstanding = 1_000_000.0;
    let mut flows = Vec::new();
    let result = emit_default_on(
        d,
        &[valid, invalid],
        &mut outstanding,
        Currency::USD,
        &mut flows,
    );

    assert!(
        result.is_err(),
        "second invalid event should fail the batch"
    );
    assert_eq!(
        outstanding, 1_000_000.0,
        "first event must not be applied when a later event fails"
    );
    assert!(flows.is_empty(), "no flows may be emitted on failure");
}

// =============================================================================
// Recovery Date Adjustment Tests
// =============================================================================

#[test]
fn recovery_bdc_adjusts_recovery_flow_date() {
    // 2025-03-01 + 3 months = 2025-06-01, a Sunday; Following on the
    // weekends calendar shifts the Recovery flow to Monday 2025-06-02.
    use finstack_quant_cashflows::builder::{emit_default_on, DefaultEvent};
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date};
    use time::Month;

    let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
    let event = DefaultEvent {
        default_date: d,
        defaulted_amount: 100_000.0,
        recovery_rate: 0.40,
        recovery_lag: 3,
        recovery_bdc: Some(BusinessDayConvention::Following),
        recovery_calendar_id: Some("weekends".to_string()),
        accrued_on_default: None,
    };

    let mut outstanding = 1_000_000.0;
    let mut flows = Vec::new();
    emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
        .expect("should succeed");

    let recovery = flows
        .iter()
        .find(|cf| cf.kind == CFKind::Recovery)
        .expect("recovery flow expected");
    let expected = Date::from_calendar_date(2025, Month::June, 2).expect("valid date");
    assert_eq!(
        recovery.date, expected,
        "Recovery flow should be adjusted from Sunday 2025-06-01 to Monday 2025-06-02"
    );
    assert_eq!(outstanding, 900_000.0);
}

// =============================================================================
// AccruedOnDefault Emission Tests
// =============================================================================

#[test]
fn accrued_on_default_emission() {
    // When accrued_on_default is Some(positive), emit_default_on should produce
    // 3 cashflows: DefaultedNotional + Recovery + AccruedOnDefault
    use finstack_quant_cashflows::builder::emit_default_on;
    use finstack_quant_cashflows::builder::DefaultEvent;
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use time::Month;

    let d = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
    let event = DefaultEvent {
        default_date: d,
        defaulted_amount: 500_000.0,
        recovery_rate: 0.40,
        recovery_lag: 6,
        recovery_bdc: None,
        recovery_calendar_id: None,
        accrued_on_default: Some(12_500.0),
    };

    let mut outstanding = 1_000_000.0;
    let mut flows = Vec::new();
    emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
        .expect("should succeed");

    // Should produce 3 cashflows
    assert_eq!(
        flows.len(),
        3,
        "Expected 3 flows: default + recovery + accrued"
    );

    // First flow: DefaultedNotional
    assert_eq!(flows[0].kind, CFKind::DefaultedNotional);
    assert_eq!(flows[0].amount.amount(), 500_000.0);
    assert_eq!(flows[0].date, d);

    // Second flow: Recovery
    assert_eq!(flows[1].kind, CFKind::Recovery);
    assert_eq!(flows[1].amount.amount(), 200_000.0); // 500k * 0.40

    // Third flow: AccruedOnDefault
    assert_eq!(flows[2].kind, CFKind::AccruedOnDefault);
    assert_eq!(flows[2].amount.amount(), 12_500.0);
    assert_eq!(flows[2].date, d); // Same date as default

    // Outstanding reduced by defaulted amount only (not by accrued)
    assert_eq!(outstanding, 500_000.0);
}

#[test]
fn accrued_on_default_none_no_extra_flow() {
    // When accrued_on_default is None, only 2 cashflows should be emitted
    use finstack_quant_cashflows::builder::emit_default_on;
    use finstack_quant_cashflows::builder::DefaultEvent;
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use time::Month;

    let d = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
    let event = DefaultEvent {
        default_date: d,
        defaulted_amount: 100_000.0,
        recovery_rate: 0.40,
        recovery_lag: 12,
        recovery_bdc: None,
        recovery_calendar_id: None,
        accrued_on_default: None,
    };

    let mut outstanding = 1_000_000.0;
    let mut flows = Vec::new();
    emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
        .expect("should succeed");

    assert_eq!(
        flows.len(),
        2,
        "Expected 2 flows: default + recovery (no accrued)"
    );
    assert_eq!(flows[0].kind, CFKind::DefaultedNotional);
    assert_eq!(flows[1].kind, CFKind::Recovery);
}

#[test]
fn accrued_on_default_zero_no_extra_flow() {
    // When accrued_on_default is Some(0.0), no AccruedOnDefault flow should be emitted
    use finstack_quant_cashflows::builder::emit_default_on;
    use finstack_quant_cashflows::builder::DefaultEvent;
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use time::Month;

    let d = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
    let event = DefaultEvent {
        default_date: d,
        defaulted_amount: 100_000.0,
        recovery_rate: 0.40,
        recovery_lag: 12,
        recovery_bdc: None,
        recovery_calendar_id: None,
        accrued_on_default: Some(0.0),
    };

    let mut outstanding = 1_000_000.0;
    let mut flows = Vec::new();
    emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
        .expect("should succeed");

    assert_eq!(
        flows.len(),
        2,
        "Expected 2 flows: zero accrued should not emit"
    );
    assert_eq!(flows[0].kind, CFKind::DefaultedNotional);
    assert_eq!(flows[1].kind, CFKind::Recovery);
}

#[test]
fn defaulted_notional_is_not_counted_as_positive_npv_cashflow() {
    use finstack_quant_cashflows::builder::schedule::CashFlowMeta;
    use finstack_quant_cashflows::builder::Notional;
    use finstack_quant_core::cashflow::{CFKind, CashFlow, Discountable};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use time::Month;

    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let default_date = Date::from_calendar_date(2025, Month::June, 1).unwrap();
    let recovery_date = Date::from_calendar_date(2025, Month::December, 1).unwrap();

    let schedule = finstack_quant_cashflows::builder::schedule::CashFlowSchedule {
        flows: vec![
            CashFlow {
                date: default_date,
                reset_date: None,
                amount: Money::new(500.0, Currency::USD),
                kind: CFKind::DefaultedNotional,
                accrual_factor: 0.0,
                rate: None,
            },
            CashFlow {
                date: recovery_date,
                reset_date: None,
                amount: Money::new(200.0, Currency::USD),
                kind: CFKind::Recovery,
                accrual_factor: 0.0,
                rate: None,
            },
        ],
        notional: Notional::par(1_000.0, Currency::USD),
        day_count: DayCount::Act365F,
        meta: CashFlowMeta::default(),
    };

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (5.0, 1.0)])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .unwrap();

    let pv = schedule.npv(&curve, base, Some(DayCount::Act365F)).unwrap();
    assert!(
        (pv.amount() - 200.0).abs() < 1e-10,
        "plain npv should include realized recovery but exclude default write-down markers, got {}",
        pv.amount()
    );
}

#[test]
fn credit_adjusted_period_pv_respects_explicit_default_and_recovery_flows() {
    use finstack_quant_cashflows::builder::schedule::CashFlowMeta;
    use finstack_quant_cashflows::builder::Notional;
    use finstack_quant_core::cashflow::{CFKind, CashFlow};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Period, PeriodId};
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use time::Month;

    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let default_date = Date::from_calendar_date(2025, Month::June, 1).unwrap();
    let recovery_date = Date::from_calendar_date(2025, Month::December, 1).unwrap();

    let schedule = finstack_quant_cashflows::builder::schedule::CashFlowSchedule {
        flows: vec![
            CashFlow {
                date: default_date,
                reset_date: None,
                amount: Money::new(500.0, Currency::USD),
                kind: CFKind::DefaultedNotional,
                accrual_factor: 0.0,
                rate: None,
            },
            CashFlow {
                date: recovery_date,
                reset_date: None,
                amount: Money::new(200.0, Currency::USD),
                kind: CFKind::Recovery,
                accrual_factor: 0.0,
                rate: None,
            },
        ],
        notional: Notional::par(1_000.0, Currency::USD),
        day_count: DayCount::Act365F,
        meta: CashFlowMeta::default(),
    };

    let periods = vec![Period {
        id: PeriodId::annual(2025),
        start: base,
        end: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
        is_actual: true,
    }];

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (5.0, 1.0)])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("USD-HZD")
        .base_date(base)
        .recovery_rate(0.40)
        .knots([(1.0, 0.50)])
        .build()
        .unwrap();

    // Combining DefaultedNotional flows with recovery_rate is now rejected
    // to prevent double-counting recovery from both explicit flows and the
    // hazard-curve adjustment.
    let err_result = schedule.pv_by_period(
        &periods,
        finstack_quant_cashflows::builder::PvDiscountSource::Discount {
            disc: &disc,
            credit: Some(finstack_quant_cashflows::builder::PvCreditAdjustment {
                hazard: Some(&hazard),
                recovery_rate: Some(0.40),
            }),
        },
        finstack_quant_cashflows::aggregation::DateContext::new(
            base,
            DayCount::Act365F,
            DayCountContext::default(),
        ),
    );
    assert!(
        err_result.is_err(),
        "should reject DefaultedNotional + recovery_rate combination"
    );

    // When recovery_rate is None, the call succeeds and the explicit
    // Recovery flow is discounted normally.
    let pv_map = schedule
        .pv_by_period(
            &periods,
            finstack_quant_cashflows::builder::PvDiscountSource::Discount {
                disc: &disc,
                credit: Some(finstack_quant_cashflows::builder::PvCreditAdjustment {
                    hazard: Some(&hazard),
                    recovery_rate: None,
                }),
            },
            finstack_quant_cashflows::aggregation::DateContext::new(
                base,
                DayCount::Act365F,
                DayCountContext::default(),
            ),
        )
        .unwrap();

    let pv = pv_map
        .get(&PeriodId::annual(2025))
        .and_then(|ccy_map| ccy_map.get(&Currency::USD))
        .expect("expected USD PV for 2025");

    // Without recovery_rate, the PV consists of the explicit Recovery
    // flow (200) discounted by the survival probability.
    assert!(
        pv.amount() > 0.0,
        "credit-adjusted PV should discount realized recovery, got {}",
        pv.amount()
    );
}

// =============================================================================
// Property-Based Tests
// =============================================================================

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Property: PSA SMM is always non-negative for any month and multiplier >= 0
        #[test]
        fn psa_smm_non_negative(month in 0u32..500, multiplier in 0.0f64..10.0) {
            use finstack_quant_cashflows::builder::PrepaymentModelSpec;

            let model = PrepaymentModelSpec::psa(multiplier);
            let smm = model.smm(month).unwrap();

            prop_assert!(
                smm >= 0.0,
                "PSA SMM should be non-negative: multiplier={}, month={}, smm={}",
                multiplier, month, smm
            );
        }

        /// Property: PSA SMM is monotonically non-decreasing in month during ramp (0-30)
        #[test]
        fn psa_smm_monotonic_during_ramp(month1 in 0u32..30, month2 in 0u32..30) {
            use finstack_quant_cashflows::builder::PrepaymentModelSpec;

            let model = PrepaymentModelSpec::psa_100();
            let smm1 = model.smm(month1).unwrap();
            let smm2 = model.smm(month2).unwrap();

            if month1 <= month2 {
                prop_assert!(
                    smm1 <= smm2 + RATE_TOLERANCE,
                    "PSA SMM should be non-decreasing during ramp: smm({})={} vs smm({})={}",
                    month1, smm1, month2, smm2
                );
            }
        }

        /// Property: PSA SMM is constant after ramp (month >= 30)
        #[test]
        fn psa_smm_constant_after_ramp(month in 30u32..500) {
            use finstack_quant_cashflows::builder::PrepaymentModelSpec;

            let model = PrepaymentModelSpec::psa_100();
            let smm_30 = model.smm(30).unwrap();
            let smm_month = model.smm(month).unwrap();

            prop_assert!(
                (smm_30 - smm_month).abs() < RATE_TOLERANCE,
                "PSA SMM should be constant after month 30: smm(30)={} vs smm({})={}",
                smm_30, month, smm_month
            );
        }

        /// Property: Higher PSA multiplier -> higher SMM (monotonicity in multiplier)
        #[test]
        fn psa_smm_monotonic_in_multiplier(mult1 in 0.0f64..5.0, mult2 in 0.0f64..5.0, month in 1u32..100) {
            use finstack_quant_cashflows::builder::PrepaymentModelSpec;

            let model1 = PrepaymentModelSpec::psa(mult1);
            let model2 = PrepaymentModelSpec::psa(mult2);

            let smm1 = model1.smm(month).unwrap();
            let smm2 = model2.smm(month).unwrap();

            if mult1 <= mult2 {
                prop_assert!(
                    smm1 <= smm2 + RATE_TOLERANCE,
                    "Higher multiplier should give higher SMM: mult1={}, mult2={}, smm1={}, smm2={}",
                    mult1, mult2, smm1, smm2
                );
            }
        }

        /// Property: CPR -> SMM -> CPR roundtrip preserves value
        #[test]
        fn cpr_smm_roundtrip(cpr in 0.0f64..0.99) {
            use finstack_quant_cashflows::builder::{cpr_to_smm, smm_to_cpr};

            let smm = cpr_to_smm(cpr).unwrap();
            let cpr_back = smm_to_cpr(smm).expect("valid SMM");

            prop_assert!(
                (cpr - cpr_back).abs() < FACTOR_TOLERANCE,
                "CPR roundtrip failed: {} -> {} -> {}",
                cpr, smm, cpr_back
            );
        }

        /// Property: SMM < CPR for all positive rates (monthly < annual)
        #[test]
        fn smm_less_than_cpr(cpr in 0.001f64..0.99) {
            use finstack_quant_cashflows::builder::cpr_to_smm;

            let smm = cpr_to_smm(cpr).unwrap();

            prop_assert!(
                smm < cpr,
                "SMM should be less than CPR: smm={}, cpr={}",
                smm, cpr
            );
        }

        /// Property: SDA MDR is always non-negative
        #[test]
        fn sda_mdr_non_negative(month in 0u32..500, multiplier in 0.0f64..10.0) {
            use finstack_quant_cashflows::builder::DefaultModelSpec;

            let model = DefaultModelSpec::sda(multiplier);
            let mdr = model.mdr(month).unwrap();

            prop_assert!(
                mdr >= 0.0,
                "SDA MDR should be non-negative: multiplier={}, month={}, mdr={}",
                multiplier, month, mdr
            );
        }

        /// Property: SDA MDR reaches terminal rate after month 120
        #[test]
        fn sda_mdr_terminal_after_120(month in 120u32..500) {
            use finstack_quant_cashflows::builder::{smm_to_cpr, DefaultModelSpec};

            let model = DefaultModelSpec::sda(1.0);
            let mdr = model.mdr(month).unwrap();
            let cdr = smm_to_cpr(mdr).expect("valid SMM");

            // Terminal rate for 100% SDA is 0.03% CDR
            prop_assert!(
                (cdr - 0.0003).abs() < RATE_TOLERANCE,
                "SDA should be at terminal 0.03% CDR after month 120: month={}, cdr={}",
                month, cdr
            );
        }

        /// Property: Zero PSA multiplier gives zero SMM
        #[test]
        fn psa_zero_multiplier_gives_zero(month in 0u32..500) {
            use finstack_quant_cashflows::builder::PrepaymentModelSpec;

            let model = PrepaymentModelSpec::psa(0.0);
            let smm = model.smm(month).unwrap();

            prop_assert!(
                smm.abs() < RATE_TOLERANCE,
                "Zero PSA multiplier should give zero SMM: month={}, smm={}",
                month, smm
            );
        }
    }
}

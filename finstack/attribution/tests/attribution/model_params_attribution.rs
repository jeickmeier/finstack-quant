//! Integration tests for model parameters attribution.
//!
//! Tests attribution of P&L from changes in model-specific parameters like
//! prepayment speeds, default rates, recovery rates, and conversion ratios.
//!
//! ## Industry Standard References
//!
//! ### PSA (Public Securities Association) Prepayment Model
//!
//! The PSA benchmark assumes:
//! - CPR starts at 0% in month 0
//! - CPR ramps linearly to 6% (600bp) by month 30
//! - CPR remains at 6% (600bp) thereafter
//!
//! PSA scaling: PSA% × 6% = annual CPR at benchmark
//! - 100% PSA = 6% CPR (steady state)
//! - 150% PSA = 9% CPR (faster prepayment)
//! - 50% PSA = 3% CPR (slower prepayment)
//!
//! The shift calculation uses:
//!   shift_bp = (PSA_t1 - PSA_t0) × 600bp
//!
//! Reference: "The Handbook of Fixed Income Securities" by Fabozzi, Chapter 29
//!
//! ### CDR (Conditional Default Rate)
//!
//! CDR is expressed as an annual rate of default on remaining balance.
//! Shift is measured in basis points:
//!   shift_bp = (CDR_t1 - CDR_t0) × 10,000
//!
//! Reference: "Credit Risk Modeling" by Lando, Chapter 7
//!
//! ### Recovery Rate
//!
//! Recovery rate is the percentage of par recovered upon default.
//! Shift is measured in percentage points (not basis points):
//!   shift_pct = (Recovery_t1 - Recovery_t0) × 100
//!
//! Reference: Moody's Annual Default Study

use finstack_attribution::{
    measure_conversion_shift, measure_default_shift, measure_prepayment_shift,
    measure_recovery_shift,
};
use finstack_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_valuations::instruments::fixed_income::convertible::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, DividendAdjustment,
};
use finstack_valuations::instruments::model_params::ModelParamsSnapshot;

/// Test PSA prepayment shift measurement.
///
/// PSA scaling formula (industry standard):
///   shift_bp = ΔPSA × 600bp
///
/// Where 600bp = 6% annual CPR at 100% PSA benchmark (steady state after month 30).
///
/// Reference: PSA Standard Prepayment Benchmark Model
#[test]
fn test_prepayment_shift_measurement_psa() {
    let params_t0 = ModelParamsSnapshot::StructuredCredit {
        prepayment_spec: PrepaymentModelSpec::psa(1.0),
        default_spec: DefaultModelSpec::constant_cdr(0.02),
        recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
    };

    let params_t1 = ModelParamsSnapshot::StructuredCredit {
        prepayment_spec: PrepaymentModelSpec::psa(1.5),
        default_spec: DefaultModelSpec::constant_cdr(0.02),
        recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
    };

    let shift = measure_prepayment_shift(&params_t0, &params_t1);

    // PSA increased from 100% to 150% (0.5 increase in PSA multiple)
    // shift_bp = 0.5 × 600bp = 300bp
    // Reference: 100% PSA = 6% CPR steady state, so 150% PSA = 9% CPR
    assert_eq!(shift, 300.0);
}

#[test]
fn test_prepayment_shift_measurement_cpr() {
    let params_t0 = ModelParamsSnapshot::StructuredCredit {
        prepayment_spec: PrepaymentModelSpec::constant_cpr(0.06),
        default_spec: DefaultModelSpec::constant_cdr(0.02),
        recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
    };

    let params_t1 = ModelParamsSnapshot::StructuredCredit {
        prepayment_spec: PrepaymentModelSpec::constant_cpr(0.08),
        default_spec: DefaultModelSpec::constant_cdr(0.02),
        recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
    };

    let shift = measure_prepayment_shift(&params_t0, &params_t1);

    // CPR increased from 6% to 8% (2% increase = 200bp)
    assert!((shift - 200.0).abs() < 0.01);
}

#[test]
fn test_default_shift_measurement() {
    let params_t0 = ModelParamsSnapshot::StructuredCredit {
        prepayment_spec: PrepaymentModelSpec::psa(1.0),
        default_spec: DefaultModelSpec::constant_cdr(0.02),
        recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
    };

    let params_t1 = ModelParamsSnapshot::StructuredCredit {
        prepayment_spec: PrepaymentModelSpec::psa(1.0),
        default_spec: DefaultModelSpec::constant_cdr(0.03),
        recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
    };

    let shift = measure_default_shift(&params_t0, &params_t1);

    // CDR increased from 2% to 3% (1% increase = 100bp)
    assert!((shift - 100.0).abs() < 0.01);
}

#[test]
fn test_recovery_shift_measurement() {
    let params_t0 = ModelParamsSnapshot::StructuredCredit {
        prepayment_spec: PrepaymentModelSpec::psa(1.0),
        default_spec: DefaultModelSpec::constant_cdr(0.02),
        recovery_spec: RecoveryModelSpec::with_lag(0.60, 12),
    };

    let params_t1 = ModelParamsSnapshot::StructuredCredit {
        prepayment_spec: PrepaymentModelSpec::psa(1.0),
        default_spec: DefaultModelSpec::constant_cdr(0.02),
        recovery_spec: RecoveryModelSpec::with_lag(0.65, 12),
    };

    let shift = measure_recovery_shift(&params_t0, &params_t1);

    // Recovery rate increased from 60% to 65% (5 percentage points)
    assert!((shift - 5.0).abs() < 0.01);
}

#[test]
fn test_conversion_shift_measurement() {
    let params_t0 = ModelParamsSnapshot::Convertible {
        conversion_spec: ConversionSpec {
            ratio: Some(10.0),
            price: None,
            policy: ConversionPolicy::Voluntary,
            anti_dilution: AntiDilutionPolicy::None,
            dividend_adjustment: DividendAdjustment::None,
            dilution_events: Vec::new(),
        },
    };

    let params_t1 = ModelParamsSnapshot::Convertible {
        conversion_spec: ConversionSpec {
            ratio: Some(12.0),
            price: None,
            policy: ConversionPolicy::Voluntary,
            anti_dilution: AntiDilutionPolicy::None,
            dividend_adjustment: DividendAdjustment::None,
            dilution_events: Vec::new(),
        },
    };

    let shift = measure_conversion_shift(&params_t0, &params_t1);

    // Conversion ratio increased from 10 to 12 (20% increase)
    assert_eq!(shift, 20.0);
}

#[test]
fn test_model_params_none_for_plain_instruments() {
    use finstack_attribution::extract_model_params;
    use finstack_core::currency::Currency;
    use finstack_core::dates::create_date;
    use finstack_core::money::Money;
    use finstack_valuations::instruments::fixed_income::bond::Bond;
    use finstack_valuations::instruments::Instrument;
    use std::sync::Arc;
    use time::Month;

    // Plain bonds don't have model parameters (no prepayment/default/recovery models)
    let bond = Bond::fixed(
        "PLAIN-BOND-001",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        create_date(2024, Month::January, 1).unwrap(),
        create_date(2029, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .unwrap();

    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);
    let params = extract_model_params(&bond_instrument);

    // Plain bond should return None for model parameters
    assert!(
        matches!(params, ModelParamsSnapshot::None),
        "Plain bond should have ModelParamsSnapshot::None, got {:?}",
        params
    );
}

#[test]
fn test_with_model_params_none_reuses_original_arc() {
    use finstack_attribution::with_model_params;
    use finstack_core::currency::Currency;
    use finstack_core::dates::create_date;
    use finstack_core::money::Money;
    use finstack_valuations::instruments::fixed_income::bond::Bond;
    use finstack_valuations::instruments::Instrument;
    use std::sync::Arc;
    use time::Month;

    let bond = Bond::fixed(
        "PLAIN-BOND-002",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        create_date(2024, Month::January, 1).unwrap(),
        create_date(2029, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .unwrap();

    let instrument: Arc<dyn Instrument> = Arc::new(bond);
    let unchanged = with_model_params(&instrument, &ModelParamsSnapshot::None)
        .expect("None model params should be a no-op");

    assert!(Arc::ptr_eq(&instrument, &unchanged));
}

#[test]
fn test_model_params_mismatch_errors_through_instrument_hook() {
    use finstack_attribution::with_model_params;
    use finstack_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
    use finstack_valuations::instruments::Instrument;
    use std::sync::Arc;

    let structured_credit: Arc<dyn Instrument> = Arc::new(StructuredCredit::example());
    let convertible_params = ModelParamsSnapshot::Convertible {
        conversion_spec: ConversionSpec {
            ratio: Some(10.0),
            price: None,
            policy: ConversionPolicy::Voluntary,
            anti_dilution: AntiDilutionPolicy::None,
            dividend_adjustment: DividendAdjustment::None,
            dilution_events: Vec::new(),
        },
    };

    let Err(err) = with_model_params(&structured_credit, &convertible_params) else {
        panic!("mismatched model parameter snapshot should fail")
    };
    let message = err.to_string();

    assert!(
        message.contains("StructuredCredit"),
        "mismatch error should identify the expected instrument type, got {message}"
    );
}

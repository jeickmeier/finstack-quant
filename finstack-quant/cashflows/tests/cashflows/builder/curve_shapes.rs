//! Curve shapes beyond PSA/SDA: ABS speed, explicit monthly vectors,
//! cumulative net-loss curves, rating-agency default timing and severity
//! vectors by month of default.

use finstack_quant_cashflows::builder::{
    abs_to_smm, cpr_to_smm, DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec,
};

const TOL: f64 = 1e-12;

#[test]
fn abs_speed_follows_the_fabozzi_convention() {
    let spec = PrepaymentModelSpec::abs(0.015);

    assert!((spec.smm(1).unwrap() - 0.015).abs() < TOL);
    assert_eq!(spec.smm(0).unwrap(), spec.smm(1).unwrap());
    // Month 36: 1.5% / (1 − 1.5% × 35) = 3.158%.
    assert!((spec.smm(36).unwrap() - 0.015 / 0.475).abs() < TOL);
    assert!((abs_to_smm(0.015, 36).unwrap() - 0.015 / 0.475).abs() < TOL);

    let mut previous = 0.0;
    for month in 1..=66 {
        let smm = spec.smm(month).unwrap();
        assert!(
            smm >= previous,
            "SMM must rise with seasoning at month {month}"
        );
        previous = smm;
    }
    // 1 − 0.015 × 66 = 0.01 ≤ speed: the original balance is gone.
    assert_eq!(spec.smm(67).unwrap(), 1.0);
    // The placeholder CPR is the annualized month-1 rate.
    assert!((spec.cpr - (1.0 - 0.985_f64.powi(12))).abs() < TOL);
    assert!(PrepaymentModelSpec::abs(1.2).validate().is_err());
}

#[test]
fn abs_speed_prepays_the_original_balance_evenly() {
    // Applying SMM_t to the surviving balance retires exactly `speed` of the
    // original balance every month until the pool is exhausted.
    let speed = 0.02;
    let spec = PrepaymentModelSpec::abs(speed);
    let mut balance = 1.0_f64;
    for month in 1..=49 {
        let prepaid = balance * spec.smm(month).unwrap();
        assert!((prepaid - speed).abs() < 1e-9, "month {month}: {prepaid}");
        balance -= prepaid;
    }
    assert!((balance - 0.02).abs() < 1e-9);
    assert_eq!(spec.smm(50).unwrap(), 1.0);
}

#[test]
fn vector_curves_hold_the_last_value() {
    let prepay = PrepaymentModelSpec::vector(vec![0.02, 0.04]);
    assert_eq!(prepay.smm(0).unwrap(), cpr_to_smm(0.02).unwrap());
    assert_eq!(prepay.smm(1).unwrap(), cpr_to_smm(0.02).unwrap());
    assert_eq!(prepay.smm(2).unwrap(), cpr_to_smm(0.04).unwrap());
    assert_eq!(prepay.smm(120).unwrap(), cpr_to_smm(0.04).unwrap());
    assert_eq!(prepay.cpr, 0.02);

    let default = DefaultModelSpec::vector(vec![0.01, 0.03]);
    assert_eq!(default.mdr(1).unwrap(), cpr_to_smm(0.01).unwrap());
    assert_eq!(default.mdr(2).unwrap(), default.mdr(240).unwrap());

    assert!(PrepaymentModelSpec::vector(Vec::new()).smm(1).is_err());
    assert!(DefaultModelSpec::vector(vec![0.01, 1.5])
        .validate()
        .is_err());
    assert!(PrepaymentModelSpec::vector(vec![f64::NAN])
        .validate()
        .is_err());
}

#[test]
fn cumulative_loss_curve_reproduces_its_defaults_without_amortization() {
    // 2% lifetime net loss at 50% severity is a 4% lifetime default rate,
    // arriving 1% of the original balance a month for four months.
    let spec = DefaultModelSpec::cumulative_loss(vec![0.5, 1.0, 1.5, 2.0], 0.5);
    spec.validate().unwrap();

    let mut balance = 1.0_f64;
    let mut defaulted = 0.0_f64;
    for month in 1..=6 {
        let defaults = balance * spec.mdr(month).unwrap();
        balance -= defaults;
        defaulted += defaults;
        let expected = spec.cumulative_default_fraction(month).unwrap().unwrap();
        assert!(
            (defaulted - expected).abs() < TOL,
            "month {month}: simulated {defaulted} vs curve {expected}"
        );
    }
    assert!((defaulted - 0.04).abs() < TOL);
    assert_eq!(spec.mdr(5).unwrap(), 0.0);
    assert_eq!(spec.cumulative_default_fraction(0).unwrap(), Some(0.0));
    assert_eq!(
        DefaultModelSpec::constant_cdr(0.02)
            .cumulative_default_fraction(12)
            .unwrap(),
        None
    );
}

#[test]
fn cumulative_loss_mdr_scales_with_the_surviving_balance() {
    let spec = DefaultModelSpec::cumulative_loss(vec![1.0, 2.0], 0.5);

    // Month 2 defaults 2% of the original balance; on a pool that has paid
    // down to half its original size that is 4% of the surviving balance.
    assert!((spec.mdr_with_survival(2, 1.0).unwrap() - 0.02).abs() < TOL);
    assert!((spec.mdr_with_survival(2, 0.5).unwrap() - 0.04).abs() < TOL);
    // The rate-only form divides by the curve's own survival (1 − 2%).
    assert!((spec.mdr(2).unwrap() - 0.02 / 0.98).abs() < TOL);
    assert_eq!(spec.mdr_with_survival(2, 0.0).unwrap(), 0.0);
    assert!(spec.mdr_with_survival(2, -0.1).is_err());
    assert!(spec.mdr_with_survival(2, f64::NAN).is_err());
    // Rate-based curves ignore the survival fraction.
    let constant = DefaultModelSpec::constant_cdr(0.02);
    assert_eq!(
        constant.mdr_with_survival(7, 0.3).unwrap(),
        constant.mdr(7).unwrap()
    );
}

#[test]
fn timing_curve_sums_to_the_cumulative_default_rate() {
    let spec = DefaultModelSpec::timing(0.06, vec![15.0, 30.0, 30.0, 15.0, 10.0]);
    spec.validate().unwrap();
    let fraction = |month| spec.cumulative_default_fraction(month).unwrap().unwrap();

    assert_eq!(fraction(0), 0.0);
    assert!((fraction(6) - 0.06 * 0.15 * 0.5).abs() < TOL);
    assert!((fraction(12) - 0.06 * 0.15).abs() < TOL);
    assert!((fraction(24) - 0.06 * 0.45).abs() < TOL);
    assert!((fraction(60) - 0.06).abs() < TOL);
    assert!((fraction(120) - 0.06).abs() < TOL);

    let mut balance = 1.0_f64;
    let mut defaulted = 0.0_f64;
    for month in 1..=72 {
        let defaults = balance * spec.mdr(month).unwrap();
        balance -= defaults;
        defaulted += defaults;
    }
    assert!((defaulted - 0.06).abs() < TOL);

    assert!(DefaultModelSpec::timing(0.06, vec![50.0, 40.0])
        .validate()
        .is_err());
    assert!(DefaultModelSpec::timing(1.5, vec![100.0])
        .validate()
        .is_err());
    assert!(DefaultModelSpec::timing(0.05, vec![120.0, -20.0])
        .validate()
        .is_err());
}

#[test]
fn cumulative_loss_curves_must_be_non_decreasing_with_a_positive_severity() {
    assert!(DefaultModelSpec::cumulative_loss(vec![1.0, 0.5], 0.5)
        .validate()
        .is_err());
    assert!(DefaultModelSpec::cumulative_loss(vec![1.0], 0.0)
        .validate()
        .is_err());
    assert!(DefaultModelSpec::cumulative_loss(vec![1.0], 1.5)
        .validate()
        .is_err());
    assert!(DefaultModelSpec::cumulative_loss(Vec::new(), 0.5)
        .validate()
        .is_err());
}

#[test]
fn severity_vector_overrides_the_flat_recovery_by_month_of_default() {
    let spec = RecoveryModelSpec::with_lag(0.40, 12).with_severity_vector(vec![0.7, 0.6]);
    spec.validate().unwrap();

    assert!((spec.recovery_rate(0) - 0.3).abs() < TOL);
    assert!((spec.recovery_rate(1) - 0.3).abs() < TOL);
    assert!((spec.recovery_rate(2) - 0.4).abs() < TOL);
    assert!((spec.recovery_rate(60) - 0.4).abs() < TOL);
    assert_eq!(RecoveryModelSpec::with_lag(0.40, 12).recovery_rate(7), 0.40);

    assert!(RecoveryModelSpec::with_lag(0.4, 1)
        .with_severity_vector(vec![1.2])
        .validate()
        .is_err());
    assert!(RecoveryModelSpec::with_lag(0.4, 1)
        .with_severity_vector(Vec::new())
        .validate()
        .is_err());
}

#[test]
fn curve_shapes_round_trip_through_serde() {
    for spec in [
        PrepaymentModelSpec::abs(0.015),
        PrepaymentModelSpec::vector(vec![0.01, 0.02]),
    ] {
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(
            serde_json::from_str::<PrepaymentModelSpec>(&json).unwrap(),
            spec
        );
    }
    for spec in [
        DefaultModelSpec::vector(vec![0.01]),
        DefaultModelSpec::cumulative_loss(vec![0.5, 1.0], 0.5),
        DefaultModelSpec::timing(0.05, vec![60.0, 40.0]),
    ] {
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(
            serde_json::from_str::<DefaultModelSpec>(&json).unwrap(),
            spec
        );
    }
    let recovery = RecoveryModelSpec::with_lag(0.4, 6).with_severity_vector(vec![0.6]);
    let json = serde_json::to_string(&recovery).unwrap();
    assert!(json.contains("severity_vector"));
    assert_eq!(
        serde_json::from_str::<RecoveryModelSpec>(&json).unwrap(),
        recovery
    );
    assert!(!serde_json::to_string(&RecoveryModelSpec::with_lag(0.4, 6))
        .unwrap()
        .contains("severity_vector"));
}

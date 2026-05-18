//! Shared test fixtures and utilities for CDS tranche tests.

use finstack_cashflows::builder::ScheduleParams;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::market_data::term_structures::{
    BaseCorrelationCurve, CreditIndexData, HazardCurve,
};
use finstack_core::money::Money;
use finstack_valuations::instruments::credit_derivatives::cds_tranche::CDSTrancheParams;
use finstack_valuations::instruments::credit_derivatives::cds_tranche::{CDSTranche, TrancheSide};
use std::sync::Arc;
use time::Month;

/// Standard base date for test scenarios
pub fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

/// Standard 5-year maturity date
pub fn maturity_5y() -> Date {
    Date::from_calendar_date(2030, Month::January, 1).unwrap()
}

/// Create a standard discount curve for testing (USD-OIS)
pub fn standard_discount_curve() -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base_date())
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
        .interp(finstack_core::math::interp::InterpStyle::LogLinear)
        .build()
        .unwrap()
}

/// Create a standard index hazard curve (CDX.NA.IG style)
pub fn standard_hazard_curve() -> HazardCurve {
    HazardCurve::builder("CDX.NA.IG.42")
        .base_date(base_date())
        .recovery_rate(0.40)
        .knots(vec![(1.0, 0.01), (3.0, 0.015), (5.0, 0.02), (10.0, 0.025)])
        .par_spreads(vec![(1.0, 60.0), (3.0, 80.0), (5.0, 100.0), (10.0, 140.0)])
        .build()
        .unwrap()
}

/// Create a standard base correlation curve.
///
/// The slope must be gentle enough to stay arbitrage-free at the tranchelet
/// level: a steeply rising base correlation (e.g. 0.25 → 0.45 over the
/// 3% → 7% strikes) pushes the equity-tranche EL *down* enough that the
/// tranchelet decomposition `EL(0,D) − EL(0,A)` goes negative at short
/// horizons (low PD) — genuine base-correlation arbitrage, which the tranche
/// pricer now rejects with an explicit error when arbitrage validation is
/// enabled (the default). The shape below was verified to keep
/// `EL(0,D) ≥ EL(0,A)` across all payment-date PDs for this index.
pub fn standard_correlation_curve() -> BaseCorrelationCurve {
    BaseCorrelationCurve::builder("CDX.NA.IG.42_5Y")
        .knots(vec![
            (3.0, 0.25),  // 0-3% equity
            (7.0, 0.30),  // 0-7% junior mezzanine
            (10.0, 0.34), // 0-10% senior mezzanine
            (15.0, 0.40), // 0-15% senior
            (30.0, 0.50), // 0-30% super senior
        ])
        .build()
        .unwrap()
}

/// Create standard credit index data (homogeneous pool)
pub fn standard_credit_index() -> CreditIndexData {
    CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(standard_hazard_curve()))
        .base_correlation_curve(Arc::new(standard_correlation_curve()))
        .build()
        .unwrap()
}

/// Create a complete market context with standard curves
pub fn standard_market_context() -> MarketContext {
    MarketContext::new()
        .insert(standard_discount_curve())
        .insert(standard_hazard_curve())
        .insert(standard_correlation_curve())
        .insert_credit_index("CDX.NA.IG.42", standard_credit_index())
}

/// Create a market context with heterogeneous issuer curves
pub fn market_context_with_issuers(n: usize) -> MarketContext {
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date())
        .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.84), (10.0, 0.68)])
        .build()
        .unwrap();

    let index_curve = Arc::new(
        HazardCurve::builder("CDX.NA.IG.42")
            .base_date(base_date())
            .recovery_rate(0.40)
            .knots(vec![
                (1.0, 0.012),
                (3.0, 0.017),
                (5.0, 0.022),
                (10.0, 0.028),
            ])
            .par_spreads(vec![(1.0, 65.0), (3.0, 85.0), (5.0, 105.0), (10.0, 145.0)])
            .build()
            .unwrap(),
    );

    let base_corr_curve = standard_correlation_curve();

    let mut issuer_curves = finstack_core::HashMap::default();
    for i in 0..n {
        let id = format!("ISSUER-{:03}", i + 1);
        let bump = (i as f64) * 0.001;
        let hz = HazardCurve::builder(id.as_str())
            .base_date(base_date())
            .recovery_rate(0.40)
            .knots(vec![
                (1.0, (0.012 + bump).min(0.2)),
                (3.0, (0.017 + bump).min(0.2)),
                (5.0, (0.022 + bump).min(0.2)),
                (10.0, (0.028 + bump).min(0.2)),
            ])
            .build()
            .unwrap();
        issuer_curves.insert(id, Arc::new(hz));
    }

    let index = CreditIndexData::builder()
        .num_constituents(n as u16)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::clone(&index_curve))
        .base_correlation_curve(Arc::new(base_corr_curve))
        .issuer_curves(issuer_curves)
        .build()
        .unwrap();

    MarketContext::new()
        .insert(discount_curve)
        .insert(index_curve.as_ref().clone())
        .insert_credit_index("CDX.NA.IG.42", index)
}

/// Create standard mezzanine tranche (3-7%)
pub fn mezzanine_tranche() -> CDSTranche {
    let tranche_params = CDSTrancheParams::mezzanine_tranche(
        "CDX.NA.IG.42",
        42,
        Money::new(10_000_000.0, Currency::USD),
        maturity_5y(),
        500.0, // 5% running coupon
    );
    let schedule_params = ScheduleParams::quarterly_act360();
    CDSTranche::new(
        "CDX_IG42_3_7_5Y",
        &tranche_params,
        &schedule_params,
        finstack_core::types::CurveId::from("USD-OIS"),
        finstack_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters")
}

/// Create equity tranche (0-3%)
pub fn equity_tranche() -> CDSTranche {
    let tranche_params = CDSTrancheParams::equity_tranche(
        "CDX.NA.IG.42",
        42,
        Money::new(10_000_000.0, Currency::USD),
        maturity_5y(),
        1000.0, // 10% running coupon (typical for equity)
    );
    let schedule_params = ScheduleParams::quarterly_act360();
    CDSTranche::new(
        "CDX_IG42_0_3_5Y",
        &tranche_params,
        &schedule_params,
        finstack_core::types::CurveId::from("USD-OIS"),
        finstack_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters")
}

/// Create senior tranche (7-10%)
pub fn senior_tranche() -> CDSTranche {
    let tranche_params = CDSTrancheParams::new(
        "CDX.NA.IG.42",
        42,
        7.0,
        10.0,
        Money::new(10_000_000.0, Currency::USD),
        maturity_5y(),
        100.0, // 1% running coupon (typical for senior)
    );
    let schedule_params = ScheduleParams::quarterly_act360();
    CDSTranche::new(
        "CDX_IG42_7_10_5Y",
        &tranche_params,
        &schedule_params,
        finstack_core::types::CurveId::from("USD-OIS"),
        finstack_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters")
}

/// Create custom tranche with specified parameters
pub fn custom_tranche(
    attach_pct: f64,
    detach_pct: f64,
    running_coupon_bp: f64,
    side: TrancheSide,
) -> CDSTranche {
    let tranche_params = CDSTrancheParams::new(
        "CDX.NA.IG.42",
        42,
        attach_pct,
        detach_pct,
        Money::new(10_000_000.0, Currency::USD),
        maturity_5y(),
        running_coupon_bp,
    );
    let schedule_params = ScheduleParams::quarterly_act360();
    CDSTranche::new(
        "CDX_IG42_CUSTOM",
        &tranche_params,
        &schedule_params,
        finstack_core::types::CurveId::from("USD-OIS"),
        finstack_core::types::CurveId::from("CDX.NA.IG.42"),
        side,
    )
    .expect("Valid tranche parameters")
}

/// Assertion helper: check value is finite and non-negative
pub fn assert_finite_non_negative(value: f64, context: &str) {
    assert!(
        value.is_finite(),
        "{}: value should be finite, got {}",
        context,
        value
    );
    assert!(
        value >= 0.0,
        "{}: value should be non-negative, got {}",
        context,
        value
    );
}

/// Assertion helper: check value is within expected relative tolerance
pub fn assert_relative_eq(actual: f64, expected: f64, tolerance: f64, context: &str) {
    let rel_error = if expected.abs() > 1e-10 {
        ((actual - expected) / expected).abs()
    } else {
        (actual - expected).abs()
    };
    assert!(
        rel_error <= tolerance,
        "{}: relative error {} exceeds tolerance {} (actual={}, expected={})",
        context,
        rel_error,
        tolerance,
        actual,
        expected
    );
}

/// Assertion helper: check value is within absolute tolerance
pub fn assert_absolute_eq(actual: f64, expected: f64, tolerance: f64, context: &str) {
    let abs_error = (actual - expected).abs();
    assert!(
        abs_error <= tolerance,
        "{}: absolute error {} exceeds tolerance {} (actual={}, expected={})",
        context,
        abs_error,
        tolerance,
        actual,
        expected
    );
}

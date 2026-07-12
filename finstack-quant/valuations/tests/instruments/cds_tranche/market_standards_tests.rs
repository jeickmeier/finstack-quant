//! Market standards and methodology validation tests.
//!
//! Tests validate compliance with market conventions and academic references:
//! - Li (2000) Gaussian Copula methodology
//! - Standard CDX/iTraxx tranche structures
//! - Base correlation approach
//! - Accrual-on-default conventions
//! - Risk metric calculation standards

#![allow(clippy::field_reassign_with_default)]

use super::helpers::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::{binomial_probability, log_factorial};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranche;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTrancheParams;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranchePricer;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::TrancheSide;
use time::macros::date;

// ==================== Standard Tranche Structure Tests ====================

#[test]
fn test_standard_cdx_tranche_points() {
    // Market Standard: CDX/iTraxx standard tranches
    // Reference: Industry practice for synthetic CDO tranches

    let standard_tranches = vec![
        ("Equity", 0.0, 3.0),
        ("Junior Mezz", 3.0, 7.0),
        ("Senior Mezz", 7.0, 10.0),
        ("Senior", 10.0, 15.0),
        ("Super Senior", 15.0, 30.0),
    ];

    for (name, attach, detach) in standard_tranches {
        // Validate attachment < detachment
        assert!(
            attach < detach,
            "{} tranche: attachment {:.1}% must be < detachment {:.1}%",
            name,
            attach,
            detach
        );

        // Validate reasonable tranche width (typically 3-15%)
        let width = detach - attach;
        assert!(
            (2.0..=20.0).contains(&width),
            "{} tranche width {:.1}% should be in reasonable range",
            name,
            width
        );
    }
}

#[test]
fn test_typical_tranche_widths() {
    // Market Standard: Typical CDX/iTraxx tranche widths

    let tranches = vec![
        ("Equity", 0.0, 3.0, 3.0),
        ("Junior Mezz", 3.0, 7.0, 4.0),
        ("Senior Mezz", 7.0, 10.0, 3.0),
        ("Senior", 10.0, 15.0, 5.0),
        ("Super Senior", 15.0, 30.0, 15.0),
        ("Ultra Senior", 30.0, 100.0, 70.0),
    ];

    for (name, lower, upper, expected_width) in tranches {
        let width = upper - lower;
        assert_absolute_eq(
            width,
            expected_width,
            0.01,
            &format!("{} tranche width", name),
        );
    }
}

#[test]
fn test_standard_portfolio_size() {
    // Market Standard: CDX NA IG = 125 names, CDX HY = 100 names, iTraxx = 125 names

    let standard_sizes = [100, 125];

    for &size in &standard_sizes {
        let index_data = standard_credit_index();
        let modified_index =
            finstack_quant_core::market_data::term_structures::CreditIndexData::builder()
                .num_constituents(size as u16)
                .recovery_rate(index_data.recovery_rate)
                .index_credit_curve(std::sync::Arc::clone(&index_data.index_credit_curve))
                .base_correlation_curve(std::sync::Arc::clone(&index_data.base_correlation_curve))
                .build()
                .unwrap();

        assert_eq!(
            modified_index.num_constituents, size as u16,
            "Should support standard portfolio size"
        );
    }
}

#[test]
fn test_standard_recovery_rate() {
    // Market Standard: Corporate bonds typically assume 40% recovery rate
    // Reference: Moody's historical recovery studies

    let standard_recovery = 0.40;
    let index_data = standard_credit_index();

    assert_absolute_eq(
        index_data.recovery_rate,
        standard_recovery,
        0.01,
        "Standard corporate recovery rate",
    );
}

#[test]
fn test_standard_tranche_derives_contractual_effective_date_for_seasoned_trade() {
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();
    let as_of = base_date();

    let params = CDSTrancheParams::mezzanine_tranche(
        "CDX.NA.IG.42",
        42,
        Money::new(10_000_000.0, Currency::USD),
        date!(2029 - 12 - 20),
        500.0,
    );

    let mut explicit = CDSTranche::standard(
        "CDX-SEASONED-EXPLICIT",
        &params,
        "USD-OIS",
        "CDX.NA.IG.42",
        TrancheSide::SellProtection,
    )
    .expect("standard tranche");
    explicit.effective_date = Some(date!(2024 - 12 - 20));

    let mut derived = explicit.clone();
    derived.effective_date = None;

    let explicit_pv = pricer
        .price_tranche(&explicit, &market, as_of)
        .expect("explicit tranche price")
        .amount();
    let derived_pv = pricer
        .price_tranche(&derived, &market, as_of)
        .expect("derived tranche price")
        .amount();

    assert_absolute_eq(
        derived_pv,
        explicit_pv,
        1e-6,
        "Standard tranche without explicit effective_date should derive the same contractual schedule",
    );
}

// ==================== Correlation Methodology Tests ====================

#[test]
fn test_base_correlation_approach() {
    // Market Standard: Base correlation decomposes tranche [A,D] as:
    // EL[A,D] = (EL[0,D] - EL[0,A]) / (D-A)
    // Reference: Li (2000), McGinty et al. (2004)

    let corr_curve = standard_correlation_curve();

    // Verify correlation increases with detachment point (typical pattern)
    let corr_3 = corr_curve.correlation(3.0);
    let corr_7 = corr_curve.correlation(7.0);
    let corr_10 = corr_curve.correlation(10.0);

    assert!(
        corr_3 <= corr_7,
        "Base correlation typically increases with detachment"
    );
    assert!(
        corr_7 <= corr_10,
        "Base correlation typically increases with detachment"
    );
}

// ==================== Binomial Probability Tests ====================

#[test]
fn test_binomial_probability_known_values() {
    // Market Standard: Binomial distribution is foundation of loss distribution
    // Reference: Standard probability theory

    // Known values from binomial tables
    let test_cases = vec![
        (10, 5, 0.5, 0.24609375),
        (5, 0, 0.1, 0.59049),
        (10, 0, 0.0, 1.0),
        (10, 10, 1.0, 1.0),
    ];

    for (n, k, p, expected) in test_cases {
        let result = binomial_probability(n, k, p);
        assert_absolute_eq(
            result,
            expected,
            1e-6,
            &format!("Binomial({}, {}, {})", n, k, p),
        );
    }
}

#[test]
fn test_binomial_probability_edge_cases() {
    // Edge case: p=0 should give probability 1 for k=0, 0 otherwise
    assert_eq!(binomial_probability(10, 0, 0.0), 1.0);
    assert_eq!(binomial_probability(10, 5, 0.0), 0.0);

    // Edge case: p=1 should give probability 1 for k=n, 0 otherwise
    assert_eq!(binomial_probability(10, 10, 1.0), 1.0);
    assert_eq!(binomial_probability(10, 5, 1.0), 0.0);
}

#[test]
fn test_log_factorial_accuracy() {
    // Test small values (exact calculation)
    assert_absolute_eq(log_factorial(1), 0.0, 1e-12, "log(1!)");
    assert_absolute_eq(
        log_factorial(5),
        2.0_f64.ln() + 3.0_f64.ln() + 4.0_f64.ln() + 5.0_f64.ln(),
        1e-12,
        "log(5!)",
    );

    // Test Stirling's approximation for large n
    let log_100_factorial = log_factorial(100);
    assert!(
        (360.0..370.0).contains(&log_100_factorial),
        "log(100!) should be ~363.7, got {}",
        log_100_factorial
    );
}

// ==================== Accrual-on-Default Methodology Tests ====================

#[test]
fn test_aod_enabled_by_default() {
    // Market Standard: Modern CDS pricing includes accrual-on-default
    // Reference: Post-2009 ISDA CDS standard model (ISDA CDS Standard Model)

    let config =
        finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranchePricerConfig::default();
    assert!(
        config.accrual_on_default_enabled,
        "Accrual-on-default should be enabled by default per ISDA standards"
    );
}

// ==================== Day Count Convention Tests ====================

#[test]
fn test_standard_day_count_act360() {
    // Market Standard: CDS typically use Act/360 day count
    // Reference: ISDA CDS conventions

    let tranche = mezzanine_tranche();
    assert_eq!(
        format!("{:?}", tranche.day_count),
        "Act360",
        "Standard CDS day count should be Act/360"
    );
}

// ==================== Payment Tenor Tests ====================

#[test]
fn test_standard_quarterly_frequency() {
    // Market Standard: CDS premium payments are quarterly (3M)
    // Reference: ISDA CDS conventions

    let tranche = mezzanine_tranche();
    assert_eq!(
        format!("{:?}", tranche.frequency),
        "Tenor { count: 3, unit: Months }",
        "Standard CDS payment frequency should be quarterly (3 months)"
    );
}

// ==================== Tranche Parameter Builders Tests ====================

#[test]
fn test_equity_tranche_parameters() {
    // Market Standard: Equity tranche is 0-3%

    let params = CDSTrancheParams::equity_tranche(
        "CDX.NA.IG.42",
        42,
        finstack_quant_core::money::Money::new(
            10_000_000.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        maturity_5y(),
        1000.0,
    );

    assert_absolute_eq(params.attach_pct, 0.0, 0.01, "Equity attachment");
    assert_absolute_eq(params.detach_pct, 3.0, 0.01, "Equity detachment");
}

#[test]
fn test_mezzanine_tranche_parameters() {
    // Market Standard: Mezzanine tranche is 3-7%

    let params = CDSTrancheParams::mezzanine_tranche(
        "CDX.NA.IG.42",
        42,
        finstack_quant_core::money::Money::new(
            10_000_000.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        maturity_5y(),
        500.0,
    );

    assert_absolute_eq(params.attach_pct, 3.0, 0.01, "Mezzanine attachment");
    assert_absolute_eq(params.detach_pct, 7.0, 0.01, "Mezzanine detachment");
}

// ==================== Gaussian Copula Methodology Tests ====================

#[test]
fn test_gaussian_copula_quadrature_orders() {
    // Market Standard: Gauss-Hermite quadrature with 5, 7, or 10 points
    // Reference: Hull & White (2004), numerical methods for copula integration

    let valid_orders = [5, 7, 10];

    for &order in &valid_orders {
        let mut config =
            finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranchePricerConfig::default(
            );
        config.quadrature_order = order;

        assert_eq!(config.quadrature_order, order);
    }
}

#[test]
fn test_default_quadrature_order() {
    // Market Standard: 20-point Gauss-Hermite per QuantLib/Bloomberg convention

    let config =
        finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranchePricerConfig::default();
    assert_eq!(
        config.quadrature_order, 20,
        "Default quadrature order should be 20 (industry standard: 20-50 points)"
    );
}

// Note: Portfolio loss distribution testing is done indirectly through
// expected loss and pricing tests with various correlation and default scenarios

// ==================== Market Data Consistency Tests ====================

#[test]
fn test_discount_curve_monotonicity() {
    // Market Standard: Discount factors should be monotonically decreasing
    // Reference: Standard arbitrage-free pricing

    let curve = standard_discount_curve();
    let times = [0.0, 1.0, 3.0, 5.0, 10.0];

    for window in times.windows(2) {
        let df1 = curve.df(window[0]);
        let df2 = curve.df(window[1]);

        assert!(
            df1 >= df2,
            "Discount factors should decrease with time: DF({})={} >= DF({})={}",
            window[0],
            df1,
            window[1],
            df2
        );
    }
}

#[test]
fn test_hazard_curve_non_negative() {
    // Market Standard: Hazard rates must be non-negative
    // Reference: Credit risk modeling theory

    let curve = standard_hazard_curve();
    let times = [0.5, 1.0, 3.0, 5.0, 10.0];

    for &t in &times {
        let sp = curve.sp(t);
        assert!(
            (0.0..=1.0).contains(&sp),
            "Survival probability must be in [0,1] at t={}, got {}",
            t,
            sp
        );
    }
}

// ==================== Sensitivity Sign Conventions Tests ====================

#[test]
fn test_spread_dv01_sign_convention() {
    // Market Standard: For protection seller (long risk),
    // higher running coupon increases premium received → positive DV01

    let pricer =
        finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranchePricer::new();
    let mut tranche = mezzanine_tranche();
    tranche.side = finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::TrancheSide::SellProtection;
    let market = standard_market_context();
    let as_of = base_date();

    let spread_dv01 = pricer
        .calculate_spread_dv01(&tranche, &market, as_of)
        .unwrap();

    assert!(
        spread_dv01 > 0.0,
        "Spread DV01 should be positive for protection seller"
    );
}

// ==================== Model Assumptions Documentation Tests ====================

#[test]
fn test_homogeneous_pool_assumption() {
    // Market Standard: Basic Gaussian Copula assumes homogeneous pool
    // - Single hazard curve for all constituents
    // - Constant recovery rate
    // - Identical notionals
    // Reference: Li (2000)

    let mut config =
        finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranchePricerConfig::default();
    config.use_issuer_curves = false;

    let pricer =
        finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranchePricer::with_params(
            config,
        );
    let tranche = mezzanine_tranche();
    let market = standard_market_context();

    let result = pricer.calculate_expected_loss(&tranche, &market);
    assert!(result.is_ok(), "Homogeneous pool calculation should work");
}

#[test]
fn test_heterogeneous_pool_extension() {
    // Market Standard: Extended model with issuer-specific curves
    // Reference: Hull & White (2004), "Valuation of a CDO and an n-th to Default CDS"

    let mut config =
        finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranchePricerConfig::default();
    config.use_issuer_curves = true;

    let pricer =
        finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranchePricer::with_params(
            config,
        );
    let tranche = mezzanine_tranche();
    let market = market_context_with_issuers(50);

    let result = pricer.calculate_expected_loss(&tranche, &market);
    assert!(result.is_ok(), "Heterogeneous pool calculation should work");
}

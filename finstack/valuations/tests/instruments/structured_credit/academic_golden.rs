//! Academic and industry-standard golden tests for structured credit.
//!
//! These tests validate our implementations against known reference values from:
//! - PSA standard curve (Public Securities Association, 1985)
//! - SDA standard curve (Standard Default Assumption)
//! - Moody's WARF rating factors
//! - Fabozzi "Fixed Income Mathematics" reference calculations
//!
//! # References
//!
//! - Fabozzi, F.J. (2006). "Fixed Income Mathematics", 4th ed., McGraw-Hill.
//! - PSA (Public Securities Association). "Standard Prepayment Model", 1985.
//! - Moody's Investors Service. "WARF®: Measuring Portfolio Credit Risk in CLOs."

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_core::types::{moodys_warf_factor, CreditRating};
use finstack_valuations::instruments::fixed_income::structured_credit::{
    cdr_to_mdr, cpr_to_smm, mdr_to_cdr, psa_to_cpr, DealType, Pool, PoolAsset,
};
use time::Month;

// ============================================================================
// Market-Standard Tolerances
// ============================================================================

/// Tolerance for rate conversions (mathematically exact within f64)
const RATE_TOLERANCE: f64 = 1e-10;

/// Tolerance for calculated metrics (allow for accumulation error)
const METRIC_TOLERANCE: f64 = 1e-6;

/// Tolerance for WAL calculations (0.01 years = ~3.65 days)
const WAL_TOLERANCE: f64 = 0.01;

// ============================================================================
// PSA Model Golden Tests
// Reference: PSA Standard Prepayment Model (1985)
// ============================================================================

#[test]
fn test_psa_golden_100_percent() {
    // 100% PSA standard curve reference values
    // Source: PSA (Public Securities Association) standard, 1985

    // PSA ramp: 0.2% CPR per month for first 30 months
    // Terminal: 6% CPR from month 30 onwards

    // Month 1: 0.2% CPR
    assert!(
        (psa_to_cpr(1.0, 1) - 0.002).abs() < RATE_TOLERANCE,
        "100% PSA month 1 should be exactly 0.2% CPR"
    );

    // Month 15: 3.0% CPR (halfway through ramp)
    assert!(
        (psa_to_cpr(1.0, 15) - 0.03).abs() < RATE_TOLERANCE,
        "100% PSA month 15 should be exactly 3.0% CPR"
    );

    // Month 30: 6.0% CPR (terminal rate)
    assert!(
        (psa_to_cpr(1.0, 30) - 0.06).abs() < RATE_TOLERANCE,
        "100% PSA month 30 should be exactly 6.0% CPR"
    );

    // Month 60: 6.0% CPR (flat after ramp)
    assert!(
        (psa_to_cpr(1.0, 60) - 0.06).abs() < RATE_TOLERANCE,
        "100% PSA month 60 should remain at 6.0% CPR"
    );

    // Month 120: 6.0% CPR (flat after ramp)
    assert!(
        (psa_to_cpr(1.0, 120) - 0.06).abs() < RATE_TOLERANCE,
        "100% PSA month 120 should remain at 6.0% CPR"
    );
}

#[test]
fn test_psa_golden_multiples() {
    // PSA speed multiples scale linearly
    // Reference: Industry convention

    let speeds = [0.5, 1.0, 1.5, 2.0, 2.5, 3.0];
    let month = 30; // Terminal month

    for speed in speeds {
        let expected_cpr = speed * 0.06; // speed × 6% terminal CPR
        let actual_cpr = psa_to_cpr(speed, month);

        assert!(
            (actual_cpr - expected_cpr).abs() < RATE_TOLERANCE,
            "{}% PSA at month {} should be {}% CPR, got {}%",
            speed * 100.0,
            month,
            expected_cpr * 100.0,
            actual_cpr * 100.0
        );
    }
}

#[test]
fn test_psa_golden_smm_conversion() {
    // SMM conversion for 100% PSA at terminal rate
    // Reference: SMM = 1 - (1 - CPR)^(1/12)

    // 6% CPR → SMM = 1 - 0.94^(1/12) = 0.51430...%
    let cpr: f64 = 0.06;
    let expected_smm = 1.0 - (1.0 - cpr).powf(1.0 / 12.0);
    let actual_smm = cpr_to_smm(cpr);

    assert!(
        (actual_smm - expected_smm).abs() < RATE_TOLERANCE,
        "6% CPR should convert to SMM = {}, got {}",
        expected_smm,
        actual_smm
    );

    // Verify exact value: 0.0051430...
    assert!(
        (actual_smm - 0.005143).abs() < 0.000001,
        "6% CPR SMM should be approximately 0.5143%"
    );
}

// ============================================================================
// CDR/MDR Conversion Golden Tests
// Reference: Industry standard formulas
// ============================================================================

#[test]
fn test_cdr_mdr_golden_conversion() {
    // MDR = 1 - (1 - CDR)^(1/12)
    // Reference: Fabozzi, Fixed Income Mathematics

    let test_cases: [(f64, &str); 5] = [
        (0.01, "1% CDR"),  // Low default
        (0.02, "2% CDR"),  // Standard CLO
        (0.05, "5% CDR"),  // High default
        (0.10, "10% CDR"), // Stressed
        (0.20, "20% CDR"), // Severely distressed
    ];

    for (cdr, label) in test_cases {
        let expected_mdr = 1.0 - (1.0 - cdr).powf(1.0 / 12.0);
        let actual_mdr = cdr_to_mdr(cdr);

        assert!(
            (actual_mdr - expected_mdr).abs() < RATE_TOLERANCE,
            "{}: MDR should be {}, got {}",
            label,
            expected_mdr,
            actual_mdr
        );

        // Verify roundtrip
        let cdr_back = mdr_to_cdr(actual_mdr);
        assert!(
            (cdr_back - cdr).abs() < RATE_TOLERANCE,
            "{}: roundtrip failed: {} -> {} -> {}",
            label,
            cdr,
            actual_mdr,
            cdr_back
        );
    }
}

#[test]
fn test_cdr_mdr_golden_2_percent() {
    // 2% CDR is standard for CLO modeling
    // Reference: Moody's CLO methodology

    let cdr = 0.02;
    let mdr = cdr_to_mdr(cdr);

    // Expected: 1 - 0.98^(1/12) = 0.0016833...
    let expected = 1.0 - 0.98_f64.powf(1.0 / 12.0);

    assert!(
        (mdr - expected).abs() < RATE_TOLERANCE,
        "2% CDR should give MDR = {}, got {}",
        expected,
        mdr
    );

    // Verify approximately 0.168% monthly (0.001683...)
    // The exact value is 0.00168334..., not 0.001679
    assert!(
        (mdr - 0.001683).abs() < 0.00001,
        "2% CDR MDR should be approximately 0.168% (got {})",
        mdr
    );
}

// ============================================================================
// Moody's WARF Golden Tests
// Reference: Moody's "Approach to Rating Collateralized Loan Obligations"
// ============================================================================

#[test]
fn test_warf_golden_moody_standard() {
    // Moody's IDEALIZED DEFAULT RATES table
    // Source: Moody's CLO Methodology

    let test_cases = [
        (CreditRating::AAA, 1.0, "AAA / Aaa"),
        (CreditRating::AAPlus, 10.0, "AA+ / Aa1"),
        (CreditRating::AA, 20.0, "AA / Aa2"),
        (CreditRating::AAMinus, 40.0, "AA- / Aa3"),
        (CreditRating::APlus, 70.0, "A+ / A1"),
        (CreditRating::A, 120.0, "A / A2"),
        (CreditRating::AMinus, 180.0, "A- / A3"),
        (CreditRating::BBBPlus, 260.0, "BBB+ / Baa1"),
        (CreditRating::BBB, 360.0, "BBB / Baa2"),
        (CreditRating::BBBMinus, 610.0, "BBB- / Baa3"),
        (CreditRating::BBPlus, 940.0, "BB+ / Ba1"),
        (CreditRating::BB, 1350.0, "BB / Ba2"),
        (CreditRating::BBMinus, 1760.0, "BB- / Ba3"),
        (CreditRating::BPlus, 2220.0, "B+ / B1"),
        (CreditRating::B, 2720.0, "B / B2"),
        (CreditRating::BMinus, 3490.0, "B- / B3"),
        (CreditRating::CCCPlus, 4770.0, "CCC+ / Caa1"),
        (CreditRating::CCC, 6500.0, "CCC / Caa2"),
        (CreditRating::CCCMinus, 8070.0, "CCC- / Caa3"),
        (CreditRating::CC, 9550.0, "CC / Ca"),
        (CreditRating::C, 10000.0, "C"),
        (CreditRating::D, 10000.0, "D"),
    ];

    for (rating, expected_factor, label) in test_cases {
        let actual_factor = moodys_warf_factor(rating).unwrap();

        assert_eq!(
            actual_factor, expected_factor,
            "Moody's WARF factor for {} should be {}, got {}",
            label, expected_factor, actual_factor
        );
    }
}

#[test]
fn test_warf_golden_pool_calculation() {
    // WARF calculation for a sample CLO pool
    // Reference: Moody's WARF methodology
    //
    // Pool composition:
    // - 20% BB rated (factor 1350)
    // - 60% B rated (factor 2720)
    // - 20% CCC rated (factor 6500)
    //
    // Expected WARF = 0.20 × 1350 + 0.60 × 2720 + 0.20 × 6500
    //              = 270 + 1632 + 1300 = 3202

    let expected_warf: f64 = 0.20 * 1350.0 + 0.60 * 2720.0 + 0.20 * 6500.0;

    // Verify expected calculation
    assert!(
        (expected_warf - 3202.0).abs() < METRIC_TOLERANCE,
        "Expected WARF calculation should be 3202"
    );

    // Verify using our rating factors
    let calculated_warf = 0.20 * moodys_warf_factor(CreditRating::BB).unwrap()
        + 0.60 * moodys_warf_factor(CreditRating::B).unwrap()
        + 0.20 * moodys_warf_factor(CreditRating::CCC).unwrap();

    assert!(
        (calculated_warf - expected_warf).abs() < METRIC_TOLERANCE,
        "WARF from rating factors should be {}, got {}",
        expected_warf,
        calculated_warf
    );
}

// ============================================================================
// WAL Golden Tests
// Reference: Fabozzi "Fixed Income Mathematics", Chapter 6
// ============================================================================

#[test]
fn test_wal_golden_uniform_amortization() {
    // WAL for uniform amortization (equal principal payments)
    // Reference: Fabozzi Example 6.1
    //
    // For n equal principal payments at times t1, t2, ..., tn:
    // WAL = (t1 + t2 + ... + tn) / n = (n + 1) / 2 when ti = i
    //
    // For 4 annual payments (years 1, 2, 3, 4):
    // WAL = (1 + 2 + 3 + 4) / 4 = 2.5 years

    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let pool = Pool::new("UNIFORM_POOL", DealType::ABS, Currency::USD);

    let cashflows = vec![
        (
            Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            Money::new(25_000.0, Currency::USD),
        ), // Year 1
        (
            Date::from_calendar_date(2027, Month::January, 1).unwrap(),
            Money::new(25_000.0, Currency::USD),
        ), // Year 2
        (
            Date::from_calendar_date(2028, Month::January, 1).unwrap(),
            Money::new(25_000.0, Currency::USD),
        ), // Year 3
        (
            Date::from_calendar_date(2029, Month::January, 1).unwrap(),
            Money::new(25_000.0, Currency::USD),
        ), // Year 4
    ];

    let wal = pool.weighted_avg_life_from_cashflows(&cashflows, as_of);

    // Expected WAL = (1 + 2 + 3 + 4) / 4 = 2.5 years
    let expected_wal = 2.5;

    assert!(
        (wal - expected_wal).abs() < WAL_TOLERANCE,
        "Uniform amortization WAL should be {} years, got {}",
        expected_wal,
        wal
    );
}

#[test]
fn test_wal_golden_front_loaded() {
    // WAL for front-loaded amortization
    // Reference: Fabozzi Example 6.2
    //
    // Principal: 70% year 1, 20% year 2, 10% year 3
    // WAL = 0.70 × 1 + 0.20 × 2 + 0.10 × 3 = 0.70 + 0.40 + 0.30 = 1.4 years

    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let pool = Pool::new("FRONT_LOADED", DealType::ABS, Currency::USD);

    let total_principal = 100_000.0;
    let cashflows = vec![
        (
            Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            Money::new(total_principal * 0.70, Currency::USD),
        ), // Year 1: 70%
        (
            Date::from_calendar_date(2027, Month::January, 1).unwrap(),
            Money::new(total_principal * 0.20, Currency::USD),
        ), // Year 2: 20%
        (
            Date::from_calendar_date(2028, Month::January, 1).unwrap(),
            Money::new(total_principal * 0.10, Currency::USD),
        ), // Year 3: 10%
    ];

    let wal = pool.weighted_avg_life_from_cashflows(&cashflows, as_of);

    // Expected WAL = 0.70 × 1 + 0.20 × 2 + 0.10 × 3 = 1.4 years
    let expected_wal = 0.70 * 1.0 + 0.20 * 2.0 + 0.10 * 3.0;

    assert!(
        (wal - expected_wal).abs() < WAL_TOLERANCE,
        "Front-loaded WAL should be {} years, got {}",
        expected_wal,
        wal
    );
}

#[test]
fn test_wal_golden_back_loaded() {
    // WAL for back-loaded amortization
    // Reference: Fabozzi Example 6.3
    //
    // Principal: 10% year 1, 20% year 2, 70% year 3
    // WAL = 0.10 × 1 + 0.20 × 2 + 0.70 × 3 = 0.10 + 0.40 + 2.10 = 2.6 years

    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let pool = Pool::new("BACK_LOADED", DealType::ABS, Currency::USD);

    let total_principal = 100_000.0;
    let cashflows = vec![
        (
            Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            Money::new(total_principal * 0.10, Currency::USD),
        ), // Year 1: 10%
        (
            Date::from_calendar_date(2027, Month::January, 1).unwrap(),
            Money::new(total_principal * 0.20, Currency::USD),
        ), // Year 2: 20%
        (
            Date::from_calendar_date(2028, Month::January, 1).unwrap(),
            Money::new(total_principal * 0.70, Currency::USD),
        ), // Year 3: 70%
    ];

    let wal = pool.weighted_avg_life_from_cashflows(&cashflows, as_of);

    // Expected WAL = 0.10 × 1 + 0.20 × 2 + 0.70 × 3 = 2.6 years
    let expected_wal = 0.10 * 1.0 + 0.20 * 2.0 + 0.70 * 3.0;

    assert!(
        (wal - expected_wal).abs() < WAL_TOLERANCE,
        "Back-loaded WAL should be {} years, got {}",
        expected_wal,
        wal
    );
}

// ============================================================================
// WAS (Weighted Average Spread) Golden Tests
// Reference: CLO Industry Standard
// ============================================================================

#[test]
fn test_was_golden_calculation() {
    // WAS calculation for a sample CLO pool
    // Reference: Industry standard weighted average
    //
    // Pool composition:
    // - $50M at SOFR + 400 bps
    // - $30M at SOFR + 450 bps
    // - $20M at SOFR + 500 bps
    //
    // Expected WAS = (50 × 400 + 30 × 450 + 20 × 500) / 100 = 43,500 / 100 = 435 bps

    let maturity = Date::from_calendar_date(2030, Month::December, 31).unwrap();

    let mut pool = Pool::new("WAS_TEST", DealType::CLO, Currency::USD);

    pool.assets.push(
        PoolAsset::floating_rate_loan(
            "LOAN1",
            Money::new(50_000_000.0, Currency::USD),
            "SOFR-3M",
            400.0,
            maturity,
            finstack_core::dates::DayCount::Act360,
        )
        .with_rating(CreditRating::BB),
    );

    pool.assets.push(
        PoolAsset::floating_rate_loan(
            "LOAN2",
            Money::new(30_000_000.0, Currency::USD),
            "SOFR-3M",
            450.0,
            maturity,
            finstack_core::dates::DayCount::Act360,
        )
        .with_rating(CreditRating::B),
    );

    pool.assets.push(
        PoolAsset::floating_rate_loan(
            "LOAN3",
            Money::new(20_000_000.0, Currency::USD),
            "SOFR-3M",
            500.0,
            maturity,
            finstack_core::dates::DayCount::Act360,
        )
        .with_rating(CreditRating::B),
    );

    let was = pool.weighted_avg_spread();

    // Expected WAS = (50×400 + 30×450 + 20×500) / 100 = 435 bps
    let expected_was =
        (50_000_000.0 * 400.0 + 30_000_000.0 * 450.0 + 20_000_000.0 * 500.0) / 100_000_000.0;

    assert!(
        (was - expected_was).abs() < METRIC_TOLERANCE,
        "WAS should be {} bps, got {} bps",
        expected_was,
        was
    );

    // Verify exact value
    assert!(
        (was - 435.0).abs() < METRIC_TOLERANCE,
        "WAS should be 435 bps"
    );
}

// ============================================================================
// Recovery Rate Golden Tests
// Reference: S&P and Moody's historical data
// ============================================================================

#[test]
fn test_recovery_rate_golden_industry_standards() {
    // Industry standard recovery rates by asset class
    // Reference: Moody's Default Study, S&P Recovery Studies
    //
    // These are the standard assumptions used in modeling:

    use finstack_valuations::instruments::fixed_income::structured_credit::{
        abs_auto_standard_recovery, clo_standard_recovery, cmbs_standard_recovery,
        rmbs_standard_recovery,
    };

    // CLO (Senior Secured Loans): ~40% recovery
    // Source: Moody's "Annual Default Study"
    assert_eq!(
        clo_standard_recovery(),
        0.40,
        "CLO standard recovery should be 40%"
    );

    // RMBS (Residential Mortgages): ~60% recovery
    // Source: S&P RMBS methodology
    assert_eq!(
        rmbs_standard_recovery(),
        0.60,
        "RMBS standard recovery should be 60%"
    );

    // Auto ABS: ~45% recovery
    // Source: S&P Auto ABS methodology
    assert_eq!(
        abs_auto_standard_recovery(),
        0.45,
        "Auto ABS standard recovery should be 45%"
    );

    // CMBS (Commercial Mortgages): ~65% recovery
    // Source: Moody's CMBS methodology
    assert_eq!(
        cmbs_standard_recovery(),
        0.65,
        "CMBS standard recovery should be 65%"
    );
}

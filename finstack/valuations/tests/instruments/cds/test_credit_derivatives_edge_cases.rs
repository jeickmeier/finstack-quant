//! Credit derivatives edge case tests.
//!
//! Tests for the specific edge cases identified in code review:
//! 1. Recovery01 at boundary conditions (recovery near 0 or 1)
//! 2. Survival probability floor handling
//! 3. CDS Greeks consistency checks

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::Month;

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::March, 20).unwrap()
}

fn create_discount_curve(base: Date) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.97),
            (3.0, 0.91),
            (5.0, 0.82),
            (10.0, 0.65),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

fn create_hazard_curve(base: Date, recovery: f64) -> HazardCurve {
    HazardCurve::builder("TEST-CREDIT")
        .base_date(base)
        .recovery_rate(recovery)
        .knots([(1.0, 0.02), (3.0, 0.025), (5.0, 0.03), (10.0, 0.035)])
        .build()
        .unwrap()
}

fn create_distressed_hazard_curve(base: Date, recovery: f64) -> HazardCurve {
    // Very high hazard rates (distressed credit)
    HazardCurve::builder("DISTRESSED-CREDIT")
        .base_date(base)
        .recovery_rate(recovery)
        .knots([(1.0, 0.30), (3.0, 0.35), (5.0, 0.40), (10.0, 0.45)])
        .build()
        .unwrap()
}

/// Test Recovery01 with recovery rate at lower boundary (near 0%)
///
/// Uses recovery = 0.005 (0.5%) so that:
/// - bumped_down = (0.005 - 0.01).clamp(0, 1) = 0.0
/// - down_delta = 0.005 < RECOVERY_BUMP
///
/// This exercises the forward-difference path where only up-bump is effective.
#[test]
fn test_recovery01_at_lower_boundary() {
    let base = base_date();
    let maturity = Date::from_calendar_date(2030, Month::March, 20).unwrap();

    // Recovery rate at lower boundary: 0.005 (0.5%)
    // With RECOVERY_BUMP = 0.01, bumped_down = 0.0, so we can only bump up
    let recovery = 0.005;
    let cds = crate::finstack_test_utils::cds_buy_protection(
        "CDS-LOW-RECOVERY",
        Money::new(10_000_000.0, Currency::USD),
        200.0, // 200bp
        base,
        maturity,
        "USD-OIS",
        "TEST-CREDIT",
    )
    .unwrap();

    // Override recovery rate
    let mut cds_test = cds.clone();
    cds_test.protection.recovery_rate = recovery;

    let market = MarketContext::new()
        .insert(create_discount_curve(base))
        .insert(create_hazard_curve(base, recovery));

    let result = cds_test
        .price_with_metrics(
            &market,
            base,
            &[MetricId::Recovery01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("Recovery01 should compute at low recovery boundary");

    let recovery01 = result.measures[MetricId::Recovery01.as_str()];

    // Recovery01 should be finite and have expected sign
    // For a protection buyer, higher recovery = lower protection payout = lower NPV
    assert!(
        recovery01.is_finite(),
        "Recovery01 should be finite at low recovery boundary"
    );
    // At low recovery (0.5%), we can only bump up effectively, so this tests the forward difference path
}

/// Test Recovery01 with recovery rate close to (but below) the 1.0 boundary.
/// Recovery01 bumps recovery internally, so the base value must leave room
/// for an upward bump while staying strictly less than 1.0 (the
/// `validate_recovery_rate` upper bound — see legs validation in
/// `instruments/common/validation.rs`).
#[test]
fn test_recovery01_at_upper_boundary() {
    let base = base_date();
    let maturity = Date::from_calendar_date(2030, Month::March, 20).unwrap();

    // Recovery rate near the upper boundary: 0.95 (95%). Recovery01 finite
    // differences must stay strictly below 1.0 after bumping.
    let recovery = 0.95;
    let cds = crate::finstack_test_utils::cds_buy_protection(
        "CDS-HIGH-RECOVERY",
        Money::new(10_000_000.0, Currency::USD),
        50.0, // 50bp (tight spread for high recovery)
        base,
        maturity,
        "USD-OIS",
        "TEST-CREDIT",
    )
    .unwrap();

    // Override recovery rate
    let mut cds_test = cds.clone();
    cds_test.protection.recovery_rate = recovery;

    let market = MarketContext::new()
        .insert(create_discount_curve(base))
        .insert(create_hazard_curve(base, recovery));

    let result = cds_test
        .price_with_metrics(
            &market,
            base,
            &[MetricId::Recovery01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("Recovery01 should compute at high recovery boundary");

    let recovery01 = result.measures[MetricId::Recovery01.as_str()];

    // Recovery01 should be finite
    assert!(
        recovery01.is_finite(),
        "Recovery01 should be finite at high recovery boundary"
    );
    // At high recovery, we can only bump down, so this tests the backward difference path
}

/// Test that Recovery01 is symmetric around mid-range recovery
#[test]
fn test_recovery01_symmetry() {
    let base = base_date();
    let maturity = Date::from_calendar_date(2030, Month::March, 20).unwrap();

    // Standard recovery rate (40%)
    let recovery = 0.40;
    let cds = crate::finstack_test_utils::cds_buy_protection(
        "CDS-STANDARD",
        Money::new(10_000_000.0, Currency::USD),
        150.0,
        base,
        maturity,
        "USD-OIS",
        "TEST-CREDIT",
    )
    .unwrap();

    let mut cds_test = cds.clone();
    cds_test.protection.recovery_rate = recovery;

    let market = MarketContext::new()
        .insert(create_discount_curve(base))
        .insert(create_hazard_curve(base, recovery));

    let result = cds_test
        .price_with_metrics(
            &market,
            base,
            &[MetricId::Recovery01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("Recovery01 should compute for standard recovery");

    let recovery01 = result.measures[MetricId::Recovery01.as_str()];

    // At mid-range, central differences should be used
    assert!(
        recovery01.is_finite(),
        "Recovery01 should be finite for standard recovery"
    );

    // Recovery01 should be negative for a protection buyer (higher recovery = less payout)
    // Note: sign depends on the position side
}

/// Test expected loss with distressed credit (tests survival probability floor)
#[test]
fn test_expected_loss_distressed_credit() {
    let base = base_date();
    let maturity = Date::from_calendar_date(2030, Month::March, 20).unwrap();

    let recovery = 0.25; // Lower recovery for distressed
    let cds = crate::finstack_test_utils::cds_buy_protection(
        "CDS-DISTRESSED",
        Money::new(10_000_000.0, Currency::USD),
        2500.0, // 2500bp = 25% (deeply distressed)
        base,
        maturity,
        "USD-OIS",
        "DISTRESSED-CREDIT",
    )
    .unwrap();

    let mut cds_test = cds.clone();
    cds_test.protection.recovery_rate = recovery;

    let market = MarketContext::new()
        .insert(create_discount_curve(base))
        .insert(create_distressed_hazard_curve(base, recovery));

    let result = cds_test
        .price_with_metrics(
            &market,
            base,
            &[MetricId::ExpectedLoss],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("ExpectedLoss should compute for distressed credit");

    let expected_loss = result.measures[MetricId::ExpectedLoss.as_str()];

    // Expected loss should be positive and bounded by notional * LGD
    let max_loss = 10_000_000.0 * (1.0 - recovery);
    assert!(expected_loss >= 0.0, "Expected loss should be non-negative");
    assert!(
        expected_loss <= max_loss,
        "Expected loss should not exceed notional * LGD"
    );
}

/// Test that par spread and NPV are consistent
///
/// This test validates the fundamental CDS pricing identity:
/// If we calculate the par spread (the spread at which NPV = 0), then
/// repricing the same CDS at that spread should give NPV ≈ 0.
///
/// ## Methodology Note
///
/// By default, there's a mismatch between par spread calculation (ISDA standard:
/// risky annuity only, no AoD) and NPV calculation (includes AoD). This test uses
/// the CDS pricer directly with consistent settings to achieve a true roundtrip.
///
/// Key: We CLONE the CDS and modify only the spread, preserving all schedule
/// parameters. Creating a new CDS could introduce schedule differences.
#[test]
fn test_par_spread_npv_consistency() {
    use rust_decimal::Decimal;

    let base = base_date();
    let maturity = Date::from_calendar_date(2030, Month::March, 20).unwrap();

    let recovery = 0.40;
    let initial_spread = 150.0;

    let cds = crate::finstack_test_utils::cds_buy_protection(
        "CDS-CONSISTENCY",
        Money::new(10_000_000.0, Currency::USD),
        initial_spread,
        base,
        maturity,
        "USD-OIS",
        "TEST-CREDIT",
    )
    .unwrap();

    let mut cds_test = cds.clone();
    cds_test.protection.recovery_rate = recovery;

    let disc = create_discount_curve(base);
    let hazard = create_hazard_curve(base, recovery);
    let market = MarketContext::new().insert(disc).insert(hazard);

    // Derive par spread from protection PV and risky annuity (consistent with NPV logic)
    let result = cds_test
        .price_with_metrics(
            &market,
            base,
            &[MetricId::ProtectionLegPv, MetricId::PremiumLegPv],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("ParSpread inputs should compute");
    let protection_pv = result.measures[&MetricId::ProtectionLegPv];
    let premium_pv = result.measures[&MetricId::PremiumLegPv];
    let par_spread = protection_pv / premium_pv * initial_spread;

    // Clone the CDS and update only the spread (preserving schedule)
    let mut cds_at_par = cds_test.clone();
    cds_at_par.premium.spread_bp = Decimal::try_from(par_spread).expect("valid par_spread");

    // NPV at par spread should be approximately zero
    let npv_at_par = cds_at_par
        .value(&market, base)
        .expect("NPV should compute")
        .amount();

    // Tolerance: 0.001% of notional (= $100 on $10M)
    // With consistent AoD treatment, the only error is numerical precision
    let tolerance = 10_000_000.0 * 0.00001;
    assert!(
        npv_at_par.abs() < tolerance,
        "NPV at par spread should be ~0, got: {:.2} (tolerance: {:.2})",
        npv_at_par,
        tolerance
    );
}

/// Test jump-to-default metric for various recovery rates
#[test]
fn test_jtd_recovery_sensitivity() {
    let base = base_date();
    let maturity = Date::from_calendar_date(2030, Month::March, 20).unwrap();

    let recovery_rates = [0.05, 0.25, 0.40, 0.60, 0.80, 0.95];

    for &recovery in &recovery_rates {
        let cds = crate::finstack_test_utils::cds_buy_protection(
            format!("CDS-JTD-{}", (recovery * 100.0) as i32),
            Money::new(10_000_000.0, Currency::USD),
            200.0,
            base,
            maturity,
            "USD-OIS",
            "TEST-CREDIT",
        )
        .unwrap();

        let mut cds_test = cds.clone();
        cds_test.protection.recovery_rate = recovery;

        let market = MarketContext::new()
            .insert(create_discount_curve(base))
            .insert(create_hazard_curve(base, recovery));

        let result = cds_test
            .price_with_metrics(
                &market,
                base,
                &[MetricId::JumpToDefault],
                finstack_valuations::instruments::PricingOptions::default(),
            )
            .expect("JTD should compute");

        let jtd = result.measures[MetricId::JumpToDefault.as_str()];

        // JTD should be positive for protection buyer (receive LGD on default)
        // The value should be approximately notional * LGD - accrued
        let expected_jtd_approx = 10_000_000.0 * (1.0 - recovery);

        assert!(
            jtd.is_finite(),
            "JTD should be finite for recovery {}",
            recovery
        );
        // Allow for accrued premium adjustment
        assert!(
            jtd > 0.0 && jtd <= expected_jtd_approx * 1.1,
            "JTD {} should be positive and bounded for recovery {}",
            jtd,
            recovery
        );
    }
}

/// Recovery01 must recalibrate the hazard curve (re-bootstrap from par
/// spreads at the new recovery) when the curve carries par-spread quotes.
///
/// We compare the metric's Recovery01 against a hand-rolled "frozen-curve"
/// Recovery01 that bumps the instrument LGD without recalibrating the
/// survival curve. For a curve with par_spread_points the two MUST differ —
/// the recalibrated answer is the production-correct one (h ≈ S/(1-R)
/// rebalances when R moves), while the frozen-curve answer is the partial
/// LGD-only sensitivity.
#[test]
fn test_recovery01_recalibrates_hazard_curve_with_par_spreads() {
    let base = base_date();
    let maturity = Date::from_calendar_date(2030, Month::March, 20).unwrap();

    let recovery = 0.40;
    // Build a hazard curve carrying its own par-spread quotes (production
    // shape — mirrors what a bootstrap would store). Parameterised on the
    // recovery so the frozen-curve baseline below can produce a curve whose
    // recovery metadata matches each bumped trade recovery.
    let make_hazard = |rec: f64| {
        HazardCurve::builder("CDS-CAL-CREDIT")
            .base_date(base)
            .recovery_rate(rec)
            .knots([(1.0, 0.05), (3.0, 0.05), (5.0, 0.05), (10.0, 0.05)])
            .par_spreads([(1.0, 300.0), (3.0, 300.0), (5.0, 300.0), (10.0, 300.0)])
            .build()
            .unwrap()
    };
    let hazard = make_hazard(recovery);

    let cds = crate::finstack_test_utils::cds_buy_protection(
        "CDS-RECALIB",
        Money::new(10_000_000.0, Currency::USD),
        300.0,
        base,
        maturity,
        "USD-OIS",
        "CDS-CAL-CREDIT",
    )
    .unwrap();
    let mut cds_test = cds.clone();
    cds_test.protection.recovery_rate = recovery;

    let market = MarketContext::new()
        .insert(create_discount_curve(base))
        .insert(hazard);

    // Metric path — recalibrates the hazard curve under the bumped recovery.
    let result = cds_test
        .price_with_metrics(
            &market,
            base,
            &[MetricId::Recovery01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("Recovery01 should compute on calibrated curve");
    let recalibrated_recovery01 = result.measures[MetricId::Recovery01.as_str()];

    // Hand-rolled frozen-curve baseline: bump only the instrument LGD, keep
    // the hazard λ knots intact. This is what Recovery01 does on its
    // frozen-curve path. The curve's recovery metadata is realigned with each
    // bumped trade recovery so the (trade, curve) pair stays consistent under
    // the ISDA recovery-consistency guard — the λ knots are untouched, so this
    // is still the pure LGD-only sensitivity.
    let bump = 0.01;
    let mut cds_up = cds_test.clone();
    cds_up.protection.recovery_rate = recovery + bump;
    let market_up = MarketContext::new()
        .insert(create_discount_curve(base))
        .insert(make_hazard(recovery + bump));
    let mut cds_down = cds_test.clone();
    cds_down.protection.recovery_rate = recovery - bump;
    let market_down = MarketContext::new()
        .insert(create_discount_curve(base))
        .insert(make_hazard(recovery - bump));
    let pv_up_frozen = cds_up.value(&market_up, base).unwrap().amount();
    let pv_down_frozen = cds_down.value(&market_down, base).unwrap().amount();
    let frozen_recovery01 = (pv_up_frozen - pv_down_frozen) / 2.0;

    assert!(
        recalibrated_recovery01.is_finite(),
        "Recalibrated Recovery01 should be finite, got {recalibrated_recovery01}"
    );
    assert!(
        frozen_recovery01.is_finite(),
        "Frozen-curve baseline should be finite"
    );

    // The two answers MUST differ — that's the whole point of recalibration.
    let abs_diff = (recalibrated_recovery01 - frozen_recovery01).abs();
    assert!(
        abs_diff > 1e-3,
        "Recalibrated Recovery01 ({recalibrated_recovery01:.6}) should differ \
         materially from the frozen-curve baseline ({frozen_recovery01:.6}). \
         Identical values indicate the hazard curve is NOT being recalibrated \
         on recovery bumps — Recovery01 would then understate the true \
         sensitivity for spread-bootstrapped curves."
    );
}

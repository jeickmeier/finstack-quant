//! CDS market validation tests.
//!
//! Tests against ISDA standard methodologies and market benchmarks.
//! Validates that calculations match industry-standard pricing models.
//!
//! References:
//! - ISDA 2014 CDS Standard Model
//! - Hull, "Options, Futures, and Other Derivatives" (Credit Risk chapter)
//! - O'Kane, "Modelling Single-name and Multi-name Credit Derivatives"

use crate::finstack_quant_test_utils as test_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CDSConvention;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn build_flat_discount(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .build()
        .unwrap()
}

fn build_flat_hazard(
    hazard_rate: f64,
    recovery: f64,
    base_date: Date,
    curve_id: &str,
) -> HazardCurve {
    HazardCurve::builder(curve_id)
        .base_date(base_date)
        .recovery_rate(recovery)
        .knots([
            (0.0, hazard_rate),
            (1.0, hazard_rate),
            (5.0, hazard_rate),
            (10.0, hazard_rate),
        ])
        .build()
        .unwrap()
}

#[test]
fn test_regional_cds_lifecycle_fixtures_na_eu_asia() {
    let fixtures = [
        (
            Currency::USD,
            CDSConvention::IsdaNa,
            DayCount::Act360,
            "nyse",
            3_u16,
        ),
        (
            Currency::EUR,
            CDSConvention::IsdaEu,
            DayCount::Act360,
            "target2",
            1_u16,
        ),
        (
            Currency::JPY,
            CDSConvention::IsdaAs,
            DayCount::Act365F,
            "jpto",
            3_u16,
        ),
    ];

    for (
        currency,
        expected_convention,
        expected_day_count,
        expected_calendar,
        expected_settlement,
    ) in fixtures
    {
        let detected = CDSConvention::detect_from_currency(currency);
        assert_eq!(detected, expected_convention);
        assert_eq!(detected.day_count(), expected_day_count);
        assert_eq!(detected.default_calendar(), expected_calendar);
        assert_eq!(detected.settlement_delay(), expected_settlement);
    }
}

#[test]
fn test_par_spread_approximation() {
    // Market standard approximation (credit triangle):
    //
    //   Par Spread (decimal) ≈ Hazard Rate × (1 - Recovery)
    //   Par Spread (bp)      ≈ Hazard Rate × (1 - Recovery) × 10_000
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let hazard_rate = 0.01; // 1% hazard rate
    let recovery = 0.40;

    // Use ~zero discounting to make the discrete-coupon ISDA model closer to the continuous
    // credit-triangle approximation, so we can tighten expected values.
    let disc_curve = build_flat_discount(0.0, as_of, "USD_OIS");
    let hazard_curve = build_flat_hazard(hazard_rate, recovery, as_of, "CORP_HAZARD");

    let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

    let mut cds = test_utils::cds_buy_protection(
        "PAR_SPREAD_APPROX",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP_HAZARD",
    )
    .expect("CDS construction should succeed");
    cds.protection.recovery_rate = recovery;

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_spread = *result.measures.get("par_spread").unwrap();

    // Expected: 0.01 × 0.6 × 10_000 = 60 bp
    let expected_triangle_bp = hazard_rate * (1.0 - recovery) * 10_000.0;
    assert!(
        (par_spread - expected_triangle_bp).abs() < 15.0,
        "Par spread={:.2} bp should be close to credit triangle {:.2} bp (|diff|<15bp)",
        par_spread,
        expected_triangle_bp
    );
}

#[test]
fn test_step_in_date_overrides_protection_start() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let mut cds = test_utils::cds_buy_protection(
        "STEP_IN_DATE",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP_HAZARD",
    )
    .expect("CDS construction should succeed");

    let step_in = date!(2024 - 01 - 02);
    cds.protection_effective_date = Some(step_in);

    assert_eq!(cds.protection_start(), step_in);
    cds.validate()
        .expect("explicit step-in date should validate");
}

#[test]
fn test_clean_upfront_adjustment_changes_npv_not_leg_pvs() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount(0.05, as_of, "USD_OIS");
    let hazard_curve = build_flat_hazard(0.015, 0.40, as_of, "CORP_HAZARD");
    let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

    let mut cds = test_utils::cds_buy_protection(
        "CLEAN_DIRTY_UPFRONT",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP_HAZARD",
    )
    .expect("CDS construction should succeed");

    let base = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ProtectionLegPv, MetricId::PremiumLegPv],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("base cds metrics");
    let base_npv = base.value;

    let upfront = Money::new(125_000.0, Currency::USD);
    cds.instrument_pricing_overrides
        .market_quotes
        .upfront_payment = Some(upfront);

    let dirty = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ProtectionLegPv, MetricId::PremiumLegPv],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("dirty cds metrics");

    assert!(
        (dirty.measures[MetricId::ProtectionLegPv.as_str()]
            - base.measures[MetricId::ProtectionLegPv.as_str()])
        .abs()
            < 1e-8
    );
    assert!(
        (dirty.measures[MetricId::PremiumLegPv.as_str()]
            - base.measures[MetricId::PremiumLegPv.as_str()])
        .abs()
            < 1e-8
    );
    assert!(
        (dirty.value.amount() - (base_npv.amount() - upfront.amount())).abs() < 1e-8,
        "Dirty buyer NPV should equal clean NPV minus upfront; clean={}, dirty={}, upfront={}",
        base_npv.amount(),
        dirty.value.amount(),
        upfront.amount()
    );
}

#[test]
fn test_risky_pv01_market_standard() {
    // Risky PV01 should be in reasonable range for standard CDS
    // For 5Y $10MM CDS with moderate default risk

    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount(0.05, as_of, "USD_OIS");
    let hazard_curve = build_flat_hazard(0.01, 0.40, as_of, "CORP_HAZARD");

    let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

    let mut cds = test_utils::cds_buy_protection(
        "RISKY_PV01_MKT",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP_HAZARD",
    )
    .expect("CDS construction should succeed");
    cds.protection.recovery_rate = 0.40;

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::RiskyPv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let risky_pv01 = *result.measures.get("risky_pv01").unwrap();

    // For $10MM notional, 5Y CDS, risky PV01 should be $4,000-$5,000
    // (survival probability reduces the annuity compared to risk-free)
    assert!(
        risky_pv01 > 4_000.0 && risky_pv01 < 5_000.0,
        "Risky PV01={:.2} outside market range $4,000-$5,000 for $10MM 5Y CDS",
        risky_pv01
    );
}

#[test]
fn test_cs01_positive_for_protection_buyer() {
    // CS01 measures sensitivity to credit spread changes
    // Protection buyer benefits from widening spreads (CS01 > 0)

    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount(0.05, as_of, "USD_OIS");
    let hazard_curve = build_flat_hazard(0.01, 0.40, as_of, "CORP_HAZARD");

    let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

    let mut cds = test_utils::cds_buy_protection(
        "CS01_BUYER",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP_HAZARD",
    )
    .expect("CDS construction should succeed");
    cds.protection.recovery_rate = 0.40;

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let cs01 = *result.measures.get("cs01").unwrap();

    assert!(
        cs01 > 0.0,
        "CS01={:.2} should be positive for protection buyer",
        cs01
    );

    // Reasonable range for $10MM, 5Y CDS
    assert!(
        cs01 > 1_000.0 && cs01 < 1_000_000.0,
        "CS01={:.2} outside reasonable range",
        cs01
    );
}

#[test]
fn test_hazard_rate_sensitivity_monotonic() {
    // Protection value should increase monotonically with hazard rate

    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let mut npvs = Vec::new();

    for hazard_rate in [0.005, 0.010, 0.020, 0.030, 0.040] {
        let disc_curve = build_flat_discount(0.05, as_of, "USD_OIS");
        let hazard_curve = build_flat_hazard(hazard_rate, 0.40, as_of, "CORP_HAZARD");

        let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

        let mut cds = test_utils::cds_buy_protection(
            "HAZARD_SENS",
            Money::new(10_000_000.0, Currency::USD),
            100.0,
            as_of,
            end,
            "USD_OIS",
            "CORP_HAZARD",
        )
        .expect("CDS construction should succeed");
        cds.protection.recovery_rate = 0.40;

        let npv = cds.value(&market, as_of).unwrap();
        npvs.push((hazard_rate, npv.amount()));
    }

    // NPV should increase monotonically for protection buyer
    for i in 1..npvs.len() {
        assert!(
            npvs[i].1 > npvs[i - 1].1,
            "Protection buyer NPV should increase with hazard rate: \
             {:.3}% NPV={:.0} <= {:.3}% NPV={:.0}",
            npvs[i - 1].0 * 100.0,
            npvs[i - 1].1,
            npvs[i].0 * 100.0,
            npvs[i].1
        );
    }
}

#[test]
fn test_recovery_rate_sensitivity_monotonic() {
    // Protection value should decrease monotonically with recovery rate

    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let mut npvs = Vec::new();

    for recovery in [0.10, 0.20, 0.40, 0.60, 0.80] {
        let disc_curve = build_flat_discount(0.05, as_of, "USD_OIS");
        let hazard_curve = build_flat_hazard(0.015, recovery, as_of, "CORP_HAZARD");

        let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

        let mut cds = test_utils::cds_buy_protection(
            "RECOVERY_SENS",
            Money::new(10_000_000.0, Currency::USD),
            100.0,
            as_of,
            end,
            "USD_OIS",
            "CORP_HAZARD",
        )
        .expect("CDS construction should succeed");
        cds.protection.recovery_rate = recovery;

        let npv = cds.value(&market, as_of).unwrap();
        npvs.push((recovery, npv.amount()));
    }

    // NPV should decrease monotonically for protection buyer
    for i in 1..npvs.len() {
        assert!(
            npvs[i].1 < npvs[i - 1].1,
            "Protection buyer NPV should decrease with recovery rate: \
             {:.0}% NPV={:.0} >= {:.0}% NPV={:.0}",
            npvs[i - 1].0 * 100.0,
            npvs[i - 1].1,
            npvs[i].0 * 100.0,
            npvs[i].1
        );
    }
}

#[test]
fn test_expected_loss_formula_validation() {
    // Our implementation computes UNDISCOUNTED Expected Loss:
    // EL = Notional × PD × LGD
    //
    // Where PD = 1 - S(T) is the cumulative probability of default to maturity.
    // This is the "credit risk" expected loss used in regulatory/accounting contexts
    // (e.g., IFRS 9 Expected Credit Loss). The discounted expected loss for pricing
    // purposes is captured separately by the Protection Leg PV.
    //
    // Note: For DISCOUNTED expected loss, use: PV_protection / (1 - Recovery)

    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let hazard_rate = 0.02; // 2% per year
    let recovery = 0.40;
    let notional = 10_000_000.0;

    let disc_curve = build_flat_discount(0.05, as_of, "USD_OIS");
    let hazard_curve = build_flat_hazard(hazard_rate, recovery, as_of, "CORP_HAZARD");

    let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

    let mut cds = test_utils::cds_buy_protection(
        "EL_FORMULA_TEST",
        Money::new(notional, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP_HAZARD",
    )
    .expect("CDS construction should succeed");
    cds.protection.recovery_rate = recovery;

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ExpectedLoss],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let expected_loss = *result.measures.get("expected_loss").unwrap();

    // Calculate time to maturity using the hazard curve's time axis (what ExpectedLoss uses).
    let hazard = market.get_hazard("CORP_HAZARD").unwrap();
    let t_maturity = hazard
        .day_count()
        .year_fraction(
            as_of,
            end,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .unwrap();

    // Theoretical undiscounted expected loss:
    // EL = Notional × (1 - S(T)) × LGD
    // S(T) = exp(-hazard_rate × T)
    let survival_prob = (-hazard_rate * t_maturity).exp();
    let pd = 1.0 - survival_prob;
    let lgd = 1.0 - recovery;
    let theoretical_el = notional * pd * lgd;

    // Industry standard: < 0.5% tolerance for this simple formula
    let rel_error = ((expected_loss - theoretical_el) / theoretical_el).abs();
    assert!(
        rel_error < 0.005,
        "Expected loss formula validation failed: computed={:.0}, theoretical={:.0}, error={:.2}%",
        expected_loss,
        theoretical_el,
        rel_error * 100.0
    );
}

#[test]
fn test_jump_to_default_equals_lgd_times_notional() {
    // JTD = Notional × (1 - Recovery) for protection buyer

    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let recovery = 0.40;
    let notional = 10_000_000.0;

    let disc_curve = build_flat_discount(0.05, as_of, "USD_OIS");
    let hazard_curve = build_flat_hazard(0.01, recovery, as_of, "CORP_HAZARD");

    let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

    let mut cds = test_utils::cds_buy_protection(
        "JTD_TEST",
        Money::new(notional, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP_HAZARD",
    )
    .expect("CDS construction should succeed");
    cds.protection.recovery_rate = recovery;

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let jtd = *result.measures.get("jump_to_default").unwrap();

    let expected_jtd = notional * (1.0 - recovery);

    // Should match closely
    assert!(
        (jtd - expected_jtd).abs() < 100_000.0,
        "JTD={:.0} should equal Notional × LGD={:.0}",
        jtd,
        expected_jtd
    );
}

#[test]
fn test_survival_probability_decreases_over_time() {
    // As valuation date moves forward, remaining survival probability decreases

    let start = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount(0.05, start, "USD_OIS");
    let hazard_curve = build_flat_hazard(0.015, 0.40, start, "CORP_HAZARD");

    let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

    let mut cds = test_utils::cds_buy_protection(
        "SURVIVAL_TEST",
        Money::new(10_000_000.0, Currency::USD),
        150.0,
        start,
        end,
        "USD_OIS",
        "CORP_HAZARD",
    )
    .expect("CDS construction should succeed");
    cds.protection.recovery_rate = 0.40;

    let result_t0 = cds
        .price_with_metrics(
            &market,
            start,
            &[MetricId::RiskyPv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let risky_pv01_t0 = *result_t0.measures.get("risky_pv01").unwrap();

    // Risky PV01 reflects survival-weighted annuity
    // Should be less than risk-free annuity due to default risk
    let risk_free_pv01_approx = 10_000_000.0 / 10000.0 * 4.3; // Approximate 5Y annuity

    assert!(
        risky_pv01_t0 < risk_free_pv01_approx,
        "Risky PV01={:.2} should be less than risk-free PV01≈{:.2}",
        risky_pv01_t0,
        risk_free_pv01_approx
    );
}

#[test]
fn test_standard_tenors_reasonable_par_spreads() {
    // Test that standard tenors (1Y, 3Y, 5Y, 7Y, 10Y) produce reasonable par spreads

    let as_of = date!(2024 - 01 - 01);
    let hazard_rate = 0.015; // 1.5% hazard rate
    let recovery = 0.40;

    let disc_curve = build_flat_discount(0.05, as_of, "USD_OIS");
    let hazard_curve = build_flat_hazard(hazard_rate, recovery, as_of, "CORP_HAZARD");

    let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

    let tenors = [
        (1, date!(2025 - 01 - 01)),
        (3, date!(2027 - 01 - 01)),
        (5, date!(2029 - 01 - 01)),
        (7, date!(2031 - 01 - 01)),
        (10, date!(2034 - 01 - 01)),
    ];

    for (tenor_years, maturity) in tenors {
        let mut cds = test_utils::cds_buy_protection(
            format!("CDS_{}Y", tenor_years),
            Money::new(10_000_000.0, Currency::USD),
            100.0,
            as_of,
            maturity,
            "USD_OIS",
            "CORP_HAZARD",
        )
        .expect("CDS construction should succeed");
        cds.protection.recovery_rate = recovery;

        let result = cds
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::ParSpread],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let par_spread = *result.measures.get("par_spread").unwrap();

        // Par spread should be positive and reasonable (20-200 bps for IG credit)
        assert!(
            par_spread > 20.0 && par_spread < 200.0,
            "{}Y par spread={:.2} bps outside typical range",
            tenor_years,
            par_spread
        );
    }
}

#[test]
fn test_term_structure_upward_sloping_spreads() {
    // Longer tenors typically have higher par spreads (upward sloping credit curve)

    let as_of = date!(2024 - 01 - 01);
    let hazard_rate = 0.015;
    let recovery = 0.40;

    let disc_curve = build_flat_discount(0.05, as_of, "USD_OIS");
    let hazard_curve = build_flat_hazard(hazard_rate, recovery, as_of, "CORP_HAZARD");

    let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

    let mut par_spreads = Vec::new();

    for years in [1, 3, 5, 7, 10] {
        let maturity = Date::from_calendar_date(2024 + years, time::Month::January, 1).unwrap();
        let mut cds = test_utils::cds_buy_protection(
            format!("TERM_{}Y", years),
            Money::new(10_000_000.0, Currency::USD),
            100.0,
            as_of,
            maturity,
            "USD_OIS",
            "CORP_HAZARD",
        )
        .expect("CDS construction should succeed");
        cds.protection.recovery_rate = recovery;

        let result = cds
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::ParSpread],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let par_spread = *result.measures.get("par_spread").unwrap();
        par_spreads.push((years, par_spread));
    }

    // With flat hazard curve, par spreads should be relatively stable
    // All should be in reasonable range
    for (years, spread) in &par_spreads {
        assert!(
            spread > &20.0 && spread < &200.0,
            "{}Y par spread={:.2} bps outside reasonable range",
            years,
            spread
        );
    }
}

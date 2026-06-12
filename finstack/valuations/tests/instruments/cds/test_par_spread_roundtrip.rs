//! CDS par spread round-trip tests.
//!
//! Validates that:
//! 1. Bootstrap hazard curve from CDS par spreads
//! 2. Reprice CDS at bootstrapped spreads
//! 3. Assert NPV ≈ 0 (within tolerance: 1bp of notional)
//!
//! This is a critical validation that the hazard curve calibration and
//! CDS pricing are internally consistent.
//!
//! Market Standards Review (Priority 4, Week 4)

use crate::finstack_test_utils::calibration::execute_step;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, ParInterp, Seniority};
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_valuations::calibration::api::schema::{HazardCurveParams, StepParams};
use finstack_valuations::calibration::CalibrationConfig;
use finstack_valuations::calibration::CalibrationMethod;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
use finstack_valuations::market::quotes::cds::CdsQuote;
use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_valuations::market::quotes::market_quote::MarketQuote;
use time::Month;

fn create_discount_curve(base_date: Date) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (3.0, 0.94),
            (5.0, 0.88),
            (10.0, 0.70),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

#[test]
fn test_cds_par_spread_roundtrip_1y() {
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::March, 20).unwrap();
    let par_spread_bp = 100.0; // 100bp

    // Create CDS quote
    let quotes = vec![CdsQuote::CdsParSpread {
        id: QuoteId::new("CDS-1Y"),
        entity: "ROUNDTRIP-TEST".to_string(),
        pillar: Pillar::Date(maturity),
        spread_bp: par_spread_bp,
        recovery_rate: 0.40,
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::IsdaNa,
        },
    }];

    let disc = create_discount_curve(base);
    let market_calib = MarketContext::new().insert(disc);

    // Bootstrap hazard curve (v2 step engine)
    let settings = CalibrationConfig::default();
    let params = HazardCurveParams {
        curve_id: "ROUNDTRIP-TEST-senior".into(),
        entity: "ROUNDTRIP-TEST".to_string(),
        seniority: Seniority::Senior,
        currency: Currency::USD,
        base_date: base,
        discount_curve_id: "USD-OIS".into(),
        recovery_rate: 0.40,
        notional: 1.0,
        method: CalibrationMethod::Bootstrap,
        interpolation: finstack_core::math::interp::InterpStyle::LogLinear,
        par_interp: ParInterp::Linear,
        doc_clause: None,
        cds_valuation_convention: None,
    };
    let step = StepParams::Hazard(params.clone());
    let market_quotes: Vec<MarketQuote> = quotes.into_iter().map(MarketQuote::Cds).collect();
    let (ctx, _report) = execute_step(&step, &market_quotes, &market_calib, &settings).unwrap();
    let hazard_curve = ctx
        .get_hazard(params.curve_id.as_str())
        .expect("hazard inserted");

    // Create CDS at the quoted spread
    // Hazard curve ID is "{entity}-{seniority}" per HazardCurveCalibrator
    let cds = crate::finstack_test_utils::cds_buy_protection(
        "ROUNDTRIP-CDS",
        Money::new(10_000_000.0, Currency::USD),
        par_spread_bp,
        base,
        maturity,
        "USD-OIS",
        "ROUNDTRIP-TEST-senior",
    )
    .expect("CDS construction should succeed");

    // Price the CDS with calibrated hazard curve
    let market_price = MarketContext::new()
        .insert(create_discount_curve(base))
        .insert((*hazard_curve).clone());

    let npv = cds
        .value(&market_price, base)
        .expect("CDS valuation should succeed with correctly configured market");

    // Property: NPV should be ≈ 0 at par spread
    let tolerance = 1.0; // Tightened: Calibration should be precise to within $1 on $10M

    assert!(
        npv.amount().abs() < tolerance,
        "CDS repriced at par spread should have NPV ≈ 0, got: {:.2} (tolerance: {:.2})",
        npv.amount(),
        tolerance
    );
}

#[test]
fn test_cds_par_spread_roundtrip_multi_tenor() {
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();

    // Multiple tenors
    let tenors_and_spreads = vec![
        (
            Date::from_calendar_date(2026, Month::March, 20).unwrap(),
            80.0,
        ), // 1Y, 80bp
        (
            Date::from_calendar_date(2028, Month::March, 20).unwrap(),
            120.0,
        ), // 3Y, 120bp
        (
            Date::from_calendar_date(2030, Month::March, 20).unwrap(),
            150.0,
        ), // 5Y, 150bp
    ];

    // Create quotes
    let quotes: Vec<CdsQuote> = tenors_and_spreads
        .iter()
        .enumerate()
        .map(|(i, (maturity, spread))| CdsQuote::CdsParSpread {
            id: QuoteId::new(format!("CDS-{}", i)),
            entity: "MULTI-TENOR-TEST".to_string(),
            pillar: Pillar::Date(*maturity),
            spread_bp: *spread,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
        })
        .collect();

    let disc = create_discount_curve(base);
    let market_calib = MarketContext::new().insert(disc);

    let settings = CalibrationConfig::default();
    let params = HazardCurveParams {
        curve_id: "MULTI-TENOR-TEST-senior".into(),
        entity: "MULTI-TENOR-TEST".to_string(),
        seniority: Seniority::Senior,
        currency: Currency::USD,
        base_date: base,
        discount_curve_id: "USD-OIS".into(),
        recovery_rate: 0.40,
        notional: 1.0,
        method: CalibrationMethod::Bootstrap,
        interpolation: finstack_core::math::interp::InterpStyle::LogLinear,
        par_interp: ParInterp::Linear,
        doc_clause: None,
        cds_valuation_convention: None,
    };
    let step = StepParams::Hazard(params.clone());
    let market_quotes: Vec<MarketQuote> = quotes.iter().cloned().map(MarketQuote::Cds).collect();
    let (ctx, _report) = execute_step(&step, &market_quotes, &market_calib, &settings).unwrap();
    let hazard_curve = ctx
        .get_hazard(params.curve_id.as_str())
        .expect("hazard inserted");

    // Reprice each CDS at its quoted spread
    let market_price = MarketContext::new()
        .insert(create_discount_curve(base))
        .insert((*hazard_curve).clone());

    for (maturity, par_spread_bp) in &tenors_and_spreads {
        // Hazard curve ID is "{entity}-{seniority}" per HazardCurveCalibrator
        let cds = crate::finstack_test_utils::cds_buy_protection(
            format!("ROUNDTRIP-CDS-{}", maturity),
            Money::new(10_000_000.0, Currency::USD),
            *par_spread_bp,
            base,
            *maturity,
            "USD-OIS",
            "MULTI-TENOR-TEST-senior",
        )
        .expect("CDS construction should succeed");

        let npv = cds
            .value(&market_price, base)
            .expect("CDS valuation should succeed with correctly configured market");

        // Each CDS should have NPV ≈ 0 at its par spread
        let tolerance = 1.0; // Tightened: Calibration should be precise to within $1 on $10M

        assert!(
            npv.amount().abs() < tolerance,
            "CDS maturity {} repriced at par spread {:.0}bp should have NPV ≈ 0, got: {:.2}",
            maturity,
            par_spread_bp,
            npv.amount()
        );
    }
}

#[test]
fn test_cds_par_spread_calculation_consistency() {
    // Test that calculating par spread from a hazard curve, then repricing
    // with that spread, gives NPV ≈ 0
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::March, 20).unwrap();

    let quotes = [CdsQuote::CdsParSpread {
        id: QuoteId::new("CDS-5Y"),
        entity: "PAR-CONSISTENCY-TEST".to_string(),
        pillar: Pillar::Date(maturity),
        spread_bp: 200.0, // 200bp
        recovery_rate: 0.40,
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::IsdaNa,
        },
    }];

    let disc = create_discount_curve(base);
    let market_calib = MarketContext::new().insert(disc);

    let settings = CalibrationConfig::default();
    let params = HazardCurveParams {
        curve_id: "PAR-CONSISTENCY-TEST-senior".into(),
        entity: "PAR-CONSISTENCY-TEST".to_string(),
        seniority: Seniority::Senior,
        currency: Currency::USD,
        base_date: base,
        discount_curve_id: "USD-OIS".into(),
        recovery_rate: 0.40,
        notional: 1.0,
        method: CalibrationMethod::Bootstrap,
        interpolation: finstack_core::math::interp::InterpStyle::LogLinear,
        par_interp: ParInterp::Linear,
        doc_clause: None,
        cds_valuation_convention: None,
    };
    let step = StepParams::Hazard(params.clone());
    let market_quotes: Vec<MarketQuote> = quotes.iter().cloned().map(MarketQuote::Cds).collect();
    let (ctx, _report) = execute_step(&step, &market_quotes, &market_calib, &settings).unwrap();
    let hazard_curve = ctx
        .get_hazard(params.curve_id.as_str())
        .expect("hazard inserted");

    // Create market with calibrated hazard curve
    let market_price = MarketContext::new()
        .insert(create_discount_curve(base))
        .insert((*hazard_curve).clone());

    // Create CDS with arbitrary spread
    // Hazard curve ID is "{entity}-{seniority}" per HazardCurveCalibrator
    let cds_test = crate::finstack_test_utils::cds_buy_protection(
        "TEST-CDS",
        Money::new(10_000_000.0, Currency::USD),
        150.0, // Different spread (150bp)
        base,
        maturity,
        "USD-OIS",
        "PAR-CONSISTENCY-TEST-senior",
    )
    .expect("CDS construction should succeed");

    // Calculate par spread metric
    let result = cds_test
        .price_with_metrics(
            &market_price,
            base,
            &[finstack_valuations::metrics::MetricId::ParSpread],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("CDS pricing with metrics should succeed with correctly configured market");

    let calculated_par_spread =
        result.measures[finstack_valuations::metrics::MetricId::ParSpread.as_str()];

    // Calculated par spread should be close to the original 200bp used for calibration
    assert!(
        (calculated_par_spread - 200.0).abs() < 1.0, // Within 1bp
        "Calculated par spread {:.2}bp should match calibration input 200bp",
        calculated_par_spread
    );
}

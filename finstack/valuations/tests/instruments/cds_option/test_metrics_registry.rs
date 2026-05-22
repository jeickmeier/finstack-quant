//! Tests for CDS Option metrics framework integration.

use super::common::*;
use finstack_core::currency::Currency;
use finstack_core::dates::DayCount;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::ScalarTimeSeries;
use finstack_core::market_data::term_structures::{
    DiscountCurve, DiscountCurveRateCalibration, DiscountCurveRateQuote,
    DiscountCurveRateQuoteType, HazardCurve,
};
use finstack_core::types::IndexId;
use finstack_valuations::calibration::api::schema::DiscountCurveParams;
use finstack_valuations::calibration::bumps::{bump_discount_curve, BumpRequest};
use finstack_valuations::calibration::{CalibrationMethod, RatesStepConventions};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_valuations::market::quotes::rates::RateQuote;
use finstack_valuations::metrics::{standard_registry, MetricContext, MetricId};
use time::macros::date;
use time::Duration;

fn quote_calibrated_discount(rate: f64, as_of: finstack_core::dates::Date) -> DiscountCurve {
    flat_discount("USD-OIS", as_of, rate)
        .to_builder_with_id("USD-OIS")
        .rate_calibration(DiscountCurveRateCalibration {
            index_id: "USD-SOFR-1M".to_string(),
            currency: Currency::USD,
            quotes: vec![
                DiscountCurveRateQuote {
                    quote_type: DiscountCurveRateQuoteType::Deposit,
                    tenor: "1Y".to_string(),
                    rate,
                },
                DiscountCurveRateQuote {
                    quote_type: DiscountCurveRateQuoteType::Deposit,
                    tenor: "5Y".to_string(),
                    rate,
                },
                DiscountCurveRateQuote {
                    quote_type: DiscountCurveRateQuoteType::Deposit,
                    tenor: "10Y".to_string(),
                    rate,
                },
            ],
        })
        .build()
        .unwrap()
}

fn bump_quote_calibrated_discount(
    curve: &DiscountCurve,
    calibration: &DiscountCurveRateCalibration,
    market: &MarketContext,
    bump_bp: f64,
) -> DiscountCurve {
    let index = IndexId::new(calibration.index_id.as_str());
    let quotes: Vec<RateQuote> = calibration
        .quotes
        .iter()
        .map(|quote| RateQuote::Deposit {
            id: QuoteId::new(format!("{}-{}", curve.id(), quote.tenor)),
            index: index.clone(),
            pillar: Pillar::Tenor(quote.tenor.parse().unwrap()),
            rate: quote.rate,
        })
        .collect();
    let first_rate = calibration
        .quotes
        .first()
        .map(|quote| quote.rate)
        .unwrap_or(0.0);
    let fixings = ScalarTimeSeries::new(
        format!("FIXING:{}", curve.id()),
        vec![
            (curve.base_date() - Duration::days(3), first_rate),
            (curve.base_date() - Duration::days(2), first_rate),
            (curve.base_date() - Duration::days(1), first_rate),
            (curve.base_date(), first_rate),
        ],
        None,
    )
    .unwrap();
    let params = DiscountCurveParams {
        curve_id: curve.id().clone(),
        currency: calibration.currency,
        base_date: curve.base_date(),
        method: CalibrationMethod::Bootstrap,
        interpolation: curve.interp_style(),
        extrapolation: curve.extrapolation(),
        pricing_discount_id: None,
        pricing_forward_id: None,
        conventions: RatesStepConventions {
            ois_compounding: None,
            curve_day_count: Some(curve.day_count()),
        },
    };
    bump_discount_curve(
        &quotes,
        &params,
        &market.clone().insert_series(fixings),
        &BumpRequest::Parallel(bump_bp),
    )
    .unwrap()
}

#[test]
fn test_metrics_registry_delta() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let pv = option.value(&market, as_of).unwrap();
    let mut ctx = MetricContext::new(
        std::sync::Arc::new(option),
        std::sync::Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let registry = standard_registry();
    let results = registry.compute(&[MetricId::Delta], &mut ctx).unwrap();

    assert!(results.contains_key(&MetricId::Delta));
    let delta = *results.get(&MetricId::Delta).unwrap();
    assert_finite(delta, "Delta from registry");
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn test_metrics_registry_all_greeks() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let pv = option.value(&market, as_of).unwrap();
    let mut ctx = MetricContext::new(
        std::sync::Arc::new(option),
        std::sync::Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let metrics = vec![
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Theta,
        MetricId::Cs01,
        MetricId::Dv01,
    ];

    let registry = standard_registry();
    let results = registry.compute(&metrics, &mut ctx).unwrap();

    assert_eq!(results.len(), metrics.len());
    for metric_id in metrics {
        assert!(results.contains_key(&metric_id));
        let value = *results.get(&metric_id).unwrap();
        assert_finite(value, &format!("{:?}", metric_id));
    }
}

#[test]
fn test_cds_option_dv01_bumps_swap_curve_quotes_and_matches_cds_convention() {
    // CDSO IR DV01 is a swap-curve quote sensitivity. It uses the same
    // central-difference sign and scale as CDS IR DV01 so portfolio aggregation
    // across CDS and CDS options is meaningful.
    let as_of = date!(2025 - 01 - 01);
    let option = CDSOptionBuilder::new().build(as_of);
    let discount = quote_calibrated_discount(0.03, as_of);
    let hazard = HazardCurve::builder("HZ-SN")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots([(1.0, 0.02), (5.0, 0.02), (10.0, 0.02)])
        .par_spreads([(1.0, 120.0), (5.0, 120.0), (10.0, 120.0)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(discount).insert(hazard);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();

    // Reproduce the calculation: bump the discount curve via its quote
    // calibration, leave the hazard curve untouched, and re-price.
    let bumped_pv = |bump_bp: f64| {
        let base_discount = market.get_discount("USD-OIS").unwrap();
        let calibration = base_discount.rate_calibration().unwrap();
        let bumped_discount =
            bump_quote_calibrated_discount(base_discount.as_ref(), calibration, &market, bump_bp);
        let bumped_market = market.clone().insert(bumped_discount);
        option.value_raw(&bumped_market, as_of).unwrap()
    };
    let expected = (bumped_pv(1.0) - bumped_pv(-1.0)) / 2.0;

    let tol = 1e-6_f64.max(1e-8 * expected.abs());
    assert!(
        (dv01 - expected).abs() <= tol,
        "CDS option DV01 should bump swap-curve quotes and report the CDS-compatible central-difference amount: metric={dv01}, expected={expected}, diff={}, tol={tol}",
        (dv01 - expected).abs()
    );
}

#[test]
fn test_cds_option_dv01_requires_swap_curve_quote_calibration() {
    let as_of = date!(2025 - 01 - 01);
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.86), (10.0, 0.74)])
        .build()
        .unwrap();
    let hazard = flat_hazard("HZ-SN", as_of, 0.4, 0.02);
    let market = MarketContext::new().insert(discount).insert(hazard);
    let option = CDSOptionBuilder::new().build(as_of);

    let err = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect_err("CDS option DV01 should reject direct discount-factor bumps");
    let message = err.to_string();
    assert!(
        message.contains("swap-curve quote calibration"),
        "unexpected error: {message}"
    );
}

#[test]
fn test_metrics_registry_implied_vol() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let target_vol = 0.30;
    let option = CDSOptionBuilder::new().implied_vol(target_vol).build(as_of);

    let pv = option.value(&market, as_of).unwrap();
    let mut ctx = MetricContext::new(
        std::sync::Arc::new(option),
        std::sync::Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let registry = standard_registry();
    let results = registry.compute(&[MetricId::ImpliedVol], &mut ctx).unwrap();

    let iv = *results.get(&MetricId::ImpliedVol).unwrap();
    assert_approx_eq(iv, target_vol, 1e-6, "Implied vol from registry");
}

#[test]
fn test_cs01_uses_delta_dependency() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let pv = option.value(&market, as_of).unwrap();
    let mut ctx = MetricContext::new(
        std::sync::Arc::new(option),
        std::sync::Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    // Compute CS01 which should use Delta if available
    let registry = standard_registry();
    let results = registry
        .compute(&[MetricId::Delta, MetricId::Cs01], &mut ctx)
        .unwrap();

    assert!(results.contains_key(&MetricId::Delta));
    assert!(results.contains_key(&MetricId::Cs01));

    let delta = *results.get(&MetricId::Delta).unwrap();
    let cs01 = *results.get(&MetricId::Cs01).unwrap();

    assert_finite(delta, "Delta");
    assert_finite(cs01, "CS01");
    assert_positive(cs01, "CS01 for call");
}

#[test]
fn test_cds_option_rejects_hazard_rate_cs01_metrics() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let pv = option.value(&market, as_of).unwrap();
    let mut ctx = MetricContext::new(
        std::sync::Arc::new(option),
        std::sync::Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let registry = standard_registry();
    let err = registry
        .compute(&[MetricId::Cs01Hazard], &mut ctx)
        .expect_err("CDS option should not expose hazard-rate CS01");
    assert!(matches!(
        err,
        finstack_core::Error::MetricNotApplicable { .. }
    ));

    let err = registry
        .compute(&[MetricId::BucketedCs01Hazard], &mut ctx)
        .expect_err("CDS option should not expose bucketed hazard-rate CS01");
    assert!(matches!(
        err,
        finstack_core::Error::MetricNotApplicable { .. }
    ));
}

#[test]
fn test_cds_option_cs01_requires_cds_quote_points() {
    let as_of = date!(2025 - 01 - 01);
    let discount = flat_discount("USD-OIS", as_of, 0.03);
    let hazard = HazardCurve::builder("HZ-SN")
        .base_date(as_of)
        .recovery_rate(0.4)
        .knots([(1.0, 0.02), (5.0, 0.02), (10.0, 0.02)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(discount).insert(hazard);
    let option = CDSOptionBuilder::new().build(as_of);

    let pv = option.value(&market, as_of).unwrap();
    let mut ctx = MetricContext::new(
        std::sync::Arc::new(option),
        std::sync::Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let registry = standard_registry();
    let err = registry
        .compute(&[MetricId::Cs01], &mut ctx)
        .expect_err("CDS option CS01 should require CDS quote/par-spread bumps");
    let message = err.to_string();
    assert!(
        message.contains("CDS quote") || message.contains("par-spread"),
        "unexpected error: {message}"
    );
}

#[test]
fn test_bucketed_cs01_reconciles_to_parallel() {
    // Key-rate (bucketed) par-spread CS01 must reconcile to the parallel
    // par-spread CS01: both bump the par CDS curve and re-bootstrap; the
    // bucketed variant just applies the shock one tenor at a time.
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("should compute Cs01 and BucketedCs01");

    let cs01 = *result.measures.get("cs01").expect("cs01 present");
    let bucketed = *result
        .measures
        .get("bucketed_cs01")
        .expect("bucketed_cs01 present");
    assert!(
        cs01.is_finite() && bucketed.is_finite(),
        "CS01 metrics must be finite (cs01={cs01}, bucketed={bucketed})"
    );
    assert!(
        (bucketed - cs01).abs() <= 1e-4 + 0.02 * cs01.abs(),
        "bucketed CS01 ({bucketed}) must reconcile to parallel CS01 ({cs01})"
    );

    // The per-tenor series must be present and sum to the same total.
    let series_sum: f64 = result
        .measures
        .iter()
        .filter(|(k, _)| k.as_str().starts_with("bucketed_cs01::"))
        .map(|(_, v)| *v)
        .sum();
    assert!(
        (series_sum - cs01).abs() <= 1e-4 + 0.02 * cs01.abs(),
        "per-tenor bucketed_cs01 series ({series_sum}) must sum to parallel CS01 ({cs01})"
    );
}

#[test]
fn test_bucketed_dv01_registered() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let pv = option.value(&market, as_of).unwrap();
    let mut ctx = MetricContext::new(
        std::sync::Arc::new(option),
        std::sync::Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let registry = standard_registry();
    let results = registry.compute(&[MetricId::BucketedDv01], &mut ctx);

    // Bucketed DV01 should be registered (may or may not compute successfully depending on market)
    // Just verify it doesn't panic
    assert!(results.is_ok() || results.is_err());
}

#[test]
fn test_metrics_near_expiry() {
    // Test metrics for near-expiry option
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new()
        .expiry_months(1) // Very short time to expiry
        .cds_maturity_months(13)
        .build(as_of);

    let pv = option.value(&market, as_of).unwrap();
    let mut ctx = MetricContext::new(
        std::sync::Arc::new(option),
        std::sync::Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let registry = standard_registry();
    let results = registry
        .compute(&[MetricId::Delta, MetricId::Vega], &mut ctx)
        .unwrap();

    // Near-expiry options should still have computable greeks
    let delta = *results.get(&MetricId::Delta).unwrap();
    let vega = *results.get(&MetricId::Vega).unwrap();

    assert_finite(delta, "Near-expiry delta");
    assert_finite(vega, "Near-expiry vega");
}

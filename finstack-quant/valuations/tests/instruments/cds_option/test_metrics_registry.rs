//! Tests for CDS Option metrics framework integration.

use super::common::*;
use finstack_quant_calibration::recalibration::bump_discount_curve_from_rate_calibration;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, HazardCurve, RateCalibrationCurveRole, RateCalibrationMethod,
    RateCalibrationPillar, RateCalibrationQuote, RateCalibrationRecipe,
};
use finstack_quant_core::types::{CurveId, IndexId};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
use finstack_quant_valuations::recalibration::QuoteBump;
use time::macros::date;

fn quote_calibrated_discount(rate: f64, as_of: finstack_quant_core::dates::Date) -> DiscountCurve {
    flat_discount("USD-OIS", as_of, rate)
        .to_builder_with_id("USD-OIS")
        .rate_calibration(RateCalibrationRecipe {
            currency: Currency::USD,
            method: RateCalibrationMethod::Bootstrap,
            curve_day_count: DayCount::Act365F,
            ois_compounding: None,
            role: RateCalibrationCurveRole::Discount {
                projection_curve_id: CurveId::new("USD-OIS"),
            },
            quotes: vec![
                RateCalibrationQuote::Deposit {
                    index_id: IndexId::new("USD-SOFR-1M"),
                    pillar: RateCalibrationPillar::Tenor("1Y".parse().unwrap()),
                    rate,
                },
                RateCalibrationQuote::Deposit {
                    index_id: IndexId::new("USD-SOFR-1M"),
                    pillar: RateCalibrationPillar::Tenor("5Y".parse().unwrap()),
                    rate,
                },
                RateCalibrationQuote::Deposit {
                    index_id: IndexId::new("USD-SOFR-1M"),
                    pillar: RateCalibrationPillar::Tenor("10Y".parse().unwrap()),
                    rate,
                },
            ],
        })
        .build()
        .unwrap()
}

fn bump_quote_calibrated_discount(
    curve: &DiscountCurve,
    calibration: &RateCalibrationRecipe,
    market: &MarketContext,
    bump_bp: f64,
) -> DiscountCurve {
    bump_discount_curve_from_rate_calibration(
        curve,
        calibration,
        market,
        &QuoteBump::ParallelBp(bump_bp),
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
    let market = replayable_standard_market(as_of);
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
            crate::test_support::credit::pricing_options(),
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
fn test_cds_option_dv01_falls_back_to_direct_bump_without_calibration() {
    // A directly-specified discount curve carries no swap-quote calibration
    // metadata. IR DV01 must still be well-defined: fall back to a parallel
    // discount-factor bump (same as `CdsDv01Calculator`) instead of erroring,
    // so the metric is available for portfolio aggregation.
    let as_of = date!(2025 - 01 - 01);
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.86), (10.0, 0.74)])
        .build()
        .unwrap();
    let hazard = flat_hazard("HZ-SN", as_of, 0.4, 0.02);
    let market = MarketContext::new().insert(discount).insert(hazard);
    let option = CDSOptionBuilder::new().build(as_of);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            crate::test_support::credit::pricing_options(),
        )
        .expect("CDS option DV01 should fall back to a direct discount-factor bump");
    let dv01 = *result.measures.get("dv01").expect("dv01 present");
    assert_finite(dv01, "CDS option DV01 (direct-bump fallback)");

    // Reproduce: parallel-bump the discount factors directly, hazard held fixed.
    let bumped_pv = |bump_bp: f64| {
        let mut bumped = market.clone();
        bumped
            .apply_curve_bump_in_place(
                &"USD-OIS".into(),
                finstack_quant_core::market_data::bumps::BumpSpec::parallel_bp(bump_bp),
            )
            .unwrap();
        option.value_raw(&bumped, as_of).unwrap()
    };
    let expected = (bumped_pv(1.0) - bumped_pv(-1.0)) / 2.0;
    let tol = 1e-6_f64.max(1e-8 * expected.abs());
    assert!(
        (dv01 - expected).abs() <= tol,
        "CDS option DV01 fallback should match a direct central-difference bump: metric={dv01}, expected={expected}"
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
    relative_eq(iv, target_vol, 1e-6, "Implied vol from registry");
}

#[test]
fn test_cs01_dependency_propagates_replay_error() {
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
    let error = registry
        .compute(&[MetricId::Delta, MetricId::Cs01], &mut ctx)
        .expect_err("standard option CS01 requires quote-space replay");
    assert!(error.to_string().contains("calibration recipe"));
}

#[test]
fn test_cds_option_cs01_requires_replay_recipe() {
    // A directly-specified hazard curve has no CDS par-spread replay recipe.
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
    let error = registry
        .compute(&[MetricId::Cs01], &mut ctx)
        .expect_err("standard CDS option CS01 requires quote-space replay");
    assert!(error.to_string().contains("calibration recipe"));
}

#[test]
fn test_bucketed_cs01_requires_replay_recipe() {
    // The standard fixture has no replay recipe.
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let error = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            crate::test_support::credit::pricing_options(),
        )
        .expect_err("standard CDS option CS01 metrics require quote-space replay");
    assert!(error.to_string().contains("calibration recipe"));
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

/// `SpreadDv01` is standard quote-space spread risk and requires replay.
#[test]
fn test_spread_dv01_requires_replay_recipe() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().call().build(as_of);

    let error = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::SpreadDv01],
            crate::test_support::credit::pricing_options(),
        )
        .expect_err("standard spread DV01 requires quote-space replay");
    assert!(error.to_string().contains("calibration recipe"));
}

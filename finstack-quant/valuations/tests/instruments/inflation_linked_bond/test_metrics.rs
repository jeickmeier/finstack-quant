//! Metrics integration tests for Inflation-Linked Bonds
//!
//! Tests cover:
//! - Metric calculator registration
//! - MetricId enumeration
//! - price_with_metrics functionality
//! - Metric calculation via framework
//! - Multiple metrics in single call

use super::common::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

fn running_under_coverage() -> bool {
    // `cargo llvm-cov` runs tests with LLVM coverage instrumentation enabled, which can slow down
    // execution significantly and make time-based assertions flaky.
    std::env::var_os("LLVM_PROFILE_FILE").is_some() || std::env::var_os("CARGO_LLVM_COV").is_some()
}

#[test]
fn test_price_with_metrics_real_yield() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::RealYield],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    assert!(result.measures.contains_key(MetricId::RealYield.as_str()));
    let real_yield = result.measures[MetricId::RealYield.as_str()];
    assert!(real_yield.is_finite());
    assert!(real_yield > -1.0 && real_yield < 1.0); // Reasonable range
}

#[test]
fn test_price_with_metrics_index_ratio() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::IndexRatio],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    assert!(result.measures.contains_key(MetricId::IndexRatio.as_str()));
    let ratio = result.measures[MetricId::IndexRatio.as_str()];
    assert!(ratio > 0.0);
}

#[test]
fn test_price_with_metrics_real_duration() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::RealDuration],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    assert!(result
        .measures
        .contains_key(MetricId::RealDuration.as_str()));
    let duration = result.measures[MetricId::RealDuration.as_str()];
    assert!(duration > 0.0);
}

#[test]
fn test_price_with_metrics_breakeven_inflation() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::BreakevenInflation],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    assert!(result
        .measures
        .contains_key(MetricId::BreakevenInflation.as_str()));
    let breakeven = result.measures[MetricId::BreakevenInflation.as_str()];
    assert!(breakeven.is_finite());
}

#[test]
fn test_price_with_metrics_dv01() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    assert!(result.measures.contains_key(MetricId::Dv01.as_str()));
    let dv01 = result.measures[MetricId::Dv01.as_str()];
    // DV01 = PV(bumped) - PV(base); when real rates rise, PV falls, so DV01 is negative
    assert!(dv01 <= 0.0);
}

#[test]
fn test_price_with_metrics_theta() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    assert!(result.measures.contains_key(MetricId::Theta.as_str()));
    let theta = result.measures[MetricId::Theta.as_str()];
    assert!(theta.is_finite());
}

#[test]
fn test_price_with_multiple_metrics() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    let metrics = [
        MetricId::RealYield,
        MetricId::IndexRatio,
        MetricId::RealDuration,
        MetricId::BreakevenInflation,
        MetricId::Dv01,
        MetricId::Theta,
    ];

    // Act
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert - all requested metrics should be present
    for metric in &metrics {
        assert!(
            result.measures.contains_key(metric.as_str()),
            "Missing metric: {:?}",
            metric
        );
        let value = result.measures[metric.as_str()];
        assert!(value.is_finite(), "Non-finite metric: {:?}", metric);
    }
}

#[test]
fn test_price_with_metrics_includes_base_value() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::RealYield],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert - result should include the base present value
    assert!(result.value.amount() > 0.0);
}

#[test]
fn test_price_with_no_metrics() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert - should still return base value, but no metrics
    assert!(result.value.amount() > 0.0);
    assert!(result.measures.is_empty());
}

#[test]
fn test_metrics_consistency_with_direct_calls() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act - calculate via metrics framework
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::RealDuration],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let duration_via_framework = result.measures[MetricId::RealDuration.as_str()];

    // Calculate via direct method
    let duration_direct = ilb.real_duration(&ctx, as_of).unwrap();

    // Assert - should be identical
    assert_approx_eq(
        duration_via_framework,
        duration_direct,
        EPSILON,
        "duration consistency",
    );
}

#[test]
fn test_metrics_real_yield_consistency() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.quoted_clean = Some(100.0); // Ensure quoted price is set
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act - calculate via metrics framework
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::RealYield],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let yield_via_framework = result.measures[MetricId::RealYield.as_str()];

    // Calculate via direct method
    let clean_price = ilb.quoted_clean.unwrap();
    let yield_direct = ilb.real_yield(clean_price, &ctx, as_of).unwrap();

    // Assert - should be identical since both call the same real_yield method
    assert_approx_eq(
        yield_via_framework,
        yield_direct,
        EPSILON, // Both use the same calculation path
        "real yield consistency",
    );
}

#[test]
fn test_metrics_index_ratio_consistency() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act - calculate via metrics framework
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::IndexRatio],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ratio_via_framework = result.measures[MetricId::IndexRatio.as_str()];

    // Calculate via direct method
    let ratio_direct = ilb.index_ratio_from_market(as_of, &ctx).unwrap();

    // Assert - should be identical
    assert_approx_eq(
        ratio_via_framework,
        ratio_direct,
        EPSILON,
        "index ratio consistency",
    );
}

// Note: Test removed - bonds don't exist after maturity, so testing metrics after maturity
// doesn't make practical sense. Once a bond has matured, all cashflows have been paid
// and there are no future cashflows to have sensitivity to.

#[test]
fn test_price_with_metrics_uk_gilt() {
    // Arrange
    let ilb = sample_uk_linker();
    let (ctx, _) = uk_market_context();
    let as_of = d(2025, 1, 2);

    let metrics = [
        MetricId::RealYield,
        MetricId::RealDuration,
        MetricId::IndexRatio,
    ];

    // Act
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    for metric in &metrics {
        assert!(result.measures.contains_key(metric.as_str()));
        let value = result.measures[metric.as_str()];
        assert!(value.is_finite());
    }
}

#[test]
fn test_price_with_metrics_performance() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    if running_under_coverage() {
        // Coverage builds are expected to be slower; this test is intended to catch performance
        // regressions in normal, non-instrumented test runs.
        return;
    }

    let metrics = [
        MetricId::RealYield,
        MetricId::IndexRatio,
        MetricId::RealDuration,
        MetricId::BreakevenInflation,
        MetricId::Dv01,
    ];

    // Act
    let start = std::time::Instant::now();
    for _ in 0..10 {
        let _ = ilb
            .price_with_metrics(
                &ctx,
                as_of,
                &metrics,
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();
    }
    let elapsed = start.elapsed();

    // Assert - 10 full metric calculations should be fast (< 500ms)
    assert!(elapsed.as_millis() < 500);
}

#[test]
fn test_metric_ids_have_str_representation() {
    // Arrange & Act & Assert
    assert!(!MetricId::RealYield.as_str().is_empty());
    assert!(!MetricId::IndexRatio.as_str().is_empty());
    assert!(!MetricId::RealDuration.as_str().is_empty());
    assert!(!MetricId::BreakevenInflation.as_str().is_empty());
    assert!(!MetricId::Dv01.as_str().is_empty());
    assert!(!MetricId::Theta.as_str().is_empty());
}

#[test]
fn test_bucketed_dv01_metric() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::BucketedDv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    assert!(result
        .measures
        .contains_key(MetricId::BucketedDv01.as_str()));
    // Note: BucketedDv01 might return 0.0 or aggregate value depending on implementation
    let bucketed = result.measures[MetricId::BucketedDv01.as_str()];
    assert!(bucketed.is_finite());
}

#[test]
fn test_metrics_with_deflation_protection() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.deflation_protection =
        finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::DeflationProtection::AllPayments;

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    let metrics = [MetricId::RealYield, MetricId::IndexRatio];

    // Act
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert - all metrics should calculate successfully
    for metric in &metrics {
        assert!(result.measures.contains_key(metric.as_str()));
        let value = result.measures[metric.as_str()];
        assert!(value.is_finite());
    }
}

#[test]
fn test_breakeven_inflation_metric_consistency() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let result = ilb
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::BreakevenInflation],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let breakeven_via_framework = result.measures[MetricId::BreakevenInflation.as_str()];

    // Direct calculation (using annually-compounded discount curve zero rate as nominal).
    // `BreakevenInflationCalculator` uses `disc.zero_annual(t)` so the direct call
    // must also use the annually-compounded zero to stay on the same basis as the
    // internally-converted annual real yield inside `breakeven_inflation`.
    let disc = ctx.get_discount(ilb.discount_curve_id.as_str()).unwrap();
    let t = disc
        .day_count()
        .year_fraction(
            disc.base_date(),
            ilb.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .unwrap();
    let nominal_yield = disc.zero_annual(t); // annual convention, matching the calculator
    let breakeven_direct = ilb.breakeven_inflation(nominal_yield, &ctx, as_of).unwrap();

    // Assert - should be identical since both use the same calculation path
    assert_approx_eq(
        breakeven_via_framework,
        breakeven_direct,
        EPSILON, // Both use the same calculation path
        "breakeven consistency",
    );
}

/// Breakeven metric against a NON-FLAT nominal discount curve with a
/// hand-computable expectation.
///
/// `discount_curve_id` is a nominal curve by contract, so the metric's Fisher
/// identity is `(1 + nominal_annual) / (1 + real_annual) - 1` where the
/// nominal leg is the annually-compounded zero read off that curve at the
/// bond's maturity. Setup:
///
/// - Nominal curve zeros: 1% at 1y, 2% at 5y, 3% at maturity (non-flat), with
///   a knot exactly at t(maturity) so the 3% zero is exact.
/// - 10y semi-annual 4% real coupon linker quoted at par with flat CPI, so the
///   real yield is exactly 4% Street => real_annual = 1.02² - 1 = 4.04%.
/// - Expected breakeven = 1.03 / 1.0404 - 1 ≈ -0.99962%.
///
/// With the pre-fix real-named defaults, a real curve fed the nominal leg and
/// the metric collapsed toward zero; the 1y-vs-maturity zero gap here also
/// guards against reading the wrong tenor off the curve.
#[test]
fn test_breakeven_inflation_metric_non_flat_nominal_curve() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{
        BusinessDayConvention, DayCount, DayCountContext, StubKind, Tenor,
    };
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::InflationLag;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::{
        DeflationProtection, IndexationMethod, InflationLinkedBond,
    };
    use finstack_quant_valuations::instruments::Attributes;

    let as_of = d(2025, 1, 15);
    let maturity = d(2035, 1, 15);

    let mut bond = InflationLinkedBond::builder()
        .id(InstrumentId::new("ILB-BEI-NONFLAT"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .real_coupon(rust_decimal::Decimal::try_from(0.04).unwrap())
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Thirty360)
        .issue_date(as_of)
        .maturity(maturity)
        .base_index(100.0)
        .base_date(as_of)
        .indexation_method(IndexationMethod::TIPS)
        .lag(InflationLag::None)
        .deflation_protection(DeflationProtection::MaturityOnly)
        .bdc(BusinessDayConvention::Following)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .inflation_index_id(CurveId::new("US-CPI"))
        .attributes(Attributes::new())
        .build()
        .unwrap();
    bond.quoted_clean = Some(100.0); // par => real yield = 4% Street exactly

    // Non-flat nominal curve: DF knots chosen so the annually-compounded zero
    // is 1% at 1y, 2% at 5y and exactly 3% at the bond's maturity.
    let t_mat = DayCount::Act365F
        .year_fraction(as_of, maturity, DayCountContext::default())
        .unwrap();
    let nominal_annual = 0.03_f64;
    // Day count set explicitly: the builder would otherwise infer ACT/360 from
    // the "USD-OIS" id and t(maturity) would miss the knot placed at `t_mat`.
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, 1.01_f64.powi(-1)),
            (5.0, 1.02_f64.powi(-5)),
            (t_mat, (1.0 + nominal_annual).powf(-t_mat)),
        ])
        .build()
        .unwrap();
    // Flat CPI so the real-yield leg is unaffected by inflation.
    let inflation = InflationCurve::builder("US-CPI")
        .base_date(as_of)
        .base_cpi(100.0)
        .knots([(0.0, 100.0), (11.0, 100.0)])
        .build()
        .unwrap();
    let ctx = MarketContext::new().insert(discount).insert(inflation);

    let result = bond
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::BreakevenInflation],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let breakeven = result.measures[MetricId::BreakevenInflation.as_str()];

    // Hand computation: real_annual = (1 + 0.04/2)^2 - 1 = 4.04%.
    let real_annual = 1.02_f64.powi(2) - 1.0;
    let expected = (1.0 + nominal_annual) / (1.0 + real_annual) - 1.0;

    // 0.1 bp tolerance: the real-yield leg comes from an iterative solver, so
    // a few 1e-6 of convergence noise is expected; convention errors (wrong
    // curve, wrong tenor, flat-curve shortcut) are all >= 40 bp away.
    assert!(
        (breakeven - expected).abs() < 1e-5,
        "breakeven mismatch: got {breakeven:.8}, expected {expected:.8}"
    );

    // Discriminator: reading the 1y zero instead of the maturity zero (or a
    // flat-curve shortcut) would move the result by ~2% — far outside tolerance.
    let wrong_short_end = (1.0 + 0.01) / (1.0 + real_annual) - 1.0;
    assert!(
        (breakeven - wrong_short_end).abs() > 1e-3,
        "test is not discriminating against short-end zero"
    );
}

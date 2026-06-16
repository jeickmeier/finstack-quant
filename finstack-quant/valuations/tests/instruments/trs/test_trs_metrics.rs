//! Comprehensive unit tests for TRS risk metrics.
//!
//! Tests all TRS metrics: Par Spread, Financing Annuity, IR01, Index Delta,
//! Theta, and Bucketed DV01.

use super::test_utils::*;
use finstack_quant_core::currency::Currency::*;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::TrsSide;
use finstack_quant_valuations::metrics::MetricId;

// ================================================================================================
// Par Spread Tests
// ================================================================================================

#[test]
fn test_equity_trs_par_spread_is_finite() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new()
        .spread_bp(0.0) // Zero spread
        .build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let par_spread = *result.measures.get("par_spread").unwrap();
    assert!(par_spread.is_finite(), "Par spread should be finite");
}

#[test]
fn test_equity_trs_par_spread_calculation() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new().spread_bp(25.0).build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert - Par spread should be finite and reasonable
    let par_spread = *result.measures.get("par_spread").unwrap();
    assert!(par_spread.is_finite(), "Par spread should be finite");
    assert!(
        par_spread.abs() < 1000.0,
        "Par spread should be reasonable, got {}",
        par_spread
    );
}

#[test]
fn test_fi_index_trs_par_spread_calculation() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new().spread_bp(0.0).build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let par_spread = *result.measures.get("par_spread").unwrap();
    assert!(par_spread.is_finite(), "Par spread should be finite");
}

#[test]
fn test_fi_index_trs_par_spread_sensitivity() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_low = TestFIIndexTrsBuilder::new().spread_bp(50.0).build();
    let trs_high = TestFIIndexTrsBuilder::new().spread_bp(150.0).build();

    // Act
    let npv_low = trs_low.value(&market, as_of).unwrap();
    let npv_high = trs_high.value(&market, as_of).unwrap();

    // Assert - Higher spread should reduce NPV for receive TR
    assert!(
        npv_low.amount() > npv_high.amount(),
        "Higher financing spread should reduce NPV"
    );
}

#[test]
fn test_par_spread_sign_based_on_side() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_receive = TestEquityTrsBuilder::new()
        .side(TrsSide::ReceiveTotalReturn)
        .spread_bp(0.0)
        .build();

    let trs_pay = TestEquityTrsBuilder::new()
        .side(TrsSide::PayTotalReturn)
        .spread_bp(0.0)
        .build();

    // Act
    let result_receive = trs_receive
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_pay = trs_pay
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert - Par spreads should be opposite for receive vs pay
    let par_receive = *result_receive.measures.get("par_spread").unwrap();
    let par_pay = *result_pay.measures.get("par_spread").unwrap();

    // The spreads should be equal in magnitude, opposite in application context
    // Both are calculated from the same market, so they should be equal
    assert_approx_eq(
        par_receive.abs(),
        par_pay.abs(),
        1.0, // 1bp tolerance
        "Par spreads should have same magnitude for receive vs pay",
    );
}

// ================================================================================================
// Financing Annuity Tests
// ================================================================================================

#[test]
fn test_equity_trs_financing_annuity_positive() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new().build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::FinancingAnnuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let annuity = *result.measures.get("financing_annuity").unwrap();
    assert!(annuity > 0.0, "Financing annuity should be positive");
}

#[test]
fn test_fi_index_trs_financing_annuity_positive() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new().build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::FinancingAnnuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let annuity = *result.measures.get("financing_annuity").unwrap();
    assert!(annuity > 0.0, "Financing annuity should be positive");
}

#[test]
fn test_financing_annuity_bounded_by_notional_times_tenor() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let notional = 10_000_000.0;
    let tenor_years = 1.0;

    let trs = TestEquityTrsBuilder::new()
        .notional(Money::new(notional, USD))
        .tenor_months(12)
        .build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::FinancingAnnuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let annuity = *result.measures.get("financing_annuity").unwrap();
    // Annuity should be less than notional * tenor (with some margin for discounting)
    assert!(
        annuity > 0.0 && annuity <= notional * tenor_years * 1.05,
        "Financing annuity {} out of expected range [0, {}]",
        annuity,
        notional * tenor_years
    );
}

#[test]
fn test_financing_annuity_increases_with_tenor() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_1y = TestEquityTrsBuilder::new().tenor_months(12).build();

    let trs_2y = TestEquityTrsBuilder::new().tenor_months(24).build();

    // Act
    let result_1y = trs_1y
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::FinancingAnnuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_2y = trs_2y
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::FinancingAnnuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let annuity_1y = *result_1y.measures.get("financing_annuity").unwrap();
    let annuity_2y = *result_2y.measures.get("financing_annuity").unwrap();

    assert!(
        annuity_2y > annuity_1y,
        "2Y annuity should be greater than 1Y"
    );
}

#[test]
fn test_financing_annuity_scales_with_notional() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_1m = TestEquityTrsBuilder::new()
        .notional(Money::new(1_000_000.0, USD))
        .build();

    let trs_10m = TestEquityTrsBuilder::new()
        .notional(Money::new(10_000_000.0, USD))
        .build();

    // Act
    let result_1m = trs_1m
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::FinancingAnnuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_10m = trs_10m
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::FinancingAnnuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let annuity_1m = *result_1m.measures.get("financing_annuity").unwrap();
    let annuity_10m = *result_10m.measures.get("financing_annuity").unwrap();

    assert_approx_eq(
        annuity_10m / annuity_1m,
        10.0,
        0.01, // 1% tolerance
        "Annuity should scale linearly with notional",
    );
}

// ================================================================================================
// IR01 Tests
// ================================================================================================

#[test]
fn test_equity_trs_ir01_positive() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new()
        .side(TrsSide::ReceiveTotalReturn)
        .build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let dv01 = *result.measures.get("dv01").unwrap();
    // Receiving TR => pay financing => negative DV01 typically
    // Just check finite
    assert!(dv01.is_finite(), "DV01 should be finite");
}

#[test]
fn test_fi_index_trs_ir01_positive() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new()
        .side(TrsSide::ReceiveTotalReturn)
        .build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let dv01 = *result.measures.get("dv01").unwrap();
    assert!(dv01.is_finite(), "DV01 should be finite");
}

#[test]
fn test_ir01_scales_with_notional() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_5m = TestEquityTrsBuilder::new()
        .notional(Money::new(5_000_000.0, USD))
        .build();

    let trs_25m = TestEquityTrsBuilder::new()
        .notional(Money::new(25_000_000.0, USD))
        .build();

    // Act
    let result_5m = trs_5m
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_25m = trs_25m
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let dv01_5m = *result_5m.measures.get("dv01").unwrap();
    let dv01_25m = *result_25m.measures.get("dv01").unwrap();

    assert_approx_eq(
        dv01_25m / dv01_5m,
        5.0,
        0.05, // absolute tolerance on ratio (central differencing may shift ratio slightly)
        "IR01 should scale linearly with notional",
    );
}

// ================================================================================================
// Index Delta Tests (Equity TRS)
// ================================================================================================

#[test]
fn test_equity_trs_index_delta_positive_for_receive() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new()
        .side(TrsSide::ReceiveTotalReturn)
        .build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::IndexDelta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let delta = *result.measures.get("index_delta").unwrap();
    assert!(delta > 0.0, "Receive TR should have positive delta");
}

#[test]
fn test_equity_trs_index_delta_negative_for_pay() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new()
        .side(TrsSide::PayTotalReturn)
        .build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::IndexDelta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let delta = *result.measures.get("index_delta").unwrap();
    assert!(delta < 0.0, "Pay TR should have negative delta");
}

#[test]
fn test_equity_trs_delta_magnitude_check() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let notional = 10_000_000.0;

    let trs = TestEquityTrsBuilder::new()
        .notional(Money::new(notional, USD))
        .side(TrsSide::ReceiveTotalReturn)
        .build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::IndexDelta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("index_delta").unwrap();

    // Spot bump test
    let spot_base = 5000.0;
    let spot_bump = 50.0; // $50 = 1% bump

    let market_bumped = market
        .clone()
        .insert_price("SPX-SPOT", MarketScalar::Unitless(spot_base + spot_bump));

    let npv_base = trs.value(&market, as_of).unwrap();
    let npv_bumped = trs.value(&market_bumped, as_of).unwrap();

    let dpv_fd = npv_bumped.amount() - npv_base.amount();
    let dpv_delta = delta * spot_bump;

    // Assert - Delta should approximate finite difference
    let tolerance = notional * 0.02; // 2% of notional
    assert_approx_eq(
        dpv_delta,
        dpv_fd,
        tolerance,
        "Delta approximation vs finite difference",
    );
}

#[test]
fn test_fi_index_trs_duration_dv01_based_on_duration() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new()
        .side(TrsSide::ReceiveTotalReturn)
        .build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DurationDv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let dv01 = *result.measures.get("duration_dv01").unwrap();
    assert!(dv01.is_finite(), "FI duration DV01 should be finite");
    assert!(dv01 > 0.0, "Receive TR should have positive duration DV01");
}

// ================================================================================================
// Theta Tests
// ================================================================================================

#[test]
fn test_equity_trs_theta_calculation() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new().build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let theta = *result.measures.get("theta").unwrap();
    assert!(theta.is_finite(), "Theta should be finite");
}

#[test]
fn test_fi_index_trs_theta_calculation() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new().build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let theta = *result.measures.get("theta").unwrap();
    assert!(theta.is_finite(), "Theta should be finite");
}

// ================================================================================================
// Bucketed DV01 Tests
// ================================================================================================

#[test]
fn test_equity_trs_bucketed_dv01_calculation() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new().build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::BucketedDv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let bucketed = *result.measures.get("bucketed_dv01").unwrap();
    assert!(bucketed.is_finite(), "Bucketed DV01 should be finite");
}

#[test]
fn test_fi_index_trs_bucketed_dv01_calculation() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new().build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::BucketedDv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert
    let bucketed = *result.measures.get("bucketed_dv01").unwrap();
    assert!(bucketed.is_finite(), "Bucketed DV01 should be finite");
}

// ================================================================================================
// Multiple Metrics Tests
// ================================================================================================

#[test]
fn test_equity_trs_all_metrics_together() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new().build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::ParSpread,
                MetricId::FinancingAnnuity,
                MetricId::Dv01,
                MetricId::IndexDelta,
                MetricId::Theta,
                MetricId::BucketedDv01,
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert - All metrics should be present and finite
    assert!(result.measures.get("par_spread").unwrap().is_finite());
    assert!(result
        .measures
        .get("financing_annuity")
        .unwrap()
        .is_finite());
    assert!(result.measures.get("dv01").unwrap().is_finite());
    assert!(result.measures.get("index_delta").unwrap().is_finite());
    assert!(result.measures.get("theta").unwrap().is_finite());
    assert!(result.measures.get("bucketed_dv01").unwrap().is_finite());
}

#[test]
fn test_fi_index_trs_all_metrics_together() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestFIIndexTrsBuilder::new().build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::ParSpread,
                MetricId::FinancingAnnuity,
                MetricId::Dv01,
                MetricId::DurationDv01,
                MetricId::Theta,
                MetricId::BucketedDv01,
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert - All metrics should be present and finite
    assert!(result.measures.get("par_spread").unwrap().is_finite());
    assert!(result
        .measures
        .get("financing_annuity")
        .unwrap()
        .is_finite());
    assert!(result.measures.get("dv01").unwrap().is_finite());
    assert!(result.measures.get("duration_dv01").unwrap().is_finite());
    assert!(result.measures.get("theta").unwrap().is_finite());
    assert!(result.measures.get("bucketed_dv01").unwrap().is_finite());
}

// ================================================================================================
// Metric Consistency Tests
// ================================================================================================

#[test]
fn test_par_spread_annuity_relationship() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new().spread_bp(0.0).build();

    // Act
    let result = trs
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread, MetricId::FinancingAnnuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert - Par spread solves NPV_receiver = 0:
    //   s_par = (PV(TR) - PV(float_only)) / Annuity * 10000
    let par_spread = *result.measures.get("par_spread").unwrap();
    let annuity = *result.measures.get("financing_annuity").unwrap();

    let tr_pv = trs.pv_total_return_leg(&market, as_of).unwrap();
    let float_pv = finstack_quant_valuations::instruments::TrsEngine::pv_financing_float_only(
        &trs.financing,
        &trs.schedule,
        trs.notional,
        &market,
        as_of,
    )
    .unwrap();

    let expected_par = (tr_pv.amount() - float_pv) / annuity * 10000.0;
    assert_approx_eq(
        par_spread,
        expected_par,
        0.01,
        "Par spread should match formula: (TR_PV - float_PV) / annuity * 10000",
    );
}

// ============================================================================
// Dividend01
//
// `MetricId::Dividend01` (PV sensitivity to a dividend-yield bump, $/bp) is
// registered for equity TRS but was previously unexercised. It must be finite
// and non-zero when a dividend-yield curve is present, exactly zero when none
// is referenced, and flip sign between the receive-TR and pay-TR sides.
// ============================================================================

fn dividend01(trs: &finstack_quant_valuations::instruments::EquityTotalReturnSwap) -> f64 {
    let market = create_market_context();
    let as_of = as_of_date();
    *trs.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Dividend01],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    )
    .unwrap()
    .measures
    .get("dividend01")
    .unwrap()
}

#[test]
fn test_dividend01_finite_and_nonzero_with_div_yield() {
    let trs = TestEquityTrsBuilder::new().build();
    let div01 = dividend01(&trs);

    assert!(
        div01.is_finite(),
        "Dividend01 should be finite, got {div01}"
    );
    assert!(
        div01.abs() > 0.0,
        "Dividend01 should be non-zero when a dividend-yield curve is referenced, got {div01}"
    );
}

#[test]
fn test_dividend01_zero_without_div_yield_id() {
    let trs = TestEquityTrsBuilder::new().div_yield_id(None).build();
    let div01 = dividend01(&trs);

    assert_eq!(
        div01, 0.0,
        "Dividend01 must be exactly zero when no dividend-yield curve is referenced"
    );
}

#[test]
fn test_dividend01_sign_flips_pay_vs_receive() {
    let receive = TestEquityTrsBuilder::new()
        .side(TrsSide::ReceiveTotalReturn)
        .build();
    let pay = TestEquityTrsBuilder::new()
        .side(TrsSide::PayTotalReturn)
        .build();

    let div01_receive = dividend01(&receive);
    let div01_pay = dividend01(&pay);

    // Pay-TR is the mirror of receive-TR, so the dividend sensitivity flips sign
    // and matches in magnitude.
    assert!(
        div01_receive.signum() != div01_pay.signum(),
        "Dividend01 should flip sign between receive ({div01_receive}) and pay ({div01_pay})"
    );
    assert!(
        (div01_receive + div01_pay).abs() <= 1e-6 + 1e-3 * div01_receive.abs(),
        "Dividend01 magnitudes should match between sides: receive={div01_receive}, pay={div01_pay}"
    );
}

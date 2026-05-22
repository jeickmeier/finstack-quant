//! Tests for Tsiveriotis-Zhang credit/equity decomposition.
//!
//! Exercises the code path where `credit_curve_id` is set to a separate curve
//! with meaningful credit spread. These tests verify that:
//! - Wider credit spreads lower the bond floor (cash component discounted at risky rate)
//! - The equity component is still discounted at risk-free (conversion value preserved)
//! - CS01 is non-zero when a credit curve is present
//! - DV01 and CS01 are distinct (one bumps risk-free, the other bumps credit)

use super::fixtures::*;
use finstack_valuations::instruments::fixed_income::convertible::{
    price_convertible_bond, ConvertibleTreeType,
};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;

const TREE: ConvertibleTreeType = ConvertibleTreeType::Binomial(100);

#[test]
fn test_credit_spread_reduces_price() {
    let bond = create_convertible_with_credit();
    let bond_no_credit = create_standard_convertible();

    // Same market but the credit bond uses a separate, wider credit curve
    let market_with_credit = create_market_context_with_credit(200.0); // 200bp spread
    let market_no_credit = create_market_context();

    let price_with_credit =
        price_convertible_bond(&bond, &market_with_credit, TREE, dates::base_date())
            .expect("should price with credit curve");
    let price_no_credit =
        price_convertible_bond(&bond_no_credit, &market_no_credit, TREE, dates::base_date())
            .expect("should price without credit curve");

    // With credit spread, the cash component is discounted more aggressively,
    // so total price should be lower (OTM convertibles affected most).
    // For ITM (spot=150, conv_value=1500 >> par), the effect is smaller since
    // the equity component dominates. But the bond floor component is reduced.
    assert!(
        price_with_credit.amount() < price_no_credit.amount(),
        "Credit spread should reduce price: with_credit={}, no_credit={}",
        price_with_credit.amount(),
        price_no_credit.amount()
    );
}

#[test]
fn test_wider_credit_spread_reduces_price_further() {
    let bond = create_convertible_with_credit();

    let market_100bp = create_market_context_with_credit(100.0);
    let market_300bp = create_market_context_with_credit(300.0);

    let price_100bp = price_convertible_bond(&bond, &market_100bp, TREE, dates::base_date())
        .expect("should price at 100bp spread");
    let price_300bp = price_convertible_bond(&bond, &market_300bp, TREE, dates::base_date())
        .expect("should price at 300bp spread");

    assert!(
        price_300bp.amount() < price_100bp.amount(),
        "Wider spread should reduce price: 300bp={}, 100bp={}",
        price_300bp.amount(),
        price_100bp.amount()
    );
}

#[test]
fn test_credit_spread_effect_larger_for_otm() {
    // OTM convertibles are more bond-like, so credit spread has a bigger impact
    let bond = create_convertible_with_credit();

    // OTM market: spot=50, conversion_value=500 << par
    let otm_no_spread = create_market_context_with_params(
        market_params::SPOT_LOW,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );
    let otm_with_spread = {
        let base_date = dates::base_date();
        let rf_rate = market_params::RISK_FREE_RATE;
        let credit_rate = rf_rate + 0.02; // 200bp spread
        use finstack_core::market_data::scalars::MarketScalar;
        use finstack_core::market_data::term_structures::DiscountCurve;
        use finstack_core::math::interp::InterpStyle;

        let rf_curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (10.0, (-rf_rate * 10.0).exp())])
            .interp(InterpStyle::Linear)
            .build()
            .unwrap();
        let credit_curve = DiscountCurve::builder("USD-CREDIT")
            .base_date(base_date)
            .knots([(0.0, 1.0), (10.0, (-credit_rate * 10.0).exp())])
            .interp(InterpStyle::Linear)
            .build()
            .unwrap();

        finstack_core::market_data::context::MarketContext::new()
            .insert(rf_curve)
            .insert(credit_curve)
            .insert_price("AAPL", MarketScalar::Unitless(market_params::SPOT_LOW))
            .insert_price(
                "AAPL-VOL",
                MarketScalar::Unitless(market_params::VOL_STANDARD),
            )
            .insert_price(
                "AAPL-DIVYIELD",
                MarketScalar::Unitless(market_params::DIV_YIELD),
            )
    };

    let mut bond_no_credit = create_standard_convertible();
    bond_no_credit.credit_curve_id = None;

    let otm_price_no_credit =
        price_convertible_bond(&bond_no_credit, &otm_no_spread, TREE, dates::base_date())
            .expect("should price OTM no credit");
    let otm_price_with_credit =
        price_convertible_bond(&bond, &otm_with_spread, TREE, dates::base_date())
            .expect("should price OTM with credit");

    let otm_impact_pct = (otm_price_no_credit.amount() - otm_price_with_credit.amount())
        / otm_price_no_credit.amount();

    // ITM: use standard spot=150
    let itm_no_spread = create_market_context();
    let itm_with_spread = create_market_context_with_credit(200.0);

    let itm_price_no_credit =
        price_convertible_bond(&bond_no_credit, &itm_no_spread, TREE, dates::base_date())
            .expect("should price ITM no credit");
    let itm_price_with_credit =
        price_convertible_bond(&bond, &itm_with_spread, TREE, dates::base_date())
            .expect("should price ITM with credit");

    let itm_impact_pct = (itm_price_no_credit.amount() - itm_price_with_credit.amount())
        / itm_price_no_credit.amount();

    assert!(
        otm_impact_pct > itm_impact_pct,
        "Credit spread impact should be larger OTM ({:.4}%) than ITM ({:.4}%)",
        otm_impact_pct * 100.0,
        itm_impact_pct * 100.0
    );
}

#[test]
fn test_cs01_nonzero_with_credit_curve() {
    let bond = create_convertible_with_credit();
    let market = create_market_context_with_credit(200.0);

    let result = bond
        .price_with_metrics(
            &market,
            dates::base_date(),
            &[MetricId::Cs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("should compute CS01");

    let cs01 = *result.measures.get("cs01").expect("CS01 should be present");

    // CS01 should be negative (wider credit spread reduces PV)
    assert!(
        cs01 < 0.0,
        "CS01 should be negative for a bond with credit curve: got {}",
        cs01
    );

    // CS01 magnitude: for a 5Y bond at 200bp spread, expect a non-trivial value
    assert!(
        cs01.abs() > 0.001,
        "CS01 magnitude should be meaningful: got {}",
        cs01
    );
}

#[test]
fn test_cs01_zero_without_credit_curve() {
    let bond = create_standard_convertible(); // no credit_curve_id
    let market = create_market_context();

    let result = bond
        .price_with_metrics(
            &market,
            dates::base_date(),
            &[MetricId::Cs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("should compute CS01");

    let cs01 = *result.measures.get("cs01").expect("CS01 should be present");

    // Without a separate credit curve, CS01 should be zero
    assert!(
        cs01.abs() < 1e-10,
        "CS01 should be zero without credit curve: got {}",
        cs01
    );
}

#[test]
fn test_dv01_and_cs01_are_distinct() {
    let bond = create_convertible_with_credit();
    let market = create_market_context_with_credit(200.0);

    let result = bond
        .price_with_metrics(
            &market,
            dates::base_date(),
            &[MetricId::Dv01, MetricId::Cs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("should compute DV01 and CS01");

    let dv01 = *result.measures.get("dv01").expect("DV01 should be present");
    let cs01 = *result.measures.get("cs01").expect("CS01 should be present");

    // Both should be non-zero and different
    assert!(dv01.abs() > 1e-6, "DV01 should be non-zero: {}", dv01);
    assert!(cs01.abs() > 1e-6, "CS01 should be non-zero: {}", cs01);
    assert!(
        (dv01 - cs01).abs() > 1e-6,
        "DV01 ({}) and CS01 ({}) should differ (different curves bumped)",
        dv01,
        cs01
    );
}

/// Key-rate (bucketed) CS01 must reconcile to the parallel CS01.
///
/// `ConvertibleBucketedCs01Calculator` applies a *triangular key-rate* shock to
/// the credit curve at each standard bucket tenor. Because the triangular
/// weights are a partition of unity over every relevant cashflow tenor, the
/// summed key-rate shocks equal a single parallel shock, so the per-bucket
/// central-difference CS01s sum (to within finite-difference tolerance) to the
/// parallel CS01 produced by `Cs01Calculator`.
#[test]
fn test_convertible_bucketed_cs01_reconciles_to_parallel() {
    let bond = create_convertible_with_credit();
    let market = create_market_context_with_credit(200.0);

    let result = bond
        .price_with_metrics(
            &market,
            dates::base_date(),
            &[MetricId::Cs01, MetricId::BucketedCs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("should compute Cs01 and BucketedCs01");

    let cs01 = *result
        .measures
        .get("cs01")
        .expect("parallel Cs01 should be computed");
    let bucketed = *result
        .measures
        .get("bucketed_cs01")
        .expect("BucketedCs01 should be computed");

    assert!(
        cs01.is_finite() && bucketed.is_finite(),
        "CS01 metrics must be finite (cs01={cs01}, bucketed={bucketed})"
    );
    assert!(
        cs01.abs() > 1e-6,
        "parallel CS01 should be non-trivial for a convertible with a credit curve: {cs01}"
    );
    // Summed triangular key-rate shocks = parallel shock → bucketed ≈ parallel.
    // Tolerance covers third-order Taylor terms and tree discretisation noise.
    assert!(
        (bucketed - cs01).abs() <= 1e-5 + 1e-4 * cs01.abs(),
        "bucketed CS01 ({bucketed}) must reconcile to parallel CS01 ({cs01})"
    );

    // The per-tenor series must also be present and sum to the same total.
    let series_sum: f64 = result
        .measures
        .iter()
        .filter(|(k, _)| k.as_str().starts_with("bucketed_cs01::"))
        .map(|(_, v)| *v)
        .sum();
    assert!(
        (series_sum - cs01).abs() <= 1e-5 + 1e-4 * cs01.abs(),
        "per-tenor bucketed_cs01 series ({series_sum}) must sum to parallel CS01 ({cs01})"
    );
}

#[test]
fn test_recovery_rate_increases_price() {
    // With recovery > 0, the credit spread effect is reduced, so PV increases.
    let mut bond_zero_recovery = create_convertible_with_credit();
    bond_zero_recovery.recovery_rate = Some(0.0);

    let mut bond_40pct_recovery = create_convertible_with_credit();
    bond_40pct_recovery.recovery_rate = Some(0.40);

    let market = create_market_context_with_credit(200.0);

    let price_zero = price_convertible_bond(&bond_zero_recovery, &market, TREE, dates::base_date())
        .expect("should price zero recovery");

    let price_40 = price_convertible_bond(&bond_40pct_recovery, &market, TREE, dates::base_date())
        .expect("should price 40% recovery");

    assert!(
        price_40.amount() > price_zero.amount(),
        "40% recovery should increase price vs zero recovery: 40%={}, 0%={}",
        price_40.amount(),
        price_zero.amount()
    );
}

#[test]
fn test_recovery_rate_100_pct_equals_no_credit() {
    // With 100% recovery, the credit curve has no effect (cash discounted at rf).
    let mut bond_full_recovery = create_convertible_with_credit();
    bond_full_recovery.recovery_rate = Some(1.0);

    let bond_no_credit = create_standard_convertible();

    let market_with_credit = create_market_context_with_credit(200.0);
    let market_no_credit = create_market_context();

    let price_full_recovery = price_convertible_bond(
        &bond_full_recovery,
        &market_with_credit,
        TREE,
        dates::base_date(),
    )
    .expect("should price full recovery");

    let price_no_credit =
        price_convertible_bond(&bond_no_credit, &market_no_credit, TREE, dates::base_date())
            .expect("should price no credit");

    // Should be very close since 100% recovery = risk-free discounting on cash
    let diff_pct =
        (price_full_recovery.amount() - price_no_credit.amount()).abs() / price_no_credit.amount();
    assert!(
        diff_pct < 0.005,
        "100% recovery should match no-credit price: full_recovery={}, no_credit={}, diff={:.4}%",
        price_full_recovery.amount(),
        price_no_credit.amount(),
        diff_pct * 100.0,
    );
}

#[test]
fn test_recovery_rate_monotonic() {
    // Higher recovery rate should always increase PV (for same credit curve)
    let market = create_market_context_with_credit(300.0);

    let recovery_levels = [0.0, 0.20, 0.40, 0.60, 0.80];
    let mut prev_price = 0.0;

    for &r in &recovery_levels {
        let mut bond = create_convertible_with_credit();
        bond.recovery_rate = Some(r);

        let price = price_convertible_bond(&bond, &market, TREE, dates::base_date())
            .expect("should price")
            .amount();

        if r > 0.0 {
            assert!(
                price >= prev_price - 0.01, // small tolerance for tree discretization
                "Price should increase with recovery: R={}, price={}, prev={}",
                r,
                price,
                prev_price
            );
        }
        prev_price = price;
    }
}

#[test]
fn test_greeks_with_credit_curve() {
    let bond = create_convertible_with_credit();
    let market = create_market_context_with_credit(200.0);

    let result = bond
        .price_with_metrics(
            &market,
            dates::base_date(),
            &[
                MetricId::Delta,
                MetricId::Gamma,
                MetricId::Vega,
                MetricId::Rho,
            ],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("should compute Greeks with credit curve");

    let delta = *result.measures.get("delta").expect("delta present");
    let gamma = *result.measures.get("gamma").expect("gamma present");
    let vega = *result.measures.get("vega").expect("vega present");
    let rho = *result.measures.get("rho").expect("rho present");

    // Delta should be positive (ITM at spot=150)
    assert!(delta > 0.0, "Delta should be positive: {}", delta);
    // Gamma should be non-negative
    assert!(gamma >= -1e-6, "Gamma should be non-negative: {}", gamma);
    // Vega should be non-negative (option value increases with vol)
    assert!(vega >= -1e-6, "Vega should be non-negative: {}", vega);
    // Rho should be finite
    assert!(rho.is_finite(), "Rho should be finite: {}", rho);
}

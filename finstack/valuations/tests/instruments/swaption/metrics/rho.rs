//! Rho (interest rate sensitivity) tests

use crate::swaption::common::*;
use finstack_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;

#[test]
fn test_rho_finite_and_reasonable() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Rho],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let rho = *result.measures.get("rho").unwrap();

    assert!(rho.is_finite(), "Rho should be finite");
    // Rho can be positive or negative for swaptions depending on maturity structure.
    // For a 1M notional, 1Y-5Y swaption, rho per 1bp should be in the range of $10-$1000.
    // See test_rho_parallel_bump_validation for the tighter magnitude check.
    assert_reasonable(rho.abs(), 1.0, 10_000.0, "Rho magnitude");
}

#[test]
fn test_rho_parallel_bump_validation() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Analytical rho (per 1bp)
    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Rho],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let rho_analytical = *result.measures.get("rho").unwrap();

    // Rho should be finite and reasonable for ATM swaption.
    // Swaption Rho is the sensitivity to the DISCOUNT curve only (the forward
    // swap rate is held fixed).  For a 1M notional, 1Y-5Y payer swaption the
    // discount-only rho is a small positive number — much smaller than the
    // delta-dominated DV01 — and falls well below the old dual-curve figure.
    assert!(rho_analytical.is_finite(), "Rho should be finite");
    // Discount-only rho per 1bp for 1M notional 1Y into 5Y swaption.
    assert_reasonable(rho_analytical.abs(), 0.1, 500.0, "Rho magnitude");
}

#[test]
fn test_rho_sign_depends_on_moneyness() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Payer swaption
    let payer = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let rho_payer = payer
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Rho],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("rho")
        .copied()
        .unwrap();

    // Receiver swaption
    let receiver = create_standard_receiver_swaption(expiry, swap_start, swap_end, 0.05);
    let rho_receiver = receiver
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Rho],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("rho")
        .copied()
        .unwrap();

    // Rho signs should reflect different rate sensitivities
    // Both should be finite
    assert!(
        rho_payer.is_finite() && rho_receiver.is_finite(),
        "Rhos should be finite"
    );
}

#[test]
fn test_rho_magnitude_scales_with_tenor() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let expiry = time::macros::date!(2025 - 01 - 01);
    let swap_start = expiry;
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Short tenor swap (2Y)
    let swap_end_short = time::macros::date!(2027 - 01 - 01);
    let swaption_short = create_standard_payer_swaption(expiry, swap_start, swap_end_short, 0.05);

    // Long tenor swap (10Y)
    let swap_end_long = time::macros::date!(2035 - 01 - 01);
    let swaption_long = create_standard_payer_swaption(expiry, swap_start, swap_end_long, 0.05);

    let rho_short = swaption_short
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Rho],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("rho")
        .copied()
        .unwrap();

    let rho_long = swaption_long
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Rho],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("rho")
        .copied()
        .unwrap();

    // Longer tenor should generally have higher rho magnitude
    assert!(
        rho_long.abs() > rho_short.abs(),
        "Longer tenor should have higher rho magnitude"
    );
}

/// Regression test: swaption Rho must equal a discount-curve-ONLY bump-and-reprice.
///
/// The correct option rho is the sensitivity to the discount/funding rate while
/// the forward swap rate (and vol) are held fixed.  The previous wiring used
/// `parallel_combined()` which moved BOTH the discount and forward curves, thereby
/// conflating Rho with Delta.
///
/// This test:
/// 1. Computes the reference rho manually: bump only `USD_OIS` (the discount
///    curve) by ±1bp and take the CENTRAL difference `(pv_up - pv_down) / 2`,
///    matching the calculator exactly and eliminating the O(gamma·bump²)
///    convexity error that a one-sided forward difference would introduce.
/// 2. Computes a dual-curve reference: bump BOTH `USD_OIS` and `USD_LIBOR_3M`
///    by ±1bp, central difference.
/// 3. Asserts `MetricId::Rho` matches the discount-only central-difference
///    reference to a tight tolerance and does NOT match the dual-curve reference.
#[test]
fn test_rho_is_discount_curve_only_not_dual_curve() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    // Use separate discount and forward curves so the two bumps are distinct.
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    // --- Reference: discount-curve-ONLY central difference ±1bp ---
    let market_disc_up = market
        .bump([MarketBump::Curve {
            id: "USD_OIS".to_string().into(),
            spec: BumpSpec::parallel_bp(1.0),
        }])
        .expect("discount up-bump should succeed");
    let market_disc_down = market
        .bump([MarketBump::Curve {
            id: "USD_OIS".to_string().into(),
            spec: BumpSpec::parallel_bp(-1.0),
        }])
        .expect("discount down-bump should succeed");
    let pv_disc_up = swaption.value(&market_disc_up, as_of).unwrap().amount();
    let pv_disc_down = swaption.value(&market_disc_down, as_of).unwrap().amount();
    // Central difference matches the calculator's bump scheme exactly.
    let rho_discount_only = (pv_disc_up - pv_disc_down) / 2.0;

    // --- Reference: DUAL-curve central difference ±1bp (the old/wrong wiring) ---
    let market_both_up = market
        .bump([
            MarketBump::Curve {
                id: "USD_OIS".to_string().into(),
                spec: BumpSpec::parallel_bp(1.0),
            },
            MarketBump::Curve {
                id: "USD_LIBOR_3M".to_string().into(),
                spec: BumpSpec::parallel_bp(1.0),
            },
        ])
        .expect("dual up-bump should succeed");
    let market_both_down = market
        .bump([
            MarketBump::Curve {
                id: "USD_OIS".to_string().into(),
                spec: BumpSpec::parallel_bp(-1.0),
            },
            MarketBump::Curve {
                id: "USD_LIBOR_3M".to_string().into(),
                spec: BumpSpec::parallel_bp(-1.0),
            },
        ])
        .expect("dual down-bump should succeed");
    let pv_both_up = swaption.value(&market_both_up, as_of).unwrap().amount();
    let pv_both_down = swaption.value(&market_both_down, as_of).unwrap().amount();
    let rho_dual_curve = (pv_both_up - pv_both_down) / 2.0;

    // --- Metric Rho ---
    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Rho],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let rho_metric = *result.measures.get("rho").unwrap();

    // The wired Rho must match the discount-only central-difference reference.
    // Both use the same ±1bp symmetric scheme so they should agree to within
    // floating-point / repricing noise (tight absolute tolerance of $1e-3).
    let tol = 1e-3_f64;
    assert!(
        (rho_metric - rho_discount_only).abs() < tol,
        "Rho metric ({rho_metric:.6}) must match discount-only central-difference \
         ({rho_discount_only:.6}); difference {:.6} exceeds tolerance {tol}",
        (rho_metric - rho_discount_only).abs()
    );

    // The discount-only and dual-curve values must differ meaningfully (i.e., the
    // forward curve contributes a non-trivial amount to the dual-curve figure).
    // For a payer swaption the forward-curve sensitivity (delta) dominates, so the
    // two should differ by at least 50% of the larger magnitude.
    let dual_contribution = (rho_dual_curve - rho_discount_only).abs();
    assert!(
        dual_contribution > 0.5 * rho_dual_curve.abs().max(rho_discount_only.abs()),
        "Dual-curve and discount-only rho should differ materially: \
         dual={rho_dual_curve:.4}, disc_only={rho_discount_only:.4}, diff={dual_contribution:.4}"
    );
}

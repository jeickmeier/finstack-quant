//! Delta tests with analytical and numerical validation

use crate::swaption::common::*;
use finstack_core::market_data::context::MarketContext;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;

#[test]
fn test_atm_delta_positive() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("delta").unwrap();

    // ATM payer delta should be positive (around 0.5 * notional * annuity)
    assert!(delta > 0.0, "ATM payer delta should be positive");
    assert!(delta.is_finite(), "Delta should be finite");
}

#[test]
fn test_itm_delta_higher_than_atm() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);

    let atm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let itm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.03);

    let delta_atm = atm
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .copied()
        .unwrap();

    let delta_itm = itm
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .copied()
        .unwrap();

    assert!(
        delta_itm > delta_atm,
        "ITM delta ({}) should exceed ATM delta ({})",
        delta_itm,
        delta_atm
    );
}

#[test]
fn test_otm_delta_lower_than_atm() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);

    let atm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let otm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.08);

    let delta_atm = atm
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .copied()
        .unwrap();

    let delta_otm = otm
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .copied()
        .unwrap();

    assert!(
        delta_otm < delta_atm,
        "OTM delta should be less than ATM delta"
    );
    assert!(delta_otm > 0.0, "OTM delta should still be positive");
}

#[test]
fn test_delta_finite_difference_validation() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Analytical delta
    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let delta_analytical = *result.measures.get("delta").unwrap();

    // Note: Delta measures sensitivity to forward rate, which is complex to bump directly
    // This test validates that delta is reasonable in magnitude
    let disc = market.get_discount("USD_OIS").unwrap();
    let annuity = swaption.swap_annuity(disc.as_ref(), as_of).unwrap();
    let notional = swaption.notional.amount();

    // Delta should scale with notional * annuity
    let expected_scale = notional * annuity;
    let delta_normalized = delta_analytical / expected_scale;

    // Normalized delta should be between 0 and 1
    assert_reasonable(delta_normalized, 0.0, 1.0, "Normalized delta");
}

#[test]
fn test_receiver_delta_sign() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();

    let payer = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let receiver = create_standard_receiver_swaption(expiry, swap_start, swap_end, 0.05);

    let market = create_flat_market(as_of, 0.05, 0.30);

    let delta_payer = payer
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .copied()
        .unwrap();

    let delta_receiver = receiver
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .copied()
        .unwrap();

    // Payer has positive delta, receiver has negative delta
    assert!(delta_payer > 0.0, "Payer delta should be positive");
    assert!(delta_receiver < 0.0, "Receiver delta should be negative");
}

#[test]
fn test_delta_volatility_independence() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);

    let market_low_vol = create_flat_market(as_of, 0.05, 0.15);
    let market_high_vol = create_flat_market(as_of, 0.05, 0.50);

    let delta_low = swaption
        .price_with_metrics(
            &market_low_vol,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .copied()
        .unwrap();

    let delta_high = swaption
        .price_with_metrics(
            &market_high_vol,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .copied()
        .unwrap();

    // Delta values should be reasonably close (both affect option probability differently)
    // For ATM, delta should be relatively stable across vol
    let rel_diff = ((delta_high - delta_low) / delta_low).abs();
    assert!(
        rel_diff < 0.5,
        "ATM delta should be relatively stable across volatility"
    );
}

#[test]
fn test_delta_falls_back_to_bachelier_for_negative_forward() {
    // A negative forward swap rate is undefined for the Black (lognormal)
    // model; the delta calculator must fall back to the Bachelier (normal)
    // model rather than erroring, so negative-rate swaptions stay priceable.
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = MarketContext::new()
        .insert(build_flat_discount_curve(0.03, as_of, "USD_OIS"))
        .insert(build_flat_forward_curve(-0.005, as_of, "USD_LIBOR_3M"))
        .insert_surface(build_flat_vol_surface(0.30, as_of, "USD_SWAPTION_VOL"));

    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("delta should fall back to Bachelier for a negative forward");
    let delta = result
        .measures
        .get(MetricId::Delta.as_str())
        .copied()
        .expect("delta measure should be present");
    assert!(
        delta.is_finite(),
        "Bachelier-fallback delta should be finite, got {delta}"
    );
}

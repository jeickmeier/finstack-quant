//! Numerical stability edge cases

use crate::swaption::common::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_deep_itm_stability() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.10, 0.30);

    // Deep ITM: strike much lower than forward
    let deep_itm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.01);

    let pv = deep_itm.value(&market, as_of).unwrap().amount();
    let result = deep_itm
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert!(
        pv > 0.0 && pv.is_finite(),
        "Deep ITM pricing should be stable"
    );
    assert!(
        result.measures.get("delta").unwrap().is_finite(),
        "Deep ITM delta should be finite"
    );
    assert!(
        result.measures.get("gamma").unwrap().is_finite(),
        "Deep ITM gamma should be finite"
    );
}

#[test]
fn test_deep_otm_stability() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.02, 0.30);

    // Deep OTM: strike much higher than forward
    let deep_otm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.20);

    let pv = deep_otm.value(&market, as_of).unwrap().amount();
    let result = deep_otm
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert!(
        pv >= 0.0 && pv.is_finite(),
        "Deep OTM pricing should be stable"
    );
    assert!(
        result.measures.get("delta").unwrap().is_finite(),
        "Deep OTM delta should be finite"
    );
    assert!(
        result.measures.get("vega").unwrap().is_finite(),
        "Deep OTM vega should be finite"
    );
}

#[test]
fn test_extreme_volatility_high() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);

    // Very high volatility (200%)
    let market = create_flat_market(as_of, 0.05, 2.0);

    let pv = swaption.value(&market, as_of).unwrap().amount();

    // Should handle gracefully
    assert!(
        pv > 0.0 && pv.is_finite(),
        "High vol pricing should be stable"
    );
}

#[test]
fn test_extreme_volatility_low() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);

    // Very low volatility (1%)
    let market = create_flat_market(as_of, 0.05, 0.01);

    let pv = swaption.value(&market, as_of).unwrap().amount();

    // Should handle gracefully
    assert!(
        pv >= 0.0 && pv.is_finite(),
        "Low vol pricing should be stable"
    );
}

#[test]
fn test_extreme_rates_high() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);

    // Very high rate (20%)
    let market = create_flat_market(as_of, 0.20, 0.30);

    let pv = swaption.value(&market, as_of).unwrap().amount();

    // Should handle gracefully
    assert!(
        pv > 0.0 && pv.is_finite(),
        "High rate pricing should be stable"
    );
}

#[test]
fn test_extreme_rates_low() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);

    // Low rate (2%)
    let market = create_flat_market(as_of, 0.02, 0.30);

    let pv = swaption.value(&market, as_of).unwrap().amount();

    // Should handle gracefully
    assert!(
        pv > 0.0 && pv.is_finite(),
        "Low rate pricing should be stable"
    );
}

#[test]
fn test_large_notional_scaling() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Very large notional (1 billion)
    let mut swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    swaption.notional = finstack_quant_core::money::Money::new(
        1_000_000_000.0,
        finstack_quant_core::currency::Currency::USD,
    );

    let pv = swaption.value(&market, as_of).unwrap().amount();
    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Should scale properly
    assert!(
        pv > 0.0 && pv.is_finite(),
        "Large notional pricing should be stable"
    );
    assert!(
        result.measures.get("delta").unwrap().is_finite(),
        "Large notional delta should be finite"
    );
}

#[test]
fn test_small_notional_scaling() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Very small notional (1000)
    let mut swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    swaption.notional = finstack_quant_core::money::Money::new(
        1_000.0,
        finstack_quant_core::currency::Currency::USD,
    );

    let pv = swaption.value(&market, as_of).unwrap().amount();
    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Should scale properly
    assert!(
        pv > 0.0 && pv.is_finite(),
        "Small notional pricing should be stable"
    );
    assert!(
        result.measures.get("delta").unwrap().is_finite(),
        "Small notional delta should be finite"
    );
}

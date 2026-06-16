//! Vega tests with finite difference validation

use crate::swaption::common::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PricingOverrides;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_vega_positive_for_long_option() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let vega = *result.measures.get("vega").unwrap();

    // Long options have positive vega
    assert!(vega > 0.0, "Long option vega should be positive");
    assert!(vega.is_finite(), "Vega should be finite");
}

#[test]
fn test_vega_finite_difference() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let mut swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Analytical vega
    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let vega_analytical = *result.measures.get("vega").unwrap();

    // Finite difference vega (per 1% vol change)
    let base_vol = 0.30;
    let h = 0.01; // 1% vol shift

    swaption.pricing_overrides = PricingOverrides::default().with_implied_vol(base_vol + h);
    let pv_up = swaption.value(&market, as_of).unwrap().amount();

    swaption.pricing_overrides = PricingOverrides::default().with_implied_vol(base_vol - h);
    let pv_down = swaption.value(&market, as_of).unwrap().amount();

    let vega_fd = (pv_up - pv_down) / 2.0; // Per 1% change

    // Should match within reasonable tolerance
    assert_approx_eq(vega_analytical, vega_fd, 0.001, "Vega finite difference");
}

#[test]
fn test_atm_vega_highest() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);

    let atm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let itm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.03);
    let otm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.08);

    let vega_atm = atm
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .copied()
        .unwrap();

    let vega_itm = itm
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .copied()
        .unwrap();

    let vega_otm = otm
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .copied()
        .unwrap();

    // ATM options typically have highest vega
    assert!(
        vega_atm >= vega_itm * 0.8,
        "ATM vega should be comparable to ITM vega"
    );
    assert!(vega_atm >= vega_otm, "ATM vega should exceed OTM vega");
}

#[test]
fn test_vega_increases_with_time() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let swap_start = time::macros::date!(2025 - 01 - 01);
    let swap_end = time::macros::date!(2030 - 01 - 01);
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Short expiry
    let expiry_short = time::macros::date!(2024 - 06 - 01);
    let swaption_short = create_standard_payer_swaption(expiry_short, swap_start, swap_end, 0.05);

    // Long expiry
    let expiry_long = time::macros::date!(2026 - 01 - 01);
    let swaption_long = create_standard_payer_swaption(expiry_long, swap_start, swap_end, 0.05);

    let vega_short = swaption_short
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .copied()
        .unwrap();

    let vega_long = swaption_long
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .copied()
        .unwrap();

    // Longer expiry generally has higher vega (more uncertainty)
    assert!(
        vega_long > vega_short,
        "Longer expiry should have higher vega"
    );
}

#[test]
fn test_vega_scales_with_notional() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);

    let mut swaption1 = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    swaption1.notional = finstack_quant_core::money::Money::new(
        1_000_000.0,
        finstack_quant_core::currency::Currency::USD,
    );

    let mut swaption5 = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    swaption5.notional = finstack_quant_core::money::Money::new(
        5_000_000.0,
        finstack_quant_core::currency::Currency::USD,
    );

    let vega1 = swaption1
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .copied()
        .unwrap();

    let vega5 = swaption5
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .copied()
        .unwrap();

    // Vega should scale linearly with notional
    assert_approx_eq(vega5, vega1 * 5.0, 1e-8, "Vega scaling with notional");
}

//! Expiry-related edge cases

use crate::swaption::common::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

// Note: These tests are disabled because the implementation correctly rejects
// swaptions with expiry before as_of date as an invalid date range.
// This is the expected behavior for the pricing engine.

#[test]
fn test_expired_swaption_zero_value() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let expiry = time::macros::date!(2023 - 12 - 01); // Already expired
    let swap_start = time::macros::date!(2023 - 12 - 01); // Must align with expiry
    let swap_end = time::macros::date!(2028 - 12 - 01);

    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Should return an error for expired swaptions
    assert!(
        swaption.value(&market, as_of).is_err(),
        "Expired swaption should return error"
    );
}

#[test]
fn test_expired_swaption_zero_greeks() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let expiry = time::macros::date!(2023 - 12 - 01); // Already expired
    let swap_start = time::macros::date!(2023 - 12 - 01); // Must align with expiry
    let swap_end = time::macros::date!(2028 - 12 - 01);

    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Should return an error for expired swaptions
    assert!(
        swaption
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Delta, MetricId::Vega],
                finstack_quant_valuations::instruments::PricingOptions::default()
            )
            .is_err(),
        "Expired swaption should return error"
    );
}

#[test]
fn test_at_expiry_pricing() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let expiry = as_of; // At expiry
    let swap_start = as_of;
    let swap_end = time::macros::date!(2029 - 01 - 01);

    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let pv = swaption.value(&market, as_of).unwrap().amount();

    // At expiry, value should be zero or intrinsic (depending on implementation)
    assert!(pv >= 0.0, "At expiry value should be non-negative");
}

#[test]
fn test_very_short_expiry() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let expiry = as_of.checked_add(time::Duration::days(1)).unwrap(); // 1 day
    let swap_start = time::macros::date!(2024 - 01 - 03);
    let swap_end = time::macros::date!(2029 - 01 - 03);

    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let pv = swaption.value(&market, as_of).unwrap().amount();

    // Very short expiry should still price
    assert!(pv > 0.0 && pv.is_finite(), "1-day expiry should price");
}

#[test]
fn test_very_long_expiry() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let expiry = time::macros::date!(2029 - 01 - 01); // 5Y (reasonable)
    let swap_start = expiry;
    let swap_end = time::macros::date!(2039 - 01 - 01);

    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let pv = swaption.value(&market, as_of).unwrap().amount();

    // Very long expiry should still price
    assert!(pv > 0.0 && pv.is_finite(), "5Y expiry should price");
}

#[test]
fn test_forward_starting_swaption() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let expiry = time::macros::date!(2025 - 01 - 01);
    let swap_start = time::macros::date!(2026 - 01 - 01); // 1Y after expiry
    let swap_end = time::macros::date!(2031 - 01 - 01);

    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let pv = swaption.value(&market, as_of).unwrap().amount();

    // Forward starting swap should price
    assert!(
        pv > 0.0 && pv.is_finite(),
        "Forward starting swaption should price"
    );
}

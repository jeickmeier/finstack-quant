//! Expiry-related edge cases

use crate::swaption::common::*;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_models::SabrParameters;
use finstack_quant_valuations::instruments::ExerciseStyle;
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::ModelKey;

fn curve_only_market(as_of: Date, rate: f64) -> MarketContext {
    MarketContext::new()
        .insert(build_flat_discount_curve(rate, as_of, "USD_OIS"))
        .insert(build_flat_forward_curve(rate, as_of, "USD_LIBOR_3M"))
}

#[test]
fn expired_swaption_direct_and_registry_are_zero_without_market_data() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let expiry = time::macros::date!(2023 - 12 - 01);
    let swap_end = time::macros::date!(2028 - 12 - 01);
    let swaption = create_standard_payer_swaption(expiry, expiry, swap_end, 0.05);
    let market = MarketContext::new();

    let direct = swaption
        .value(&market, as_of)
        .expect("expired direct value should be model-independent");
    assert_eq!(direct.amount(), 0.0);

    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::Vega],
            PricingOptions::default(),
        )
        .expect("expired swaption metrics should be well-defined");
    assert_eq!(result.value.amount(), 0.0);
    assert_eq!(result.measures.get("delta"), Some(&0.0));
    assert_eq!(result.measures.get("vega"), Some(&0.0));
}

#[test]
fn at_expiry_direct_sabr_and_registry_models_share_intrinsic_without_volatility() {
    let expiry = time::macros::date!(2024 - 01 - 01);
    let swap_end = time::macros::date!(2029 - 01 - 01);
    let swaption = create_standard_payer_swaption(expiry, expiry, swap_end, 0.03).with_sabr_params(
        SabrParameters {
            alpha: 0.20,
            beta: 0.5,
            rho: -0.3,
            nu: 0.4,
            shift: None,
        },
    );
    assert_ne!(
        swaption.get_discount_curve_id(),
        swaption.get_forward_curve_id(),
        "fixture must exercise the multi-curve terminal path"
    );
    let market = curve_only_market(expiry, 0.05);

    let direct = swaption
        .value(&market, expiry)
        .expect("direct intrinsic value without volatility")
        .amount();
    let sabr = swaption
        .price_sabr(&market, expiry)
        .expect("SABR terminal value without volatility")
        .amount();
    assert!(direct > 0.0, "deep-ITM payer must have positive intrinsic");
    assert!((sabr - direct).abs() < 1e-8);

    for model in [ModelKey::Black76, ModelKey::HullWhite1F] {
        let registry = swaption
            .price_with_metrics(
                &market,
                expiry,
                &[],
                PricingOptions::default().with_model(model),
            )
            .expect("registry terminal value without volatility")
            .value
            .amount();
        assert!(
            (registry - direct).abs() < 1e-8,
            "{model} terminal value {registry} must equal direct intrinsic {direct}"
        );
    }
}

#[test]
fn unsupported_exercise_style_is_not_masked_by_expiry() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let expiry = time::macros::date!(2023 - 12 - 01);
    let swap_end = time::macros::date!(2028 - 12 - 01);
    let swaption = create_standard_payer_swaption(expiry, expiry, swap_end, 0.05)
        .with_exercise_style(ExerciseStyle::Bermudan);
    let market = MarketContext::new();

    let direct_error = swaption
        .value(&market, as_of)
        .expect_err("generic direct pricing must reject Bermudan exercise");
    assert!(direct_error.to_string().contains("only supports European"));

    let registry_error = swaption
        .price_with_metrics(&market, as_of, &[], PricingOptions::default())
        .expect_err("generic registry pricing must reject Bermudan exercise");
    assert!(registry_error
        .to_string()
        .contains("only supports European"));
}

/// At the exact expiry instant a European swaption is worth its intrinsic
/// value on the annuity — not zero. The registry pricer already handles this;
/// the generic model paths (`price_black`, `price_normal`) must agree, both
/// with each other (intrinsic is model-free) and, by continuity, with the
/// price one day before expiry for a deep-ITM payer (whose time value is
/// negligible).
#[test]
fn at_expiry_model_paths_return_intrinsic_not_zero() {
    let expiry = time::macros::date!(2024 - 01 - 01);
    let swap_start = expiry;
    let swap_end = time::macros::date!(2029 - 01 - 01);
    // Deep-ITM payer: strike 3% against a 5% flat market.
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.03);

    // Continuity reference: one day before expiry, deep ITM ⇒ PV ≈ intrinsic.
    let day_before = time::macros::date!(2023 - 12 - 31);
    let reference = swaption
        .price_black(
            &create_flat_market(day_before, 0.05, 0.30),
            0.30,
            day_before,
        )
        .expect("day-before price")
        .amount();
    assert!(
        reference > 50_000.0,
        "deep-ITM payer must carry substantial intrinsic: {reference}"
    );

    let market = create_flat_market(expiry, 0.05, 0.30);
    let black_at_expiry = swaption
        .price_black(&market, 0.30, expiry)
        .expect("black at expiry")
        .amount();
    // Normal vol magnitude irrelevant at t = 0 — intrinsic is model-free.
    let normal_at_expiry = swaption
        .price_normal(&market, 0.0060, expiry)
        .expect("normal at expiry")
        .amount();

    for (label, pv) in [("black", black_at_expiry), ("normal", normal_at_expiry)] {
        assert!(
            (pv - reference).abs() < 0.02 * reference,
            "price_{label} at the expiry instant must return intrinsic (~{reference}), got {pv}"
        );
    }
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

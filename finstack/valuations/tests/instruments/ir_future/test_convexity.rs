//! IR Future convexity adjustment tests.

use super::utils::*;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::surfaces::VolSurface;
use finstack_valuations::instruments::rates::ir_future::{FutureContractSpecs, Position};

#[test]
fn test_strict_convexity_error() {
    let (as_of, start, end) = near_term_dates();
    let market = build_standard_market(as_of, 0.05);

    // Create future with explicit None convexity (overriding default 0.0 from utils)
    let mut future = create_custom_future(
        "STRICT_TEST",
        1_000_000.0,
        start,
        start,
        end,
        97.50,
        Position::Long,
    );
    future.contract_specs.convexity_adjustment = None;
    future.vol_surface_id = None;

    let result = future.value(&market, as_of);

    // Should fail because strict mode requires explicit adjustment or vol surface
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Missing vol_surface_id"));
}

#[test]
fn test_automatic_convexity_far_forward_error() {
    let (as_of, start, end) = far_forward_dates();
    let market = build_standard_market(as_of, 0.05);

    let mut future =
        create_custom_future("FAR", 1_000_000.0, start, start, end, 97.50, Position::Long);
    future.contract_specs.convexity_adjustment = None;

    let result = future.value(&market, as_of);

    // Should fail in strict mode
    assert!(result.is_err());
}

#[test]
fn test_explicit_convexity_adjustment() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    let specs_with_ca = FutureContractSpecs {
        face_value: 1_000_000.0,
        tick_size: 0.0025,
        tick_value: 6.25,
        delivery_months: 3,
        convexity_adjustment: Some(0.0005), // 5 bps adjustment
    };

    let future = create_standard_future(start, end).with_contract_specs(specs_with_ca);

    let pv = future.value(&market, as_of).unwrap();
    assert!(pv.amount().is_finite());
}

#[test]
fn test_convexity_impact() {
    let (as_of, start, end) = far_forward_dates();
    let market = build_standard_market(as_of, 0.05);

    // Case 1: Explicit Zero
    let specs_zero_ca = FutureContractSpecs {
        convexity_adjustment: Some(0.0),
        ..FutureContractSpecs::default()
    };
    let with_zero_ca = create_standard_future(start, end).with_contract_specs(specs_zero_ca);
    let pv_zero_ca = with_zero_ca.value(&market, as_of).unwrap().amount();

    // Case 2: Explicit Positive Adjustment
    let specs_pos_ca = FutureContractSpecs {
        convexity_adjustment: Some(0.0050), // 50bps
        ..FutureContractSpecs::default()
    };
    let with_pos_ca = create_standard_future(start, end).with_contract_specs(specs_pos_ca);
    let pv_pos_ca = with_pos_ca.value(&market, as_of).unwrap().amount();

    // PV should be lower with positive convexity adjustment (Long Future: PV ~ Rate_implied - (Rate_fwd + Conv))
    // If Conv is positive, AdjustedRate is higher, so (Implied - Adjusted) is smaller.
    assert!(pv_pos_ca < pv_zero_ca);
}

#[test]
fn test_convexity_adjustment_sign() {
    let (as_of, start, end) = far_forward_dates();
    let market = build_standard_market(as_of, 0.05);

    // Positive convexity adjustment (typical for futures)
    let positive_ca = FutureContractSpecs {
        convexity_adjustment: Some(0.0010),
        ..FutureContractSpecs::default()
    };

    // Negative convexity adjustment (unusual, but test it)
    let negative_ca = FutureContractSpecs {
        convexity_adjustment: Some(-0.0010),
        ..FutureContractSpecs::default()
    };

    let fut_pos = create_standard_future(start, end).with_contract_specs(positive_ca);
    let fut_neg = create_standard_future(start, end).with_contract_specs(negative_ca);

    let pv_pos = fut_pos.value(&market, as_of).unwrap().amount();
    let pv_neg = fut_neg.value(&market, as_of).unwrap().amount();

    // Different convexity adjustments should produce different valuations
    assert_ne!(
        pv_pos, pv_neg,
        "Convexity adjustments should impact valuation"
    );
}

#[test]
fn test_large_convexity_adjustment() {
    let (as_of, start, end) = standard_dates();
    let market = build_standard_market(as_of, 0.05);

    let large_ca = FutureContractSpecs {
        convexity_adjustment: Some(0.02), // 200 bps - unrealistically large
        ..FutureContractSpecs::default()
    };

    let future = create_standard_future(start, end).with_contract_specs(large_ca);

    let pv = future.value(&market, as_of).unwrap();

    // Should still produce valid result even with extreme convexity adjustment
    // 200bp on $1M ≈ $5k impact; $100k allows for numerical edge cases in this test
    assert!(pv.amount().is_finite());
    assert!(
        pv.amount().abs() < 100_000.0,
        "Large convexity PV should still be bounded, got {}",
        pv.amount()
    );
}

/// Build a flat volatility surface for testing convexity adjustments.
fn build_flat_vol_surface(vol: f64, surface_id: &str) -> VolSurface {
    VolSurface::builder(surface_id)
        .expiries(&[0.25, 1.0, 2.0, 5.0, 10.0])
        .strikes(&[0.01, 0.03, 0.05, 0.07, 0.10])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .build()
        .unwrap()
}

#[test]
fn test_volatility_based_convexity_adjustment() {
    // Use far forward dates where convexity adjustment is more material
    let (as_of, start, end) = far_forward_dates();

    // Build market with volatility surface
    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    // 120bp/yr normal (Bachelier) vol — the CA formula's contract.
    let vol_surface = build_flat_vol_surface(0.012, "USD_SWAPTION_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    // Create future with volatility-based convexity (no explicit adjustment)
    let mut future = create_custom_future(
        "VOL_CONVEX",
        1_000_000.0,
        start,
        start,
        end,
        95.0, // 5% implied rate
        Position::Long,
    );
    future.contract_specs.convexity_adjustment = None; // Use vol-based calculation
    future.vol_surface_id = Some("USD_SWAPTION_VOL".into());

    let pv = future.value(&market, as_of).unwrap();

    // Should produce valid result
    assert!(
        pv.amount().is_finite(),
        "Vol-based convexity should produce finite PV"
    );

    // Compare with zero convexity to verify adjustment is applied
    let mut future_zero_ca = create_custom_future(
        "ZERO_CA",
        1_000_000.0,
        start,
        start,
        end,
        95.0,
        Position::Long,
    );
    future_zero_ca.contract_specs.convexity_adjustment = Some(0.0);

    let pv_zero_ca = future_zero_ca.value(&market, as_of).unwrap().amount();

    // Convexity adjustment should make a difference (positive adjustment reduces PV for long)
    // For 2-year forward with 120bp normal vol: CA ≈ 0.5 × 0.012² × 2 × 2.25 ≈ 3bp
    assert_ne!(
        pv.amount(),
        pv_zero_ca,
        "Vol-based convexity should differ from zero adjustment"
    );
    assert!(
        pv.amount() < pv_zero_ca,
        "Positive convexity adjustment should reduce PV for long position"
    );
}

#[test]
fn test_volatility_based_convexity_increases_with_maturity() {
    let as_of = time::macros::date!(2024 - 01 - 01);

    // Build market with volatility surface
    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    // 100bp/yr normal (Bachelier) vol — the CA formula's contract.
    let vol_surface = build_flat_vol_surface(0.010, "USD_SWAPTION_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    // Near-dated future (6 months forward)
    let near_start = time::macros::date!(2024 - 07 - 01);
    let near_end = time::macros::date!(2024 - 10 - 01);
    let mut near_future = create_custom_future(
        "NEAR",
        1_000_000.0,
        near_start,
        near_start,
        near_end,
        95.0,
        Position::Long,
    );
    near_future.contract_specs.convexity_adjustment = None;
    near_future.vol_surface_id = Some("USD_SWAPTION_VOL".into());

    // Far-dated future (3 years forward)
    let far_start = time::macros::date!(2027 - 01 - 01);
    let far_end = time::macros::date!(2027 - 04 - 01);
    let mut far_future = create_custom_future(
        "FAR",
        1_000_000.0,
        far_start,
        far_start,
        far_end,
        95.0,
        Position::Long,
    );
    far_future.contract_specs.convexity_adjustment = None;
    far_future.vol_surface_id = Some("USD_SWAPTION_VOL".into());

    // Calculate PVs with vol-based convexity
    let pv_near = near_future.value(&market, as_of).unwrap().amount();
    let pv_far = far_future.value(&market, as_of).unwrap().amount();

    // Also calculate with zero convexity for comparison
    let mut near_zero = create_custom_future(
        "NEAR_Z",
        1_000_000.0,
        near_start,
        near_start,
        near_end,
        95.0,
        Position::Long,
    );
    near_zero.contract_specs.convexity_adjustment = Some(0.0);

    let mut far_zero = create_custom_future(
        "FAR_Z",
        1_000_000.0,
        far_start,
        far_start,
        far_end,
        95.0,
        Position::Long,
    );
    far_zero.contract_specs.convexity_adjustment = Some(0.0);

    let pv_near_zero = near_zero.value(&market, as_of).unwrap().amount();
    let pv_far_zero = far_zero.value(&market, as_of).unwrap().amount();

    // Convexity impact: difference between zero-CA and vol-based
    let near_convexity_impact = pv_near_zero - pv_near;
    let far_convexity_impact = pv_far_zero - pv_far;

    // Far-dated future should have larger convexity impact
    // (CA scales with T_fixing² approximately)
    assert!(
        far_convexity_impact > near_convexity_impact,
        "Far-dated future should have larger convexity impact: {} vs {}",
        far_convexity_impact,
        near_convexity_impact
    );
}

#[test]
fn test_volatility_based_convexity_increases_with_vol() {
    let (as_of, start, end) = far_forward_dates();

    // Build base curves
    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");

    // Low volatility market: 50bp/yr normal vol
    let low_vol_surface = build_flat_vol_surface(0.005, "USD_SWAPTION_VOL");
    let low_vol_market = MarketContext::new()
        .insert(disc_curve.clone())
        .insert(fwd_curve.clone())
        .insert_surface(low_vol_surface);

    // High volatility market: 150bp/yr normal vol (3× the low-vol case)
    let high_vol_surface = build_flat_vol_surface(0.015, "USD_SWAPTION_VOL");
    let high_vol_market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(high_vol_surface);

    // Create future with vol-based convexity
    let mut future = create_custom_future(
        "VOL_TEST",
        1_000_000.0,
        start,
        start,
        end,
        95.0,
        Position::Long,
    );
    future.contract_specs.convexity_adjustment = None;
    future.vol_surface_id = Some("USD_SWAPTION_VOL".into());

    let pv_low_vol = future.value(&low_vol_market, as_of).unwrap().amount();
    let pv_high_vol = future.value(&high_vol_market, as_of).unwrap().amount();

    // Higher vol should result in larger convexity adjustment
    // For long position, larger CA means lower PV
    assert!(
        pv_high_vol < pv_low_vol,
        "Higher vol should produce lower PV due to larger convexity adjustment: {} vs {}",
        pv_high_vol,
        pv_low_vol
    );

    // The impact should scale roughly with vol² (CA ∝ σ²)
    // With 3x vol (150bp vs 50bp), expect ~9x convexity impact
    // Get zero-CA baseline
    let mut future_zero =
        create_custom_future("ZERO", 1_000_000.0, start, start, end, 95.0, Position::Long);
    future_zero.contract_specs.convexity_adjustment = Some(0.0);
    let pv_zero = future_zero.value(&low_vol_market, as_of).unwrap().amount();

    let low_vol_impact = pv_zero - pv_low_vol;
    let high_vol_impact = pv_zero - pv_high_vol;

    // High vol impact should be roughly 9x low vol impact (150bp/50bp)² = 9
    let ratio = high_vol_impact / low_vol_impact;
    assert!(
        ratio > 7.0 && ratio < 11.0,
        "Convexity impact ratio should be ~9 (σ² scaling): got {}",
        ratio
    );
}

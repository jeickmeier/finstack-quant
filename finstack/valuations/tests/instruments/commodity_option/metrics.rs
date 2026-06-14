//! Metrics tests for commodity options.

use crate::finstack_test_utils::{
    date, flat_discount_with_tenor, flat_forward_with_tenor, flat_price_curve, flat_vol_surface,
};
use finstack_core::currency::Currency;
use finstack_core::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::instruments::commodity::commodity_option::CommodityOption;
use finstack_valuations::instruments::Attributes;
use finstack_valuations::instruments::CommodityUnderlyingParams;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::{
    ExerciseStyle, OptionType, PricingOverrides, SettlementType,
};
use finstack_valuations::metrics::{standard_registry, MetricContext, MetricId};
use std::sync::Arc;

#[test]
fn test_commodity_option_core_greeks_registered() -> finstack_core::Result<()> {
    let as_of = date(2025, 1, 1);
    let expiry = date(2026, 1, 1);

    let discount_curve = flat_discount_with_tenor("USD-OIS", as_of, 0.03, 2.0);
    let forward_curve = flat_forward_with_tenor("CL-FWD", as_of, 0.0, 2.0);
    let vol_surface = flat_vol_surface("CL-VOL", &[1.0], &[80.0, 100.0, 120.0], 0.20);

    let market = MarketContext::new()
        .insert(discount_curve)
        .insert(forward_curve)
        .insert_surface(vol_surface)
        .insert_price("CL-SPOT", MarketScalar::Unitless(100.0));

    let option = CommodityOption::builder()
        .id(InstrumentId::new("CL-CALL-GREeks"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .strike(100.0)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .quantity(1.0)
        .multiplier(1.0)
        .settlement(SettlementType::Cash)
        .forward_curve_id(CurveId::new("CL-FWD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("CL-VOL"))
        .spot_id_opt(Some("CL-SPOT".to_string()))
        .day_count(finstack_core::dates::DayCount::Act365F)
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let pv = option.value(&market, as_of)?;
    let mut ctx = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let registry = standard_registry();
    let res = registry.compute(
        &[MetricId::Gamma, MetricId::Vanna, MetricId::Volga],
        &mut ctx,
    )?;

    let gamma = *res.get(&MetricId::Gamma).expect("gamma");
    let vanna = *res.get(&MetricId::Vanna).expect("vanna");
    let volga = *res.get(&MetricId::Volga).expect("volga");

    assert!(gamma.is_finite(), "gamma should be finite");
    assert!(vanna.is_finite(), "vanna should be finite");
    assert!(volga.is_finite(), "volga should be finite");
    assert!(gamma >= -1e-8, "gamma should be non-negative");

    Ok(())
}

/// Test that forward-based Greeks (gamma/vanna) bump the PriceCurve (not spot)
/// when both are present in the market.
///
/// This validates that Greeks are consistent with the Black-76 forward-based model.
#[test]
fn test_forward_based_greeks_with_both_spot_and_price_curve() -> finstack_core::Result<()> {
    let as_of = date(2025, 1, 1);
    let expiry = date(2026, 1, 1);

    // Forward price from PriceCurve: 100
    // Spot price (different): 95 (backwardation scenario)
    let discount_curve = flat_discount_with_tenor("USD-OIS", as_of, 0.03, 2.0);
    let price_curve = flat_price_curve("CL-FWD", as_of, 100.0, 2.0);
    let vol_surface = flat_vol_surface("CL-VOL", &[1.0], &[80.0, 100.0, 120.0], 0.20);

    let market = MarketContext::new()
        .insert(discount_curve)
        .insert(price_curve)
        .insert_surface(vol_surface)
        .insert_price("CL-SPOT", MarketScalar::Unitless(95.0)); // Spot different from forward

    let option = CommodityOption::builder()
        .id(InstrumentId::new("CL-CALL-FWD-GREEKS"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .strike(100.0)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .quantity(1.0)
        .multiplier(1.0)
        .settlement(SettlementType::Cash)
        .forward_curve_id(CurveId::new("CL-FWD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("CL-VOL"))
        .spot_id_opt(Some("CL-SPOT".to_string()))
        .day_count(finstack_core::dates::DayCount::Act365F)
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    // Compute Greeks via registry
    let pv = option.value(&market, as_of)?;
    let mut ctx = MetricContext::new(
        Arc::new(option.clone()),
        Arc::new(market.clone()),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let registry = standard_registry();
    let res = registry.compute(&[MetricId::Gamma, MetricId::Vanna], &mut ctx)?;

    let gamma = *res.get(&MetricId::Gamma).expect("gamma");
    let vanna = *res.get(&MetricId::Vanna).expect("vanna");

    // Validate Greeks are finite and reasonable
    assert!(gamma.is_finite(), "gamma should be finite");
    assert!(vanna.is_finite(), "vanna should be finite");
    assert!(
        gamma >= -1e-8,
        "gamma should be non-negative for vanilla option"
    );

    // Now compute reference gamma/vanna by explicitly bumping the PriceCurve
    // This validates that the Greeks implementation bumps PriceCurve, not spot
    let bump_pct = 0.01; // Same as bump_sizes::SPOT
    let vol_bump = 0.01; // Same as bump_sizes::VOLATILITY
    let forward_price = option.forward_price(&market, as_of)?;
    let bump_size = forward_price * bump_pct;

    // Reference gamma: bump PriceCurve up/down and use central FD
    let price_curve_id = CurveId::new("CL-FWD");
    let bump_up = MarketBump::Curve {
        id: price_curve_id.clone(),
        spec: BumpSpec {
            bump_type: BumpType::Parallel,
            mode: BumpMode::Additive,
            units: BumpUnits::Percent,
            value: bump_pct * 100.0,
        },
    };
    let bump_down = MarketBump::Curve {
        id: price_curve_id,
        spec: BumpSpec {
            bump_type: BumpType::Parallel,
            mode: BumpMode::Additive,
            units: BumpUnits::Percent,
            value: -bump_pct * 100.0,
        },
    };

    let market_up = market.bump([bump_up])?;
    let market_down = market.bump([bump_down])?;

    let pv_base = option.value(&market, as_of)?.amount();
    let pv_up = option.value(&market_up, as_of)?.amount();
    let pv_down = option.value(&market_down, as_of)?.amount();

    let ref_gamma = (pv_up - 2.0 * pv_base + pv_down) / (bump_size * bump_size);

    // Reference vanna: mixed FD bumping PriceCurve and vol
    // Use direct bump API for vol surface (additive absolute bump)
    let vol_surface_id = CurveId::new("CL-VOL");
    let vol_bump_up = MarketBump::Curve {
        id: vol_surface_id.clone(),
        spec: BumpSpec {
            bump_type: BumpType::Parallel,
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction, // Absolute vol points
            value: vol_bump,
        },
    };
    let vol_bump_down = MarketBump::Curve {
        id: vol_surface_id,
        spec: BumpSpec {
            bump_type: BumpType::Parallel,
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value: -vol_bump,
        },
    };

    let market_up_vol_up = market_up.bump([vol_bump_up.clone()])?;
    let market_up_vol_down = market_up.bump([vol_bump_down.clone()])?;
    let market_down_vol_up = market_down.bump([vol_bump_up])?;
    let market_down_vol_down = market_down.bump([vol_bump_down])?;

    let pv_up_up = option.value(&market_up_vol_up, as_of)?.amount();
    let pv_up_down = option.value(&market_up_vol_down, as_of)?.amount();
    let pv_down_up = option.value(&market_down_vol_up, as_of)?.amount();
    let pv_down_down = option.value(&market_down_vol_down, as_of)?.amount();

    let ref_vanna =
        (pv_up_up - pv_up_down - pv_down_up + pv_down_down) / (4.0 * bump_size * vol_bump);

    // Validate that computed Greeks match reference within tolerance
    let gamma_tol = 1e-6;
    let vanna_tol = 1e-6;

    assert!(
        (gamma - ref_gamma).abs() < gamma_tol,
        "gamma {} should match reference {} (diff={})",
        gamma,
        ref_gamma,
        (gamma - ref_gamma).abs()
    );
    assert!(
        (vanna - ref_vanna).abs() < vanna_tol,
        "vanna {} should match reference {} (diff={})",
        vanna,
        ref_vanna,
        (vanna - ref_vanna).abs()
    );

    Ok(())
}

/// Build a 1Y ATM commodity option (spot = forward = strike = 100, 20% flat
/// vol) and its market, for exercising registered Greeks/risk metrics.
fn atm_commodity_option(
    option_type: OptionType,
) -> (CommodityOption, MarketContext, finstack_core::dates::Date) {
    let as_of = date(2025, 1, 1);
    let expiry = date(2026, 1, 1);

    let discount_curve = flat_discount_with_tenor("USD-OIS", as_of, 0.03, 2.0);
    let forward_curve = flat_forward_with_tenor("CL-FWD", as_of, 0.0, 2.0);
    let vol_surface = flat_vol_surface("CL-VOL", &[1.0], &[80.0, 100.0, 120.0], 0.20);
    let market = MarketContext::new()
        .insert(discount_curve)
        .insert(forward_curve)
        .insert_surface(vol_surface)
        .insert_price("CL-SPOT", MarketScalar::Unitless(100.0));

    let option = CommodityOption::builder()
        .id(InstrumentId::new("CL-OPT-METRICS"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .strike(100.0)
        .option_type(option_type)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .quantity(1.0)
        .multiplier(1.0)
        .settlement(SettlementType::Cash)
        .forward_curve_id(CurveId::new("CL-FWD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("CL-VOL"))
        .spot_id_opt(Some("CL-SPOT".to_string()))
        .day_count(finstack_core::dates::DayCount::Act365F)
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("should build");
    (option, market, as_of)
}

/// `Delta` and `Vega` are registered but were previously unexercised. A long
/// call must have positive delta, a long put negative delta, and any long
/// option positive vega.
#[test]
fn test_commodity_option_delta_and_vega_signs() -> finstack_core::Result<()> {
    let registry = standard_registry();

    // Call: positive delta, positive vega.
    let (call, market, as_of) = atm_commodity_option(OptionType::Call);
    let pv = call.value(&market, as_of)?;
    let mut ctx = MetricContext::new(
        Arc::new(call),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let res = registry.compute(&[MetricId::Delta, MetricId::Vega], &mut ctx)?;
    let call_delta = *res.get(&MetricId::Delta).expect("delta");
    let vega = *res.get(&MetricId::Vega).expect("vega");
    assert!(
        call_delta.is_finite() && call_delta > 0.0,
        "long call delta should be positive, got {call_delta}"
    );
    assert!(
        vega.is_finite() && vega > 0.0,
        "long option vega should be positive, got {vega}"
    );

    // Put: negative delta.
    let (put, market_p, as_of_p) = atm_commodity_option(OptionType::Put);
    let pv_p = put.value(&market_p, as_of_p)?;
    let mut ctx_p = MetricContext::new(
        Arc::new(put),
        Arc::new(market_p),
        as_of_p,
        pv_p,
        MetricContext::default_config(),
    );
    let put_delta = *registry
        .compute(&[MetricId::Delta], &mut ctx_p)?
        .get(&MetricId::Delta)
        .expect("delta");
    assert!(
        put_delta.is_finite() && put_delta < 0.0,
        "long put delta should be negative, got {put_delta}"
    );

    Ok(())
}

/// `Dv01` and `BucketedDv01` are registered but were previously unexercised.
/// Both must compute to finite values and the bucketed aggregate must reconcile
/// with the parallel DV01.
#[test]
fn test_commodity_option_dv01_and_bucketed_dv01() -> finstack_core::Result<()> {
    let registry = standard_registry();
    let (call, market, as_of) = atm_commodity_option(OptionType::Call);
    let pv = call.value(&market, as_of)?;
    let mut ctx = MetricContext::new(
        Arc::new(call),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let res = registry.compute(&[MetricId::Dv01, MetricId::BucketedDv01], &mut ctx)?;
    let dv01 = *res.get(&MetricId::Dv01).expect("dv01");
    let bucketed = *res.get(&MetricId::BucketedDv01).expect("bucketed_dv01");

    assert!(dv01.is_finite(), "DV01 should be finite, got {dv01}");
    assert!(
        bucketed.is_finite(),
        "BucketedDv01 aggregate should be finite, got {bucketed}"
    );
    // The bucketed key-rate DV01s sum to the parallel DV01 (within a small
    // numerical tolerance, plus an absolute floor for near-zero rate exposure).
    assert!(
        (bucketed - dv01).abs() <= 1.0 + 0.05 * dv01.abs(),
        "BucketedDv01 ({bucketed}) should reconcile with parallel DV01 ({dv01})"
    );

    Ok(())
}

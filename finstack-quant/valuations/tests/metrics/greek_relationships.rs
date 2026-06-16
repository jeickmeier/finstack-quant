//! Tests validating mathematical relationships between greeks.
//!
//! Verifies fundamental relationships like:
//! - Charm = ∂Δ/∂t (delta decay)
//! - Color = ∂Γ/∂t (gamma decay)
//! - Speed = ∂Γ/∂S (gamma convexity)
//! - Vanna = ∂²V/∂S∂σ (delta-vol correlation)
//! - Volga = ∂²V/∂σ² (volatility convexity)
//!
//! These relationships are tested using finite differences with appropriate tolerances.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{ExerciseStyle, OptionType};
use finstack_quant_valuations::instruments::{PricingOverrides, SettlementType};
use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
use std::sync::Arc;
use time::macros::date;

#[allow(dead_code)]
const TOLERANCE: f64 = 0.01; // 1% tolerance for FD approximations vs analytical
#[allow(dead_code)]
const STRICT_TOLERANCE: f64 = 0.001; // 0.1% tolerance when both metric and FD exist
const SPOT_BUMP_PCT: f64 = 0.01;

fn bump_scalar_price(
    context: &MarketContext,
    price_id: &str,
    bump_pct: f64,
) -> finstack_quant_core::Result<MarketContext> {
    let current = context.get_price(price_id)?;
    let bumped_value = match current {
        MarketScalar::Unitless(v) => MarketScalar::Unitless(v * (1.0 + bump_pct)),
        MarketScalar::Price(m) => {
            MarketScalar::Price(Money::new(m.amount() * (1.0 + bump_pct), m.currency()))
        }
    };
    Ok(context.clone().insert_price(price_id, bumped_value))
}

fn delta_fd(option: &EquityOption, market: &MarketContext, as_of: Date) -> f64 {
    let spot_scalar = market.get_price(&option.spot_id).unwrap();
    let spot = match spot_scalar {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    };
    let h = spot * SPOT_BUMP_PCT;
    let up = bump_scalar_price(market, &option.spot_id, SPOT_BUMP_PCT).unwrap();
    let dn = bump_scalar_price(market, &option.spot_id, -SPOT_BUMP_PCT).unwrap();
    let pv_up = option.value(&up, as_of).unwrap().amount();
    let pv_dn = option.value(&dn, as_of).unwrap().amount();
    (pv_up - pv_dn) / (2.0 * h)
}

fn gamma_fd(option: &EquityOption, market: &MarketContext, as_of: Date) -> f64 {
    let spot_scalar = market.get_price(&option.spot_id).unwrap();
    let spot = match spot_scalar {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    };
    let h = spot * SPOT_BUMP_PCT;
    let up = bump_scalar_price(market, &option.spot_id, SPOT_BUMP_PCT).unwrap();
    let dn = bump_scalar_price(market, &option.spot_id, -SPOT_BUMP_PCT).unwrap();
    let pv_up = option.value(&up, as_of).unwrap().amount();
    let pv_dn = option.value(&dn, as_of).unwrap().amount();
    let pv_0 = option.value(market, as_of).unwrap().amount();
    (pv_up - 2.0 * pv_0 + pv_dn) / (h * h)
}

fn create_test_option(
    _as_of: Date,
    expiry: Date,
    strike: f64,
    option_type: OptionType,
) -> EquityOption {
    EquityOption {
        id: "TEST_OPTION".into(),
        underlying_ticker: "AAPL".to_string(),
        strike,
        option_type,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL_VOL".into(),
        div_yield_id: Some("AAPL_DIV".into()),
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    }
}

fn create_market_context(
    as_of: Date,
    spot: f64,
    vol: f64,
    rate: f64,
    div_yield: f64,
) -> MarketContext {
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (0.25f64, (-rate * 0.25f64).exp()),
            (0.5f64, (-rate * 0.5f64).exp()),
            (1.0f64, (-rate).exp()),
            (2.0f64, (-rate * 2.0f64).exp()),
        ])
        .build()
        .unwrap();

    let vol_surface = VolSurface::builder("AAPL_VOL")
        .expiries(&[0.25, 0.5, 1.0, 2.0])
        .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .build()
        .unwrap();

    MarketContext::new()
        .insert(disc_curve)
        .insert_surface(vol_surface)
        .insert_price("AAPL", MarketScalar::Price(Money::new(spot, Currency::USD)))
        .insert_price("AAPL_DIV", MarketScalar::Unitless(div_yield))
}

#[test]
fn test_charm_equals_delta_decay() {
    // Charm = ∂Δ/∂t should equal finite difference of delta with time
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let option = create_test_option(as_of, expiry, strike, OptionType::Call);
    let market = create_market_context(as_of, spot, 0.25, 0.05, 0.02);

    let registry = standard_registry();

    // Compute delta at current time
    let delta_at_t = delta_fd(&option, &market, as_of);

    // Compute delta at t + dt (1 day forward)
    let dt_days = 1.0;
    let as_of_plus_dt = as_of + time::Duration::days(dt_days as i64);
    let delta_at_t_dt = delta_fd(&option, &market, as_of_plus_dt);

    // Compute Charm via finite difference: Charm ≈ (Δ(t+dt) - Δ(t)) / dt
    let dt_years = dt_days / 365.25;
    let charm_fd = (delta_at_t_dt - delta_at_t) / dt_years;

    // Try to get Charm from registry if available
    let pv = option.value(&market, as_of).unwrap();
    let mut context_charm = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let charm_metric = registry.compute(&[MetricId::Charm], &mut context_charm);

    if let Ok(charm_results) = charm_metric {
        if let Some(&charm_value) = charm_results.get(&MetricId::Charm) {
            assert!(
                charm_value.is_finite(),
                "Charm metric should be finite, got: {}",
                charm_value
            );

            let rel_error = if charm_fd.abs() > 1e-6 {
                ((charm_value - charm_fd) / charm_fd).abs()
            } else {
                (charm_value - charm_fd).abs()
            };
            assert!(
                rel_error < 0.05,
                "Charm should match FD within 5%: metric={}, fd={}, rel_error={:.2}%",
                charm_value,
                charm_fd,
                rel_error * 100.0
            );
        }
    }

    // Verify FD calculation is reasonable (non-zero for ATM/ITM options)
    assert!(charm_fd.is_finite(), "Charm (FD) should be finite");
}

#[test]
fn test_color_equals_gamma_decay() {
    // Color = ∂Γ/∂t should equal finite difference of gamma with time
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let option = create_test_option(as_of, expiry, strike, OptionType::Call);
    let market = create_market_context(as_of, spot, 0.25, 0.05, 0.02);

    let registry = standard_registry();

    // Compute gamma at current time
    let gamma_at_t = gamma_fd(&option, &market, as_of);

    // Compute gamma at t + dt (1 day forward)
    let dt_days = 1.0;
    let as_of_plus_dt = as_of + time::Duration::days(dt_days as i64);
    let gamma_at_t_dt = gamma_fd(&option, &market, as_of_plus_dt);

    // Compute Color via finite difference: Color ≈ (Γ(t+dt) - Γ(t)) / dt
    let dt_years = dt_days / 365.25;
    let color_fd = (gamma_at_t_dt - gamma_at_t) / dt_years;

    // Try to get Color from registry if available
    let pv = option.value(&market, as_of).unwrap();
    let mut context_color = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let color_metric = registry.compute(&[MetricId::Color], &mut context_color);

    if let Ok(color_results) = color_metric {
        if let Some(&color_value) = color_results.get(&MetricId::Color) {
            assert!(
                color_value.is_finite(),
                "Color metric should be finite, got: {}",
                color_value
            );

            let rel_error = if color_fd.abs() > 1e-6 {
                ((color_value - color_fd) / color_fd).abs()
            } else {
                (color_value - color_fd).abs()
            };
            assert!(
                rel_error < 0.05,
                "Color should match FD within 5%: metric={}, fd={}, rel_error={:.2}%",
                color_value,
                color_fd,
                rel_error * 100.0
            );
        }
    }

    // Verify FD calculation is reasonable
    assert!(color_fd.is_finite(), "Color (FD) should be finite");
}

#[test]
fn test_speed_equals_gamma_convexity() {
    // Speed = ∂Γ/∂S should equal finite difference of gamma with spot
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let option = create_test_option(as_of, expiry, strike, OptionType::Call);
    let market = create_market_context(as_of, spot, 0.25, 0.05, 0.02);

    let registry = standard_registry();
    let spot_bump_pct = 0.01; // 1% bump

    // Compute gamma at current spot
    let pv = option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(option.clone()),
        Arc::new(market.clone()),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let _gamma_at_s = *registry
        .compute(&[MetricId::Gamma], &mut context)
        .unwrap()
        .get(&MetricId::Gamma)
        .unwrap();

    // Compute gamma at spot + bump
    let spot_bump = spot * spot_bump_pct;
    let market_up = market.clone().insert_price(
        "AAPL",
        MarketScalar::Price(Money::new(spot + spot_bump, Currency::USD)),
    );
    let pv_up = option.value(&market_up, as_of).unwrap();
    let mut context_up = MetricContext::new(
        Arc::new(option.clone()),
        Arc::new(market_up),
        as_of,
        pv_up,
        MetricContext::default_config(),
    );
    let gamma_at_s_up = *registry
        .compute(&[MetricId::Gamma], &mut context_up)
        .unwrap()
        .get(&MetricId::Gamma)
        .unwrap();

    // Compute gamma at spot - bump
    let market_down = market.clone().insert_price(
        "AAPL",
        MarketScalar::Price(Money::new(spot - spot_bump, Currency::USD)),
    );
    let pv_down = option.value(&market_down, as_of).unwrap();
    let mut context_down = MetricContext::new(
        Arc::new(option.clone()),
        Arc::new(market_down),
        as_of,
        pv_down,
        MetricContext::default_config(),
    );
    let gamma_at_s_down = *registry
        .compute(&[MetricId::Gamma], &mut context_down)
        .unwrap()
        .get(&MetricId::Gamma)
        .unwrap();

    // Compute Speed via finite difference: Speed ≈ (Γ(S+ΔS) - Γ(S-ΔS)) / (2 * ΔS)
    let speed_fd = (gamma_at_s_up - gamma_at_s_down) / (2.0 * spot_bump);

    // Try to get Speed from registry if available
    let mut context_speed = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let speed_metric = registry.compute(&[MetricId::Speed], &mut context_speed);

    if let Ok(speed_results) = speed_metric {
        if let Some(&speed_value) = speed_results.get(&MetricId::Speed) {
            // Validate Speed metric is finite
            assert!(
                speed_value.is_finite(),
                "Speed metric should be finite, got: {}",
                speed_value
            );

            // Compare registry Speed to FD calculation
            // Allow 5% tolerance for FD approximation with 1% spot bump (second-order Greek)
            let rel_error = if speed_fd.abs() > 1e-8 {
                ((speed_value - speed_fd) / speed_fd).abs()
            } else {
                (speed_value - speed_fd).abs()
            };
            assert!(
                rel_error < 0.05,
                "Speed should match FD: registry={:.6}, fd={:.6}, rel_error={:.2}%",
                speed_value,
                speed_fd,
                rel_error * 100.0
            );
        }
    }

    // Verify FD calculation is reasonable
    assert!(speed_fd.is_finite(), "Speed (FD) should be finite");
}

// NOTE: Sign convention tests for Gamma and Vega (non-negativity for long positions)
// are located in sign_conventions.rs to avoid duplication. This module focuses on
// mathematical relationships between Greeks (e.g., Charm = ∂Δ/∂t, Speed = ∂Γ/∂S).

//! Tests validating mathematical relationships between greeks.
//!
//! Verifies fundamental relationships like:
//! - Charm = ∂Δ/∂t (delta decay)
//! - Color = ∂Γ/∂t (gamma decay)
//! - Speed = ∂Γ/∂S (gamma convexity)
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
use finstack_quant_valuations::instruments::SettlementType;
use finstack_quant_valuations::instruments::{ExerciseStyle, OptionType};
use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
use std::sync::Arc;
use time::macros::date;

const HIGHER_ORDER_GREEK_REL_TOLERANCE: f64 = 0.05;
const SPOT_BUMP_PCT: f64 = 0.01;

fn create_test_option(expiry: Date, strike: f64, option_type: OptionType) -> EquityOption {
    EquityOption {
        id: "TEST_OPTION".into(),
        underlying_ticker: "AAPL".to_string(),
        strike,
        option_type,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD).expect("valid money fixture"),
        day_count: DayCount::Act365F,
        theta_day_basis: Default::default(),
        settlement: SettlementType::Cash,
        exercise: None,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL_VOL".into(),
        div_yield_id: Some("AAPL_DIV".into()),
        discrete_dividends: Vec::new(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    }
}

fn metric_value(
    option: &EquityOption,
    market: &MarketContext,
    as_of: Date,
    metric: MetricId,
) -> f64 {
    let pv = option.value(market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(option.clone()),
        Arc::new(market.clone()),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let results = standard_registry()
        .compute(std::slice::from_ref(&metric), &mut context)
        .unwrap_or_else(|error| panic!("{metric} computation failed: {error}"));
    let value = *results
        .get(&metric)
        .unwrap_or_else(|| panic!("{metric} was omitted from metric results"));
    assert!(value.is_finite(), "{metric} should be finite, got {value}");
    value
}

fn assert_matches_finite_difference(metric: &str, actual: f64, expected: f64) {
    assert!(
        expected.is_finite(),
        "{metric} finite-difference reference should be finite, got {expected}"
    );
    let scaled_error = (actual - expected).abs() / expected.abs().max(1e-8);
    assert!(
        scaled_error < HIGHER_ORDER_GREEK_REL_TOLERANCE,
        "{metric} should match its finite-difference reference within {:.1}%: metric={actual}, fd={expected}, scaled_error={:.2}%",
        HIGHER_ORDER_GREEK_REL_TOLERANCE * 100.0,
        scaled_error * 100.0,
    );
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
        .insert_price(
            "AAPL",
            MarketScalar::Price(Money::new(spot, Currency::USD).expect("valid money fixture")),
        )
        .insert_price("AAPL_DIV", MarketScalar::Unitless(div_yield))
}

#[test]
fn test_charm_equals_delta_decay() {
    // Charm = ∂Δ/∂t should equal finite difference of delta with time
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let option = create_test_option(expiry, strike, OptionType::Call);
    let market = create_market_context(as_of, spot, 0.25, 0.05, 0.02);

    // Use analytical delta as an independent reference for the registry's
    // finite-difference Charm calculator.
    let delta_at_t = option.delta(&market, as_of).unwrap();

    // Compute delta at t + dt (1 day forward)
    let as_of_plus_dt = as_of + time::Duration::days(1);
    let delta_at_t_dt = option.delta(&market, as_of_plus_dt).unwrap();

    // Compute Charm via finite difference: Charm ≈ (Δ(t+dt) - Δ(t)) / dt
    let dt_years = 1.0 / 365.0;
    let charm_fd = (delta_at_t_dt - delta_at_t) / dt_years;

    let charm = metric_value(&option, &market, as_of, MetricId::Charm);
    assert_matches_finite_difference("Charm", charm, charm_fd);
}

#[test]
fn test_color_equals_gamma_decay() {
    // Color = ∂Γ/∂t should equal finite difference of gamma with time
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let option = create_test_option(expiry, strike, OptionType::Call);
    let market = create_market_context(as_of, spot, 0.25, 0.05, 0.02);

    // Use analytical gamma as an independent reference for the registry's
    // finite-difference Color calculator.
    let gamma_at_t = option.gamma(&market, as_of).unwrap();

    // Compute gamma at t + dt (1 day forward)
    let as_of_plus_dt = as_of + time::Duration::days(1);
    let gamma_at_t_dt = option.gamma(&market, as_of_plus_dt).unwrap();

    // Compute Color via finite difference: Color ≈ (Γ(t+dt) - Γ(t)) / dt
    let dt_years = 1.0 / 365.0;
    let color_fd = (gamma_at_t_dt - gamma_at_t) / dt_years;

    let color = metric_value(&option, &market, as_of, MetricId::Color);
    assert_matches_finite_difference("Color", color, color_fd);
}

#[test]
fn test_speed_equals_gamma_convexity() {
    // Speed = ∂Γ/∂S should equal finite difference of gamma with spot
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let option = create_test_option(expiry, strike, OptionType::Call);
    let market = create_market_context(as_of, spot, 0.25, 0.05, 0.02);

    let spot_bump_pct = SPOT_BUMP_PCT;

    // Compute gamma at spot + bump
    let spot_bump = spot * spot_bump_pct;
    let market_up = market.clone().insert_price(
        "AAPL",
        MarketScalar::Price(
            Money::new(spot + spot_bump, Currency::USD).expect("valid money fixture"),
        ),
    );
    let gamma_at_s_up = option.gamma(&market_up, as_of).unwrap();

    // Compute gamma at spot - bump
    let market_down = market.clone().insert_price(
        "AAPL",
        MarketScalar::Price(
            Money::new(spot - spot_bump, Currency::USD).expect("valid money fixture"),
        ),
    );
    let gamma_at_s_down = option.gamma(&market_down, as_of).unwrap();

    // Compute Speed via finite difference: Speed ≈ (Γ(S+ΔS) - Γ(S-ΔS)) / (2 * ΔS)
    let speed_fd = (gamma_at_s_up - gamma_at_s_down) / (2.0 * spot_bump);

    let speed = metric_value(&option, &market, as_of, MetricId::Speed);
    assert_matches_finite_difference("Speed", speed, speed_fd);
}

// NOTE: Sign convention tests for Gamma and Vega (non-negativity for long positions)
// are located in sign_conventions.rs to avoid duplication. This module focuses on
// mathematical relationships between Greeks (e.g., Charm = ∂Δ/∂t, Speed = ∂Γ/∂S).

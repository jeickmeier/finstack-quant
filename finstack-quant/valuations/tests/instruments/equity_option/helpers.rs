//! Shared test helpers and utilities for equity option tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::{ExerciseStyle, OptionType};
use finstack_quant_valuations::instruments::{PricingOverrides, SettlementType};

/// Standard curve IDs
pub const DISC_ID: &str = "USD_DISC";
pub const SPOT_ID: &str = "AAPL";
pub const VOL_ID: &str = "AAPL_VOL";
pub const DIV_ID: &str = "AAPL_DIV";

/// Approximation tolerance for floating point comparisons
pub const DEFAULT_TOL: f64 = 1e-6;
pub const TIGHT_TOL: f64 = 1e-10;
pub const LOOSE_TOL: f64 = 1e-3;

/// Build a flat discount curve with constant zero rate
pub fn build_flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    let mut builder = DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.25, (-rate * 0.25).exp()),
            (0.5, (-rate * 0.5).exp()),
            (1.0, (-rate).exp()),
            (2.0, (-rate * 2.0).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ]);

    // For zero or negative rates, the curve may be flat or increasing
    // which requires allow_non_monotonic() and a simpler interpolation style
    // (MonotoneConvex enforces non-increasing DFs).
    if rate.abs() < 1e-10 || rate < 0.0 {
        builder = builder
            .validation(
                finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                    allow_non_monotonic: true,
                    forward_floor: None,
                },
            )
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear);
    }

    builder.build().unwrap()
}

/// Build a flat volatility surface (same vol for all strikes/expiries)
pub fn build_flat_vol_surface(vol: f64, _base_date: Date, surface_id: &str) -> VolSurface {
    VolSurface::builder(surface_id)
        .expiries(&[0.25, 0.5, 1.0, 2.0, 5.0])
        .strikes(&[50.0, 80.0, 100.0, 120.0, 150.0])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .build()
        .unwrap()
}

/// Build a smile vol surface (higher vol at wings)
pub fn build_smile_vol_surface(_base_date: Date, surface_id: &str) -> VolSurface {
    let expiries = [0.25, 0.5, 1.0, 2.0];
    let strikes = [70.0, 90.0, 100.0, 110.0, 130.0];

    VolSurface::builder(surface_id)
        .expiries(&expiries)
        .strikes(&strikes)
        .row(&[0.35, 0.28, 0.25, 0.28, 0.35]) // 3M
        .row(&[0.34, 0.27, 0.25, 0.27, 0.34]) // 6M
        .row(&[0.33, 0.26, 0.24, 0.26, 0.33]) // 1Y
        .row(&[0.32, 0.25, 0.23, 0.25, 0.32]) // 2Y
        .build()
        .unwrap()
}

/// Create a standard European call option
pub fn create_call(_as_of: Date, expiry: Date, strike: f64) -> EquityOption {
    EquityOption {
        id: "EQ_CALL_TEST".into(),
        underlying_ticker: SPOT_ID.into(),
        strike,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: DISC_ID.into(),
        spot_id: SPOT_ID.into(),
        vol_surface_id: VOL_ID.into(),
        div_yield_id: Some(DIV_ID.into()),
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    }
}

/// Create a standard European put option
pub fn create_put(_as_of: Date, expiry: Date, strike: f64) -> EquityOption {
    EquityOption {
        id: "EQ_PUT_TEST".into(),
        underlying_ticker: SPOT_ID.into(),
        strike,
        option_type: OptionType::Put,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: DISC_ID.into(),
        spot_id: SPOT_ID.into(),
        vol_surface_id: VOL_ID.into(),
        div_yield_id: Some(DIV_ID.into()),
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    }
}

/// Create a standard market context with flat curves
pub fn build_standard_market(
    as_of: Date,
    spot: f64,
    vol: f64,
    rate: f64,
    div_yield: f64,
) -> MarketContext {
    let disc_curve = build_flat_discount_curve(rate, as_of, DISC_ID);
    let vol_surface = build_flat_vol_surface(vol, as_of, VOL_ID);

    MarketContext::new()
        .insert(disc_curve)
        .insert_surface(vol_surface)
        .insert_price(
            SPOT_ID,
            MarketScalar::Price(Money::new(spot, Currency::USD)),
        )
        .insert_price(DIV_ID, MarketScalar::Unitless(div_yield))
}

/// Create a market context with vol smile
pub fn build_smile_market(as_of: Date, spot: f64, rate: f64, div_yield: f64) -> MarketContext {
    let disc_curve = build_flat_discount_curve(rate, as_of, DISC_ID);
    let vol_surface = build_smile_vol_surface(as_of, VOL_ID);

    MarketContext::new()
        .insert(disc_curve)
        .insert_surface(vol_surface)
        .insert_price(
            SPOT_ID,
            MarketScalar::Price(Money::new(spot, Currency::USD)),
        )
        .insert_price(DIV_ID, MarketScalar::Unitless(div_yield))
}

/// Assert approximate equality with default tolerance
pub fn assert_approx_eq(actual: f64, expected: f64, label: &str) {
    assert_approx_eq_tol(actual, expected, DEFAULT_TOL, label);
}

/// Assert approximate equality with custom tolerance
pub fn assert_approx_eq_tol(actual: f64, expected: f64, tol: f64, label: &str) {
    let diff = (actual - expected).abs();
    assert!(
        diff <= tol,
        "{}: expected {}, got {} (diff {} > tol {})",
        label,
        expected,
        actual,
        diff,
        tol
    );
}

/// Assert value is within bounds (inclusive)
pub fn assert_in_range(value: f64, min: f64, max: f64, label: &str) {
    assert!(
        value >= min && value <= max,
        "{}: {} not in range [{}, {}]",
        label,
        value,
        min,
        max
    );
}

/// Assert value is positive
pub fn assert_positive(value: f64, label: &str) {
    assert!(value > 0.0, "{}: expected positive, got {}", label, value);
}

/// Assert value is non-negative
pub fn assert_non_negative(value: f64, label: &str) {
    assert!(
        value >= 0.0,
        "{}: expected non-negative, got {}",
        label,
        value
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    #[test]
    fn test_assert_approx_eq_helper() {
        assert_approx_eq(1.0 + DEFAULT_TOL * 0.5, 1.0, "Within default tolerance");
    }

    #[test]
    fn test_smile_surface_builder() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let surface = build_smile_vol_surface(as_of, "EQ_SMILE");
        assert_eq!(surface.id().as_str(), "EQ_SMILE");
    }

    #[test]
    fn test_smile_market_builder() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let market = build_smile_market(as_of, 150.0, 0.02, 0.01);
        assert!(market.get_discount(DISC_ID).is_ok());
    }
}

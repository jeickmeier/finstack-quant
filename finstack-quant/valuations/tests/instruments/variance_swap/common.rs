//! Common test fixtures and helpers for variance swap tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    MarketScalar, ScalarTimeSeries, SeriesInterpolation,
};
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::equity::variance_swap::{
    PayReceive, RealizedVarMethod, VarianceSwap,
};
use finstack_quant_valuations::instruments::Attributes;

pub const DISC_ID: &str = "USD_OIS";
pub const UNDERLYING_ID: &str = "SPX";
pub const DEFAULT_NOTIONAL: f64 = 1_000_000.0;
pub const DEFAULT_STRIKE_VAR: f64 = 0.04; // 20% vol => 0.04 variance

// Re-export the canonical date helper from shared test utilities.
pub use crate::finstack_quant_test_utils::date;

/// Default start and end dates for test swaps.
pub fn default_dates() -> (Date, Date) {
    (date(2025, 1, 2), date(2025, 4, 1))
}

/// Build a sample variance swap with standard parameters.
pub fn sample_swap(side: PayReceive) -> VarianceSwap {
    let (start, end) = default_dates();
    VarianceSwap::builder()
        .id(InstrumentId::new(format!("VAR-{side:?}")))
        .underlying_ticker(UNDERLYING_ID.to_string())
        .notional(Money::new(DEFAULT_NOTIONAL, Currency::USD))
        .strike_variance(DEFAULT_STRIKE_VAR)
        .start_date(start)
        .maturity(end)
        .observation_freq(Tenor::daily())
        .observation_calendar_id("USNY".to_string())
        .realized_var_method(RealizedVarMethod::CloseToClose)
        .side(side)
        .discount_curve_id(CurveId::new(DISC_ID))
        .day_count(DayCount::Act365F)
        .attributes(Attributes::new())
        .build()
        .unwrap()
}

/// Build a market context with discount curve and spot, but no forward
/// volatility input. Use only in missing-market-data tests.
pub fn base_context_without_vol() -> MarketContext {
    // Use an earlier base date to allow pre-start valuations
    // Swap starts 2025-01-02, so use 2024-12-01 as curve base
    let curve_base = date(2024, 12, 1);
    let disc_curve = DiscountCurve::builder(DISC_ID)
        .base_date(curve_base)
        .knots([
            (0.0, 1.0),
            (0.25, 0.995),
            (0.5, 0.98),
            (0.75, 0.965),
            (1.0, 0.95),
        ])
        .build()
        .unwrap();
    MarketContext::new()
        .insert(disc_curve)
        .insert_price(UNDERLYING_ID, MarketScalar::Unitless(5_000.0))
}

/// Build a complete base market context with a scalar 20% implied vol.
pub fn base_context() -> MarketContext {
    add_unitless(
        base_context_without_vol(),
        format!("{}_IMPL_VOL", UNDERLYING_ID),
        0.20,
    )
}

/// Add a time series to the market context.
pub fn add_series(ctx: MarketContext, prices: &[(Date, f64)]) -> MarketContext {
    let series = ScalarTimeSeries::new(UNDERLYING_ID, prices.to_vec(), None)
        .unwrap()
        .with_interpolation(SeriesInterpolation::Step);
    ctx.insert_series(series)
}

/// Add a unitless scalar to the market context.
pub fn add_unitless(ctx: MarketContext, id: impl AsRef<str>, value: f64) -> MarketContext {
    ctx.insert_price(id, MarketScalar::Unitless(value))
}

/// Add a volatility surface to the market context.
pub fn add_surface(ctx: MarketContext, surface: VolSurface) -> MarketContext {
    ctx.insert_surface(surface)
}

/// Create a sample volatility surface for testing.
pub fn sample_surface() -> VolSurface {
    VolSurface::builder(UNDERLYING_ID)
        .expiries(&[0.25, 0.50, 0.75, 1.0])
        .strikes(&[4_500.0, 4_800.0, 5_000.0, 5_200.0, 5_500.0])
        .row(&[0.32, 0.30, 0.29, 0.28, 0.27])
        .row(&[0.30, 0.28, 0.27, 0.26, 0.25])
        .row(&[0.29, 0.27, 0.26, 0.25, 0.24])
        .row(&[0.28, 0.26, 0.25, 0.24, 0.23])
        .build()
        .unwrap()
}

/// Generate price series for a swap's observation dates.
pub fn price_series(swap: &VarianceSwap, base: f64, step: f64) -> Vec<(Date, f64)> {
    swap.observation_dates()
        .expect("observation schedule")
        .into_iter()
        .enumerate()
        .map(|(i, d)| (d, base + step * i as f64))
        .collect()
}

/// Calculate observation-based weight for realized variance blending.
pub fn observation_weight(swap: &VarianceSwap, as_of: Date) -> f64 {
    let all = swap.observation_dates().expect("observation schedule");
    if all.is_empty() {
        return 0.0;
    }
    if as_of <= swap.start_date {
        return 0.0;
    }
    if as_of >= swap.maturity {
        return 1.0;
    }
    let total = all.len() as f64;
    let realized = all.iter().filter(|&&d| d <= as_of).count() as f64;
    (realized / total).clamp(0.0, 1.0)
}

/// Approximate epsilon for floating point comparisons.
pub const EPSILON: f64 = 1e-10;
pub const LOOSE_EPSILON: f64 = 1e-6;

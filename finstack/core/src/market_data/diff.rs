//! Market data comparison and shift measurement.
//!
//! Provides utilities for measuring market movements between two `MarketContext`
//! instances. Used primarily for metrics-based P&L attribution, risk reporting,
//! and scenario analysis.
//!
//! # Purpose
//!
//! This module is symmetric to `bumps.rs`:
//! - **bumps.rs**: Apply shifts to create scenarios
//! - **diff.rs**: Measure shifts between markets
//!
//! # Use Cases
//!
//! - **P&L Attribution**: Explain P&L changes via DV01 × Δrates, CS01 × Δspreads
//! - **Risk Reporting**: Measure daily market moves for VaR and stress testing
//! - **Calibration**: Compare calibrated curves vs market inputs
//! - **Market Analysis**: Track curve movements over time
//!
//! # Examples
//!
//! ```rust
//! use finstack_core::market_data::diff::{measure_discount_curve_shift, TenorSamplingMethod};
//! use finstack_core::market_data::context::MarketContext;
//! use finstack_core::types::CurveId;
//!
//! # fn example(market_yesterday: MarketContext, market_today: MarketContext) -> finstack_core::Result<()> {
//! // Measure rate shift between two markets
//! let shift_bp = measure_discount_curve_shift(
//!     &CurveId::from("USD-OIS"),
//!     &market_yesterday,
//!     &market_today,
//!     TenorSamplingMethod::Standard,
//! )?;
//!
//! println!("USD-OIS moved {} basis points", shift_bp);
//! # Ok(())
//! # }
//! ```

use super::context::MarketContext;
use crate::currency::Currency;
use crate::dates::Date;
use crate::Result;

use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

/// Standard market tenors for curve sampling (in years).
///
/// Based on liquid swap market points: 3M, 6M, 1Y, 2Y, 3Y, 5Y, 7Y, 10Y, 30Y.
/// These are the conventional points where curves are most actively traded
/// and quoted.
pub const STANDARD_TENORS: &[f64] = &[0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 30.0];

/// ATM reference strike multiplier (1.0 = 100% of spot).
pub const ATM_MONEYNESS: f64 = 1.0;

/// Default volatility surface expiry for sampling (1 year).
pub const DEFAULT_VOL_EXPIRY: f64 = 1.0;

// -----------------------------------------------------------------------------
// Tenor Sampling Method
// -----------------------------------------------------------------------------

/// Method for selecting tenor points when measuring curve shifts.
///
/// Different sampling strategies trade off accuracy, performance, and
/// robustness to curve structure.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenorSamplingMethod {
    /// Standard swap market tenors (3M, 6M, 1Y, 2Y, 3Y, 5Y, 7Y, 10Y, 30Y).
    ///
    /// Most robust for typical interest rate curves. Matches market liquidity
    /// points and works well for parallel shift detection.
    #[default]
    Standard,

    /// Use curve's own knot points dynamically.
    ///
    /// Adapts to curve structure but may miss shifts between pillars.
    /// Good for curves with non-standard pillar structure.
    Dynamic,

    /// Custom tenor list specified by caller.
    ///
    /// Allows fine-grained control for specific use cases (e.g., matching
    /// an instrument's cashflow schedule).
    Custom(Vec<f64>),
}

impl TenorSamplingMethod {
    /// Get the tenor points to sample based on the method.
    ///
    /// For `Dynamic`, uses the knot points from the provided curve.
    /// For `Standard`, uses `STANDARD_TENORS`.
    /// For `Custom`, uses the provided tenor list.
    fn tenors<'a>(&'a self, curve_knots: Option<&'a [f64]>) -> &'a [f64] {
        match self {
            Self::Standard => STANDARD_TENORS,
            Self::Dynamic => curve_knots.unwrap_or(STANDARD_TENORS),
            Self::Custom(tenors) => tenors.as_slice(),
        }
    }
}

// -----------------------------------------------------------------------------
// Curve Shift Measurements
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// Curve Shift Measurements
// -----------------------------------------------------------------------------

/// Generic internal measurement helper for curve-like objects.
fn measure_average_shift(
    sample_points: &[f64],
    scaling_factor: f64,
    mut value_t0: impl FnMut(f64) -> f64,
    mut value_t1: impl FnMut(f64) -> f64,
) -> f64 {
    let mut total_shift = 0.0;
    let mut count = 0;

    for &t in sample_points {
        if t <= 0.0 {
            continue;
        }
        let v0 = value_t0(t);
        let v1 = value_t1(t);

        let shift = if values_are_effectively_equal(v0, v1) {
            0.0
        } else {
            (v1 - v0) * scaling_factor
        };
        total_shift += shift;
        count += 1;
    }

    if count == 0 {
        return 0.0;
    }
    total_shift / count as f64
}

fn values_are_effectively_equal(a: f64, b: f64) -> bool {
    const RELATIVE_EQUALITY_TOLERANCE: f64 = 1.0e-14;

    if !(a.is_finite() && b.is_finite()) {
        return false;
    }
    let scale = a.abs().max(b.abs()).max(1.0);
    (a - b).abs() <= RELATIVE_EQUALITY_TOLERANCE * scale
}

/// Measure average parallel rate shift in discount curve (basis points).
pub fn measure_discount_curve_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    method: TenorSamplingMethod,
) -> Result<f64> {
    let curve_t0 = market_t0.get_discount(&curve_id)?;
    let curve_t1 = market_t1.get_discount(&curve_id)?;

    let tenors = method.tenors(Some(curve_t0.knots()));
    Ok(measure_average_shift(
        tenors,
        10_000.0,
        |t| curve_t0.zero(t),
        |t| curve_t1.zero(t),
    ))
}

/// Measure average parallel spread shift in hazard curve (basis points).
pub fn measure_hazard_curve_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    method: TenorSamplingMethod,
) -> Result<f64> {
    let curve_t0 = market_t0.get_hazard(&curve_id)?;
    let curve_t1 = market_t1.get_hazard(&curve_id)?;

    let t0_knots: Vec<f64> = curve_t0.knot_points().map(|(t, _)| t).collect();
    let tenors = match method {
        TenorSamplingMethod::Dynamic => t0_knots.as_slice(),
        _ => method.tenors(None),
    };

    Ok(measure_average_shift(
        tenors,
        10_000.0,
        |t| curve_t0.hazard_rate(t),
        |t| curve_t1.hazard_rate(t),
    ))
}

/// Measure average inflation rate shift (basis points).
pub fn measure_inflation_curve_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let curve_t0 = market_t0.get_inflation_curve(&curve_id)?;
    let curve_t1 = market_t1.get_inflation_curve(&curve_id)?;
    Ok(measure_average_shift(
        STANDARD_TENORS,
        10_000.0,
        |t| {
            let ratio = curve_t0.cpi(t) / curve_t0.base_cpi();
            if t == 0.0 {
                0.0
            } else {
                ratio.powf(1.0 / t) - 1.0
            }
        },
        |t| {
            let ratio = curve_t1.cpi(t) / curve_t1.base_cpi();
            if t == 0.0 {
                0.0
            } else {
                ratio.powf(1.0 / t) - 1.0
            }
        },
    ))
}

// -----------------------------------------------------------------------------
// Surface Shift Measurements
// -----------------------------------------------------------------------------

/// Measure volatility surface shift (percentage points).
///
/// Measures the change in implied volatility levels. Can measure at a specific
/// point or sample across the surface for average shift.
///
/// # Arguments
///
/// * `surface_id` - Volatility surface identifier
/// * `market_t0` - Market context at T₀
/// * `market_t1` - Market context at T₁
/// * `reference_expiry` - Optional expiry to measure (defaults to 1Y ATM)
/// * `reference_strike` - Optional strike to measure (defaults to ATM)
///
/// # Returns
///
/// Average volatility shift in percentage points (e.g., 2.0 = +2% vol).
///
/// # Errors
///
/// Returns error if surface not found in either market.
///
/// # Examples
///
/// ```rust
/// # use finstack_core::market_data::diff::measure_vol_surface_shift;
/// # use finstack_core::market_data::context::MarketContext;
/// # use finstack_core::types::CurveId;
/// # fn example(market_t0: MarketContext, market_t1: MarketContext) -> finstack_core::Result<()> {
/// // Measure 1Y ATM vol shift
/// let vol_shift = measure_vol_surface_shift(
///     &CurveId::from("SPX-VOL"),
///     &market_t0,
///     &market_t1,
///     Some(1.0),  // 1Y expiry
///     Some(1.0),  // ATM (100%)
/// )?;
/// # Ok(())
/// # }
/// ```
pub fn measure_vol_surface_shift(
    surface_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    reference_expiry: Option<f64>,
    reference_strike: Option<f64>,
) -> Result<f64> {
    let surface_t0 = market_t0.get_surface(&surface_id)?;
    let surface_t1 = market_t1.get_surface(&surface_id)?;

    // If specific point requested, measure there
    if let (Some(expiry), Some(strike)) = (reference_expiry, reference_strike) {
        let vol_t0 = surface_t0.value_checked(expiry, strike)?;
        let vol_t1 = surface_t1.value_checked(expiry, strike)?;

        // Shift in percentage points
        return Ok((vol_t1 - vol_t0) * 100.0);
    }

    // Otherwise, sample across surface at standard points
    let expiries = surface_t0.expiries();
    let strikes = surface_t0.strikes();

    let mut total_shift = 0.0;
    let mut sample_count = 0;

    // Sample at available expiries and middle strike (approximately ATM)
    for &expiry in expiries {
        if expiry <= 0.0 {
            continue;
        }

        // Use middle strike as ATM approximation
        if strikes.is_empty() {
            continue;
        }
        let mid_idx = strikes.len() / 2;
        let strike = strikes[mid_idx];

        let vol_t0 = surface_t0.value_checked(expiry, strike)?;
        let vol_t1 = surface_t1.value_checked(expiry, strike)?;

        let shift_pct = (vol_t1 - vol_t0) * 100.0;

        total_shift += shift_pct;
        sample_count += 1;
    }

    if sample_count == 0 {
        return Ok(0.0);
    }

    Ok(total_shift / sample_count as f64)
}

// -----------------------------------------------------------------------------
// FX and Scalar Shift Measurements
// -----------------------------------------------------------------------------

/// Measure FX spot rate shift (percentage change).
///
/// Calculates the percentage change in FX rate between two markets.
///
/// # Arguments
///
/// * `base_ccy` - Base currency
/// * `quote_ccy` - Quote currency
/// * `market_t0` - Market context at T₀
/// * `market_t1` - Market context at T₁
///
/// # Returns
///
/// Percentage change in FX rate: (rate_t1 / rate_t0 - 1) * 100
///
/// # Errors
///
/// Returns error if FX matrix not available in either market or conversion fails.
///
/// # Examples
///
/// ```rust
/// # use finstack_core::market_data::diff::measure_fx_shift;
/// # use finstack_core::market_data::context::MarketContext;
/// # use finstack_core::currency::Currency;
/// # use time::macros::date;
/// # fn example(market_t0: MarketContext, market_t1: MarketContext) -> finstack_core::Result<()> {
/// let date_t0 = date!(2024 - 01 - 01);
/// let date_t1 = date!(2024 - 01 - 02);
/// let fx_shift = measure_fx_shift(
///     Currency::USD,
///     Currency::EUR,
///     &market_t0,
///     &market_t1,
///     date_t0,
///     date_t1,
/// )?;
///
/// println!("USD/EUR moved {}%", fx_shift);
/// # Ok(())
/// # }
/// ```
pub fn measure_fx_shift(
    base_ccy: Currency,
    quote_ccy: Currency,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
) -> Result<f64> {
    use crate::money::fx::FxQuery;

    // Get FX matrices via the shared helper on MarketContext.
    let fx_t0 = market_t0.fx_required()?;
    let fx_t1 = market_t1.fx_required()?;

    // Get rates using FxQuery with the provided valuation dates
    let query_t0 = FxQuery::new(base_ccy, quote_ccy, as_of_t0);
    let query_t1 = FxQuery::new(base_ccy, quote_ccy, as_of_t1);
    let rate_t0 = fx_t0.rate(query_t0)?.rate;
    let rate_t1 = fx_t1.rate(query_t1)?.rate;

    // Guard against division by zero
    if rate_t0 == 0.0 {
        return Err(crate::Error::Validation(format!(
            "Cannot compute FX shift: rate_t0 is zero for {}/{}",
            base_ccy, quote_ccy
        )));
    }

    // Percentage change
    let pct_change = (rate_t1 / rate_t0 - 1.0) * 100.0;

    Ok(pct_change)
}

/// Measure market scalar shift (percentage change).
///
/// Measures the change in market scalar values (equity prices, commodities, etc.).
///
/// # Arguments
///
/// * `scalar_id` - Market scalar identifier
/// * `market_t0` - Market context at T₀
/// * `market_t1` - Market context at T₁
///
/// # Returns
///
/// Percentage change in scalar value.
///
/// # Errors
///
/// Returns error if scalar not found in either market.
pub fn measure_scalar_shift(
    scalar_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    use crate::market_data::scalars::MarketScalar;

    let scalar_t0 = market_t0.get_price(&scalar_id)?;
    let scalar_t1 = market_t1.get_price(&scalar_id)?;

    // Extract numeric values from enum
    let value_t0 = match scalar_t0 {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    };

    let value_t1 = match scalar_t1 {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    };

    // Guard against division by zero
    if value_t0.abs() < 1e-15 {
        return Err(crate::Error::Validation(format!(
            "Cannot compute percentage shift: baseline value is zero for scalar '{}'",
            scalar_id.as_ref()
        )));
    }

    // Percentage change
    let pct_change = (value_t1 / value_t0 - 1.0) * 100.0;

    if !pct_change.is_finite() {
        return Err(crate::Error::Validation(format!(
            "Non-finite percentage shift computed for scalar '{}': t0={}, t1={}",
            scalar_id.as_ref(),
            value_t0,
            value_t1
        )));
    }

    Ok(pct_change)
}

/// Compute the **absolute** change in a market scalar between two market states.
///
/// Unlike [`measure_scalar_shift`], which returns a percentage change, this
/// returns `value_t1 - value_t0` in the scalar's native units. Use this when
/// applying a sensitivity defined per unit of underlying move — e.g. an option
/// delta `dPV/dS` or a dividend sensitivity `dPV/d(dividend)` — where
/// multiplying by a percentage shift would introduce a `100 / level` scaling
/// error.
///
/// # Errors
///
/// Returns `Err` if either market lacks the scalar or the computed shift is
/// non-finite.
pub fn measure_scalar_absolute_shift(
    scalar_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    use crate::market_data::scalars::MarketScalar;

    let scalar_t0 = market_t0.get_price(&scalar_id)?;
    let scalar_t1 = market_t1.get_price(&scalar_id)?;

    let value_t0 = match scalar_t0 {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    };
    let value_t1 = match scalar_t1 {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    };

    let abs_change = value_t1 - value_t0;
    if !abs_change.is_finite() {
        return Err(crate::Error::Validation(format!(
            "Non-finite absolute shift computed for scalar '{}': t0={}, t1={}",
            scalar_id.as_ref(),
            value_t0,
            value_t1
        )));
    }
    Ok(abs_change)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::Date;
    use crate::market_data::term_structures::{DiscountCurve, HazardCurve};
    use crate::math::interp::InterpStyle;
    use time::Month;

    fn sample_date() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date")
    }

    #[test]
    fn test_parallel_discount_shift() {
        let base_date = sample_date();

        // Create base curve (4% rates approximately)
        let curve_t0 = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.82), (10.0, 0.67)])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("Market diff calculation should succeed in test");

        // Create curve with higher rates (~4.5%, which is ~50bp higher)
        // DF(t) = exp(-r*t), so for +50bp: DF_new = DF_old * exp(-0.005*t)
        let curve_t1 = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([
                (0.0, 1.0),
                (1.0, 0.96 * (-0.005_f64 * 1.0).exp()),
                (5.0, 0.82 * (-0.005_f64 * 5.0).exp()),
                (10.0, 0.67 * (-0.005_f64 * 10.0).exp()),
            ])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("Market diff calculation should succeed in test");

        let market_t0 = MarketContext::new().insert(curve_t0);
        let market_t1 = MarketContext::new().insert(curve_t1);

        let shift = measure_discount_curve_shift(
            "USD-OIS",
            &market_t0,
            &market_t1,
            TenorSamplingMethod::Standard,
        )
        .expect("Market diff calculation should succeed in test");

        // Should detect approximately 50bp shift (within tolerance for sampling/interpolation)
        assert!((shift - 50.0).abs() < 5.0, "Expected ~50bp, got {}", shift);
    }

    #[test]
    fn test_hazard_curve_shift() {
        let base_date = sample_date();

        let curve_t0 = HazardCurve::builder("CORP-01")
            .base_date(base_date)
            .recovery_rate(0.4)
            .knots(vec![(1.0, 0.01), (5.0, 0.02), (10.0, 0.025)])
            .build()
            .expect("Market diff calculation should succeed in test");

        // Create a shifted curve (+25bp = 0.0025)
        let curve_t1 = HazardCurve::builder("CORP-01")
            .base_date(base_date)
            .recovery_rate(0.4)
            .knots(vec![(1.0, 0.0125), (5.0, 0.0225), (10.0, 0.0275)])
            .build()
            .expect("Market diff calculation should succeed in test");

        let market_t0 = MarketContext::new().insert(curve_t0);
        let market_t1 = MarketContext::new().insert(curve_t1);

        let shift = measure_hazard_curve_shift(
            "CORP-01",
            &market_t0,
            &market_t1,
            TenorSamplingMethod::Standard,
        )
        .expect("Market diff calculation should succeed in test");

        assert!((shift - 25.0).abs() < 1.0, "Expected ~25bp, got {}", shift);
    }

    #[test]
    fn test_missing_curve_error() {
        let market_t0 = MarketContext::new();
        let market_t1 = MarketContext::new();

        let result = measure_discount_curve_shift(
            "MISSING",
            &market_t0,
            &market_t1,
            TenorSamplingMethod::Standard,
        );

        assert!(result.is_err());
    }

    #[test]
    fn average_shift_uses_relative_equality_for_large_magnitudes() {
        let shift = measure_average_shift(&[1.0], 10_000.0, |_| 1.0e9, |_| 1.0e9 + 1.0e-6);

        assert_eq!(shift, 0.0);
    }

    #[test]
    fn test_tenor_sampling_methods() {
        // Test that all sampling methods produce results
        let base_date = sample_date();

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("Market diff calculation should succeed in test");

        let market = MarketContext::new().insert(curve);

        // Standard
        let shift_std = measure_discount_curve_shift(
            "USD-OIS",
            &market,
            &market,
            TenorSamplingMethod::Standard,
        )
        .expect("Market diff calculation should succeed in test");
        assert_eq!(shift_std, 0.0); // Same market → zero shift

        // Dynamic
        let shift_dyn =
            measure_discount_curve_shift("USD-OIS", &market, &market, TenorSamplingMethod::Dynamic)
                .expect("Market diff calculation should succeed in test");
        assert_eq!(shift_dyn, 0.0);

        // Custom
        let shift_custom = measure_discount_curve_shift(
            "USD-OIS",
            &market,
            &market,
            TenorSamplingMethod::Custom(vec![1.0, 2.0, 3.0]),
        )
        .expect("Market diff calculation should succeed in test");
        assert_eq!(shift_custom, 0.0);
    }
}

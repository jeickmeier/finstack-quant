//! Market adapter for the piecewise-constant GBM used by the equity
//! path-dependent Monte Carlo pricers (cliquet, autocallable).
//!
//! The process, its exact discretization and the bootstrap live in
//! [`finstack_quant_models::monte_carlo::process::piecewise_gbm`]; this
//! module turns dated schedule points, a discount curve and the volatility
//! override / surface resolution into the model-level checkpoints.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::Result;
use finstack_quant_models::monte_carlo::process::piecewise_gbm::{
    GbmCheckpoint, PiecewiseGbmProcess,
};
use finstack_quant_valuations_core::instruments::common_impl::vol_resolution::resolve_sigma_at;
use finstack_quant_valuations_core::instruments::MarketQuoteOverrides;

/// Bootstrap the forward-vol / forward-rate GBM for dated schedule points.
///
/// Discount factors are the exact date-based factors from `as_of`, so the
/// bridge is correct even when the curve base date or day count differs from
/// the model clock; volatility comes from the override or the surface at the
/// ATM forward of each point.
///
/// # Arguments
///
/// * `disc_curve` - Discount curve for the pricing currency.
/// * `curves` - Market holding the volatility surface.
/// * `market_quotes` - Instrument quote overrides; a flat implied volatility override wins over the surface.
/// * `vol_surface_id` - Surface identifier used when no override is set.
/// * `as_of` - Valuation date the discount factors start from.
/// * `initial_spot` - Spot at `as_of`.
/// * `div_yield` - Continuous dividend yield as a decimal.
/// * `check_points` - Model time and calendar date of each schedule point, in increasing time.
/// * `context_label` - Caller name used in error messages.
///
/// # Errors
///
/// Propagates discount-factor, volatility-resolution and bootstrap errors.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bootstrap_forward_gbm(
    disc_curve: &DiscountCurve,
    curves: &MarketContext,
    market_quotes: &MarketQuoteOverrides,
    vol_surface_id: &str,
    as_of: Date,
    initial_spot: f64,
    div_yield: f64,
    check_points: &[(f64, Date)],
    context_label: &str,
) -> Result<PiecewiseGbmProcess> {
    let mut points = Vec::with_capacity(check_points.len());
    for &(t, date) in check_points {
        points.push(GbmCheckpoint {
            t,
            df: disc_curve.df_between_dates(as_of, date)?,
        });
    }
    finstack_quant_models::monte_carlo::process::piecewise_gbm::bootstrap_forward_gbm(
        &points,
        initial_spot,
        div_yield,
        |t, forward| resolve_sigma_at(market_quotes, curves, vol_surface_id, t, forward),
        context_label,
    )
}

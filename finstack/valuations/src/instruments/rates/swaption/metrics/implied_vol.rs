//! Implied volatility metric for swaptions.
//!
//! Solves for the Black implied volatility that reproduces the current PV
//! (from `context.base_value`) using the `/math` solvers. Uses a robust
//! parameterization in log-vol space and falls back to reasonable defaults
//! if inversion is not possible.

use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::pricing_overrides::VolSurfaceExtrapolation;
use crate::instruments::rates::swaption::Swaption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::market_data::surfaces::VolSurfaceAxis;
use finstack_core::math::solver::{BrentSolver, Solver};
use finstack_core::Result;

/// Implied Volatility calculator for swaptions
pub(crate) struct ImpliedVolCalculator;

impl MetricCalculator for ImpliedVolCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &Swaption = context.instrument_as()?;
        let strike = option.strike_f64()?;

        // Time to expiry from as_of
        let t = year_fraction(option.day_count, context.as_of, option.expiry)?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        // Target price is the base PV already computed under instrument pricing
        let target_pv = context.base_value.amount();

        let forward = option.forward_swap_rate(context.curves.as_ref(), context.as_of)?;
        if option.vol_model == crate::instruments::rates::swaption::VolatilityModel::Black
            && (forward <= 0.0 || strike <= 0.0)
        {
            return Err(finstack_core::Error::Validation(format!(
                "Black swaption implied vol requires positive forward and strike, got forward={} strike={}",
                forward, strike
            )));
        }

        // Build objective in log-vol space x = ln(sigma)
        let f = |x: f64| -> f64 {
            let sigma = x.exp();
            // Use Black pricing along the same path as instrument pricing (not SABR)
            // since we are solving for the equivalent Black vol.
            match option.price_black(context.curves.as_ref(), sigma, context.as_of) {
                Ok(m) => m.amount() - target_pv,
                Err(_) => 1.0e6, // steer solver away from invalid regions
            }
        };

        // Initial guess: overrides -> SABR ATM -> surface -> 20%
        let initial_sigma =
            if let Some(ov) = option.pricing_overrides.market_quotes.implied_volatility {
                ov
            } else if let Some(sabr) = &option.sabr_params {
                let model = crate::models::SABRModel::new(sabr.to_internal()?);
                model.implied_volatility(forward, strike, t).unwrap_or(0.2)
            } else {
                context
                    .curves
                    .get_surface(option.vol_surface_id.as_str())
                    .and_then(|s| {
                        s.require_secondary_axis(VolSurfaceAxis::Strike)?;
                        match option
                            .pricing_overrides
                            .model_config
                            .vol_surface_extrapolation
                        {
                            VolSurfaceExtrapolation::Clamp
                            | VolSurfaceExtrapolation::LinearInVariance => {
                                // LinearInVariance falls back to Clamp until surface impl is ready
                                Ok(s.value_clamped(t, strike))
                            }
                            VolSurfaceExtrapolation::Error => s.value_checked(t, strike),
                        }
                    })
                    .unwrap_or(0.2)
            };

        let eps = 1e-8;
        let x0 = (initial_sigma.max(eps)).ln();

        // Try Brent solver
        let solver = BrentSolver::new().tolerance(1e-10);
        let implied_x = match solver.solve(f, x0) {
            Ok(root) => root,
            Err(_) => {
                // Fallback: sample two bounds and pick the closer one
                let x_lo = (1e-6_f64).ln();
                let x_hi = (3.0_f64).ln();
                let flo = f(x_lo).abs();
                let fhi = f(x_hi).abs();
                if flo <= fhi {
                    x_lo
                } else {
                    x_hi
                }
            }
        };

        let sigma = implied_x.exp();
        Ok(sigma)
    }
}

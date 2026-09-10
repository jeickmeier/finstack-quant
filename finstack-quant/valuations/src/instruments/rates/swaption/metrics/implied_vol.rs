//! Implied volatility metric for swaptions.
//!
//! Solves for the configured quoted volatility that reproduces the current PV
//! (from `context.base_value`) using the `/math` solvers. Uses a robust
//! non-negative volatility bracket. If inversion is not possible (solver
//! failure or non-converged residual) an error is returned rather than a
//! fabricated bound value, so risk systems never receive a fake vol.

use crate::instruments::rates::swaption::{Swaption, VolatilityModel};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::math::solver::BrentSolver;
use finstack_quant_core::{Error, Result};

/// Implied volatility in the configured normal or displaced Black quote units.
pub(crate) struct ImpliedVolCalculator;

impl MetricCalculator for ImpliedVolCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &Swaption = context.instrument_as()?;
        if context.as_of >= option.expiry {
            return Ok(0.0);
        }
        let target = context.base_value.amount();
        let notional = option.notional.amount();
        if !target.is_finite() || target < 0.0 || notional <= 0.0 {
            return Err(Error::Validation(
                "swaption implied vol requires finite non-negative PV and positive notional"
                    .to_owned(),
            ));
        }
        let price = |sigma: f64| match option.vol_model {
            VolatilityModel::Normal => {
                option.price_normal(context.curves.as_ref(), sigma, context.as_of)
            }
            VolatilityModel::Black => {
                option.price_black(context.curves.as_ref(), sigma, context.as_of)
            }
        };
        // Check model/domain errors before entering the scalar solver.
        price(0.0)?;
        let objective =
            |sigma: f64| price(sigma).map_or(f64::NAN, |pv| (pv.amount() - target) / notional);
        let upper = match option.vol_model {
            VolatilityModel::Normal => 1.0,
            VolatilityModel::Black => 5.0,
        };
        let sigma = BrentSolver::new()
            .tolerance(1e-12)
            .solve_in_bracket(objective, 0.0, upper)?;
        let residual = price(sigma)?.amount() - target;
        let tolerance = 1e-8 * target.abs().max(1.0);
        if residual.abs() > tolerance {
            return Err(Error::Validation(format!(
                "swaption implied vol price residual {residual} exceeds {tolerance}"
            )));
        }
        Ok(sigma)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::rates::swaption::SwaptionParams;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::money::Money;
    use std::sync::Arc;
    use time::macros::date;

    fn flat_market(as_of: Date, rate: f64, vol: f64) -> MarketContext {
        let disc = DiscountCurve::builder("USD_OIS")
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([
                (0.0, 1.0),
                (1.0, (-rate).exp()),
                (5.0, (-rate * 5.0).exp()),
                (10.0, (-rate * 10.0).exp()),
            ])
            .build()
            .expect("discount curve");
        let fwd = ForwardCurve::builder("USD_LIBOR_3M", 0.25)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, rate), (10.0, rate)])
            .build()
            .expect("forward curve");
        let surface = VolSurface::builder("USD_SWAPTION_VOL")
            .expiries(&[0.25, 1.0, 5.0, 10.0])
            .strikes(&[0.02, 0.03, 0.05, 0.07])
            .row(&[vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol])
            .build()
            .expect("vol surface");
        MarketContext::new()
            .insert(disc)
            .insert(fwd)
            .insert_surface(surface)
    }

    fn payer_swaption(strike: f64) -> Swaption {
        let params = SwaptionParams::payer(
            Money::from((1_000_000_i64, Currency::USD)),
            strike,
            date!(2025 - 01 - 01),
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
        )
        .expect("valid swaption params")
        .with_fixed_frequency(Tenor::semi_annual())
        .with_float_frequency(Tenor::quarterly())
        .with_fixed_day_count(DayCount::Thirty360)
        .with_float_day_count(DayCount::Act360);
        Swaption::new(
            "SWAPTION_IV_TEST",
            &params,
            "USD_OIS",
            "USD_LIBOR_3M",
            "USD_SWAPTION_VOL",
        )
    }

    fn context_with_target(target_pv: f64, strike: f64) -> MetricContext {
        let as_of = date!(2024 - 01 - 01);
        let market = flat_market(as_of, 0.05, 0.20);
        let swaption = payer_swaption(strike);
        MetricContext::new(
            Arc::new(swaption),
            Arc::new(market),
            as_of,
            Money::new(target_pv, Currency::USD).expect("valid money fixture"),
            MetricContext::default_config(),
        )
    }

    /// A target PV below the discounted intrinsic value has no Black-vol root;
    /// the calculator must return an error rather than a fabricated bound
    /// endpoint (the previous behavior returned 1e-6 or 3.0 silently).
    #[test]
    fn target_below_intrinsic_returns_error() {
        // Deep ITM payer (forward ~5%, strike 1%): intrinsic PV is large; a
        // 1-dollar target is unreachable for any non-negative vol.
        let mut ctx = context_with_target(1.0, 0.01);
        let result = ImpliedVolCalculator.calculate(&mut ctx);
        assert!(
            result.is_err(),
            "expected solver-failure error, got {result:?}"
        );
    }

    /// Round-trip sanity: a genuine Black PV must still invert cleanly.
    #[test]
    fn round_trip_recovers_vol() {
        let as_of = date!(2024 - 01 - 01);
        let market = flat_market(as_of, 0.05, 0.20);
        let swaption = payer_swaption(0.05);
        let target = swaption
            .price_black(&market, 0.25, as_of)
            .expect("black price");
        let mut ctx = MetricContext::new(
            Arc::new(swaption),
            Arc::new(market),
            as_of,
            target,
            MetricContext::default_config(),
        );
        let sigma = ImpliedVolCalculator
            .calculate(&mut ctx)
            .expect("implied vol");
        assert!((sigma - 0.25).abs() < 1e-6, "expected ~0.25, got {sigma}");
    }
}

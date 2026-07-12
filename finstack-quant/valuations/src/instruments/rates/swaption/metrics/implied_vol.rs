//! Implied volatility metric for swaptions.
//!
//! Solves for the Black implied volatility that reproduces the current PV
//! (from `context.base_value`) using the `/math` solvers. Uses a robust
//! parameterization in log-vol space. If inversion is not possible (solver
//! failure or non-converged residual) an error is returned rather than a
//! fabricated bound value, so risk systems never receive a fake vol.

use crate::instruments::pricing_overrides::VolSurfaceExtrapolation;
use crate::instruments::rates::swaption::Swaption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::surfaces::VolSurfaceAxis;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::Result;

/// Implied Volatility calculator for swaptions
pub(crate) struct ImpliedVolCalculator;

impl MetricCalculator for ImpliedVolCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &Swaption = context.instrument_as()?;
        let strike = option.strike_f64()?;

        // Time to expiry from as_of. ACT/365F matches the pricer's option-time
        // convention (`Swaption::price_black` uses ACT/365F regardless of the
        // instrument's accrual day count), so the inverted vol lives on the
        // same time axis as the vol used for pricing.
        let t = finstack_quant_core::dates::DayCount::Act365F.signed_year_fraction(
            context.as_of,
            option.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        // Target price is the base PV already computed under instrument pricing
        let target_pv = context.base_value.amount();

        let forward = option.forward_swap_rate(context.curves.as_ref(), context.as_of)?;
        if option.vol_model == crate::instruments::rates::swaption::VolatilityModel::Black
            && (forward <= 0.0 || strike <= 0.0)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
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
                let model = crate::models::SABRModel::new(sabr.clone());
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

        // Try Brent solver; on failure return an error instead of fabricating
        // a bound endpoint (a 0.0001%/300% vol is indistinguishable from a
        // real solution downstream).
        let solver = BrentSolver::new().tolerance(1e-10);
        let implied_x = solver.solve(f, x0).map_err(|e| {
            finstack_quant_core::Error::Validation(format!(
                "swaption implied vol solver failed (target_pv={target_pv}, forward={forward}, \
                 strike={strike}): {e}"
            ))
        })?;

        // Reject pseudo-roots: the residual at the returned point must
        // actually reproduce the target PV (e.g. target below discounted
        // intrinsic has no root and some solvers return a boundary point).
        let residual = f(implied_x);
        let pv_tol = 1e-6 * target_pv.abs().max(1.0);
        if !residual.is_finite() || residual.abs() > pv_tol {
            return Err(finstack_quant_core::Error::Validation(format!(
                "swaption implied vol did not converge: residual {residual} exceeds tolerance \
                 {pv_tol} (target_pv={target_pv}, forward={forward}, strike={strike})"
            )));
        }

        let sigma = implied_x.exp();
        Ok(sigma)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::rates::swaption::{
        SwaptionExercise, SwaptionSettlement, VolatilityModel,
    };
    use crate::instruments::{OptionType, PricingOverrides};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::money::Money;
    use rust_decimal::Decimal;
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
        Swaption {
            id: "SWAPTION_IV_TEST".into(),
            option_type: OptionType::Call,
            notional: Money::new(1_000_000.0, Currency::USD),
            strike: Decimal::try_from(strike).expect("valid decimal"),
            expiry: date!(2025 - 01 - 01),
            swap_start: date!(2025 - 01 - 01),
            swap_end: date!(2030 - 01 - 01),
            fixed_freq: Tenor::semi_annual(),
            float_freq: Tenor::quarterly(),
            day_count: DayCount::Thirty360,
            exercise_style: SwaptionExercise::European,
            settlement: SwaptionSettlement::Physical,
            cash_settlement_method: Default::default(),
            vol_model: VolatilityModel::Black,
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            vol_surface_id: "USD_SWAPTION_VOL".into(),
            pricing_overrides: PricingOverrides::default(),
            calendar_id: None,
            underlying_fixed_leg: None,
            underlying_float_leg: None,
            sabr_params: None,
            attributes: Default::default(),
        }
    }

    fn context_with_target(target_pv: f64, strike: f64) -> MetricContext {
        let as_of = date!(2024 - 01 - 01);
        let market = flat_market(as_of, 0.05, 0.20);
        let swaption = payer_swaption(strike);
        MetricContext::new(
            Arc::new(swaption),
            Arc::new(market),
            as_of,
            Money::new(target_pv, Currency::USD),
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

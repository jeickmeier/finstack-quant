//! Equity option PDE pricer using 1D Crank-Nicolson finite differences.
//!
//! Solves the Black-Scholes PDE in log-spot coordinates on a sinh-concentrated
//! grid. Supports both European and American exercise via the penalty method.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::equity_option::pricing::{
    collect_inputs_extended, has_future_discrete_dividends, resolve_lifecycle_value,
};
use crate::instruments::equity::equity_option::types::EquityOption;
use crate::instruments::ExerciseStyle;
use crate::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

use crate::instruments::common_impl::parameters::OptionType;
use finstack_quant_models::pde::{BlackScholesPde, Grid1D, Solver1D};

/// Equity option pricer using 1D PDE (Crank-Nicolson) with Black-Scholes dynamics.
///
/// Solves the BS PDE on a log-spot grid concentrated near the strike. Supports
/// European and American exercise styles (American via penalty early-exercise).
pub(crate) struct EquityOptionPdePricer {
    /// Number of spatial grid points.
    space_points: usize,
    /// Number of time steps.
    time_steps: usize,
}

impl Default for EquityOptionPdePricer {
    fn default() -> Self {
        Self {
            space_points: 200,
            time_steps: 100,
        }
    }
}

impl EquityOptionPdePricer {
    /// Price the equity option via the 1D Black-Scholes PDE.
    fn price_internal(
        &self,
        inst: &EquityOption,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<Money, PricingError> {
        if let Some(value) = resolve_lifecycle_value(inst, market, as_of).map_err(|error| {
            PricingError::model_failure_with_context(
                error.to_string(),
                PricingErrorContext::from_instrument(inst).model(ModelKey::PdeCrankNicolson1D),
            )
        })? {
            return Ok(value);
        }
        if matches!(inst.exercise_style, ExerciseStyle::Bermudan) {
            return Err(PricingError::model_failure_with_context(
                "EquityOption PDE1D does not support Bermudan exercise; use a tree model",
                PricingErrorContext::from_instrument(inst).model(ModelKey::PdeCrankNicolson1D),
            ));
        }
        if matches!(inst.exercise_style, ExerciseStyle::American)
            && has_future_discrete_dividends(inst, as_of)
        {
            return Err(PricingError::model_failure_with_context(
                "EquityOption PDE1D does not support American exercise with discrete \
                 dividends; use the discrete-dividend tree model",
                PricingErrorContext::from_instrument(inst).model(ModelKey::PdeCrankNicolson1D),
            ));
        }
        let inputs = collect_inputs_extended(inst, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(inst).model(ModelKey::PdeCrankNicolson1D),
            )
        })?;
        let spot = inputs.spot;
        let r = inputs.r;
        let q = inputs.q;
        let sigma = inputs.sigma;
        let t = inputs.t_vol;
        let ccy = inst.currency;

        if t <= 0.0 {
            let intrinsic = match inst.option_type {
                OptionType::Call => (spot - inst.strike).max(0.0),
                OptionType::Put => (inst.strike - spot).max(0.0),
            };
            return Money::new(intrinsic * inst.quantity, ccy).map_err(|error| {
                crate::pricer::PricingError::from_core(
                    error,
                    crate::pricer::PricingErrorContext::from_instrument(inst)
                        .model(ModelKey::PdeCrankNicolson1D),
                )
            });
        }

        let is_call = matches!(inst.option_type, OptionType::Call);

        if sigma == 0.0 {
            let price = deterministic_price(
                spot,
                inst.strike,
                r,
                q,
                t,
                is_call,
                matches!(inst.exercise_style, ExerciseStyle::American),
            );
            return Money::new(price * inst.quantity, ccy).map_err(|error| {
                PricingError::from_core(
                    error,
                    PricingErrorContext::from_instrument(inst).model(ModelKey::PdeCrankNicolson1D),
                )
            });
        }

        let pde = BlackScholesPde {
            sigma,
            rate: r,
            dividend: q,
            strike: inst.strike,
            maturity: t,
            is_call,
        };

        // Grid: span both ln(spot) and ln(strike), including deterministic
        // drift as well as a 5σ√t margin even when volatility is very small,
        // concentrated near the strike (payoff kink).
        let spread = 5.0 * sigma * t.sqrt() + ((r - q) * t).abs();
        let ln_spot = spot.ln();
        let ln_strike = inst.strike.ln();
        let x_min = ln_spot.min(ln_strike) - spread;
        let x_max = ln_spot.max(ln_strike) + spread;

        let grid = Grid1D::sinh_concentrated(x_min, x_max, self.space_points, ln_strike, 0.1)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::from_instrument(inst).model(ModelKey::PdeCrankNicolson1D),
                )
            })?;

        // Build solver, optionally with American exercise
        let solver = match inst.exercise_style {
            ExerciseStyle::American => {
                // Interior payoff values for early exercise penalty
                let payoff_values: Vec<f64> = grid.points()[1..grid.n() - 1]
                    .iter()
                    .map(|&x| {
                        let s = x.exp();
                        if is_call {
                            (s - inst.strike).max(0.0)
                        } else {
                            (inst.strike - s).max(0.0)
                        }
                    })
                    .collect();

                // Use Rannacher smoothing (4 initial implicit steps, then CN)
                // to eliminate oscillations near the early exercise boundary
                // caused by the payoff discontinuity (Rannacher 1984).
                Solver1D::builder()
                    .grid(grid)
                    .rannacher(4, self.time_steps)
                    .american(payoff_values)
                    .build()
            }
            // European: Rannacher start-up (4 fully-implicit steps, then CN)
            // damps the high-frequency error component injected by the payoff
            // kink at the strike (Rannacher 1984; Giles & Carter 2006), which
            // plain CN propagates undamped. Measured at the production
            // defaults (200 sinh points, 100 steps) the effect on PV and
            // bump-and-revalue gamma is small, but the damped scheme is the
            // configuration the American path and the tight-tolerance
            // reference tests already use, at negligible cost.
            _ => Solver1D::builder()
                .grid(grid)
                .rannacher(4, self.time_steps)
                .build(),
        }
        .map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(inst).model(ModelKey::PdeCrankNicolson1D),
            )
        })?;

        let solution = solver.solve(&pde, t).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(inst).model(ModelKey::PdeCrankNicolson1D),
            )
        })?;
        let price = solution.interpolate(spot.ln());

        Money::new(price * inst.quantity, ccy).map_err(|error| {
            crate::pricer::PricingError::from_core(
                error,
                crate::pricer::PricingErrorContext::from_instrument(inst)
                    .model(ModelKey::PdeCrankNicolson1D),
            )
        })
    }
}

/// Exact discounted payoff when spot follows deterministic risk-neutral carry.
/// For American exercise the maximum occurs at an endpoint or at the single
/// stationary point of S exp(-q t) - K exp(-r t).
fn deterministic_price(
    spot: f64,
    strike: f64,
    rate: f64,
    dividend: f64,
    maturity: f64,
    is_call: bool,
    american: bool,
) -> f64 {
    let direction = if is_call { 1.0 } else { -1.0 };
    let exercise_value = |time: f64| {
        (direction * (spot * (-dividend * time).exp() - strike * (-rate * time).exp())).max(0.0)
    };
    let mut price = exercise_value(maturity);
    if american {
        price = price.max(exercise_value(0.0));
        let rate_spread = rate - dividend;
        if dividend != 0.0 && rate_spread.abs() > 0.0 {
            let ratio = rate * strike / (dividend * spot);
            if ratio > 0.0 {
                let stationary_time = ratio.ln() / rate_spread;
                if stationary_time > 0.0 && stationary_time < maturity {
                    price = price.max(exercise_value(stationary_time));
                }
            }
        }
    }
    price
}

impl Pricer for EquityOptionPdePricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::EquityOption, ModelKey::PdeCrankNicolson1D)
    }

    #[tracing::instrument(
        name = "equity_option.pde1d.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let equity_option = expect_inst::<EquityOption>(instrument, InstrumentType::EquityOption)?;

        let pv = self.price_internal(equity_option, market, as_of)?;

        Ok(ValuationResult::stamped(equity_option.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use time::{macros::date, Duration};

    #[test]
    fn low_volatility_pde_mesh_refinement_converges_to_vanilla_limit() {
        let as_of = date!(2025 - 01 - 06);
        let mut option = EquityOption::example().unwrap();
        option.expiry = as_of + Duration::days(365);
        option.strike = 100.0;
        option.quantity = 1.0;
        option.div_yield_id = None;
        let curve = DiscountCurve::builder(option.discount_curve_id.clone())
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.05_f64).exp())])
            .build()
            .unwrap();
        let market = MarketContext::new()
            .insert(curve)
            .insert_price(option.spot_id.as_str(), MarketScalar::Unitless(100.0));
        for volatility in [0.04, 0.02, 0.01] {
            option.instrument_pricing_overrides = option
                .instrument_pricing_overrides
                .with_implied_volatility(volatility);
            let reference = finstack_quant_models::closed_form::bs_price(
                100.0,
                100.0,
                0.05,
                0.0,
                volatility,
                1.0,
                OptionType::Call,
            )
            .unwrap();
            let coarse = EquityOptionPdePricer {
                space_points: 100,
                time_steps: 100,
            }
            .price_internal(&option, &market, as_of)
            .unwrap()
            .amount();
            let fine = EquityOptionPdePricer {
                space_points: 400,
                time_steps: 400,
            }
            .price_internal(&option, &market, as_of)
            .unwrap()
            .amount();
            let coarse_error = (coarse - reference).abs();
            let fine_error = (fine - reference).abs();
            assert!(
                fine_error < coarse_error,
                "vol={volatility}: coarse={coarse_error}, fine={fine_error}"
            );
            assert!(fine_error < 5e-4, "vol={volatility}: error={fine_error}");
        }
    }
}

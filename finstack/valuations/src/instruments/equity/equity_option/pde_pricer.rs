//! Equity option PDE pricer using 1D Crank-Nicolson finite differences.
//!
//! Solves the Black-Scholes PDE in log-spot coordinates on a sinh-concentrated
//! grid. Supports both European and American exercise via the penalty method.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::equity_option::pricer::collect_inputs_extended;
use crate::instruments::equity::equity_option::types::EquityOption;
use crate::instruments::ExerciseStyle;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, PricingResult,
};
use crate::results::ValuationResult;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

use crate::instruments::common_impl::models::pde::{BlackScholesPde, Grid1D, Solver1D};
use crate::instruments::common_impl::parameters::OptionType;

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
    ) -> Result<Money, PricingError> {
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
        let ccy = inst.notional.currency();

        if t <= 0.0 {
            let intrinsic = match inst.option_type {
                OptionType::Call => (spot - inst.strike).max(0.0),
                OptionType::Put => (inst.strike - spot).max(0.0),
            };
            return Ok(Money::new(intrinsic * inst.notional.amount(), ccy));
        }

        let is_call = matches!(inst.option_type, OptionType::Call);

        let pde = BlackScholesPde {
            sigma,
            rate: r,
            dividend: q,
            strike: inst.strike,
            maturity: t,
            is_call,
        };

        // Grid: span both ln(spot) and ln(strike) with margin of 5σ√t,
        // concentrated near the strike (payoff kink).
        let spread = 5.0 * sigma * t.sqrt();
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
            _ => Solver1D::builder()
                .grid(grid)
                .crank_nicolson(self.time_steps)
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

        Ok(Money::new(price * inst.notional.amount(), ccy))
    }
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
    ) -> PricingResult<ValuationResult> {
        let equity_option = instrument
            .as_any()
            .downcast_ref::<EquityOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::EquityOption, instrument.key())
            })?;

        let pv = self.price_internal(equity_option, market, as_of)?;

        Ok(ValuationResult::stamped(equity_option.id(), as_of, pv))
    }
}

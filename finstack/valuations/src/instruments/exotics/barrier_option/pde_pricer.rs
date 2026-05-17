//! Barrier option PDE pricer using 1D Crank-Nicolson finite differences.
//!
//! Implements barrier enforcement via Dirichlet boundary conditions at the
//! barrier level. Knock-out options are priced directly; knock-in options
//! use the parity relationship: knock_in = vanilla - knock_out.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::exotics::barrier_option::types::{BarrierOption, BarrierType};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, PricingResult,
};
use crate::results::ValuationResult;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

use crate::instruments::common_impl::models::pde::{
    BoundaryCondition, Grid1D, PdeProblem1D, Solver1D,
};

/// Black-Scholes PDE with barrier enforcement via boundary conditions.
///
/// For knock-out barriers, the option value is forced to zero at the barrier
/// level using a Dirichlet(0) boundary condition. The grid domain is truncated
/// at the barrier so the barrier coincides with a grid boundary.
struct BarrierPde {
    /// Volatility (annualized, decimal).
    sigma: f64,
    /// Risk-free rate (continuous, decimal).
    rate: f64,
    /// Continuous dividend yield (decimal).
    dividend: f64,
    /// Strike price.
    strike: f64,
    /// True for call, false for put.
    is_call: bool,
    /// True if the barrier is at the upper boundary.
    barrier_is_upper: bool,
}

impl PdeProblem1D for BarrierPde {
    fn diffusion(&self, _x: f64, _t: f64) -> f64 {
        0.5 * self.sigma * self.sigma
    }

    fn convection(&self, _x: f64, _t: f64) -> f64 {
        self.rate - self.dividend - 0.5 * self.sigma * self.sigma
    }

    fn reaction(&self, _x: f64, _t: f64) -> f64 {
        -self.rate
    }

    fn terminal_condition(&self, x: f64) -> f64 {
        let s = x.exp();
        if self.is_call {
            (s - self.strike).max(0.0)
        } else {
            (self.strike - s).max(0.0)
        }
    }

    fn lower_boundary(&self, _t: f64) -> BoundaryCondition {
        if !self.barrier_is_upper {
            // Barrier at lower boundary: knock-out => Dirichlet(0)
            BoundaryCondition::Dirichlet(0.0)
        } else if self.is_call {
            // No barrier here, deep OTM call
            BoundaryCondition::Dirichlet(0.0)
        } else {
            // No barrier here, deep ITM put
            BoundaryCondition::Linear
        }
    }

    fn upper_boundary(&self, _t: f64) -> BoundaryCondition {
        if self.barrier_is_upper {
            // Barrier at upper boundary: knock-out => Dirichlet(0)
            BoundaryCondition::Dirichlet(0.0)
        } else if self.is_call {
            // No barrier here, deep ITM call
            BoundaryCondition::Linear
        } else {
            // No barrier here, deep OTM put
            BoundaryCondition::Dirichlet(0.0)
        }
    }

    fn is_time_homogeneous(&self) -> bool {
        true
    }
}

/// Barrier option pricer using 1D PDE (Crank-Nicolson) with barrier enforcement.
///
/// European exercise only. Knock-out barriers are enforced via Dirichlet(0)
/// boundary conditions at the barrier level. Knock-in options are computed
/// via parity: KI = Vanilla - KO.
pub(crate) struct BarrierOptionPdePricer {
    /// Number of spatial grid points.
    space_points: usize,
    /// Number of time steps.
    time_steps: usize,
}

struct KnockOutPdeInputs {
    spot: f64,
    strike: f64,
    barrier: f64,
    rate: f64,
    dividend: f64,
    sigma: f64,
    maturity: f64,
    is_call: bool,
    barrier_is_upper: bool,
}

struct VanillaPdeInputs {
    spot: f64,
    strike: f64,
    rate: f64,
    dividend: f64,
    sigma: f64,
    maturity: f64,
    is_call: bool,
}

impl Default for BarrierOptionPdePricer {
    fn default() -> Self {
        Self {
            space_points: 200,
            time_steps: 100,
        }
    }
}

impl BarrierOptionPdePricer {
    /// Price a barrier option via the 1D PDE with barrier enforcement.
    fn price_internal(
        &self,
        inst: &BarrierOption,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money, PricingError> {
        let bs_inputs = crate::instruments::common_impl::helpers::collect_black_scholes_inputs_df(
            &inst.spot_id,
            &inst.discount_curve_id,
            inst.div_yield_id.as_ref(),
            &inst.vol_surface_id,
            inst.strike,
            inst.expiry,
            inst.day_count,
            market,
            as_of,
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let spot = bs_inputs.spot;
        let df = bs_inputs.df;
        let q = bs_inputs.q;
        let sigma = bs_inputs.sigma;
        let t = bs_inputs.t;
        let ccy = inst.notional.currency();

        if t <= 0.0 {
            // Delegate to the expired barrier handler via the standard pricer path
            return Err(PricingError::model_failure_with_context(
                "Barrier option is expired; use the analytical pricer for expired barriers"
                    .to_string(),
                PricingErrorContext::default(),
            ));
        }

        // Derive rate from DF
        let r = if t > 0.0 && df > 0.0 {
            -df.ln() / t
        } else {
            0.0
        };

        let barrier_level = inst.barrier.amount();
        let is_call = matches!(inst.option_type, crate::instruments::OptionType::Call);
        let is_knock_out = matches!(
            inst.barrier_type,
            BarrierType::UpAndOut | BarrierType::DownAndOut
        );
        let barrier_is_upper = matches!(
            inst.barrier_type,
            BarrierType::UpAndOut | BarrierType::UpAndIn
        );

        // Compute knock-out price (knock-in will use parity)
        let ko_price = self.price_knock_out(KnockOutPdeInputs {
            spot,
            strike: inst.strike,
            barrier: barrier_level,
            rate: r,
            dividend: q,
            sigma,
            maturity: t,
            is_call,
            barrier_is_upper,
        })?;

        let unit_price = if is_knock_out {
            ko_price
        } else {
            // Knock-in = Vanilla - Knock-out
            let vanilla_price = self.price_vanilla(VanillaPdeInputs {
                spot,
                strike: inst.strike,
                rate: r,
                dividend: q,
                sigma,
                maturity: t,
                is_call,
            })?;
            // Exact parity (Merton 1973) — do not clamp to zero; a small
            // negative value indicates numerical noise, not a real price.
            vanilla_price - ko_price
        };

        Ok(Money::new(unit_price * inst.notional.amount(), ccy))
    }

    /// Price a knock-out barrier option via PDE.
    fn price_knock_out(&self, inputs: KnockOutPdeInputs) -> Result<f64, PricingError> {
        let ln_barrier = inputs.barrier.ln();
        let ln_spot = inputs.spot.ln();
        let spread = 5.0 * inputs.sigma * inputs.maturity.sqrt();

        // Set grid bounds so the barrier is at one edge
        let (x_min, x_max) = if inputs.barrier_is_upper {
            // Barrier at upper end; lower end extends below spot
            let lower = (ln_spot - spread).min(ln_barrier - spread);
            (lower, ln_barrier)
        } else {
            // Barrier at lower end; upper end extends above spot
            let upper = (ln_spot + spread).max(ln_barrier + spread);
            (ln_barrier, upper)
        };

        // Concentrate grid near the strike
        let center = inputs.strike.ln();
        let grid = Grid1D::sinh_concentrated(x_min, x_max, self.space_points, center, 0.1)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let pde = BarrierPde {
            sigma: inputs.sigma,
            rate: inputs.rate,
            dividend: inputs.dividend,
            strike: inputs.strike,
            is_call: inputs.is_call,
            barrier_is_upper: inputs.barrier_is_upper,
        };

        let solver = Solver1D::builder()
            .grid(grid)
            .crank_nicolson(self.time_steps)
            .build()
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let solution = solver.solve(&pde, inputs.maturity).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        Ok(solution.interpolate(ln_spot))
    }

    /// Price a vanilla option via PDE (for knock-in parity).
    fn price_vanilla(&self, inputs: VanillaPdeInputs) -> Result<f64, PricingError> {
        use crate::instruments::common_impl::models::pde::BlackScholesPde;

        let pde = BlackScholesPde {
            sigma: inputs.sigma,
            rate: inputs.rate,
            dividend: inputs.dividend,
            strike: inputs.strike,
            maturity: inputs.maturity,
            is_call: inputs.is_call,
        };

        let spread = 5.0 * inputs.sigma * inputs.maturity.sqrt();
        let ln_spot = inputs.spot.ln();
        let ln_strike = inputs.strike.ln();
        let x_min = ln_spot.min(ln_strike) - spread;
        let x_max = ln_spot.max(ln_strike) + spread;

        let grid = Grid1D::sinh_concentrated(x_min, x_max, self.space_points, ln_strike, 0.1)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let solver = Solver1D::builder()
            .grid(grid)
            .crank_nicolson(self.time_steps)
            .build()
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let solution = solver.solve(&pde, inputs.maturity).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        Ok(solution.interpolate(inputs.spot.ln()))
    }
}

impl Pricer for BarrierOptionPdePricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::BarrierOption, ModelKey::PdeCrankNicolson1D)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> PricingResult<ValuationResult> {
        let barrier_opt = instrument
            .as_any()
            .downcast_ref::<BarrierOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::BarrierOption, instrument.key())
            })?;

        let pv = self.price_internal(barrier_opt, market, as_of)?;

        Ok(ValuationResult::stamped(barrier_opt.id(), as_of, pv))
    }
}

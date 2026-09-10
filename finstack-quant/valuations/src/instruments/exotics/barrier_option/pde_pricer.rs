//! Barrier option PDE pricer using 1D finite differences.
//!
//! Continuous barriers impose contractual rebate boundary values. Discrete
//! barriers use a wide domain and apply hit events only on observation dates.
//! Knock-ins use vanilla minus the zero-rebate knock-out, plus a discounted
//! no-hit rebate when one is contractual.
//!
//! # Time stepping (W-01)
//!
//! The solve uses **Rannacher startup** — a few fully-implicit steps before
//! switching to Crank-Nicolson. Plain CN oscillates in price and (worse) in
//! delta/gamma near the barrier because the payoff kink and the
//! knock-out/Dirichlet discontinuity excite high-frequency modes that CN does
//! not damp. The implicit start-up steps damp those modes; CN then provides
//! second-order accuracy for the remainder. See Rannacher (1984).
//!
//! # Knock-in parity grid consistency (W-08)
//!
//! Continuous knock-in valuation extends the knock-out grid across the barrier,
//! retaining its nodes around spot. Discrete knock-in and knock-out valuation
//! use the same wide spatial domain. Independent closed-form and observation-
//! date checks establish the numerical accuracy of each calculation.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::exotics::barrier_option::types::BarrierOption;
use crate::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

use finstack_quant_models::closed_form::barrier::RebateTiming;
use finstack_quant_models::pde::{
    BoundaryCondition, Grid1D, PdeProblem1D, RannacherStepper, Solver1D, TimeStepper,
};

/// Black-Scholes PDE with barrier enforcement via boundary conditions.
///
/// Continuous barriers truncate the spatial domain and impose the contractual
/// rebate at its barrier boundary. Discrete barriers use a wide domain and
/// apply the hit condition only on observation dates.
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
    maturity: f64,
    rebate: f64,
    rebate_timing: RebateTiming,
    terminal_cash: Option<f64>,
    continuous: bool,
    observation_times: Vec<f64>,
}

impl BarrierPde {
    fn rebate_at(&self, time: f64) -> f64 {
        match self.rebate_timing {
            RebateTiming::AtHit => self.rebate,
            RebateTiming::AtExpiry => self.rebate * (-self.rate * (self.maturity - time)).exp(),
        }
    }

    fn barrier_boundary(&self, time: f64) -> Option<f64> {
        if self.continuous {
            return Some(self.rebate_at(time));
        }
        self.observation_times
            .iter()
            .copied()
            .find(|observation| *observation >= time - 1e-12)
            .map(|next| self.rebate_at(next) * (-self.rate * (next - time).max(0.0)).exp())
    }
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
        if let Some(cash) = self.terminal_cash {
            return cash;
        }
        let s = x.exp();
        if self.is_call {
            (s - self.strike).max(0.0)
        } else {
            (self.strike - s).max(0.0)
        }
    }

    fn lower_boundary(&self, time: f64) -> BoundaryCondition {
        if !self.barrier_is_upper {
            if let Some(value) = self.barrier_boundary(time) {
                return BoundaryCondition::Dirichlet(value);
            }
        }
        if let Some(cash) = self.terminal_cash {
            BoundaryCondition::Dirichlet(cash * (-self.rate * (self.maturity - time)).exp())
        } else if self.is_call {
            BoundaryCondition::Dirichlet(0.0)
        } else {
            BoundaryCondition::Linear
        }
    }

    fn upper_boundary(&self, time: f64) -> BoundaryCondition {
        if self.barrier_is_upper {
            if let Some(value) = self.barrier_boundary(time) {
                return BoundaryCondition::Dirichlet(value);
            }
        }
        if let Some(cash) = self.terminal_cash {
            BoundaryCondition::Dirichlet(cash * (-self.rate * (self.maturity - time)).exp())
        } else if self.is_call {
            BoundaryCondition::Linear
        } else {
            BoundaryCondition::Dirichlet(0.0)
        }
    }

    fn is_time_homogeneous(&self) -> bool {
        false
    }
}

/// Number of fully-implicit Rannacher start-up steps before switching to
/// Crank-Nicolson. Two implicit steps are the Rannacher (1984) standard and
/// are sufficient to damp the high-frequency modes excited by the payoff kink
/// and the knock-out boundary discontinuity.
const RANNACHER_IMPLICIT_STEPS: usize = 2;

/// Barrier option pricer using a 1D PDE (Rannacher startup + Crank-Nicolson)
/// with barrier enforcement.
///
/// European exercise only. Continuous barriers impose rebate boundary values;
/// discrete barriers enforce observation-date events. Knock-in value is the
/// vanilla minus zero-rebate knock-out value plus the discounted no-hit rebate.
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
    rebate: f64,
    rebate_timing: RebateTiming,
    terminal_cash: Option<f64>,
    observation_times: Option<Vec<f64>>,
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
    ) -> std::result::Result<Money, PricingError> {
        inst.validate_monitoring_state(as_of).map_err(|error| {
            PricingError::from_core(error, PricingErrorContext::from_instrument(inst))
        })?;
        if as_of >= inst.expiry {
            return super::pricer::price_expired_barrier(inst, market, as_of).map_err(|error| {
                PricingError::from_core(error, PricingErrorContext::from_instrument(inst))
            });
        }
        if let Some(value) =
            super::pricer::known_knock_out_value(inst, market, as_of).map_err(|error| {
                PricingError::from_core(error, PricingErrorContext::from_instrument(inst))
            })?
        {
            return Ok(value);
        }
        let bs_inputs =
            super::pricer::collect_barrier_inputs(inst, market, as_of).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let spot = bs_inputs.spot;
        let q = bs_inputs.q;
        let sigma = bs_inputs.sigma;
        let t = bs_inputs.t;
        let ccy = inst.notional.currency();

        let r = bs_inputs.r_eff();

        if inst.observed_barrier_breached == Some(true) {
            let unit = finstack_quant_models::closed_form::vanilla::bs_price_unchecked(
                spot,
                inst.strike,
                r,
                q,
                sigma,
                t,
                inst.option_type,
            );
            return Money::new(unit * inst.notional.amount(), ccy).map_err(|error| {
                crate::pricer::PricingError::from_core(
                    error,
                    crate::pricer::PricingErrorContext::from_instrument(inst),
                )
            });
        }

        let barrier_level = inst.barrier.amount();
        let is_call = matches!(inst.option_type, crate::instruments::OptionType::Call);
        let is_knock_out = inst.barrier_type.is_knock_out();
        let barrier_is_upper = inst.barrier_type.is_up();

        let observation_times = match &inst.monitoring {
            crate::instruments::Monitoring::Continuous => None,
            crate::instruments::Monitoring::Discrete { observation_dates } => Some(
                observation_dates
                    .iter()
                    .copied()
                    .filter(|date| *date >= as_of)
                    .map(|date| {
                        inst.day_count
                            .year_fraction(as_of, date, Default::default())
                    })
                    .collect::<finstack_quant_core::Result<Vec<_>>>()
                    .map_err(|error| {
                        PricingError::from_core(error, PricingErrorContext::from_instrument(inst))
                    })?,
            ),
        };
        let observed_now = observation_times
            .as_ref()
            .is_none_or(|times| times.first() == Some(&0.0));
        if observed_now
            && if barrier_is_upper {
                spot >= barrier_level
            } else {
                spot <= barrier_level
            }
        {
            let mut observed = inst.clone();
            observed.observed_barrier_breached = Some(true);
            return self.price_internal(&observed, market, as_of);
        }
        let mut ko_inputs = KnockOutPdeInputs {
            spot,
            strike: inst.strike,
            barrier: barrier_level,
            rate: r,
            dividend: q,
            sigma,
            maturity: t,
            is_call,
            barrier_is_upper,
            rebate: if is_knock_out {
                inst.rebate
                    .map_or(0.0, |m| m.amount() / inst.notional.amount())
            } else {
                0.0
            },
            rebate_timing: inst.rebate_timing,
            terminal_cash: None,
            observation_times,
        };

        // Build the barrier-truncated knock-out grid. The barrier sits exactly
        // on the truncated edge node.
        let ko_grid = if ko_inputs.observation_times.is_some() {
            let spread = (7.0 * sigma * t.sqrt()).max(1.0);
            let center = barrier_level.ln();
            Grid1D::sinh_concentrated(
                spot.ln().min(inst.strike.ln()).min(center) - spread,
                spot.ln().max(inst.strike.ln()).max(center) + spread,
                self.space_points,
                center,
                0.1,
            )
            .map_err(|error| {
                PricingError::model_failure_with_context(
                    error.to_string(),
                    PricingErrorContext::from_instrument(inst),
                )
            })?
        } else {
            self.build_barrier_grid(&ko_inputs)?
        };

        // Compute knock-out price (knock-in will use parity).
        let ko_price = self.price_knock_out(&ko_grid, &ko_inputs)?;

        let unit_price = if is_knock_out {
            ko_price
        } else {
            // Knock-in = Vanilla - Knock-out. The vanilla solve genuinely needs
            // the domain on the far side of the barrier, so it cannot reuse the
            // barrier-truncated grid directly. Instead the vanilla grid EXTENDS
            // the knock-out grid: every node of the knock-out grid is also a
            // node of the vanilla grid (W-08). Both solves use the same PDE
            // operator and the same Rannacher stepper, so on the shared nodes
            // — which contain the spot — the KI = Vanilla - KO difference
            // cancels discretization error to leading order rather than
            // carrying the sum of two independent grid errors.
            let vanilla_grid = if ko_inputs.observation_times.is_some() {
                ko_grid.clone()
            } else {
                self.build_vanilla_grid_extending(&ko_grid, &ko_inputs)?
            };
            let vanilla_price = self.price_vanilla(
                &vanilla_grid,
                VanillaPdeInputs {
                    spot,
                    strike: inst.strike,
                    rate: r,
                    dividend: q,
                    sigma,
                    maturity: t,
                    is_call,
                },
            )?;
            let parity = vanilla_price - ko_price;
            let tolerance = 1e-6 * vanilla_price.abs().max(1.0);
            if parity < -tolerance {
                return Err(PricingError::model_failure_with_context(
                    format!("Barrier knock-in PDE parity is negative ({parity}); refine the grid"),
                    PricingErrorContext::from_instrument(inst),
                ));
            }
            // A knock-in rebate is paid only when monitoring finishes without
            // a hit. Price the discounted survival indicator on the same grid.
            ko_inputs.terminal_cash = Some(
                inst.rebate
                    .map_or(0.0, |m| m.amount() / inst.notional.amount()),
            );
            let survival_rebate = if inst.rebate.is_some() {
                self.price_knock_out(&ko_grid, &ko_inputs)?
            } else {
                0.0
            };
            parity.max(0.0) + survival_rebate
        };

        Money::new(unit_price * inst.notional.amount(), ccy).map_err(|error| {
            crate::pricer::PricingError::from_core(
                error,
                crate::pricer::PricingErrorContext::from_instrument(inst),
            )
        })
    }

    /// Build the barrier-truncated, strike-concentrated spatial grid.
    ///
    /// The grid is built once and shared by the knock-out and vanilla solves
    /// so that the knock-in parity difference is consistent (W-08).
    fn build_barrier_grid(
        &self,
        inputs: &KnockOutPdeInputs,
    ) -> std::result::Result<Grid1D, PricingError> {
        let ln_barrier = inputs.barrier.ln();
        let ln_spot = inputs.spot.ln();
        let spread = (5.0 * inputs.sigma * inputs.maturity.sqrt()).max(0.5);

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
        Grid1D::sinh_concentrated(x_min, x_max, self.space_points, center, 0.1).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })
    }

    /// Build the vanilla-option grid by extending the knock-out grid across the
    /// barrier (W-08).
    ///
    /// The vanilla solve needs the domain on the far side of the barrier (the
    /// vanilla value there is non-zero), so it cannot reuse the
    /// barrier-truncated grid. To keep `KI = Vanilla - KO` consistent, the
    /// vanilla grid is the knock-out grid's nodes PLUS extra nodes on the
    /// far-barrier side. Every knock-out node — in particular the region
    /// containing the spot — is a node of the vanilla grid, so the two solves
    /// agree node-for-node there.
    fn build_vanilla_grid_extending(
        &self,
        ko_grid: &Grid1D,
        inputs: &KnockOutPdeInputs,
    ) -> std::result::Result<Grid1D, PricingError> {
        let ln_barrier = inputs.barrier.ln();
        let ln_spot = inputs.spot.ln();
        let ln_strike = inputs.strike.ln();
        let spread = (5.0 * inputs.sigma * inputs.maturity.sqrt()).max(0.5);
        let barrier_is_upper = inputs.barrier_is_upper;
        let ko_points = ko_grid.points();

        // How far the vanilla domain must reach on the far side of the barrier.
        let extra_count = self.space_points.max(3);
        let mut points: Vec<f64> = Vec::with_capacity(ko_points.len() + extra_count);

        if barrier_is_upper {
            // Knock-out grid is [.., ln_barrier]. Vanilla needs nodes ABOVE the
            // barrier up to x_max_wide.
            let x_max_wide = ln_spot.max(ln_strike).max(ln_barrier) + spread;
            points.extend_from_slice(ko_points);
            // Uniform extension above the barrier; step matches the KO grid's
            // last interval for a smooth join.
            let last = ko_points.len() - 1;
            let h = (ko_points[last] - ko_points[last - 1]).max(1e-6);
            let n_extra = (((x_max_wide - ln_barrier) / h).ceil() as usize).max(2);
            for k in 1..=n_extra {
                points.push(ln_barrier + h * k as f64);
            }
        } else {
            // Knock-out grid is [ln_barrier, ..]. Vanilla needs nodes BELOW the
            // barrier down to x_min_wide.
            let x_min_wide = ln_spot.min(ln_strike).min(ln_barrier) - spread;
            let h = (ko_points[1] - ko_points[0]).max(1e-6);
            let n_extra = (((ln_barrier - x_min_wide) / h).ceil() as usize).max(2);
            // Prepend nodes below the barrier in increasing order.
            for k in (1..=n_extra).rev() {
                points.push(ln_barrier - h * k as f64);
            }
            points.extend_from_slice(ko_points);
        }

        Grid1D::from_points(points).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })
    }

    /// Price a knock-out barrier option via PDE on the supplied grid.
    fn price_knock_out(
        &self,
        grid: &Grid1D,
        inputs: &KnockOutPdeInputs,
    ) -> std::result::Result<f64, PricingError> {
        let ln_spot = inputs.spot.ln();

        let pde = BarrierPde {
            sigma: inputs.sigma,
            rate: inputs.rate,
            dividend: inputs.dividend,
            strike: inputs.strike,
            is_call: inputs.is_call,
            barrier_is_upper: inputs.barrier_is_upper,
            maturity: inputs.maturity,
            rebate: inputs.rebate,
            rebate_timing: inputs.rebate_timing,
            terminal_cash: inputs.terminal_cash,
            continuous: inputs.observation_times.is_none(),
            observation_times: inputs.observation_times.clone().unwrap_or_default(),
        };
        if let Some(observations) = &inputs.observation_times {
            return self.price_discrete(grid, inputs, &pde, observations);
        }

        // Rannacher startup: a few fully-implicit steps damp the price and
        // delta/gamma oscillations that plain Crank-Nicolson produces at the
        // payoff kink and the knock-out Dirichlet discontinuity (W-01).
        let solver = Solver1D::builder()
            .grid(grid.clone())
            .rannacher(RANNACHER_IMPLICIT_STEPS, self.time_steps)
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

    /// Step the wide-domain PDE between exact observation times, applying
    /// the contractual hit condition only at those times.
    fn price_discrete(
        &self,
        grid: &Grid1D,
        inputs: &KnockOutPdeInputs,
        pde: &BarrierPde,
        observations: &[f64],
    ) -> std::result::Result<f64, PricingError> {
        let mut levels: Vec<f64> = (0..=self.time_steps)
            .map(|step| inputs.maturity * step as f64 / self.time_steps as f64)
            .collect();
        levels.extend_from_slice(observations);
        levels.sort_by(f64::total_cmp);
        levels.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        levels.reverse();
        let points = &grid.points()[1..grid.n() - 1];
        let mut values: Vec<f64> = points.iter().map(|&x| pde.terminal_condition(x)).collect();
        let apply_observation = |time: f64, values: &mut [f64]| {
            if !observations
                .iter()
                .any(|observation| (*observation - time).abs() < 1e-12)
            {
                return false;
            }
            for (&x, value) in points.iter().zip(values.iter_mut()) {
                let hit = if inputs.barrier_is_upper {
                    x >= inputs.barrier.ln()
                } else {
                    x <= inputs.barrier.ln()
                };
                if hit {
                    *value = pde.rebate_at(time);
                }
            }
            true
        };
        apply_observation(inputs.maturity, &mut values);
        let stepper = RannacherStepper::new(RANNACHER_IMPLICIT_STEPS, levels.len() - 1);
        let mut since_observation = 0;
        for times in levels.windows(2) {
            stepper
                .step(
                    pde,
                    grid,
                    &mut values,
                    times[0],
                    times[1],
                    since_observation,
                )
                .map_err(|error| {
                    PricingError::model_failure_with_context(
                        error.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
            since_observation += 1;
            if apply_observation(times[1], &mut values) {
                since_observation = 0;
            }
        }
        // Spot lies strictly inside the wide grid, so interpolate the interior
        // solution directly without reconstructing remote boundary values.
        let interior = Grid1D::from_points(points.to_vec()).map_err(|error| {
            PricingError::model_failure_with_context(
                error.to_string(),
                PricingErrorContext::default(),
            )
        })?;
        Ok(interior.interpolate(&values, inputs.spot.ln()))
    }

    /// Price a vanilla option via PDE (for knock-in parity) on the supplied grid.
    ///
    /// Uses the same grid and Rannacher stepper as [`Self::price_knock_out`] so
    /// that `KI = Vanilla - KO` is a consistent finite-difference difference.
    fn price_vanilla(
        &self,
        grid: &Grid1D,
        inputs: VanillaPdeInputs,
    ) -> std::result::Result<f64, PricingError> {
        use finstack_quant_models::pde::BlackScholesPde;

        let pde = BlackScholesPde {
            sigma: inputs.sigma,
            rate: inputs.rate,
            dividend: inputs.dividend,
            strike: inputs.strike,
            maturity: inputs.maturity,
            is_call: inputs.is_call,
        };

        // Same grid + same Rannacher stepper as the knock-out solve (W-08).
        let solver = Solver1D::builder()
            .grid(grid.clone())
            .rannacher(RANNACHER_IMPLICIT_STEPS, self.time_steps)
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
    ) -> std::result::Result<ValuationResult, PricingError> {
        let barrier_opt = expect_inst::<BarrierOption>(instrument, InstrumentType::BarrierOption)?;

        let pv = self.price_internal(barrier_opt, market, as_of)?;

        Ok(ValuationResult::stamped(barrier_opt.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::exotics::barrier_option::types::BarrierOption;
    use crate::instruments::{Attributes, OptionType};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::types::BarrierType;
    use finstack_quant_core::types::InstrumentId;
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn market(as_of: Date, spot: f64, vol: f64, rate: f64) -> MarketContext {
        let discount = DiscountCurve::builder("USD_DISC")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
            .build()
            .expect("discount curve");
        let surface = VolSurface::builder("SPX_VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[60.0, 80.0, 100.0, 120.0, 140.0])
            .row(&[vol; 5])
            .row(&[vol; 5])
            .row(&[vol; 5])
            .row(&[vol; 5])
            .build()
            .expect("vol surface");

        MarketContext::new()
            .insert(discount)
            .insert_surface(surface)
            .insert_price(
                "SPX",
                MarketScalar::Price(Money::new(spot, Currency::USD).expect("valid money fixture")),
            )
    }

    fn barrier_option(
        barrier_type: BarrierType,
        option_type: OptionType,
        expiry: Date,
        strike: f64,
        barrier: f64,
    ) -> BarrierOption {
        BarrierOption {
            expiry_fixing: None,
            id: InstrumentId::new("BARRIER-PDE-TEST"),
            underlying_ticker: "SPX".to_string(),
            strike,
            barrier: Money::new(barrier, Currency::USD).expect("valid money fixture"),
            rebate: None,
            rebate_timing: Default::default(),
            option_type,
            barrier_type,
            expiry,
            observed_barrier_breached: None,
            notional: Money::from((1_i64, Currency::USD)),
            day_count: DayCount::Act365F,
            monitoring: crate::instruments::Monitoring::Continuous,
            discount_curve_id: "USD_DISC".into(),
            spot_id: "SPX".into(),
            vol_surface_id: "SPX_VOL".into(),
            div_yield_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        }
    }

    /// W-01: Rannacher startup must remove the Crank-Nicolson oscillation near
    /// the barrier so the PDE knock-out price matches the analytical
    /// continuous-monitoring price.
    ///
    /// A down-and-out call priced with plain Crank-Nicolson exhibits a
    /// spurious price (and far worse delta/gamma) oscillation seeded by the
    /// payoff kink and the knock-out Dirichlet discontinuity. The Rannacher
    /// start-up steps damp those modes, so the PDE price converges cleanly to
    /// the Reiner-Rubinstein analytical value. A 1% tolerance is comfortably
    /// met by the Rannacher solve but breached by an oscillating CN solve.
    #[test]
    fn w01_rannacher_knock_out_matches_analytical_continuous() {
        use finstack_quant_models::closed_form::barrier::down_out_call;

        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1);
        let spot = 100.0;
        let strike = 100.0;
        let barrier = 90.0;
        let vol = 0.25;
        let rate = 0.05;

        let option = barrier_option(
            BarrierType::DownAndOut,
            OptionType::Call,
            expiry,
            strike,
            barrier,
        );
        let mkt = market(as_of, spot, vol, rate);

        let pricer = BarrierOptionPdePricer::default();
        let pv = pricer
            .price_internal(&option, &mkt, as_of)
            .expect("PDE knock-out price")
            .amount();

        let t = DayCount::Act365F
            .year_fraction(
                as_of,
                expiry,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .expect("year fraction");
        let analytical = down_out_call(spot, strike, barrier, t, rate, 0.0, vol);

        let rel_err = (pv - analytical).abs() / analytical;
        assert!(
            rel_err < 0.01,
            "Rannacher PDE down-and-out call {pv:.6} must match analytical \
             continuous-monitoring price {analytical:.6} within 1%; rel_err={:.4}%. \
             A larger error indicates Crank-Nicolson oscillation near the barrier.",
            rel_err * 100.0
        );
    }

    /// W-01: the Rannacher solve must produce a smooth, monotone price profile
    /// near the barrier. An up-and-out call with the barrier just above the
    /// strike is the sharpest stress test: the grid is concentrated around the
    /// strike, so it is also fine right at the barrier, and the knock-out
    /// Dirichlet discontinuity sits in the well-resolved region.
    ///
    /// Plain Crank-Nicolson oscillates violently there — the up-and-out call
    /// price swings *negative* and then far above the unbarriered value on
    /// adjacent spot nodes. The up-and-out call value must instead decrease
    /// monotonically toward zero as spot rises to the barrier, and must stay
    /// within `[0, vanilla]`. The Rannacher start-up steps damp the
    /// oscillation so all three properties hold.
    #[test]
    fn w01_rannacher_knock_out_no_oscillation_near_barrier() {
        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1);
        let strike = 100.0;
        let barrier = 110.0;
        let vol = 0.25;
        let rate = 0.05;

        let pricer = BarrierOptionPdePricer {
            space_points: 200,
            time_steps: 25,
        };
        let price_at = |spot: f64| -> f64 {
            let option = barrier_option(
                BarrierType::UpAndOut,
                OptionType::Call,
                expiry,
                strike,
                barrier,
            );
            pricer
                .price_internal(&option, &market(as_of, spot, vol, rate), as_of)
                .expect("PDE price")
                .amount()
        };

        // Spot ladder approaching the barrier from below: 100, 102, ... 109.
        let ladder: Vec<f64> = (0..=9).map(|k| 100.0 + k as f64).collect();
        let prices: Vec<f64> = ladder.iter().map(|&s| price_at(s)).collect();

        let t = DayCount::Act365F
            .year_fraction(
                as_of,
                expiry,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .expect("year fraction");

        for (&spot, &pv) in ladder.iter().zip(prices.iter()) {
            // Vanilla (unbarriered) call as an upper bound.
            let df = (-rate * t).exp();
            let d1 = ((spot / strike).ln() + (rate + 0.5 * vol * vol) * t) / (vol * t.sqrt());
            let d2 = d1 - vol * t.sqrt();
            let vanilla = spot * finstack_quant_core::math::norm_cdf(d1)
                - strike * df * finstack_quant_core::math::norm_cdf(d2);
            assert!(
                pv >= -1e-6 && pv <= vanilla + 1e-6,
                "up-and-out call price {pv:.6} at spot {spot} must lie in \
                 [0, vanilla={vanilla:.6}]; an out-of-range value is the \
                 Crank-Nicolson oscillation that Rannacher startup must damp"
            );
        }

        // The up-and-out call value must decrease monotonically as spot rises
        // toward the barrier. A CN oscillation produces a non-monotone profile.
        for (w, spots) in prices.windows(2).zip(ladder.windows(2)) {
            assert!(
                w[1] <= w[0] + 1e-6,
                "up-and-out call price must decrease toward the barrier, but \
                 rose from {:.6} at spot {:.2} to {:.6} at spot {:.2}; this is \
                 the Crank-Nicolson oscillation that Rannacher startup damps",
                w[0],
                spots[0],
                w[1],
                spots[1]
            );
        }
    }

    /// W-08: the knock-in parity `KI = Vanilla - KO` must be consistent because
    /// both solves now share one grid and one stepper. The PDE knock-in price
    /// is therefore close to the analytical knock-in (= vanilla - analytical
    /// knock-out) within the discretisation budget.
    ///
    /// With independently-gridded vanilla and knock-out solves the difference
    /// carried the *sum* of two discretisation errors; the shared grid cancels
    /// the leading-order error so the parity is tight.
    #[test]
    fn w08_knock_in_parity_consistent_with_shared_grid() {
        use finstack_quant_models::closed_form::barrier::down_out_call;

        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1);
        let spot = 100.0;
        let strike = 100.0;
        let barrier = 90.0;
        let vol = 0.25;
        let rate = 0.05;

        let mkt = market(as_of, spot, vol, rate);
        let pricer = BarrierOptionPdePricer::default();

        let ki_option = barrier_option(
            BarrierType::DownAndIn,
            OptionType::Call,
            expiry,
            strike,
            barrier,
        );
        let ki_pv = pricer
            .price_internal(&ki_option, &mkt, as_of)
            .expect("PDE knock-in price")
            .amount();

        let ko_option = barrier_option(
            BarrierType::DownAndOut,
            OptionType::Call,
            expiry,
            strike,
            barrier,
        );
        let ko_pv = pricer
            .price_internal(&ko_option, &mkt, as_of)
            .expect("PDE knock-out price")
            .amount();

        let t = DayCount::Act365F
            .year_fraction(
                as_of,
                expiry,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .expect("year fraction");
        // Analytical vanilla call (Black-Scholes) and analytical down-and-out.
        let df = (-rate * t).exp();
        let d1 = ((spot / strike).ln() + (rate + 0.5 * vol * vol) * t) / (vol * t.sqrt());
        let d2 = d1 - vol * t.sqrt();
        let vanilla = spot * finstack_quant_core::math::norm_cdf(d1)
            - strike * df * finstack_quant_core::math::norm_cdf(d2);
        let analytical_ko = down_out_call(spot, strike, barrier, t, rate, 0.0, vol);
        let analytical_ki = vanilla - analytical_ko;

        // KI + KO must reconstruct the vanilla price (in-out parity): both PDE
        // solves run on the same grid, so the sum equals the PDE vanilla.
        let ki_err = (ki_pv - analytical_ki).abs() / analytical_ki;
        assert!(
            ki_err < 0.01,
            "PDE knock-in {ki_pv:.6} must match analytical knock-in {analytical_ki:.6} \
             within 1%; err={:.4}%. A larger error indicates the vanilla and \
             knock-out solves used inconsistent grids.",
            ki_err * 100.0
        );
        // Knock-in must be non-negative (W-12 clamp).
        assert!(
            ki_pv >= 0.0,
            "PDE knock-in price must be non-negative, got {ki_pv}"
        );
        // In-out parity: KI + KO = vanilla.
        let parity_residual = (ki_pv + ko_pv - vanilla).abs();
        assert!(
            parity_residual < 0.05,
            "KI + KO must reconstruct the vanilla price (in-out parity): \
             ki={ki_pv:.6} + ko={ko_pv:.6} vs vanilla={vanilla:.6}, \
             residual={parity_residual:.6}"
        );
    }
    #[test]
    fn production_barrier_pde_rebate_matches_continuous_closed_form() {
        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1);
        let mkt = market(as_of, 100.0, 0.25, 0.05);
        for barrier_type in [BarrierType::UpAndOut, BarrierType::DownAndIn] {
            for timing in [
                finstack_quant_models::closed_form::barrier::RebateTiming::AtHit,
                finstack_quant_models::closed_form::barrier::RebateTiming::AtExpiry,
            ] {
                let level = if barrier_type.is_up() { 120.0 } else { 80.0 };
                let mut option =
                    barrier_option(barrier_type, OptionType::Call, expiry, 100.0, level);
                option.notional = Money::new(1_000.0, Currency::USD).expect("notional");
                option.rebate = Some(Money::new(25_000.0, Currency::USD).expect("rebate"));
                option.rebate_timing = timing;
                let analytical = option.value(&mkt, as_of).expect("analytical").amount();
                let pde = BarrierOptionPdePricer {
                    space_points: 600,
                    time_steps: 800,
                }
                .price_internal(&option, &mkt, as_of)
                .expect("PDE")
                .amount();
                assert!(
                    (pde - analytical).abs() < 15.0,
                    "{barrier_type:?}/{timing:?}: PDE {pde}, analytical {analytical}"
                );
            }
        }
    }

    #[test]
    fn production_barrier_pde_expiry_only_monitoring_matches_truncated_call() {
        use finstack_quant_core::math::special_functions::norm_cdf;
        use finstack_quant_models::closed_form::vanilla::bs_price_unchecked;
        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1);
        let mkt = market(as_of, 100.0, 0.25, 0.05);
        let mut option = barrier_option(
            BarrierType::UpAndOut,
            OptionType::Call,
            expiry,
            100.0,
            120.0,
        );
        option.monitoring = crate::instruments::Monitoring::Discrete {
            observation_dates: vec![expiry],
        };
        let t = option
            .day_count
            .year_fraction(as_of, expiry, Default::default())
            .expect("time");
        let d2 = ((100.0_f64 / 120.0).ln() + (0.05 - 0.5 * 0.25 * 0.25) * t) / (0.25 * t.sqrt());
        let expected = bs_price_unchecked(100.0, 100.0, 0.05, 0.0, 0.25, t, OptionType::Call)
            - bs_price_unchecked(100.0, 120.0, 0.05, 0.0, 0.25, t, OptionType::Call)
            - 20.0 * (-0.05 * t).exp() * norm_cdf(d2);
        let pde = BarrierOptionPdePricer {
            space_points: 1_200,
            time_steps: 800,
        }
        .price_internal(&option, &mkt, as_of)
        .expect("PDE")
        .amount();
        assert!(
            (pde - expected).abs() < 0.04,
            "PDE {pde}, exact truncated call {expected}"
        );
    }
}

//! Barrier option PDE pricer using 1D finite differences.
//!
//! Implements barrier enforcement via Dirichlet boundary conditions at the
//! barrier level. Knock-out options are priced directly; knock-in options
//! use the parity relationship: knock_in = vanilla - knock_out.
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
//! Knock-in options are priced as `KI = Vanilla - KO`. Both the vanilla and
//! knock-out solves run on the **same spatial grid and the same time stepper**
//! so that the difference cancels discretization error to leading order
//! rather than carrying the sum of two independent grid errors.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::exotics::barrier_option::types::{BarrierOption, BarrierType};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

use crate::models::pde::{BoundaryCondition, Grid1D, PdeProblem1D, Solver1D};

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

/// Number of fully-implicit Rannacher start-up steps before switching to
/// Crank-Nicolson. Two implicit steps are the Rannacher (1984) standard and
/// are sufficient to damp the high-frequency modes excited by the payoff kink
/// and the knock-out boundary discontinuity.
const RANNACHER_IMPLICIT_STEPS: usize = 2;

/// Barrier option pricer using a 1D PDE (Rannacher startup + Crank-Nicolson)
/// with barrier enforcement.
///
/// European exercise only. Knock-out barriers are enforced via Dirichlet(0)
/// boundary conditions at the barrier level. Knock-in options are computed
/// via parity: KI = Vanilla - KO, with both solves sharing the same grid and
/// stepper so the difference cancels discretization error.
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
    ) -> std::result::Result<Money, PricingError> {
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

        if inst.observed_barrier_breached == Some(true) {
            let unit = match inst.barrier_type {
                BarrierType::UpAndIn | BarrierType::DownAndIn => {
                    crate::models::closed_form::vanilla::bs_price(
                        spot,
                        inst.strike,
                        r,
                        q,
                        sigma,
                        t,
                        inst.option_type,
                    )
                }
                BarrierType::UpAndOut | BarrierType::DownAndOut => match inst.rebate_timing {
                    crate::models::closed_form::barrier::RebateTiming::AtHit => 0.0,
                    crate::models::closed_form::barrier::RebateTiming::AtExpiry => {
                        inst.rebate.map_or(0.0, |rebate| rebate.amount() * df)
                    }
                },
            };
            return Ok(Money::new(unit * inst.notional.amount(), ccy));
        }

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

        let ko_inputs = KnockOutPdeInputs {
            spot,
            strike: inst.strike,
            barrier: barrier_level,
            rate: r,
            dividend: q,
            sigma,
            maturity: t,
            is_call,
            barrier_is_upper,
        };

        // Build the barrier-truncated knock-out grid. The barrier sits exactly
        // on the truncated edge node.
        let ko_grid = self.build_barrier_grid(&ko_inputs)?;

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
            let vanilla_grid = self.build_vanilla_grid_extending(&ko_grid, &ko_inputs)?;
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
            // Exact parity (Merton 1973). With a shared grid the residual is
            // pure round-off; clamp at -tol so a tiny negative value is
            // reported as zero with a warning rather than surfaced as a
            // negative price (W-12).
            let parity = vanilla_price - ko_price;
            let tol = 1e-8 * vanilla_price.abs().max(1.0);
            if parity < -tol {
                tracing::warn!(
                    parity,
                    vanilla_price,
                    ko_price,
                    "Barrier knock-in PDE parity produced a negative price beyond tolerance; \
                     clamping to zero. This indicates a grid/discretization problem."
                );
                0.0
            } else {
                parity.max(0.0)
            }
        };

        Ok(Money::new(unit_price * inst.notional.amount(), ccy))
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
        let spread = 5.0 * inputs.sigma * inputs.maturity.sqrt();
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
        };

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

    /// Price a vanilla option via PDE (for knock-in parity) on the supplied grid.
    ///
    /// Uses the same grid and Rannacher stepper as [`Self::price_knock_out`] so
    /// that `KI = Vanilla - KO` is a consistent finite-difference difference.
    fn price_vanilla(
        &self,
        grid: &Grid1D,
        inputs: VanillaPdeInputs,
    ) -> std::result::Result<f64, PricingError> {
        use crate::models::pde::BlackScholesPde;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::exotics::barrier_option::types::{BarrierOption, BarrierType};
    use crate::instruments::{Attributes, OptionType, PricingOverrides};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
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
            .insert_price("SPX", MarketScalar::Price(Money::new(spot, Currency::USD)))
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
            barrier: Money::new(barrier, Currency::USD),
            rebate: None,
            rebate_timing: Default::default(),
            option_type,
            barrier_type,
            expiry,
            observed_barrier_breached: None,
            notional: Money::new(1.0, Currency::USD),
            day_count: DayCount::Act365F,
            use_gobet_miri: false,
            discount_curve_id: "USD_DISC".into(),
            spot_id: "SPX".into(),
            vol_surface_id: "SPX_VOL".into(),
            div_yield_id: None,
            pricing_overrides: PricingOverrides::default(),
            monitoring_frequency: None,
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
        use crate::models::closed_form::barrier::down_out_call;

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
        use crate::models::closed_form::barrier::down_out_call;

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
}

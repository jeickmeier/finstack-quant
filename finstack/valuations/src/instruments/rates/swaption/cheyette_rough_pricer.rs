//! Bermudan swaption pricer using Cheyette + rough stochastic volatility.
//!
//! Prices Bermudan swaptions under the Cheyette (1-factor Markovian HJM)
//! model with rough stochastic volatility driven by a Volterra fractional
//! Brownian motion.  The short rate is reconstructed as r(t) = x(t) + phi(t)
//! where phi(t) is the initial forward rate curve.
//!
//! For Bermudan exercise, this implementation uses LSMC (Longstaff-Schwartz)
//! backward induction with regression on the Cheyette state variables [x, y].
//!
//! # References
//!
//! - Cheyette, O. (1994). "Markov Representation of the Heath-Jarrow-Morton Model."
//! - Bayer, C., Friz, P. & Gatheral, J. (2016). "Pricing under rough volatility."

use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::swaption::BermudanSwaption;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::ForwardVarianceCurve;
use finstack_core::market_data::traits::Discounting;
use finstack_core::math::fractional::HurstExponent;
use finstack_core::money::Money;
use finstack_monte_carlo::discretization::cheyette_rough::CheyetteRoughEuler;
use finstack_monte_carlo::online_stats::OnlineStats;
use finstack_monte_carlo::pricer::lsq::solve_least_squares;
use finstack_monte_carlo::process::cheyette_rough::{
    CheyetteRoughVolParams, CheyetteRoughVolProcess,
};
use finstack_monte_carlo::rng::fbm::create_fbm_generator;
use finstack_monte_carlo::rng::philox::PhiloxRng;
use finstack_monte_carlo::time_grid::TimeGrid;
use finstack_monte_carlo::traits::{Discretization, RandomStream};

/// Configuration for the Cheyette rough vol Bermudan swaption pricer.
#[derive(Debug, Clone)]
pub struct CheyetteRoughConfig {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Number of simulation time steps.
    pub num_steps: usize,
    /// Polynomial degree for LSMC regression basis.
    pub basis_degree: usize,
}

impl Default for CheyetteRoughConfig {
    fn default() -> Self {
        let defaults = &finstack_monte_carlo::registry::embedded_defaults_or_panic()
            .rust
            .cheyette_rough;
        Self {
            num_paths: defaults.num_paths,
            num_steps: defaults.num_steps,
            basis_degree: defaults.basis_degree,
        }
    }
}

/// Bermudan swaption pricer using Cheyette + rough stochastic volatility.
///
/// Uses Monte Carlo simulation with the Cheyette rough vol process and
/// LSMC backward induction for optimal exercise decisions.  The Cheyette
/// state variables [x, y] are used as regression basis for the continuation
/// value estimate.
///
/// # Default Parameters
///
/// The default Cheyette parameters (kappa=0.03, eta=1.5, H=0.1, rho=-0.5)
/// are generic starting values.  For production use, these should be
/// calibrated to the swaption volatility surface.
#[derive(Default)]
pub struct BermudanSwaptionCheyetteRoughPricer {
    config: CheyetteRoughConfig,
}

struct SwapValueInputs {
    exercise_time: f64,
    swap_end_time: f64,
    period: f64,
    strike: f64,
    is_payer: bool,
    notional: f64,
}

impl BermudanSwaptionCheyetteRoughPricer {
    /// Build the phi(t) forward curve as (time, rate) pairs from the discount curve.
    fn build_phi_points(disc: &dyn Discounting, maturity: f64) -> Vec<(f64, f64)> {
        let num_points = 50;
        let dt = maturity / num_points as f64;
        let mut points = Vec::with_capacity(num_points + 1);

        for i in 0..=num_points {
            let t = i as f64 * dt;
            // Instantaneous forward rate approximation: f(0,t) ~ -d/dt ln(P(0,t))
            let eps = 0.001_f64.min(dt * 0.5).max(1e-6);
            let df_minus = disc.df((t - eps).max(0.0));
            let df_plus = disc.df(t + eps);
            let fwd = if df_minus > 1e-15 && df_plus > 1e-15 {
                -(df_plus.ln() - df_minus.ln()) / (2.0 * eps)
            } else {
                0.03 // fallback
            };
            // For t=0 use a slightly positive time to ensure strictly increasing
            let time = if i == 0 { 0.0 } else { t };
            points.push((time, fwd.max(-0.01))); // floor at -1%
        }

        points
    }

    /// Cheyette bond-reconstruction factor `B(t, T) = (1 - exp(-kappa*tau)) / kappa`.
    ///
    /// `tau = T - t`. As `kappa -> 0` the limit is `B = tau`.
    fn b_factor(kappa: f64, tau: f64) -> f64 {
        if kappa.abs() < 1e-12 {
            tau
        } else {
            (1.0 - (-kappa * tau).exp()) / kappa
        }
    }

    /// Reconstruct the time-`t` zero-coupon bond `P(t, T; x, y)` from the
    /// Cheyette `[x, y]` state.
    ///
    /// For the quasi-Gaussian (Cheyette) model the bond price is exactly
    ///
    /// ```text
    /// P(t, T; x, y) = [P_M(0, T) / P_M(0, t)]
    ///                 * exp(-B(t, T)*x - 0.5*B(t, T)^2*y)
    /// ```
    ///
    /// where `P_M(0, .)` is the market discount curve and `B(t, T)` the
    /// reconstruction factor above (Andersen & Piterbarg 2010, Vol. II, §12).
    fn reconstruct_bond(
        kappa: f64,
        x_state: f64,
        y_state: f64,
        df_t: f64,
        df_cap_t: f64,
        tau: f64,
    ) -> f64 {
        if df_t.abs() < 1e-15 {
            return 0.0;
        }
        let b = Self::b_factor(kappa, tau);
        (df_cap_t / df_t) * (-b * x_state - 0.5 * b * b * y_state).exp()
    }

    /// Compute the swap value from the Cheyette `[x, y]` state and market data.
    ///
    /// The realized swap value is reconstructed from the full Cheyette term
    /// structure: each future bond `P(T_ex, T_j)` is rebuilt from the `[x, y]`
    /// state via [`Self::reconstruct_bond`], rather than discounting with a
    /// flat short rate `exp(-r_t * t_j)`.  The flat-rate approximation is
    /// materially biased on steep curves because it ignores both the shape of
    /// `phi(t)` and the variance state `y`.
    fn compute_swap_value(
        x_state: f64,
        y_state: f64,
        kappa: f64,
        disc: &dyn Discounting,
        inputs: &SwapValueInputs,
    ) -> f64 {
        let remaining = inputs.swap_end_time - inputs.exercise_time;
        if remaining < inputs.period * 0.5 {
            return 0.0;
        }

        // Number of remaining periods
        let n_periods = ((remaining / inputs.period).round() as usize).max(1);
        let actual_period = remaining / n_periods as f64;

        // Market discount factor at the exercise date (curve origin for the
        // reconstruction).  All absolute times are measured from `as_of` (0),
        // consistent with `disc.df(.)`.
        let df_cap_t = disc.df(inputs.exercise_time);
        if df_cap_t.abs() < 1e-15 {
            return 0.0;
        }

        // Reconstruct annuity and terminal bond from the Cheyette term structure.
        let mut annuity = 0.0;
        let mut df_end = 1.0;
        for j in 1..=n_periods {
            let t_j = inputs.exercise_time + j as f64 * actual_period;
            let df_market_tj = disc.df(t_j);
            let p_j = Self::reconstruct_bond(
                kappa,
                x_state,
                y_state,
                df_cap_t,
                df_market_tj,
                t_j - inputs.exercise_time,
            );
            annuity += actual_period * p_j;
            if j == n_periods {
                df_end = p_j;
            }
        }

        // Forward swap rate: P(t, T_0) = 1 since T_0 = exercise date.
        let swap_rate = if annuity.abs() > 1e-15 {
            (1.0 - df_end) / annuity
        } else {
            return 0.0;
        };

        // Intrinsic value
        if inputs.is_payer {
            (swap_rate - inputs.strike) * annuity * inputs.notional
        } else {
            (inputs.strike - swap_rate) * annuity * inputs.notional
        }
    }

    /// Price the Bermudan swaption using Cheyette rough vol MC + LSMC.
    fn price_internal(
        &self,
        swaption: &BermudanSwaption,
        market: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> Result<(Money, f64), PricingError> {
        let disc = market
            .get_discount(swaption.discount_curve_id.as_str())
            .map_err(|e| {
                PricingError::missing_market_data_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let ttm = swaption.time_to_maturity(as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        if ttm <= 0.0 {
            return Ok((Money::new(0.0, swaption.notional.currency()), 0.0));
        }

        let strike = swaption.strike_f64().map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        let is_payer =
            swaption.option_type == crate::instruments::common_impl::parameters::OptionType::Call;
        let notional = swaption.notional.amount();
        let currency = swaption.notional.currency();

        let swap_end_time =
            year_fraction(swaption.day_count, as_of, swaption.swap_end).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        // Fixed leg period
        let tenor_months = swaption.fixed_freq.months().unwrap_or(6) as f64;
        let period = tenor_months / 12.0;

        // Exercise times
        let exercise_times = swaption
            .bermudan_schedule
            .exercise_times(as_of, swaption.day_count)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        if exercise_times.is_empty() {
            return Ok((Money::new(0.0, currency), 0.0));
        }

        // Build Cheyette parameters
        let phi_points = Self::build_phi_points(disc.as_ref(), swap_end_time);

        // Get base vol from vol surface (use ATM vol at midpoint expiry)
        let base_vol = market
            .get_surface(swaption.vol_surface_id.as_str())
            .map(|surf| {
                let mid_t = exercise_times.first().copied().unwrap_or(1.0);
                // Convert Black vol to short-rate vol (approximate: divide by sqrt(T))
                let black_vol = surf.value_clamped(mid_t, strike);
                // Short rate vol is roughly Black vol * forward rate
                let fwd_rate = phi_points.last().map(|&(_, r)| r).unwrap_or(0.03);
                (black_vol * fwd_rate).max(0.001)
            })
            .unwrap_or(0.005); // 50bps default base vol

        // Cheyette model parameters (uncalibrated defaults)
        let kappa = 0.03;
        let eta = 1.5;
        let hurst_val = 0.1;
        let rho = -0.5;

        let hurst = HurstExponent::new(hurst_val).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let sigma_base = ForwardVarianceCurve::flat(base_vol).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let params = CheyetteRoughVolParams::new(kappa, sigma_base, hurst, eta, rho, &phi_points)
            .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let process = CheyetteRoughVolProcess::new(params.clone());
        let euler = CheyetteRoughEuler::new(hurst);

        // Build uniform time grid
        let num_steps = self.config.num_steps;
        let dt = ttm / num_steps as f64;
        let times: Vec<f64> = (0..=num_steps).map(|i| i as f64 * dt).collect();

        let time_grid = TimeGrid::from_times(times.clone()).map_err(|e| {
            PricingError::model_failure_with_context(
                format!("Failed to build time grid: {e}"),
                PricingErrorContext::default(),
            )
        })?;

        // Map exercise times to grid step indices
        let exercise_step_indices: Vec<usize> = exercise_times
            .iter()
            .filter_map(|&ex_t| {
                if ex_t <= 0.0 || ex_t > ttm {
                    return None;
                }
                let mut best_idx = 0;
                let mut best_dist = f64::MAX;
                for (idx, &t) in times.iter().enumerate() {
                    let d = (t - ex_t).abs();
                    if d < best_dist {
                        best_dist = d;
                        best_idx = idx;
                    }
                }
                Some(best_idx)
            })
            .collect();

        if exercise_step_indices.is_empty() {
            return Ok((Money::new(0.0, currency), 0.0));
        }

        // Create fBM generator for rough vol
        let fbm_gen = create_fbm_generator(&times, hurst_val).map_err(|e| {
            PricingError::model_failure_with_context(
                format!("Failed to create fBM generator: {e}"),
                PricingErrorContext::default(),
            )
        })?;

        let work_size = euler.work_size(&process);
        // Derive deterministic seed from instrument id for reproducible but
        // instrument-specific MC noise (consistent with equity MC pricers).
        let seed_val = finstack_monte_carlo::seed::derive_seed(&swaption.id, "base");
        let base_rng = PhiloxRng::new(seed_val);

        // --- Phase 1: Simulate paths ---
        // Store [x, y] state at each exercise step for each path
        let num_exercises = exercise_step_indices.len();
        let num_paths = self.config.num_paths;

        // states_at_exercise[ex_idx][path_idx] = (x, y)
        let mut states_at_exercise: Vec<Vec<(f64, f64)>> =
            vec![Vec::with_capacity(num_paths); num_exercises];

        // df_at_exercise[ex_idx][path_idx] = accumulated P(0, T_ex) along the path.
        // Used for (a) discounting LSMC continuation values between exercise dates
        // and (b) discounting the final cashflow back to time 0.
        let mut df_at_exercise: Vec<Vec<f64>> = vec![Vec::with_capacity(num_paths); num_exercises];

        for path_id in 0..num_paths {
            let mut rng = base_rng.substream(path_id as u64);
            let mut x = vec![0.0, 0.0]; // [x, y] initial state
            let mut work = vec![0.0; work_size];
            let mut z = vec![0.0; 2]; // 2 factors

            // Generate fBM increments for this path
            let mut fbm_normals = vec![0.0; num_steps];
            rng.fill_std_normals(&mut fbm_normals);
            let mut fbm_increments = vec![0.0; num_steps];
            fbm_gen.generate(&fbm_normals, &mut fbm_increments);

            // Track which exercise index we're on
            let mut ex_ptr = 0;

            // Accumulate path-wise discount factor
            let mut cum_df = 1.0;

            for (step, fbm_increment) in fbm_increments.iter().copied().enumerate() {
                let t = time_grid.time(step);
                let step_dt = time_grid.dt(step);

                // Short rate BEFORE the Euler step (for trapezoidal discounting)
                let r_before = x[0] + params.phi(t);

                // Fill z[0] with independent normal, z[1] with fBM increment
                rng.fill_std_normals(&mut z[..1]);
                z[1] = fbm_increment;

                // Euler step
                euler.step(&process, t, step_dt, &mut x, &z, &mut work);

                // Short rate AFTER the step
                let r_after = x[0] + params.phi(t + step_dt);

                // Trapezoidal rule for discount factor accumulation
                // (reduces bias from O(dt) to O(dt^2), Glasserman §6.1)
                let r_avg = 0.5 * (r_before + r_after);
                cum_df *= (-r_avg * step_dt).exp();

                // Record state and discount factor at exercise dates
                if ex_ptr < num_exercises && step + 1 == exercise_step_indices[ex_ptr] {
                    states_at_exercise[ex_ptr].push((x[0], x[1]));
                    df_at_exercise[ex_ptr].push(cum_df);
                    ex_ptr += 1;
                }
            }

            // Fill any remaining exercise dates (in case of grid alignment issues)
            while ex_ptr < num_exercises {
                states_at_exercise[ex_ptr].push((x[0], x[1]));
                df_at_exercise[ex_ptr].push(cum_df);
                ex_ptr += 1;
            }
        }

        // --- Phase 2: LSMC backward induction ---
        // cashflow[path_idx] stores the (undiscounted) cashflow at the exercise
        // date where exercise occurs. cashflow_ex_idx[path_idx] records WHICH
        // exercise date that cashflow belongs to, so we can discount correctly.
        let mut cashflow = vec![0.0_f64; num_paths];
        let mut cashflow_ex_idx = vec![num_exercises - 1; num_paths];

        for ex_idx in (0..num_exercises).rev() {
            let step = exercise_step_indices[ex_idx];
            let ex_time = times[step];
            let swap_value_inputs = SwapValueInputs {
                exercise_time: ex_time,
                swap_end_time,
                period,
                strike,
                is_payer,
                notional,
            };

            // Compute exercise values at each path
            let mut exercise_values: Vec<f64> = Vec::with_capacity(num_paths);
            let mut basis_inputs: Vec<(f64, f64)> = Vec::with_capacity(num_paths);

            for &(x_val, y_val) in states_at_exercise[ex_idx].iter().take(num_paths) {
                // Reconstruct the realized swap value from the full Cheyette
                // term structure (W-16): the prior flat-rate discounting
                // `exp(-r_t * t_j)` is materially biased on steep curves.
                let ev = Self::compute_swap_value(
                    x_val,
                    y_val,
                    kappa,
                    disc.as_ref(),
                    &swap_value_inputs,
                );

                exercise_values.push(ev);
                basis_inputs.push((x_val, y_val));
            }

            if ex_idx == num_exercises - 1 {
                // Last exercise: exercise if positive
                for (i, &ev) in exercise_values.iter().enumerate() {
                    if ev > 0.0 {
                        cashflow[i] = ev;
                        cashflow_ex_idx[i] = ex_idx;
                    }
                }
            } else {
                // Interior exercise: regression for continuation.
                // The continuation value must be discounted from the future
                // exercise date back to the current exercise date
                // (Longstaff-Schwartz 2001, §2 eq. 4).
                let mut itm_indices = Vec::new();
                let mut itm_basis = Vec::new();
                let mut itm_continuation = Vec::new();

                for (i, &ev) in exercise_values.iter().enumerate() {
                    if ev > 0.0 {
                        itm_indices.push(i);
                        let (x_val, y_val) = basis_inputs[i];
                        // Polynomial basis: [1, x, y, x^2, x*y, y^2]
                        let mut b = vec![1.0, x_val, y_val];
                        if self.config.basis_degree >= 2 {
                            b.push(x_val * x_val);
                            b.push(x_val * y_val);
                            b.push(y_val * y_val);
                        }
                        if self.config.basis_degree >= 3 {
                            b.push(x_val * x_val * x_val);
                        }
                        itm_basis.push(b);

                        // Discount the future cashflow from its exercise date
                        // back to the current exercise date:
                        //   cont = cashflow[i] * DF(0, T_future) / DF(0, T_current)
                        let future_ex = cashflow_ex_idx[i];
                        let df_current = df_at_exercise[ex_idx][i];
                        let df_future = df_at_exercise[future_ex][i];
                        let discounted_cf = if df_current.abs() > 1e-15 {
                            cashflow[i] * df_future / df_current
                        } else {
                            cashflow[i]
                        };
                        itm_continuation.push(discounted_cf);
                    }
                }

                let num_basis_cols = itm_basis.first().map_or(0, |b| b.len());

                if itm_indices.len() > num_basis_cols + 3 {
                    // Solve least-squares regression
                    let mut a_matrix = vec![0.0; itm_indices.len() * num_basis_cols];
                    for (row, basis) in itm_basis.iter().enumerate() {
                        for (col, &val) in basis.iter().enumerate() {
                            a_matrix[row * num_basis_cols + col] = val;
                        }
                    }

                    if let Ok(coeffs) = solve_least_squares(
                        &a_matrix,
                        &itm_continuation,
                        itm_indices.len(),
                        num_basis_cols,
                    ) {
                        // Exercise vs continuation decision
                        for (local_idx, &global_idx) in itm_indices.iter().enumerate() {
                            let mut cont_value = 0.0;
                            for (c, &coeff) in coeffs.iter().enumerate() {
                                cont_value += coeff * itm_basis[local_idx][c];
                            }
                            let ev = exercise_values[global_idx];
                            if ev > cont_value {
                                cashflow[global_idx] = ev;
                                cashflow_ex_idx[global_idx] = ex_idx;
                            }
                        }
                    }
                } else {
                    // Too few ITM paths: exercise if positive
                    for &idx in &itm_indices {
                        // Compare discounted continuation
                        let future_ex = cashflow_ex_idx[idx];
                        let df_current = df_at_exercise[ex_idx][idx];
                        let df_future = df_at_exercise[future_ex][idx];
                        let discounted_cf = if df_current.abs() > 1e-15 {
                            cashflow[idx] * df_future / df_current
                        } else {
                            cashflow[idx]
                        };
                        if exercise_values[idx] > discounted_cf {
                            cashflow[idx] = exercise_values[idx];
                            cashflow_ex_idx[idx] = ex_idx;
                        }
                    }
                }
            }
        }

        // --- Phase 3: Average discounted cashflows ---
        // Each cashflow[i] is at the exercise date cashflow_ex_idx[i].
        // Discount it to time 0 using df_at_exercise for that date.
        let mut stats = OnlineStats::new();
        for (i, &cf) in cashflow.iter().enumerate() {
            let ex = cashflow_ex_idx[i];
            let df_to_zero = df_at_exercise[ex][i];
            stats.update(cf * df_to_zero);
        }

        let mean = stats.mean();
        let stderr = stats.stderr();
        Ok((Money::new(mean, currency), stderr))
    }
}

impl Pricer for BermudanSwaptionCheyetteRoughPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(
            InstrumentType::BermudanSwaption,
            ModelKey::MonteCarloCheyetteRoughVol,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> Result<ValuationResult, PricingError> {
        let swaption = instrument
            .as_any()
            .downcast_ref::<BermudanSwaption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::BermudanSwaption, instrument.key())
            })?;

        let (pv, stderr) = self.price_internal(swaption, market, as_of)?;

        let mut result = ValuationResult::stamped(swaption.id.as_str(), as_of, pv);
        if stderr > 0.0 {
            result
                .measures
                .insert(crate::metrics::MetricId::custom("mc_stderr"), stderr);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::Date;
    use finstack_core::market_data::traits::{Discounting, TermStructure};
    use finstack_core::types::CurveId;
    use time::macros::date;

    /// Steep discount curve: instantaneous forward rises linearly from
    /// `f0` at t=0 to `f0 + slope*t`, so `P(0, t) = exp(-(f0*t + 0.5*slope*t^2))`.
    struct SteepCurve {
        id: CurveId,
        f0: f64,
        slope: f64,
    }

    impl TermStructure for SteepCurve {
        fn id(&self) -> &CurveId {
            &self.id
        }
    }

    impl Discounting for SteepCurve {
        fn base_date(&self) -> Date {
            date!(2025 - 01 - 01)
        }
        fn df(&self, t: f64) -> f64 {
            (-(self.f0 * t + 0.5 * self.slope * t * t)).exp()
        }
    }

    /// On a steep curve, the realized swap value at the zero Cheyette state
    /// `[x, y] = [0, 0]` must equal the genuine market forward swap rate
    /// implied by the discount curve.  The prior flat-rate discounting
    /// `exp(-r_t * t_j)` does NOT reproduce this — it ignores curve slope.
    #[test]
    fn cheyette_realized_swap_value_matches_term_structure_on_steep_curve() {
        let curve = SteepCurve {
            id: CurveId::from("STEEP"),
            f0: 0.01,
            slope: 0.02, // 2% per year of curve steepness
        };

        let inputs = SwapValueInputs {
            exercise_time: 2.0,
            swap_end_time: 7.0,
            period: 1.0,
            strike: 0.03,
            is_payer: true,
            notional: 1.0,
        };
        let kappa = 0.03;

        // Production value at the zero state (x = y = 0).
        let value = BermudanSwaptionCheyetteRoughPricer::compute_swap_value(
            0.0, 0.0, kappa, &curve, &inputs,
        );

        // Independent term-structure reconstruction: at x = y = 0 the
        // reconstructed bonds collapse to the market discount factors, so the
        // swap rate is the curve's true forward swap rate.
        let n = 5usize; // (7 - 2) / 1
        let df_t = curve.df(inputs.exercise_time);
        let mut annuity = 0.0;
        let mut df_end = 1.0;
        for j in 1..=n {
            let t_j = inputs.exercise_time + j as f64;
            let p_j = curve.df(t_j) / df_t;
            annuity += p_j;
            if j == n {
                df_end = p_j;
            }
        }
        let swap_rate = (1.0 - df_end) / annuity;
        let expected = (swap_rate - inputs.strike) * annuity * inputs.notional;

        assert!(
            (value - expected).abs() < 1e-10,
            "term-structure realized value {value} != reconstruction {expected}"
        );

        // Sanity: the flat-rate approximation (old behaviour) discounts with
        // exp(-r_t * t_j) using r_t = phi(exercise_time). On this steep curve
        // that is materially different, confirming the fix is load-bearing.
        let r_flat = curve.f0 + curve.slope * inputs.exercise_time; // phi(2) = 0.05
        let mut flat_annuity = 0.0;
        let mut flat_df_end = 1.0;
        for j in 1..=n {
            let df_j = (-r_flat * j as f64).exp();
            flat_annuity += df_j;
            if j == n {
                flat_df_end = df_j;
            }
        }
        let flat_rate = (1.0 - flat_df_end) / flat_annuity;
        let flat_value = (flat_rate - inputs.strike) * flat_annuity * inputs.notional;
        assert!(
            (flat_value - expected).abs() > 1e-3,
            "flat-rate value should be materially biased on a steep curve"
        );
    }

    /// At a non-zero `[x, y]` state the reconstruction must still be
    /// curve-consistent: a positive rate shock `x > 0` lowers all bond prices
    /// and therefore raises a payer swap value relative to the zero state.
    #[test]
    fn cheyette_realized_swap_value_responds_to_state() {
        let curve = SteepCurve {
            id: CurveId::from("STEEP"),
            f0: 0.02,
            slope: 0.01,
        };
        let inputs = SwapValueInputs {
            exercise_time: 1.0,
            swap_end_time: 6.0,
            period: 1.0,
            strike: 0.03,
            is_payer: true,
            notional: 1.0,
        };
        let kappa = 0.05;

        let base = BermudanSwaptionCheyetteRoughPricer::compute_swap_value(
            0.0, 0.0, kappa, &curve, &inputs,
        );
        let shocked = BermudanSwaptionCheyetteRoughPricer::compute_swap_value(
            0.01, 0.0, kappa, &curve, &inputs,
        );
        assert!(
            shocked > base,
            "payer swap value should rise under a positive rate shock: \
             base={base}, shocked={shocked}"
        );
    }
}

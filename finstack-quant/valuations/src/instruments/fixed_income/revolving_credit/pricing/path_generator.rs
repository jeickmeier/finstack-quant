//! Three-factor Monte Carlo path generation for revolving credit facilities.
//!
//! Generates correlated paths for utilization, interest rates, and credit spreads
//! using the existing `RevolvingCreditProcess` infrastructure.
//!
//! # Variance Reduction
//!
//! Supports antithetic variance reduction when enabled via `StochasticUtilizationSpec.antithetic`.
//! This mirrors each path with negated random variates, typically reducing variance by ~50%
//! for smooth payoff functions.
//!
//! # CIR Process Stability
//!
//! The CIR credit spread process requires the Feller condition (2κθ > σ²) to guarantee
//! positive spreads. When violated, a warning is logged and the process may occasionally
//! touch zero. The QE discretization scheme handles boundary behavior gracefully.

use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;

use crate::instruments::fixed_income::revolving_credit::pricing::monte_carlo_discretization::RevolvingCreditDiscretization;
use crate::instruments::fixed_income::revolving_credit::pricing::monte_carlo_process::{
    CreditSpreadParams, InterestRateSpec, RevolvingCreditProcess, RevolvingCreditProcessParams,
    UtilizationParams,
};
use crate::instruments::rates::hw1f::{initial_short_rate_from_curve, prepare_hw1f_params};
use finstack_quant_models::monte_carlo::process::ou::HullWhite1FParams;
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_models::monte_carlo::rng::sobol::SobolRng;
use finstack_quant_models::monte_carlo::traits::{Discretization, RandomStream, StochasticProcess};
use finstack_quant_models::monte_carlo::TimeGrid;
use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;

use super::super::cashflow_engine::ThreeFactorPathData;
use super::super::types::{
    BaseRateSpec, CreditSpreadProcessSpec, InterestRateProcessSpec, McConfig, RevolvingCredit,
    StochasticUtilizationSpec, UtilizationProcess,
};

/// Generate 3-factor MC paths using the existing process infrastructure.
///
/// This function creates correlated paths for utilization, interest rates, and credit spreads
/// by simulating the `RevolvingCreditProcess` across the payment schedule.
///
/// # Arguments
///
/// * `stoch_spec` - Stochastic specification with utilization process and MC config
/// * `mc_config` - Monte Carlo configuration with correlation and process details
/// * `facility` - Revolving credit facility
/// * `market` - Market context for curves
/// * `payment_dates` - Observation dates on which factor state is recorded:
///   the contractual accrual boundaries plus term-index reset dates (see
///   `utils::build_observation_dates`), sorted ascending.
/// * `as_of` - Valuation date; simulation starts here (not at the commitment
///   date) with the facility's current utilization as the known t₀ state.
///   Observation dates at or before `as_of` record the t₀ state.
///
/// # Variance Reduction
///
/// When `stoch_spec.antithetic` is true and Sobol QMC is not used, generates paths
/// in pairs using antithetic variates (z and -z), reducing variance for smooth payoffs.
///
/// # Returns
///
/// Vector of `ThreeFactorPathData`, one per simulated path
pub fn generate_three_factor_paths(
    stoch_spec: &StochasticUtilizationSpec,
    mc_config: &McConfig,
    facility: &RevolvingCredit,
    market: &MarketContext,
    payment_dates: &[Date],
    as_of: Date,
) -> Result<Vec<ThreeFactorPathData>> {
    if stoch_spec.num_paths < 2 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "stochastic revolving-credit pricing requires num_paths >= 2 \
             (a single path has no variance estimate), got {}",
            stoch_spec.num_paths
        )));
    }

    // Simulation time runs on the ACT/365F model clock (the clock the rate and
    // credit processes are calibrated on); accrual keeps the facility day count.
    let day_count = super::super::MC_CLOCK_DAY_COUNT;
    // All stochastic factors start at the valuation date for seasoned
    // facilities.  Keep path time on the facility axis, but retain the
    // simulation anchor so curve-fitted models can use their own t=0.
    let simulation_anchor = as_of.max(facility.commitment_date);
    let t_asof = if as_of > facility.commitment_date {
        day_count.year_fraction(facility.commitment_date, as_of, DayCountContext::default())?
    } else {
        0.0
    };
    let (util_params, is_zero_vol) = match &stoch_spec.utilization_process {
        UtilizationProcess::MeanReverting {
            target_rate,
            speed,
            volatility,
            spread_sensitivity,
        } => {
            // Zero (or effectively-zero) volatility is parity mode: pass a
            // true σ = 0 so the utilization diffusion term vanishes exactly
            // rather than substituting a tiny placeholder volatility.
            let is_zero = volatility.abs() < 1e-8;
            let vol = if is_zero { 0.0 } else { *volatility };
            (
                UtilizationParams::new(*speed, *target_rate, vol)?
                    .with_spread_sensitivity(*spread_sensitivity)?,
                is_zero,
            )
        }
    };

    let disc_curve = market.get_discount(facility.discount_curve_id.as_str())?;
    // `obs_forward_rates` is `Some` in deterministic-forward mode: the index
    // forward read on the curve's own clock at each observation date, recorded
    // verbatim as the path's short rate so the stochastic engine projects the
    // same fixing the deterministic engine does.
    let (interest_rate_spec, obs_forward_rates, rate_time_offset): (
        InterestRateSpec,
        Option<Vec<f64>>,
        f64,
    ) = match &facility.base_rate_spec {
        BaseRateSpec::Fixed { rate } => (InterestRateSpec::Fixed { rate: *rate }, None, 0.0),
        BaseRateSpec::Floating(spec) => {
            let overnight = crate::instruments::common_impl::pricing::overnight_conventions::resolved_overnight_compounding(
                spec.index_id.as_str(),
                spec.overnight_compounding.as_ref(),
            )?;
            if overnight.is_some()
                && matches!(
                    &mc_config.interest_rate_process,
                    Some(InterestRateProcessSpec::HullWhite1F { sigma, .. }) if *sigma > 0.0
                )
            {
                return Err(finstack_quant_core::Error::Validation(
                    "Overnight RFR revolving-credit facilities cannot be priced with \
                     stochastic Hull-White rates: the path generator records short \
                     rates only on payment dates, not the daily fixing path required \
                     to compound an overnight coupon. Use a term index, set \
                     Hull-White sigma to 0, or price overnight facilities on the \
                     static forward curve."
                        .to_string(),
                ));
            }
            match &mc_config.interest_rate_process {
                Some(InterestRateProcessSpec::HullWhite1F {
                    kappa,
                    sigma,
                    initial,
                    theta,
                }) => {
                    if *sigma > 0.0 {
                        let horizon = day_count
                            .year_fraction(
                                simulation_anchor,
                                facility.maturity,
                                DayCountContext::default(),
                            )?
                            .max(1e-6);
                        let scalar = HullWhiteCalibrationParams::new(*kappa, *sigma)?;
                        let params = prepare_hw1f_params(
                            scalar,
                            disc_curve.as_ref(),
                            simulation_anchor,
                            horizon,
                        )?;
                        let initial =
                            initial_short_rate_from_curve(disc_curve.as_ref(), simulation_anchor)?;
                        (
                            InterestRateSpec::Floating { params, initial },
                            None,
                            -t_asof,
                        )
                    } else {
                        // Preserve the zero-volatility mode used for
                        // deterministic parity tests. With no diffusion,
                        // the supplied constant mean level remains exact.
                        (
                            InterestRateSpec::Floating {
                                params: HullWhite1FParams::new(*kappa, *sigma, *theta)?,
                                initial: *initial,
                            },
                            None,
                            0.0,
                        )
                    }
                }
                None => {
                    // Use deterministic forward curve
                    let fwd = market.get_forward(spec.index_id.as_str())?;
                    let times = fwd.knots().to_vec();
                    let rates = fwd.forwards().to_vec();
                    let curve_offset = fwd.day_count().signed_year_fraction(
                        fwd.base_date(),
                        facility.commitment_date,
                        DayCountContext::default(),
                    )?;
                    let observed = payment_dates
                        .iter()
                        .map(|&date| {
                            let t = fwd.day_count().signed_year_fraction(
                                fwd.base_date(),
                                date,
                                DayCountContext::default(),
                            )?;
                            Ok(fwd.rate(t.max(0.0)))
                        })
                        .collect::<Result<Vec<f64>>>()?;
                    (
                        InterestRateSpec::DeterministicForward { times, rates },
                        Some(observed),
                        curve_offset,
                    )
                }
            }
        }
    };

    let credit_spread_params =
        build_credit_spread_params(mc_config, facility, market, simulation_anchor)?;

    // Reject malformed rate specifications before any path is simulated: the
    // discretization indexes rate knots by position, so a mismatched or
    // non-monotone curve must fail here rather than panic (or silently price
    // a zero rate) mid-simulation.
    interest_rate_spec.validate()?;

    // Create 3-factor process with correlation
    let mut process_params =
        RevolvingCreditProcessParams::new(util_params, interest_rate_spec, credit_spread_params);

    if let Some(corr_matrix) = &mc_config.correlation_matrix {
        process_params = process_params.with_correlation(*corr_matrix);
    } else if let Some(rho) = mc_config.util_credit_corr {
        // Documented 2-factor shorthand: utilization–credit correlation ρ
        // with the rate factor uncorrelated. The field must be applied here,
        // never accepted-then-ignored.
        process_params =
            process_params.with_correlation([[1.0, 0.0, rho], [0.0, 1.0, 0.0], [rho, 0.0, 1.0]]);
    }

    process_params = process_params.with_time_offset(rate_time_offset);

    let process = RevolvingCreditProcess::new(process_params);

    // Whether the short-rate trajectory is genuinely stochastic (Hull-White
    // with σ > 0). The pricer uses this to choose pathwise bank-account
    // discounting over the static curve.
    let stochastic_rates = matches!(
        &process.get_params().interest_rate,
        InterestRateSpec::Floating { params, .. } if params.sigma_at_time(0.0) > 0.0
    );

    // Convert payment dates to time points using facility's day count
    let raw_time_points = dates_to_times(payment_dates, facility.commitment_date, day_count)?;

    // Seasoned facilities simulate from the VALUATION date, not the
    // commitment date: the current utilization is a KNOWN t₀ state, and
    // re-simulating the elapsed history would overstate dispersion at every
    // future date. Payment dates at or before `as_of` simply record the t₀
    // state.
    // Payment dates at/before as_of record the initial state (at least the
    // first payment date, which is the commitment date itself).
    let num_initial = raw_time_points
        .iter()
        .filter(|&&t| t <= t_asof)
        .count()
        .max(1);
    let sim_start = t_asof.max(raw_time_points[0]);
    let mut sim_times = Vec::with_capacity(raw_time_points.len() + 1);
    sim_times.push(sim_start);
    sim_times.extend(raw_time_points.iter().copied().filter(|&t| t > sim_start));
    if sim_times.len() < 2 {
        // Everything is in the past — add a dummy step so the grid is valid;
        // recordings are capped at the payment-date count so the dummy is
        // never recorded.
        sim_times.push(sim_start + 1e-6);
    }

    // Refine grid to ensure no step exceeds MAX_MC_TIME_STEP for numerical
    // stability. Stepping stays on the facility axis so `time_offset`
    // semantics are unchanged, but `TimeGrid` requires its first point to be
    // exactly zero, so the grid is validated on the anchor-relative axis.
    // Seasoned facilities start at `sim_start > 0`; validating the facility
    // axis directly rejected every valuation after the commitment date.
    let refined = refine_time_grid(&sim_times);
    let time_grid = TimeGrid::from_times(refined.times.iter().map(|t| t - sim_start).collect())?;
    let times_ref: &[f64] = &refined.times;

    let disc = RevolvingCreditDiscretization::new(process.correlation())?;

    let num_paths = stoch_spec.num_paths;
    let num_steps = time_grid.num_steps();
    let num_factors = process.num_factors();
    let initial_state = if sim_start == 0.0 {
        process
            .get_params()
            .initial_state(facility.utilization_rate())
    } else {
        process
            .get_params()
            .initial_state_at(facility.utilization_rate(), sim_start)
    };
    let num_payment_dates = payment_dates.len();
    let obs_rates: Option<&[f64]> = obs_forward_rates.as_deref();

    let mut paths = Vec::with_capacity(num_paths);
    let seed = stoch_spec.seed.unwrap_or(42);
    let use_sobol = stoch_spec.use_sobol_qmc;
    // Antithetic is incompatible with Sobol QMC; `RevolvingCredit::validate()`
    // rejects the combination, and this guard covers direct callers.
    let use_antithetic = stoch_spec.antithetic && !use_sobol;

    // Reusable scratch buffer (used by the serial Sobol path; the parallel
    // Philox path allocates per-thread inside the rayon closure).
    let mut work = vec![0.0; disc.work_size(&process)];

    if use_sobol {
        // One Sobol point per PATH: each path consumes a
        // `num_steps × num_factors`-dimensional coordinate, per the Sobol
        // dimension contract (see `monte_carlo::rng::sobol`). Drawing a
        // 3-dimensional point per time step (the previous behavior) feeds
        // van-der-Corput anti-correlated consecutive coordinates into
        // successive time steps — statistically invalid path dynamics, not
        // just reduced efficiency. Schedules whose refined grid exceeds the
        // supported Sobol dimension are rejected; use pseudorandom paths
        // (`use_sobol_qmc = false`) instead.
        let sobol_dim = num_steps.saturating_mul(num_factors);
        let mut rng = SobolRng::try_new(sobol_dim, seed).map_err(|err| {
            finstack_quant_core::Error::Validation(format!(
                "use_sobol_qmc requires one Sobol coordinate per (step, factor): \
                 num_steps ({num_steps}) × num_factors ({num_factors}) = {sobol_dim}, \
                 which is not supported ({err}); disable use_sobol_qmc for this schedule"
            ))
        })?;
        let mut z_path = vec![0.0; sobol_dim];

        for _path_idx in 0..num_paths {
            // Draw the full path's coordinate vector up front.
            rng.fill_std_normals(&mut z_path);
            let mut state = initial_state.to_vec();
            // Only record states at payment dates, not at intermediate simulation steps
            let mut utilization_path = Vec::with_capacity(num_payment_dates);
            let mut short_rate_path = Vec::with_capacity(num_payment_dates);
            let mut credit_spread_path = Vec::with_capacity(num_payment_dates);

            // Record the t₀ state for every payment date at/before as_of
            // (at least the first).
            for _ in 0..num_initial {
                let idx = utilization_path.len();
                utilization_path.push(state[0].clamp(0.0, 1.0));
                short_rate_path.push(obs_rates.map_or(state[1], |rates| rates[idx]));
                credit_spread_path.push(state[2].max(0.0));
            }

            // Track which simulation anchor we're recording next
            let mut next_payment_idx = 1;

            // Evolve through time on the refined grid
            for i in 0..num_steps {
                let t_next = times_ref[i + 1];

                {
                    let t = times_ref[i];
                    let dt = t_next - t;

                    // Slice this step's factors out of the path's Sobol point
                    let z = &z_path[i * num_factors..(i + 1) * num_factors];

                    // Apply discretization scheme to evolve state. Zero
                    // utilization vol freezes ONLY the utilization factor —
                    // rate and credit-spread dynamics must keep stepping
                    // (the previous behavior froze all three factors).
                    let u_frozen = state[0];
                    disc.step(&process, t, dt, &mut state, z, &mut work);
                    if is_zero_vol {
                        state[0] = u_frozen;
                    }
                }

                // Only record state at observation dates (not intermediate steps)
                if next_payment_idx < refined.payment_indices.len()
                    && i + 1 == refined.payment_indices[next_payment_idx]
                    && utilization_path.len() < num_payment_dates
                {
                    let idx = utilization_path.len();
                    utilization_path.push(state[0].clamp(0.0, 1.0));
                    short_rate_path.push(obs_rates.map_or(state[1], |rates| rates[idx]));
                    credit_spread_path.push(state[2].max(0.0));
                    next_payment_idx += 1;
                }
            }

            paths.push(ThreeFactorPathData {
                utilization_path,
                short_rate_path,
                credit_spread_path,
                time_points: raw_time_points.clone(),
                payment_dates: payment_dates.to_vec(),
                stochastic_rates,
            });
        }
    } else {
        // Parallel Philox path generation.
        //
        // Each iteration runs in its own rayon task with a unique Philox substream
        // (`stream_id = iter_idx`), keeping results bit-identical across thread
        // counts: substreams are deterministic and independent, so the path at
        // index `i` does not depend on which thread generates it. Iterations are
        // independent and CPU-bound; on multi-core machines this is the dominant
        // wall-time win for the entire pricer.
        let paths_per_iteration = if use_antithetic { 2 } else { 1 };
        let num_iterations = if use_antithetic {
            num_paths.div_ceil(2)
        } else {
            num_paths
        };

        // Shared read-only handles (cheap to capture in parallel closure).
        let work_size = disc.work_size(&process);
        let raw_time_points_ref = &raw_time_points;
        let payment_dates_ref = payment_dates;
        let payment_indices_ref = &refined.payment_indices;

        let generate_iteration = |iter_idx: usize| {
            // Each iteration has its own RNG substream and its own
            // per-thread scratch buffers. PhiloxRng is counter-based, so
            // (seed, stream_id) uniquely seeds an independent substream.
            let mut rng = PhiloxRng::with_stream(seed, iter_idx as u64);
            let mut z = vec![0.0; num_factors];
            let mut z_neg = if use_antithetic {
                vec![0.0; num_factors]
            } else {
                Vec::new()
            };
            let mut work = vec![0.0; work_size];

            // Generate random variates for this iteration on the refined grid.
            let mut z_sequences: Vec<Vec<f64>> = Vec::with_capacity(num_steps);
            for _ in 0..num_steps {
                rng.fill_std_normals(&mut z);
                z_sequences.push(z.clone());
            }

            let mut local_paths = Vec::with_capacity(paths_per_iteration);
            for sign_idx in 0..paths_per_iteration {
                let mut state = initial_state.to_vec();
                let mut utilization_path = Vec::with_capacity(num_payment_dates);
                let mut short_rate_path = Vec::with_capacity(num_payment_dates);
                let mut credit_spread_path = Vec::with_capacity(num_payment_dates);

                // Record the t₀ state for every payment date at/before
                // as_of (at least the first).
                for _ in 0..num_initial {
                    let idx = utilization_path.len();
                    utilization_path.push(state[0].clamp(0.0, 1.0));
                    short_rate_path.push(obs_rates.map_or(state[1], |rates| rates[idx]));
                    credit_spread_path.push(state[2].max(0.0));
                }

                let mut next_payment_idx = 1;

                for (i, z_seq) in z_sequences.iter().enumerate().take(num_steps) {
                    let t_next = times_ref[i + 1];

                    {
                        let t = times_ref[i];
                        let dt = t_next - t;

                        // Zero utilization vol freezes ONLY the
                        // utilization factor; rate/spread keep stepping.
                        let u_frozen = state[0];
                        if sign_idx == 0 {
                            disc.step(&process, t, dt, &mut state, z_seq, &mut work);
                        } else {
                            for (j, val) in z_seq.iter().enumerate() {
                                z_neg[j] = -val;
                            }
                            disc.step(&process, t, dt, &mut state, &z_neg, &mut work);
                        }
                        if is_zero_vol {
                            state[0] = u_frozen;
                        }
                    }

                    if next_payment_idx < payment_indices_ref.len()
                        && i + 1 == payment_indices_ref[next_payment_idx]
                        && utilization_path.len() < num_payment_dates
                    {
                        let idx = utilization_path.len();
                        utilization_path.push(state[0].clamp(0.0, 1.0));
                        short_rate_path.push(obs_rates.map_or(state[1], |rates| rates[idx]));
                        credit_spread_path.push(state[2].max(0.0));
                        next_payment_idx += 1;
                    }
                }

                local_paths.push(ThreeFactorPathData {
                    utilization_path,
                    short_rate_path,
                    credit_spread_path,
                    time_points: raw_time_points_ref.clone(),
                    payment_dates: payment_dates_ref.to_vec(),
                    stochastic_rates,
                });
            }
            local_paths
        };

        #[cfg(not(target_arch = "wasm32"))]
        let chunked: Vec<Vec<ThreeFactorPathData>> = {
            use rayon::prelude::*;
            (0..num_iterations)
                .into_par_iter()
                .map(generate_iteration)
                .collect()
        };

        #[cfg(target_arch = "wasm32")]
        let chunked: Vec<Vec<ThreeFactorPathData>> =
            (0..num_iterations).map(generate_iteration).collect();

        // Flatten — iteration order is preserved by `collect()` so paths are
        // in the same order as the original serial loop (modulo the antithetic
        // pairing within an iteration).
        for iter_paths in chunked {
            for p in iter_paths {
                if paths.len() >= num_paths {
                    break;
                }
                paths.push(p);
            }
            if paths.len() >= num_paths {
                break;
            }
        }
    }

    let _ = work; // suppress unused-mut warning for the Sobol-only scratch buffer
    Ok(paths)
}

// Use centralized constants from parent module
use super::super::MIN_CIR_SPREAD as CIR_MIN_SPREAD;

/// Build credit spread parameters from MC config.
///
/// # Feller Condition
///
/// For CIR processes, validates the Feller condition: 2κθ > σ². When violated,
/// the process can reach zero. A warning is logged but the process proceeds
/// since the QE discretization handles boundary behavior gracefully.
/// Credit-spread process parameters for one facility.
///
/// Market-anchored specs start at the hazard curve's instantaneous spread on
/// `simulation_anchor` and revert to the conditional average spread over the
/// remaining tenor; explicit CIR and constant specs are guarded for
/// stability.
///
/// # Arguments
///
/// * `mc_config` - Facility Monte Carlo configuration carrying the spread process.
/// * `facility` - Facility supplying recovery and maturity for the anchoring.
/// * `market` - Market context holding the hazard curve for anchored specs.
/// * `simulation_anchor` - Date the spread state is anchored at.
pub(crate) fn build_credit_spread_params(
    mc_config: &McConfig,
    facility: &RevolvingCredit,
    market: &MarketContext,
    simulation_anchor: Date,
) -> Result<CreditSpreadParams> {
    match &mc_config.credit_spread_process {
        CreditSpreadProcessSpec::Cir {
            kappa,
            theta,
            sigma,
            initial,
        } => {
            // Apply stability guards for CIR parameters
            let stable_initial = initial.max(CIR_MIN_SPREAD);
            let stable_theta = theta.max(CIR_MIN_SPREAD);
            let stable_kappa = kappa.max(CIR_MIN_SPREAD);

            // Check Feller condition: 2κθ > σ²
            // When satisfied, the process is guaranteed to stay positive
            let feller_lhs = 2.0 * stable_kappa * stable_theta;
            let feller_rhs = sigma * sigma;
            let feller_ratio = feller_lhs / feller_rhs.max(CIR_MIN_SPREAD);

            if feller_ratio < 1.0 {
                // Feller condition violated; QE discretization will still clip to zero.
                tracing::warn!(
                    target: "finstack_quant_valuations::credit",
                    feller_ratio,
                    kappa = stable_kappa,
                    theta = stable_theta,
                    sigma,
                    "CIR Feller condition violated (2κθ/σ² < 1); credit spreads may touch zero"
                );
            }

            CreditSpreadParams::new(stable_kappa, stable_theta, *sigma, stable_initial)
        }
        CreditSpreadProcessSpec::Constant(spread) => {
            // Use constant spread with minimal dynamics
            let stable_spread = spread.max(0.0);
            CreditSpreadParams::new(0.01, stable_spread, 0.001, stable_spread)
        }
        CreditSpreadProcessSpec::MarketAnchored {
            credit_curve_id,
            kappa,
            implied_vol,
            tenor_years,
        } => {
            // Anchor both the initial state and the target average hazard at
            // the simulation date. Using the curve's first segment here would
            // reintroduce elapsed credit history for seasoned facilities.
            let hazard = market.get_hazard(credit_curve_id.as_str())?;
            let day_count = hazard.day_count();
            let base_date = hazard.base_date();
            let t_anchor = day_count.signed_year_fraction(
                base_date,
                simulation_anchor,
                DayCountContext::default(),
            )?;
            if t_anchor < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "hazard curve '{}' has base date {} after revolving-credit simulation anchor {}",
                    credit_curve_id, base_date, simulation_anchor
                )));
            }
            let remaining = day_count
                .year_fraction(
                    simulation_anchor,
                    facility.maturity,
                    DayCountContext::default(),
                )?
                .max(CIR_MIN_SPREAD);
            let t = tenor_years
                .unwrap_or(remaining)
                .min(remaining)
                .max(CIR_MIN_SPREAD);

            // Conditional survival and average hazard over [anchor, anchor+T],
            // and the hazard↔spread mapping, both from the shared
            // market-anchored helpers so the callable lattice and this Monte
            // Carlo path cannot drift apart.
            let sp_0 = hazard.sp(t_anchor).max(f64::MIN_POSITIVE);
            let sp_t = hazard.sp(t_anchor + t).max(f64::MIN_POSITIVE);
            let avg_lambda =
                finstack_quant_models::credit::market_anchored::conditional_average_hazard(
                    sp_0, sp_t, t,
                )?;
            let lambda0 = hazard.hazard_rate(t_anchor).max(0.0);

            // Credit triangle: s ≈ (1 − R) · λ, using the facility recovery
            // rate for consistency with pricing. The `1e-6` guard keeps a
            // recovery of exactly 1 from collapsing the spread scale.
            let one_minus_r = (1.0 - facility.recovery_rate).max(1e-6);
            let s0 = (one_minus_r * lambda0).max(CIR_MIN_SPREAD);
            let s_bar = (one_minus_r * avg_lambda).max(CIR_MIN_SPREAD);

            // Mean-anchored CIR params
            let k = kappa.max(CIR_MIN_SPREAD);
            let a = if (k * t).abs() < CIR_MIN_SPREAD {
                1.0 - 0.5 * k * t
            } else {
                (1.0 - (-k * t).exp()) / (k * t)
            };
            let theta = if (1.0 - a).abs() < 1e-12 {
                s_bar
            } else {
                ((s_bar - a * s0) / (1.0 - a)).max(CIR_MIN_SPREAD)
            };

            // CIR diffusion coefficient matching the local fractional vol at
            // the anchored spread: σ_CIR·√s_ref = σ_fractional·s_ref.
            let sigma = finstack_quant_models::credit::market_anchored::cir_diffusion_coefficient(
                *implied_vol,
                s_bar.max(CIR_MIN_SPREAD),
            )?;

            // Check Feller condition: 2κθ > σ²
            let feller_lhs = 2.0 * k * theta;
            let feller_rhs = sigma * sigma;
            let feller_ratio = feller_lhs / feller_rhs.max(CIR_MIN_SPREAD);

            if feller_ratio < 1.0 {
                tracing::warn!(
                    target: "finstack_quant_valuations::credit",
                    feller_ratio,
                    kappa = k,
                    theta,
                    sigma,
                    "market-anchored CIR Feller condition violated (2κθ/σ² < 1)"
                );
            }

            CreditSpreadParams::new(k, theta, sigma, s0)
        }
    }
}

/// Maximum time step for Monte Carlo simulation (in years).
///
/// Stochastic processes like CIR (credit spread) and Hull-White (rates) require
/// sufficiently fine time steps for numerical convergence and boundary stability.
/// A step of ~1 week (1/52 year) provides better accuracy for volatile processes.
const MAX_MC_TIME_STEP: f64 = 1.0 / 52.0; // ~1 week

/// Convert payment dates to time points (years from commitment date).
///
/// Uses the specified day count convention for consistent time fraction calculations
/// across the facility's cashflow engine and path generation.
fn dates_to_times(
    payment_dates: &[Date],
    commitment_date: Date,
    day_count: DayCount,
) -> Result<Vec<f64>> {
    payment_dates
        .iter()
        .map(|&date| day_count.year_fraction(commitment_date, date, DayCountContext::default()))
        .collect()
}

/// Result of refining a time grid.
///
/// Contains both the refined grid and a mapping from refined indices to
/// original payment date indices (for extracting state at payment dates only).
struct RefinedGrid {
    /// Refined time points with intermediate steps inserted
    times: Vec<f64>,
    /// Indices in the refined grid that correspond to original payment dates
    payment_indices: Vec<usize>,
}

/// Refine a time grid to ensure no step exceeds MAX_MC_TIME_STEP.
///
/// Inserts intermediate points between existing grid points where the step size
/// exceeds the maximum. This ensures stochastic process convergence without
/// modifying the original payment date alignment.
///
/// # Arguments
///
/// * `times` - Original time points (years from commitment date)
///
/// # Returns
///
/// A `RefinedGrid` containing the refined times and indices mapping back to
/// original payment dates.
fn refine_time_grid(times: &[f64]) -> RefinedGrid {
    if times.len() < 2 {
        return RefinedGrid {
            times: times.to_vec(),
            payment_indices: (0..times.len()).collect(),
        };
    }

    let mut refined = Vec::with_capacity(times.len() * 4); // Pre-allocate with margin
    let mut payment_indices = Vec::with_capacity(times.len());

    refined.push(times[0]);
    payment_indices.push(0);

    for i in 0..(times.len() - 1) {
        let t0 = times[i];
        let t1 = times[i + 1];
        let dt = t1 - t0;

        if dt > MAX_MC_TIME_STEP {
            let num_steps = (dt / MAX_MC_TIME_STEP).ceil() as usize;
            let step_size = dt / num_steps as f64;

            for j in 1..num_steps {
                refined.push(t0 + j as f64 * step_size);
            }
        }

        refined.push(t1);
        payment_indices.push(refined.len() - 1);
    }

    RefinedGrid {
        times: refined,
        payment_indices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use time::macros::date;

    #[test]
    fn market_anchored_credit_starts_at_simulation_anchor_hazard() {
        let mut facility = RevolvingCredit::example().expect("facility");
        facility.recovery_rate = 0.4;
        let market = MarketContext::new().insert(
            HazardCurve::builder("RC-HZ")
                .base_date(date!(2024 - 01 - 01))
                .knots([(0.5, 0.01), (5.0, 0.05)])
                .recovery_rate(0.40)
                .build()
                .expect("hazard curve"),
        );
        let config = McConfig {
            recovery_rate: 0.4,
            credit_spread_process: CreditSpreadProcessSpec::MarketAnchored {
                credit_curve_id: "RC-HZ".into(),
                kappa: 0.5,
                implied_vol: 0.2,
                tenor_years: None,
            },
            interest_rate_process: None,
            correlation_matrix: None,
            util_credit_corr: None,
        };

        let params = build_credit_spread_params(&config, &facility, &market, date!(2025 - 01 - 01))
            .expect("credit parameters");

        assert!((params.initial - 0.03).abs() < 1e-12);
    }

    /// Behavioral baseline for the market-anchored CIR parameters.
    ///
    /// Every field is re-derived here from the documented mapping rather
    /// than copied from a prior run, so the test states the contract the
    /// shared `models::credit::market_anchored` extraction must preserve:
    ///
    /// ```text
    /// avg_lambda = -ln(sp(t_a + T) / sp(t_a)) / T
    /// s_ref      = (1 - R) * lambda_ref            (credit triangle)
    /// a          = (1 - exp(-k T)) / (k T)
    /// theta      = (s_bar - a * s0) / (1 - a)      (mean-anchored CIR)
    /// sigma_CIR  = sigma_fractional * sqrt(s_bar)
    /// ```
    ///
    /// If the extraction changes any produced parameter this fails with the
    /// offending field named.
    #[test]
    fn market_anchored_parameters_match_documented_mapping() {
        let mut facility = RevolvingCredit::example().expect("facility");
        facility.recovery_rate = 0.4;
        let hazard = HazardCurve::builder("RC-HZ")
            .base_date(date!(2024 - 01 - 01))
            .knots([(0.5, 0.01), (5.0, 0.05)])
            .recovery_rate(0.40)
            .build()
            .expect("hazard curve");
        let market = MarketContext::new().insert(hazard.clone());
        let (kappa, implied_vol) = (0.5_f64, 0.2_f64);
        let config = McConfig {
            recovery_rate: 0.4,
            credit_spread_process: CreditSpreadProcessSpec::MarketAnchored {
                credit_curve_id: "RC-HZ".into(),
                kappa,
                implied_vol,
                tenor_years: None,
            },
            interest_rate_process: None,
            correlation_matrix: None,
            util_credit_corr: None,
        };
        let as_of = date!(2025 - 01 - 01);

        let params =
            build_credit_spread_params(&config, &facility, &market, as_of).expect("parameters");

        // Independent re-derivation of the documented mapping.
        let day_count = hazard.day_count();
        let t_anchor = day_count
            .signed_year_fraction(hazard.base_date(), as_of, DayCountContext::default())
            .expect("anchor year fraction");
        let horizon = day_count
            .year_fraction(as_of, facility.maturity, DayCountContext::default())
            .expect("remaining horizon");
        let sp_0 = hazard.sp(t_anchor);
        let sp_t = hazard.sp(t_anchor + horizon);
        let avg_lambda = -(sp_t / sp_0).ln() / horizon;
        let lambda0 = hazard.hazard_rate(t_anchor);
        let one_minus_r = 1.0 - facility.recovery_rate;
        let s0 = one_minus_r * lambda0;
        let s_bar = one_minus_r * avg_lambda;
        let a = (1.0 - (-kappa * horizon).exp()) / (kappa * horizon);
        let theta = (s_bar - a * s0) / (1.0 - a);
        let sigma = implied_vol * s_bar.sqrt();

        for (label, actual, expected) in [
            ("initial (s0)", params.initial, s0),
            ("kappa", params.cir.kappa, kappa),
            ("theta", params.cir.theta, theta),
            ("sigma", params.cir.sigma, sigma),
        ] {
            assert!(
                (actual - expected).abs() < 1e-12,
                "market-anchored {label} drifted: actual={actual}, \
                 documented mapping={expected}"
            );
        }

        // Recovery cancels out of the hazard-vol mapping under the same
        // triangle approximation: sigma_lambda = sigma_fractional * lambda_ref
        // equals sigma_s_abs / (1 - R). Pinning it here fixes the identity the
        // extracted helper must reproduce.
        let sigma_s_abs = implied_vol * s_bar;
        let sigma_lambda = implied_vol * avg_lambda;
        assert!(
            (sigma_lambda - sigma_s_abs / one_minus_r).abs() < 1e-15,
            "recovery must cancel: sigma_lambda={sigma_lambda}, \
             sigma_s_abs/(1-R)={}",
            sigma_s_abs / one_minus_r
        );
    }

    #[test]
    fn stochastic_hull_white_ignores_legacy_constant_seed_and_fits_curve() {
        let facility = RevolvingCredit::example().expect("facility");
        let as_of = date!(2024 - 01 - 01);
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(date!(2024 - 01 - 01))
                .day_count(DayCount::Act365F)
                .knots([
                    (0.0, 1.0),
                    (1.0, (-0.03_f64).exp()),
                    (5.0, (-0.15_f64).exp()),
                ])
                .build()
                .expect("discount curve"),
        );
        let config = McConfig {
            recovery_rate: facility.recovery_rate,
            credit_spread_process: CreditSpreadProcessSpec::Constant(0.0),
            interest_rate_process: Some(InterestRateProcessSpec::HullWhite1F {
                kappa: 0.1,
                sigma: 0.01,
                initial: 0.99,
                theta: 0.99,
            }),
            correlation_matrix: None,
            util_credit_corr: None,
        };
        let stochastic = StochasticUtilizationSpec {
            utilization_process: UtilizationProcess::MeanReverting {
                target_rate: 0.5,
                speed: 1.0,
                volatility: 0.1,
                spread_sensitivity: 0.0,
            },
            num_paths: 2,
            seed: Some(7),
            antithetic: false,
            use_sobol_qmc: false,
            mc_config: Some(config.clone()),
        };
        let dates =
            super::super::super::utils::build_accrual_boundary_dates(&facility).expect("dates");

        let paths =
            generate_three_factor_paths(&stochastic, &config, &facility, &market, &dates, as_of)
                .expect("paths");

        let initial_rate = paths[0].short_rate_path[0];
        assert!(
            (initial_rate - 0.03).abs() < 5e-4,
            "initial rate: {initial_rate}"
        );
        assert!((initial_rate - 0.99).abs() > 0.5);
    }
}

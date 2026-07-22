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
use rayon::prelude::*;

use crate::calibration::hull_white::HullWhiteParams;
use crate::instruments::fixed_income::revolving_credit::pricer::monte_carlo_discretization::RevolvingCreditDiscretization;
use crate::instruments::fixed_income::revolving_credit::pricer::monte_carlo_process::{
    CreditSpreadParams, InterestRateSpec, RevolvingCreditProcess, RevolvingCreditProcessParams,
    UtilizationParams,
};
use crate::instruments::rates::exotics_shared::{
    calibrate_hw1f_params, initial_short_rate_from_curve,
};
use finstack_quant_monte_carlo::process::ou::HullWhite1FParams;
use finstack_quant_monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_monte_carlo::rng::sobol::SobolRng;
use finstack_quant_monte_carlo::time_grid::TimeGrid;
use finstack_quant_monte_carlo::traits::{Discretization, RandomStream, StochasticProcess};

use super::super::cashflow_engine::ThreeFactorPathData;
use super::super::types::{
    BaseRateSpec, CreditSpreadProcessSpec, InterestRateProcessSpec, McConfig, RevolvingCredit,
    StochasticUtilizationSpec, UtilizationProcess,
};

/// Type alias for optional rate curve data (times and rates).
type RateCurveData = Option<(Vec<f64>, Vec<f64>)>;

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
/// * `payment_dates` - Payment schedule dates
/// * `as_of` - Valuation date; simulation starts here (not at the commitment
///   date) with the facility's current utilization as the known t₀ state.
///   Payment dates at or before `as_of` record the t₀ state.
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

    // Use facility's day count for consistent time calculations
    let day_count = facility.day_count;
    // All stochastic factors start at the valuation date for seasoned
    // facilities.  Keep path time on the facility axis, but retain the
    // simulation anchor so curve-fitted models can use their own t=0.
    let simulation_anchor = as_of.max(facility.commitment_date);
    let t_asof = if as_of > facility.commitment_date {
        day_count.year_fraction(facility.commitment_date, as_of, DayCountContext::default())?
    } else {
        0.0
    };
    // Build utilization parameters
    let (util_params, is_zero_vol) = match &stoch_spec.utilization_process {
        UtilizationProcess::MeanReverting {
            target_rate,
            speed,
            volatility,
        } => {
            // Handle zero volatility case (for deterministic/parity testing)
            let is_zero = volatility.abs() < 1e-8;
            let vol = if is_zero { 1e-10 } else { *volatility };
            (UtilizationParams::new(*speed, *target_rate, vol)?, is_zero)
        }
    };

    // Build interest rate specification
    let disc_curve = market.get_discount(facility.discount_curve_id.as_str())?;
    let (interest_rate_spec, rate_curve_opt, rate_time_offset): (
        InterestRateSpec,
        RateCurveData,
        f64,
    ) = match &facility.base_rate_spec {
        BaseRateSpec::Fixed { rate } => (InterestRateSpec::Fixed { rate: *rate }, None, 0.0),
        BaseRateSpec::Floating(spec) => {
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
                        let scalar = HullWhiteParams::new(*kappa, *sigma)?;
                        let params = calibrate_hw1f_params(
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
                                params: HullWhite1FParams::new(*kappa, *sigma, *theta),
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
                    (
                        InterestRateSpec::DeterministicForward {
                            times: times.clone(),
                            rates: rates.clone(),
                        },
                        Some((times, rates)),
                        curve_offset,
                    )
                }
            }
        }
    };

    // Build credit spread parameters
    let credit_spread_params =
        build_credit_spread_params(mc_config, facility, market, simulation_anchor)?;

    // Create 3-factor process with correlation
    let mut process_params =
        RevolvingCreditProcessParams::new(util_params, interest_rate_spec, credit_spread_params);

    if let Some(corr_matrix) = &mc_config.correlation_matrix {
        process_params = process_params.with_correlation(*corr_matrix);
    } else if let Some(rho) = mc_config.util_credit_corr {
        // Documented 2-factor shorthand: utilization–credit correlation ρ
        // with the rate factor uncorrelated. Previously this field was
        // accepted, validated, and then silently ignored.
        process_params =
            process_params.with_correlation([[1.0, 0.0, rho], [0.0, 1.0, 0.0], [rho, 0.0, 1.0]]);
    }

    process_params = process_params.with_time_offset(rate_time_offset);

    let process = RevolvingCreditProcess::new(process_params);

    // Whether the short-rate trajectory is genuinely stochastic (Hull-White
    // with σ > 0). The pricer uses this to choose pathwise bank-account
    // discounting over the static curve.
    let stochastic_rates = matches!(
        &process.params().interest_rate,
        InterestRateSpec::Floating { params, .. } if params.sigma > 0.0
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

    // Refine grid to ensure no step exceeds MAX_MC_TIME_STEP for numerical stability
    let refined = refine_time_grid(&sim_times);
    let time_grid = TimeGrid::from_times(refined.times.clone())?;

    // Set up discretization scheme
    let disc = RevolvingCreditDiscretization::new(process.correlation())?;

    // Prepare buffers for simulation
    let num_paths = stoch_spec.num_paths;
    let num_steps = time_grid.num_steps();
    let num_factors = process.num_factors();
    let initial_state = if sim_start == 0.0 {
        process.params().initial_state(facility.utilization_rate())
    } else {
        process
            .params()
            .initial_state_at(facility.utilization_rate(), sim_start)
    };
    let num_payment_dates = payment_dates.len();

    let mut paths = Vec::with_capacity(num_paths);
    let seed = stoch_spec.seed.unwrap_or(42);
    let use_sobol = stoch_spec.use_sobol_qmc;
    let use_antithetic = stoch_spec.antithetic && !use_sobol; // Antithetic not compatible with Sobol

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

            // For deterministic forward, set initial rate from curve on the
            // CURVE's time axis (market-t = time_offset + path-t).
            if let Some((ref times, ref rates)) = rate_curve_opt {
                state[1] = interpolate_rate(rate_time_offset + time_grid.times()[0], times, rates);
            }

            // Record the t₀ state for every payment date at/before as_of
            // (at least the first).
            for _ in 0..num_initial {
                utilization_path.push(state[0].clamp(0.0, 1.0));
                short_rate_path.push(state[1]);
                credit_spread_path.push(state[2].max(0.0));
            }

            // Track which simulation anchor we're recording next
            let mut next_payment_idx = 1;

            // Evolve through time on the refined grid
            for i in 0..num_steps {
                let t_next = time_grid.times()[i + 1];

                {
                    let t = time_grid.times()[i];
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

                // For deterministic forward, manually update short rate from
                // the curve on its own axis (market-t = time_offset + path-t).
                if let Some((ref times, ref rates)) = rate_curve_opt {
                    state[1] = interpolate_rate(rate_time_offset + t_next, times, rates);
                }

                // Only record state at payment dates (not intermediate steps)
                if next_payment_idx < refined.payment_indices.len()
                    && i + 1 == refined.payment_indices[next_payment_idx]
                    && utilization_path.len() < num_payment_dates
                {
                    utilization_path.push(state[0].clamp(0.0, 1.0));
                    short_rate_path.push(state[1]);
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
        let times_ref = time_grid.times();

        let chunked: Vec<Vec<ThreeFactorPathData>> = (0..num_iterations)
            .into_par_iter()
            .map(|iter_idx| {
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

                    if let Some((ref times, ref rates)) = rate_curve_opt {
                        state[1] = interpolate_rate(rate_time_offset + times_ref[0], times, rates);
                    }

                    // Record the t₀ state for every payment date at/before
                    // as_of (at least the first).
                    for _ in 0..num_initial {
                        utilization_path.push(state[0].clamp(0.0, 1.0));
                        short_rate_path.push(state[1]);
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

                        if let Some((ref times, ref rates)) = rate_curve_opt {
                            state[1] = interpolate_rate(rate_time_offset + t_next, times, rates);
                        }

                        if next_payment_idx < payment_indices_ref.len()
                            && i + 1 == payment_indices_ref[next_payment_idx]
                            && utilization_path.len() < num_payment_dates
                        {
                            utilization_path.push(state[0].clamp(0.0, 1.0));
                            short_rate_path.push(state[1]);
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
            })
            .collect();

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
fn build_credit_spread_params(
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
            hazard_curve_id,
            kappa,
            implied_vol,
            tenor_years,
        } => {
            // Anchor both the initial state and the target average hazard at
            // the simulation date. Using the curve's first segment here would
            // reintroduce elapsed credit history for seasoned facilities.
            let hazard = market.get_hazard(hazard_curve_id.as_str())?;
            let dc = hazard.day_count();
            let base_date = hazard.base_date();
            let t_anchor =
                dc.signed_year_fraction(base_date, simulation_anchor, DayCountContext::default())?;
            if t_anchor < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "hazard curve '{}' has base date {} after revolving-credit simulation anchor {}",
                    hazard_curve_id, base_date, simulation_anchor
                )));
            }
            let remaining = dc
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

            // Conditional survival and average hazard over [anchor, anchor+T].
            let sp_0 = hazard.sp(t_anchor).max(f64::MIN_POSITIVE);
            let sp_t = hazard.sp(t_anchor + t).max(f64::MIN_POSITIVE);
            let avg_lambda = (-(sp_t / sp_0).ln() / t).max(0.0);
            let lambda0 = hazard.hazard_rate(t_anchor).max(0.0);

            // Map hazard ↔ spread using s ≈ (1 − R) · λ
            // Use facility recovery rate for consistency with pricing
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

            // Volatility scaled to match fractional vol
            let sigma = (*implied_vol) * s_bar.max(CIR_MIN_SPREAD).sqrt();

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
            // Insert intermediate points
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

/// Interpolate rate from knot points (linear interpolation with binary search).
///
/// Uses `partition_point` for O(log n) interval lookup on sorted time grids,
/// with flat extrapolation beyond boundaries.
fn interpolate_rate(t: f64, times: &[f64], rates: &[f64]) -> f64 {
    if times.is_empty() {
        return 0.0;
    }
    if times.len() == 1 || t <= times[0] {
        return rates[0];
    }
    let n = times.len();
    if t >= times[n - 1] {
        return rates[n - 1];
    }

    // Binary search: find first index where times[idx] > t
    let idx = times.partition_point(|&ti| ti <= t);
    // idx is in [1, n-1] since t > times[0] and t < times[n-1]
    let i = idx.saturating_sub(1);
    let alpha = (t - times[i]) / (times[i + 1] - times[i]);
    rates[i] + alpha * (rates[i + 1] - rates[i])
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
                .build()
                .expect("hazard curve"),
        );
        let config = McConfig {
            recovery_rate: 0.4,
            credit_spread_process: CreditSpreadProcessSpec::MarketAnchored {
                hazard_curve_id: "RC-HZ".into(),
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

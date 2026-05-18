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

use finstack_core::dates::{Date, DayCount, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::Result;
use rayon::prelude::*;

use crate::instruments::fixed_income::revolving_credit::pricer::monte_carlo_discretization::RevolvingCreditDiscretization;
use crate::instruments::fixed_income::revolving_credit::pricer::monte_carlo_process::{
    CreditSpreadParams, InterestRateSpec, RevolvingCreditProcess, RevolvingCreditProcessParams,
    UtilizationParams,
};
use finstack_monte_carlo::process::ou::HullWhite1FParams;
use finstack_monte_carlo::rng::philox::PhiloxRng;
use finstack_monte_carlo::rng::sobol::SobolRng;
use finstack_monte_carlo::time_grid::TimeGrid;
use finstack_monte_carlo::traits::{Discretization, RandomStream, StochasticProcess};

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
) -> Result<Vec<ThreeFactorPathData>> {
    // Use facility's day count for consistent time calculations
    let day_count = facility.day_count;
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
    let (interest_rate_spec, rate_curve_opt): (InterestRateSpec, RateCurveData) =
        match &facility.base_rate_spec {
            BaseRateSpec::Fixed { rate } => (InterestRateSpec::Fixed { rate: *rate }, None),
            BaseRateSpec::Floating(spec) => {
                match &mc_config.interest_rate_process {
                    Some(InterestRateProcessSpec::HullWhite1F {
                        kappa,
                        sigma,
                        initial,
                        theta,
                    }) => (
                        InterestRateSpec::Floating {
                            params: HullWhite1FParams::new(*kappa, *sigma, *theta),
                            initial: *initial,
                        },
                        None,
                    ),
                    None => {
                        // Use deterministic forward curve
                        let fwd = market.get_forward(spec.index_id.as_str())?;
                        let times = fwd.knots().to_vec();
                        let rates = fwd.forwards().to_vec();
                        (
                            InterestRateSpec::DeterministicForward {
                                times: times.clone(),
                                rates: rates.clone(),
                            },
                            Some((times, rates)),
                        )
                    }
                }
            }
        };

    // Build credit spread parameters
    let credit_spread_params = build_credit_spread_params(mc_config, facility, market)?;

    // Create 3-factor process with correlation
    let mut process_params =
        RevolvingCreditProcessParams::new(util_params, interest_rate_spec, credit_spread_params);

    if let Some(corr_matrix) = &mc_config.correlation_matrix {
        process_params = process_params.with_correlation(*corr_matrix);
    }

    // Compute time offset for market curve alignment
    let disc_curve = market.get_discount(facility.discount_curve_id.as_str())?;
    let disc_dc = disc_curve.day_count();
    let base_date = disc_curve.base_date();
    let t_start = disc_dc.year_fraction(
        base_date,
        facility.commitment_date,
        DayCountContext::default(),
    )?;
    process_params = process_params.with_time_offset(t_start);

    let process = RevolvingCreditProcess::new(process_params);

    // Convert payment dates to time points using facility's day count
    let raw_time_points = dates_to_times(payment_dates, facility.commitment_date, day_count)?;

    // Refine grid to ensure no step exceeds MAX_MC_TIME_STEP for numerical stability
    let refined = refine_time_grid(&raw_time_points);
    let time_grid = TimeGrid::from_times(refined.times.clone())?;

    // Set up discretization scheme
    let disc = RevolvingCreditDiscretization::new(process.correlation())?;

    // Prepare buffers for simulation
    let num_paths = stoch_spec.num_paths;
    let num_steps = time_grid.num_steps();
    let num_factors = process.num_factors();
    let initial_state = process.params().initial_state(facility.utilization_rate());
    let num_payment_dates = payment_dates.len();

    let mut paths = Vec::with_capacity(num_paths);
    let seed = stoch_spec.seed.unwrap_or(42);
    let use_sobol = stoch_spec.use_sobol_qmc;
    let use_antithetic = stoch_spec.antithetic && !use_sobol; // Antithetic not compatible with Sobol

    // Allocate reusable buffers (used by the serial Sobol path; the parallel
    // Philox path allocates per-thread inside the rayon closure).
    let mut z = vec![0.0; num_factors];
    let mut work = vec![0.0; disc.work_size(&process)];

    if use_sobol {
        let mut rng = SobolRng::try_new(num_factors, seed)
            .map_err(|err| finstack_core::Error::Validation(err.to_string()))?;

        for _path_idx in 0..num_paths {
            let mut state = initial_state.to_vec();
            // Only record states at payment dates, not at intermediate simulation steps
            let mut utilization_path = Vec::with_capacity(num_payment_dates);
            let mut short_rate_path = Vec::with_capacity(num_payment_dates);
            let mut credit_spread_path = Vec::with_capacity(num_payment_dates);

            // For deterministic forward, set initial rate from curve
            if let Some((ref times, ref rates)) = rate_curve_opt {
                state[1] = interpolate_rate(time_grid.times()[0], times, rates);
            }

            // Record initial state (first payment date)
            utilization_path.push(state[0].clamp(0.0, 1.0));
            short_rate_path.push(state[1]);
            credit_spread_path.push(state[2].max(0.0));

            // Track which payment date index we're recording next
            let mut next_payment_idx = 1;

            // Evolve through time on the refined grid
            for i in 0..num_steps {
                let t_next = time_grid.times()[i + 1];

                if !is_zero_vol {
                    let t = time_grid.times()[i];
                    let dt = t_next - t;

                    // Generate random normal variates
                    rng.fill_std_normals(&mut z);

                    // Apply discretization scheme to evolve state
                    disc.step(&process, t, dt, &mut state, &z, &mut work);
                }
                // else: keep state constant for zero volatility

                // For deterministic forward, manually update short rate from curve
                if let Some((ref times, ref rates)) = rate_curve_opt {
                    state[1] = interpolate_rate(t_next, times, rates);
                }

                // Only record state at payment dates (not intermediate steps)
                if next_payment_idx < refined.payment_indices.len()
                    && i + 1 == refined.payment_indices[next_payment_idx]
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
                        state[1] = interpolate_rate(times_ref[0], times, rates);
                    }

                    utilization_path.push(state[0].clamp(0.0, 1.0));
                    short_rate_path.push(state[1]);
                    credit_spread_path.push(state[2].max(0.0));

                    let mut next_payment_idx = 1;

                    for (i, z_seq) in z_sequences.iter().enumerate().take(num_steps) {
                        let t_next = times_ref[i + 1];

                        if !is_zero_vol {
                            let t = times_ref[i];
                            let dt = t_next - t;

                            if sign_idx == 0 {
                                disc.step(&process, t, dt, &mut state, z_seq, &mut work);
                            } else {
                                for (j, val) in z_seq.iter().enumerate() {
                                    z_neg[j] = -val;
                                }
                                disc.step(&process, t, dt, &mut state, &z_neg, &mut work);
                            }
                        }

                        if let Some((ref times, ref rates)) = rate_curve_opt {
                            state[1] = interpolate_rate(t_next, times, rates);
                        }

                        if next_payment_idx < payment_indices_ref.len()
                            && i + 1 == payment_indices_ref[next_payment_idx]
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

    let _ = (z, work); // suppress unused-mut warnings for the Sobol-only buffers
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
                    target: "finstack_valuations::credit",
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
            // Pull hazard curve and compute tenor
            let hazard = market.get_hazard(hazard_curve_id.as_str())?;
            let dc = hazard.day_count();
            let base_date = hazard.base_date();

            let t_maturity =
                dc.year_fraction(base_date, facility.maturity, DayCountContext::default())?;
            let t = tenor_years.unwrap_or_else(|| t_maturity.max(CIR_MIN_SPREAD));

            // Survival and average hazard over [0,T]
            let sp_t = hazard.sp(t);
            let avg_lambda = if t > 0.0 { (-sp_t.ln()) / t } else { 0.0 };

            // Initial hazard from first segment
            let mut first_lambda = None;
            if let Some((_, lambda)) = hazard.knot_points().next() {
                first_lambda = Some(lambda.max(0.0));
            }
            let lambda0 = first_lambda.unwrap_or(avg_lambda).max(0.0);

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
                    target: "finstack_valuations::credit",
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

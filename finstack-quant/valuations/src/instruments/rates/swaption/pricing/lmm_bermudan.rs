//! Bermudan swaption pricing via LSMC with LMM/BGM dynamics.
//!
//! Uses the calibrated LMM process with predictor-corrector discretization and
//! Longstaff-Schwartz backward induction for optimal exercise decisions.
//!
//! # Product
//!
//! This prices the standard **co-terminal** Bermudan swaption: the right, at
//! each exercise date `T_k`, to enter the *remaining* swap `[T_k, T_N]`. Every
//! exercise date shares the common terminal date `T_N`. The exercise intrinsic
//! is therefore the genuine time-`T_k` value of the co-terminal swap, computed
//! from the forwards still alive at `T_k` (`start_idx = first_alive(T_k)`), not
//! the value of a fixed `[T_0, T_N]` swap.
//!
//! The payoff is evaluated entirely from forward rates in the path state,
//! making it naturally multi-curve-consistent (no short-rate reconstruction
//! needed). The simulation is conducted under the terminal measure with
//! `P(t, T_N)` as numeraire.
//!
//! # References
//!
//! - Longstaff, F. A. & Schwartz, E. S. (2001). "Valuing American Options
//!   by Simulation: A Simple Least-Squares Approach." *Review of Financial
//!   Studies*, 14(1), 113-147.
//! - Andersen, L. & Piterbarg, V. (2010). *Interest Rate Modeling*, Vol. 2,
//!   Ch. 15-16, Atlantic Financial Press. `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
//! - Glasserman, P. (2003). *Monte Carlo Methods in Financial Engineering*,
//!   Ch. 8, Springer. `docs/REFERENCES.md#glasserman-2004-monte-carlo`

use crate::instruments::rates::hw1f::hw1f_mc::{
    build_event_aligned_grid, money_estimate_from_pairs,
};
use crate::instruments::rates::hw1f::mc_config::RateExoticMcConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::Result;
use finstack_quant_models::monte_carlo::discretization::lmm_predictor_corrector::LmmPredictorCorrector;
use finstack_quant_models::monte_carlo::engine::MAX_NUM_PATHS;
use finstack_quant_models::monte_carlo::pricer::lsq::solve_least_squares;
use finstack_quant_models::monte_carlo::process::lmm::{LmmParams, LmmProcess};
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_models::monte_carlo::traits::{Discretization, RandomStream};
use finstack_quant_models::monte_carlo::TimeGrid;

const EXERCISE_TIME_TOLERANCE: f64 = 1e-10;

/// Price a Bermudan swaption using LMM dynamics and LSMC.
///
/// # Arguments
///
/// * `params` — Calibrated LMM parameters.
/// * `exercise_times` — Strictly increasing, unique year fractions at which
///   the holder may exercise. Every time must be finite and lie strictly
///   between zero and the terminal LMM tenor.
/// * `strike` — Fixed rate K of the underlying swap.
/// * `payer` — `true` for payer swaption, `false` for receiver.
/// * `notional` — Swap notional.
/// * `discount_factor_terminal` — `P(0, T_N)` for the terminal tenor.
/// * `currency` — Currency used for the result.
/// * `config` — Monte Carlo configuration. `num_paths` must not exceed
///   [`MAX_NUM_PATHS`]; antithetic sampling requires an even count, and the
///   pricer needs at least two independent pricing observations (2 paths for
///   plain in-sample pricing, 4 with either antithetic or out-of-sample
///   pricing, 8 with both). `min_steps_between_events` is the minimum number
///   of simulation sub-steps between consecutive exercise dates. Use
///   [`RateExoticMcConfig::lmm_bermudan`] for the registry defaults.
///
/// # Returns
///
/// A [`MoneyEstimate`] with the Bermudan swaption price and standard error.
///
/// # Errors
///
/// Returns an error if the terminal tenor is not finite and positive, if the
/// exercise schedule is empty, unordered, duplicated within `1e-10`, or has a
/// time outside `(0, maturity)`, if the path configuration cannot provide at
/// least two independent pricing observations, or if the LMM parameters are
/// inconsistent.
#[allow(clippy::too_many_arguments)]
pub fn price_bermudan_lmm(
    params: &LmmParams,
    exercise_times: &[f64],
    strike: f64,
    payer: bool,
    notional: f64,
    discount_factor_terminal: f64,
    currency: Currency,
    config: &RateExoticMcConfig,
) -> Result<MoneyEstimate> {
    let maturity = *params
        .tenors
        .last()
        .ok_or_else(|| finstack_quant_core::Error::Validation("empty tenors".to_string()))?;
    validate_path_config(config)?;
    let (time_grid, exercise_step_indices) =
        build_exercise_aligned_grid(exercise_times, maturity, config.min_steps_between_events)?;

    let n = params.num_forwards;
    let process = LmmProcess::new(params.clone());
    let disc = LmmPredictorCorrector::new();

    // Build time grid aligned to exercise dates and the final maturity
    // (forward fixing dates are NOT inserted as grid nodes; exercise dates
    // are snapped to the nearest node of the sub-divided grid).
    let num_steps = time_grid.num_steps();
    let work_size = disc.work_size(&process);

    let raw_paths = config.raw_stream_count();

    // --- Phase 1: Simulate forward rate paths ---
    //
    // paths[path_idx][step] = Vec<f64> of N forward rates at that step
    let mut all_paths: Vec<Vec<Vec<f64>>> = Vec::with_capacity(config.num_paths);
    let base_rng = PhiloxRng::new(config.seed);

    for path_id in 0..raw_paths {
        let mut rng = base_rng.substream(path_id as u64);
        let mut x = params.initial_forwards.clone();
        let mut work = vec![0.0; work_size];
        let mut z = vec![0.0; params.num_factors];

        let mut path_states = Vec::with_capacity(num_steps + 1);
        path_states.push(x.clone());

        for step in 0..num_steps {
            let t = time_grid.time(step);
            let dt = time_grid.dt(step);
            rng.fill_std_normals(&mut z);
            disc.step(&process, t, dt, &mut x, &z, &mut work);
            path_states.push(x.clone());
        }
        all_paths.push(path_states);

        if config.antithetic {
            // Antithetic path: replay with negated shocks
            let mut rng2 = base_rng.substream(path_id as u64);
            let mut x2 = params.initial_forwards.clone();
            let mut work2 = vec![0.0; work_size];
            let mut z2 = vec![0.0; params.num_factors];

            let mut path_states2 = Vec::with_capacity(num_steps + 1);
            path_states2.push(x2.clone());

            for step in 0..num_steps {
                let t = time_grid.time(step);
                let dt = time_grid.dt(step);
                rng2.fill_std_normals(&mut z2);
                for zz in z2.iter_mut() {
                    *zz = -*zz; // negate
                }
                disc.step(&process, t, dt, &mut x2, &z2, &mut work2);
                path_states2.push(x2.clone());
            }
            all_paths.push(path_states2);
        }
    }

    let total_paths = all_paths.len();

    // --- Phase 2: LSMC backward induction ---
    //
    // The simulation is under the terminal measure with numéraire
    // `P(t, T_N)`. The per-path estimator of a payoff realised at exercise
    // time `t` is therefore `payoff / P(t, T_N)`, deflated by the
    // *pathwise* terminal numéraire — and `cashflow[path_idx]` carries that
    // deflated, terminal-measure value. Because every entry is expressed in
    // the common `P(·, T_N)` accounting unit, deflated values from
    // different exercise dates are directly comparable in the
    // Longstaff-Schwartz continuation regression, and Phase 3 recovers the
    // time-0 price with a single `× P(0, T_N)`.
    let mut cashflow = vec![0.0_f64; total_paths];

    // Split-sample partition (see `RateExoticMcConfig::split`).
    let split = config.split();

    // Polynomial basis: [1, S, A, S^2, S*A, S^3, ...]
    let make_basis = |sr: f64, ann: f64| -> Vec<f64> {
        let mut b = Vec::with_capacity(config.basis_degree + 3);
        b.push(1.0);
        b.push(sr);
        b.push(ann);
        if config.basis_degree >= 2 {
            b.push(sr * sr);
            b.push(sr * ann);
        }
        if config.basis_degree >= 3 {
            b.push(sr * sr * sr);
        }
        b
    };

    // Iterate backward through exercise dates
    for ex_idx in (0..exercise_step_indices.len()).rev() {
        let step = exercise_step_indices[ex_idx];
        let t_exercise = time_grid.time(step);

        let mut exercise_values = Vec::with_capacity(total_paths);
        let mut basis_inputs = Vec::with_capacity(total_paths);

        // Index of the first forward still alive at this exercise date.
        // A forward `j` is alive while its fixing date `T_j >= t`. The
        // co-terminal swap entered on exercise at `T_k` covers exactly the
        // periods `[T_{first_alive}, T_N]`.
        let first_alive = first_alive_forward(&params.tenors[..n], t_exercise);

        for path in &all_paths {
            let forwards = &path[step];
            // Numerator: the *genuine time-`t`* co-terminal swap value.
            //
            // A Bermudan SWAPTION exercised at `T_k` confers the right to
            // enter the swap `[T_k, T_N]` — the *remaining* swap, not the
            // full `[T_0, T_N]` swap. `compute_swap_rate_and_annuity` with
            // `start_idx = first_alive` returns `S_t` and the annuity
            // `A_t = Σ_{j>=first_alive} τ_j P(t,T_{j+1})`, both discounted
            // to time `t`. Hence `intrinsic = (S_t-K)·A_t·N` is a genuine
            // *time-`t`* quantity (reference date `t`).
            let (swap_rate, annuity) =
                compute_swap_rate_and_annuity(forwards, &params.accrual_factors, first_alive, n);
            let intrinsic = if payer {
                (swap_rate - strike) * annuity * notional
            } else {
                (strike - swap_rate) * annuity * notional
            };
            // Deflator: the pathwise terminal-measure numéraire
            // `P(t,T_N) = Π_{j>=first_alive} 1/(1+τ_j F_j)` — also a
            // *time-`t`* quantity, built from the same `first_alive`
            // forwards. Numerator and deflator therefore share reference
            // date `t`, so the terminal-measure identity
            //   V_0 = P(0,T_N) · E^{T_N}[ H_t / P(t,T_N) ]
            // holds with `H_t` the genuine time-`t` co-terminal swap value:
            //   intrinsic / P(t,T_N) = (S_t-K)·A_t·N / P(t,T_N).
            // (Hard-coding `start_idx = 0` would make the numerator the
            // `T_0`-referenced `P(T_0,t)·Swap_t`, leaving a spurious
            // `P(T_0,t)` factor against this `t`-referenced deflator.)
            let numeraire = pathwise_terminal_numeraire(forwards, params, t_exercise);
            let deflated = if numeraire > 0.0 {
                intrinsic / numeraire
            } else {
                0.0
            };
            exercise_values.push(deflated);

            // Regression features are the (un-deflated) state variables:
            // forward swap rate and annuity.
            basis_inputs.push((swap_rate, annuity));
        }

        if ex_idx == exercise_step_indices.len() - 1 {
            // Last exercise date: exercise if intrinsic > 0
            for (i, &ev) in exercise_values.iter().enumerate() {
                if ev > 0.0 {
                    cashflow[i] = ev;
                }
            }
        } else {
            // Interior exercise date: regress the continuation value.
            //
            // No explicit time-stepping discount factor is applied here:
            // `cashflow[i]` already holds the terminal-measure-deflated
            // value `payoff / P(t', T_N)` from the future exercise step,
            // and the current `exercise_values[i]` are deflated the same
            // way, so exercise vs continuation is compared in a single
            // consistent accounting unit.

            // Collect ITM train paths for regression. In split-sample mode
            // only train paths feed the fit; the fitted rule is applied to
            // every ITM path below so price paths get it out-of-sample.
            let mut itm_indices = Vec::new();
            let mut itm_basis = Vec::new();
            let mut itm_continuation = Vec::new();

            for (i, &ev) in exercise_values.iter().enumerate() {
                if ev > 0.0 && split.is_train(i) {
                    itm_indices.push(i);
                    let (sr, ann) = basis_inputs[i];
                    itm_basis.push(make_basis(sr, ann));
                    itm_continuation.push(cashflow[i]);
                }
            }

            if itm_indices.len() > config.basis_degree + 3 {
                // Solve least-squares regression. A regression failure is a
                // hard error (matching the HW1F LSMC harness) rather than a
                // silent skip that would leave biased continuation values.
                let num_basis = itm_basis.first().map_or(0, |b| b.len());
                let mut a_matrix = vec![0.0; itm_indices.len() * num_basis];
                for (row, basis) in itm_basis.iter().enumerate() {
                    for (col, &val) in basis.iter().enumerate() {
                        a_matrix[row * num_basis + col] = val;
                    }
                }

                let coeffs = solve_least_squares(
                    &a_matrix,
                    &itm_continuation,
                    itm_indices.len(),
                    num_basis,
                )?;
                // Apply the fitted rule to every ITM path (train and price).
                for (i, &ev) in exercise_values.iter().enumerate() {
                    if ev <= 0.0 {
                        continue;
                    }
                    let (sr, ann) = basis_inputs[i];
                    let basis = make_basis(sr, ann);
                    let cont_value: f64 = basis.iter().zip(coeffs.iter()).map(|(b, c)| b * c).sum();
                    if ev > cont_value {
                        cashflow[i] = ev;
                    }
                    // else keep existing cashflow (continuation)
                }
            } else {
                // Too few ITM train paths for regression: exercise if positive
                for (i, &ev) in exercise_values.iter().enumerate() {
                    if ev > 0.0 && ev > cashflow[i] {
                        cashflow[i] = ev;
                    }
                }
            }
        }
    }

    // --- Phase 3: Recover the time-0 price ---
    //
    // `cashflow[i]` is the terminal-measure-deflated payoff
    // `payoff / P(t, T_N)`. The terminal-measure pricing identity
    // `V(0) = P(0, T_N) · E^{T_N}[ payoff / P(t, T_N) ]` then recovers the
    // present value with a single multiplication by the constant
    // `discount_factor_terminal = P(0, T_N)`.
    money_estimate_from_pairs(&cashflow, split, discount_factor_terminal, currency)
}

/// Build a time grid with steps aligned to exercise dates.
///
/// Public so reference simulations (e.g. no-arbitrage bound tests) can
/// replay the *identical* grid the [`price_bermudan_lmm`] engine uses.
///
/// # Arguments
///
/// * `exercise_times` — Strictly increasing, unique exercise times in year
///   fractions. Every time must be finite and lie in `(0, maturity)`.
/// * `maturity` — Finite positive terminal maturity in years.
/// * `min_steps_between` — Minimum number of simulation intervals between
///   adjacent critical dates. Values below one are treated as one.
///
/// # Returns
///
/// The simulation grid and the exact grid index corresponding to each input
/// exercise time, in caller-supplied order.
///
/// # Errors
///
/// Returns an error if `maturity` is not finite and positive, or if the
/// exercise schedule is empty, unordered, duplicated within `1e-10`, or has a
/// time outside `(0, maturity)`.
pub fn build_exercise_aligned_grid(
    exercise_times: &[f64],
    maturity: f64,
    min_steps_between: usize,
) -> Result<(TimeGrid, Vec<usize>)> {
    validate_exercise_schedule(exercise_times, maturity)?;
    build_event_aligned_grid(exercise_times, maturity, min_steps_between)
}

fn validate_exercise_schedule(exercise_times: &[f64], maturity: f64) -> Result<()> {
    if !maturity.is_finite() || maturity <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "LMM Bermudan maturity must be finite and positive, got {maturity}"
        )));
    }
    if exercise_times.is_empty() {
        return Err(finstack_quant_core::Error::Validation(
            "LMM Bermudan exercise schedule must not be empty".to_string(),
        ));
    }

    for (index, &exercise_time) in exercise_times.iter().enumerate() {
        if !exercise_time.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "LMM Bermudan exercise time at index {index} must be finite, got {exercise_time}"
            )));
        }
        if exercise_time <= 0.0 || exercise_time >= maturity {
            return Err(finstack_quant_core::Error::Validation(format!(
                "LMM Bermudan exercise time at index {index} must lie strictly inside (0, {maturity}), got {exercise_time}"
            )));
        }

        if let Some(&previous) = index.checked_sub(1).and_then(|i| exercise_times.get(i)) {
            let separation = exercise_time - previous;
            if separation.abs() <= EXERCISE_TIME_TOLERANCE {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "LMM Bermudan exercise times at indices {} and {index} are duplicates within {EXERCISE_TIME_TOLERANCE:e}",
                    index - 1
                )));
            }
            if separation < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "LMM Bermudan exercise times must be strictly increasing; index {} is {previous} but index {index} is {exercise_time}",
                    index - 1
                )));
            }
        }
    }

    Ok(())
}

fn validate_path_config(config: &RateExoticMcConfig) -> Result<()> {
    if config.num_paths > MAX_NUM_PATHS {
        return Err(finstack_quant_core::Error::Validation(format!(
            "LMM Bermudan num_paths {} exceeds maximum {MAX_NUM_PATHS}",
            config.num_paths
        )));
    }
    if config.antithetic && !config.num_paths.is_multiple_of(2) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "LMM Bermudan antithetic sampling requires an even num_paths, got {}",
            config.num_paths
        )));
    }

    let multiplicity = config.split().multiplicity;
    let raw_streams = config.num_paths / multiplicity;
    let pricing_observations = if config.oos_lsmc {
        raw_streams / 2
    } else {
        raw_streams
    };
    if pricing_observations < 2 {
        let minimum_paths = if config.oos_lsmc {
            4 * multiplicity
        } else {
            2 * multiplicity
        };
        return Err(finstack_quant_core::Error::Validation(format!(
            "LMM Bermudan pricing requires at least two independent pricing observations; num_paths must be at least {minimum_paths} for antithetic={} and oos_lsmc={}, got {}",
            config.antithetic, config.oos_lsmc, config.num_paths
        )));
    }

    Ok(())
}

/// Compute forward swap rate and annuity from forward rates.
///
/// For the swap covering periods `[start_idx, end_idx)`:
/// - Swap rate `S = (1 - P(T_start, T_end)) / A`
/// - Annuity `A = Σ τ_j P(T_start, T_{j+1})`
fn compute_swap_rate_and_annuity(
    forwards: &[f64],
    accrual_factors: &[f64],
    start_idx: usize,
    end_idx: usize,
) -> (f64, f64) {
    // Discount factors from T_start: P(T_start, T_j) = Π_{k=start}^{j-1} 1/(1+τ_k F_k)
    let count = end_idx - start_idx;
    let mut df = vec![1.0; count + 1];
    for k in 1..=count {
        let abs_k = start_idx + k - 1;
        df[k] = df[k - 1] / (1.0 + accrual_factors[abs_k] * forwards[abs_k]);
    }

    let mut annuity = 0.0;
    for j in 0..count {
        annuity += accrual_factors[start_idx + j] * df[j + 1];
    }

    // Swap rate: S = (1 - P(T_start, T_end)) / A
    let swap_rate = if annuity.abs() > 1e-15 {
        (1.0 - df[count]) / annuity
    } else {
        0.0
    };

    (swap_rate, annuity)
}

/// Index of the first forward still *alive* at time `t`.
///
/// A forward `j` is alive while its fixing date satisfies `T_j >= t`, up to
/// a small tolerance: tenors that sit a floating-point rounding error below
/// `t` (e.g. a schedule date reconstructed via repeated `+= period` that
/// lands at `t − 1e-12`) are treated as alive rather than expired. Without
/// the snap, such a perturbation silently drops the first swap period from
/// the co-terminal swap.
pub(crate) fn first_alive_forward(tenors: &[f64], t: f64) -> usize {
    tenors.partition_point(|&tenor| tenor + 1e-8 < t)
}

/// Pathwise terminal-measure numéraire `P(t, T_N)` at time `t`.
///
/// Under the LMM terminal measure the numéraire is the zero-coupon bond
/// `P(t, T_N)`. From the forward rates still *alive* at time `t` it is the
/// product of single-period discount factors
///
/// ```text
/// P(t, T_N) = Π_{j = first_alive(t)}^{N-1} 1 / (1 + τ_j F_j(t))
/// ```
///
/// A forward `j` is alive while its fixing date `T_j >= t`. This matches
/// the numéraire `LmmProcess::populate_path_state` records as
/// `"lmm_numeraire"`; it is recomputed here because the engine works with
/// raw forward-rate path states rather than `PathState`.
fn pathwise_terminal_numeraire(forwards: &[f64], params: &LmmParams, t: f64) -> f64 {
    let n = params.num_forwards;
    // First alive forward: the first index with tenor T_j >= t.
    let first_alive = first_alive_forward(&params.tenors[..n], t);
    let mut numeraire = 1.0;
    for (fwd, tau) in forwards[first_alive..n]
        .iter()
        .zip(&params.accrual_factors[first_alive..n])
    {
        numeraire /= 1.0 + tau * fwd;
    }
    numeraire
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_lmm_params() -> LmmParams {
        LmmParams {
            num_forwards: 4,
            num_factors: 2,
            tenors: vec![0.0, 1.0, 2.0, 3.0, 4.0],
            accrual_factors: vec![1.0, 1.0, 1.0, 1.0],
            displacements: vec![0.005; 4],
            vol_times: vec![],
            vol_values: vec![vec![
                [0.12, 0.04, 0.0],
                [0.11, 0.05, 0.0],
                [0.10, 0.06, 0.0],
                [0.09, 0.07, 0.0],
            ]],
            initial_forwards: vec![0.03, 0.032, 0.034, 0.036],
        }
        .validate()
        .expect("valid params")
    }

    fn test_config(num_paths: usize, antithetic: bool, oos_lsmc: bool) -> RateExoticMcConfig {
        RateExoticMcConfig {
            num_paths,
            seed: 7,
            basis_degree: 2,
            antithetic,
            min_steps_between_events: 1,
            oos_lsmc,
        }
    }

    fn assert_validation_contains<T: std::fmt::Debug>(result: Result<T>, expected: &str) {
        match result {
            Err(finstack_quant_core::Error::Validation(message)) => assert!(
                message.contains(expected),
                "validation message {message:?} did not contain {expected:?}"
            ),
            other => panic!("expected validation error containing {expected:?}, got {other:?}"),
        }
    }

    #[test]
    fn test_swap_rate_computation() {
        let forwards = vec![0.03, 0.035, 0.04];
        let taus = vec![1.0, 1.0, 1.0];
        let (sr, ann) = compute_swap_rate_and_annuity(&forwards, &taus, 0, 3);

        // Annuity = τ_0 df_1 + τ_1 df_2 + τ_2 df_3
        let df1 = 1.0 / 1.03;
        let df2 = df1 / 1.035;
        let df3 = df2 / 1.04;
        let expected_ann = df1 + df2 + df3;
        assert!((ann - expected_ann).abs() < 1e-10);

        let expected_sr = (1.0 - df3) / expected_ann;
        assert!((sr - expected_sr).abs() < 1e-10);
    }

    #[test]
    fn first_alive_forward_snaps_rounding_error_tenors() {
        let tenors = [0.0, 1.0, 2.0, 3.0];
        // Exact hit: T_1 == t → alive (T_j >= t).
        assert_eq!(first_alive_forward(&tenors, 1.0), 1);
        // Tenor perturbed a rounding error BELOW t must still count as alive.
        let tenors_perturbed = [0.0, 1.0 - 1e-12, 2.0, 3.0];
        assert_eq!(
            first_alive_forward(&tenors_perturbed, 1.0),
            1,
            "a 1e-12 perturbation must not expire the first forward"
        );
        // Genuinely expired tenor is excluded.
        assert_eq!(first_alive_forward(&tenors, 1.5), 2);
    }

    #[test]
    fn test_exercise_aligned_grid() {
        let exercise_times = vec![1.0, 2.0, 3.0];
        let (grid, indices) = build_exercise_aligned_grid(&exercise_times, 4.0, 4).expect("ok");
        assert!(grid.num_steps() >= 4);
        assert_eq!(indices.len(), 3);
        // Every exercise time is an exact critical grid point.
        for (i, &idx) in indices.iter().enumerate() {
            assert_eq!(grid.time(idx), exercise_times[i]);
        }
    }

    #[test]
    fn exercise_schedule_rejects_invalid_times_without_normalizing() {
        let cases = [
            (Vec::new(), 4.0, "must not be empty"),
            (vec![f64::NAN], 4.0, "must be finite"),
            (vec![f64::INFINITY], 4.0, "must be finite"),
            (vec![0.0], 4.0, "strictly inside"),
            (vec![-0.5], 4.0, "strictly inside"),
            (vec![4.0], 4.0, "strictly inside"),
            (vec![4.5], 4.0, "strictly inside"),
            (vec![1.0, 1.0], 4.0, "duplicates"),
            (vec![1.0, 1.0 + 5e-11], 4.0, "duplicates"),
            (vec![2.0, 1.0], 4.0, "strictly increasing"),
        ];

        for (exercise_times, maturity, expected) in cases {
            assert_validation_contains(
                build_exercise_aligned_grid(&exercise_times, maturity, 1),
                expected,
            );
        }
    }

    #[test]
    fn exercise_schedule_rejects_invalid_maturity() {
        for maturity in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_validation_contains(
                build_exercise_aligned_grid(&[0.5], maturity, 1),
                "maturity must be finite and positive",
            );
        }
    }

    #[test]
    fn path_config_requires_two_independent_pricing_observations() {
        let invalid = [
            (test_config(0, false, false), "at least two"),
            (test_config(1, false, false), "at least two"),
            (test_config(2, false, true), "at least two"),
            (test_config(3, false, true), "at least two"),
            (test_config(2, true, false), "at least two"),
            (test_config(4, true, true), "at least two"),
            (test_config(6, true, true), "at least two"),
            (test_config(3, true, false), "even num_paths"),
            (
                test_config(MAX_NUM_PATHS + 1, false, false),
                "exceeds maximum",
            ),
        ];
        for (config, expected) in invalid {
            assert_validation_contains(validate_path_config(&config), expected);
        }

        for config in [
            test_config(2, false, false),
            test_config(4, true, false),
            test_config(4, false, true),
            test_config(8, true, true),
        ] {
            assert!(
                validate_path_config(&config).is_ok(),
                "minimum valid configuration should be accepted: {config:?}"
            );
        }
    }

    #[test]
    fn public_pricer_rejects_invalid_path_config_before_simulation() {
        let params = test_lmm_params();
        let config = test_config(0, false, false);
        assert_validation_contains(
            price_bermudan_lmm(
                &params,
                &[1.0],
                0.03,
                true,
                1_000_000.0,
                0.9,
                Currency::USD,
                &config,
            ),
            "at least two independent pricing observations",
        );
    }

    #[test]
    fn test_bermudan_price_positive() {
        let params = test_lmm_params();
        let exercise_times = vec![1.0, 2.0, 3.0];
        let strike = 0.025; // ITM payer swaption (forwards ~3-3.6%)
        let df_terminal = (-0.03 * 4.0_f64).exp();
        let config = RateExoticMcConfig {
            num_paths: 5_000,
            seed: 123,
            basis_degree: 2,
            antithetic: true,
            min_steps_between_events: 4,
            oos_lsmc: false,
        };

        let result = price_bermudan_lmm(
            &params,
            &exercise_times,
            strike,
            true, // payer
            1_000_000.0,
            df_terminal,
            Currency::USD,
            &config,
        );

        assert!(result.is_ok(), "pricing failed: {result:?}");
        let estimate = result.expect("ok");
        assert!(
            estimate.mean.amount() > 0.0,
            "ITM payer swaption should have positive value: {}",
            estimate.mean.amount()
        );
    }

    #[test]
    fn test_bermudan_geq_european() {
        // Bermudan (3 exercise dates) should be >= European (1 exercise date)
        let params = test_lmm_params();
        let strike = 0.030;
        let df_terminal = (-0.03 * 4.0_f64).exp();
        let config = RateExoticMcConfig {
            num_paths: 10_000,
            seed: 42,
            basis_degree: 2,
            antithetic: true,
            min_steps_between_events: 4,
            oos_lsmc: false,
        };

        let european = price_bermudan_lmm(
            &params,
            &[1.0], // single exercise = European
            strike,
            true,
            1_000_000.0,
            df_terminal,
            Currency::USD,
            &config,
        )
        .expect("european ok");

        let bermudan = price_bermudan_lmm(
            &params,
            &[1.0, 2.0, 3.0], // three exercise dates
            strike,
            true,
            1_000_000.0,
            df_terminal,
            Currency::USD,
            &config,
        )
        .expect("bermudan ok");

        // Allow for MC noise: Bermudan should be approximately >= European
        let euro_val = european.mean.amount();
        let berm_val = bermudan.mean.amount();
        let tolerance = 3.0 * (european.stderr + bermudan.stderr);
        assert!(
            berm_val >= euro_val - tolerance,
            "Bermudan ({berm_val:.2}) should be >= European ({euro_val:.2}) within MC noise"
        );
    }

    /// Split-sample (out-of-sample) LSMC aggregates over the pricing half
    /// only and stays consistent with the in-sample estimate within MC noise.
    #[test]
    fn test_bermudan_oos_lsmc_uses_pricing_half() {
        let params = test_lmm_params();
        let exercise_times = vec![1.0, 2.0, 3.0];
        let strike = 0.025;
        let df_terminal = (-0.03 * 4.0_f64).exp();
        let base = RateExoticMcConfig {
            num_paths: 8_000,
            seed: 123,
            basis_degree: 2,
            antithetic: false,
            min_steps_between_events: 4,
            oos_lsmc: false,
        };
        let oos = RateExoticMcConfig {
            oos_lsmc: true,
            ..base
        };

        let in_sample = price_bermudan_lmm(
            &params,
            &exercise_times,
            strike,
            true,
            1_000_000.0,
            df_terminal,
            Currency::USD,
            &base,
        )
        .expect("in-sample ok");
        let out_sample = price_bermudan_lmm(
            &params,
            &exercise_times,
            strike,
            true,
            1_000_000.0,
            df_terminal,
            Currency::USD,
            &oos,
        )
        .expect("oos ok");

        assert_eq!(in_sample.num_paths, 8_000);
        assert_eq!(out_sample.num_paths, 4_000, "OOS prices on half the paths");
        let tol = 4.0 * (in_sample.stderr + out_sample.stderr);
        assert!(
            (in_sample.mean.amount() - out_sample.mean.amount()).abs() <= tol,
            "in-sample {} and OOS {} should agree within MC noise {tol}",
            in_sample.mean.amount(),
            out_sample.mean.amount()
        );
    }
}

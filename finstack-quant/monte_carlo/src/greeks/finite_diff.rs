//! Finite differences with Common Random Numbers (CRN).
//!
//! Computes Greeks by bump-and-revalue using the same random numbers for base
//! and bumped scenarios. This reduces variance significantly compared with an
//! independent re-run.
//!
//! # CRN invariant
//!
//! CRN here relies on a **splittable, counter-based RNG** whose `split(i)`
//! output depends only on the seed, never on how much of the stream has been
//! consumed. Philox (the default [`crate::rng::philox::PhiloxRng`]) satisfies
//! this; Sobol explicitly does not. These helpers therefore guard at runtime
//! via [`RandomStream::supports_splitting`] to prevent silent CRN breakage.
//!
//! # Reported standard errors are conservative
//!
//! The `stderr` returned by [`finite_diff_delta`] and [`finite_diff_gamma`]
//! combines the per-run MC standard errors **as if the bumped and base runs
//! were statistically independent**:
//!
//! ```text
//! se(Δ̂) ≈ √(se_up² + se_down²) / (2h)
//! se(Γ̂) ≈ √(se_up² + 4·se_base² + se_down²) / h²
//! ```
//!
//! CRN introduces strong positive correlation between the paired estimators,
//! so the *true* variance of the difference is almost always smaller — often
//! by one to two orders of magnitude for smooth payoffs. The quantity we
//! report is therefore an **upper bound** on the CRN stderr, not the CRN
//! stderr itself. A tight CRN stderr requires per-path pairing of the bumped
//! and base path values, which is not exposed through the current
//! [`McEngine::price`] API. Treat these numbers as safe for sizing error
//! budgets but not as an accurate diagnostic of the finite-difference noise.

use super::super::engine::McEngine;
use crate::engine::build_correlation_factor;
use crate::online_stats::OnlineStats;
use crate::traits::Payoff;
use crate::traits::{Discretization, RandomStream, StochasticProcess};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::Result;

const MIN_SPOT_FOR_BUMP: f64 = 1.0e-12;

/// Guard that the supplied RNG supports deterministic splitting, which is a
/// prerequisite for CRN across bump-and-revalue calls.
fn require_splittable_rng<R: RandomStream>(rng: &R, routine: &str) -> Result<()> {
    if !rng.supports_splitting() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{routine} requires an RNG that supports deterministic splitting (e.g. PhiloxRng); \
             the supplied generator reports supports_splitting() = false. Without stream \
             splitting the bumped and base valuations consume different random numbers and CRN \
             variance reduction is lost."
        )));
    }
    Ok(())
}

/// Compute a symmetric-difference bump size in absolute price units.
///
/// The `bump_size` argument is interpreted as a *relative* bump (e.g. `0.01`
/// for 1 % of spot). A floor of `1e-8` keeps the bump numerically meaningful
/// for near-zero spots (rates, FX implied yields, etc.).
fn bump_amount(initial_spot: f64, bump_size: f64) -> f64 {
    (initial_spot.abs() * bump_size).max(1e-8)
}

fn bumped_down_spot(initial_spot: f64, h: f64) -> f64 {
    (initial_spot - h).max(MIN_SPOT_FOR_BUMP)
}

/// Compute delta using central finite differences with CRN.
///
/// ```text
/// Δ ≈ (V(S₀+h) − V(S₀−h)) / (2h)
/// ```
///
/// Both valuations reuse `rng` by reference. This works only when the RNG is
/// splittable (e.g. [`crate::rng::philox::PhiloxRng`]) so that `rng.split(i)`
/// produces identical per-path streams across calls regardless of how much of
/// the parent stream has been consumed.
///
/// # Arguments
///
/// * `engine` - MC engine (borrowed, used for the two price runs)
/// * `rng` - Random number generator; must support splitting
/// * `process` - Stochastic process
/// * `disc` - Discretization scheme
/// * `initial_spot` - Initial spot price (S₀)
/// * `payoff` - Payoff specification
/// * `currency` - Currency tag
/// * `discount_factor` - Discount factor
/// * `bump_size` - Relative bump (e.g. `0.01` for 1 %)
///
/// # Returns
///
/// `(delta, stderr)` — the central-difference estimator and its standard
/// error under the assumption of independence between the up and down runs
/// (conservative; CRN makes the true stderr smaller but computing it exactly
/// would require per-path pairing outside this helper).
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when the supplied RNG does
/// not support splitting or when either `engine.price` call fails.
#[allow(clippy::too_many_arguments)]
pub fn finite_diff_delta<R, P, D, F>(
    engine: &McEngine,
    rng: &R,
    process: &P,
    disc: &D,
    initial_spot: f64,
    payoff: &F,
    currency: Currency,
    discount_factor: f64,
    bump_size: f64,
) -> Result<(f64, f64)>
where
    R: RandomStream,
    P: StochasticProcess,
    D: Discretization<P>,
    F: Payoff,
{
    require_splittable_rng(rng, "finite_diff_delta")?;
    let h = bump_amount(initial_spot, bump_size);

    let initial_up = vec![initial_spot + h];
    let result_up = engine.price(
        rng,
        process,
        disc,
        &initial_up,
        payoff,
        currency,
        discount_factor,
    )?;

    let initial_down = vec![bumped_down_spot(initial_spot, h)];
    let result_down = engine.price(
        rng,
        process,
        disc,
        &initial_down,
        payoff,
        currency,
        discount_factor,
    )?;

    let delta = (result_up.mean.amount() - result_down.mean.amount()) / (2.0 * h);
    // Conservative stderr under the independence assumption. Propagating a
    // tighter CRN stderr would require per-path bookkeeping not exposed
    // through the current engine API.
    let se_up = result_up.stderr;
    let se_down = result_down.stderr;
    let stderr = (se_up * se_up + se_down * se_down).sqrt() / (2.0 * h);

    Ok((delta, stderr))
}

/// Compute gamma using a second central difference with CRN.
///
/// ```text
/// Γ ≈ (V(S₀+h) − 2·V(S₀) + V(S₀−h)) / h²
/// ```
///
/// # Returns
///
/// `(gamma, stderr)` — the second-difference estimator and a conservative
/// standard error under the assumption of independent MC stderrs at the
/// three grid points.
#[allow(clippy::too_many_arguments)]
pub fn finite_diff_gamma<R, P, D, F>(
    engine: &McEngine,
    rng: &R,
    process: &P,
    disc: &D,
    initial_spot: f64,
    payoff: &F,
    currency: Currency,
    discount_factor: f64,
    bump_size: f64,
) -> Result<(f64, f64)>
where
    R: RandomStream,
    P: StochasticProcess,
    D: Discretization<P>,
    F: Payoff,
{
    require_splittable_rng(rng, "finite_diff_gamma")?;
    let h = bump_amount(initial_spot, bump_size);

    let initial_base = vec![initial_spot];
    let result_base = engine.price(
        rng,
        process,
        disc,
        &initial_base,
        payoff,
        currency,
        discount_factor,
    )?;

    let initial_up = vec![initial_spot + h];
    let result_up = engine.price(
        rng,
        process,
        disc,
        &initial_up,
        payoff,
        currency,
        discount_factor,
    )?;

    let initial_down = vec![bumped_down_spot(initial_spot, h)];
    let result_down = engine.price(
        rng,
        process,
        disc,
        &initial_down,
        payoff,
        currency,
        discount_factor,
    )?;

    let gamma = (result_up.mean.amount() - 2.0 * result_base.mean.amount()
        + result_down.mean.amount())
        / (h * h);
    let se_up = result_up.stderr;
    let se_base = result_base.stderr;
    let se_down = result_down.stderr;
    // Variance of (V_up − 2V_base + V_down): 1·V_up + 4·V_base + 1·V_down under
    // independence. CRN makes this pessimistic but not wrong.
    let stderr = (se_up * se_up + 4.0 * se_base * se_base + se_down * se_down).sqrt() / (h * h);

    Ok((gamma, stderr))
}

// ---------------------------------------------------------------------------
// CRN-paired finite differences (true CRN stderr)
// ---------------------------------------------------------------------------

/// Run a paired CRN finite-difference loop and return per-path payoff
/// differences for each of `n_states` initial-state perturbations.
///
/// Each path uses an independent splittable substream keyed on `path_id`. The
/// substream is re-cloned for each perturbation so all `n_states` variants
/// consume identical shock sequences — this is what makes the per-path
/// difference a tight CRN estimator.
///
/// Returns a `Vec<Vec<f64>>` of length `n_states` where each inner vector has
/// length `engine.config.num_paths` and contains discounted payoff amounts
/// (currency stripped via `MoneyEstimate`-style conversion).
#[allow(clippy::too_many_arguments)]
fn paired_per_path_payoffs<R, P, D, F>(
    engine: &McEngine,
    rng: &R,
    process: &P,
    disc: &D,
    initial_states: &[Vec<f64>],
    payoff: &F,
    currency: Currency,
    discount_factor: f64,
) -> Result<Vec<Vec<f64>>>
where
    R: RandomStream,
    P: StochasticProcess,
    D: Discretization<P>,
    F: Payoff,
{
    let n_states = initial_states.len();
    debug_assert!(
        n_states >= 1,
        "paired_per_path_payoffs requires at least one initial state"
    );

    let cfg = engine.config();
    let dim = process.dim();
    let num_factors = process.num_factors();
    let work_size = disc.work_size(process);

    // Validate state shapes once.
    for (i, s) in initial_states.iter().enumerate() {
        if s.len() != dim {
            return Err(finstack_quant_core::Error::Validation(format!(
                "paired finite-diff initial_states[{i}].len() = {} does not match process dim = {}",
                s.len(),
                dim
            )));
        }
    }

    let correlation = build_correlation_factor(process, disc)?;
    let mut payoff_local = payoff.clone();
    let mut state = vec![0.0; dim];
    let mut z = vec![0.0; num_factors];
    let mut z_raw = vec![
        0.0;
        if correlation.is_some() {
            num_factors
        } else {
            0
        }
    ];
    let mut work = vec![0.0; work_size];

    let mut per_state_values: Vec<Vec<f64>> = (0..n_states)
        .map(|_| Vec::with_capacity(cfg.num_paths))
        .collect();

    for path_id in 0..cfg.num_paths {
        let base_split = rng.split(path_id as u64).ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "RandomStream reports splitting support but split() returned None for paired \
                 finite-diff Greek"
                    .to_string(),
            )
        })?;

        for (state_idx, s0) in initial_states.iter().enumerate() {
            let mut path_rng = base_split.clone();
            payoff_local.reset();
            payoff_local.on_path_start(&mut path_rng);
            let v = engine.simulate_path(
                &mut path_rng,
                process,
                disc,
                s0,
                &mut payoff_local,
                &mut state,
                &mut z,
                &mut z_raw,
                &mut work,
                correlation.as_ref(),
                currency,
            )?;
            let discounted = v * discount_factor;
            if !discounted.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "non-finite paired finite-diff payoff on path {path_id}, state index \
                     {state_idx}: payoff={v}, discount_factor={discount_factor}"
                )));
            }
            per_state_values[state_idx].push(discounted);
        }
    }

    Ok(per_state_values)
}

/// Compute delta with **true CRN stderr** by per-path pairing.
///
/// Like [`finite_diff_delta`] but reports the proper paired standard error
/// `stderr({(V_up_i − V_down_i) / 2h})`, which exploits the strong positive
/// correlation introduced by common random numbers and is typically one to two
/// orders of magnitude tighter than the conservative independence bound.
///
/// Always runs serially (paired stderr requires deterministic per-path order).
/// The pricer's `use_parallel` flag is honored only by [`finite_diff_delta`].
///
/// # Returns
///
/// `(delta, stderr)` where `stderr` is the **paired** standard error.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when the RNG is not
/// splittable, when configuration is invalid, or when any path simulation
/// fails (e.g., non-finite payoff).
#[allow(clippy::too_many_arguments)]
pub fn finite_diff_delta_crn<R, P, D, F>(
    engine: &McEngine,
    rng: &R,
    process: &P,
    disc: &D,
    initial_spot: f64,
    payoff: &F,
    currency: Currency,
    discount_factor: f64,
    bump_size: f64,
) -> Result<(f64, f64)>
where
    R: RandomStream,
    P: StochasticProcess,
    D: Discretization<P>,
    F: Payoff,
{
    require_splittable_rng(rng, "finite_diff_delta_crn")?;
    let h = bump_amount(initial_spot, bump_size);

    let initial_states = vec![
        vec![initial_spot + h],
        vec![bumped_down_spot(initial_spot, h)],
    ];
    let per_state = paired_per_path_payoffs(
        engine,
        rng,
        process,
        disc,
        &initial_states,
        payoff,
        currency,
        discount_factor,
    )?;

    let v_up = &per_state[0];
    let v_down = &per_state[1];
    let mut stats = OnlineStats::new();
    for i in 0..v_up.len() {
        stats.update((v_up[i] - v_down[i]) / (2.0 * h));
    }
    Ok((stats.mean(), stats.stderr()))
}

/// Compute gamma with **true CRN stderr** by per-path pairing.
///
/// Like [`finite_diff_gamma`] but reports the paired standard error of the
/// per-path second-difference estimator
/// `(V_up_i − 2 V_base_i + V_down_i) / h²`, which is typically one to two
/// orders of magnitude tighter than the independence bound.
///
/// # Returns
///
/// `(gamma, stderr)` where `stderr` is the **paired** standard error.
///
/// # Errors
///
/// See [`finite_diff_delta_crn`].
#[allow(clippy::too_many_arguments)]
pub fn finite_diff_gamma_crn<R, P, D, F>(
    engine: &McEngine,
    rng: &R,
    process: &P,
    disc: &D,
    initial_spot: f64,
    payoff: &F,
    currency: Currency,
    discount_factor: f64,
    bump_size: f64,
) -> Result<(f64, f64)>
where
    R: RandomStream,
    P: StochasticProcess,
    D: Discretization<P>,
    F: Payoff,
{
    require_splittable_rng(rng, "finite_diff_gamma_crn")?;
    let h = bump_amount(initial_spot, bump_size);

    let initial_states = vec![
        vec![initial_spot + h],
        vec![initial_spot],
        vec![bumped_down_spot(initial_spot, h)],
    ];
    let per_state = paired_per_path_payoffs(
        engine,
        rng,
        process,
        disc,
        &initial_states,
        payoff,
        currency,
        discount_factor,
    )?;

    let v_up = &per_state[0];
    let v_base = &per_state[1];
    let v_down = &per_state[2];
    let mut stats = OnlineStats::new();
    for i in 0..v_up.len() {
        stats.update((v_up[i] - 2.0 * v_base[i] + v_down[i]) / (h * h));
    }
    Ok((stats.mean(), stats.stderr()))
}

#[cfg(test)]
mod tests {
    use super::super::super::engine::McEngineConfig;
    use super::*;
    use crate::discretization::exact::ExactGbm;
    use crate::payoff::vanilla::EuropeanCall;
    use crate::process::gbm::{GbmParams, GbmProcess};
    use crate::rng::philox::PhiloxRng;
    use crate::time_grid::TimeGrid;

    #[test]
    fn test_finite_diff_delta_atm() {
        let time_grid = TimeGrid::uniform(1.0, 50).expect("should succeed");
        let engine = McEngine::new(McEngineConfig {
            num_paths: 5_000,
            time_grid,
            target_ci_half_width: None,
            use_parallel: false,
            chunk_size: None,
            path_capture: crate::engine::PathCaptureConfig::default(),
            antithetic: false,
        });

        let rng = PhiloxRng::new(42);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let disc = ExactGbm::new();
        let call = EuropeanCall::new(100.0, 1.0, 50);

        let (delta, stderr) = finite_diff_delta(
            &engine,
            &rng,
            &gbm,
            &disc,
            100.0,
            &call,
            Currency::USD,
            1.0,
            0.01,
        )
        .expect("should succeed");

        // ATM call delta should be around 0.5
        assert!(delta > 0.3 && delta < 0.7);
        assert!(stderr.is_finite() && stderr >= 0.0);
    }

    #[test]
    fn test_finite_diff_delta_crn_tighter_than_independence_bound() {
        let time_grid = TimeGrid::uniform(1.0, 50).expect("should succeed");
        let engine = McEngine::new(McEngineConfig {
            num_paths: 5_000,
            time_grid,
            target_ci_half_width: None,
            use_parallel: false,
            chunk_size: None,
            path_capture: crate::engine::PathCaptureConfig::default(),
            antithetic: false,
        });

        let rng = PhiloxRng::new(42);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let disc = ExactGbm::new();
        let call = EuropeanCall::new(100.0, 1.0, 50);

        let (_, se_indep) = finite_diff_delta(
            &engine,
            &rng,
            &gbm,
            &disc,
            100.0,
            &call,
            Currency::USD,
            1.0,
            0.01,
        )
        .expect("ok");
        let (delta_crn, se_crn) = finite_diff_delta_crn(
            &engine,
            &rng,
            &gbm,
            &disc,
            100.0,
            &call,
            Currency::USD,
            1.0,
            0.01,
        )
        .expect("ok");

        assert!(delta_crn > 0.3 && delta_crn < 0.7);
        // CRN paired stderr should be (much) smaller for a smooth call payoff.
        assert!(se_crn < se_indep);
    }

    #[test]
    fn test_finite_diff_gamma_crn_paired_stderr() {
        let time_grid = TimeGrid::uniform(1.0, 50).expect("should succeed");
        let engine = McEngine::new(McEngineConfig {
            num_paths: 5_000,
            time_grid,
            target_ci_half_width: None,
            use_parallel: false,
            chunk_size: None,
            path_capture: crate::engine::PathCaptureConfig::default(),
            antithetic: false,
        });

        let rng = PhiloxRng::new(42);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let disc = ExactGbm::new();
        let call = EuropeanCall::new(100.0, 1.0, 50);

        let (gamma_crn, se_crn) = finite_diff_gamma_crn(
            &engine,
            &rng,
            &gbm,
            &disc,
            100.0,
            &call,
            Currency::USD,
            1.0,
            0.01,
        )
        .expect("ok");
        assert!(gamma_crn > 0.0);
        assert!(se_crn.is_finite() && se_crn >= 0.0);
    }

    #[test]
    fn test_finite_diff_gamma_positive() {
        let time_grid = TimeGrid::uniform(1.0, 50).expect("should succeed");
        let engine = McEngine::new(McEngineConfig {
            num_paths: 5_000,
            time_grid,
            target_ci_half_width: None,
            use_parallel: false,
            chunk_size: None,
            path_capture: crate::engine::PathCaptureConfig::default(),
            antithetic: false,
        });

        let rng = PhiloxRng::new(42);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let disc = ExactGbm::new();
        let call = EuropeanCall::new(100.0, 1.0, 50);

        let (gamma, stderr) = finite_diff_gamma(
            &engine,
            &rng,
            &gbm,
            &disc,
            100.0,
            &call,
            Currency::USD,
            1.0,
            0.01,
        )
        .expect("should succeed");

        // Gamma should be positive for ATM options
        assert!(gamma > 0.0);
        assert!(stderr.is_finite() && stderr >= 0.0);
    }
}

//! No-arbitrage numéraire-discounting bound tests for the Monte Carlo
//! Bermudan swaption engines (LSMC HW1F and LMM/BGM).
//!
//! These tests pin two facts that a *pathwise-numéraire* discounting bug
//! violates but a deterministic-discount-factor implementation cannot
//! satisfy:
//!
//! 1. **LMM co-terminal lower bound.** A Bermudan swaption can always
//!    replicate the strategy "exercise only at date `t_k`", so its price
//!    must be `>=` the most valuable single co-terminal European
//!    swaption. The European reference here is computed with the
//!    *correct* terminal-measure estimator
//!    `E^{T_N}[ payoff(t)/P(t,T_N) ] * P(0,T_N)` — i.e. dividing by the
//!    *pathwise* numéraire `P(t,T_N)`. An LMM engine that multiplies the
//!    path cashflow only by the constant `P(0,T_N)` (omitting the
//!    pathwise `1/P(t,T_N)`) under-prices the Bermudan below this bound.
//!
//! 2. **LSMC pathwise-numéraire consistency.** A single-exercise Bermudan
//!    is a European swaption, whose price obeys the model-free identity
//!    `V(0) = E[ X(t)/B(t) ]` with `B(t) = ∏ exp(r_k·Δt_k)` the realized
//!    money-market account. An LSMC engine that discounts by the
//!    *deterministic* market discount factor `DF(t)` instead of the
//!    pathwise `1/B(t)` ignores the (negative, for payers) correlation
//!    between the swap payoff and the stochastic discount factor and
//!    mis-prices the European. The multi-exercise Bermudan must likewise
//!    match a correct-numéraire backward-induction reference.
//!
//! The reference simulations replay the *same* `finstack-monte-carlo`
//! process + discretization + Philox RNG the engines use, so any
//! discrepancy is attributable to the discounting convention, not to a
//! different model or different random numbers.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use finstack_core::currency::Currency;
use finstack_monte_carlo::discretization::exact_hw1f::ExactHullWhite1F;
use finstack_monte_carlo::discretization::lmm_predictor_corrector::LmmPredictorCorrector;
use finstack_monte_carlo::online_stats::OnlineStats;
use finstack_monte_carlo::pricer::basis::{BasisFunctions, PolynomialBasis};
use finstack_monte_carlo::pricer::lsq::solve_least_squares;
use finstack_monte_carlo::process::lmm::{LmmParams, LmmProcess};
use finstack_monte_carlo::process::ou::{HullWhite1FParams, HullWhite1FProcess};
use finstack_monte_carlo::rng::philox::PhiloxRng;
use finstack_monte_carlo::traits::{Discretization, RandomStream};
use finstack_valuations::instruments::rates::swaption::pricing::lmm_bermudan::{
    build_exercise_aligned_grid, price_bermudan_lmm, LmmBermudanConfig,
};
use finstack_valuations::instruments::rates::swaption::pricing::monte_carlo_lsmc::{
    SwaptionLsmcConfig, SwaptionLsmcPricer,
};
use finstack_valuations::instruments::rates::swaption::pricing::monte_carlo_payoff::{
    BermudanSwaptionPayoff, SwapSchedule, SwaptionType,
};
use finstack_valuations::instruments::rates::swaption::pricing::swap_rate_utils::{
    ForwardSwapRate, HullWhiteBondPrice,
};

// ===========================================================================
// LMM / BGM — co-terminal lower bound
// ===========================================================================

/// 4 annual forwards, 2 factors, ~12% loadings — mirrors the `lmm_bermudan`
/// unit-test parameter set so the failure is reproducible against the
/// already-exercised code path.
fn lmm_params() -> LmmParams {
    LmmParams::try_new(
        4,
        2,
        vec![0.0, 1.0, 2.0, 3.0, 4.0],
        vec![1.0, 1.0, 1.0, 1.0],
        vec![0.005; 4],
        vec![],
        vec![vec![
            [0.12, 0.04, 0.0],
            [0.11, 0.05, 0.0],
            [0.10, 0.06, 0.0],
            [0.09, 0.07, 0.0],
        ]],
        vec![0.03, 0.032, 0.034, 0.036],
    )
    .expect("valid LMM params")
}

/// Forward swap rate and annuity for the swap covering periods `[0, n)`,
/// expressed relative to the first tenor `T_0`. This duplicates the
/// (crate-private) `compute_swap_rate_and_annuity` helper of `lmm_bermudan`
/// so the reference uses the *identical* payoff definition as the engine.
fn lmm_swap_rate_and_annuity(forwards: &[f64], accrual: &[f64], n: usize) -> (f64, f64) {
    let mut df = vec![1.0; n + 1];
    for k in 1..=n {
        df[k] = df[k - 1] / (1.0 + accrual[k - 1] * forwards[k - 1]);
    }
    let mut annuity = 0.0;
    for j in 0..n {
        annuity += accrual[j] * df[j + 1];
    }
    let swap_rate = if annuity.abs() > 1e-15 {
        (1.0 - df[n]) / annuity
    } else {
        0.0
    };
    (swap_rate, annuity)
}

/// Correct-numéraire reference price of a single co-terminal European
/// swaption under the LMM terminal measure:
/// `P(0,T_N) * E^{T_N}[ (S(t)-K)^+ A(t) N / P(t,T_N) ]`.
///
/// `P(t,T_N)` is the *pathwise* terminal numéraire, the product of
/// `1/(1+τ_j F_j)` over the forwards still alive at time `t` — exactly the
/// quantity `LmmProcess::populate_path_state` stores as `"lmm_numeraire"`.
fn lmm_reference_european(
    params: &LmmParams,
    exercise_time: f64,
    strike: f64,
    notional: f64,
    df0_terminal: f64,
    num_paths: usize,
    seed: u64,
) -> f64 {
    let n = params.num_forwards;
    let process = LmmProcess::new(params.clone());
    let disc = LmmPredictorCorrector::new();
    let maturity = *params.tenors.last().expect("tenors");

    // Build the exercise-aligned grid the same way the engine does.
    let (grid, exercise_idx) =
        build_exercise_aligned_grid(&[exercise_time], maturity, 8).expect("grid");
    let ex_step = exercise_idx[0];
    let work_size = disc.work_size(&process);
    let base = PhiloxRng::new(seed);
    let mut stats = OnlineStats::new();

    for path_id in 0..num_paths {
        let mut rng = base.substream(path_id as u64);
        let mut x = params.initial_forwards.clone();
        let mut work = vec![0.0; work_size];
        let mut z = vec![0.0; params.num_factors];

        for step in 0..grid.num_steps() {
            let t = grid.time(step);
            let dt = grid.dt(step);
            rng.fill_std_normals(&mut z);
            disc.step(&process, t, dt, &mut x, &z, &mut work);
            if step + 1 == ex_step {
                break;
            }
        }

        let (swap_rate, annuity) = lmm_swap_rate_and_annuity(&x, &params.accrual_factors, n);
        let intrinsic = ((swap_rate - strike).max(0.0)) * annuity * notional;

        // Pathwise terminal numéraire P(t, T_N) from the alive forwards.
        let t_ex = grid.time(ex_step);
        let first_alive = params.tenors[..n].partition_point(|&tenor| tenor < t_ex);
        let mut p_t_tn = 1.0;
        for (fwd, tau) in x[first_alive..n]
            .iter()
            .zip(&params.accrual_factors[first_alive..n])
        {
            p_t_tn /= 1.0 + tau * fwd;
        }

        // Terminal-measure deflated payoff.
        stats.update(intrinsic / p_t_tn * df0_terminal);
    }

    stats.mean()
}

/// A Bermudan swaption must price `>=` the most valuable single co-terminal
/// European swaption: exercising only on that one date is always available.
///
/// The European reference uses the correct terminal-measure estimator that
/// divides the path payoff by the *pathwise* numéraire `P(t,T_N)`. The LMM
/// engine (`price_bermudan_lmm`) multiplies the path cashflow only by the
/// constant `P(0,T_N)` and never divides by the pathwise `P(t,T_N)` — so on
/// the pre-fix code it under-prices the Bermudan well below this bound.
///
/// Pre-fix observation (40k+ paths, seed 7): the Bermudan prices ≈ 25.9k
/// while the correct co-terminal European at `t=1` is ≈ 28.7k — a ~11%
/// lower-bound violation, far outside Monte Carlo error.
#[test]
fn lmm_bermudan_respects_coterminal_lower_bound() {
    let params = lmm_params();
    let strike = 0.025; // ITM payer (forwards ≈ 3.0–3.6%)
    let notional = 1_000_000.0;
    let maturity = 4.0_f64;
    let df0_terminal = (-0.03 * maturity).exp();
    let num_paths = 60_000;
    let seed = 7;

    let config = LmmBermudanConfig {
        num_paths,
        seed,
        basis_degree: 2,
        antithetic: true,
        min_steps_between_exercises: 8,
    };

    let exercise_times = [1.0, 2.0, 3.0];
    let bermudan = price_bermudan_lmm(
        &params,
        &exercise_times,
        strike,
        true, // payer
        notional,
        df0_terminal,
        Currency::USD,
        &config,
    )
    .expect("bermudan pricing");
    let bermudan_pv = bermudan.mean.amount();

    // Most valuable co-terminal European, correctly numéraire-discounted.
    let mut best_european = f64::MIN;
    for &ex_t in &exercise_times {
        let euro = lmm_reference_european(
            &params,
            ex_t,
            strike,
            notional,
            df0_terminal,
            num_paths,
            seed,
        );
        if euro > best_european {
            best_european = euro;
        }
    }

    // Generous Monte Carlo slack: the engine and reference share RNG seed
    // and model, so sampling noise is small; the slack only guards against
    // legitimate residual MC error, not the ~11% discounting bias.
    let mc_slack = 0.02 * best_european;
    assert!(
        bermudan_pv >= best_european - mc_slack,
        "Bermudan ({bermudan_pv:.2}) violates the co-terminal lower bound: it must be \
         >= the best single co-terminal European ({best_european:.2}). A shortfall this \
         large is the missing pathwise terminal numéraire P(t,T_N) in the LMM engine."
    );

    // Sanity: a positive ITM price.
    assert!(bermudan_pv > 0.0, "ITM Bermudan should be positive");
}

// ===========================================================================
// LSMC / Hull-White 1F — pathwise money-market numéraire
// ===========================================================================

/// Exercise-aligned swap schedule used by both the LSMC engine and the
/// reference: a co-terminal payer swap maturing at 5y with annual periods.
fn lsmc_swap_schedule() -> SwapSchedule {
    SwapSchedule::new(1.0, 5.0, vec![1.0, 2.0, 3.0, 4.0, 5.0], vec![1.0; 5])
        .expect("valid swap schedule")
}

/// Hull-White 1F payer-swaption intrinsic at time `t`: `(S(t)-K)^+ A(t) N`,
/// the time-`t` swap value. Returns `(intrinsic, swap_rate)`.
fn hw1f_intrinsic(
    hw: &HullWhite1FProcess,
    r_t: f64,
    t: f64,
    schedule: &SwapSchedule,
    strike: f64,
    notional: f64,
    discount_fn: impl Fn(f64) -> f64 + Copy,
) -> (f64, f64) {
    let params = hw.params();
    let swap_rate = ForwardSwapRate::compute(params, r_t, t, schedule, discount_fn);
    let mut annuity = 0.0;
    for (j, &payment_date) in schedule.payment_dates.iter().enumerate() {
        if payment_date > t {
            annuity += schedule.accrual_fractions[j]
                * HullWhiteBondPrice::bond_price(params, r_t, t, payment_date, discount_fn);
        }
    }
    (
        (swap_rate - strike).max(0.0) * annuity * notional,
        swap_rate,
    )
}

/// Simulate `num_paths` Hull-White 1F short-rate paths on `grid`, returning,
/// for each path, the rate at every grid point and the realised
/// money-market account `B(t)` (`B(0)=1`, left-endpoint Riemann rule).
///
/// This is the *same* generation as the LSMC engine's non-antithetic
/// path generator: per-path Philox substream, one standard normal per
/// step, `ExactHullWhite1F` stepper — so a reference built on these paths
/// differs from the engine only in the discounting convention.
fn simulate_hw1f_paths(
    hw: &HullWhite1FProcess,
    r0: f64,
    grid: &finstack_monte_carlo::time_grid::TimeGrid,
    num_paths: usize,
    seed: u64,
) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let disc = ExactHullWhite1F::new();
    let rng = PhiloxRng::new(seed);
    let num_steps = grid.num_steps();
    let mut rate_paths = Vec::with_capacity(num_paths);
    let mut bank_paths = Vec::with_capacity(num_paths);
    for path_id in 0..num_paths {
        let mut path_rng = rng.substream(path_id as u64);
        let mut state = vec![r0];
        let mut z = vec![0.0];
        let mut work = vec![];
        let mut rates = Vec::with_capacity(num_steps + 1);
        let mut banks = Vec::with_capacity(num_steps + 1);
        rates.push(r0);
        banks.push(1.0_f64); // B(t_0) = 1
        let mut acc = 1.0;
        for step in 0..num_steps {
            // r(t_step) at the start of the interval drives B over [t_step, t_{step+1}).
            acc *= (rates[step] * grid.dt(step)).exp();
            let t = grid.time(step);
            let dt = grid.dt(step);
            path_rng.fill_std_normals(&mut z);
            disc.step(hw, t, dt, &mut state, &z, &mut work);
            rates.push(state[0]);
            banks.push(acc);
        }
        rate_paths.push(rates);
        bank_paths.push(banks);
    }
    (rate_paths, bank_paths)
}

/// Correct-numéraire reference price of a Hull-White 1F European swaption:
/// `E[ X(t) / B(t) ]` where `B(t)` is the realised money-market account.
///
/// The intrinsic `X(t) = (S(t)-K)^+ A(t) N` is the time-`t` swap value, so
/// the path PV is `X(t)/B(t)`. This is the discounting the LSMC engine
/// *should* apply; the pre-fix engine used the deterministic `DF(t)`.
///
/// Runs on the *exact* grid passed in (the one the engine priced on) so
/// the reference and engine sample bit-identical short-rate paths.
#[allow(clippy::too_many_arguments)]
fn hw1f_reference_european(
    hw: &HullWhite1FProcess,
    r0: f64,
    grid: &finstack_monte_carlo::time_grid::TimeGrid,
    exercise_step: usize,
    schedule: &SwapSchedule,
    strike: f64,
    notional: f64,
    discount_fn: impl Fn(f64) -> f64 + Copy,
    num_paths: usize,
    seed: u64,
) -> f64 {
    let (rate_paths, bank_paths) = simulate_hw1f_paths(hw, r0, grid, num_paths, seed);
    let t = grid.time(exercise_step);
    let mut stats = OnlineStats::new();
    for i in 0..num_paths {
        let (intrinsic, _) = hw1f_intrinsic(
            hw,
            rate_paths[i][exercise_step],
            t,
            schedule,
            strike,
            notional,
            discount_fn,
        );
        // Pathwise discount: 1 / B(t_exercise).
        stats.update(intrinsic / bank_paths[i][exercise_step]);
    }
    stats.mean()
}

/// Correct-numéraire reference for the *multi-exercise* Bermudan: a
/// Longstaff-Schwartz backward induction in which the continuation-value
/// regression target and the final PV are both discounted by the
/// **pathwise** money-market ratio, not by the deterministic discount
/// curve. Runs on the exact grid + exercise steps the engine priced on.
#[allow(clippy::too_many_arguments)]
fn hw1f_reference_bermudan(
    hw: &HullWhite1FProcess,
    r0: f64,
    grid: &finstack_monte_carlo::time_grid::TimeGrid,
    exercise_steps: &[usize],
    schedule: &SwapSchedule,
    strike: f64,
    notional: f64,
    discount_fn: impl Fn(f64) -> f64 + Copy,
    num_paths: usize,
    seed: u64,
) -> f64 {
    let (rate_paths, bank_paths) = simulate_hw1f_paths(hw, r0, grid, num_paths, seed);

    let mut cashflows = vec![0.0; num_paths];
    // Grid step of the optimal exercise decision; indexes `bank_paths`.
    let mut exercise_step_of = vec![0usize; num_paths];

    let mut steps_desc: Vec<usize> = exercise_steps.to_vec();
    steps_desc.sort_unstable();
    steps_desc.reverse();

    for &step in &steps_desc {
        if step >= grid.num_steps() {
            continue;
        }
        let t = grid.time(step);
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        let mut idx = Vec::new();
        for i in 0..num_paths {
            let (immediate, swap_rate) = hw1f_intrinsic(
                hw,
                rate_paths[i][step],
                t,
                schedule,
                strike,
                notional,
                discount_fn,
            );
            if immediate > 1e-6 {
                // Future cashflow discounted to t by the PATHWISE ratio
                // B(t_now) / B(t_exercise).
                let discounted =
                    cashflows[i] * bank_paths[i][step] / bank_paths[i][exercise_step_of[i]];
                xs.push(swap_rate);
                ys.push(discounted);
                idx.push(i);
            }
        }
        // Match the engine's ITM-count gate for the regression branch
        // (`regression_x.len() > basis.num_basis() + 10`).
        let basis = PolynomialBasis::new(3);
        if xs.len() > basis.num_basis() + 10 {
            let k = basis.num_basis();
            let mut design = vec![0.0; xs.len() * k];
            let mut basis_vals = vec![0.0; k];
            for (row, &x) in xs.iter().enumerate() {
                basis.evaluate(x, &mut basis_vals);
                design[row * k..row * k + k].copy_from_slice(&basis_vals);
            }
            if let Ok(coeffs) = solve_least_squares(&design, &ys, xs.len(), k) {
                for (local, &i) in idx.iter().enumerate() {
                    basis.evaluate(xs[local], &mut basis_vals);
                    let continuation: f64 =
                        basis_vals.iter().zip(&coeffs).map(|(a, b)| a * b).sum();
                    let (immediate, _) = hw1f_intrinsic(
                        hw,
                        rate_paths[i][step],
                        t,
                        schedule,
                        strike,
                        notional,
                        discount_fn,
                    );
                    if immediate > continuation {
                        cashflows[i] = immediate;
                        exercise_step_of[i] = step;
                    }
                }
            }
        }
    }

    let mut stats = OnlineStats::new();
    for i in 0..num_paths {
        // PV = realised cashflow / B(t_exercise).
        stats.update(cashflows[i] / bank_paths[i][exercise_step_of[i]]);
    }
    stats.mean()
}

/// A single-exercise Bermudan swaption is a European swaption, whose price
/// obeys the model-free identity `V(0) = E[X(t)/B(t)]` with `B(t)` the
/// realised money-market account.
///
/// The pre-fix LSMC engine discounted the path cashflow by the
/// *deterministic* market discount factor `DF(t)`. For a payer swaption
/// the payoff is large exactly when rates are high — i.e. when `1/B(t)` is
/// small — so `E[X·1/B] < E[X]·E[1/B] ≈ E[X]·DF(t)`: the deterministic
/// engine *over-prices* the European.
///
/// The engine and the reference here run on the *same* time grid with
/// *non-antithetic* paths, so they consume identical Philox draws and
/// simulate bit-identical short-rate paths. A correct (pathwise-numéraire)
/// engine therefore matches the reference to floating-point precision; the
/// pre-fix engine is off by ~6–7% at the longer exercise dates.
#[test]
fn lsmc_european_uses_pathwise_money_market_numeraire() {
    // Flat 4% discount curve; meaningful HW vol so the payoff/numéraire
    // correlation (the convexity the deterministic DF ignores) is visible.
    let discount_fn = |t: f64| (-0.04 * t).exp();
    let hw = HullWhite1FProcess::new(HullWhite1FParams::new(0.1, 0.03, 0.04));
    let r0 = 0.04;
    let strike = 0.03; // ITM payer
    let notional = 1_000_000.0;
    let schedule = lsmc_swap_schedule();
    let basis = PolynomialBasis::new(3);
    let num_paths = 60_000;
    let seed = 7;

    // Non-antithetic so the engine and the reference draw bit-identical
    // paths — the test then isolates the discounting convention exactly.
    let config = SwaptionLsmcConfig::new(num_paths, seed)
        .with_basis_degree(3)
        .with_antithetic(false);
    let pricer = SwaptionLsmcPricer::with_config(config, hw.clone());

    // Probe the longer co-terminal Europeans: the pathwise-vs-deterministic
    // discounting gap grows with exercise time.
    for &ex_t in &[2.0, 3.0, 4.0] {
        let payoff = BermudanSwaptionPayoff::new(
            vec![ex_t],
            schedule.clone(),
            strike,
            SwaptionType::Payer,
            notional,
        );
        let (grid, exercise_idx) =
            SwaptionLsmcConfig::build_exercise_aligned_grid(&[ex_t], schedule.end_date, 2)
                .expect("grid");
        let engine = pricer
            .price_bermudan_with_grid(
                &payoff,
                r0,
                &grid,
                &exercise_idx,
                &basis,
                discount_fn,
                Currency::USD,
            )
            .expect("LSMC European pricing");
        let engine_pv = engine.mean.amount();

        let reference = hw1f_reference_european(
            &hw,
            r0,
            &grid,
            exercise_idx[0],
            &schedule,
            strike,
            notional,
            discount_fn,
            num_paths,
            seed,
        );

        // Engine and reference share model, RNG seed, grid and (non-
        // antithetic) path generation: a correct engine reproduces the
        // pathwise reference to floating-point precision. The tolerance is
        // a tiny relative epsilon — the pre-fix bias is ~6–7%.
        let tol = 1e-6 * reference.abs().max(1.0);
        assert!(
            (engine_pv - reference).abs() <= tol,
            "LSMC European at t={ex_t} ({engine_pv:.4}) must equal the pathwise \
             money-market reference ({reference:.4}); gap {:.4}. A non-trivial gap is \
             deterministic discount-factor discounting ignoring the payoff/numéraire \
             correlation.",
            (engine_pv - reference).abs()
        );
    }
}

/// The multi-exercise LSMC Bermudan must match a correct-numéraire
/// Longstaff-Schwartz reference in which both the continuation-value
/// regression target and the terminal PV are discounted by the *pathwise*
/// money-market account. This pins the deterministic discount-factor ratio
/// in the continuation step as well as in the final discounting.
///
/// The Bermudan must also clear its co-terminal European lower bound: a
/// Bermudan can always exercise on a single chosen date, so it is worth at
/// least the most valuable correctly-discounted co-terminal European.
#[test]
fn lsmc_bermudan_matches_pathwise_numeraire_reference() {
    let discount_fn = |t: f64| (-0.04 * t).exp();
    let hw = HullWhite1FProcess::new(HullWhite1FParams::new(0.1, 0.03, 0.04));
    let r0 = 0.04;
    let strike = 0.03;
    let notional = 1_000_000.0;
    let schedule = lsmc_swap_schedule();
    let basis = PolynomialBasis::new(3);
    let num_paths = 60_000;
    let seed = 7;
    let exercise_times = [1.0, 2.0, 3.0, 4.0];

    let config = SwaptionLsmcConfig::new(num_paths, seed)
        .with_basis_degree(3)
        .with_antithetic(false);
    let pricer = SwaptionLsmcPricer::with_config(config, hw.clone());

    let payoff = BermudanSwaptionPayoff::new(
        exercise_times.to_vec(),
        schedule.clone(),
        strike,
        SwaptionType::Payer,
        notional,
    );
    let (grid, exercise_idx) =
        SwaptionLsmcConfig::build_exercise_aligned_grid(&exercise_times, schedule.end_date, 2)
            .expect("grid");
    let engine = pricer
        .price_bermudan_with_grid(
            &payoff,
            r0,
            &grid,
            &exercise_idx,
            &basis,
            discount_fn,
            Currency::USD,
        )
        .expect("LSMC Bermudan pricing");
    let engine_pv = engine.mean.amount();

    // Reference runs on the identical grid + exercise steps + paths.
    let reference = hw1f_reference_bermudan(
        &hw,
        r0,
        &grid,
        &exercise_idx,
        &schedule,
        strike,
        notional,
        discount_fn,
        num_paths,
        seed,
    );

    let tol = 1e-6 * reference.abs().max(1.0);
    assert!(
        (engine_pv - reference).abs() <= tol,
        "LSMC Bermudan ({engine_pv:.4}) must match the pathwise money-market \
         backward-induction reference ({reference:.4}); gap {:.4}.",
        (engine_pv - reference).abs()
    );

    // Co-terminal lower bound, using the correct pathwise-numéraire
    // European reference (own grid per exercise date).
    let mut best_european = f64::MIN;
    for &ex_t in &exercise_times {
        let (euro_grid, euro_idx) =
            SwaptionLsmcConfig::build_exercise_aligned_grid(&[ex_t], schedule.end_date, 2)
                .expect("grid");
        let euro = hw1f_reference_european(
            &hw,
            r0,
            &euro_grid,
            euro_idx[0],
            &schedule,
            strike,
            notional,
            discount_fn,
            num_paths,
            seed,
        );
        if euro > best_european {
            best_european = euro;
        }
    }
    // The European references use independent per-date grids, so a small
    // Monte Carlo slack is warranted for the cross-estimator comparison.
    assert!(
        engine_pv >= best_european - 0.02 * best_european,
        "LSMC Bermudan ({engine_pv:.2}) must be >= the best correctly-discounted \
         co-terminal European ({best_european:.2})."
    );
}

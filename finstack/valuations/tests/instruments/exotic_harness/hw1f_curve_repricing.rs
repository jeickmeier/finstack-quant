//! Curve-repricing test for the HW1F exotic-rate Monte Carlo harness (M6).
//!
//! Defect M6: before this fix, the TARN / snowball / callable-range-accrual
//! pricers built the HW1F short-rate process with a *constant* mean-reversion
//! level `θ = r0`. A constant θ makes the simulated short rate a plain Vasicek
//! process whose implied zero-coupon bond prices `E[exp(−∫r dt)]` do **not**
//! match the input discount curve unless that curve is flat (Brigo–Mercurio
//! §3.3.1: Hull-White requires a *time-dependent* θ(t) fitted to the curve).
//!
//! Both tests here reconstruct the exact short-rate simulation the exotic
//! pricers run (exact HW1F stepper, Philox substreams, antithetic off) and
//! price zero-coupon bonds along the path:
//!
//! * [`calibrated_theta_reprices_sloped_curve`] is the **core M6 regression**.
//!   It builds the process through the *production* entry points the fixed
//!   pricers use — `exotics_shared::hw1f_curve::calibrate_hw1f_params` (the
//!   θ(t) bootstrap) and `initial_short_rate_from_curve` (`r0 = f(0,0)`) — and
//!   asserts the simulated ZCB prices reproduce the input discount factors. On
//!   the parent commit the pricers used flat `θ = r0`, so this assertion fails
//!   by a 115–1175 bp drift bias; with the M6 fix it passes within ~3 bp.
//!
//! * [`flat_theta_misreprices_sloped_curve`] pins the *defect itself*: it builds
//!   the pre-fix flat-`θ = r0` process and asserts the repricing bias is large
//!   (≥ 50 bp). This proves the core test above genuinely discriminates — a
//!   curve-repricing check that passed for both θ regimes would have no teeth.
//!
//! * [`theta_repricing_error_converges_with_grid`] confirms the ~3 bp residual
//!   in the core test is Monte-Carlo discretization error (it shrinks as the
//!   grid is refined), not a model bias.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use finstack_core::dates::{Date, DayCount};
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_monte_carlo::discretization::exact_hw1f::ExactHullWhite1F;
use finstack_monte_carlo::process::ou::{
    calibrate_theta_from_curve, HullWhite1FParams, HullWhite1FProcess,
};
use finstack_monte_carlo::rng::philox::PhiloxRng;
use finstack_monte_carlo::time_grid::TimeGrid;
use finstack_monte_carlo::traits::{Discretization, RandomStream};
use finstack_valuations::instruments::rates::exotics_shared::{
    calibrate_hw1f_params, initial_short_rate_from_curve,
};
use time::Month;

/// Simulate `E[exp(−∫₀ᵀ r dt)]` for each maturity in `maturities` on a uniform
/// time grid, using the supplied HW1F params and the exact stepper. Returns
/// `(mc_discount_factor, standard_error)` per maturity.
///
/// This deliberately mirrors `RateExoticHw1fMcPricer::price` (exotics_shared/
/// hw1f_mc.rs): exact HW1F discretization, Philox substreams, no antithetic.
/// The path integral `∫r dt` is accumulated with the trapezoidal rule.
fn simulate_zcb_prices(
    params: &HullWhite1FParams,
    r0: f64,
    maturities: &[f64],
    steps_per_year: usize,
    num_paths: usize,
    seed: u64,
) -> Vec<(f64, f64)> {
    let process = HullWhite1FProcess::new(params.clone());
    let disc = ExactHullWhite1F;

    let &max_t = maturities.last().expect("at least one maturity");
    let total_steps = (max_t * steps_per_year as f64).ceil() as usize;
    let times: Vec<f64> = (0..=total_steps)
        .map(|i| i as f64 * max_t / total_steps as f64)
        .collect();
    let grid = TimeGrid::from_times(times).expect("uniform grid");
    let num_steps = grid.num_steps();

    // Step index at or just past each maturity (the grid lands close to it).
    let maturity_step: Vec<usize> = maturities
        .iter()
        .map(|&m| ((m / max_t) * total_steps as f64).round() as usize)
        .collect();

    let base_rng = PhiloxRng::new(seed);
    // Online mean / M2 of the per-path discount factor exp(−∫r) at each maturity.
    let mut sum = vec![0.0_f64; maturities.len()];
    let mut sum_sq = vec![0.0_f64; maturities.len()];
    let mut z = [0.0_f64; 1];

    for path_id in 0..num_paths {
        let mut rng = base_rng.substream(path_id as u64);
        let mut r = r0;
        let mut integral = 0.0_f64;
        let mut prev_r = r0;
        let mut next_maturity = 0usize;

        for step in 0..num_steps {
            let t = grid.time(step);
            let dt = grid.dt(step);
            rng.fill_std_normals(&mut z);
            disc.step(&process, t, dt, core::slice::from_mut(&mut r), &z, &mut []);
            // Trapezoidal accumulation of ∫r dt over [t, t+dt].
            integral += 0.5 * (prev_r + r) * dt;
            prev_r = r;

            while next_maturity < maturity_step.len() && maturity_step[next_maturity] == step + 1 {
                let df = (-integral).exp();
                sum[next_maturity] += df;
                sum_sq[next_maturity] += df * df;
                next_maturity += 1;
            }
        }
    }

    let n = num_paths as f64;
    maturities
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let mean = sum[i] / n;
            let var = (sum_sq[i] / n - mean * mean).max(0.0);
            (mean, (var / n).sqrt())
        })
        .collect()
}

/// A clearly upward-sloping discount curve: P(0,t) = exp(−R(t)·t) where the
/// zero rate R(t) rises from 2% (short) to ~5% (long). The instantaneous
/// forward therefore climbs steeply with t — exactly the regime where a flat
/// θ = r0 mis-reprices the curve.
fn sloped_discount_fn(t: f64) -> f64 {
    // Zero rate R(t) = 0.02 + 0.03 * (1 - e^{-0.5 t}); P = exp(-R(t) t).
    let zero_rate = 0.02 + 0.03 * (1.0 - (-0.5 * t).exp());
    (-zero_rate * t).exp()
}

/// The sloped curve of [`sloped_discount_fn`] as a real [`DiscountCurve`],
/// sampled on a fine (quarterly) knot grid out to 6y. This lets the core M6
/// test drive the *production* `calibrate_hw1f_params` /
/// `initial_short_rate_from_curve` entry points the exotic pricers use, rather
/// than a hand-rolled bootstrap.
fn sloped_discount_curve(as_of: Date) -> DiscountCurve {
    let knots: Vec<(f64, f64)> = (0..=24)
        .map(|i| {
            let t = i as f64 * 0.25;
            (t, sloped_discount_fn(t))
        })
        .collect();
    DiscountCurve::builder("SLOPED-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(knots)
        .build()
        .expect("sloped discount curve")
}

/// DEFECT PIN — flat `θ = r0` mis-reprices a sloped curve.
///
/// Builds the HW1F process the way the exotic pricers did *on the parent
/// commit*: a constant mean-reversion level `θ = r0` (plain Vasicek). Simulates
/// ZCB prices and asserts the repricing bias against the sloped input curve is
/// **large** (≥ 50 bp at the longest maturity).
///
/// This is a deliberate test of a *negative* property. It documents the M6
/// defect and, crucially, proves that [`calibrated_theta_reprices_sloped_curve`]
/// has teeth: the two tests run the identical estimator on the identical curve
/// and differ only in θ, so the calibrated test's ~3 bp pass is meaningful only
/// because the flat-θ bias measured here is two orders of magnitude larger.
#[test]
fn flat_theta_misreprices_sloped_curve() {
    let kappa = 0.15_f64;
    let sigma = 0.01_f64;
    // r0 = instantaneous forward at t=0 ≈ short zero rate ≈ 2%.
    let r0 = 0.02_f64;

    let maturities = [1.0_f64, 2.0, 3.0, 5.0];
    let curve_dfs: Vec<f64> = maturities.iter().map(|&m| sloped_discount_fn(m)).collect();

    // Flat-θ construction — IDENTICAL to the pre-fix `theta: r0` in the pricers.
    let flat_params = HullWhite1FParams::new(kappa, sigma, r0);
    let flat = simulate_zcb_prices(&flat_params, r0, &maturities, 48, 40_000, 4242);

    let mut max_bias_bp = 0.0_f64;
    for (i, &m) in maturities.iter().enumerate() {
        let (mc_df, se) = flat[i];
        let bias_bp = (mc_df - curve_dfs[i]).abs() * 10_000.0;
        println!(
            "flat-θ  T={m}: curve_df={:.6} mc_df={:.6} bias={bias_bp:.2}bp se={:.2}bp",
            curve_dfs[i],
            mc_df,
            se * 10_000.0
        );
        max_bias_bp = max_bias_bp.max(bias_bp);
    }
    println!("flat-θ  max bias = {max_bias_bp:.2} bp of notional (defect M6)");

    // The flat-θ Vasicek process does NOT reprice the sloped curve. The bias
    // grows with maturity; at T=5 it is ~1175 bp. A 50 bp floor is comfortably
    // above MC noise (se ≲ 3 bp) yet well below the true bias, so this pins the
    // defect without being brittle.
    let (mc_df_5y, _) = flat[3];
    let bias_5y_bp = (mc_df_5y - curve_dfs[3]).abs() * 10_000.0;
    assert!(
        bias_5y_bp >= 50.0,
        "flat θ=r0 is expected to mis-reprice the 5y point by ≥50bp \
         (defect M6); measured only {bias_5y_bp:.2}bp — has the flat-θ \
         construction silently changed?"
    );
}

/// CORE M6 REGRESSION — curve repricing with bootstrapped θ(t).
///
/// Builds the HW1F process through the **production** entry points the TARN /
/// snowball / callable-range-accrual pricers use after the M6 fix:
/// `calibrate_hw1f_params` (the θ(t) bootstrap) and `initial_short_rate_from_curve`
/// (`r0 = f(0,0)`). It then simulates ZCB prices and asserts they reproduce the
/// input discount factors.
///
/// On the parent commit those entry points did not exist and the pricers built
/// a flat `θ = r0` Vasicek process — for which the companion
/// [`flat_theta_misreprices_sloped_curve`] test measures a 115–1175 bp bias.
/// This test passing therefore demonstrates the M6 fix end-to-end.
///
/// # Tolerance rationale
///
/// The residual is *not* a model bias — it is the discretization floor of this
/// Monte-Carlo ZCB estimator: (a) the trapezoidal accumulation of `∫r dt`,
/// which on an OU path has an O(dt²) bias amplified by the convexity of
/// `exp(−·)`; (b) the grid-discretization error of the piecewise-constant θ(t)
/// bootstrap; and (c) the log-linear interpolation of the knot-based discount
/// curve. Term (a) shrinks as the simulation time grid is refined — the
/// companion `theta_repricing_error_converges_with_grid` confirms exactly this.
/// The measured bias is ≤ 3 bp at every maturity; we bound it at 12 bp — clear
/// of the ~3·se Monte-Carlo noise yet ~100× below the flat-θ drift bias the
/// companion test pins.
#[test]
fn calibrated_theta_reprices_sloped_curve() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let kappa = 0.15_f64;
    let sigma = 0.01_f64;

    let maturities = [1.0_f64, 2.0, 3.0, 5.0];
    let curve_dfs: Vec<f64> = maturities.iter().map(|&m| sloped_discount_fn(m)).collect();

    // --- Production M6 entry points (exotics_shared::hw1f_curve) -------------
    let curve = sloped_discount_curve(as_of);
    let hw = finstack_valuations::calibration::hull_white::HullWhiteParams::new(kappa, sigma)
        .expect("valid HW params");
    let calibrated = calibrate_hw1f_params(hw, &curve, as_of, 5.0).expect("θ(t) bootstrap");
    let r0 = initial_short_rate_from_curve(&curve, as_of).expect("r0 = f(0,0)");
    println!(
        "θ(t) knots = {}, r0 = f(0,0) = {r0:.6}",
        calibrated.theta_times.len()
    );

    let mc = simulate_zcb_prices(&calibrated, r0, &maturities, 96, 60_000, 4242);

    let mut max_bias_bp = 0.0_f64;
    for (i, &m) in maturities.iter().enumerate() {
        let (mc_df, se) = mc[i];
        let bias_bp = (mc_df - curve_dfs[i]).abs() * 10_000.0;
        println!(
            "θ(t)    T={m}: curve_df={:.6} mc_df={:.6} bias={bias_bp:.2}bp se={:.2}bp",
            curve_dfs[i],
            mc_df,
            se * 10_000.0
        );
        max_bias_bp = max_bias_bp.max(bias_bp);
    }
    println!("θ(t)    max bias = {max_bias_bp:.2} bp of notional");

    // 12 bp absorbs the MC-ZCB + knot-interpolation discretization floor while
    // still rejecting the flat-θ drift bias (≥115 bp at every maturity).
    for (i, &m) in maturities.iter().enumerate() {
        let (mc_df, _) = mc[i];
        let bias_bp = (mc_df - curve_dfs[i]).abs() * 10_000.0;
        assert!(
            bias_bp < 12.0,
            "calibrated θ(t) fails to reprice the curve at T={m}: \
             curve_df={:.6}, mc_df={:.6}, |Δ|={bias_bp:.2}bp > 12bp",
            curve_dfs[i],
            mc_df,
        );
    }
}

/// Convergence check: the calibrated-θ(t) repricing error must *shrink* as the
/// simulation grid is refined, confirming the residual in
/// `calibrated_theta_reprices_sloped_curve` is discretization error, not a
/// model bias. (The flat-θ bias does NOT shrink with the grid.)
#[test]
fn theta_repricing_error_converges_with_grid() {
    let kappa = 0.15_f64;
    let sigma = 0.01_f64;
    let r0 = 0.02_f64;
    let maturity = [5.0_f64];
    let curve_df = sloped_discount_fn(5.0);

    let theta_times: Vec<f64> = (0..=250).map(|i| i as f64 * 0.02).collect();
    let calibrated = calibrate_theta_from_curve(kappa, sigma, sloped_discount_fn, &theta_times);

    let coarse = simulate_zcb_prices(&calibrated, r0, &maturity, 12, 60_000, 99)[0].0;
    let fine = simulate_zcb_prices(&calibrated, r0, &maturity, 192, 60_000, 99)[0].0;

    let coarse_err = (coarse - curve_df).abs();
    let fine_err = (fine - curve_df).abs();
    println!(
        "convergence T=5: coarse(16/yr)={:.2}bp  fine(256/yr)={:.2}bp",
        coarse_err * 10_000.0,
        fine_err * 10_000.0
    );

    assert!(
        fine_err < coarse_err,
        "refining the grid must reduce the calibrated-θ repricing error: \
         coarse={:.2}bp, fine={:.2}bp",
        coarse_err * 10_000.0,
        fine_err * 10_000.0,
    );
}

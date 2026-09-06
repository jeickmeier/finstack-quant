//! Independent payoff and Gaussian-moment regressions for lookbacks and commodities.
use finstack_quant_core::math::norm_cdf;
use finstack_quant_models::closed_form::lookback::{
    fixed_strike_lookback_call, fixed_strike_lookback_put, floating_strike_lookback_put,
};
use finstack_quant_models::monte_carlo::{
    discretization::ExactSchwartzSmith,
    process::schwartz_smith::{SchwartzSmithParams, SchwartzSmithProcess},
    traits::Discretization,
};

fn simpson_reflected(spot: f64, lower: f64, drift: f64, vol: f64, t: f64, sign: f64) -> f64 {
    let upper = lower + 12.0 * vol * t.sqrt() + drift.abs() * t;
    let n = 20_000;
    let h = (upper - lower) / n as f64;
    let mut integral = 0.0;
    for i in 0..=n {
        let x = lower + i as f64 * h;
        let survival = norm_cdf((drift * t - x) / (vol * t.sqrt()))
            + (2.0 * drift * x / (vol * vol)).exp() * norm_cdf((-drift * t - x) / (vol * t.sqrt()));
        let weight = if i == 0 || i == n {
            1.0
        } else if i % 2 == 0 {
            2.0
        } else {
            4.0
        };
        integral += weight * spot * (sign * x).exp() * survival;
    }
    integral * h / 3.0
}

fn maximum_mean(spot: f64, maximum: f64, rate: f64, dividend: f64, vol: f64, t: f64) -> f64 {
    let drift = rate - dividend - 0.5 * vol * vol;
    maximum + simpson_reflected(spot, (maximum / spot).ln(), drift, vol, t, 1.0)
}

fn deficit_below(spot: f64, threshold: f64, rate: f64, dividend: f64, vol: f64, t: f64) -> f64 {
    let drift = -(rate - dividend - 0.5 * vol * vol);
    simpson_reflected(spot, (spot / threshold).ln(), drift, vol, t, -1.0)
}

#[test]
fn lookbacks_match_independent_extremum_integrals_across_carry_and_history() {
    let (spot, vol, t, q) = (100.0_f64, 0.2_f64, 1.0_f64, 0.02_f64);
    for rate in [-0.01_f64, q, 0.05] {
        let df = (-rate * t).exp();
        for maximum in [spot, 120.0] {
            let expected_max = maximum_mean(spot, maximum, rate, q, vol, t);
            let put = floating_strike_lookback_put(spot, t, rate, q, vol, maximum);
            let expected_put = df * expected_max - spot * (-q * t).exp();
            assert!(
                (put - expected_put).abs() < 1e-8,
                "floating put: {put} vs {expected_put}"
            );
            for strike in [90.0_f64, 100.0, 130.0] {
                let expected =
                    df * (maximum_mean(spot, maximum.max(strike), rate, q, vol, t) - strike);
                let actual = fixed_strike_lookback_call(spot, strike, t, rate, q, vol, maximum);
                assert!(
                    (actual - expected).abs() < 1e-8,
                    "fixed call: {actual} vs {expected}"
                );
            }
        }
        for minimum in [80.0_f64, spot] {
            for strike in [70.0_f64, 100.0, 110.0] {
                let threshold = strike.min(minimum);
                let expected =
                    df * (strike - threshold + deficit_below(spot, threshold, rate, q, vol, t));
                let actual = fixed_strike_lookback_put(spot, strike, t, rate, q, vol, minimum);
                assert!(
                    (actual - expected).abs() < 1e-8,
                    "fixed put: {actual} vs {expected}"
                );
            }
        }
    }
}

#[test]
fn lookback_zero_carry_small_vol_converges_to_locked_in_payoffs() {
    let df = (-0.02_f64).exp();
    for vol in [0.0, 1e-4] {
        let call = fixed_strike_lookback_call(100.0, 100.0, 1.0, 0.02, 0.02, vol, 120.0);
        let put = fixed_strike_lookback_put(100.0, 100.0, 1.0, 0.02, 0.02, vol, 80.0);
        let floating = floating_strike_lookback_put(100.0, 1.0, 0.02, 0.02, vol, 120.0);
        for price in [call, put, floating] {
            assert!((price - 20.0 * df).abs() < 1e-10);
        }
    }
}

#[test]
fn schwartz_smith_joint_moments_match_futures_for_any_step_partition() {
    for rho in [-1.0, -0.5, 0.0, 0.8, 1.0] {
        for kappa in [1e-10_f64, 2.0] {
            let params = SchwartzSmithParams::new(kappa, 0.3, 0.02, 0.15, rho)
                .unwrap()
                .with_lambda_x(0.05)
                .unwrap();
            let process = SchwartzSmithProcess::new(params, 0.1, 4.5);
            for steps in [vec![1.0], vec![1.0 / 12.0; 12], vec![0.01, 0.19, 0.8]] {
                let mut disc = ExactSchwartzSmith::from_process(&process).unwrap();
                let grid = finstack_quant_models::monte_carlo::TimeGrid::uniform(1.0, 12).unwrap();
                disc.prepare(&process, &grid);
                let mut mean = process.initial_state();
                let (mut vx, mut vy, mut cov) = (0.0, 0.0, 0.0);
                for dt in steps {
                    let mut zero = [0.0; 2];
                    let mut first = [0.0; 2];
                    let mut second = [0.0; 2];
                    disc.step(&process, 0.0, dt, &mut zero, &[0.0, 0.0], &mut []);
                    disc.step(&process, 0.0, dt, &mut first, &[1.0, 0.0], &mut []);
                    disc.step(&process, 0.0, dt, &mut second, &[0.0, 1.0], &mut []);
                    for i in 0..2 {
                        first[i] -= zero[i];
                        second[i] -= zero[i];
                    }
                    let step_cov = first[0] * first[1] + second[0] * second[1];
                    let expected_cov = rho * 0.3 * 0.15 * -(-kappa * dt).exp_m1() / kappa;
                    assert!((step_cov - expected_cov).abs() < 1e-13);
                    let decay = (-kappa * dt).exp();
                    vx = decay * decay * vx + first[0].powi(2) + second[0].powi(2);
                    vy += first[1].powi(2) + second[1].powi(2);
                    cov = decay * cov + step_cov;
                    disc.step(&process, 0.0, dt, &mut mean, &[0.0, 0.0], &mut []);
                }
                let expected_spot = (mean[0] + mean[1] + 0.5 * (vx + vy + 2.0 * cov)).exp();
                assert!((expected_spot / process.futures_price(1.0) - 1.0).abs() < 1e-12);
            }
        }
    }
}

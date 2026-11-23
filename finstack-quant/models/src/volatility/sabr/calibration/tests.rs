use super::super::parameters::SabrParameters;
use super::calibrator::{SabrCalibrationOutcome, SabrCalibrator};
use super::fit::{initial_alpha_guess, vega_weight};
use super::multi_start::{
    compute_sabr_jacobian, deterministic_sabr_starts, run_deterministic_sabr_starts,
    sabr_termination_is_acceptable, unconstrained_to_bounded,
};
use super::shift::SabrShift;
use finstack_quant_core::Error;
use finstack_quant_core::market_data::surfaces::VolQuoteType;
use finstack_quant_core::math::solver_multi::LmSolution;
#[test]
fn test_sabr_atm_pinning_interpolates_when_grid_lacks_forward() {
    let true_params = SabrParameters::new(0.20, 0.5, 0.30, -0.25).expect("valid params");

    let forward = 100.0_f64;
    let expiry = 1.0_f64;

    // Strike grid deliberately OMITS the forward (100). The inner strikes 98
    // and 102 bracket F tightly; the nearest quote (98 or 102) is still
    // off-ATM, which is what the old nearest-strike pin would have used.
    let strikes = vec![90.0, 98.0, 102.0, 110.0];
    let market_vols: Vec<f64> = strikes
        .iter()
        .map(|&k| {
            true_params
                .implied_vol(forward, k, expiry, VolQuoteType::BlackLognormal)
                .expect("synthetic vol should compute")
        })
        .collect();

    let true_atm = true_params
        .implied_vol(forward, forward, expiry, VolQuoteType::BlackLognormal)
        .expect("true ATM vol should compute");

    // The nearest-strike quote (K=98) — what the OLD `find_atm_vol` would pin
    // to. Assert it is measurably off the true ATM so the test is meaningful.
    let nearest_vol = market_vols[1]; // K = 98
    let nearest_err = (nearest_vol - true_atm).abs();
    assert!(
        nearest_err > 5e-4,
        "test setup: nearest-strike quote {nearest_vol} must be off true ATM {true_atm}"
    );

    // Interpolation supplies an approximate ATM target, so pinning it perturbs
    // the original exact smile. Require explicit acceptance of that quote error.
    assert!(
        SabrCalibrator::new()
            .with_atm_pinning(true)
            .calibrate(
                forward,
                &strikes,
                &market_vols,
                expiry,
                0.5,
                VolQuoteType::BlackLognormal
            )
            .is_err()
    );
    let calibrated_atm = SabrCalibrator::new()
        .find_atm_vol(forward, &strikes, &market_vols)
        .expect("interpolated ATM quote");
    let interp_err = (calibrated_atm - true_atm).abs();

    // The interpolated pin must be substantially closer to the true ATM than
    // the nearest-strike quote — the concrete improvement from the fix.
    assert!(
        interp_err < nearest_err * 0.5,
        "interpolated ATM pin must beat the nearest-strike quote: \
         calibrated_atm={calibrated_atm} (err {interp_err:.6}), \
         nearest quote={nearest_vol} (err {nearest_err:.6}), true_atm={true_atm}"
    );
}

use finstack_quant_core::math::solver_multi::{LmStats, LmTerminationReason};

#[test]
fn sabr_acceptance_requires_finite_residual_within_tolerance() {
    let tolerance = 1.0e-4;
    assert!(!sabr_termination_is_acceptable(
        LmTerminationReason::ConvergedResidualNorm,
        1.0e-3,
        tolerance
    ));
    assert!(!sabr_termination_is_acceptable(
        LmTerminationReason::ConvergedResidualNorm,
        f64::NAN,
        tolerance
    ));
    assert!(sabr_termination_is_acceptable(
        LmTerminationReason::ConvergedResidualNorm,
        1.0e-5,
        tolerance
    ));
    assert!(sabr_termination_is_acceptable(
        LmTerminationReason::StepTooSmall,
        1.0e-5,
        tolerance
    ));
    assert!(!sabr_termination_is_acceptable(
        LmTerminationReason::MaxIterations,
        1.0e-5,
        tolerance
    ));
    assert!(!sabr_termination_is_acceptable(
        LmTerminationReason::DampingLimit,
        1.0e-5,
        tolerance
    ));
}

#[test]
fn deterministic_runner_filters_before_selection_and_retains_rejections() {
    let starts = vec![[0.02, 0.15, -0.6], [0.02, 0.4, 0.0]];
    let result = run_deterministic_sabr_starts(
        starts.clone(),
        1.0e-4,
        "test SABR",
        0.5,
        |start| {
            let (score, reason, iterations) = if start == starts[0] {
                (1.0e-2, LmTerminationReason::MaxIterations, 2)
            } else {
                (1.0e-5, LmTerminationReason::ConvergedResidualNorm, 3)
            };
            (
                Ok(LmSolution {
                    params: vec![0.02, start[1], start[2]],
                    stats: LmStats {
                        iterations,
                        accepted_steps: iterations,
                        residual_evals: iterations + 1,
                        jacobian_evals: iterations,
                        termination_reason: reason,
                        final_residual_norm: score,
                        last_step_norm: Some(0.0),
                        last_lambda: Some(1.0),
                    },
                }),
                iterations + 1,
            )
        },
        |solution| {
            Some(
                solution
                    .params
                    .as_slice()
                    .try_into()
                    .expect("three parameters"),
            )
        },
    )
    .expect("converged candidate within tolerance should win");

    assert_eq!(result.outcome.winning_start, starts[1]);
    assert_eq!(result.outcome.residual_evaluations, 7);
    assert_eq!(result.outcome.parameters.alpha, 0.02);
    assert_eq!(result.outcome.parameters.nu, 0.4);
    assert_eq!(result.outcome.parameters.rho, 0.0);
    assert_eq!(result.rejected.rejected_starts, 1);
    assert_eq!(result.rejected.solver_failures, 0);
    assert!(matches!(
        result.rejected.best_rejected,
        Some((score, LmTerminationReason::MaxIterations, 2))
            if score == 1.0e-2
    ));
}

#[test]
fn sabr_jacobian_matches_transformed_residuals() {
    for beta in [0.0, 0.5, 1.0] {
        let template = SabrParameters::new(0.3, beta, 0.5, 0.0).expect("parameters");
        let strikes: Vec<_> = [0.005, 0.009999999, 0.01, 0.010000001, 0.02]
            .into_iter()
            .map(|k| {
                super::super::expansion::PreparedSabrStrike::new(
                    &template,
                    0.01,
                    k,
                    2.0,
                    if beta == 0.0 {
                        VolQuoteType::Normal
                    } else {
                        VolQuoteType::BlackLognormal
                    },
                )
                .expect("strike")
            })
            .collect();
        let weights = [0.25, 0.5, 1.0, 0.5, 0.25];
        for coordinates in [
            [-2.0, -2.0, -2.0],
            [-2.0, 2.0, 2.0],
            [-2.0, -20.0, 20.0],
            [-2.0, 20.0, -20.0],
        ] {
            let mut jacobian = vec![0.0; strikes.len() * 3];
            if compute_sabr_jacobian(&strikes, &weights, beta, &coordinates, &mut jacobian).is_err()
            {
                let parameters = SabrParameters::new(
                    coordinates[0].exp(),
                    beta,
                    unconstrained_to_bounded(coordinates[1], 0.001, 2.0),
                    unconstrained_to_bounded(coordinates[2], -0.99, 0.99),
                )
                .expect("parameters");
                assert!(
                    strikes
                        .iter()
                        .any(|strike| strike.evaluate(&parameters).is_err()),
                    "valid trial unexpectedly lacks derivatives"
                );
                continue;
            }
            let residual = |x: &[f64; 3], i: usize| {
                let parameters = SabrParameters::new(
                    x[0].exp(),
                    beta,
                    unconstrained_to_bounded(x[1], 0.001, 2.0),
                    unconstrained_to_bounded(x[2], -0.99, 0.99),
                )
                .expect("transformed parameters");
                weights[i] * strikes[i].evaluate(&parameters).expect("vol")
            };
            for i in 0..strikes.len() {
                for j in 0..3 {
                    let analytic = jacobian[i * 3 + j];
                    let mut matches = 0;
                    for h in [1e-3, 3e-4, 1e-4, 3e-5] {
                        let mut up = coordinates;
                        let mut down = coordinates;
                        up[j] += h;
                        down[j] -= h;
                        let finite = (residual(&up, i) - residual(&down, i)) / (2.0 * h);
                        if (finite - analytic).abs()
                            < 2e-6 * finite.abs().max(analytic.abs()) + 2e-8
                        {
                            matches += 1;
                        }
                    }
                    assert!(
                        matches >= 2,
                        "beta={beta} x={coordinates:?} i={i} j={j} analytic={analytic}"
                    );
                }
            }
        }
    }
}

#[test]
fn sabr_preparation_fit_snapshot() {
    let mut outcomes = Vec::new();
    for beta in [0.0, 0.5, 1.0] {
        for shifted in [false, true] {
            for pinned in [false, true] {
                let forward = if shifted { -0.01 } else { 0.02 };
                let strikes: Vec<_> = (0..9).map(|i| forward + (i as f64 - 4.0) * 0.001).collect();
                let alpha = if beta == 0.0 {
                    0.01
                } else if beta == 0.5 {
                    0.05
                } else {
                    0.3
                };
                let source = if shifted {
                    SabrParameters::new_with_shift(alpha, beta, 0.5, -0.3, 0.03)
                } else {
                    SabrParameters::new(alpha, beta, 0.5, -0.3)
                }
                .expect("source");
                let vols: Vec<_> = strikes
                    .iter()
                    .map(|&strike| {
                        source
                            .implied_vol(
                                forward,
                                strike,
                                2.0,
                                if beta == 0.0 {
                                    VolQuoteType::Normal
                                } else if shifted {
                                    VolQuoteType::ShiftedBlackLognormal
                                } else {
                                    VolQuoteType::BlackLognormal
                                },
                            )
                            .expect("source volatility")
                    })
                    .collect();
                let calibrator =
                    SabrCalibrator::new()
                        .with_atm_pinning(pinned)
                        .with_shift(if shifted {
                            SabrShift::Fixed(0.03)
                        } else {
                            SabrShift::None
                        });
                let fit = calibrator
                    .calibrate(
                        forward,
                        &strikes,
                        &vols,
                        2.0,
                        beta,
                        if beta == 0.0 {
                            VolQuoteType::Normal
                        } else if shifted {
                            VolQuoteType::ShiftedBlackLognormal
                        } else {
                            VolQuoteType::BlackLognormal
                        },
                    )
                    .expect("fit");
                let repeat = calibrator
                    .calibrate(
                        forward,
                        &strikes,
                        &vols,
                        2.0,
                        beta,
                        if beta == 0.0 {
                            VolQuoteType::Normal
                        } else if shifted {
                            VolQuoteType::ShiftedBlackLognormal
                        } else {
                            VolQuoteType::BlackLognormal
                        },
                    )
                    .expect("repeat");
                assert_eq!(
                    serde_json::to_string(&fit).expect("serialize"),
                    serde_json::to_string(&repeat).expect("serialize"),
                    "deterministic fit"
                );
                for (actual, expected) in [
                    (fit.parameters.alpha, source.alpha),
                    (fit.parameters.nu, source.nu),
                    (fit.parameters.rho, source.rho),
                ] {
                    assert!(
                        (actual - expected).abs() < 1e-6,
                        "identified synthetic parameter {actual} vs {expected}"
                    );
                }
                for i in 0..41 {
                    let strike = forward + (i as f64 - 20.0) * 0.0003;
                    let actual = fit
                        .parameters
                        .implied_vol(
                            forward,
                            strike,
                            2.0,
                            if beta == 0.0 {
                                VolQuoteType::Normal
                            } else if shifted {
                                VolQuoteType::ShiftedBlackLognormal
                            } else {
                                VolQuoteType::BlackLognormal
                            },
                        )
                        .expect("fit off grid");
                    let expected = source
                        .implied_vol(
                            forward,
                            strike,
                            2.0,
                            if beta == 0.0 {
                                VolQuoteType::Normal
                            } else if shifted {
                                VolQuoteType::ShiftedBlackLognormal
                            } else {
                                VolQuoteType::BlackLognormal
                            },
                        )
                        .expect("source off grid");
                    assert!(
                        (actual - expected).abs() < 1e-8 * expected.abs().max(0.01),
                        "off-grid smile {actual} vs {expected}"
                    );
                }
                assert!(fit.parameters_at_bounds.is_empty());
                let mut outcome = serde_json::to_value(fit).expect("outcome");
                outcome
                    .as_object_mut()
                    .expect("object")
                    .remove("residual_evaluations");
                outcomes.push(outcome);
            }
        }
    }
    if let Ok(path) = std::env::var("FQ_SABR_FIT_SNAPSHOT") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&outcomes).expect("snapshot"),
        )
        .expect("write snapshot");
    }
    if let Ok(path) = std::env::var("FQ_SABR_FIT_REFERENCE") {
        let expected = std::fs::read_to_string(path).expect("read reference");
        assert_eq!(
            serde_json::to_string_pretty(&outcomes).expect("serialize"),
            expected.trim_end(),
            "same-build fit parameters, work, selected start and quote residuals"
        );
    }
}

#[test]
fn deterministic_runner_counts_failed_rejected_and_tied_attempts() {
    let starts = vec![
        [0.02, 0.15, -0.6],
        [0.02, 0.4, 0.0],
        [0.02, 0.9, 0.6],
        [0.02, 0.4, 0.6],
    ];
    let mut attempt = 0;
    let result = run_deterministic_sabr_starts(
        starts.clone(),
        1e-4,
        "count test",
        0.5,
        |start| {
            attempt += 1;
            if attempt == 1 {
                return (Err(Error::Validation("forced failed solve".into())), 5);
            }
            (
                Ok(LmSolution {
                    params: start.to_vec(),
                    stats: LmStats {
                        iterations: 2,
                        accepted_steps: 2,
                        residual_evals: 999,
                        jacobian_evals: 1,
                        termination_reason: LmTerminationReason::ConvergedResidualNorm,
                        final_residual_norm: 1e-5,
                        last_step_norm: Some(0.0),
                        last_lambda: Some(1.0),
                    },
                }),
                attempt * 3,
            )
        },
        |solution| {
            (solution.params[1] != 0.9)
                .then(|| solution.params.as_slice().try_into().expect("parameters"))
        },
    )
    .expect("accepted starts");
    assert_eq!(attempt, 4);
    assert_eq!(result.outcome.residual_evaluations, 5 + 6 + 9 + 12);
    assert_eq!(
        result.outcome.winning_start, starts[1],
        "strict score ties retain first"
    );
    assert_eq!(result.outcome.total_iterations, 6);
    assert_eq!(result.outcome.winning_iterations, 2);
    assert_eq!(result.rejected.solver_failures, 1);
    assert_eq!(result.rejected.rejected_starts, 1);
}

#[test]
fn free_and_atm_pinned_multi_start_outputs_are_repeatable_and_fit() {
    let forward = 0.03;
    let strikes = [0.02, 0.025, 0.03, 0.035, 0.04];
    let time_to_expiry = 2.0;
    let beta = 0.5;
    let source_parameters = SabrParameters::new(0.02, beta, 0.4, -0.3).expect("source parameters");
    let market_vols: Vec<f64> = strikes
        .iter()
        .map(|&strike| {
            source_parameters
                .implied_vol(
                    forward,
                    strike,
                    time_to_expiry,
                    VolQuoteType::BlackLognormal,
                )
                .expect("source volatility")
        })
        .collect();
    let calibrator = SabrCalibrator::new()
        .with_tolerance(1.0e-8)
        .with_max_iterations(2_000);

    let calibrate_free = || {
        calibrator
            .calibrate(
                forward,
                &strikes,
                &market_vols,
                time_to_expiry,
                beta,
                VolQuoteType::BlackLognormal,
            )
            .expect("free-alpha calibration")
    };
    let calibrate_pinned = || {
        calibrator
            .clone()
            .with_atm_pinning(true)
            .calibrate(
                forward,
                &strikes,
                &market_vols,
                time_to_expiry,
                beta,
                VolQuoteType::BlackLognormal,
            )
            .expect("ATM-pinned calibration")
    };
    let free_first = calibrate_free();
    let free_second = calibrate_free();
    let pinned_first = calibrate_pinned();
    let pinned_second = calibrate_pinned();

    let assert_repeatable = |first: &SabrCalibrationOutcome, second: &SabrCalibrationOutcome| {
        assert_eq!(first.parameters.alpha, second.parameters.alpha);
        assert_eq!(first.parameters.beta, second.parameters.beta);
        assert_eq!(first.parameters.nu, second.parameters.nu);
        assert_eq!(first.parameters.rho, second.parameters.rho);
        assert_eq!(first.parameters.shift, second.parameters.shift);
        assert_eq!(first.total_iterations, second.total_iterations);
        assert_eq!(first.winning_iterations, second.winning_iterations);
        assert_eq!(first.residual_evaluations, second.residual_evaluations);
        assert_eq!(first.winning_start, second.winning_start);
        assert_eq!(first.parameters_at_bounds, second.parameters_at_bounds);
    };
    assert_repeatable(&free_first, &free_second);
    assert_repeatable(&pinned_first, &pinned_second);

    let residual_norm = |outcome: &SabrCalibrationOutcome, pin_atm: bool| {
        strikes
            .iter()
            .zip(&market_vols)
            .map(|(&strike, &market_vol)| {
                let is_atm = (strike - forward).abs() / forward.abs().max(1.0e-8) < 0.001;
                if pin_atm && is_atm {
                    0.0
                } else {
                    let weight = vega_weight(
                        forward,
                        strike,
                        market_vol,
                        time_to_expiry,
                        VolQuoteType::BlackLognormal,
                    )
                    .sqrt();
                    let model_vol = outcome
                        .parameters
                        .implied_vol(
                            forward,
                            strike,
                            time_to_expiry,
                            VolQuoteType::BlackLognormal,
                        )
                        .expect("fitted volatility");
                    weight * (model_vol - market_vol)
                }
            })
            .map(|residual| residual * residual)
            .sum::<f64>()
            .sqrt()
    };
    let residual_tolerance = calibrator.tolerance().sqrt();
    assert!(
        residual_norm(&free_first, false) <= residual_tolerance,
        "free-alpha winner must satisfy the configured residual tolerance"
    );
    assert!(
        residual_norm(&pinned_first, true) <= residual_tolerance,
        "ATM-pinned winner must satisfy the configured residual tolerance"
    );

    let atm_vol = calibrator
        .find_atm_vol(forward, &strikes, &market_vols)
        .expect("ATM volatility");
    let expected_alpha_start =
        initial_alpha_guess(atm_vol, forward, beta, VolQuoteType::BlackLognormal);
    for outcome in [&free_first, &pinned_first] {
        assert!((outcome.winning_start[0] - expected_alpha_start).abs() <= 1.0e-15);
        assert!(deterministic_sabr_starts(expected_alpha_start).contains(&outcome.winning_start));
        assert_eq!(outcome.parameters.beta, beta);
        assert_eq!(outcome.parameters.shift, None);
        assert!(outcome.parameters_at_bounds.is_empty());
        assert!((outcome.parameters.alpha - source_parameters.alpha).abs() <= 2.0e-6);
        assert!((outcome.parameters.nu - source_parameters.nu).abs() <= 5.0e-4);
        assert!((outcome.parameters.rho - source_parameters.rho).abs() <= 2.0e-4);
    }

    let pinned_atm = pinned_first
        .parameters
        .implied_vol(
            forward,
            forward,
            time_to_expiry,
            VolQuoteType::BlackLognormal,
        )
        .expect("pinned ATM volatility");
    assert!((pinned_atm - atm_vol).abs() <= calibrator.tolerance());
}

#[test]
fn free_alpha_path_reports_rejected_stalled_starts() {
    let calibrator = SabrCalibrator::new()
        .with_tolerance(1.0e-12)
        .with_max_iterations(1);
    let error = calibrator
        .calibrate(
            0.03,
            &[0.02, 0.025, 0.03, 0.035, 0.04],
            &[0.02, 0.015, 0.01, 0.015, 0.02],
            5.0,
            0.0,
            VolQuoteType::BlackLognormal,
        )
        .expect_err("one iteration cannot accept this bounded SABR smile");
    let message = error.to_string();

    assert!(message.contains("no acceptable deterministic SABR start"));
    assert!(message.contains("rejected_starts="));
    assert!(message.contains("best_rejected_residual="));
}

#[test]
fn atm_pinned_path_reports_rejected_stalled_starts() {
    let calibrator = SabrCalibrator::new()
        .with_tolerance(1.0e-12)
        .with_max_iterations(1);
    let error = calibrator
        .with_atm_pinning(true)
        .calibrate(
            0.03,
            &[0.02, 0.025, 0.03, 0.035, 0.04],
            &[0.02, 0.015, 0.01, 0.015, 0.02],
            5.0,
            0.0,
            VolQuoteType::BlackLognormal,
        )
        .expect_err("one iteration cannot accept this bounded ATM-pinned smile");
    let message = error.to_string();

    assert!(message.contains("no acceptable deterministic ATM-pinned SABR start"));
    assert!(message.contains("rejected_starts="));
    assert!(message.contains("best_rejected_residual="));
}

use super::super::multi_start::perturb_initial_guess;
use super::super::traits::{GlobalSolveTarget, TimeGridAndGuesses};
use super::diagnostics::*;
use super::optimize::*;
use finstack_quant_core::Result;

use crate::CalibrationConfig;
use finstack_quant_core::Error;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

#[test]
fn condition_number_detects_correlated_parameter_directions() {
    for rho in [0.9999_f64, 1.0 - 1e-8, -0.9999] {
        // J^T J = [[1, rho], [rho, 1]], with exact eigenvalues 1 +/- rho.
        // An all-ones iteration seed lies in just one of these eigenspaces.
        let jacobian = vec![vec![1.0, rho], vec![0.0, (1.0 - rho * rho).sqrt()]];
        let condition = compute_condition_number(&jacobian, 2, &[1.0, 1.0])
            .expect("correlated but full-rank Jacobian");
        let expected = (1.0 + rho.abs()) / (1.0 - rho.abs());
        assert!(
            (condition / expected - 1.0).abs() < 1e-7,
            "normal-equations condition {condition} should equal {expected} for rho={rho}"
        );
    }
}

#[test]
fn condition_number_uses_residual_weights_and_normal_equations() {
    let jacobian = vec![vec![1.0, 0.0], vec![0.0, 2.0]];
    // sqrt(W) J = diag(2, 6), so J^T W J = diag(4, 36).
    let condition = compute_condition_number(&jacobian, 2, &[2.0, 3.0])
        .expect("weighted Jacobian is full rank");
    assert!((condition - 9.0).abs() < 1e-12);
}

#[test]
fn condition_number_is_invariant_to_common_extreme_scales() {
    for (sensitivity_scale, weight_scale) in
        [(1e-200, 1.0), (1e200, 1.0), (1.0, 1e-200), (1.0, 1e200)]
    {
        let jacobian = vec![
            vec![sensitivity_scale, 0.0],
            vec![0.0, 2.0 * sensitivity_scale],
        ];
        let condition = compute_condition_number(&jacobian, 2, &[weight_scale, weight_scale])
            .expect("a common finite scale must preserve rank");
        assert!((condition - 4.0).abs() < 1e-12);
    }
}

#[test]
fn condition_number_does_not_report_a_finite_value_for_rank_deficiency() {
    for jacobian in [
        vec![vec![1.0, 1.0], vec![2.0, 2.0]],
        vec![vec![1.0, 0.0], vec![0.0, 0.0]],
        vec![vec![0.0, 0.0], vec![0.0, 0.0]],
    ] {
        assert!(compute_condition_number(&jacobian, 2, &[1.0, 1.0]).is_none());
    }
    let jacobian = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
    assert!(compute_condition_number(&jacobian, 2, &[1.0, 0.0]).is_none());
}

#[test]
fn condition_number_rejects_invalid_inputs() {
    assert!(compute_condition_number(&[vec![1.0]], 2, &[1.0]).is_none());
    assert!(compute_condition_number(&[vec![1.0]], 1, &[]).is_none());
    assert!(compute_condition_number(&[vec![1.0]], 1, &[-1.0]).is_none());
    for value in [f64::NAN, f64::INFINITY] {
        assert!(compute_condition_number(&[vec![value]], 1, &[1.0]).is_none());
        assert!(compute_condition_number(&[vec![1.0]], 1, &[value]).is_none());
    }
}

#[test]
fn global_parameter_bounds_validate_dimensions_finiteness_and_ordering() {
    let initials = [0.1, 0.2];
    assert!(
        validate_parameter_bounds(&initials, &Some(vec![0.0, 0.0]), &Some(vec![1.0, 1.0])).is_ok()
    );
    assert!(
        validate_parameter_bounds(&initials, &Some(vec![0.0]), &Some(vec![1.0, 1.0]))
            .expect_err("short lower bounds")
            .to_string()
            .contains("parameter count")
    );
    assert!(
        validate_parameter_bounds(&initials, &Some(vec![0.0, f64::NAN]), &Some(vec![1.0, 1.0]))
            .expect_err("non-finite lower bound")
            .to_string()
            .contains("must be finite")
    );
    assert!(
        validate_parameter_bounds(&initials, &Some(vec![0.0, 2.0]), &Some(vec![1.0, 1.0]))
            .expect_err("inverted bounds")
            .to_string()
            .contains("inverted")
    );
}

#[derive(Debug, Clone)]
struct DummyCurve(#[allow(dead_code)] Vec<f64>);

struct TestTarget {
    times: Vec<f64>,
    initials: Vec<f64>,
    residuals: Vec<f64>,
    weights: Option<Vec<f64>>,
    key_prefix: Option<String>,
}

impl TestTarget {
    fn new(times: Vec<f64>, initials: Vec<f64>, residuals: Vec<f64>) -> Self {
        Self {
            times,
            initials,
            residuals,
            weights: None,
            key_prefix: None,
        }
    }

    fn from_len(len: usize, residuals: Vec<f64>) -> Self {
        let times: Vec<f64> = (1..=len).map(|i| i as f64).collect();
        let initials = vec![0.0; len];
        Self::new(times, initials, residuals)
    }

    fn with_weights(mut self, weights: Vec<f64>) -> Self {
        self.weights = Some(weights);
        self
    }

    fn with_key_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.key_prefix = Some(prefix.into());
        self
    }
}

impl GlobalSolveTarget for TestTarget {
    type Quote = usize;
    type Curve = DummyCurve;

    fn build_time_grid_and_guesses(
        &self,
        quotes: &[Self::Quote],
    ) -> Result<TimeGridAndGuesses<Self::Quote>> {
        Ok(TimeGridAndGuesses {
            times: self.times.clone(),
            initials: self.initials.clone(),
            quotes: quotes.to_vec(),
        })
    }

    fn build_curve_from_params(&self, _times: &[f64], params: &[f64]) -> Result<Self::Curve> {
        Ok(DummyCurve(params.to_vec()))
    }

    fn residual_units(&self) -> crate::report::ResidualUnits {
        crate::report::ResidualUnits::PvPerUnitNotional
    }

    fn calculate_residuals(
        &self,
        _curve: &Self::Curve,
        _quotes: &[Self::Quote],
        residuals: &mut [f64],
    ) -> Result<()> {
        for (i, r) in residuals.iter_mut().enumerate() {
            *r = *self.residuals.get(i).unwrap_or(&0.0);
        }
        Ok(())
    }

    fn residual_key(&self, quote: &Self::Quote, _idx: usize) -> String {
        if let Some(prefix) = &self.key_prefix {
            format!("{}-{}", prefix, quote)
        } else {
            format!("GLOBAL-{:06}", quote)
        }
    }

    fn residual_weights(&self, quotes: &[Self::Quote], weights_out: &mut [f64]) -> Result<()> {
        if let Some(custom) = &self.weights {
            assert_eq!(
                custom.len(),
                quotes.len(),
                "weights must align with quotes for TestTarget"
            );
            for (out, value) in weights_out.iter_mut().zip(custom.iter()) {
                *out = *value;
            }
            Ok(())
        } else {
            if quotes.len() != weights_out.len() {
                return Err(finstack_quant_core::Error::Calibration {
                    message: format!(
                        "weights_out length mismatch ({} vs {})",
                        weights_out.len(),
                        quotes.len()
                    ),
                    category: "global_solve".to_string(),
                });
            }
            for weight in weights_out.iter_mut() {
                *weight = 1.0;
            }
            Ok(())
        }
    }
}

#[test]
fn rejects_colliding_residual_keys_before_acceptance() {
    let target = TestTarget::from_len(2, vec![0.005, 0.0]);
    let error = optimize(&target, &[0, 0], &CalibrationConfig::default(), 1e-6, true)
        .expect_err("a residual must never overwrite another observation");
    assert!(error.to_string().contains("duplicate residual key"));
}

/// Success is unweighted `max_i |r_i| <= validation_tolerance`.
///
/// A strip of residuals each equal to `tol` must succeed. Unweighted L2 is
/// `tol·√n`, so the old weighted-L2 / √n-scaled gate would have failed.
#[test]
fn success_gate_accepts_when_every_residual_equals_tolerance() {
    let tol = 0.1;
    let target = TestTarget::from_len(4, vec![tol; 4]);
    let quotes = vec![0usize, 1usize, 2usize, 3usize];
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1.0),
        ..Default::default()
    };
    let (_curve, report) =
        optimize(&target, &quotes, &config, tol, true).expect("optimization should complete");

    assert!(
        report.success,
        "every |r_i| == tol must succeed (convergence_reason: {})",
        report.convergence_reason,
    );
    assert_eq!(
        report
            .metadata
            .get("tolerance_definition")
            .map(String::as_str),
        Some("max_abs_residual"),
    );
}

/// One residual above `tol` must fail even when the weighted L2 is well inside
/// `tol` (small weights shrink the objective without shrinking the native residual).
#[test]
fn success_gate_fails_when_one_residual_exceeds_tolerance() {
    // Native residuals [0.15, 0.0]; weights 0.01 → √w = 0.1.
    // Weighted L2 = 0.015 << tol=0.1, but max |r| = 0.15 > 0.1.
    let target = TestTarget::from_len(2, vec![0.15, 0.0]).with_weights(vec![0.01, 0.01]);
    let quotes = vec![0usize, 1usize];
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1e-12),
        ..Default::default()
    };
    let (_curve, report) =
        optimize(&target, &quotes, &config, 0.1, true).expect("optimization should complete");
    assert!(
        !report.success,
        "max |r| > tol must fail even if weighted L2 is small \
         (objective_value={:.4}, reason: {})",
        report.objective_value, report.convergence_reason,
    );
    assert!(
        report.objective_value < 0.1,
        "weighted L2 must be below tol so this is not an L2-only failure"
    );
}

/// The old "weighted max vs sqrt(n)" success case: `|r|=0.5`, `w=0.01`, `tol=0.1`.
/// Weighted residuals are `[0.05, 0.0]` so the old gate passed; the new gate fails.
#[test]
fn success_gate_rejects_old_weighted_sqrt_n_success_case() {
    let target = TestTarget::from_len(2, vec![-0.5, 0.0]).with_weights(vec![0.01, 0.01]);
    let quotes = vec![0usize, 1usize];
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1e-12),
        ..Default::default()
    };

    // The 4th arg is the *validation/success* tolerance (distinct from the LM
    // solver tolerance set via `with_tolerance`).
    let (_curve, report) =
        optimize(&target, &quotes, &config, 0.1, true).expect("optimization should complete");

    assert!(
        !report.success,
        "|r|=0.5 > tol=0.1 must fail even though weighted max 0.05 was inside \
         the old √n-scaled bound (objective_value={:.4}, reason: {})",
        report.objective_value, report.convergence_reason,
    );
    assert_eq!(
        report.residuals.get("GLOBAL-000000"),
        Some(&-0.5),
        "residuals_map must store the signed residual"
    );
}

/// Item 2: `fill_penalty` must produce a residual vector that *depends on each
/// parameter*, so the finite-difference Jacobian has a non-zero column for every
/// parameter even when its ±h neighbourhood is also infeasible.
///
/// Pre-fix: every residual was set to one scalar `v(p)`; bumping a single parameter
/// while it stayed infeasible left `v` unchanged for that bump direction in the
/// pattern's structure — the per-column FD difference collapsed to zero because the
/// scalar mixed all parameters together and a one-parameter bump barely moved an
/// aggregate `‖violation‖`. Post-fix each residual keys on one specific parameter, so
/// a bump of parameter `j` strictly changes residual `j` (mod n_params).
#[test]
fn fill_penalty_yields_nonzero_jacobian_column_for_each_parameter() {
    let n_params = 3usize;
    let n_residuals = 6usize;
    // Lower bounds 0.0; all parameters start *below* the bound (infeasible) and stay
    // infeasible under a small bump — the exact regime that froze parameters before.
    let lb = vec![0.0_f64; n_params];
    let ub = vec![10.0_f64; n_params];

    let base_params = vec![-1.0_f64, -2.0, -0.5];
    let h = 1e-7_f64;

    for j in 0..n_params {
        let mut resid_base = vec![0.0_f64; n_residuals];
        fill_penalty(
            &mut resid_base,
            n_residuals,
            &base_params,
            Some(&lb),
            Some(&ub),
        );

        let mut bumped = base_params.clone();
        bumped[j] += h; // still < 0.0 -> still infeasible
        let mut resid_bumped = vec![0.0_f64; n_residuals];
        fill_penalty(
            &mut resid_bumped,
            n_residuals,
            &bumped,
            Some(&lb),
            Some(&ub),
        );

        // The FD column for parameter j: at least one residual must change.
        let column_norm: f64 = resid_base
            .iter()
            .zip(resid_bumped.iter())
            .map(|(a, b)| (b - a).abs())
            .fold(0.0_f64, f64::max);

        assert!(
            column_norm > 0.0,
            "fill_penalty produced a ZERO Jacobian column for parameter {j}: \
             LM would have no descent direction and freeze that parameter. \
             base={resid_base:?} bumped={resid_bumped:?}"
        );
    }
}

/// Item 2 (direction): the penalty must *decrease* as a violated parameter moves
/// back toward its bound, so the LM gradient points into the feasible box.
#[test]
fn fill_penalty_decreases_toward_feasible_region() {
    let lb = vec![0.0_f64];
    let ub = vec![10.0_f64];
    let n_residuals = 1usize;

    let mut deep = vec![0.0_f64; n_residuals];
    fill_penalty(&mut deep, n_residuals, &[-5.0], Some(&lb), Some(&ub));

    let mut shallow = vec![0.0_f64; n_residuals];
    fill_penalty(&mut shallow, n_residuals, &[-0.5], Some(&lb), Some(&ub));

    let mut feasible = vec![0.0_f64; n_residuals];
    fill_penalty(&mut feasible, n_residuals, &[5.0], Some(&lb), Some(&ub));

    assert!(
        deep[0] > shallow[0],
        "penalty must shrink as the parameter approaches the bound: deep={} shallow={}",
        deep[0],
        shallow[0],
    );
    assert!(
        shallow[0] > feasible[0],
        "penalty must be smallest when the parameter is interior: shallow={} feasible={}",
        shallow[0],
        feasible[0],
    );
}

#[test]
fn max_abs_residual_metadata_uses_absolute_value() {
    let target = TestTarget::from_len(2, vec![-0.02, -0.01]);
    let quotes = vec![0usize, 1usize];
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1.0),
        ..Default::default()
    };

    let (_curve, report) = optimize(
        &target,
        &quotes,
        &config,
        config.discount_curve.validation_tolerance,
        true,
    )
    .expect("optimization succeeds");

    let meta = report
        .metadata
        .get("final_unweighted_max_abs_residual")
        .expect("metadata should contain final_unweighted_max_abs_residual");
    let parsed: f64 = meta.parse().expect("metadata should parse as f64");

    assert!(
        (parsed - 2.0e-2).abs() < 1e-12,
        "final_unweighted_max_abs_residual metadata should use absolute values (got {})",
        parsed
    );
}

#[test]
fn rejects_length_mismatch_between_times_and_initials() {
    let target = TestTarget::new(vec![1.0, 2.0], vec![0.0], vec![0.0, 0.0]);
    let quotes = vec![0usize, 1usize];
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1.0),
        ..Default::default()
    };

    let err = optimize(
        &target,
        &quotes,
        &config,
        config.discount_curve.validation_tolerance,
        true,
    )
    .expect_err("should fail");
    let Error::Calibration { message, .. } = err else {
        unreachable!("length mismatch should return a Calibration error");
    };
    assert!(
        message.contains("times.len() == initials.len()"),
        "unexpected message: {}",
        message
    );
}

#[test]
fn rejects_non_increasing_times() {
    let target = TestTarget::new(vec![1.0, 0.5], vec![0.0, 0.0], vec![0.0, 0.0]);
    let quotes = vec![0usize, 1usize];
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1.0),
        ..Default::default()
    };

    let err = optimize(
        &target,
        &quotes,
        &config,
        config.discount_curve.validation_tolerance,
        true,
    )
    .expect_err("should fail");
    let Error::Calibration { message, .. } = err else {
        unreachable!("non-increasing times should return a Calibration error");
    };
    assert!(
        message.contains("strictly increasing"),
        "unexpected message: {}",
        message
    );
}

#[test]
fn rejects_non_finite_inputs() {
    let target = TestTarget::new(vec![f64::NAN], vec![f64::INFINITY], vec![0.0]);
    let quotes = vec![0usize];
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1.0),
        ..Default::default()
    };

    let err = optimize(
        &target,
        &quotes,
        &config,
        config.discount_curve.validation_tolerance,
        true,
    )
    .expect_err("should fail");
    let Error::Calibration { message, .. } = err else {
        unreachable!("non-finite inputs should return a Calibration error");
    };
    assert!(
        message.contains("non-negative finite times") || message.contains("finite initial guesses"),
        "unexpected message: {}",
        message
    );
}

#[test]
fn uses_custom_residual_keys_and_weights() {
    let target = TestTarget::from_len(2, vec![0.01, 0.02])
        .with_weights(vec![4.0, 1.0])
        .with_key_prefix("TEST");
    let quotes = vec![5usize, 7usize];
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1.0),
        ..Default::default()
    };

    let (_curve, report) = optimize(
        &target,
        &quotes,
        &config,
        config.discount_curve.validation_tolerance,
        true,
    )
    .expect("optimization succeeds");

    assert!(report.residuals.contains_key("TEST-5"));
    assert!(report.residuals.contains_key("TEST-7"));

    let weighted_l2 = report
        .metadata
        .get("final_weighted_resid_l2_norm")
        .expect("metadata should include final_weighted_resid_l2_norm");
    let expected = ((0.01_f64 * 2.0).powi(2) + (0.02_f64 * 1.0).powi(2)).sqrt();
    assert_eq!(
        weighted_l2,
        &format!("{:.2e}", expected),
        "weighted_l2_norm should reflect weights"
    );
}

#[test]
fn supports_overdetermined_least_squares() {
    let target = TestTarget::new(vec![1.0, 2.0], vec![0.0, 0.0], vec![0.01, -0.02, 0.03]);
    let quotes = vec![10usize, 11usize, 12usize];
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1e12),
        ..Default::default()
    };

    let (_curve, report) = optimize(
        &target,
        &quotes,
        &config,
        config.discount_curve.validation_tolerance,
        true,
    )
    .expect("should succeed");

    assert_eq!(report.residuals.len(), 3);
    assert_eq!(report.residuals.get("GLOBAL-000010"), Some(&0.01));
    assert_eq!(report.residuals.get("GLOBAL-000011"), Some(&-0.02));
    assert_eq!(report.residuals.get("GLOBAL-000012"), Some(&0.03));
}

#[test]
fn intermediate_eval_errors_are_reported_but_not_fatal_if_final_evaluation_succeeds() {
    struct FlakyResidualTarget {
        inner: TestTarget,
        fail_once: AtomicBool,
    }

    impl GlobalSolveTarget for FlakyResidualTarget {
        type Quote = usize;
        type Curve = DummyCurve;

        fn build_time_grid_and_guesses(
            &self,
            quotes: &[Self::Quote],
        ) -> Result<TimeGridAndGuesses<Self::Quote>> {
            self.inner.build_time_grid_and_guesses(quotes)
        }

        fn build_curve_from_params(&self, times: &[f64], params: &[f64]) -> Result<Self::Curve> {
            self.inner.build_curve_from_params(times, params)
        }

        fn residual_units(&self) -> crate::report::ResidualUnits {
            crate::report::ResidualUnits::PvPerUnitNotional
        }

        fn calculate_residuals(
            &self,
            curve: &Self::Curve,
            quotes: &[Self::Quote],
            residuals: &mut [f64],
        ) -> Result<()> {
            if self
                .fail_once
                .compare_exchange(false, true, AtomicOrdering::SeqCst, AtomicOrdering::SeqCst)
                .is_ok()
            {
                return Err(finstack_quant_core::Error::Calibration {
                    message: "intentional transient residual failure".to_string(),
                    category: "test".to_string(),
                });
            }
            self.inner.calculate_residuals(curve, quotes, residuals)
        }
    }

    let target = FlakyResidualTarget {
        inner: TestTarget::from_len(2, vec![0.01, 0.02]),
        fail_once: AtomicBool::new(false),
    };
    let quotes = vec![0usize, 1usize];
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1e12),
        ..Default::default()
    };

    let (_curve, report) = optimize(
        &target,
        &quotes,
        &config,
        config.discount_curve.validation_tolerance,
        true,
    )
    .expect("should succeed");

    let invalid_count: usize = report
        .metadata
        .get("invalid_eval_count")
        .expect("should include invalid_eval_count")
        .parse()
        .expect("invalid_eval_count should parse");
    assert!(invalid_count >= 1);
    assert!(
        report.metadata.contains_key("first_invalid_eval"),
        "should include first_invalid_eval"
    );
}

#[test]
fn analytical_jacobian_path_handles_more_quotes_than_probe_buffer() {
    struct EfficientTarget {
        times: Vec<f64>,
        targets: Vec<f64>,
    }

    impl GlobalSolveTarget for EfficientTarget {
        type Quote = usize;
        type Curve = DummyCurve;

        fn build_time_grid_and_guesses(
            &self,
            quotes: &[Self::Quote],
        ) -> Result<TimeGridAndGuesses<Self::Quote>> {
            Ok(TimeGridAndGuesses {
                times: self.times.clone(),
                initials: vec![0.0, 0.0],
                quotes: quotes.to_vec(),
            })
        }

        fn build_curve_from_params(&self, _times: &[f64], params: &[f64]) -> Result<Self::Curve> {
            Ok(DummyCurve(params.to_vec()))
        }

        fn residual_units(&self) -> crate::report::ResidualUnits {
            crate::report::ResidualUnits::PvPerUnitNotional
        }

        fn calculate_residuals(
            &self,
            curve: &Self::Curve,
            quotes: &[Self::Quote],
            residuals: &mut [f64],
        ) -> Result<()> {
            let level = curve.0[0] + curve.0[1];
            for (i, quote) in quotes.iter().enumerate() {
                residuals[i] = level - self.targets[*quote];
            }
            Ok(())
        }

        fn jacobian(
            &self,
            _params: &[f64],
            _times: &[f64],
            quotes: &[Self::Quote],
            jacobian: &mut [Vec<f64>],
        ) -> Result<()> {
            if jacobian.len() != quotes.len() {
                return Err(finstack_quant_core::Error::Calibration {
                    message: format!(
                        "jacobian row count mismatch: {} vs {}",
                        jacobian.len(),
                        quotes.len()
                    ),
                    category: "efficient_jacobian".to_string(),
                });
            }
            for row in jacobian.iter_mut() {
                row[0] = 1.0;
                row[1] = 1.0;
            }
            Ok(())
        }

        fn supports_efficient_jacobian(&self) -> bool {
            true
        }
    }

    let target = EfficientTarget {
        times: vec![1.0, 2.0],
        targets: vec![0.5; 10],
    };
    let quotes: Vec<usize> = (0..10).collect();
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default()
            .with_tolerance(1e-12)
            .with_max_iterations(50),
        ..Default::default()
    };

    let (_curve, report) = optimize(
        &target,
        &quotes,
        &config,
        config.discount_curve.validation_tolerance,
        true,
    )
    .expect("should succeed");

    assert_eq!(report.residuals.len(), quotes.len());
}

#[test]
fn rejects_empty_active_quotes() {
    let target = TestTarget::from_len(1, vec![0.0]);
    let quotes: Vec<usize> = Vec::new();
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1.0),
        ..Default::default()
    };

    let err = optimize(
        &target,
        &quotes,
        &config,
        config.discount_curve.validation_tolerance,
        true,
    )
    .expect_err("empty active quotes should fail");
    assert!(matches!(
        err,
        Error::Input(finstack_quant_core::InputError::TooFewPoints)
    ));
}

#[test]
fn rejects_underdetermined_least_squares_system() {
    let target = TestTarget::new(vec![1.0, 2.0, 3.0], vec![0.0, 0.0, 0.0], vec![0.01, 0.02]);
    let quotes = vec![0usize, 1usize];
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1.0),
        ..Default::default()
    };

    let err = optimize(
        &target,
        &quotes,
        &config,
        config.discount_curve.validation_tolerance,
        true,
    )
    .expect_err("n_residuals < n_params should fail");
    let Error::Calibration { message, .. } = err else {
        unreachable!("underdetermined systems should return a Calibration error");
    };
    assert!(
        message.contains("n_residuals >= n_params"),
        "unexpected message: {message}"
    );
}

#[test]
fn rejects_invalid_and_zero_residual_weights() {
    let quotes = vec![0usize, 1usize];
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1.0),
        ..Default::default()
    };

    for weights in [vec![-1.0, 1.0], vec![f64::NAN, 1.0]] {
        let target = TestTarget::from_len(2, vec![0.01, 0.02]).with_weights(weights);
        let err = optimize(
            &target,
            &quotes,
            &config,
            config.discount_curve.validation_tolerance,
            true,
        )
        .expect_err("invalid weights should fail");
        let Error::Calibration { message, .. } = err else {
            unreachable!("invalid residual weights should return a Calibration error");
        };
        assert!(
            message.contains("non-negative finite residual weights"),
            "unexpected message: {message}"
        );
    }

    let zero_target = TestTarget::from_len(2, vec![0.01, 0.02]).with_weights(vec![0.0, 0.0]);
    let zero_err = optimize(
        &zero_target,
        &quotes,
        &config,
        config.discount_curve.validation_tolerance,
        true,
    )
    .expect_err("all-zero weights should fail");
    let Error::Calibration { message, .. } = zero_err else {
        unreachable!("all-zero residual weights should return a Calibration error");
    };
    assert!(
        message.contains("at least one positive residual weight"),
        "unexpected message: {message}"
    );
}

#[test]
fn success_tolerance_override_controls_report_success() {
    let target = TestTarget::from_len(1, vec![0.15]);
    let quotes = vec![0usize];
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1.0),
        ..Default::default()
    };

    let (_curve, report) =
        optimize(&target, &quotes, &config, 0.1, true).expect("optimization should still complete");

    assert!(
        !report.success,
        "explicit success tolerance should be enforced"
    );
    assert_eq!(report.success_tolerance, Some(0.1));
    assert!(
        (report.objective_value - 0.15).abs() < 1e-12,
        "objective_value should be weighted L2 norm"
    );
}

#[test]
fn compute_diagnostics_populates_report_when_enabled() {
    let target = TestTarget::from_len(1, vec![0.01]);
    let quotes = vec![0usize];
    let config = CalibrationConfig {
        solver: crate::SolverConfig::default().with_tolerance(1.0),
        compute_diagnostics: true,
        ..Default::default()
    };

    let (_curve, report) = optimize(
        &target,
        &quotes,
        &config,
        config.discount_curve.validation_tolerance,
        true,
    )
    .expect("should succeed");

    let diagnostics = report
        .diagnostics
        .as_ref()
        .expect("diagnostics should be populated");
    assert_eq!(diagnostics.per_quote.len(), 1);
    assert!((diagnostics.max_residual - 0.01).abs() < 1e-12);
    assert!(diagnostics.condition_number.is_none());
    let jacobian = diagnostics.jacobian.as_ref().expect("jacobian");
    assert_eq!(jacobian.len(), diagnostics.per_quote.len());
}

#[test]
fn perturb_initial_guess_is_deterministic_and_respects_bounds() {
    let initials = vec![1.0, 2.0];
    let lb = vec![0.9, 1.7];
    let ub = vec![1.1, 2.5];

    // restart_idx 2 → halton(3, 2) = 0.75 on dim 0, halton(3, 3) = 1/9
    // on dim 1. Additive perturbation (scale 0.5):
    //   dim 0: 1.0 + 0.5·(2·0.75 − 1) = 1.0 + 0.25 = 1.25 → clamps to 1.1
    //   dim 1: 2.0 + 0.5·(2·(1/9) − 1) ≈ 2.0 − 0.389 = 1.611 → clamps to 1.7
    let first = perturb_initial_guess(&initials, 0.5, 2, Some(lb.as_slice()), Some(ub.as_slice()));
    let second = perturb_initial_guess(&initials, 0.5, 2, Some(lb.as_slice()), Some(ub.as_slice()));

    assert_eq!(
        first, second,
        "Halton-based perturbations should be deterministic"
    );
    assert!(
        (first[0] - 1.1).abs() < 1e-12,
        "first coordinate should clamp to upper bound, got {}",
        first[0]
    );
    assert!(
        (first[1] - 1.7).abs() < 1e-12,
        "second coordinate should clamp to lower bound, got {}",
        first[1]
    );
}

#[test]
fn global_clamp_handles_wide_narrow_and_fixed_bounds() {
    let mut out = Vec::new();
    let count = clamp_to_bounds(
        &[-1.0, 2.0, 8.0],
        &Some(vec![0.0, 1.0, 3.0]),
        &Some(vec![10.0, 1.0 + 1.0e-12, 3.0]),
        &mut out,
    );

    assert_eq!(count, 3);
    assert!(out[0] >= 0.0 && out[0] <= 10.0);
    assert!(out[1] >= 1.0 && out[1] <= 1.0 + 1.0e-12);
    assert_eq!(out[2], 3.0);
}

#[test]
fn global_clamp_handles_one_sided_bounds() {
    let mut lower_only = Vec::new();
    assert_eq!(
        clamp_to_bounds(&[-1.0], &Some(vec![0.0]), &None, &mut lower_only),
        1
    );
    assert!(lower_only[0] >= 0.0);

    let mut upper_only = Vec::new();
    assert_eq!(
        clamp_to_bounds(&[2.0], &None, &Some(vec![1.0]), &mut upper_only),
        1
    );
    assert!(upper_only[0] <= 1.0);
}

#[test]
fn global_clamp_handles_no_representable_interior() {
    let lower = 1.0_f64;
    let upper = f64::from_bits(lower.to_bits() + 1);
    let mut out = Vec::new();
    let count = clamp_to_bounds(
        &[0.0, 2.0],
        &Some(vec![lower, lower]),
        &Some(vec![upper, upper]),
        &mut out,
    );

    assert_eq!(count, 2);
    assert!(out.iter().all(|value| *value >= lower && *value <= upper));
}

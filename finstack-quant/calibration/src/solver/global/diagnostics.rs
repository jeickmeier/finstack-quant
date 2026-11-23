//! Input validation, bound clamping, and post-fit Jacobian/condition diagnostics.

use super::super::traits::GlobalSolveTarget;
use crate::CalibrationConfig;
use crate::report::{CalibrationDiagnostics, QuoteQuality};
use finstack_quant_core::Result;
use std::cell::RefCell;

pub(super) fn validate_global_inputs(
    times: &[f64],
    initials: &[f64],
    n_residuals: usize,
) -> Result<()> {
    if times.len() != initials.len() {
        return Err(finstack_quant_core::Error::Calibration {
            message: format!(
                "Global solve requires times.len() == initials.len(); got {} vs {}.",
                times.len(),
                initials.len()
            ),
            category: "global_solve".to_string(),
        });
    }

    if initials.is_empty() {
        return Err(finstack_quant_core::Error::Calibration {
            message: format!(
                "Global fit requires at least one parameter; got n_params=0 and n_residuals={}.",
                n_residuals
            ),
            category: "global_solve".to_string(),
        });
    }

    if n_residuals < initials.len() {
        return Err(finstack_quant_core::Error::Calibration {
            message: format!(
                "Global fit requires n_residuals >= n_params for a stable least-squares solve; got {} vs {}.",
                n_residuals,
                initials.len()
            ),
            category: "global_solve".to_string(),
        });
    }

    for (idx, &t) in times.iter().enumerate() {
        if !t.is_finite() || t < 0.0 {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Global solve requires non-negative finite times; got {} at index {}.",
                    t, idx
                ),
                category: "global_solve".to_string(),
            });
        }
    }

    for (idx, &init) in initials.iter().enumerate() {
        if !init.is_finite() {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Global solve requires finite initial guesses; got {} at index {}.",
                    init, idx
                ),
                category: "global_solve".to_string(),
            });
        }
    }

    for idx in 1..times.len() {
        let prev = times[idx - 1];
        let next = times[idx];
        if next <= prev {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Global solve requires strictly increasing times; indices {} -> {} have values {} -> {}.",
                    idx - 1,
                    idx,
                    prev,
                    next
                ),
                category: "global_solve".to_string(),
            });
        }
    }

    Ok(())
}

#[derive(Debug, Default)]
pub(super) struct EvalDiagnostics {
    pub(super) invalid_eval_count: usize,
    pub(super) first_invalid_eval: Option<String>,
}

pub(super) fn record_eval_error(
    store: &RefCell<EvalDiagnostics>,
    eval_idx: usize,
    stage: &str,
    params: &[f64],
    detail: &str,
) {
    let mut diag = store.borrow_mut();
    diag.invalid_eval_count += 1;
    if diag.first_invalid_eval.is_some() {
        return;
    }
    let (min_param, max_param) = param_range(params);
    diag.first_invalid_eval = Some(format!(
        "Global fit {stage} failed at eval #{eval_idx} (param_range=[{min:.4e}, {max:.4e}]): {detail}",
        stage = stage,
        eval_idx = eval_idx,
        min = min_param,
        max = max_param,
        detail = detail
    ));
}

pub(super) fn validate_parameter_bounds(
    initials: &[f64],
    lower: &Option<Vec<f64>>,
    upper: &Option<Vec<f64>>,
) -> Result<()> {
    let expected = initials.len();
    for (name, bounds) in [("lower", lower), ("upper", upper)] {
        if let Some(values) = bounds {
            if values.len() != expected {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "global solver {name} bounds length {} does not match parameter count {expected}",
                    values.len()
                )));
            }
            if let Some((index, value)) = values
                .iter()
                .enumerate()
                .find(|(_, value)| !value.is_finite())
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "global solver {name} bound at index {index} must be finite, got {value}"
                )));
            }
        }
    }
    if let (Some(lower), Some(upper)) = (lower, upper)
        && let Some(index) = lower
            .iter()
            .zip(upper)
            .position(|(lower, upper)| lower > upper)
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "global solver bounds are inverted at index {index}: lower={} > upper={}",
            lower[index], upper[index]
        )));
    }
    Ok(())
}

/// Inward offset applied when clamping solver parameters to bounds.
///
/// Keeps parameters strictly interior to the feasible region so that
/// finite-difference Jacobian perturbations (typically ~1e-7) do not land
/// exactly on the boundary, which would create a flat gradient plateau.
pub(super) const BOUND_INWARD_EPS: f64 = 1e-8;

/// Clamp `params` to representable points within the original bounds and return
/// the number of parameters that were actually clamped.
pub(super) fn clamp_to_bounds(
    params: &[f64],
    lb: &Option<Vec<f64>>,
    ub: &Option<Vec<f64>>,
    out: &mut Vec<f64>,
) -> usize {
    out.clear();
    out.reserve(params.len());
    let mut clamped = 0usize;
    for (i, &p) in params.iter().enumerate() {
        let lower_bound = lb.as_ref().map(|values| values[i]);
        let upper_bound = ub.as_ref().map(|values| values[i]);
        let value = match (lower_bound, upper_bound) {
            (Some(lower), Some(upper)) if lower.to_bits() == upper.to_bits() => lower,
            (Some(lower), Some(upper)) => {
                let width = upper - lower;
                let inward = BOUND_INWARD_EPS.min(0.25 * width);
                let interior_lower = (lower + inward).clamp(lower, upper);
                let interior_upper = (upper - inward).clamp(lower, upper);
                if interior_lower <= interior_upper {
                    p.clamp(interior_lower, interior_upper)
                } else {
                    // The interval has no representable interior point at this
                    // scale. Preserve the inclusive original bounds.
                    p.clamp(lower, upper)
                }
            }
            (Some(lower), None) => {
                let candidate = lower + BOUND_INWARD_EPS;
                let minimum = if candidate.is_finite() && candidate >= lower {
                    candidate
                } else {
                    lower
                };
                p.max(minimum).max(lower)
            }
            (None, Some(upper)) => {
                let candidate = upper - BOUND_INWARD_EPS;
                let maximum = if candidate.is_finite() && candidate <= upper {
                    candidate
                } else {
                    upper
                };
                p.min(maximum).min(upper)
            }
            (None, None) => p,
        };
        if value.to_bits() != p.to_bits() {
            clamped += 1;
        }
        out.push(value);
    }
    clamped
}

/// Return a comma-separated string naming the `k` quotes with the largest
/// absolute residuals, formatted as `"<key>=±<resid>"`. Used to make
/// calibration failure messages actionable without requiring the user to
/// re-run with `compute_diagnostics=true`.
pub(super) fn top_k_worst_fits<T>(
    target: &T,
    quotes: &[T::Quote],
    residuals: &[f64],
    k: usize,
) -> String
where
    T: GlobalSolveTarget,
{
    let mut indexed: Vec<(usize, f64)> = residuals
        .iter()
        .enumerate()
        .map(|(i, r)| (i, r.abs()))
        .collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    indexed
        .into_iter()
        .take(k)
        .filter_map(|(i, mag)| {
            quotes
                .get(i)
                .map(|q| format!("{}={:.2e}", target.residual_key(q, i), mag))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn param_range(params: &[f64]) -> (f64, f64) {
    if params.is_empty() {
        return (0.0, 0.0);
    }
    let mut min_val = f64::INFINITY;
    let mut max_val = f64::NEG_INFINITY;
    for &p in params {
        if p.is_finite() {
            if p < min_val {
                min_val = p;
            }
            if p > max_val {
                max_val = p;
            }
        }
    }
    if !min_val.is_finite() || !max_val.is_finite() {
        (0.0, 0.0)
    } else {
        (min_val, max_val)
    }
}

/// Compute calibration diagnostics after a global solve.
///
/// This function builds per-quote quality metrics using finite-difference
/// sensitivities (dResidual/dParam for the most sensitive parameter per quote),
/// computes the weighted Jacobian's normal equations (J^T * W * J) for condition number
/// estimation, and calculates residual summary statistics.
pub(super) fn compute_global_diagnostics<T>(
    target: &T,
    active_quotes: &[T::Quote],
    times: &[f64],
    solved_params: &[f64],
    resid_values: &[f64],
    weight_scales: &[f64],
    config: &CalibrationConfig,
) -> CalibrationDiagnostics
where
    T: GlobalSolveTarget,
{
    let n_residuals = resid_values.len();
    let n_params = solved_params.len();

    // 1. Compute per-quote quality with finite-difference sensitivities.
    let bump_h = super::super::bracketing::diagnostics_bump_h(config);
    let mut per_quote = Vec::with_capacity(n_residuals);

    // Build Jacobian via finite differences (n_params bumps, each producing n_residuals).
    // jacobian[i][j] = dResidual_i / dParam_j
    let mut jacobian: Vec<Vec<f64>> = vec![vec![0.0; n_params]; n_residuals];
    let mut jacobian_ok = true;

    let mut bumped = solved_params.to_vec();
    let mut resid_up = vec![0.0; n_residuals];
    let mut resid_dn = vec![0.0; n_residuals];

    for j in 0..n_params {
        let h = bump_h * (1.0 + solved_params[j].abs());

        // Central differences: O(h^2) accuracy
        bumped[j] = solved_params[j] + h;
        let ok_up = target
            .build_curve_from_params(times, &bumped)
            .and_then(|c| target.calculate_residuals(&c, active_quotes, &mut resid_up))
            .is_ok();

        bumped[j] = solved_params[j] - h;
        let ok_dn = target
            .build_curve_from_params(times, &bumped)
            .and_then(|c| target.calculate_residuals(&c, active_quotes, &mut resid_dn))
            .is_ok();

        bumped[j] = solved_params[j];

        if ok_up && ok_dn {
            for i in 0..n_residuals {
                jacobian[i][j] = (resid_up[i] - resid_dn[i]) / (2.0 * h);
            }
        } else if ok_up {
            for i in 0..n_residuals {
                jacobian[i][j] = (resid_up[i] - resid_values[i]) / h;
            }
        } else {
            jacobian_ok = false;
        }
    }

    // Build per-quote quality metrics using the Jacobian.
    for (i, (&resid, quote)) in resid_values.iter().zip(active_quotes.iter()).enumerate() {
        // Max absolute sensitivity across all parameters for this quote.
        let sensitivity = if jacobian_ok {
            jacobian[i].iter().map(|v| v.abs()).fold(0.0_f64, f64::max)
        } else {
            0.0
        };

        per_quote.push(QuoteQuality {
            quote_label: target.residual_key(quote, i),
            quote_value: target.quote_value(quote),
            residual: resid,
            sensitivity,
        });
    }

    // 2. Compute the condition number of J^T * W * J from its eigenvalues.
    let condition_number = if jacobian_ok && n_params > 0 {
        compute_condition_number(&jacobian, n_params, weight_scales)
    } else {
        None
    };

    // 3. Basic residual stats (shared with bootstrap so both paths report identically).
    CalibrationDiagnostics {
        per_quote,
        condition_number,
        // A partially filled matrix (a bump that could not be priced leaves a
        // zero column) is not exported.
        jacobian: jacobian_ok.then_some(jacobian),
        ..CalibrationDiagnostics::from_residuals(resid_values)
    }
}

/// Compute the 2-norm condition number of the weighted normal equations matrix
/// `J^T W J` using a symmetric eigendecomposition.
///
/// The eigenvalue ratio is the condition number of the normal equations, not its
/// square root (which would instead describe `sqrt(W) J`). Normalizing the weighted
/// Jacobian before forming the Gram matrix makes the result invariant to a common
/// scale and avoids squaring very large or small residual sensitivities. Returns
/// `None` for malformed or non-finite inputs and for numerical rank deficiency.
pub(super) fn compute_condition_number(
    jacobian: &[Vec<f64>],
    n_params: usize,
    weight_scales: &[f64],
) -> Option<f64> {
    use finstack_quant_core::math::linalg::symmetric_eigen;

    if n_params == 0
        || jacobian.len() != weight_scales.len()
        || jacobian.iter().any(|row| row.len() != n_params)
        || weight_scales.iter().any(|w| !w.is_finite() || *w < 0.0)
    {
        return None;
    }

    let mut scale = 0.0_f64;
    for (row, &weight) in jacobian.iter().zip(weight_scales) {
        for &value in row {
            let weighted = weight * value;
            if !weighted.is_finite() {
                return None;
            }
            scale = scale.max(weighted.abs());
        }
    }
    if scale == 0.0 {
        return None;
    }

    // Build the normalized J^T W J, with W = diag(weight_scales^2).
    let mut jtj = vec![0.0_f64; n_params.checked_mul(n_params)?];
    for (row, &weight) in jacobian.iter().zip(weight_scales) {
        for j in 0..n_params {
            let rj = (weight * row[j]) / scale;
            for k in j..n_params {
                let val = rj * ((weight * row[k]) / scale);
                jtj[j * n_params + k] += val;
                if k != j {
                    jtj[k * n_params + j] += val;
                }
            }
        }
    }

    let (eigenvalues, _) = symmetric_eigen(&jtj, n_params).ok()?;
    let mut lambda_max = 0.0_f64;
    let mut lambda_min = f64::INFINITY;
    for eigenvalue in eigenvalues {
        if !eigenvalue.is_finite() {
            return None;
        }
        lambda_max = lambda_max.max(eigenvalue);
        lambda_min = lambda_min.min(eigenvalue);
    }

    // The Gram matrix loses directions below its relative floating-point resolution.
    // Do not turn a rounded, tiny positive eigenvalue of a singular matrix into a
    // spurious finite condition number.
    let rank_tolerance = f64::EPSILON * n_params.max(jacobian.len()) as f64 * lambda_max;
    if lambda_max <= 0.0 || lambda_min <= rank_tolerance {
        return None;
    }

    Some(lambda_max / lambda_min)
}

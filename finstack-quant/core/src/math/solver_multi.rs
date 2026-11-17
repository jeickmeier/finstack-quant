//! Multi-dimensional optimization for model calibration.
//!
//! Provides robust algorithms for calibrating financial models with multiple
//! parameters, such as SABR volatility surfaces, Heston stochastic volatility,
//! and multi-curve bootstrapping problems.
//!
//! # Algorithms
//!
//! - **Levenberg-Marquardt**: Damped least-squares for sum-of-squares objectives
//!
//! # Use Cases
//!
//! - **SABR calibration**: Fit α, β, ρ, ν to market smiles
//! - **Heston calibration**: Fit stochastic volatility parameters to vanilla options
//! - **Multi-curve bootstrapping**: Simultaneous curve fitting with constraints
//! - **Smile interpolation**: Parametric volatility surface construction
//!
//! # Examples
//!
//! ```
//! use finstack_quant_core::math::solver_multi::LevenbergMarquardtSolver;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let solver = LevenbergMarquardtSolver::new(1e-8, 1e-12, 100)?;
//!
//! // Solve the residual system x + y = 5, x - y = 1.
//! let residuals = |params: &[f64], resid: &mut [f64]| {
//!     resid[0] = params[0] + params[1] - 5.0;
//!     resid[1] = params[0] - params[1] - 1.0;
//!     Ok(())
//! };
//!
//! let solution = solver.solve_least_squares(residuals, &[0.0, 0.0], 2)?;
//! assert!((solution.params[0] - 3.0).abs() < 1e-6);
//! assert!((solution.params[1] - 2.0).abs() < 1e-6);
//! # Ok(())
//! # }
//! ```
//!
//! # References
//!
//! - **Levenberg-Marquardt**:
//!   - Levenberg, K. (1944). "A Method for the Solution of Certain Non-Linear Problems
//!     in Least Squares." *Quarterly of Applied Mathematics*, 2(2), 164-168.
//!   - Marquardt, D. W. (1963). "An Algorithm for Least-Squares Estimation of Nonlinear
//!     Parameters." *Journal of the Society for Industrial and Applied Mathematics*, 11(2), 431-441.
//!   - Moré, J. J. (1978). "The Levenberg-Marquardt Algorithm: Implementation and Theory."
//!     *Numerical Analysis*, Lecture Notes in Mathematics, vol 630.
//!
//! - **Calibration Applications**:
//! - Hagan, P. S., et al. (2002). "Managing Smile Risk." *Wilmott Magazine*, September 2002.
//!   (SABR calibration techniques) `docs/REFERENCES.md#hagan-2002-sabr`
use crate::Result;

fn try_vec_filled(len: usize, value: f64) -> Result<Vec<f64>> {
    let mut vec = Vec::new();
    vec.try_reserve_exact(len).map_err(|err| {
        crate::Error::Validation(format!(
            "LM workspace allocation for {len} floats failed: {err}"
        ))
    })?;
    vec.resize(len, value);
    Ok(vec)
}

/// Pre-allocated scratch buffers for finite-difference Jacobian computation.
struct JacobianScratch {
    params_plus: Vec<f64>,
    params_minus: Vec<f64>,
    r_plus: Vec<f64>,
    r_minus: Vec<f64>,
}

impl JacobianScratch {
    fn try_new(n_params: usize, n_residuals: usize) -> Result<Self> {
        Ok(Self {
            params_plus: try_vec_filled(n_params, 0.0)?,
            params_minus: try_vec_filled(n_params, 0.0)?,
            r_plus: try_vec_filled(n_residuals, 0.0)?,
            r_minus: try_vec_filled(n_residuals, 0.0)?,
        })
    }
}

/// Pre-allocated workspace for the augmented column-pivoted QR step of LM.
struct QrStepWorkspace {
    aug: nalgebra::DMatrix<f64>,
    rhs: nalgebra::DVector<f64>,
    col_norms: Vec<f64>,
    step: Vec<f64>,
}

impl QrStepWorkspace {
    fn try_new(n_residuals: usize, n_params: usize) -> Result<Self> {
        let aug_rows = n_residuals.checked_add(n_params).ok_or_else(|| {
            invalid_lm("Levenberg-Marquardt augmented QR row count overflows usize")
        })?;
        let aug_len = aug_rows.checked_mul(n_params).ok_or_else(|| {
            invalid_lm("Levenberg-Marquardt augmented QR dimensions overflow usize")
        })?;
        let aug_data = try_vec_filled(aug_len, 0.0)?;
        let rhs_data = try_vec_filled(aug_rows, 0.0)?;
        Ok(Self {
            aug: nalgebra::DMatrix::from_vec(aug_rows, n_params, aug_data),
            rhs: nalgebra::DVector::from_vec(rhs_data),
            col_norms: try_vec_filled(n_params, 0.0)?,
            step: try_vec_filled(n_params, 0.0)?,
        })
    }
}

/// Overflow- and underflow-safe Euclidean 2-norm `||v||_2`.
#[inline]
fn scaled_norm(values: &[f64]) -> f64 {
    let mut scale = 0.0_f64;
    for &value in values {
        if !value.is_finite() {
            return f64::NAN;
        }
        scale = scale.max(value.abs());
    }
    if scale == 0.0 {
        return 0.0;
    }
    let sum_sq: f64 = values
        .iter()
        .map(|&value| {
            let u = value / scale;
            u * u
        })
        .sum();
    scale * sum_sq.sqrt()
}

/// Overflow- and underflow-safe Euclidean 2-norm of column `col` of an `m x n` row-major matrix.
#[inline]
fn scaled_column_norm(matrix: &[f64], m: usize, n: usize, col: usize) -> f64 {
    let mut scale = 0.0_f64;
    for row in 0..m {
        let value = matrix[row * n + col];
        if !value.is_finite() {
            return f64::NAN;
        }
        scale = scale.max(value.abs());
    }
    if scale == 0.0 {
        return 0.0;
    }
    let mut sum_sq = 0.0_f64;
    for row in 0..m {
        let u = matrix[row * n + col] / scale;
        sum_sq += u * u;
    }
    scale * sum_sq.sqrt()
}

/// Termination reason for the Levenberg-Marquardt solver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LmTerminationReason {
    /// Residual norm fell strictly below the configured residual tolerance.
    ConvergedResidualNorm,
    /// Parameter update step norm fell strictly below the configured step tolerance.
    StepTooSmall,
    /// Solver exhausted the configured damped-step attempt budget.
    MaxIterations,
    /// Damping parameter reached `LAMBDA_MAX` without reducing the residual norm.
    DampingLimit,
}

/// Solver statistics for diagnostics and monitoring.
#[derive(Clone, Debug)]
pub struct LmStats {
    /// Total attempted damped LM steps (`0` if the initial residual already meets tolerance).
    pub iterations: usize,
    /// Number of accepted damped steps that strictly decreased the residual norm.
    pub accepted_steps: usize,
    /// Total residual evaluations performed (including initial, Jacobian probe, and trial calls).
    pub residual_evals: usize,
    /// Total Jacobian evaluations or finite-difference assemblies performed.
    pub jacobian_evals: usize,
    /// Reason why the solver terminated.
    pub termination_reason: LmTerminationReason,
    /// Euclidean residual 2-norm corresponding to the returned accepted parameters.
    pub final_residual_norm: f64,
    /// Effective step 2-norm of the last attempted step, or `None` when no step was attempted.
    pub last_step_norm: Option<f64>,
    /// Damping parameter `lambda` used in the last attempted step, or `None` when no step was attempted.
    pub last_lambda: Option<f64>,
}

/// Solution vector plus solver statistics.
#[derive(Clone, Debug)]
pub struct LmSolution {
    /// Solved parameter vector.
    pub params: Vec<f64>,
    /// Detailed solver diagnostics.
    pub stats: LmStats,
}

/// Minimum bound for damping parameter λ (`1e-15` per CORE-NUMERICS-06).
const LAMBDA_MIN: f64 = 1e-15;

/// Maximum bound for damping parameter λ.
///
/// Lambda values above this threshold are clamped to prevent the solver from
/// becoming effectively stuck in pure gradient descent mode.
const LAMBDA_MAX: f64 = 1e15;

/// Relative floor applied to the `JᵀJ` diagonal in Marquardt damping.
///
/// The damping term is `λ · max(JᵀJ_ii, floor)` with
/// `floor = MARQUARDT_DIAG_FLOOR_REL × max_i(JᵀJ_ii)`, so a parameter with
/// (numerically) zero curvature still receives a small positive damping and
/// cannot make the damped normal-equations matrix singular or produce an
/// unbounded step.
const MARQUARDT_DIAG_FLOOR_REL: f64 = 1e-12;

/// Levenberg-Marquardt solver for non-linear least squares.
///
/// Minimizes the sum of squared residuals `||r(p)||_2^2` using an adaptive
/// trust-region damping parameter `λ` interpolated between Gauss-Newton and
/// diagonally scaled gradient descent. Particularly effective for curve
/// calibration, volatility surface fitting, and parameter estimation.
///
/// # Algorithm
///
/// At each iteration, solves the damped linear least-squares subproblem via
/// Householder QR factorization with column pivoting on the augmented matrix:
/// ```text
/// min_δ || [ J(p); sqrt(λ) · D ] δ - [ -r(p); 0 ] ||_2
///
/// where:
///   J(p) = Jacobian matrix (n_residuals x n_params)
///   r(p) = residual vector (n_residuals)
///   D    = positive diagonal scaling matrix from column norms of J(p)
///   λ    = adaptive damping parameter
///   δ    = candidate parameter update
/// ```
///
/// Both adaptive finite-difference perturbations and diagonal scaling are
/// coordinate-dependent (with a small relative column floor
/// `MARQUARDT_DIAG_FLOOR_REL`), so numerical behavior can vary if parameters
/// are rescaled by many orders of magnitude without normalization.
///
/// - **λ → 0**: Approaches Gauss-Newton steps
/// - **λ → ∞**: Approaches short diagonally scaled steepest-descent steps
///
/// # Damping Parameter Bounds
///
/// The damping parameter λ is bounded between `LAMBDA_MIN` and `LAMBDA_MAX` to
/// ensure numerical stability.
///
/// # Convergence and Termination
///
/// Callers must inspect `LmSolution::stats.termination_reason` rather than
/// assuming `Ok(...)` guarantees a small residual. Depending on the starting
/// point and problem conditioning, `solve_least_squares` and
/// `solve_least_squares_with_jacobian` return diagnostics for
/// `ConvergedResidualNorm`, `StepTooSmall`, `MaxIterations`, or
/// `DampingLimit`.
///
/// # Use Cases
///
/// - SABR volatility surface calibration
/// - Swaption volatility cube fitting
/// - Yield curve bootstrapping
/// - Credit curve calibration
///
/// # References
///
/// - Levenberg, K. (1944). "A Method for the Solution of Certain Non-Linear Problems
///   in Least Squares." *Quarterly of Applied Mathematics*, 2(2), 164-168.
/// - Marquardt, D. W. (1963). "An Algorithm for Least-Squares Estimation of Nonlinear
///   Parameters." *SIAM Journal*, 11(2), 431-441.
#[derive(Clone, Debug, PartialEq)]
pub struct LevenbergMarquardtSolver {
    residual_tolerance: f64,
    step_tolerance: f64,
    max_iterations: usize,
}

/// Initial damping parameter (lambda).
const LM_LAMBDA_INIT: f64 = 1e-3;
/// Factor for adjusting lambda (increase on failure, decrease on success).
const LM_LAMBDA_FACTOR: f64 = 10.0;

#[inline]
fn invalid_lm(reason: impl Into<String>) -> crate::Error {
    crate::Error::Validation(reason.into())
}

impl LevenbergMarquardtSolver {
    /// Create a validated Levenberg-Marquardt solver with explicit stopping tolerances and step budget.
    ///
    /// # Arguments
    ///
    /// * `residual_tolerance` - Strictly positive finite residual 2-norm convergence target `||r||_2 < residual_tolerance`.
    /// * `step_tolerance` - Strictly positive finite effective step 2-norm threshold (`||p_trial - p||_2 < step_tolerance`).
    /// * `max_iterations` - Maximum number of damped step attempts (`>= 1`).
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if either tolerance is non-finite or `<= 0.0`,
    /// or if `max_iterations == 0`.
    pub fn new(
        residual_tolerance: f64,
        step_tolerance: f64,
        max_iterations: usize,
    ) -> Result<Self> {
        if !residual_tolerance.is_finite() || residual_tolerance <= 0.0 {
            return Err(invalid_lm(format!(
                "LM residual_tolerance must be finite and positive, got {residual_tolerance}"
            )));
        }
        if !step_tolerance.is_finite() || step_tolerance <= 0.0 {
            return Err(invalid_lm(format!(
                "LM step_tolerance must be finite and positive, got {step_tolerance}"
            )));
        }
        if max_iterations == 0 {
            return Err(invalid_lm("LM max_iterations must be at least 1"));
        }
        Ok(Self {
            residual_tolerance,
            step_tolerance,
            max_iterations,
        })
    }

    /// Return the configured residual 2-norm tolerance.
    #[must_use]
    pub fn get_residual_tolerance(&self) -> f64 {
        self.residual_tolerance
    }

    /// Return the configured effective step 2-norm tolerance.
    #[must_use]
    pub fn get_step_tolerance(&self) -> f64 {
        self.step_tolerance
    }

    /// Return the configured maximum number of damped step attempts.
    #[must_use]
    pub fn get_max_iterations(&self) -> usize {
        self.max_iterations
    }

    /// Compute Jacobian for a system of residuals into a pre-allocated buffer.
    ///
    /// Writes a flat row-major Jacobian into `out`: `J[i * n_params + j]`.
    fn compute_jacobian_system_into<Res>(
        residuals: &mut Res,
        params: &[f64],
        base_resid: &[f64],
        n_residuals: usize,
        residual_eval_counter: &mut usize,
        scratch: &mut JacobianScratch,
        out: &mut [f64],
    ) -> Result<()>
    where
        Res: FnMut(&[f64], &mut [f64]) -> Result<()>,
    {
        let n_params = params.len();
        // ε^(1/3): optimal base step for central differences, balancing O(h²)
        // truncation against O(ε/h) rounding (Press et al. 2007, §5.7); the
        // per-component step is `h_j = fd_step × max(1, |x_j|)`.
        let fd_step = f64::EPSILON.cbrt();

        scratch.params_plus.copy_from_slice(params);
        scratch.params_minus.copy_from_slice(params);

        for (j, &p_j) in params.iter().enumerate() {
            let h = (p_j.abs() * fd_step).max(fd_step);
            let p_plus = p_j + h;
            let p_minus = p_j - h;
            if !p_plus.is_finite() || !p_minus.is_finite() || !(p_plus > p_j && p_j > p_minus) {
                return Err(invalid_lm(format!(
                    "LM finite-difference parameter probe overflowed or collapsed at coordinate {j}"
                )));
            }

            let step_plus = p_plus - p_j;
            let step_minus = p_j - p_minus;
            let total_span = step_plus + step_minus;
            if !step_plus.is_finite()
                || !step_minus.is_finite()
                || !total_span.is_finite()
                || step_plus <= 0.0
                || step_minus <= 0.0
                || total_span <= 0.0
            {
                return Err(invalid_lm(format!(
                    "LM finite-difference step span is invalid at coordinate {j}"
                )));
            }

            scratch.params_plus[j] = p_plus;
            scratch.r_plus.fill(f64::NAN);
            residuals(&scratch.params_plus, &mut scratch.r_plus)?;
            *residual_eval_counter = residual_eval_counter
                .checked_add(1)
                .ok_or_else(|| invalid_lm("LM residual evaluation counter overflowed usize"))?;
            if let Some((i, &val)) = scratch
                .r_plus
                .iter()
                .enumerate()
                .find(|(_, v)| !v.is_finite())
            {
                return Err(invalid_lm(format!(
                    "LM forward finite-difference residual component {i} for parameter {j} is non-finite or unwritten: {val}"
                )));
            }

            scratch.params_minus[j] = p_minus;
            scratch.r_minus.fill(f64::NAN);
            residuals(&scratch.params_minus, &mut scratch.r_minus)?;
            *residual_eval_counter = residual_eval_counter
                .checked_add(1)
                .ok_or_else(|| invalid_lm("LM residual evaluation counter overflowed usize"))?;
            if let Some((i, &val)) = scratch
                .r_minus
                .iter()
                .enumerate()
                .find(|(_, v)| !v.is_finite())
            {
                return Err(invalid_lm(format!(
                    "LM backward finite-difference residual component {i} for parameter {j} is non-finite or unwritten: {val}"
                )));
            }

            let w_plus = step_minus / total_span;
            let w_minus = step_plus / total_span;
            for i in 0..n_residuals {
                let slope_plus = (scratch.r_plus[i] - base_resid[i]) / step_plus;
                let slope_minus = (base_resid[i] - scratch.r_minus[i]) / step_minus;
                let deriv = w_plus * slope_plus + w_minus * slope_minus;
                if !deriv.is_finite() {
                    return Err(invalid_lm(format!(
                        "LM finite-difference Jacobian entry ({i}, {j}) is non-finite"
                    )));
                }
                out[i * n_params + j] = deriv;
            }
            scratch.params_plus[j] = p_j;
            scratch.params_minus[j] = p_j;
        }
        Ok(())
    }

    /// Solve the damped least-squares step `min_δ ||[J; √λ·D] δ - [-r; 0]||_2`
    /// using column-pivoted Householder QR (`nalgebra::ColPivQR`).
    ///
    /// Solves directly at condition number `κ(J)` without forming `JᵀJ` (which
    /// squares the condition number to `κ(J)²`). Normalizes the augmented
    /// matrix by `max_j ||J_{:,j}||_2` and the right-hand side by `||r||_∞`
    /// before Householder reflections so extreme residual/Jacobian scales
    /// cannot overflow or underflow `nalgebra`'s internal squared norms.
    fn solve_qr_step_into(
        &self,
        jacobian: &[f64],
        m: usize,
        n: usize,
        residuals: &[f64],
        lambda: f64,
        ws: &mut QrStepWorkspace,
    ) -> Result<()> {
        if m == 0 || n == 0 {
            ws.step.fill(0.0);
            return Ok(());
        }

        let mut c_max = 0.0_f64;
        for col in 0..n {
            let norm = scaled_column_norm(jacobian, m, n, col);
            if !norm.is_finite() {
                return Err(invalid_lm("LM Jacobian column norm is non-finite"));
            }
            ws.col_norms[col] = norm;
            c_max = c_max.max(norm);
        }

        // Exact zero Jacobian: the damped least-squares step is identically 0.
        if c_max == 0.0 {
            ws.step.fill(0.0);
            return Ok(());
        }

        let mut rhs_scale = 0.0_f64;
        for &r in residuals {
            if !r.is_finite() {
                return Err(invalid_lm("LM residual vector contains NaN or infinity"));
            }
            rhs_scale = rhs_scale.max(r.abs());
        }
        if rhs_scale == 0.0 {
            ws.step.fill(0.0);
            return Ok(());
        }

        // Preserve Marquardt's diagonal metric `d_j = max(||J_{:,j}||_2, floor)`
        // without squaring column norms (`floor = C` when `m < n`, else `1e-6 * C`).
        let floor_rel = if m < n {
            1.0
        } else {
            MARQUARDT_DIAG_FLOOR_REL.sqrt()
        };
        let sqrt_lambda = lambda.sqrt();
        if !sqrt_lambda.is_finite() || sqrt_lambda <= 0.0 {
            return Err(invalid_lm(
                "LM damping parameter is non-finite or non-positive",
            ));
        }

        ws.aug.fill(0.0);
        for row in 0..m {
            for col in 0..n {
                ws.aug[(row, col)] = jacobian[row * n + col] / c_max;
            }
        }
        for col in 0..n {
            let d_hat = (ws.col_norms[col] / c_max).max(floor_rel);
            let reg = sqrt_lambda * d_hat;
            if !reg.is_finite() || reg <= 0.0 {
                return Err(crate::Error::Validation(
                    "LM damping regularization underflowed or overflowed".to_string(),
                ));
            }
            ws.aug[(m + col, col)] = reg;
        }

        ws.rhs.fill(0.0);
        for (row, &resid) in residuals.iter().enumerate().take(m) {
            ws.rhs[row] = -resid / rhs_scale;
        }

        let qr = ws.aug.clone().col_piv_qr();
        qr.q_tr_mul(&mut ws.rhs);
        let r_mat = qr.r();

        let mut y = nalgebra::DVector::<f64>::zeros(n);
        for k in (0..n).rev() {
            let diag = r_mat[(k, k)];
            if !diag.is_finite() || diag == 0.0 {
                return Err(crate::Error::Validation(
                    "LM QR triangular factor has a zero or non-finite pivot".to_string(),
                ));
            }
            let mut sum = ws.rhs[k];
            for col in (k + 1)..n {
                sum -= r_mat[(k, col)] * y[col];
            }
            let val = sum / diag;
            if !val.is_finite() {
                return Err(crate::Error::Validation(
                    "LM QR back-substitution overflowed finite f64 range".to_string(),
                ));
            }
            y[k] = val;
        }

        qr.p().inv_permute_rows(&mut y);

        let ratio = rhs_scale / c_max;
        for j in 0..n {
            let step_j = if ratio.is_finite() && ratio != 0.0 {
                y[j] * ratio
            } else {
                (y[j] * rhs_scale) / c_max
            };
            if !step_j.is_finite() {
                return Err(crate::Error::Validation(
                    "LM parameter step overflowed finite f64 range".to_string(),
                ));
            }
            ws.step[j] = step_j;
        }

        Ok(())
    }

    fn solve_lm_core_with_stats<Res, Jac>(
        &self,
        mut params: Vec<f64>,
        mut residuals_func: Res,
        mut jacobian_func: Jac,
        n_residuals: usize,
        numerical_jacobian: bool,
    ) -> Result<LmSolution>
    where
        Res: FnMut(&[f64], &mut [f64]) -> Result<()>,
        Jac: FnMut(&mut Res, &[f64], &[f64], &mut usize, &mut [f64]) -> Result<()>,
    {
        if params.is_empty() || n_residuals == 0 {
            return Err(invalid_lm(
                "Levenberg-Marquardt requires non-empty parameters and positive residual count",
            ));
        }
        if let Some((idx, &val)) = params.iter().enumerate().find(|(_, v)| !v.is_finite()) {
            return Err(invalid_lm(format!(
                "Levenberg-Marquardt initial parameter {idx} must be finite, got {val}"
            )));
        }

        let n_params = params.len();
        let jac_len = n_residuals
            .checked_mul(n_params)
            .ok_or_else(|| invalid_lm("Levenberg-Marquardt Jacobian dimensions overflow usize"))?;
        let worst_case_resids = if numerical_jacobian {
            2usize
                .checked_mul(n_params)
                .and_then(|probes| probes.checked_add(1))
                .and_then(|per_attempt| self.max_iterations.checked_mul(per_attempt))
                .and_then(|total| total.checked_add(1))
        } else {
            self.max_iterations.checked_add(1)
        };
        if worst_case_resids.is_none() {
            return Err(invalid_lm(
                "Levenberg-Marquardt work-count bound overflows usize",
            ));
        }

        let mut qr_ws = QrStepWorkspace::try_new(n_residuals, n_params)?;
        let mut resid_vec = try_vec_filled(n_residuals, f64::NAN)?;
        let mut new_resid = try_vec_filled(n_residuals, f64::NAN)?;
        let mut new_params = try_vec_filled(n_params, 0.0)?;
        let mut step_diff = try_vec_filled(n_params, 0.0)?;
        let mut jacobian = try_vec_filled(jac_len, f64::NAN)?;

        let mut lambda = LM_LAMBDA_INIT;

        residuals_func(&params, &mut resid_vec)?;
        if let Some((idx, &val)) = resid_vec.iter().enumerate().find(|(_, v)| !v.is_finite()) {
            return Err(invalid_lm(format!(
                "LM initial residual component {idx} is non-finite or unwritten: {val}"
            )));
        }
        let mut resid_norm = scaled_norm(&resid_vec);
        if !resid_norm.is_finite() {
            return Err(invalid_lm("LM initial residual norm overflowed finite f64"));
        }
        let mut residual_evals = 1usize;
        let mut jacobian_evals = 0usize;
        let mut iterations = 0usize;
        let mut accepted_steps = 0usize;
        let mut last_step_norm = None;
        let mut last_lambda = None;

        tracing::debug!(
            n_params,
            n_residuals,
            initial_resid_norm = resid_norm,
            max_iter = self.max_iterations,
            "lm: start"
        );

        let build_solution = |params: Vec<f64>,
                              reason: LmTerminationReason,
                              iterations: usize,
                              accepted_steps: usize,
                              residual_evals: usize,
                              jacobian_evals: usize,
                              resid_norm: f64,
                              last_step_norm: Option<f64>,
                              last_lambda: Option<f64>| {
            LmSolution {
                params,
                stats: LmStats {
                    iterations,
                    accepted_steps,
                    residual_evals,
                    jacobian_evals,
                    termination_reason: reason,
                    final_residual_norm: resid_norm,
                    last_step_norm,
                    last_lambda,
                },
            }
        };

        if resid_norm < self.residual_tolerance {
            let reason = LmTerminationReason::ConvergedResidualNorm;
            tracing::debug!(iterations, resid_norm, ?reason, "lm: converged initially");
            return Ok(build_solution(
                params,
                reason,
                iterations,
                accepted_steps,
                residual_evals,
                jacobian_evals,
                resid_norm,
                last_step_norm,
                last_lambda,
            ));
        }

        let mut jacobian_valid = false;

        for _attempt in 0..self.max_iterations {
            if !jacobian_valid {
                jacobian_evals += 1;
                jacobian.fill(f64::NAN);
                jacobian_func(
                    &mut residuals_func,
                    &params,
                    &resid_vec,
                    &mut residual_evals,
                    &mut jacobian,
                )?;
                if let Some((idx, &val)) = jacobian.iter().enumerate().find(|(_, v)| !v.is_finite())
                {
                    let row = idx / n_params;
                    let col = idx % n_params;
                    return Err(invalid_lm(format!(
                        "LM Jacobian entry ({row}, {col}) is non-finite or unwritten: {val}"
                    )));
                }
                jacobian_valid = true;
            }

            self.solve_qr_step_into(
                &jacobian,
                n_residuals,
                n_params,
                &resid_vec,
                lambda,
                &mut qr_ws,
            )?;

            new_params.copy_from_slice(&params);
            for (i, &s) in qr_ws.step.iter().enumerate() {
                let candidate = new_params[i] + s;
                if !candidate.is_finite() {
                    return Err(invalid_lm(format!(
                        "LM candidate parameter {i} overflowed finite f64 range"
                    )));
                }
                new_params[i] = candidate;
                step_diff[i] = candidate - params[i];
            }

            let effective_step_norm = scaled_norm(&step_diff);
            if !effective_step_norm.is_finite() {
                return Err(invalid_lm("LM step norm overflowed finite f64 range"));
            }
            iterations += 1;
            last_step_norm = Some(effective_step_norm);
            last_lambda = Some(lambda);

            if effective_step_norm < self.step_tolerance {
                return Ok(build_solution(
                    params,
                    LmTerminationReason::StepTooSmall,
                    iterations,
                    accepted_steps,
                    residual_evals,
                    jacobian_evals,
                    resid_norm,
                    last_step_norm,
                    last_lambda,
                ));
            }

            new_resid.fill(f64::NAN);
            residuals_func(&new_params, &mut new_resid)?;
            residual_evals += 1;
            if let Some((idx, &val)) = new_resid.iter().enumerate().find(|(_, v)| !v.is_finite()) {
                return Err(invalid_lm(format!(
                    "LM trial residual component {idx} is non-finite or unwritten: {val}"
                )));
            }
            let new_norm = scaled_norm(&new_resid);
            if !new_norm.is_finite() {
                return Err(invalid_lm("LM trial residual norm overflowed finite f64"));
            }

            if new_norm < resid_norm {
                params.copy_from_slice(&new_params);
                resid_vec.copy_from_slice(&new_resid);
                resid_norm = new_norm;
                accepted_steps += 1;
                jacobian_valid = false;

                if resid_norm < self.residual_tolerance {
                    let reason = LmTerminationReason::ConvergedResidualNorm;
                    tracing::debug!(
                        iterations,
                        accepted_steps,
                        resid_norm,
                        ?reason,
                        "lm: converged"
                    );
                    return Ok(build_solution(
                        params,
                        reason,
                        iterations,
                        accepted_steps,
                        residual_evals,
                        jacobian_evals,
                        resid_norm,
                        last_step_norm,
                        last_lambda,
                    ));
                }

                lambda = (lambda / LM_LAMBDA_FACTOR).max(LAMBDA_MIN);
            } else if lambda >= LAMBDA_MAX {
                return Ok(build_solution(
                    params,
                    LmTerminationReason::DampingLimit,
                    iterations,
                    accepted_steps,
                    residual_evals,
                    jacobian_evals,
                    resid_norm,
                    last_step_norm,
                    last_lambda,
                ));
            } else {
                lambda = (lambda * LM_LAMBDA_FACTOR).min(LAMBDA_MAX);
            }
        }

        tracing::debug!(
            algorithm = "levenberg_marquardt",
            iterations,
            accepted_steps,
            max_iterations = self.max_iterations,
            final_residual_norm = resid_norm,
            ?last_step_norm,
            ?last_lambda,
            n_params,
            n_residuals,
            category = "max_iterations_exceeded",
            "lm: reached attempt budget without residual convergence"
        );
        Ok(build_solution(
            params,
            LmTerminationReason::MaxIterations,
            iterations,
            accepted_steps,
            residual_evals,
            jacobian_evals,
            resid_norm,
            last_step_norm,
            last_lambda,
        ))
    }

    /// Minimize `||r(x)||_2` via damped least squares using checked 3-point finite-difference Jacobians.
    ///
    /// Returns [`LmSolution`] carrying [`LmTerminationReason`] separately from consumer acceptance;
    /// callers should inspect `solution.stats.termination_reason` and any domain/repricing error
    /// gates before accepting parameters.
    ///
    /// # Arguments
    ///
    /// * `residuals` - Fallible callback populating `n_residuals` finite slots for a parameter slice.
    /// * `initial` - Non-empty finite initial parameter slice.
    /// * `n_residuals` - Positive number of residual terms written by `residuals`.
    ///
    /// # Errors
    ///
    /// Propagates the first callback error unchanged, or returns [`crate::Error::Validation`] if
    /// dimensions/counts overflow, buffers fail to reserve, any callback leaves slots non-finite/unwritten,
    /// finite-difference probes collapse/overflow, or QR/candidate arithmetic fails.
    pub fn solve_least_squares<Res>(
        &self,
        residuals: Res,
        initial: &[f64],
        n_residuals: usize,
    ) -> Result<LmSolution>
    where
        Res: FnMut(&[f64], &mut [f64]) -> Result<()>,
    {
        if initial.is_empty() || n_residuals == 0 {
            return Err(invalid_lm(
                "Levenberg-Marquardt requires non-empty parameters and positive residual count",
            ));
        }
        let n_params = initial.len();
        let mut initial_copy = Vec::new();
        initial_copy
            .try_reserve_exact(n_params)
            .map_err(|err| invalid_lm(format!("LM initial vector allocation failed: {err}")))?;
        initial_copy.extend_from_slice(initial);

        let mut jac_scratch = JacobianScratch::try_new(n_params, n_residuals)?;
        let jacobian_func = |res_fn: &mut Res,
                             p: &[f64],
                             base_r: &[f64],
                             eval_counter: &mut usize,
                             out: &mut [f64]| {
            Self::compute_jacobian_system_into(
                res_fn,
                p,
                base_r,
                n_residuals,
                eval_counter,
                &mut jac_scratch,
                out,
            )
        };

        self.solve_lm_core_with_stats(initial_copy, residuals, jacobian_func, n_residuals, true)
    }

    /// Minimize `||r(x)||_2` via damped least squares with a caller-supplied flat row-major Jacobian.
    ///
    /// Returns [`LmSolution`] carrying [`LmTerminationReason`] separately from consumer acceptance;
    /// callers should inspect `solution.stats.termination_reason` and any domain/repricing error
    /// gates before accepting parameters.
    ///
    /// # Arguments
    ///
    /// * `residuals` - Fallible callback populating `n_residuals` finite slots for a parameter slice.
    /// * `jacobian` - Fallible callback populating `n_residuals * initial.len()` flat row-major slots
    ///   `J[i * n_params + j] = dr_i / dp_j`.
    /// * `initial` - Non-empty finite initial parameter slice.
    /// * `n_residuals` - Positive number of residual terms written by `residuals`.
    ///
    /// # Errors
    ///
    /// Propagates the first residual or Jacobian callback error unchanged, or returns
    /// [`crate::Error::Validation`] if dimensions/counts overflow, buffers fail to reserve,
    /// any callback leaves slots non-finite/unwritten, or QR/candidate arithmetic fails.
    pub fn solve_least_squares_with_jacobian<Res, Jac>(
        &self,
        residuals: Res,
        mut jacobian: Jac,
        initial: &[f64],
        n_residuals: usize,
    ) -> Result<LmSolution>
    where
        Res: FnMut(&[f64], &mut [f64]) -> Result<()>,
        Jac: FnMut(&[f64], &mut [f64]) -> Result<()>,
    {
        if initial.is_empty() || n_residuals == 0 {
            return Err(invalid_lm(
                "Levenberg-Marquardt requires non-empty parameters and positive residual count",
            ));
        }
        let n_params = initial.len();
        let mut initial_copy = Vec::new();
        initial_copy
            .try_reserve_exact(n_params)
            .map_err(|err| invalid_lm(format!("LM initial vector allocation failed: {err}")))?;
        initial_copy.extend_from_slice(initial);

        let jacobian_func = |_res_fn: &mut Res,
                             p: &[f64],
                             _base_r: &[f64],
                             _eval_counter: &mut usize,
                             out: &mut [f64]| { jacobian(p, out) };

        self.solve_lm_core_with_stats(initial_copy, residuals, jacobian_func, n_residuals, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lm_solution_is_invariant_to_uniform_residual_scale() {
        for scale in [1e-12, 1.0, 1e12] {
            let solution = LevenbergMarquardtSolver::new(scale * 1e-10, 1e-12, 100)
                .expect("valid tolerances")
                .solve_least_squares(
                    |p, residuals| {
                        residuals[0] = scale * (p[0] - 2.0);
                        Ok(())
                    },
                    &[0.0],
                    1,
                )
                .expect("linear least squares should converge at every residual scale");
            assert_eq!(
                solution.stats.termination_reason,
                LmTerminationReason::ConvergedResidualNorm
            );
            assert!((solution.params[0] - 2.0).abs() < 1e-9, "scale={scale}");
        }
    }

    #[test]
    fn test_lm_stats_reports_convergence_reason() {
        let solver = LevenbergMarquardtSolver::new(1e-12, 1e-12, 100).expect("valid solver config");

        let residuals = |params: &[f64], resid: &mut [f64]| {
            resid[0] = params[0] + params[1] - 5.0;
            resid[1] = params[0] - params[1] - 1.0;
            Ok(())
        };

        let initial = vec![0.0, 0.0];
        let solution = solver
            .solve_least_squares(residuals, &initial, 2)
            .expect("stats solve should succeed");

        assert_eq!(
            solution.stats.termination_reason,
            LmTerminationReason::ConvergedResidualNorm
        );
        assert!(solution.stats.accepted_steps >= 1);
        assert!(solution.stats.residual_evals >= 1);
        assert!(solution.stats.jacobian_evals >= 1);
        assert!(solution.stats.last_step_norm.is_some());
        assert!(solution.stats.last_lambda.is_some());
    }

    #[test]
    fn test_lm_stats_reports_max_iterations_and_validates_config() {
        let solver = LevenbergMarquardtSolver::new(1e-12, 1e-12, 1).expect("valid solver config");

        let residuals = |params: &[f64], resid: &mut [f64]| {
            resid[0] = params[0] * params[0] - 2.0;
            Ok(())
        };

        let initial = vec![10.0];
        let solution = solver
            .solve_least_squares(residuals, &initial, 1)
            .expect("solve_least_squares returns diagnostics at max_iterations");
        assert_eq!(
            solution.stats.termination_reason,
            LmTerminationReason::MaxIterations
        );
        assert_eq!(solution.stats.iterations, 1);
        assert_eq!(solution.stats.accepted_steps, 1);
        assert!(solution.stats.last_step_norm.is_some());
        assert!(solution.stats.last_lambda.is_some());

        assert!(matches!(
            LevenbergMarquardtSolver::new(1e-12, 1e-12, 0),
            Err(crate::Error::Validation(_))
        ));
        assert!(matches!(
            LevenbergMarquardtSolver::new(0.0, 1e-12, 10),
            Err(crate::Error::Validation(_))
        ));
        assert!(matches!(
            LevenbergMarquardtSolver::new(-1e-8, 1e-12, 10),
            Err(crate::Error::Validation(_))
        ));
        assert!(matches!(
            LevenbergMarquardtSolver::new(f64::NAN, 1e-12, 10),
            Err(crate::Error::Validation(_))
        ));
        assert!(matches!(
            LevenbergMarquardtSolver::new(1e-12, 0.0, 10),
            Err(crate::Error::Validation(_))
        ));
        assert!(matches!(
            LevenbergMarquardtSolver::new(1e-12, f64::INFINITY, 10),
            Err(crate::Error::Validation(_))
        ));
        assert_eq!(solver.get_residual_tolerance(), 1e-12);
        assert_eq!(solver.get_step_tolerance(), 1e-12);
        assert_eq!(solver.get_max_iterations(), 1);
    }

    #[test]
    fn test_jacobian_system() {
        // System: x^2 + y^2 = 4, x - y = 0
        let solver = LevenbergMarquardtSolver::new(1e-10, 1e-12, 100).expect("valid solver config");

        let residuals = |params: &[f64], resid: &mut [f64]| {
            resid[0] = params[0] * params[0] + params[1] * params[1] - 4.0;
            resid[1] = params[0] - params[1];
            Ok(())
        };

        let jacobian = |params: &[f64], jac: &mut [f64]| {
            let x = params[0];
            let y = params[1];
            jac[0] = 2.0 * x; // df1/dx
            jac[1] = 2.0 * y; // df1/dy
            jac[2] = 1.0; // df2/dx
            jac[3] = -1.0; // df2/dy
            Ok(())
        };

        let initial = vec![1.0, 0.0];

        let solution = solver
            .solve_least_squares_with_jacobian(residuals, jacobian, &initial, 2)
            .expect("Minimization should succeed in test");

        assert_eq!(
            solution.stats.termination_reason,
            LmTerminationReason::ConvergedResidualNorm
        );
        let expected = 2.0_f64.sqrt();
        assert!((solution.params[0] - expected).abs() < 1e-8);
        assert!((solution.params[1] - expected).abs() < 1e-8);
    }

    #[test]
    fn test_solve_least_squares_overdetermined() {
        // Overdetermined system: 5 equations, 2 parameters
        // The least-squares solution for x + y = 5, x - y = 1 should still be (3, 2)
        // even with redundant constraints
        let solver = LevenbergMarquardtSolver::new(1e-8, 1e-12, 200).expect("valid solver config");

        let residuals = |params: &[f64], resid: &mut [f64]| {
            let x = params[0];
            let y = params[1];
            resid[0] = x + y - 5.0; // x + y = 5
            resid[1] = x - y - 1.0; // x - y = 1
            resid[2] = 2.0 * x - 6.0; // 2x = 6  (consistent: x=3)
            resid[3] = 2.0 * y - 4.0; // 2y = 4  (consistent: y=2)
            resid[4] = x + 2.0 * y - 7.0; // x + 2y = 7 (consistent)
            Ok(())
        };

        let initial = vec![0.0, 0.0];
        let solution = solver
            .solve_least_squares(residuals, &initial, 5)
            .expect("solve_least_squares should succeed");

        assert_eq!(
            solution.stats.termination_reason,
            LmTerminationReason::ConvergedResidualNorm
        );
        let result = &solution.params;
        assert!(
            (result[0] - 3.0).abs() < 1e-4,
            "x = {}, expected 3.0",
            result[0]
        );
        assert!(
            (result[1] - 2.0).abs() < 1e-4,
            "y = {}, expected 2.0",
            result[1]
        );
    }

    #[test]
    fn test_solve_least_squares_highly_overdetermined() {
        // Highly overdetermined: 20 residuals, 2 parameters
        // This tests that we don't panic even with n_residuals >> n_params
        let solver = LevenbergMarquardtSolver::new(1e-6, 1e-12, 200).expect("valid solver config");

        let residuals = |params: &[f64], resid: &mut [f64]| {
            let x = params[0];
            let y = params[1];
            // Generate 20 residuals all consistent with x=3, y=2
            for (i, r) in resid.iter_mut().enumerate().take(20) {
                let a = (i as f64) * 0.1;
                let b = 1.0 - a;
                // a*x + b*y = 3a + 2b = 3a + 2(1-a) = a + 2
                *r = a * x + b * y - (a + 2.0);
            }
            Ok(())
        };

        let initial = vec![0.0, 0.0];
        let solution = solver
            .solve_least_squares(residuals, &initial, 20)
            .expect("solve_least_squares should not panic with 20 residuals");

        assert_eq!(
            solution.stats.termination_reason,
            LmTerminationReason::ConvergedResidualNorm
        );
        let result = &solution.params;
        assert!(
            (result[0] - 3.0).abs() < 0.1,
            "x = {}, expected ~3.0",
            result[0]
        );
        assert!(
            (result[1] - 2.0).abs() < 0.1,
            "y = {}, expected ~2.0",
            result[1]
        );
    }

    #[test]
    fn test_solve_least_squares_returns_correct_length() {
        let solver = LevenbergMarquardtSolver::new(1e-8, 1e-12, 100).expect("valid solver config");

        let residuals = |params: &[f64], resid: &mut [f64]| {
            resid[0] = params[0] - 1.0;
            resid[1] = params[1] - 2.0;
            resid[2] = params[2] - 3.0;
            Ok(())
        };

        let initial = vec![0.0, 0.0, 0.0];
        let result = solver
            .solve_least_squares(residuals, &initial, 3)
            .expect("solve should succeed")
            .params;

        assert_eq!(result.len(), 3, "Result should have same length as initial");
    }

    #[test]
    fn test_jacobian_system_nonzero_residual_minimum() {
        let solver = LevenbergMarquardtSolver::new(1e-8, 1e-12, 50).expect("valid solver config");
        let residuals = |params: &[f64], resid: &mut [f64]| {
            for (i, out) in resid.iter_mut().enumerate() {
                *out = params[0] - (i as f64 + 1.0);
            }
            Ok(())
        };
        let jacobian = |_params: &[f64], jac: &mut [f64]| {
            jac.fill(1.0);
            Ok(())
        };
        let initial = vec![0.0];

        let solution = solver
            .solve_least_squares_with_jacobian(residuals, jacobian, &initial, 5)
            .expect("nonzero-residual optimum should return diagnostics");

        assert_eq!(solution.params.len(), 1);
        assert!((solution.params[0] - 3.0).abs() < 1e-8);
        assert_eq!(
            solution.stats.termination_reason,
            LmTerminationReason::StepTooSmall
        );
        assert!(solution.stats.final_residual_norm > 1.0);
    }

    #[test]
    fn test_lm_rejects_nan_and_inf_residuals_and_jacobian() {
        let solver = LevenbergMarquardtSolver::new(1e-8, 1e-12, 100).expect("valid solver config");
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let err = solver
                .solve_least_squares(
                    |_p, r| {
                        r[0] = bad;
                        Ok(())
                    },
                    &[0.0],
                    1,
                )
                .expect_err("non-finite initial residual must fail");
            assert!(matches!(err, crate::Error::Validation(_)));

            let err_probe = solver
                .solve_least_squares(
                    |p, r| {
                        r[0] = if p[0] == 0.0 { 1.0 } else { bad };
                        Ok(())
                    },
                    &[0.0],
                    1,
                )
                .expect_err("non-finite Jacobian probe must fail");
            assert!(matches!(err_probe, crate::Error::Validation(_)));
        }

        let err_jac = solver
            .solve_least_squares_with_jacobian(
                |p, r| {
                    r[0] = p[0] - 1.0;
                    Ok(())
                },
                |_p, j| {
                    j[0] = f64::NAN;
                    Ok(())
                },
                &[0.0],
                1,
            )
            .expect_err("NaN analytical Jacobian must fail");
        assert!(matches!(err_jac, crate::Error::Validation(_)));
    }

    #[test]
    fn test_lm_qr_solves_ill_conditioned_and_asymmetric_column_system() {
        // J = [[1, 100], [1e-7, 0]]: κ(J) ≈ 1e9 so JᵀJ loses double precision
        // (κ(J)² ≈ 1e18 >> 1/eps), whereas augmented Householder ColPivQR solves it cleanly.
        let eps = 1e-7_f64;
        let residuals = |params: &[f64], resid: &mut [f64]| {
            let x = params[0];
            let y = params[1];
            resid[0] = (x - 1.0) + 100.0 * (y - 2.0);
            resid[1] = eps * (x - 1.0);
            Ok(())
        };
        let solution = LevenbergMarquardtSolver::new(1e-14, 1e-14, 100)
            .expect("valid solver config")
            .solve_least_squares(residuals, &[0.0, 0.0], 2)
            .expect("augmented QR must solve ill-conditioned asymmetric system");
        assert_eq!(
            solution.stats.termination_reason,
            LmTerminationReason::ConvergedResidualNorm
        );
        assert!(
            (solution.params[0] - 1.0).abs() < 1e-6,
            "x={}",
            solution.params[0]
        );
        assert!(
            (solution.params[1] - 2.0).abs() < 1e-8,
            "y={}",
            solution.params[1]
        );
    }

    #[test]
    fn test_lm_converges_on_final_accepted_step() {
        let solver = LevenbergMarquardtSolver::new(1e-2, 1e-12, 1).expect("valid solver config");
        let residuals = |params: &[f64], resid: &mut [f64]| {
            resid[0] = params[0] - 2.0;
            Ok(())
        };
        let solution = solver
            .solve_least_squares(residuals, &[0.0], 1)
            .expect("one damped linear step should reach 1e-2 tolerance");
        assert_eq!(
            solution.stats.termination_reason,
            LmTerminationReason::ConvergedResidualNorm
        );
        assert!((solution.params[0] - 2.0).abs() < 1e-2);
    }

    #[test]
    fn test_lm_core_numerics_06_boundary_cases() {
        // 1. Initial residual already below residual_tolerance -> no step taken, Option fields None.
        let solver = LevenbergMarquardtSolver::new(1e-3, 1e-12, 20).expect("valid solver config");
        let sol0 = solver
            .solve_least_squares(
                |p, r| {
                    r[0] = p[0] - 1.0;
                    Ok(())
                },
                &[1.0 + 1e-5],
                1,
            )
            .expect("initial satisfied residual succeeds");
        assert_eq!(
            sol0.stats.termination_reason,
            LmTerminationReason::ConvergedResidualNorm
        );
        assert_eq!(sol0.stats.iterations, 0);
        assert_eq!(sol0.stats.accepted_steps, 0);
        assert_eq!(sol0.stats.residual_evals, 1);
        assert_eq!(sol0.stats.jacobian_evals, 0);
        assert_eq!(sol0.stats.last_step_norm, None);
        assert_eq!(sol0.stats.last_lambda, None);

        // 2. Poisoned unwritten slots are rejected (callbacks that omit writing fail validation).
        let err_unwritten_res = solver
            .solve_least_squares(|_p, _r| Ok(()), &[0.0], 1)
            .expect_err("unwritten residual buffer must fail NaN poisoning check");
        assert!(matches!(err_unwritten_res, crate::Error::Validation(_)));

        let err_unwritten_jac = solver
            .solve_least_squares_with_jacobian(
                |p, r| {
                    r[0] = p[0] - 1.0;
                    Ok(())
                },
                |_p, _j| Ok(()),
                &[0.0],
                1,
            )
            .expect_err("unwritten jacobian buffer must fail NaN poisoning check");
        assert!(matches!(err_unwritten_jac, crate::Error::Validation(_)));

        // 3. Finite-difference plus-probe error aborts immediately without evaluating minus probe.
        let mut minus_called = false;
        let err_probe = solver
            .solve_least_squares(
                |p, r| {
                    if (p[0] - 0.0).abs() < 1e-15 {
                        r[0] = 1.0;
                        Ok(())
                    } else if p[0] > 0.0 {
                        Err(crate::Error::Internal("probe sentinel".to_string()))
                    } else {
                        minus_called = true;
                        r[0] = 1.0;
                        Ok(())
                    }
                },
                &[0.0],
                1,
            )
            .expect_err("plus-probe failure must propagate");
        assert!(matches!(
            err_probe,
            crate::Error::Internal(ref msg) if msg == "probe sentinel"
        ));
        assert!(
            !minus_called,
            "minus probe must not execute after plus-probe Err"
        );

        // 4. Rounded unequal FD step at p = 1.0 uses exact 3-point Lagrange slope on p^2 - 1
        // and uses cached r0 (4 evals total in 1 iteration: init r0, plus, minus, trial).
        let mut evals = 0usize;
        let sol_fd = LevenbergMarquardtSolver::new(1e-12, 1e-15, 1)
            .expect("valid solver")
            .solve_least_squares(
                |p, r| {
                    evals += 1;
                    r[0] = p[0] * p[0] - 1.0;
                    Ok(())
                },
                &[1.0 + 1e-4],
                1,
            )
            .expect("quadratic 1-step FD succeeds");
        assert_eq!(evals, 4);
        assert_eq!(sol_fd.stats.residual_evals, 4);

        // 5. Extreme scale r ~ 1e200 and J = [1e200], r = [-2e200] takes damped step ~ 2 / 1.001.
        let sol_extreme = LevenbergMarquardtSolver::new(1.0, 1e-12, 1)
            .expect("valid solver")
            .solve_least_squares_with_jacobian(
                |p, r| {
                    r[0] = 1e200 * (p[0] - 2.0);
                    Ok(())
                },
                |_p, j| {
                    j[0] = 1e200;
                    Ok(())
                },
                &[0.0],
                1,
            )
            .expect("extreme residual/jacobian scale must not overflow");
        assert!(
            (sol_extreme.params[0] - 2.0 / 1.001).abs() < 1e-10,
            "expected ~{}, got {}",
            2.0 / 1.001,
            sol_extreme.params[0]
        );

        // 6. Underdetermined rank-deficient J = [1, 1], r = [-2] uses damped minimum-norm step.
        let sol_underdet = LevenbergMarquardtSolver::new(1e-12, 1e-12, 1)
            .expect("valid solver")
            .solve_least_squares_with_jacobian(
                |p, r| {
                    r[0] = (p[0] + p[1]) - 2.0;
                    Ok(())
                },
                |_p, j| {
                    j[0] = 1.0;
                    j[1] = 1.0;
                    Ok(())
                },
                &[0.0, 0.0],
                1,
            )
            .expect("underdetermined step must succeed");
        let expected_step = 2.0 / 2.001;
        assert!(
            (sol_underdet.params[0] - expected_step).abs() < 1e-10
                && (sol_underdet.params[1] - expected_step).abs() < 1e-10,
            "expected [{expected_step}, {expected_step}], got {:?}",
            sol_underdet.params
        );

        // 7. Inconsistent descent direction hits DampingLimit without extra saturated trial.
        let sol_damping = LevenbergMarquardtSolver::new(1e-12, 1e-15, 200)
            .expect("valid solver")
            .solve_least_squares_with_jacobian(
                |p, r| {
                    r[0] = 1e6 + p[0];
                    Ok(())
                },
                |_p, j| {
                    j[0] = -1.0;
                    Ok(())
                },
                &[0.0],
                1,
            )
            .expect("damping limit returns diagnostics");
        assert_eq!(
            sol_damping.stats.termination_reason,
            LmTerminationReason::DampingLimit
        );
        assert!(sol_damping.stats.iterations >= 1);
        assert_eq!(sol_damping.stats.accepted_steps, 0);
        assert!(
            sol_damping
                .stats
                .last_step_norm
                .is_some_and(|s| s.is_finite() && s > 0.0)
        );
        assert_eq!(sol_damping.stats.last_lambda, Some(LAMBDA_MAX));
    }
}

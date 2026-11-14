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
//! let solver = LevenbergMarquardtSolver::new().with_tolerance(1e-8);
//!
//! // Solve the residual system x + y = 5, x - y = 1.
//! let residuals = |params: &[f64], resid: &mut [f64]| {
//!     resid[0] = params[0] + params[1] - 5.0;
//!     resid[1] = params[0] - params[1] - 1.0;
//! };
//!
//! let solution = solver
//!     .solve_system(residuals, &[0.0, 0.0], 2)
//!     .expect("system solve should succeed");
//! assert!((solution.params[0] - 3.0).abs() < 1e-6);
//! assert!((solution.params[1] - 2.0).abs() < 1e-6);
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

/// Pre-allocated scratch buffers for finite-difference Jacobian computation.
struct JacobianScratch {
    params_plus: Vec<f64>,
    params_minus: Vec<f64>,
    r_plus: Vec<f64>,
    r_minus: Vec<f64>,
}

impl JacobianScratch {
    fn new(n_params: usize, n_residuals: usize) -> Self {
        Self {
            params_plus: vec![0.0; n_params],
            params_minus: vec![0.0; n_params],
            r_plus: vec![0.0; n_residuals],
            r_minus: vec![0.0; n_residuals],
        }
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
    fn new(n_residuals: usize, n_params: usize) -> Self {
        let aug_rows = n_residuals + n_params;
        Self {
            aug: nalgebra::DMatrix::zeros(aug_rows, n_params),
            rhs: nalgebra::DVector::zeros(aug_rows),
            col_norms: vec![0.0; n_params],
            step: vec![0.0; n_params],
        }
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

/// Trait for functions that can provide analytical derivatives.
///
/// This trait allows optimization algorithms to use exact derivatives
/// when available, significantly improving convergence speed and accuracy.
pub trait AnalyticalDerivatives {
    /// Compute the Jacobian matrix for a system of equations.
    ///
    /// # Arguments
    /// * `params` - Current parameter values
    /// * `jacobian` - Output buffer for Jacobian matrix (rows = equations, cols = params)
    ///
    /// Default implementation returns None, indicating Jacobian is not available.
    fn jacobian(&self, _params: &[f64], _jacobian: &mut [Vec<f64>]) -> Option<()> {
        None
    }

    /// Number of residual equations the Jacobian has rows for.
    fn residual_count(&self) -> usize;
}

/// Termination reason for the Levenberg-Marquardt solver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LmTerminationReason {
    /// Residual norm fell below the configured tolerance.
    ConvergedResidualNorm,
    /// Parameter update step norm fell below the solver's minimum step (`1e-12`).
    StepTooSmall,
    /// Solver exhausted the iteration budget.
    MaxIterations,
    /// Damping parameter reached `LAMBDA_MAX` without reducing the residual norm.
    DampingLimit,
}

/// Solver statistics for diagnostics and monitoring.
#[derive(Clone, Debug)]
pub struct LmStats {
    /// Number of accepted LM iterations.
    pub iterations: usize,
    /// Total residual evaluations performed (including Jacobian probes).
    pub residual_evals: usize,
    /// Total Jacobian evaluations performed.
    pub jacobian_evals: usize,
    /// Reason why the solver terminated.
    pub termination_reason: LmTerminationReason,
    /// Final residual norm when termination occurred.
    pub final_residual_norm: f64,
    /// Norm of the final accepted (or attempted) step.
    pub final_step_norm: f64,
    /// Final damping parameter (lambda) at termination.
    pub lambda_final: f64,
}

/// Solution vector plus solver statistics.
#[derive(Clone, Debug)]
pub struct LmSolution {
    /// Solved parameter vector.
    pub params: Vec<f64>,
    /// Detailed solver diagnostics.
    pub stats: LmStats,
}

/// Minimum bound for damping parameter λ (`√λ_min = 1e-12` in augmented QR).
///
/// Lambda values below this threshold are clamped to prevent numerical instability
/// from overly aggressive Gauss-Newton steps while allowing augmented QR to resolve
/// singular values down to `~1e-10` (well beyond normal-equations `f64` breakdown).
const LAMBDA_MIN: f64 = 1e-24;

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
/// Combines Gauss-Newton and gradient descent methods using a damping parameter
/// λ that adapts based on progress. Particularly effective for sum-of-squares
/// objectives arising in curve calibration and parameter fitting.
///
/// # Algorithm
///
/// At each iteration, solves:
/// ```text
/// (J^T J + λ·diag(J^T J)) δ = -J^T r
///
/// where:
///   J = Jacobian matrix
///   r = residual vector
///   λ = damping parameter (adaptive)
///   δ = parameter update
/// ```
///
/// The damping uses Marquardt's diagonal scaling `λ·diag(JᵀJ)` (with a small
/// relative floor, see `MARQUARDT_DIAG_FLOOR_REL`) rather than the unscaled
/// Levenberg `+λI`, so steps are invariant to per-parameter rescaling.
///
/// - **λ → 0**: Gauss-Newton (fast near solution)
/// - **λ → ∞**: Gradient descent (robust far from solution)
///
/// # Damping Parameter Bounds
///
/// The damping parameter λ is bounded to `LAMBDA_MIN` and `LAMBDA_MAX` to ensure
/// numerical stability.
///
/// # Convergence
///
/// Typically achieves quadratic convergence near the solution, switching to
/// linear convergence when far from optimum. More robust than pure Gauss-Newton.
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
#[derive(Clone, Debug)]
pub struct LevenbergMarquardtSolver {
    /// Convergence tolerance on the residual norm
    pub tolerance: f64,
    /// Maximum iterations
    pub max_iterations: usize,
}

/// Initial damping parameter (lambda).
const LM_LAMBDA_INIT: f64 = 1e-3;
/// Factor for adjusting lambda (increase on failure, decrease on success).
const LM_LAMBDA_FACTOR: f64 = 10.0;
/// Minimum allowed parameter step norm (for numerical stability).
const LM_MIN_STEP_SIZE: f64 = 1e-12;

impl Default for LevenbergMarquardtSolver {
    fn default() -> Self {
        Self {
            tolerance: 1e-8,
            max_iterations: 100,
        }
    }
}

#[inline]
fn invalid_lm(reason: &'static str) -> crate::Error {
    crate::Error::Validation(reason.to_string())
}

impl LevenbergMarquardtSolver {
    /// Create a new Levenberg-Marquardt solver with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set convergence tolerance.
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Set maximum iterations.
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Compute Jacobian for a system of residuals into a pre-allocated buffer.
    ///
    /// Writes a flat row-major Jacobian into `out`: `J[i * n_params + j]`.
    fn compute_jacobian_system_into<Res>(
        residuals: &Res,
        params: &[f64],
        base_resid: &[f64],
        n_residuals: usize,
        residual_eval_counter: &mut usize,
        scratch: &mut JacobianScratch,
        out: &mut [f64],
    ) -> Result<()>
    where
        Res: Fn(&[f64], &mut [f64]),
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
                return Err(invalid_lm(
                    "LM finite-difference parameter probe overflowed or collapsed",
                ));
            }

            let step_plus = p_plus - p_j;
            let step_minus = p_j - p_minus;
            let total_span = p_plus - p_minus;
            if !step_plus.is_finite()
                || !step_minus.is_finite()
                || !total_span.is_finite()
                || step_plus <= 0.0
                || step_minus <= 0.0
                || total_span <= 0.0
            {
                return Err(invalid_lm("LM finite-difference step span is invalid"));
            }

            scratch.params_plus[j] = p_plus;
            scratch.r_plus.fill(f64::NAN);
            residuals(&scratch.params_plus, &mut scratch.r_plus);
            *residual_eval_counter += 1;
            if scratch.r_plus.iter().any(|value| !value.is_finite()) {
                return Err(invalid_lm(
                    "LM forward finite-difference residual contains NaN or infinity",
                ));
            }

            scratch.params_minus[j] = p_minus;
            scratch.r_minus.fill(f64::NAN);
            residuals(&scratch.params_minus, &mut scratch.r_minus);
            *residual_eval_counter += 1;
            if scratch.r_minus.iter().any(|value| !value.is_finite()) {
                return Err(invalid_lm(
                    "LM backward finite-difference residual contains NaN or infinity",
                ));
            }

            let w_plus = step_minus / total_span;
            let w_minus = step_plus / total_span;
            for i in 0..n_residuals {
                let slope_plus = (scratch.r_plus[i] - base_resid[i]) / step_plus;
                let slope_minus = (base_resid[i] - scratch.r_minus[i]) / step_minus;
                let deriv = w_plus * slope_plus + w_minus * slope_minus;
                if !deriv.is_finite() {
                    return Err(invalid_lm(
                        "LM finite-difference Jacobian entry is non-finite",
                    ));
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
        residuals_func: &Res,
        mut jacobian_func: Jac,
        n_residuals: usize,
        require_residual_convergence: bool,
    ) -> Result<LmSolution>
    where
        Res: Fn(&[f64], &mut [f64]),
        Jac: FnMut(&[f64], &[f64], &mut usize, &mut [f64]) -> Result<()>,
    {
        if !self.tolerance.is_finite() || self.tolerance <= 0.0 || self.max_iterations == 0 {
            return Err(invalid_lm(
                "Levenberg-Marquardt requires positive finite tolerance and max_iterations > 0",
            ));
        }
        if params.is_empty() || n_residuals == 0 {
            return Err(invalid_lm(
                "Levenberg-Marquardt requires non-empty parameters and positive residual count",
            ));
        }
        if params.iter().any(|value| !value.is_finite()) {
            return Err(invalid_lm(
                "Levenberg-Marquardt initial parameters must be finite",
            ));
        }

        let n_params = params.len();
        let Some(jac_len) = n_residuals.checked_mul(n_params) else {
            return Err(invalid_lm(
                "Levenberg-Marquardt Jacobian dimensions overflow usize",
            ));
        };
        if n_residuals
            .checked_add(n_params)
            .and_then(|rows| rows.checked_mul(n_params))
            .is_none()
        {
            return Err(invalid_lm(
                "Levenberg-Marquardt augmented QR dimensions overflow usize",
            ));
        }

        let mut lambda = LM_LAMBDA_INIT;

        let mut resid_vec = vec![f64::NAN; n_residuals];
        let mut new_resid = vec![f64::NAN; n_residuals];
        let mut new_params = vec![0.0; n_params];
        let mut step_diff = vec![0.0; n_params];
        let mut jacobian = vec![f64::NAN; jac_len];
        let mut qr_ws = QrStepWorkspace::new(n_residuals, n_params);

        residuals_func(&params, &mut resid_vec);
        if resid_vec.iter().any(|value| !value.is_finite()) {
            return Err(invalid_lm(
                "LM initial residual vector contains NaN or infinity",
            ));
        }
        let mut resid_norm = scaled_norm(&resid_vec);
        if !resid_norm.is_finite() {
            return Err(invalid_lm("LM initial residual norm overflowed finite f64"));
        }
        let mut residual_evals = 1usize;
        let mut jacobian_evals = 0usize;
        let mut iterations = 0usize;
        let mut last_step_norm = 0.0_f64;

        tracing::debug!(
            n_params,
            n_residuals,
            initial_resid_norm = resid_norm,
            max_iter = self.max_iterations,
            "lm: start"
        );

        let finalize = |params: Vec<f64>,
                        reason: LmTerminationReason,
                        iterations: usize,
                        residual_evals: usize,
                        jacobian_evals: usize,
                        resid_norm: f64,
                        last_step_norm: f64,
                        lambda: f64|
         -> Result<LmSolution> {
            if require_residual_convergence && resid_norm > self.tolerance {
                let last_x = params.first().copied().unwrap_or(f64::NAN);
                return Err(crate::InputError::SolverConvergenceFailed {
                    iterations,
                    residual: resid_norm,
                    last_x,
                    reason: format!(
                        "Levenberg-Marquardt terminated with {reason:?}: residual norm {resid_norm:.6e} exceeds tolerance {:.6e}",
                        self.tolerance
                    ),
                }
                .into());
            }
            Ok(LmSolution {
                params,
                stats: LmStats {
                    iterations,
                    residual_evals,
                    jacobian_evals,
                    termination_reason: reason,
                    final_residual_norm: resid_norm,
                    final_step_norm: last_step_norm,
                    lambda_final: lambda,
                },
            })
        };

        let raise = |lambda: f64| (lambda * LM_LAMBDA_FACTOR).min(LAMBDA_MAX);
        let mut jacobian_valid = false;

        for _iter in 0..self.max_iterations {
            if resid_norm < self.tolerance {
                let reason = LmTerminationReason::ConvergedResidualNorm;
                tracing::debug!(iterations, resid_norm, ?reason, "lm: converged");
                return finalize(
                    params,
                    reason,
                    iterations,
                    residual_evals,
                    jacobian_evals,
                    resid_norm,
                    last_step_norm,
                    lambda,
                );
            }

            if !jacobian_valid {
                jacobian_evals += 1;
                jacobian.fill(f64::NAN);
                jacobian_func(&params, &resid_vec, &mut residual_evals, &mut jacobian)?;
                if jacobian.iter().any(|value| !value.is_finite()) {
                    return Err(invalid_lm("LM Jacobian contains NaN or infinity"));
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
                    return Err(crate::Error::Validation(
                        "LM candidate parameter overflowed finite f64 range".to_string(),
                    ));
                }
                new_params[i] = candidate;
                step_diff[i] = candidate - params[i];
            }

            let effective_step_norm = scaled_norm(&step_diff);
            if !effective_step_norm.is_finite() {
                return Err(crate::Error::Validation(
                    "LM step norm overflowed finite f64 range".to_string(),
                ));
            }
            last_step_norm = effective_step_norm;
            if effective_step_norm < LM_MIN_STEP_SIZE {
                return finalize(
                    params,
                    LmTerminationReason::StepTooSmall,
                    iterations,
                    residual_evals,
                    jacobian_evals,
                    resid_norm,
                    last_step_norm,
                    lambda,
                );
            }

            new_resid.fill(f64::NAN);
            residuals_func(&new_params, &mut new_resid);
            residual_evals += 1;
            if new_resid.iter().any(|value| !value.is_finite()) {
                return Err(invalid_lm(
                    "LM trial residual vector contains NaN or infinity",
                ));
            }
            let new_norm = scaled_norm(&new_resid);
            if !new_norm.is_finite() {
                return Err(invalid_lm("LM trial residual norm overflowed finite f64"));
            }

            if new_norm < resid_norm {
                params.copy_from_slice(&new_params);
                resid_vec.copy_from_slice(&new_resid);
                resid_norm = new_norm;
                iterations += 1;
                jacobian_valid = false;

                if resid_norm < self.tolerance {
                    let reason = LmTerminationReason::ConvergedResidualNorm;
                    tracing::debug!(iterations, resid_norm, ?reason, "lm: converged");
                    return finalize(
                        params,
                        reason,
                        iterations,
                        residual_evals,
                        jacobian_evals,
                        resid_norm,
                        last_step_norm,
                        lambda,
                    );
                }

                lambda = (lambda / LM_LAMBDA_FACTOR).max(LAMBDA_MIN);
            } else if lambda >= LAMBDA_MAX {
                return finalize(
                    params,
                    LmTerminationReason::DampingLimit,
                    iterations,
                    residual_evals,
                    jacobian_evals,
                    resid_norm,
                    last_step_norm,
                    lambda,
                );
            } else {
                lambda = raise(lambda);
            }
        }

        tracing::warn!(
            algorithm = "levenberg_marquardt",
            iterations,
            max_iterations = self.max_iterations,
            final_residual_norm = resid_norm,
            final_step_norm = last_step_norm,
            lambda_final = lambda,
            n_params,
            n_residuals,
            category = "max_iterations_exceeded",
            "lm: bailout — max iterations reached without convergence"
        );
        finalize(
            params,
            LmTerminationReason::MaxIterations,
            iterations,
            residual_evals,
            jacobian_evals,
            resid_norm,
            last_step_norm,
            lambda,
        )
    }

    fn run_with_fd_jacobian<Res>(
        &self,
        residuals: Res,
        initial: &[f64],
        n_residuals: usize,
        require_residual_convergence: bool,
    ) -> Result<LmSolution>
    where
        Res: Fn(&[f64], &mut [f64]),
    {
        let n_params = initial.len();
        let mut jac_scratch = JacobianScratch::new(n_params, n_residuals);

        let jacobian_func =
            |p: &[f64], base_r: &[f64], eval_counter: &mut usize, out: &mut [f64]| {
                Self::compute_jacobian_system_into(
                    &residuals,
                    p,
                    base_r,
                    n_residuals,
                    eval_counter,
                    &mut jac_scratch,
                    out,
                )
            };

        self.solve_lm_core_with_stats(
            initial.to_vec(),
            &residuals,
            jacobian_func,
            n_residuals,
            require_residual_convergence,
        )
    }

    fn run_with_analytical_jacobian<Res, D>(
        &self,
        residuals: Res,
        derivatives: &D,
        initial: &[f64],
        require_residual_convergence: bool,
    ) -> Result<LmSolution>
    where
        Res: Fn(&[f64], &mut [f64]),
        D: AnalyticalDerivatives,
    {
        if initial.is_empty() {
            return Err(invalid_lm(
                "Levenberg-Marquardt initial parameters must not be empty",
            ));
        }
        let n_residuals = derivatives.residual_count();
        if n_residuals == 0 {
            return Err(invalid_lm(
                "Levenberg-Marquardt residual_count must be positive",
            ));
        }

        let n_params = initial.len();
        let mut jac_scratch = JacobianScratch::new(n_params, n_residuals);
        let mut jac_2d = vec![vec![f64::NAN; n_params]; n_residuals];

        let jacobian_func =
            |p: &[f64], base_r: &[f64], eval_counter: &mut usize, out: &mut [f64]| {
                for row in jac_2d.iter_mut() {
                    row.fill(f64::NAN);
                }
                if derivatives.jacobian(p, &mut jac_2d).is_some() {
                    for (i, row) in jac_2d.iter().enumerate() {
                        for (j, &v) in row.iter().enumerate() {
                            if !v.is_finite() {
                                return Err(invalid_lm(
                                    "LM analytical Jacobian contains NaN or infinity",
                                ));
                            }
                            out[i * p.len() + j] = v;
                        }
                    }
                    return Ok(());
                }
                Self::compute_jacobian_system_into(
                    &residuals,
                    p,
                    base_r,
                    n_residuals,
                    eval_counter,
                    &mut jac_scratch,
                    out,
                )
            };

        self.solve_lm_core_with_stats(
            initial.to_vec(),
            &residuals,
            jacobian_func,
            n_residuals,
            require_residual_convergence,
        )
    }

    /// Solve a nonlinear system of equations `r(x) = 0`, requiring the final
    /// residual 2-norm not to exceed `self.tolerance`.
    ///
    /// # Arguments
    ///
    /// * `residuals` - Callback populating `n_residuals` finite residual slots
    ///   for a candidate parameter slice of length `initial.len()`.
    /// * `initial` - Non-empty finite initial parameter vector.
    /// * `n_residuals` - Positive number of residual equations written by `residuals`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if solver controls, dimensions,
    /// initial parameters, residuals, or finite-difference Jacobian entries are
    /// invalid or non-finite, and [`crate::InputError::SolverConvergenceFailed`]
    /// if the solver terminates with `||r||_2 > self.tolerance`.
    pub fn solve_system<Res>(
        &self,
        residuals: Res,
        initial: &[f64],
        n_residuals: usize,
    ) -> Result<LmSolution>
    where
        Res: Fn(&[f64], &mut [f64]),
    {
        self.run_with_fd_jacobian(residuals, initial, n_residuals, true)
    }

    /// Solve a nonlinear system of equations `r(x) = 0` with an optional
    /// analytical Jacobian, requiring `||r||_2 <= self.tolerance`.
    ///
    /// # Arguments
    ///
    /// * `residuals` - Callback populating `derivatives.residual_count()` finite
    ///   residual slots for a candidate parameter slice.
    /// * `derivatives` - Analytical Jacobian provider (falls back to checked
    ///   central differences when `jacobian` returns `None`).
    /// * `initial` - Non-empty finite initial parameter vector.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if controls, dimensions, initial
    /// parameters, residuals, or Jacobian entries are invalid or non-finite,
    /// and [`crate::InputError::SolverConvergenceFailed`] if termination leaves
    /// `||r||_2 > self.tolerance`.
    pub fn solve_system_with_jacobian<Res, D>(
        &self,
        residuals: Res,
        derivatives: &D,
        initial: &[f64],
    ) -> Result<LmSolution>
    where
        Res: Fn(&[f64], &mut [f64]),
        D: AnalyticalDerivatives,
    {
        self.run_with_analytical_jacobian(residuals, derivatives, initial, true)
    }

    /// Minimize `||r(x)||_2` via damped least squares using finite-difference
    /// Jacobians, returning the terminal [`LmSolution`] and [`LmTerminationReason`]
    /// even when the nonzero-residual optimum exceeds `self.tolerance`.
    ///
    /// # Arguments
    ///
    /// * `residuals` - Callback populating `n_residuals` finite residual slots
    ///   for a candidate parameter slice of length `initial.len()`.
    /// * `initial` - Non-empty finite initial parameter vector.
    /// * `n_residuals` - Positive number of residual terms written by `residuals`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if solver controls, dimensions,
    /// initial parameters, residuals, or finite-difference Jacobian entries are
    /// invalid or non-finite.
    pub fn solve_least_squares<Res>(
        &self,
        residuals: Res,
        initial: &[f64],
        n_residuals: usize,
    ) -> Result<LmSolution>
    where
        Res: Fn(&[f64], &mut [f64]),
    {
        self.run_with_fd_jacobian(residuals, initial, n_residuals, false)
    }

    /// Minimize `||r(x)||_2` via damped least squares with an analytical
    /// Jacobian, returning the terminal [`LmSolution`] and [`LmTerminationReason`]
    /// even when the nonzero-residual optimum exceeds `self.tolerance`.
    ///
    /// # Arguments
    ///
    /// * `residuals` - Callback populating `derivatives.residual_count()` finite
    ///   residual slots for a candidate parameter slice.
    /// * `derivatives` - Analytical Jacobian provider (falls back to checked
    ///   central differences when `jacobian` returns `None`).
    /// * `initial` - Non-empty finite initial parameter vector.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if solver controls, dimensions,
    /// initial parameters, residuals, or Jacobian entries are invalid or non-finite.
    pub fn solve_least_squares_with_jacobian<Res, D>(
        &self,
        residuals: Res,
        derivatives: &D,
        initial: &[f64],
    ) -> Result<LmSolution>
    where
        Res: Fn(&[f64], &mut [f64]),
        D: AnalyticalDerivatives,
    {
        self.run_with_analytical_jacobian(residuals, derivatives, initial, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lm_solution_is_invariant_to_uniform_residual_scale() {
        for scale in [1e-12, 1.0, 1e12] {
            let solution = LevenbergMarquardtSolver::new()
                .with_tolerance(scale * 1e-10)
                .solve_system(
                    |p, residuals| residuals[0] = scale * (p[0] - 2.0),
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
        let solver = LevenbergMarquardtSolver::new().with_tolerance(1e-12);

        let residuals = |params: &[f64], resid: &mut [f64]| {
            resid[0] = params[0] + params[1] - 5.0;
            resid[1] = params[0] - params[1] - 1.0;
        };

        let initial = vec![0.0, 0.0];
        let solution = solver
            .solve_system(residuals, &initial, 2)
            .expect("stats solve should succeed");

        assert_eq!(
            solution.stats.termination_reason,
            LmTerminationReason::ConvergedResidualNorm
        );
        assert!(solution.stats.residual_evals >= 1);
        assert!(solution.stats.jacobian_evals >= 1);
    }

    #[test]
    fn test_lm_stats_reports_max_iterations() {
        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(1e-12)
            .with_max_iterations(1);

        let residuals = |params: &[f64], resid: &mut [f64]| {
            resid[0] = params[0] * params[0] - 2.0;
        };

        let initial = vec![10.0];
        let err = solver
            .solve_system(residuals, &initial, 1)
            .expect_err("solve_system must fail closed when max_iterations leaves residual > tol");
        assert!(matches!(
            err,
            crate::Error::Input(crate::InputError::SolverConvergenceFailed { iterations: 1, .. })
        ));

        let solution = solver
            .solve_least_squares(residuals, &initial, 1)
            .expect("solve_least_squares returns diagnostics at max_iterations");
        assert_eq!(
            solution.stats.termination_reason,
            LmTerminationReason::MaxIterations
        );
        assert_eq!(solution.stats.iterations, 1);

        let zero_iter_err = LevenbergMarquardtSolver::new()
            .with_max_iterations(0)
            .solve_system(residuals, &initial, 1)
            .expect_err("max_iterations == 0 must be rejected");
        assert!(matches!(zero_iter_err, crate::Error::Validation(_)));
    }

    #[test]
    fn test_jacobian_system() {
        // System: x^2 + y^2 = 4, x - y = 0
        struct CircleLineJacobian;

        impl AnalyticalDerivatives for CircleLineJacobian {
            fn jacobian(&self, params: &[f64], jacobian: &mut [Vec<f64>]) -> Option<()> {
                let x = params[0];
                let y = params[1];

                jacobian[0][0] = 2.0 * x; // df1/dx
                jacobian[0][1] = 2.0 * y; // df1/dy
                jacobian[1][0] = 1.0; // df2/dx
                jacobian[1][1] = -1.0; // df2/dy

                Some(())
            }

            fn residual_count(&self) -> usize {
                2
            }
        }

        let solver = LevenbergMarquardtSolver::new().with_tolerance(1e-10);

        let residuals = |params: &[f64], resid: &mut [f64]| {
            resid[0] = params[0] * params[0] + params[1] * params[1] - 4.0;
            resid[1] = params[0] - params[1];
        };

        let derivatives = CircleLineJacobian;
        let initial = vec![1.0, 0.0];

        let result = solver
            .solve_system_with_jacobian(residuals, &derivatives, &initial)
            .expect("Minimization should succeed in test")
            .params;

        let expected = 2.0_f64.sqrt();
        assert!((result[0] - expected).abs() < 1e-8);
        assert!((result[1] - expected).abs() < 1e-8);
    }

    #[test]
    fn test_solve_system_with_dim_overdetermined() {
        // Overdetermined system: 5 equations, 2 parameters
        // The least-squares solution for x + y = 5, x - y = 1 should still be (3, 2)
        // even with redundant/noisy constraints
        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(1e-8)
            .with_max_iterations(200);

        let residuals = |params: &[f64], resid: &mut [f64]| {
            let x = params[0];
            let y = params[1];
            resid[0] = x + y - 5.0; // x + y = 5
            resid[1] = x - y - 1.0; // x - y = 1
            resid[2] = 2.0 * x - 6.0; // 2x = 6  (consistent: x=3)
            resid[3] = 2.0 * y - 4.0; // 2y = 4  (consistent: y=2)
            resid[4] = x + 2.0 * y - 7.0; // x + 2y = 7 (consistent)
        };

        let initial = vec![0.0, 0.0];
        let result = solver
            .solve_system(residuals, &initial, 5)
            .expect("solve_system should succeed")
            .params;

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
    fn test_solve_system_with_dim_highly_overdetermined() {
        // Highly overdetermined: 20 residuals, 2 parameters
        // This tests that we don't panic even with n_residuals >> n_params
        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(1e-6)
            .with_max_iterations(200);

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
        };

        let initial = vec![0.0, 0.0];
        let result = solver
            .solve_system(residuals, &initial, 20)
            .expect("solve_system should not panic with 20 residuals")
            .params;

        // Solution should be approximately x=3, y=2
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
    fn test_solve_system_with_dim_returns_correct_length() {
        let solver = LevenbergMarquardtSolver::new().with_tolerance(1e-8);

        let residuals = |params: &[f64], resid: &mut [f64]| {
            resid[0] = params[0] - 1.0;
            resid[1] = params[1] - 2.0;
            resid[2] = params[2] - 3.0;
        };

        let initial = vec![0.0, 0.0, 0.0];
        let result = solver
            .solve_system(residuals, &initial, 3)
            .expect("solve should succeed")
            .params;

        assert_eq!(result.len(), 3, "Result should have same length as initial");
    }

    #[test]
    fn test_jacobian_system_uses_explicit_residual_count() {
        struct TallJacobian;

        impl AnalyticalDerivatives for TallJacobian {
            fn jacobian(&self, _params: &[f64], jacobian: &mut [Vec<f64>]) -> Option<()> {
                for row in jacobian.iter_mut() {
                    row[0] = 1.0;
                }
                Some(())
            }

            fn residual_count(&self) -> usize {
                5
            }
        }

        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(1e-8)
            .with_max_iterations(50);
        let residuals = |params: &[f64], resid: &mut [f64]| {
            for (i, out) in resid.iter_mut().enumerate() {
                *out = params[0] - (i as f64 + 1.0);
            }
        };
        let initial = vec![0.0];

        let solution = solver
            .solve_least_squares_with_jacobian(residuals, &TallJacobian, &initial)
            .expect("explicit residual count should avoid probe truncation");

        assert_eq!(solution.params.len(), 1);
        assert!((solution.params[0] - 3.0).abs() < 1e-8);
        assert!(
            solver
                .solve_system_with_jacobian(residuals, &TallJacobian, &initial)
                .is_err(),
            "inconsistent system must fail solve_system_with_jacobian when residual > tol"
        );
    }

    #[test]
    fn test_lm_rejects_nan_and_inf_residuals_and_jacobian() {
        let solver = LevenbergMarquardtSolver::new().with_tolerance(1e-8);
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let err = solver
                .solve_system(|_p, r| r[0] = bad, &[0.0], 1)
                .expect_err("non-finite initial residual must fail");
            assert!(matches!(err, crate::Error::Validation(_)));

            let err_probe = solver
                .solve_system(|p, r| r[0] = if p[0] == 0.0 { 1.0 } else { bad }, &[0.0], 1)
                .expect_err("non-finite Jacobian probe must fail");
            assert!(matches!(err_probe, crate::Error::Validation(_)));
        }

        struct BadJacobian;
        impl AnalyticalDerivatives for BadJacobian {
            fn jacobian(&self, _params: &[f64], jacobian: &mut [Vec<f64>]) -> Option<()> {
                jacobian[0][0] = f64::NAN;
                Some(())
            }
            fn residual_count(&self) -> usize {
                1
            }
        }
        let err_jac = solver
            .solve_system_with_jacobian(|p, r| r[0] = p[0] - 1.0, &BadJacobian, &[0.0])
            .expect_err("NaN analytical Jacobian must fail");
        assert!(matches!(err_jac, crate::Error::Validation(_)));
    }

    #[test]
    fn test_lm_qr_solves_ill_conditioned_and_asymmetric_column_system() {
        // J = [[1, 1], [0, 1e-8]]: κ(J) ≈ 2e8 so JᵀJ is singular in f64 (κ(J)² ≈ 4e16),
        // whereas augmented Householder ColPivQR solves it cleanly.
        let eps = 1e-8_f64;
        let residuals = |params: &[f64], resid: &mut [f64]| {
            let x = params[0];
            let y = params[1];
            resid[0] = (x - 1.0) + 100.0 * (y - 2.0);
            resid[1] = eps * (x - 1.0);
        };
        let solution = LevenbergMarquardtSolver::new()
            .with_tolerance(1e-14)
            .with_max_iterations(100)
            .solve_system(residuals, &[0.0, 0.0], 2)
            .expect("augmented QR must solve ill-conditioned asymmetric system");
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
        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(1e-2)
            .with_max_iterations(1);
        let residuals = |params: &[f64], resid: &mut [f64]| {
            resid[0] = params[0] - 2.0;
        };
        let solution = solver
            .solve_system(residuals, &[0.0], 1)
            .expect("one damped linear step should reach 1e-2 tolerance");
        assert_eq!(
            solution.stats.termination_reason,
            LmTerminationReason::ConvergedResidualNorm
        );
        assert!((solution.params[0] - 2.0).abs() < 1e-2);
    }
}

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
//! use finstack_core::math::solver_multi::LevenbergMarquardtSolver;
//!
//! let solver = LevenbergMarquardtSolver::new().with_tolerance(1e-8);
//!
//! // Minimize sum of squares: (x-2)^2 + (y-3)^2
//! let objective = |params: &[f64]| -> f64 {
//!     (params[0] - 2.0).powi(2) + (params[1] - 3.0).powi(2)
//! };
//!
//! let initial = vec![0.0, 0.0];
//! let result = solver.minimize(objective, &initial, None).expect("Minimization should succeed");
//! assert!((result[0] - 2.0).abs() < 1e-6);
//! assert!((result[1] - 3.0).abs() < 1e-6);
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
//!   - Hagan, P. S., et al. (2002). "Managing Smile Risk." *Wilmott Magazine*, September 2002.
//!     (SABR calibration techniques)

use crate::error::InputError;
use crate::Result;

/// Pre-allocated scratch buffers for finite-difference Jacobian computation.
struct JacobianScratch {
    jacobian: Vec<f64>,
    params_plus: Vec<f64>,
    params_minus: Vec<f64>,
    r_plus: Vec<f64>,
    r_minus: Vec<f64>,
}

impl JacobianScratch {
    fn new(n_params: usize, n_residuals: usize) -> Self {
        Self {
            jacobian: vec![0.0; n_residuals * n_params],
            params_plus: vec![0.0; n_params],
            params_minus: vec![0.0; n_params],
            r_plus: vec![0.0; n_residuals],
            r_minus: vec![0.0; n_residuals],
        }
    }
}

/// Pre-allocated workspace for the normal-equations step of LM.
struct NormalEquationsWorkspace {
    matrix: Vec<f64>,
    rhs: Vec<f64>,
    chol: Vec<f64>,
    step: Vec<f64>,
}

impl NormalEquationsWorkspace {
    fn new(n_params: usize) -> Self {
        Self {
            matrix: vec![0.0; n_params * n_params],
            rhs: vec![0.0; n_params],
            chol: vec![0.0; n_params * n_params],
            step: vec![0.0; n_params],
        }
    }
}

/// Trait for functions that can provide analytical derivatives.
///
/// This trait allows optimization algorithms to use exact derivatives
/// when available, significantly improving convergence speed and accuracy.
pub trait AnalyticalDerivatives {
    /// Compute the gradient of the objective function.
    ///
    /// # Arguments
    /// * `params` - Current parameter values
    /// * `gradient` - Output buffer for gradient (must be same length as params)
    fn gradient(&self, params: &[f64], gradient: &mut [f64]);

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

    /// Returns true if analytical gradient is available.
    fn has_gradient(&self) -> bool {
        true
    }

    /// Returns true if analytical Jacobian is available.
    fn has_jacobian(&self) -> bool {
        false
    }

    /// Number of residual equations for Jacobian-based system solves, if known.
    fn residual_count(&self) -> Option<usize> {
        None
    }
}

/// Termination reason for the Levenberg-Marquardt solver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LmTerminationReason {
    /// Residual norm fell below the configured tolerance.
    ConvergedResidualNorm,
    /// Relative residual reduction fell below the configured tolerance.
    ConvergedRelativeReduction,
    /// Gradient norm fell below the configured tolerance.
    ConvergedGradient,
    /// Parameter update step became smaller than `min_step_size`.
    StepTooSmall,
    /// Solver exhausted the iteration budget.
    MaxIterations,
    /// Solver encountered an unrecoverable numerical failure.
    NumericalFailure,
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
    /// Number of times λ hit the upper or lower bound.
    ///
    /// High values may indicate an ill-conditioned problem, poor initial guess,
    /// or a problem that's near singular. Consider:
    /// - Improving the initial guess
    /// - Scaling the problem differently
    /// - Using analytical derivatives if available
    pub lambda_bound_hits: usize,
}

/// Solution vector plus solver statistics.
#[derive(Clone, Debug)]
pub struct LmSolution {
    /// Solved parameter vector.
    pub params: Vec<f64>,
    /// Detailed solver diagnostics.
    pub stats: LmStats,
}

/// Minimum bound for damping parameter λ.
///
/// Lambda values below this threshold are clamped to prevent numerical instability
/// from overly aggressive Gauss-Newton steps.
pub const LAMBDA_MIN: f64 = 1e-15;

/// Maximum bound for damping parameter λ.
///
/// Lambda values above this threshold are clamped to prevent the solver from
/// becoming effectively stuck in pure gradient descent mode.
pub const LAMBDA_MAX: f64 = 1e15;

/// Number of consecutive iterations at λ bounds before warning.
///
/// If λ hits its bounds for this many consecutive iterations, it may indicate
/// an ill-conditioned problem or poor initial guess.
pub const LAMBDA_BOUND_WARNING_THRESHOLD: usize = 5;

/// Relative floor applied to the `JᵀJ` diagonal in Marquardt damping.
///
/// The damping term is `λ · max(JᵀJ_ii, floor)` with
/// `floor = MARQUARDT_DIAG_FLOOR_REL × max_i(JᵀJ_ii)`, so a parameter with
/// (numerically) zero curvature still receives a small positive damping and
/// cannot make the damped normal-equations matrix singular or produce an
/// unbounded step.
pub const MARQUARDT_DIAG_FLOOR_REL: f64 = 1e-12;

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
/// relative floor, see [`MARQUARDT_DIAG_FLOOR_REL`]) rather than the unscaled
/// Levenberg `+λI`, so steps are invariant to per-parameter rescaling.
///
/// - **λ → 0**: Gauss-Newton (fast near solution)
/// - **λ → ∞**: Gradient descent (robust far from solution)
///
/// # Damping Parameter Bounds
///
/// The damping parameter λ is bounded to [`LAMBDA_MIN`] and [`LAMBDA_MAX`] to ensure
/// numerical stability. If λ hits these bounds for [`LAMBDA_BOUND_WARNING_THRESHOLD`]
/// consecutive iterations, the solver records this in [`LmStats::lambda_bound_hits`]
/// which may indicate an ill-conditioned problem.
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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LevenbergMarquardtSolver {
    /// Convergence tolerance for gradient norm
    pub tolerance: f64,
    /// Maximum iterations
    pub max_iterations: usize,
    /// Initial damping parameter (lambda)
    pub lambda_init: f64,
    /// Factor for adjusting lambda (increase on failure, decrease on success)
    pub lambda_factor: f64,
    /// Base finite difference step for the central-difference Jacobian.
    ///
    /// The per-component step is `h_j = fd_step × max(1, |x_j|)`. The default
    /// is `ε^(1/3) ≈ 6.06e-6`, the optimal step for central differences
    /// (truncation error O(h²) balanced against rounding error O(ε/h);
    /// Press et al. (2007), *Numerical Recipes*, Section 5.7).
    pub fd_step: f64,
    /// Minimum allowed step size (for numerical stability)
    pub min_step_size: f64,
}

impl Default for LevenbergMarquardtSolver {
    fn default() -> Self {
        Self {
            tolerance: 1e-8,
            max_iterations: 100,
            lambda_init: 1e-3,
            lambda_factor: 10.0,
            // ε^(1/3): optimal base step for central differences (the 1e-8
            // forward-difference step loses ~half the achievable accuracy).
            fd_step: f64::EPSILON.cbrt(),
            min_step_size: 1e-12,
        }
    }
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

    /// Set initial damping parameter.
    pub fn with_lambda_init(mut self, lambda: f64) -> Self {
        self.lambda_init = lambda;
        self
    }

    /// Set finite difference step size.
    pub fn with_fd_step(mut self, step: f64) -> Self {
        self.fd_step = step;
        self
    }

    /// Minimize objective function starting from an initial guess.
    ///
    /// # Scalar-objective formulation (important)
    ///
    /// This routine wraps the scalar objective as a single LM "residual", so the
    /// update step targets `f(x) = 0`, **not** `∇f(x) = 0`. It is only suitable
    /// for objectives whose minimum value is approximately zero — e.g.
    /// least-squares residual norms / sums of squared errors. Objectives with
    /// strictly positive minima may stall near the optimum, and objectives that
    /// can go negative may have improving steps rejected. See
    /// `docs/reviews/2026-06-09-core-quant-review.md` ("Scalar-objective LM is
    /// a root-finder for f(x)=0").
    ///
    /// # Arguments
    /// * `objective` - Function to minimize, takes a parameter vector and returns a scalar.
    /// * `initial` - Initial parameter guess.
    /// * `bounds` - Optional box constraints for each parameter.
    ///
    /// # Returns
    /// Optimal parameter vector that minimizes the objective.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::SolverConvergenceFailed`] when the solver
    /// encounters a numerical failure, or terminates (iteration budget
    /// exhausted, or step below `min_step_size`) at a point that satisfies
    /// neither the residual nor the (bounds-projected) gradient tolerance.
    /// Termination at a genuinely converged point — objective within
    /// `tolerance` of zero, or projected gradient below `tolerance` (e.g. a
    /// box-constrained minimum on the boundary) — still returns `Ok`.
    pub fn minimize<Obj>(
        &self,
        objective: Obj,
        initial: &[f64],
        bounds: Option<&[(f64, f64)]>,
    ) -> Result<Vec<f64>>
    where
        Obj: Fn(&[f64]) -> f64,
    {
        let residuals_func = |params: &[f64], resid: &mut [f64]| {
            resid[0] = objective(params);
        };

        let jacobian_func = |p: &[f64], _r: &[f64], _eval_counter: &mut usize, out: &mut [f64]| {
            let jac = self.compute_jacobian(&objective, p);
            out.copy_from_slice(&jac);
        };

        // Convergence check: Gradient Norm (jac is flat 1×n = gradient)
        let convergence_check =
            |_p: &[f64], _r: &[f64], jac: &[f64]| -> Option<LmTerminationReason> {
                let grad_norm: f64 = jac.iter().map(|g| g * g).sum::<f64>().sqrt();
                if grad_norm < self.tolerance {
                    Some(LmTerminationReason::ConvergedGradient)
                } else {
                    None
                }
            };

        let solution = self.solve_lm_core_with_stats(
            initial.to_vec(),
            &residuals_func,
            jacobian_func,
            convergence_check,
            1, // n_residuals
            bounds,
        )?;
        self.scalar_solution_to_result(solution, |p| self.compute_jacobian(&objective, p), bounds)
    }

    /// Map a scalar-objective [`LmSolution`] to either the parameter vector or
    /// a structured convergence error.
    ///
    /// `NumericalFailure` always becomes
    /// [`InputError::SolverConvergenceFailed`]. `MaxIterations` and
    /// `StepTooSmall` are accepted only when the iterate is converged in
    /// substance: the final residual (objective value) is within tolerance, or
    /// the bounds-projected gradient norm is within tolerance (a genuine
    /// stationary point, possibly on a box-constraint boundary). A budget
    /// exhaustion or pure stall away from any such point is an error — the
    /// previous behaviour of silently returning the best iterate is gone
    /// (review 2026-06-09, core math).
    fn scalar_solution_to_result<G>(
        &self,
        solution: LmSolution,
        gradient_at: G,
        bounds: Option<&[(f64, f64)]>,
    ) -> Result<Vec<f64>>
    where
        G: FnOnce(&[f64]) -> Vec<f64>,
    {
        use LmTerminationReason as Reason;
        let converged = match solution.stats.termination_reason {
            Reason::ConvergedResidualNorm
            | Reason::ConvergedRelativeReduction
            | Reason::ConvergedGradient => true,
            Reason::NumericalFailure => false,
            Reason::MaxIterations | Reason::StepTooSmall => {
                solution.stats.final_residual_norm.abs() <= self.tolerance || {
                    let gradient = gradient_at(&solution.params);
                    projected_gradient_norm(&gradient, &solution.params, bounds) < self.tolerance
                }
            }
        };
        if converged {
            Ok(solution.params)
        } else {
            Err(InputError::SolverConvergenceFailed {
                iterations: solution.stats.iterations,
                residual: solution.stats.final_residual_norm,
                last_x: solution.stats.final_step_norm,
                reason: format!(
                    "Levenberg-Marquardt minimize terminated with {:?} before reaching \
                     tolerance {:.1e} (last_x reports the final step norm)",
                    solution.stats.termination_reason, self.tolerance
                ),
            }
            .into())
        }
    }

    /// Compute Jacobian matrix using finite differences.
    ///
    /// Returns a flat `Vec<f64>` in row-major layout: `jacobian[i * n_params + j]`
    /// where i = residual index, j = param index. For scalar objective, Jacobian is 1×n.
    fn compute_jacobian<Obj>(&self, objective: &Obj, params: &[f64]) -> Vec<f64>
    where
        Obj: Fn(&[f64]) -> f64,
    {
        let n = params.len();
        let mut jacobian = vec![0.0; n]; // For scalar objective, Jacobian is 1×n

        let mut params_plus = params.to_vec();
        let mut params_minus = params.to_vec();

        for j in 0..n {
            let h = (params[j].abs() * self.fd_step).max(self.fd_step);
            params_plus[j] = params[j] + h;
            params_minus[j] = params[j] - h;
            let f_plus = objective(&params_plus);
            let f_minus = objective(&params_minus);
            jacobian[j] = (f_plus - f_minus) / (2.0 * h);
            params_plus[j] = params[j];
            params_minus[j] = params[j];
        }

        jacobian
    }

    /// Compute gradient using analytical derivatives if available, otherwise finite differences.
    fn compute_gradient_with_analytical<Obj, D>(
        &self,
        objective: &Obj,
        params: &[f64],
        derivatives: Option<&D>,
    ) -> Vec<f64>
    where
        Obj: Fn(&[f64]) -> f64,
        D: AnalyticalDerivatives,
    {
        if let Some(deriv) = derivatives {
            if deriv.has_gradient() {
                let mut gradient = vec![0.0; params.len()];
                deriv.gradient(params, &mut gradient);
                return gradient;
            }
        }

        // Fall back to finite differences (Jacobian is 1×n, flat row)
        self.compute_jacobian(objective, params)
    }

    /// Compute Jacobian for a system of residuals into a pre-allocated buffer.
    ///
    /// Writes a flat row-major Jacobian into `scratch.jacobian`: `J[i * n_params + j]`.
    fn compute_jacobian_system_into<Res>(
        &self,
        residuals: &Res,
        params: &[f64],
        n_residuals: usize,
        residual_eval_counter: &mut usize,
        scratch: &mut JacobianScratch,
    ) where
        Res: Fn(&[f64], &mut [f64]),
    {
        let n_params = params.len();

        scratch.params_plus.copy_from_slice(params);
        scratch.params_minus.copy_from_slice(params);

        for (j, &p_j) in params.iter().enumerate() {
            let h = (p_j.abs() * self.fd_step).max(self.fd_step);
            scratch.params_plus[j] = p_j + h;
            scratch.params_minus[j] = p_j - h;
            residuals(&scratch.params_plus, &mut scratch.r_plus);
            residuals(&scratch.params_minus, &mut scratch.r_minus);
            *residual_eval_counter += 2;

            for i in 0..n_residuals {
                scratch.jacobian[i * n_params + j] =
                    (scratch.r_plus[i] - scratch.r_minus[i]) / (2.0 * h);
            }
            scratch.params_plus[j] = p_j;
            scratch.params_minus[j] = p_j;
        }
    }

    /// Solve the damped normal equations `(JᵀJ + λ·diag(JᵀJ)) δ = -Jᵀr` into
    /// pre-allocated workspace.
    ///
    /// Uses Marquardt's diagonal scaling (Marquardt 1963) rather than the
    /// unscaled Levenberg `+λI`, making the step invariant to per-parameter
    /// rescaling. Diagonal entries are floored at
    /// [`MARQUARDT_DIAG_FLOOR_REL`]` × max_i(JᵀJ_ii)` so zero-curvature
    /// components cannot stall the factorisation or produce unbounded steps.
    fn solve_normal_equations_into(
        &self,
        jacobian: &[f64],
        m: usize,
        n: usize,
        residuals: &[f64],
        lambda: f64,
        ws: &mut NormalEquationsWorkspace,
    ) -> Result<()> {
        use super::linalg::{cholesky_decomposition_into, cholesky_solve};

        let min_dim = n.min(m);

        if min_dim == 0 {
            ws.step.fill(0.0);
            return Ok(());
        }

        ws.matrix.fill(0.0);

        for row in 0..m {
            for i in 0..n {
                let ri = jacobian[row * n + i];
                for j in 0..=i {
                    ws.matrix[i * n + j] += ri * jacobian[row * n + j];
                }
            }
        }

        // Damping term λ·max(JᵀJ_ii, floor).
        //
        // For a genuine least-squares system (m ≥ n) the floor is a tiny
        // fraction of the largest diagonal entry, giving Marquardt's
        // scale-invariant λ·diag(JᵀJ) damping (Marquardt 1963).
        //
        // For rank-deficient formulations (m < n, e.g. the scalar-objective
        // wrapper where JᵀJ = ggᵀ is rank 1) true Marquardt damping is
        // pathological: in the large-λ limit the step components scale as
        // -f/(λ·g_i), *diverging* exactly where the gradient is small, and
        // the iteration zigzags to a stall. There the floor is raised to
        // max_diag itself, i.e. scale-invariant Levenberg damping
        // λ·max_diag·I (still invariant under uniform rescaling of the
        // objective, unlike the historical unscaled +λI).
        let max_diag = (0..n).map(|i| ws.matrix[i * n + i]).fold(0.0_f64, f64::max);
        let diag_floor = if max_diag <= 0.0 {
            // Degenerate all-zero Jacobian: fall back to unit Levenberg +λI.
            1.0
        } else if m < n {
            max_diag
        } else {
            MARQUARDT_DIAG_FLOOR_REL * max_diag
        };
        for i in 0..n {
            let d = ws.matrix[i * n + i];
            ws.matrix[i * n + i] = d + lambda * d.max(diag_floor);
            for j in 0..i {
                ws.matrix[j * n + i] = ws.matrix[i * n + j];
            }
        }

        ws.rhs.fill(0.0);
        for k in 0..m {
            let r = residuals[k];
            for i in 0..n {
                ws.rhs[i] -= jacobian[k * n + i] * r;
            }
        }

        cholesky_decomposition_into(&ws.matrix, n, &mut ws.chol)
            .map_err(|_| crate::error::InputError::Invalid)?;

        ws.step.fill(0.0);
        cholesky_solve(&ws.chol, &ws.rhs, &mut ws.step)
            .map_err(|_| crate::error::InputError::Invalid)?;

        Ok(())
    }

    /// Apply box constraints to parameters.
    fn apply_bounds(&self, params: &mut [f64], bounds: Option<&[(f64, f64)]>) {
        if let Some(bounds) = bounds {
            for (i, (lo, hi)) in bounds.iter().enumerate().take(params.len()) {
                params[i] = params[i].clamp(*lo, *hi);
            }
        }
    }

    fn solve_lm_core_with_stats<Res, Jac, Check>(
        &self,
        mut params: Vec<f64>,
        residuals_func: &Res,
        mut jacobian_func: Jac,
        convergence_check: Check,
        n_residuals: usize,
        bounds: Option<&[(f64, f64)]>,
    ) -> Result<LmSolution>
    where
        Res: Fn(&[f64], &mut [f64]),
        Jac: FnMut(&[f64], &[f64], &mut usize, &mut [f64]),
        Check: Fn(&[f64], &[f64], &[f64]) -> Option<LmTerminationReason>,
    {
        if params.is_empty() || n_residuals == 0 {
            return Err(InputError::Invalid.into());
        }

        let mut lambda = self.lambda_init;
        let n_params = params.len();

        let mut resid_vec = vec![0.0; n_residuals];
        let mut new_resid = vec![0.0; n_residuals];
        let mut new_params = vec![0.0; n_params];
        let mut jacobian = vec![0.0; n_residuals * n_params];
        let mut ne_ws = NormalEquationsWorkspace::new(n_params);

        residuals_func(&params, &mut resid_vec);
        let mut resid_norm: f64 = resid_vec.iter().map(|r| r * r).sum::<f64>().sqrt();
        let mut residual_evals = 1usize;
        let mut jacobian_evals = 0usize;
        let mut iterations = 0usize;
        let mut last_step_norm = 0.0_f64;
        let mut lambda_bound_hits = 0usize;

        tracing::debug!(
            n_params,
            n_residuals,
            initial_resid_norm = resid_norm,
            max_iter = self.max_iterations,
            "lm: start"
        );

        for _iter in 0..self.max_iterations {
            jacobian_evals += 1;
            jacobian_func(&params, &resid_vec, &mut residual_evals, &mut jacobian);

            if let Some(reason) = convergence_check(&params, &resid_vec, &jacobian[..]) {
                tracing::debug!(iterations, resid_norm, ?reason, "lm: converged");
                return Ok(LmSolution {
                    params,
                    stats: LmStats {
                        iterations,
                        residual_evals,
                        jacobian_evals,
                        termination_reason: reason,
                        final_residual_norm: resid_norm,
                        final_step_norm: last_step_norm,
                        lambda_final: lambda,
                        lambda_bound_hits,
                    },
                });
            }

            match self.solve_normal_equations_into(
                &jacobian,
                n_residuals,
                n_params,
                &resid_vec,
                lambda,
                &mut ne_ws,
            ) {
                Ok(()) => {}
                Err(_) => {
                    let new_lambda = (lambda * self.lambda_factor).min(LAMBDA_MAX);
                    if new_lambda >= LAMBDA_MAX {
                        lambda_bound_hits += 1;
                    }
                    lambda = new_lambda;
                    continue;
                }
            };

            new_params.copy_from_slice(&params);
            for (i, &s) in ne_ws.step.iter().enumerate() {
                new_params[i] += s;
            }
            self.apply_bounds(&mut new_params, bounds);

            let effective_step_norm: f64 = new_params
                .iter()
                .zip(params.iter())
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                .sqrt();
            last_step_norm = effective_step_norm;
            if effective_step_norm < self.min_step_size {
                return Ok(LmSolution {
                    params,
                    stats: LmStats {
                        iterations,
                        residual_evals,
                        jacobian_evals,
                        termination_reason: LmTerminationReason::StepTooSmall,
                        final_residual_norm: resid_norm,
                        final_step_norm: last_step_norm,
                        lambda_final: lambda,
                        lambda_bound_hits,
                    },
                });
            }

            residuals_func(&new_params, &mut new_resid);
            residual_evals += 1;
            let new_norm: f64 = new_resid.iter().map(|r| r * r).sum::<f64>().sqrt();

            if new_norm < resid_norm {
                params.copy_from_slice(&new_params);
                resid_vec.copy_from_slice(&new_resid);
                resid_norm = new_norm;
                iterations += 1;

                let new_lambda = (lambda / self.lambda_factor).max(LAMBDA_MIN);
                if new_lambda <= LAMBDA_MIN {
                    lambda_bound_hits += 1;
                }
                lambda = new_lambda;
            } else {
                let new_lambda = (lambda * self.lambda_factor).min(LAMBDA_MAX);
                if new_lambda >= LAMBDA_MAX {
                    lambda_bound_hits += 1;
                }
                lambda = new_lambda;
            }
        }

        tracing::warn!(
            algorithm = "levenberg_marquardt",
            iterations,
            max_iterations = self.max_iterations,
            final_residual_norm = resid_norm,
            final_step_norm = last_step_norm,
            lambda_final = lambda,
            lambda_bound_hits,
            n_params,
            n_residuals,
            category = "max_iterations_exceeded",
            "lm: bailout — max iterations reached without convergence"
        );
        Ok(LmSolution {
            params,
            stats: LmStats {
                iterations,
                residual_evals,
                jacobian_evals,
                termination_reason: LmTerminationReason::MaxIterations,
                final_residual_norm: resid_norm,
                final_step_norm: last_step_norm,
                lambda_final: lambda,
                lambda_bound_hits,
            },
        })
    }

    /// Minimize objective function with analytical derivatives.
    ///
    /// The same scalar-objective formulation caveat and error semantics as
    /// [`Self::minimize`] apply: the step targets `f(x) = 0`, and
    /// non-convergent terminations return
    /// [`InputError::SolverConvergenceFailed`] instead of silently handing back
    /// the best iterate.
    ///
    /// # Arguments
    /// * `objective` - Function to minimize
    /// * `derivatives` - Provider of analytical derivatives
    /// * `initial` - Initial parameter guess
    /// * `bounds` - Optional box constraints
    ///
    /// # Returns
    /// Optimal parameter vector
    ///
    /// # Errors
    ///
    /// Returns [`InputError::SolverConvergenceFailed`] on `MaxIterations`,
    /// `NumericalFailure`, or a stall at a non-converged point (see
    /// [`Self::minimize`]).
    pub fn minimize_with_derivatives<Obj, D>(
        &self,
        objective: Obj,
        derivatives: &D,
        initial: &[f64],
        bounds: Option<&[(f64, f64)]>,
    ) -> Result<Vec<f64>>
    where
        Obj: Fn(&[f64]) -> f64,
        D: AnalyticalDerivatives,
    {
        let residuals_func = |params: &[f64], resid: &mut [f64]| {
            resid[0] = objective(params);
        };

        let jacobian_func = |p: &[f64], _r: &[f64], _eval_counter: &mut usize, out: &mut [f64]| {
            let grad = self.compute_gradient_with_analytical(&objective, p, Some(derivatives));
            out.copy_from_slice(&grad);
        };

        // Convergence check: Gradient Norm (jac is flat 1×n = gradient)
        let convergence_check =
            |_p: &[f64], _r: &[f64], jac: &[f64]| -> Option<LmTerminationReason> {
                let grad_norm: f64 = jac.iter().map(|g| g * g).sum::<f64>().sqrt();
                if grad_norm < self.tolerance {
                    Some(LmTerminationReason::ConvergedGradient)
                } else {
                    None
                }
            };

        let solution = self.solve_lm_core_with_stats(
            initial.to_vec(),
            &residuals_func,
            jacobian_func,
            convergence_check,
            1, // n_residuals
            bounds,
        )?;
        self.scalar_solution_to_result(
            solution,
            |p| self.compute_gradient_with_analytical(&objective, p, Some(derivatives)),
            bounds,
        )
    }

    /// Solve system of equations with explicit residual dimension and stats.
    pub fn solve_system_with_dim_stats<Res>(
        &self,
        residuals: Res,
        initial: &[f64],
        n_residuals: usize,
    ) -> Result<LmSolution>
    where
        Res: Fn(&[f64], &mut [f64]),
    {
        let n_params = initial.len();
        let mut jac_scratch = JacobianScratch::new(n_params, n_residuals);

        let jacobian_func = |p: &[f64], _r: &[f64], eval_counter: &mut usize, out: &mut [f64]| {
            self.compute_jacobian_system_into(
                &residuals,
                p,
                n_residuals,
                eval_counter,
                &mut jac_scratch,
            );
            out.copy_from_slice(&jac_scratch.jacobian);
        };

        let convergence_check =
            |_p: &[f64], r: &[f64], _jac: &[f64]| -> Option<LmTerminationReason> {
                let resid_norm: f64 = r.iter().map(|val| val * val).sum::<f64>().sqrt();
                if resid_norm < self.tolerance {
                    Some(LmTerminationReason::ConvergedResidualNorm)
                } else {
                    None
                }
            };

        self.solve_lm_core_with_stats(
            initial.to_vec(),
            &residuals,
            jacobian_func,
            convergence_check,
            n_residuals,
            None,
        )
    }

    /// Solve system with analytical Jacobian and stats.
    pub fn solve_system_with_jacobian_stats<Res, D>(
        &self,
        residuals: Res,
        derivatives: &D,
        initial: &[f64],
    ) -> Result<LmSolution>
    where
        Res: Fn(&[f64], &mut [f64]),
        D: AnalyticalDerivatives,
    {
        if initial.is_empty() {
            return Err(InputError::Invalid.into());
        }
        let n_residuals = derivatives.residual_count().ok_or_else(|| {
            crate::Error::Validation(
                "solve_system_with_jacobian_stats requires AnalyticalDerivatives::residual_count() to be implemented"
                    .to_string(),
            )
        })?;

        let n_params = initial.len();
        let mut jac_scratch = JacobianScratch::new(n_params, n_residuals);
        let mut jac_2d = vec![vec![0.0; n_params]; n_residuals];

        let jacobian_func = |p: &[f64], _r: &[f64], eval_counter: &mut usize, out: &mut [f64]| {
            if derivatives.has_jacobian() {
                for row in jac_2d.iter_mut() {
                    row.fill(0.0);
                }
                if derivatives.jacobian(p, &mut jac_2d).is_some() {
                    for (i, row) in jac_2d.iter().enumerate() {
                        for (j, &v) in row.iter().enumerate() {
                            out[i * p.len() + j] = v;
                        }
                    }
                    return;
                }
            }
            self.compute_jacobian_system_into(
                &residuals,
                p,
                n_residuals,
                eval_counter,
                &mut jac_scratch,
            );
            out.copy_from_slice(&jac_scratch.jacobian);
        };

        // Convergence check: Residual Norm
        let convergence_check =
            |_p: &[f64], r: &[f64], _jac: &[f64]| -> Option<LmTerminationReason> {
                let resid_norm: f64 = r.iter().map(|val| val * val).sum::<f64>().sqrt();
                if resid_norm < self.tolerance {
                    Some(LmTerminationReason::ConvergedResidualNorm)
                } else {
                    None
                }
            };

        self.solve_lm_core_with_stats(
            initial.to_vec(),
            &residuals,
            jacobian_func,
            convergence_check,
            n_residuals,
            None, // bounds
        )
    }
}

/// Norm of the gradient projected onto the feasible directions of box bounds.
///
/// Gradient components that push a parameter further into an active bound
/// (lower bound with positive gradient, upper bound with negative gradient —
/// for a minimization step `-g`) are zeroed: at such points the objective
/// cannot be reduced without leaving the box, so they do not count against
/// stationarity. Without bounds this is the plain Euclidean gradient norm.
fn projected_gradient_norm(gradient: &[f64], params: &[f64], bounds: Option<&[(f64, f64)]>) -> f64 {
    let mut sum_sq = 0.0;
    for (i, &g) in gradient.iter().enumerate() {
        let blocked = bounds
            .and_then(|b| b.get(i))
            .is_some_and(|&(lo, hi)| (params[i] <= lo && g > 0.0) || (params[i] >= hi && g < 0.0));
        if !blocked {
            sum_sq += g * g;
        }
    }
    sum_sq.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenberg_marquardt_simple_minimum() {
        // Review 2026-06-09 (core math): this test previously passed via the
        // silent best-guess-on-MaxIterations behaviour with tolerance 1e-10
        // (the scalar root-finder formulation stalls at gradient ~1e-9 once
        // the residual underflows toward f64 epsilon). Use an attainable
        // gradient tolerance now that non-convergence is a hard error.
        let solver = LevenbergMarquardtSolver::new().with_tolerance(1e-8);

        // Minimize (x-2)^2 + (y-3)^2
        let objective =
            |params: &[f64]| -> f64 { (params[0] - 2.0).powi(2) + (params[1] - 3.0).powi(2) };

        let initial = vec![0.0, 0.0];
        let result = solver
            .minimize(objective, &initial, None)
            .expect("Minimization should succeed in test");

        assert!((result[0] - 2.0).abs() < 1e-6);
        assert!((result[1] - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_levenberg_marquardt_with_bounds() {
        let solver = LevenbergMarquardtSolver::new();

        // Minimize (x-5)^2 + (y-5)^2 with bounds
        let objective =
            |params: &[f64]| -> f64 { (params[0] - 5.0).powi(2) + (params[1] - 5.0).powi(2) };

        let initial = vec![0.0, 0.0];
        let bounds = vec![(0.0, 3.0), (0.0, 3.0)]; // Constrain solution
        let result = solver
            .minimize(objective, &initial, Some(&bounds))
            .expect("Minimization should succeed in test");

        // Solution should be at boundary (3, 3)
        assert!((result[0] - 3.0).abs() < 1e-6);
        assert!((result[1] - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_minimize_errors_on_max_iterations() {
        // Review 2026-06-09 (core math): minimize() used to silently return the
        // best iterate on MaxIterations; it must now fail loudly.
        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(1e-12)
            .with_max_iterations(0);

        let objective = |params: &[f64]| -> f64 { (params[0] - 2.0).powi(2) };
        let result = solver.minimize(objective, &[0.0], None);

        match result {
            Err(crate::Error::Input(InputError::SolverConvergenceFailed { reason, .. })) => {
                assert!(reason.contains("MaxIterations"), "reason: {reason}");
            }
            other => panic!("expected SolverConvergenceFailed, got {other:?}"),
        }
    }

    #[test]
    fn test_minimize_with_derivatives_errors_on_max_iterations() {
        struct QuadraticDerivatives;
        impl AnalyticalDerivatives for QuadraticDerivatives {
            fn gradient(&self, params: &[f64], gradient: &mut [f64]) {
                gradient[0] = 2.0 * (params[0] - 2.0);
            }
        }

        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(1e-12)
            .with_max_iterations(0);

        let objective = |params: &[f64]| -> f64 { (params[0] - 2.0).powi(2) };
        let result =
            solver.minimize_with_derivatives(objective, &QuadraticDerivatives, &[0.0], None);
        assert!(
            matches!(
                result,
                Err(crate::Error::Input(
                    InputError::SolverConvergenceFailed { .. }
                ))
            ),
            "expected SolverConvergenceFailed, got {result:?}"
        );
    }

    #[test]
    fn test_projected_gradient_norm_zeroes_active_bounds() {
        // At the upper bound with a negative gradient (descent direction blocked
        // by the box), the component must not count against stationarity.
        let gradient = [-4.0, -4.0];
        let params = [3.0, 3.0];
        let bounds = [(0.0, 3.0), (0.0, 3.0)];
        assert_eq!(
            projected_gradient_norm(&gradient, &params, Some(&bounds)),
            0.0
        );
        // Without bounds the full norm applies.
        let norm = projected_gradient_norm(&gradient, &params, None);
        assert!((norm - 32.0_f64.sqrt()).abs() < 1e-12);
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
            .solve_system_with_dim_stats(residuals, &initial, 2)
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
            .with_max_iterations(0);

        let residuals = |params: &[f64], resid: &mut [f64]| {
            resid[0] = params[0] - 1.0;
        };

        let initial = vec![0.0];
        let solution = solver
            .solve_system_with_dim_stats(residuals, &initial, 1)
            .expect("stats solve should succeed");

        assert_eq!(
            solution.stats.termination_reason,
            LmTerminationReason::MaxIterations
        );
        assert_eq!(solution.stats.iterations, 0);
    }

    #[test]
    fn test_least_squares_fitting() {
        // Review 2026-06-09 (core math): previously passed via silent
        // best-guess-on-MaxIterations (residual ~5e-5 after 100 evals). The
        // scalar formulation converges linearly here, so give it an adequate
        // budget and an attainable gradient tolerance.
        let solver = LevenbergMarquardtSolver::new()
            .with_tolerance(1e-6)
            .with_max_iterations(2000);

        // Fit a quadratic y = a*x^2 + b*x + c to some data points
        // True parameters: a=1, b=-2, c=3
        let x_data = [-2.0, -1.0, 0.0, 1.0, 2.0];
        let y_data = [11.0, 6.0, 3.0, 2.0, 3.0]; // y = x^2 - 2*x + 3

        // Least squares objective
        let objective = move |params: &[f64]| -> f64 {
            let a = params[0];
            let b = params[1];
            let c = params[2];

            x_data
                .iter()
                .zip(y_data.iter())
                .map(|(&x, &y_true)| {
                    let y_pred = a * x * x + b * x + c;
                    (y_pred - y_true).powi(2)
                })
                .sum()
        };

        let initial = vec![0.5, 0.5, 0.5];
        let result = solver
            .minimize(objective, &initial, None)
            .expect("Minimization should succeed in test");

        // Should recover the true parameters (within reasonable tolerance)
        assert!(
            (result[0] - 1.0).abs() < 0.01,
            "a = {}, expected 1.0",
            result[0]
        );
        assert!(
            (result[1] - (-2.0)).abs() < 0.01,
            "b = {}, expected -2.0",
            result[1]
        );
        assert!(
            (result[2] - 3.0).abs() < 0.01,
            "c = {}, expected 3.0",
            result[2]
        );
    }

    #[test]
    fn test_analytical_derivatives_simple() {
        // Simple quadratic with analytical derivatives
        struct QuadraticDerivatives;

        impl AnalyticalDerivatives for QuadraticDerivatives {
            fn gradient(&self, params: &[f64], gradient: &mut [f64]) {
                // f(x,y) = (x-2)^2 + (y-3)^2
                gradient[0] = 2.0 * (params[0] - 2.0);
                gradient[1] = 2.0 * (params[1] - 3.0);
            }
        }

        // Review 2026-06-09 (core math): tolerance loosened from 1e-10 to an
        // attainable gradient norm now that MaxIterations is a hard error.
        let solver = LevenbergMarquardtSolver::new().with_tolerance(1e-8);

        let objective =
            |params: &[f64]| -> f64 { (params[0] - 2.0).powi(2) + (params[1] - 3.0).powi(2) };

        let derivatives = QuadraticDerivatives;
        let initial = vec![0.0, 0.0];

        let result = solver
            .minimize_with_derivatives(objective, &derivatives, &initial, None)
            .expect("Minimization should succeed in test");

        assert!((result[0] - 2.0).abs() < 1e-8);
        assert!((result[1] - 3.0).abs() < 1e-8);
    }

    #[test]
    fn test_jacobian_system() {
        // System: x^2 + y^2 = 4, x - y = 0
        struct CircleLineJacobian;

        impl AnalyticalDerivatives for CircleLineJacobian {
            fn gradient(&self, _params: &[f64], _gradient: &mut [f64]) {
                // Not used for this test
            }

            fn jacobian(&self, params: &[f64], jacobian: &mut [Vec<f64>]) -> Option<()> {
                let x = params[0];
                let y = params[1];

                jacobian[0][0] = 2.0 * x; // df1/dx
                jacobian[0][1] = 2.0 * y; // df1/dy
                jacobian[1][0] = 1.0; // df2/dx
                jacobian[1][1] = -1.0; // df2/dy

                Some(())
            }

            fn has_jacobian(&self) -> bool {
                true
            }

            fn has_gradient(&self) -> bool {
                false
            }

            fn residual_count(&self) -> Option<usize> {
                Some(2)
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
            .solve_system_with_jacobian_stats(residuals, &derivatives, &initial)
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
            .solve_system_with_dim_stats(residuals, &initial, 5)
            .expect("solve_system_with_dim_stats should succeed")
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
            .solve_system_with_dim_stats(residuals, &initial, 20)
            .expect("solve_system_with_dim_stats should not panic with 20 residuals")
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
            .solve_system_with_dim_stats(residuals, &initial, 3)
            .expect("solve should succeed")
            .params;

        assert_eq!(result.len(), 3, "Result should have same length as initial");
    }

    #[test]
    fn test_analytic_vs_finite_diff_performance() {
        // Test that analytic derivatives converge faster than finite differences
        struct SimpleGradient;
        impl AnalyticalDerivatives for SimpleGradient {
            fn gradient(&self, params: &[f64], gradient: &mut [f64]) {
                // f(x,y) = (x-2)^2 + (y-3)^2
                gradient[0] = 2.0 * (params[0] - 2.0);
                gradient[1] = 2.0 * (params[1] - 3.0);
            }
        }

        // Review 2026-06-09 (core math): tolerance loosened from 1e-10 to an
        // attainable gradient norm now that MaxIterations is a hard error.
        let solver = LevenbergMarquardtSolver::new().with_tolerance(1e-8);
        let objective =
            |params: &[f64]| -> f64 { (params[0] - 2.0).powi(2) + (params[1] - 3.0).powi(2) };
        let derivatives = SimpleGradient;
        let initial = vec![0.0, 0.0];

        // Method 1: With analytic derivatives
        let result1 = solver
            .minimize_with_derivatives(objective, &derivatives, &initial, None)
            .expect("minimize_with_derivatives should succeed in test");

        // Method 2: With finite differences
        let result2 = solver
            .minimize(objective, &initial, None)
            .expect("minimize should succeed in test");

        // Both should converge to same solution (within reasonable tolerance)
        // Note: Different convergence paths may yield slightly different final values
        for (i, (&v1, &v2)) in result1.iter().zip(result2.iter()).enumerate() {
            assert!(
                (v1 - v2).abs() < 1e-6,
                "Analytic and finite diff should converge to same solution at index {}: {} vs {}",
                i,
                v1,
                v2
            );
        }

        // Both should be close to [2.0, 3.0]
        assert!((result1[0] - 2.0).abs() < 1e-8);
        assert!((result1[1] - 3.0).abs() < 1e-8);
    }

    #[test]
    fn test_jacobian_system_uses_explicit_residual_count() {
        struct TallJacobian;

        impl AnalyticalDerivatives for TallJacobian {
            fn gradient(&self, _params: &[f64], _gradient: &mut [f64]) {}

            fn jacobian(&self, _params: &[f64], jacobian: &mut [Vec<f64>]) -> Option<()> {
                for row in jacobian.iter_mut() {
                    row[0] = 1.0;
                }
                Some(())
            }

            fn has_jacobian(&self) -> bool {
                true
            }

            fn residual_count(&self) -> Option<usize> {
                Some(5)
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
            .solve_system_with_jacobian_stats(residuals, &TallJacobian, &initial)
            .expect("explicit residual count should avoid probe truncation");

        assert_eq!(solution.params.len(), 1);
    }

    #[test]
    fn test_jacobian_system_requires_explicit_residual_count() {
        struct MissingCountJacobian;

        impl AnalyticalDerivatives for MissingCountJacobian {
            fn gradient(&self, _params: &[f64], _gradient: &mut [f64]) {}

            fn jacobian(&self, _params: &[f64], jacobian: &mut [Vec<f64>]) -> Option<()> {
                for row in jacobian.iter_mut() {
                    row[0] = 1.0;
                }
                Some(())
            }

            fn has_jacobian(&self) -> bool {
                true
            }
        }

        let solver = LevenbergMarquardtSolver::new();
        let residuals = |params: &[f64], resid: &mut [f64]| {
            resid[0] = params[0] - 1.0;
        };

        let err = solver
            .solve_system_with_jacobian_stats(residuals, &MissingCountJacobian, &[0.0])
            .expect_err("missing residual_count should fail loudly");

        assert!(
            err.to_string().contains("residual_count"),
            "expected residual_count guidance, got {err}"
        );
    }
}

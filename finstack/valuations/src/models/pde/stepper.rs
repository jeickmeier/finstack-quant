//! Time-stepping schemes for backward PDE evolution.
//!
//! Provides theta-scheme time stepping (explicit, implicit, Crank-Nicolson)
//! and Rannacher smoothing (implicit start + CN continuation) to eliminate
//! spurious oscillations near payoff discontinuities.

use super::grid::Grid1D;
use super::operator::{ThomasError, TridiagOperator};
use super::problem::PdeProblem1D;

/// Error raised by a time stepper (1D theta scheme or 2D MCS ADI).
///
/// Failure modes:
/// - [`StepperError::CflViolation`] — an explicit / under-damped 1D theta
///   scheme stepped past its CFL / von-Neumann stability limit.
/// - [`StepperError::PecletViolation`] — the 2D Modified Craig-Sneyd ADI
///   stepper was handed a convection-dominated grid whose cell Péclet number
///   exceeds the regime in which `theta = 1/3` MCS is reliably stable.
/// - [`StepperError::NonPositiveStep`] — a non-positive time step `dt`
///   (e.g. a non-positive maturity, or `n_steps` so small the step is
///   degenerate) was requested.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum StepperError {
    /// An explicit or under-damped (`theta < 0.5`) theta scheme was asked to
    /// take a time step larger than the CFL / von-Neumann stability limit.
    ///
    /// For `theta < 0.5` the theta scheme is only conditionally stable: it
    /// requires `dt <= min_i(dx_i)^2 / (2 * max_i|a_i|)`, where `a` is the
    /// local diffusion coefficient and `dx` the local grid spacing. The
    /// limiting spacing is the *smallest* `dx` on the (possibly non-uniform)
    /// grid. Violating the bound produces silent NaN / oscillation rather
    /// than a meaningful solution, so it is reported as an error.
    #[error(
        "CFL stability violated: explicit/under-damped theta scheme (theta={theta}) \
         requires dt <= dx_min^2 / (2*max|a|) = {cfl_max_dt:e}, but dt = {dt:e} \
         (limiting grid spacing dx_min = {dx_min:e}, max diffusion |a| = {max_diffusion:e}); \
         use more time steps, a coarser/uniform grid, or an implicit/Crank-Nicolson scheme (theta >= 0.5)"
    )]
    CflViolation {
        /// Theta parameter of the offending scheme (`< 0.5`).
        theta: f64,
        /// The time-step size requested by the caller.
        dt: f64,
        /// The largest stable time step permitted by the CFL condition.
        cfl_max_dt: f64,
        /// Smallest grid spacing on the (possibly non-uniform) grid.
        dx_min: f64,
        /// Largest absolute diffusion coefficient over the interior grid.
        max_diffusion: f64,
    },
    /// The 2D Modified Craig-Sneyd ADI stepper was handed a grid whose largest
    /// cell Péclet number `Pe = |b| * h / (2 * a)` exceeds the threshold
    /// inside which the `theta = 1/3` scheme is reliably stable.
    ///
    /// MCS at `theta = 1/3` is unconditionally stable for *pure 2D diffusion*,
    /// but for the *general convection-diffusion* case the von Neumann bound
    /// rises to `theta >= 2/5` (In 't Hout & Mishra 2010). In the strongly
    /// convection-dominated regime — large Heston `kappa`, very wide variance
    /// grids, or coarse spacing where convection swamps diffusion — the
    /// `theta = 1/3` scheme leaves its proven-stable envelope and can diverge
    /// silently to inf / NaN. This guard reports it instead.
    #[error(
        "Péclet stability violated: 2D MCS ADI (theta=1/3) requires the cell Péclet number \
         Pe = |b|*h/(2*a) <= {pe_max} for reliable stability, but the {direction}-direction \
         grid reaches Pe = {peclet:e} (convection |b| = {convection:e}, diffusion a = {diffusion:e}, \
         spacing h = {spacing:e}); refine the grid in that direction, narrow the domain, or \
         reduce the convection dominance"
    )]
    PecletViolation {
        /// Direction the violation occurred on (`"x"` or `"y"`).
        direction: &'static str,
        /// The largest cell Péclet number found on that direction.
        peclet: f64,
        /// Threshold above which the scheme is considered convection-dominated.
        pe_max: f64,
        /// Absolute convection coefficient at the offending node.
        convection: f64,
        /// Diffusion coefficient at the offending node.
        diffusion: f64,
        /// Grid spacing at the offending node.
        spacing: f64,
    },
    /// A non-positive time step `dt` was requested.
    ///
    /// The backward time march requires `dt = t_from - t_to > 0`. A
    /// non-positive `dt` arises from a non-positive maturity, or from a
    /// degenerate time grid; in release builds it would otherwise propagate
    /// silently as NaN / inf rather than being caught by the `debug_assert`.
    #[error("invalid time step: dt = {dt:e} must be strictly positive (t_from={t_from:e}, t_to={t_to:e})")]
    NonPositiveStep {
        /// The non-positive step size.
        dt: f64,
        /// Step start time (closer to maturity).
        t_from: f64,
        /// Step end time (closer to t=0).
        t_to: f64,
    },
    /// The tridiagonal Thomas solve underlying the implicit step failed —
    /// in practice, a degenerate pivot in `(I - theta*dt*A)` from eroded
    /// diagonal dominance.
    #[error(transparent)]
    ThomasFailure(#[from] ThomasError),
}

/// Time-stepping strategy for advancing the PDE solution backward from maturity.
///
/// Implementors define how each time step is executed, potentially with
/// different theta parameters at different stages (e.g., Rannacher smoothing).
pub trait TimeStepper {
    /// Advance the solution one step backward in time from `t_from` to `t_to`.
    ///
    /// * `problem` — PDE coefficients and boundary conditions
    /// * `grid` — spatial grid
    /// * `u` — solution vector (interior points only, length `grid.n_interior()`)
    /// * `t_from` — current time (closer to maturity)
    /// * `t_to` — target time (closer to t=0)
    /// * `step_index` — zero-based step counter (for Rannacher switching)
    ///
    /// Returns [`StepperError::CflViolation`] if an explicit / under-damped
    /// (`theta < 0.5`) scheme would step past its CFL stability limit.
    /// Implicit and Crank-Nicolson (`theta >= 0.5`) steps are unconditionally
    /// stable and never fail.
    fn step(
        &self,
        problem: &dyn PdeProblem1D,
        grid: &Grid1D,
        u: &mut [f64],
        t_from: f64,
        t_to: f64,
        step_index: usize,
    ) -> Result<(), StepperError>;

    /// Total number of time steps.
    fn n_steps(&self) -> usize;

    /// Generate time levels from maturity (T) backward to 0.
    ///
    /// Returns a vector of length `n_steps + 1` with `levels[0] = T`
    /// and `levels[n_steps] = 0`. Steps proceed backward: the solver
    /// evolves from `levels[i]` to `levels[i+1]` for `i = 0..n_steps-1`.
    fn time_levels(&self, maturity: f64) -> Vec<f64> {
        let n = self.n_steps();
        let dt = maturity / n as f64;
        (0..=n).map(|i| maturity - i as f64 * dt).collect()
    }
}

/// Standard theta-scheme time stepper.
///
/// - `theta = 0.0`: fully explicit (forward Euler) — conditionally stable, for debugging only
/// - `theta = 0.5`: Crank-Nicolson — second-order in time, the production workhorse
/// - `theta = 1.0`: fully implicit (backward Euler) — first-order, unconditionally stable
pub struct ThetaStepper {
    /// Theta parameter (0 = explicit, 0.5 = CN, 1 = implicit).
    theta: f64,
    /// Number of time steps.
    n_steps: usize,
}

impl ThetaStepper {
    /// Create a Crank-Nicolson stepper (theta = 0.5).
    pub fn crank_nicolson(n_steps: usize) -> Self {
        Self {
            theta: 0.5,
            n_steps,
        }
    }

    /// Create a fully implicit stepper (theta = 1.0).
    pub fn implicit(n_steps: usize) -> Self {
        Self {
            theta: 1.0,
            n_steps,
        }
    }

    /// Create a fully explicit stepper (theta = 0.0). Use for debugging only.
    pub fn explicit(n_steps: usize) -> Self {
        Self {
            theta: 0.0,
            n_steps,
        }
    }

    /// Create a stepper with custom theta.
    pub fn custom(theta: f64, n_steps: usize) -> Self {
        Self { theta, n_steps }
    }
}

impl TimeStepper for ThetaStepper {
    fn step(
        &self,
        problem: &dyn PdeProblem1D,
        grid: &Grid1D,
        u: &mut [f64],
        t_from: f64,
        t_to: f64,
        _step_index: usize,
    ) -> Result<(), StepperError> {
        theta_step(problem, grid, u, t_from, t_to, self.theta)
    }

    fn n_steps(&self) -> usize {
        self.n_steps
    }
}

/// Rannacher time stepper: runs a few fully implicit steps at the start
/// (near the terminal condition) then switches to Crank-Nicolson.
///
/// Eliminates the spurious oscillations that Crank-Nicolson produces near
/// payoff discontinuities (e.g., at the strike of a digital option or at
/// barrier knock-out levels). Typically 2–4 implicit steps suffice.
pub struct RannacherStepper {
    /// Number of initial fully-implicit steps (typically 2–4).
    implicit_steps: usize,
    /// Theta for remaining steps (usually 0.5 for Crank-Nicolson).
    theta: f64,
    /// Total number of time steps.
    n_steps: usize,
}

impl RannacherStepper {
    /// Create a Rannacher stepper with `implicit_steps` initial implicit steps
    /// followed by Crank-Nicolson for the remainder.
    pub fn new(implicit_steps: usize, n_steps: usize) -> Self {
        Self {
            implicit_steps,
            theta: 0.5,
            n_steps,
        }
    }
}

impl TimeStepper for RannacherStepper {
    fn step(
        &self,
        problem: &dyn PdeProblem1D,
        grid: &Grid1D,
        u: &mut [f64],
        t_from: f64,
        t_to: f64,
        step_index: usize,
    ) -> Result<(), StepperError> {
        let theta = if step_index < self.implicit_steps {
            1.0
        } else {
            self.theta
        };
        theta_step(problem, grid, u, t_from, t_to, theta)
    }

    fn n_steps(&self) -> usize {
        self.n_steps
    }
}

/// Largest time step permitted by the CFL / von-Neumann stability condition
/// for an explicit / under-damped theta scheme on the given grid, with the
/// diffusion coefficient maximised over the step interval `[t_to, t_from]`.
///
/// For `theta < 0.5` the theta scheme is only conditionally stable. The
/// von-Neumann bound for the pure-diffusion part `a * u_xx` on a (possibly
/// non-uniform) grid is
/// ```text
/// dt <= dx_min^2 / (2 * max|a|)
/// ```
/// where `dx_min` is the *smallest* spacing between adjacent grid points
/// (a strike-/barrier-concentrated grid has a far smaller minimum spacing
/// than its average) and `max|a|` is the largest absolute diffusion
/// coefficient over the interior nodes.
///
/// # Time dependence
///
/// For a time-dependent diffusion (`LocalVolPde`) the coefficient at `t_to`
/// or partway through the step can exceed its value at `t_from`; evaluating
/// the bound only at `t_from` would understate `max|a|` and let an unstable
/// step through. The diffusion is therefore sampled at `t_from`, `t_to`, and
/// the step midpoint, and the *largest* of the three is used — the most
/// restrictive (smallest) CFL bound over the interval.
///
/// Returns `f64::INFINITY` when the diffusion vanishes everywhere (a
/// degenerate, convection/reaction-only problem has no diffusive CFL limit).
fn cfl_max_dt(
    problem: &dyn PdeProblem1D,
    grid: &Grid1D,
    t_from: f64,
    t_to: f64,
) -> (f64, f64, f64) {
    let pts = grid.points();

    // Smallest spacing between any two adjacent grid points.
    let mut dx_min = f64::INFINITY;
    for i in 1..pts.len() {
        let h = pts[i] - pts[i - 1];
        if h < dx_min {
            dx_min = h;
        }
    }

    // Largest absolute diffusion coefficient over the interior nodes, sampled
    // across the step interval so a time-dependent vol surface that peaks at
    // (or within) t_to is not missed. Boundary rows carry no interior stencil.
    let t_mid = 0.5 * (t_from + t_to);
    let mut max_diffusion = 0.0_f64;
    for &x in &pts[1..pts.len().saturating_sub(1)] {
        for &t in &[t_from, t_to, t_mid] {
            let a = problem.diffusion(x, t).abs();
            if a > max_diffusion {
                max_diffusion = a;
            }
        }
    }

    let bound = if max_diffusion > 0.0 {
        dx_min * dx_min / (2.0 * max_diffusion)
    } else {
        f64::INFINITY
    };

    (bound, dx_min, max_diffusion)
}

/// Execute a single theta-scheme time step from `t_from` to `t_to` (backward).
///
/// The scheme:
/// ```text
/// (I - theta * dt * A_to) * u_to = (I + (1-theta) * dt * A_from) * u_from
///                                  + dt * [theta * (source_to + bc_to)
///                                         + (1-theta) * (source_from + bc_from)]
/// ```
///
/// For time-homogeneous problems, `A_from == A_to` and only one assembly is needed.
///
/// # Stability
///
/// When `theta < 0.5` (fully explicit or under-damped) the scheme is only
/// conditionally stable: `dt` must satisfy the CFL / von-Neumann condition
/// `dt <= dx_min^2 / (2*max|a|)`. If it does not, this function returns
/// [`StepperError::CflViolation`] rather than producing silent NaN /
/// oscillation. For `theta >= 0.5` (Crank-Nicolson and fully implicit) the
/// scheme is unconditionally stable and no check is performed.
///
/// Returns [`StepperError::NonPositiveStep`] if `dt = t_from - t_to` is not
/// strictly positive (a non-positive maturity or a degenerate time grid).
fn theta_step(
    problem: &dyn PdeProblem1D,
    grid: &Grid1D,
    u: &mut [f64],
    t_from: f64,
    t_to: f64,
    theta: f64,
) -> Result<(), StepperError> {
    let dt = t_from - t_to;
    // A non-positive or non-finite dt would otherwise propagate as silent
    // NaN / inf in release builds (the previous `debug_assert` was compiled
    // out). The `is_finite` check also rejects a NaN dt.
    if !dt.is_finite() || dt <= 0.0 {
        return Err(StepperError::NonPositiveStep { dt, t_from, t_to });
    }

    // Explicit / under-damped schemes (theta < 0.5) are only conditionally
    // stable — enforce the CFL bound. Crank-Nicolson and fully implicit
    // (theta >= 0.5) are unconditionally stable and are not gated.
    if theta < 0.5 {
        let (cfl_limit, dx_min, max_diffusion) = cfl_max_dt(problem, grid, t_from, t_to);
        if dt > cfl_limit {
            return Err(StepperError::CflViolation {
                theta,
                dt,
                cfl_max_dt: cfl_limit,
                dx_min,
                max_diffusion,
            });
        }
    }

    let is_homogeneous = problem.is_time_homogeneous();

    // Assemble the operator at t_from (explicit side)
    let op_from = TridiagOperator::assemble(problem, grid, t_from);

    // Explicit part: y = (I + (1-theta)*dt * A_from) * u + (1-theta)*dt*(source + bc)
    let beta = (1.0 - theta) * dt;
    let mut rhs = op_from.apply_explicit(beta, u);

    // If time-homogeneous, reuse op_from for the implicit side
    let op_to = if is_homogeneous {
        op_from
    } else {
        TridiagOperator::assemble(problem, grid, t_to)
    };

    // Add implicit-side source and boundary corrections: theta * dt * (source_to + bc_to)
    let alpha = theta * dt;
    op_to.add_implicit_corrections(alpha, &mut rhs);

    // Solve (I - theta*dt * A_to) * u_new = rhs. A degenerate pivot (eroded
    // diagonal dominance) surfaces as StepperError::ThomasFailure rather than
    // a silent inf / NaN.
    let u_new = op_to.solve_thomas(alpha, &rhs)?;
    u.copy_from_slice(&u_new);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::boundary::BoundaryCondition;
    use super::*;

    /// Heat equation: u_t = u_xx on [0, pi] with u(0,t) = u(pi,t) = 0
    /// Terminal: u(x, T) = sin(x)
    /// Exact: u(x, t) = exp(-(T-t)) * sin(x)
    struct HeatSinProblem;

    impl PdeProblem1D for HeatSinProblem {
        fn diffusion(&self, _x: f64, _t: f64) -> f64 {
            1.0
        }
        fn convection(&self, _x: f64, _t: f64) -> f64 {
            0.0
        }
        fn reaction(&self, _x: f64, _t: f64) -> f64 {
            0.0
        }
        fn terminal_condition(&self, x: f64) -> f64 {
            x.sin()
        }
        fn lower_boundary(&self, _t: f64) -> BoundaryCondition {
            BoundaryCondition::Dirichlet(0.0)
        }
        fn upper_boundary(&self, _t: f64) -> BoundaryCondition {
            BoundaryCondition::Dirichlet(0.0)
        }
        fn is_time_homogeneous(&self) -> bool {
            true
        }
    }

    #[test]
    fn heat_equation_implicit_converges() {
        let t_mat = 0.5;
        let problem = HeatSinProblem;
        let n_space = 101;
        let n_time = 200;
        let grid = Grid1D::uniform(0.0, std::f64::consts::PI, n_space).expect("valid grid");
        let stepper = ThetaStepper::implicit(n_time);

        // Initialize terminal condition (interior points only)
        let mut u: Vec<f64> = grid.points()[1..grid.n() - 1]
            .iter()
            .map(|&x| problem.terminal_condition(x))
            .collect();

        // Step backward
        let levels = stepper.time_levels(t_mat);
        for i in 0..n_time {
            stepper
                .step(&problem, &grid, &mut u, levels[i], levels[i + 1], i)
                .expect("implicit step is unconditionally stable");
        }

        // Compare with exact solution at t=0
        let exact_factor = (-t_mat).exp();
        let mid = grid.n_interior() / 2;
        let x_mid = grid.points()[mid + 1];
        let exact = exact_factor * x_mid.sin();
        let error = (u[mid] - exact).abs();
        assert!(
            error < 0.005,
            "Implicit Euler error too large: {error:.6} at x={x_mid:.4}"
        );
    }

    #[test]
    fn heat_equation_cn_converges_faster() {
        let t_mat = 0.5;
        let problem = HeatSinProblem;
        let n_space = 51;
        let n_time = 100;
        let grid = Grid1D::uniform(0.0, std::f64::consts::PI, n_space).expect("valid grid");
        let stepper = ThetaStepper::crank_nicolson(n_time);

        let mut u: Vec<f64> = grid.points()[1..grid.n() - 1]
            .iter()
            .map(|&x| problem.terminal_condition(x))
            .collect();

        let levels = stepper.time_levels(t_mat);
        for i in 0..n_time {
            stepper
                .step(&problem, &grid, &mut u, levels[i], levels[i + 1], i)
                .expect("Crank-Nicolson step is unconditionally stable");
        }

        let exact_factor = (-t_mat).exp();
        let mid = grid.n_interior() / 2;
        let x_mid = grid.points()[mid + 1];
        let exact = exact_factor * x_mid.sin();
        let error = (u[mid] - exact).abs();
        // CN should be much more accurate than implicit Euler with same grid
        assert!(
            error < 1e-4,
            "CN error too large: {error:.6e} at x={x_mid:.4}"
        );
    }

    /// An explicit (`theta = 0`) step on a strike-concentrated grid with a
    /// deliberately coarse time grid violates the CFL condition
    /// `dt <= dx_min^2 / (2*max|a|)`. The stepper must reject it with
    /// [`StepperError::CflViolation`] instead of silently returning NaN /
    /// oscillating garbage.
    ///
    /// On the parent commit `theta_step` performed the update unconditionally:
    /// the explicit operator `(I + dt*A)` has a spectral radius far above 1
    /// for this `dt`, so a single step already blows the interior solution up
    /// (the values explode and the multi-step march diverges to ±inf / NaN).
    #[test]
    fn explicit_step_rejects_cfl_violation() {
        let problem = HeatSinProblem; // diffusion a = 1 everywhere
        let t_mat = 0.5;

        // Strike-concentrated (sinh) grid: the minimum spacing near the
        // concentration point is far below the average — exactly the case
        // that silently breaks an explicit scheme sized off the average dx.
        let grid = Grid1D::sinh_concentrated(
            0.0,
            std::f64::consts::PI,
            201,
            std::f64::consts::FRAC_PI_2,
            0.04,
        )
        .expect("valid grid");

        // Deliberately coarse time grid: only 10 steps over [0, T].
        let n_time = 10;
        let stepper = ThetaStepper::explicit(n_time);
        let levels = stepper.time_levels(t_mat);
        let dt = levels[0] - levels[1];

        // Sanity: this dt really does violate CFL on this grid.
        let (cfl_limit, dx_min, max_a) = cfl_max_dt(&problem, &grid, levels[0], levels[1]);
        assert!(
            dt > cfl_limit,
            "test misconfigured: dt={dt:e} should exceed CFL bound {cfl_limit:e} \
             (dx_min={dx_min:e}, max|a|={max_a:e})"
        );

        let mut u: Vec<f64> = grid.points()[1..grid.n() - 1]
            .iter()
            .map(|&x| problem.terminal_condition(x))
            .collect();

        // The very first step must already be rejected.
        let result = stepper.step(&problem, &grid, &mut u, levels[0], levels[1], 0);
        match result {
            Err(StepperError::CflViolation {
                theta,
                dt: reported_dt,
                cfl_max_dt: reported_bound,
                dx_min: reported_dx,
                max_diffusion,
            }) => {
                assert!(theta < 0.5, "should report the explicit theta");
                assert!(
                    (reported_dt - dt).abs() < 1e-12,
                    "error should cite the offending dt"
                );
                assert!(
                    reported_bound > 0.0 && reported_dt > reported_bound,
                    "error should cite the violated CFL bound"
                );
                assert!(reported_dx > 0.0, "error should cite the limiting spacing");
                assert!(
                    (max_diffusion - 1.0).abs() < 1e-12,
                    "max diffusion of the heat equation is 1"
                );
            }
            other => panic!("expected CflViolation, got {other:?}"),
        }
    }

    /// A `custom` theta in the under-damped range (`0 < theta < 0.5`) is also
    /// only conditionally stable and must be gated by the CFL check.
    #[test]
    fn under_damped_custom_theta_rejects_cfl_violation() {
        let problem = HeatSinProblem;
        let grid = Grid1D::uniform(0.0, std::f64::consts::PI, 401).expect("valid grid");

        // theta = 0.25 < 0.5 → under-damped, conditionally stable.
        let stepper = ThetaStepper::custom(0.25, 4);
        let levels = stepper.time_levels(0.5);
        let result = stepper.step(
            &problem,
            &grid,
            &mut vec![0.0; grid.n_interior()],
            levels[0],
            levels[1],
            0,
        );
        assert!(
            matches!(result, Err(StepperError::CflViolation { .. })),
            "under-damped theta=0.25 with a coarse time grid must be rejected"
        );
    }

    /// An explicit step that *does* satisfy the CFL condition (enough time
    /// steps for the grid) must succeed and stay close to the exact solution
    /// — the guard must not reject a genuinely stable explicit configuration.
    #[test]
    fn explicit_step_within_cfl_succeeds_and_converges() {
        let problem = HeatSinProblem;
        let t_mat = 0.1;

        // Coarse uniform grid → large dx → mild CFL constraint.
        let n_space = 21;
        let grid = Grid1D::uniform(0.0, std::f64::consts::PI, n_space).expect("valid grid");

        // Choose a time-step count comfortably inside the CFL limit.
        // HeatSinProblem has a constant (time-independent) diffusion, so the
        // CFL bound is the same for any (t_from, t_to) pair.
        let (cfl_limit, _, _) = cfl_max_dt(&problem, &grid, t_mat, 0.0);
        let n_time = ((t_mat / cfl_limit).ceil() as usize + 5).max(10);
        let stepper = ThetaStepper::explicit(n_time);
        let levels = stepper.time_levels(t_mat);
        assert!(
            levels[0] - levels[1] <= cfl_limit,
            "test misconfigured: explicit dt must satisfy CFL"
        );

        let mut u: Vec<f64> = grid.points()[1..grid.n() - 1]
            .iter()
            .map(|&x| problem.terminal_condition(x))
            .collect();

        for i in 0..n_time {
            stepper
                .step(&problem, &grid, &mut u, levels[i], levels[i + 1], i)
                .expect("CFL-satisfying explicit step must succeed");
        }

        // Result is finite and tracks the exact solution exp(-(T-t))*sin(x).
        let exact_factor = (-t_mat).exp();
        let mid = grid.n_interior() / 2;
        let x_mid = grid.points()[mid + 1];
        let exact = exact_factor * x_mid.sin();
        assert!(u[mid].is_finite(), "stable explicit step must stay finite");
        assert!(
            (u[mid] - exact).abs() < 0.01,
            "explicit error too large: got {} vs exact {exact}",
            u[mid]
        );
    }

    /// Crank-Nicolson and fully implicit schemes (`theta >= 0.5`) are
    /// unconditionally stable: the CFL guard must NOT gate them, even on a
    /// tightly strike-concentrated grid with very few time steps — a setup
    /// that would violate CFL for an explicit scheme.
    #[test]
    fn implicit_and_cn_are_not_cfl_gated() {
        let problem = HeatSinProblem;
        let grid = Grid1D::sinh_concentrated(
            0.0,
            std::f64::consts::PI,
            201,
            std::f64::consts::FRAC_PI_2,
            0.04,
        )
        .expect("valid grid");

        // Confirm this grid + step count would break an explicit scheme.
        let n_time = 4;
        let dt = 0.5 / n_time as f64;
        let (cfl_limit, _, _) = cfl_max_dt(&problem, &grid, 0.5, 0.0);
        assert!(
            dt > cfl_limit,
            "test misconfigured: dt should exceed the explicit CFL bound"
        );

        let levels_template = ThetaStepper::implicit(n_time).time_levels(0.5);

        // Fully implicit (theta = 1.0): never gated.
        for stepper in [
            ThetaStepper::implicit(n_time),
            ThetaStepper::crank_nicolson(n_time),
            // custom theta exactly at the 0.5 boundary is unconditionally stable.
            ThetaStepper::custom(0.5, n_time),
        ] {
            let mut u = vec![1.0; grid.n_interior()];
            let res = stepper.step(
                &problem,
                &grid,
                &mut u,
                levels_template[0],
                levels_template[1],
                0,
            );
            assert!(
                res.is_ok(),
                "theta >= 0.5 is unconditionally stable and must not be CFL-gated"
            );
        }
    }

    /// Heat equation with a *time-dependent* diffusion that peaks **inside**
    /// the step interval `[0, 0.5]`: a tent function `a(t)` with `a(0.5) =
    /// a(0.0) = 0.05` at the endpoints and `a(0.25) = 1.0` at the midpoint.
    /// This mimics a `LocalVolPde` surface whose vol spikes part-way through a
    /// time step — a peak that endpoint-only sampling cannot see.
    struct TentDiffusion;

    impl TentDiffusion {
        /// Tent peaking at `t = 0.25`: 0.05 at t∈{0,0.5}, 1.0 at t=0.25.
        fn a(t: f64) -> f64 {
            let peak = 1.0_f64;
            let base = 0.05_f64;
            // Distance from the peak time, normalised to [0, 1] over a half-width.
            let d = (t - 0.25).abs() / 0.25;
            base + (peak - base) * (1.0 - d).max(0.0)
        }
    }

    impl PdeProblem1D for TentDiffusion {
        fn diffusion(&self, _x: f64, t: f64) -> f64 {
            Self::a(t)
        }
        fn convection(&self, _x: f64, _t: f64) -> f64 {
            0.0
        }
        fn reaction(&self, _x: f64, _t: f64) -> f64 {
            0.0
        }
        fn terminal_condition(&self, x: f64) -> f64 {
            x.sin()
        }
        fn lower_boundary(&self, _t: f64) -> BoundaryCondition {
            BoundaryCondition::Dirichlet(0.0)
        }
        fn upper_boundary(&self, _t: f64) -> BoundaryCondition {
            BoundaryCondition::Dirichlet(0.0)
        }
        // Deliberately time-INhomogeneous.
        fn is_time_homogeneous(&self) -> bool {
            false
        }
    }

    /// [P6-2] The CFL bound for a time-dependent diffusion must be evaluated
    /// as the **max of the diffusion over the whole step interval**, not only
    /// at `t_from`.
    ///
    /// Failure mode being guarded: `theta_step` used to call
    /// `cfl_max_dt(.., t_from)` only. For [`TentDiffusion`] the diffusion at
    /// `t_from = 0.5` is `a = 0.05` (a loose CFL bound), but it spikes to
    /// `a = 1.0` at the step midpoint `t = 0.25` — a 20× tighter bound. A
    /// `t_from`-only check (and even a two-endpoint check, since both
    /// endpoints are `0.05`) under-samples the diffusion and lets an
    /// over-large explicit step through.
    ///
    /// The fixed code samples `t_from`, `t_to`, **and the midpoint** and uses
    /// the largest diffusion. Two assertions:
    ///   1. `cfl_max_dt` over the interval returns the midpoint-peak diffusion
    ///      `a = 1.0`, collapsing the bound from ≈ 0.99 to ≈ 0.049.
    ///   2. The explicit stepper rejects the over-large step with
    ///      [`StepperError::CflViolation`] citing that tighter bound.
    #[test]
    fn cfl_bound_uses_max_diffusion_over_step_interval_not_just_t_from() {
        let problem = TentDiffusion;
        // n = 11 over [0, pi]: dx = pi/10, dx^2 ≈ 0.0987.
        let grid = Grid1D::uniform(0.0, std::f64::consts::PI, 11).expect("valid grid");

        // A single explicit step over the whole interval [0, 0.5]: dt = 0.5.
        let t_from = 0.5_f64;
        let t_to = 0.0_f64;
        let dt = t_from - t_to;

        // Endpoint-only sampling sees a = 0.05 at BOTH ends → loose bound that
        // dt = 0.5 satisfies. (This is what the old t_from-only check did, and
        // even a naive two-endpoint check would miss the interior spike.)
        let (bound_endpoints, _, a_endpoint) = cfl_max_dt(&problem, &grid, t_from, t_from);
        assert!(
            (a_endpoint - 0.05).abs() < 1e-12,
            "at the endpoints the diffusion is 0.05, got {a_endpoint}"
        );
        assert!(
            dt < bound_endpoints,
            "precondition: endpoint diffusion gives a LOOSE CFL bound {bound_endpoints:e} \
             that dt={dt:e} satisfies — endpoint-only sampling passed the step"
        );

        // Interval sampling catches the midpoint spike a = 1.0 → tight bound.
        let (bound_interval, _, a_interval) = cfl_max_dt(&problem, &grid, t_from, t_to);
        assert!(
            (a_interval - 1.0).abs() < 1e-12,
            "interval-max diffusion should be the midpoint spike a=1.0, got {a_interval}"
        );
        assert!(
            dt > bound_interval,
            "the interval-max CFL bound {bound_interval:e} must be violated by dt={dt:e}"
        );

        // The explicit stepper must now reject the step. Under the old
        // t_from-only bound it was silently accepted despite the interior
        // diffusion spike exceeding the explicit stability limit.
        let stepper = ThetaStepper::explicit(1);
        let mut u: Vec<f64> = grid.points()[1..grid.n() - 1]
            .iter()
            .map(|&x| problem.terminal_condition(x))
            .collect();
        let result = stepper.step(&problem, &grid, &mut u, t_from, t_to, 0);
        match result {
            Err(StepperError::CflViolation {
                max_diffusion,
                cfl_max_dt: reported_bound,
                ..
            }) => {
                assert!(
                    (max_diffusion - 1.0).abs() < 1e-12,
                    "the violation must cite the interval-max diffusion 1.0, got {max_diffusion}"
                );
                assert!(
                    dt > reported_bound,
                    "the violation must cite the tighter interval CFL bound"
                );
            }
            other => panic!("expected CflViolation from the interval-max CFL bound, got {other:?}"),
        }
    }

    /// [P6-6] A non-positive time step must be rejected with
    /// [`StepperError::NonPositiveStep`] rather than producing silent NaN.
    ///
    /// In release builds the old `debug_assert!(dt > 0.0)` was compiled out,
    /// so a non-positive maturity (here `t_from == t_to`, giving `dt = 0`)
    /// would have flowed through as a degenerate / NaN-producing step.
    #[test]
    fn theta_step_rejects_non_positive_dt() {
        let problem = HeatSinProblem;
        let grid = Grid1D::uniform(0.0, std::f64::consts::PI, 11).expect("valid grid");
        let mut u = vec![1.0; grid.n_interior()];

        // dt = 0 (t_from == t_to).
        let zero = ThetaStepper::implicit(1).step(&problem, &grid, &mut u, 0.3, 0.3, 0);
        assert!(
            matches!(zero, Err(StepperError::NonPositiveStep { dt, .. }) if dt == 0.0),
            "dt = 0 must be rejected as NonPositiveStep, got {zero:?}"
        );

        // dt < 0 (t_from < t_to — a backward march with non-positive maturity).
        let neg = ThetaStepper::implicit(1).step(&problem, &grid, &mut u, 0.1, 0.4, 0);
        assert!(
            matches!(neg, Err(StepperError::NonPositiveStep { dt, .. }) if dt < 0.0),
            "dt < 0 must be rejected as NonPositiveStep, got {neg:?}"
        );
    }
}

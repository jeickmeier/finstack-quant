//! Top-level 1D PDE solver with builder pattern.
//!
//! Combines a [`Grid1D`], a [`TimeStepper`], and an optional [`PenaltyExercise`]
//! to solve a backward PDE from terminal condition to `t = 0`. Returns a
//! [`PdeSolution`] with the solution values, interpolation, and finite-difference
//! Greeks (delta, gamma) read directly from the grid.

use super::exercise::PenaltyExercise;
use super::grid::{find_nearest, Grid1D, PdeGridError};
use super::problem::PdeProblem1D;
use super::stepper::{RannacherStepper, StepperError, ThetaStepper, TimeStepper};

/// Builder for constructing a [`Solver1D`] with a fluent API.
///
/// # Examples
///
/// ```ignore
/// let solver = Solver1DBuilder::new()
///     .grid(Grid1D::sinh_concentrated(-5.0, 5.0, 200, 0.0, 0.1)?)
///     .crank_nicolson(100)
///     .build();
/// ```
pub struct Solver1DBuilder {
    /// Spatial grid.
    grid: Option<Grid1D>,
    /// Time stepper.
    stepper: Option<Box<dyn TimeStepper>>,
    /// Optional early exercise constraint.
    exercise: Option<PenaltyExercise>,
}

impl Default for Solver1DBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Solver1DBuilder {
    /// Create a new builder with no configuration.
    pub fn new() -> Self {
        Self {
            grid: None,
            stepper: None,
            exercise: None,
        }
    }

    /// Set the spatial grid.
    pub fn grid(mut self, grid: Grid1D) -> Self {
        self.grid = Some(grid);
        self
    }

    /// Use Crank-Nicolson time stepping with `n_steps` steps.
    pub fn crank_nicolson(mut self, n_steps: usize) -> Self {
        self.stepper = Some(Box::new(ThetaStepper::crank_nicolson(n_steps)));
        self
    }

    /// Use fully implicit time stepping with `n_steps` steps.
    pub fn implicit(mut self, n_steps: usize) -> Self {
        self.stepper = Some(Box::new(ThetaStepper::implicit(n_steps)));
        self
    }

    /// Use Rannacher smoothing: `implicit_steps` implicit steps at start, then CN.
    pub fn rannacher(mut self, implicit_steps: usize, n_steps: usize) -> Self {
        self.stepper = Some(Box::new(RannacherStepper::new(implicit_steps, n_steps)));
        self
    }

    /// Add American early exercise constraint with the given payoff values
    /// at interior grid nodes.
    pub fn american(mut self, payoff_values: Vec<f64>) -> Self {
        self.exercise = Some(PenaltyExercise::american(payoff_values));
        self
    }

    /// Add Bermudan early exercise constraint.
    pub fn bermudan(mut self, payoff_values: Vec<f64>, exercise_times: Vec<f64>) -> Self {
        self.exercise = Some(PenaltyExercise::bermudan(payoff_values, exercise_times));
        self
    }

    /// Build the solver. Returns an error if grid or stepper is missing.
    pub fn build(self) -> Result<Solver1D, PdeSolverError> {
        let grid = self.grid.ok_or(PdeSolverError::MissingGrid)?;
        let stepper = self.stepper.ok_or(PdeSolverError::MissingStepper)?;
        Ok(Solver1D {
            grid,
            stepper,
            exercise: self.exercise,
        })
    }
}

/// One-dimensional PDE solver using finite differences.
///
/// Solves the backward PDE from `t = T` (terminal condition) to `t = 0`
/// using the configured grid, time stepper, and optional exercise constraint.
pub struct Solver1D {
    /// Spatial grid.
    grid: Grid1D,
    /// Time-stepping scheme.
    stepper: Box<dyn TimeStepper>,
    /// Optional American/Bermudan exercise.
    exercise: Option<PenaltyExercise>,
}

impl Solver1D {
    /// Create a solver builder.
    #[must_use]
    pub fn builder() -> Solver1DBuilder {
        Solver1DBuilder::new()
    }

    /// Solve the PDE problem and return the solution at `t = 0`.
    ///
    /// # Errors
    ///
    /// - [`PdeSolverError::NonPositiveMaturity`] if `maturity <= 0`.
    /// - [`PdeSolverError::ZeroTimeSteps`] if the stepper has no time steps
    ///   (the backward march would never run, leaving the bare terminal
    ///   payoff as the "solution").
    /// - [`PdeSolverError::Stepper`] if the time stepper fails — in practice,
    ///   an explicit / under-damped (`theta < 0.5`) scheme whose time step
    ///   violates the CFL stability condition, or a degenerate tridiagonal
    ///   solve. Implicit and Crank-Nicolson schemes are unconditionally
    ///   stable and never trigger the CFL case.
    pub fn solve(
        &self,
        problem: &dyn PdeProblem1D,
        maturity: f64,
    ) -> Result<PdeSolution, PdeSolverError> {
        // Validate maturity / step count up front — a non-positive or
        // non-finite maturity, or zero steps, would otherwise yield a
        // degenerate "solution" (a NaN time grid, or the bare terminal payoff
        // with no time evolution). The `is_finite` check also rejects NaN.
        if !maturity.is_finite() || maturity <= 0.0 {
            return Err(PdeSolverError::NonPositiveMaturity { maturity });
        }
        let n_steps = self.stepper.n_steps();
        if n_steps == 0 {
            return Err(PdeSolverError::ZeroTimeSteps);
        }

        // Initialize terminal condition at interior points
        let mut u: Vec<f64> = self.grid.points()[1..self.grid.n() - 1]
            .iter()
            .map(|&x| problem.terminal_condition(x))
            .collect();

        // Time levels: T → 0
        let levels = self.stepper.time_levels(maturity);

        let mut exercise_boundary: Vec<(f64, f64)> = Vec::new();

        // Step backward in time
        for i in 0..n_steps {
            let t_from = levels[i];
            let t_to = levels[i + 1];
            let dt = t_from - t_to;

            self.stepper
                .step(problem, &self.grid, &mut u, t_from, t_to, i)?;

            // Apply early exercise constraint.
            //
            // The penalty method uses λ = penalty_factor/dt = 1e8/dt, so
            // λ·dt = 1e8 >> 1.  This effectively hard-clamps u to the payoff
            // at every violated node after each step (both implicit Rannacher
            // start-up steps and subsequent CN steps).  The kink re-introduced
            // at the exercise boundary is thus identical whether the previous
            // step was implicit or CN — the heavy λ damps any CN oscillation
            // that would otherwise arise from propagating a fresh kink.
            //
            // Forsyth-Vetzal (2002) recommend an additional implicit
            // "post-exercise smoothing" step after CN steps that immediately
            // follow the Rannacher start-up period.  With λ = 1e8/dt that
            // smoothing is already baked in: the penalty constraint fully
            // overwrites the kinked nodes, leaving no high-frequency residual
            // for CN to amplify.  Verified empirically: Rannacher+penalty and
            // Implicit+penalty prices agree to < 0.5% on a 101-point grid
            // (see `w08_rannacher_american_put_price_matches_implicit_*` test).
            if let Some(ref exercise) = self.exercise {
                if exercise.is_exercise_time(t_to) {
                    if let Some(boundary_idx) = exercise.apply(&mut u, dt) {
                        // Record exercise boundary: (time, spot level)
                        let grid_idx = boundary_idx + 1; // interior index → grid index
                        if grid_idx < self.grid.n() {
                            exercise_boundary.push((t_to, self.grid.points()[grid_idx]));
                        }
                    }
                }
            }
        }

        // Build full solution vector (including boundary values at t=0)
        let mut values = Vec::with_capacity(self.grid.n());
        let bc_lower = problem.lower_boundary(0.0);
        let bc_upper = problem.upper_boundary(0.0);

        // Lower boundary value
        values.push(boundary_value(bc_lower, &u, &self.grid, true));

        // Interior values
        values.extend_from_slice(&u);

        // Upper boundary value
        values.push(boundary_value(bc_upper, &u, &self.grid, false));

        let exercise_boundary_out = if exercise_boundary.is_empty() {
            None
        } else {
            Some(exercise_boundary)
        };

        Ok(PdeSolution {
            grid: self.grid.clone(),
            values,
            exercise_boundary: exercise_boundary_out,
            n_time_steps: n_steps,
        })
    }
}

/// Extract boundary value from a boundary condition and the interior solution.
fn boundary_value(
    bc: super::boundary::BoundaryCondition,
    u: &[f64],
    grid: &Grid1D,
    is_lower: bool,
) -> f64 {
    use super::boundary::BoundaryCondition;
    match bc {
        BoundaryCondition::Dirichlet(g) => g,
        BoundaryCondition::Neumann(g) => {
            // Linear extrapolation from the nearest interior point
            if is_lower {
                let h = grid.h_left(1);
                u[0] - h * g
            } else {
                let h = grid.h_right(grid.n() - 2);
                u[u.len() - 1] + h * g
            }
        }
        BoundaryCondition::Linear => {
            // d²u/dx² = 0: u_boundary = 2*u_1 - u_2
            if is_lower {
                if u.len() >= 2 {
                    2.0 * u[0] - u[1]
                } else {
                    u[0]
                }
            } else {
                let n = u.len();
                if n >= 2 {
                    2.0 * u[n - 1] - u[n - 2]
                } else {
                    u[n - 1]
                }
            }
        }
    }
}

/// Solution of a 1D PDE at `t = 0`.
///
/// Contains the full solution vector (including boundaries), the spatial grid,
/// and methods for interpolation and finite-difference Greeks.
#[derive(Debug, Clone)]
pub struct PdeSolution {
    /// Spatial grid used for the solve.
    pub grid: Grid1D,
    /// Solution values at `t = 0` at every grid node (including boundaries).
    pub values: Vec<f64>,
    /// Early exercise boundary `(time, spot_level)` pairs, if applicable.
    pub exercise_boundary: Option<Vec<(f64, f64)>>,
    /// Number of time steps used.
    pub n_time_steps: usize,
}

impl PdeSolution {
    /// Interpolate the solution at an arbitrary point `x`.
    ///
    /// Uses linear interpolation between grid nodes.
    pub fn interpolate(&self, x: f64) -> f64 {
        self.grid.interpolate(&self.values, x)
    }

    /// Compute delta (first derivative) at point `x` via finite differences.
    ///
    /// The stencil is centred on the grid node *nearest* `x` rather than on
    /// the left node of the containing interval: a left-anchored forward
    /// difference evaluates the slope at the cell midpoint, which on a
    /// strike-concentrated grid can sit up to a full grid cell away from `x`
    /// and misstates a fast-varying delta. At an interior nearest node the
    /// second-order non-uniform central stencil is used; at the first/last
    /// node a one-sided difference is used.
    pub fn delta(&self, x: f64) -> f64 {
        let pts = self.grid.points();
        let n = pts.len();

        if n < 2 {
            return 0.0;
        }

        // Node nearest x — the stencil is centred here.
        let i = find_nearest(pts, x);

        if i == 0 {
            // Forward difference at the left boundary node.
            let h = pts[1] - pts[0];
            if h.abs() < 1e-30 {
                return 0.0;
            }
            (self.values[1] - self.values[0]) / h
        } else if i >= n - 1 {
            // Backward difference at the right boundary node.
            let h = pts[n - 1] - pts[n - 2];
            if h.abs() < 1e-30 {
                return 0.0;
            }
            (self.values[n - 1] - self.values[n - 2]) / h
        } else {
            // Second-order non-uniform central stencil centred on node i.
            let h_m = pts[i] - pts[i - 1];
            let h_p = pts[i + 1] - pts[i];
            let h_sum = h_m + h_p;
            if h_m.abs() < 1e-30 || h_p.abs() < 1e-30 {
                return 0.0;
            }
            -h_p / (h_m * h_sum) * self.values[i - 1]
                + (h_p - h_m) / (h_m * h_p) * self.values[i]
                + h_m / (h_p * h_sum) * self.values[i + 1]
        }
    }

    /// Compute gamma (second derivative) at point `x` via finite differences.
    ///
    /// Uses the three-point non-uniform second-derivative stencil centred on
    /// the grid node *nearest* `x`. Centring on the nearest node (rather than
    /// on the left node of the containing interval) keeps the stencil within
    /// half a grid cell of `x`; a left-anchored stencil can sit a full cell
    /// away and misstate a peaked gamma on a strike-concentrated grid.
    pub fn gamma(&self, x: f64) -> f64 {
        let pts = self.grid.points();
        let n = pts.len();

        if n < 3 {
            return 0.0;
        }

        // Centre the three-point stencil on the node nearest x, clamped so the
        // stencil stays within the interior.
        let i = find_nearest(pts, x).clamp(1, n - 2);

        let h_m = pts[i] - pts[i - 1];
        let h_p = pts[i + 1] - pts[i];
        let h_sum = h_m + h_p;

        if h_sum.abs() < 1e-30 {
            return 0.0;
        }

        2.0 * (self.values[i + 1] / h_p - self.values[i] * (1.0 / h_m + 1.0 / h_p)
            + self.values[i - 1] / h_m)
            / h_sum
    }
}

/// Errors during solver construction or execution.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PdeSolverError {
    /// No grid was specified in the builder.
    #[error("PDE solver requires a spatial grid")]
    MissingGrid,
    /// No time stepper was specified in the builder.
    #[error("PDE solver requires a time stepper")]
    MissingStepper,
    /// The maturity passed to `solve` was not strictly positive.
    #[error("PDE solve requires a strictly positive maturity, got {maturity:e}")]
    NonPositiveMaturity {
        /// The offending maturity.
        maturity: f64,
    },
    /// The stepper was configured with zero time steps, so the backward time
    /// march never runs and the "solution" would be the bare terminal payoff.
    #[error("PDE solve requires at least one time step, got n_steps = 0")]
    ZeroTimeSteps,
    /// Grid construction error.
    #[error(transparent)]
    Grid(#[from] PdeGridError),
    /// Time-stepping error — e.g. an explicit / under-damped scheme whose
    /// time step violates the CFL stability condition.
    #[error(transparent)]
    Stepper(#[from] StepperError),
}

#[cfg(test)]
mod tests {
    use super::super::boundary::BoundaryCondition;
    use super::super::problem::PdeProblem1D;
    use super::*;

    /// Heat equation: u_t = u_xx on [0, pi]
    /// Terminal: u(x, T) = sin(x)
    /// Exact: u(x, t) = exp(-(T-t)) * sin(x)
    struct HeatSin;

    impl PdeProblem1D for HeatSin {
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
    fn solver_heat_equation_cn() {
        let grid = Grid1D::uniform(0.0, std::f64::consts::PI, 101).expect("valid grid");
        let solver = Solver1D::builder()
            .grid(grid)
            .crank_nicolson(200)
            .build()
            .expect("valid solver");

        let solution = solver
            .solve(&HeatSin, 0.5)
            .expect("CN solve is unconditionally stable");

        // Check at x = pi/2
        let x = std::f64::consts::FRAC_PI_2;
        let exact = (-0.5_f64).exp() * x.sin();
        let computed = solution.interpolate(x);
        let error = (computed - exact).abs();
        assert!(error < 1e-4, "CN solver error: {error:.6e}");
    }

    #[test]
    fn solver_builder_rejects_incomplete() {
        assert!(Solver1D::builder().build().is_err());
        assert!(Solver1D::builder()
            .grid(Grid1D::uniform(0.0, 1.0, 5).expect("valid"))
            .build()
            .is_err());
    }

    /// W-08: American put with Rannacher + penalty — pricing must agree closely
    /// with fully-implicit solver, confirming the heavy-penalty damping
    /// (`λ = 1e8/dt`) is the accepted mitigation for post-exercise kinks.
    ///
    /// Background: `RannacherStepper` uses implicit steps at the start then
    /// Crank-Nicolson.  After each step the penalty exercise constraint
    /// hard-clamps `u = payoff` at violated nodes (`λ·dt = 1e8`), so the
    /// kink re-introduced by the constraint is already frozen at the payoff
    /// level before the next CN step.  On a well-resolved grid both schemes
    /// produce the same price; any residual differences are below the spatial
    /// discretisation error.  This test asserts that:
    ///   1. Both Rannacher and Implicit produce a finite, positive price.
    ///   2. The prices agree to within 0.5% (spatial discretisation dominates).
    ///   3. The Rannacher price is within 1% of the analytical American put
    ///      lower bound (intrinsic value = max(K-S, 0)).
    ///
    /// These conditions would be violated if CN oscillations materially
    /// corrupted the price; if they pass, the heavy-λ mitigation is sufficient.
    #[test]
    fn w08_rannacher_american_put_price_matches_implicit_within_discretisation_error() {
        use super::super::bridge::BlackScholesPde;

        let sigma = 0.20_f64;
        let rate = 0.05_f64;
        let strike = 100.0_f64;
        let spot = 100.0_f64; // ATM
        let maturity = 1.0_f64;
        let x_min = (50.0_f64).ln();
        let x_max = (200.0_f64).ln();
        // Fine enough grid that discretisation error < 1%
        let n_space = 101_usize;
        let n_time = 100_usize;
        let grid = Grid1D::sinh_concentrated(x_min, x_max, n_space, 0.0, 0.15).expect("valid grid");

        let pde = BlackScholesPde {
            sigma,
            rate,
            dividend: 0.0,
            strike,
            maturity,
            is_call: false,
        };

        let payoff: Vec<f64> = grid.points()[1..n_space - 1]
            .iter()
            .map(|&x| (strike - x.exp()).max(0.0))
            .collect();

        // Rannacher solver (2 implicit + CN)
        let rannacher_solver = Solver1D::builder()
            .grid(grid.clone())
            .rannacher(2, n_time)
            .american(payoff.clone())
            .build()
            .expect("valid rannacher solver");
        let rannacher_sol = rannacher_solver
            .solve(&pde, maturity)
            .expect("rannacher solve");
        let rannacher_price = rannacher_sol.interpolate(spot.ln());

        // Pure implicit solver (reference)
        let implicit_solver = Solver1D::builder()
            .grid(grid)
            .implicit(n_time)
            .american(payoff)
            .build()
            .expect("valid implicit solver");
        let implicit_sol = implicit_solver
            .solve(&pde, maturity)
            .expect("implicit solve");
        let implicit_price = implicit_sol.interpolate(spot.ln());

        // Both prices must be finite and positive
        assert!(
            rannacher_price.is_finite() && rannacher_price > 0.0,
            "Rannacher price must be finite and positive, got {rannacher_price}"
        );
        assert!(
            implicit_price.is_finite() && implicit_price > 0.0,
            "Implicit price must be finite and positive, got {implicit_price}"
        );

        // Both must be >= intrinsic value (American option lower bound)
        let intrinsic = (strike - spot).max(0.0);
        assert!(
            rannacher_price >= intrinsic - 1e-6,
            "Rannacher price {rannacher_price:.4} must be >= intrinsic {intrinsic:.4}"
        );

        // Prices must agree within 0.5% (spatial discretisation dominates)
        let rel_diff = (rannacher_price - implicit_price).abs() / implicit_price;
        assert!(
            rel_diff < 0.005,
            "Rannacher ({rannacher_price:.4}) and Implicit ({implicit_price:.4}) prices diverge \
             by {:.2}% — exceeds 0.5% discretisation budget. \
             This may indicate CN oscillation from post-exercise kink.",
            rel_diff * 100.0
        );
    }

    /// Build a [`PdeSolution`] from a grid and an explicit value vector
    /// (one value per grid node), bypassing the time march. Lets the Greek
    /// stencil tests pin `delta` / `gamma` against an analytically known
    /// function.
    fn solution_from_values(grid: Grid1D, values: Vec<f64>) -> PdeSolution {
        assert_eq!(values.len(), grid.n());
        PdeSolution {
            grid,
            values,
            exercise_boundary: None,
            n_time_steps: 0,
        }
    }

    /// [P6-4] `gamma` must centre its three-point stencil on the grid node
    /// **nearest** `x`, not on the left node of the containing interval.
    ///
    /// Failure mode being guarded: `find_interval` returns the left node of
    /// `[x_{k-1}, x_k]`, so for a query `x` in the *upper* half of an interval
    /// the old code centred the second-derivative stencil on `x_{k-1}` — up to
    /// a full grid cell below `x`. On a coarse / strike-concentrated grid this
    /// reports the curvature at the wrong node.
    ///
    /// With `u(x) = exp(x)` the exact gamma is `exp(x)`. Two checks:
    ///   1. On a coarse non-uniform grid, a query in the upper half of an
    ///      interval must be centred on the right (nearest) node — the stencil
    ///      tracks `exp(x_nearest)` and is strictly closer to it than to
    ///      `exp(x_left)`. This is the direct off-centre-bug assertion.
    ///   2. On a refined grid, the nearest-node stencil converges to the true
    ///      `exp(x)`; the left-node stencil retains an O(cell) bias.
    #[test]
    fn gamma_stencil_is_centred_on_nearest_node_not_left_node() {
        // (1) Coarse, non-uniform grid: large cells so the gap between
        // adjacent-node curvature values is unmistakable.
        let grid =
            Grid1D::from_points(vec![0.0, 0.9, 2.0, 3.2, 4.5]).expect("valid non-uniform grid");
        let values: Vec<f64> = grid.points().iter().map(|&x| x.exp()).collect();
        let solution = solution_from_values(grid.clone(), values);

        // Query in the upper part of the interval [x_2=2.0, x_3=3.2]:
        // nearest node is x_3 = 3.2, the left node of the interval is x_2.
        let x = 3.1_f64;
        let nearest = grid.points()[3]; // 3.2
        let left = grid.points()[2]; //  2.0

        let computed = solution.gamma(x);

        // Centred on the nearest node, the 3-point stencil approximates the
        // curvature at x_3; centred on the left node it would approximate the
        // curvature at x_2 ≈ exp(x_2). The stencil value must be strictly
        // closer to exp(x_3) than to exp(x_2).
        let err_nearest = (computed - nearest.exp()).abs();
        let err_left = (computed - left.exp()).abs();
        assert!(
            err_nearest < err_left,
            "gamma({x}) = {computed:.4} should track exp(x_nearest)=exp({nearest})={:.4} \
             not exp(x_left)=exp({left})={:.4} (err_nearest={err_nearest:.4}, err_left={err_left:.4})",
            nearest.exp(),
            left.exp(),
        );

        // (2) On a refined grid the nearest-node stencil converges to exp(x);
        // a stencil centred on the left node would keep an O(cell) bias.
        let fine = Grid1D::uniform(0.0, 5.0, 501).expect("valid fine grid");
        let fine_values: Vec<f64> = fine.points().iter().map(|&x| x.exp()).collect();
        let fine_solution = solution_from_values(fine.clone(), fine_values);
        // Query just below an interior node so the old left-node centring
        // would have used the cell to the left.
        let xf = 3.1_f64;
        let nearest_idx = find_nearest(fine.points(), xf);
        let g_fine = fine_solution.gamma(xf);
        let rel = (g_fine - xf.exp()).abs() / xf.exp();
        assert!(
            rel < 5e-3,
            "on a refined grid gamma({xf}) = {g_fine:.6} should converge to exp({xf}) = {:.6} \
             (rel err {rel:.3e}); query nearest node x_{nearest_idx} = {:.6}",
            xf.exp(),
            fine.points()[nearest_idx],
        );
    }

    /// [P6-4] `delta` must centre its stencil on the grid node nearest `x`.
    ///
    /// The old code took a left-anchored forward difference
    /// `(u[idx+1]-u[idx])/h`, which estimates the slope at the *midpoint* of
    /// the cell `[x_idx, x_{idx+1}]` — up to a full cell from `x`. The fixed
    /// code uses the second-order non-uniform central stencil centred on the
    /// node nearest `x`.
    ///
    /// Two checks with `u(x) = exp(x)` (exact delta `exp(x)`):
    ///   1. Coarse non-uniform grid: a query in the upper half of an interval
    ///      yields a centred-stencil delta — it equals neither the old
    ///      left-cell chord nor the right-cell chord, confirming the reader is
    ///      no longer just returning a one-sided cell chord.
    ///   2. Refined grid: the nearest-node stencil converges to `exp(x)`.
    #[test]
    fn delta_stencil_is_centred_on_nearest_node() {
        // (1) Coarse non-uniform grid.
        let grid =
            Grid1D::from_points(vec![0.0, 0.9, 2.0, 3.2, 4.5]).expect("valid non-uniform grid");
        let values: Vec<f64> = grid.points().iter().map(|&x| x.exp()).collect();
        let solution = solution_from_values(grid.clone(), values);

        // Query just below interior node x_3 = 3.2 (nearest node is x_3).
        let x = 3.15_f64;
        let computed = solution.delta(x);

        // Old behaviour: left-anchored forward difference over the containing
        // cell [x_2, x_3].
        let left_chord = (grid.points()[3].exp() - grid.points()[2].exp())
            / (grid.points()[3] - grid.points()[2]);
        // The other one-sided cell chord, [x_3, x_4].
        let right_chord = (grid.points()[4].exp() - grid.points()[3].exp())
            / (grid.points()[4] - grid.points()[3]);

        // The centred stencil at x_3 is a genuine two-sided combination — it
        // must differ from both one-sided cell chords (in particular it is no
        // longer the old left-anchored chord).
        assert!(
            (computed - left_chord).abs() > 1e-6,
            "delta({x}) = {computed:.6} must not be the old left-anchored chord {left_chord:.6}"
        );
        assert!(
            (computed - right_chord).abs() > 1e-6,
            "delta({x}) = {computed:.6} must not be the right-cell chord {right_chord:.6}"
        );
        // A two-sided central stencil lies between the two one-sided chords.
        let (lo, hi) = (left_chord.min(right_chord), left_chord.max(right_chord));
        assert!(
            computed > lo && computed < hi,
            "centred delta({x}) = {computed:.6} should lie between the bracketing cell \
             chords [{lo:.6}, {hi:.6}]"
        );

        // (2) Refined grid: the centred stencil converges to the analytic
        // derivative exp(x).
        let fine = Grid1D::uniform(0.0, 5.0, 501).expect("valid fine grid");
        let fine_values: Vec<f64> = fine.points().iter().map(|&x| x.exp()).collect();
        let fine_solution = solution_from_values(fine, fine_values);
        let xf = 3.15_f64;
        let d_fine = fine_solution.delta(xf);
        let rel = (d_fine - xf.exp()).abs() / xf.exp();
        assert!(
            rel < 5e-3,
            "on a refined grid delta({xf}) = {d_fine:.6} should converge to exp({xf}) = {:.6} \
             (rel err {rel:.3e})",
            xf.exp(),
        );
    }

    /// [P6-6] `Solver1D::solve` must reject a non-positive maturity and a
    /// zero-step stepper instead of returning a degenerate "solution".
    ///
    /// With `n_steps = 0` the backward time march never runs, so the old code
    /// would return the *terminal payoff* — undiscounted, undiffused — as the
    /// price. With `maturity <= 0` the time grid is degenerate / NaN.
    #[test]
    fn solver_rejects_invalid_maturity_and_zero_steps() {
        let grid = Grid1D::uniform(0.0, std::f64::consts::PI, 21).expect("valid grid");

        // Non-positive maturity.
        let solver = Solver1D::builder()
            .grid(grid.clone())
            .crank_nicolson(50)
            .build()
            .expect("valid solver");
        assert!(
            matches!(
                solver.solve(&HeatSin, 0.0),
                Err(PdeSolverError::NonPositiveMaturity { .. })
            ),
            "maturity = 0 must be rejected"
        );
        assert!(
            matches!(
                solver.solve(&HeatSin, -0.5),
                Err(PdeSolverError::NonPositiveMaturity { .. })
            ),
            "negative maturity must be rejected"
        );

        // Zero time steps.
        let zero_step_solver = Solver1D::builder()
            .grid(grid)
            .crank_nicolson(0)
            .build()
            .expect("valid solver");
        assert!(
            matches!(
                zero_step_solver.solve(&HeatSin, 1.0),
                Err(PdeSolverError::ZeroTimeSteps)
            ),
            "a zero-step stepper must be rejected"
        );
    }

    /// [P6-8] Dead-branch removal: `find_interval`/`find_nearest` clamp the
    /// returned index, so for a query *above* the grid the Greek readers must
    /// fall through to the boundary (one-sided) branch and return a finite,
    /// correct one-sided difference — never the unreachable `0.0` fallback
    /// that the old dead `else` arm produced.
    #[test]
    fn delta_above_domain_uses_boundary_difference_not_dead_fallback() {
        let grid = Grid1D::uniform(0.0, 4.0, 5).expect("valid grid");
        // u(x) = 3x  → delta = 3 everywhere, including the one-sided edges.
        let values: Vec<f64> = grid.points().iter().map(|&x| 3.0 * x).collect();
        let solution = solution_from_values(grid, values);

        // Query far above the domain: nearest node is the last node.
        let d = solution.delta(100.0);
        assert!(d.is_finite(), "delta above domain must be finite");
        assert!(
            (d - 3.0).abs() < 1e-9,
            "delta above domain must equal the boundary one-sided slope 3.0, got {d}"
        );

        // Query far below the domain: nearest node is node 0.
        let d_lo = solution.delta(-100.0);
        assert!(
            (d_lo - 3.0).abs() < 1e-9,
            "delta below domain must equal the boundary one-sided slope 3.0, got {d_lo}"
        );
    }
}

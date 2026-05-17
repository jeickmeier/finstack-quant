//! Top-level 2D PDE solver with builder pattern.
//!
//! Combines a [`Grid2D`], a [`CraigSneydStepper`], and an optional exercise
//! constraint to solve a backward 2D PDE from terminal condition to `t = 0`.
//! Returns a [`PdeSolution2D`] with bilinear interpolation and finite-difference
//! Greeks.

use super::adi::{fill_boundaries, AdiWorkBuffers, CraigSneydStepper};
use super::grid::find_nearest;
use super::grid2d::Grid2D;
use super::problem2d::PdeProblem2D;
use super::stepper::StepperError;

/// Builder for constructing a [`Solver2D`].
///
/// # Examples
///
/// ```rust,ignore
/// let solver = Solver2DBuilder::new()
///     .grid(Grid2D::new(x_grid, v_grid))
///     .craig_sneyd(200)
///     .build()?;
/// ```
pub struct Solver2DBuilder {
    /// Tensor-product grid.
    grid: Option<Grid2D>,
    /// ADI stepper.
    stepper: Option<CraigSneydStepper>,
}

impl Default for Solver2DBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Solver2DBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            grid: None,
            stepper: None,
        }
    }

    /// Set the 2D grid.
    pub fn grid(mut self, grid: Grid2D) -> Self {
        self.grid = Some(grid);
        self
    }

    /// Use Modified Craig-Sneyd (MCS) ADI with `n_steps` time steps.
    pub fn craig_sneyd(mut self, n_steps: usize) -> Self {
        self.stepper = Some(CraigSneydStepper::new(n_steps));
        self
    }

    /// Use Modified Craig-Sneyd (MCS) ADI with Rannacher-style smoothing.
    pub fn craig_sneyd_rannacher(mut self, implicit_start: usize, n_steps: usize) -> Self {
        self.stepper = Some(CraigSneydStepper::with_rannacher(implicit_start, n_steps));
        self
    }

    /// Build the solver.
    pub fn build(self) -> Result<Solver2D, PdeSolver2DError> {
        let grid = self.grid.ok_or(PdeSolver2DError::MissingGrid)?;
        let stepper = self.stepper.ok_or(PdeSolver2DError::MissingStepper)?;
        Ok(Solver2D { grid, stepper })
    }
}

/// Two-dimensional PDE solver using Modified Craig-Sneyd (MCS) ADI splitting.
pub struct Solver2D {
    /// Tensor-product grid.
    grid: Grid2D,
    /// ADI time-stepper.
    stepper: CraigSneydStepper,
}

impl Solver2D {
    /// Create a solver builder.
    pub fn builder() -> Solver2DBuilder {
        Solver2DBuilder::new()
    }

    /// Solve the 2D PDE problem and return the solution at `t = 0`.
    ///
    /// # Errors
    ///
    /// - [`PdeSolver2DError::NonPositiveMaturity`] if `maturity <= 0`.
    /// - [`PdeSolver2DError::ZeroTimeSteps`] if the stepper has no time steps
    ///   (the backward march would never run).
    /// - [`PdeSolver2DError::Stepper`] if a Modified Craig-Sneyd ADI step
    ///   fails — a convection-dominated grid outside the scheme's reliably
    ///   stable regime, or a degenerate tridiagonal solve. The previous
    ///   implementation could diverge silently to inf / NaN in these cases.
    pub fn solve(
        &self,
        problem: &dyn PdeProblem2D,
        maturity: f64,
    ) -> Result<PdeSolution2D, PdeSolver2DError> {
        // Validate maturity / step count up front — a non-positive or
        // non-finite maturity, or zero steps, would otherwise yield a
        // degenerate "solution" (NaN time grid, or the bare terminal payoff
        // with no time evolution). The `is_finite` check also rejects NaN.
        if !maturity.is_finite() || maturity <= 0.0 {
            return Err(PdeSolver2DError::NonPositiveMaturity { maturity });
        }
        let n_steps = self.stepper.n_steps();
        if n_steps == 0 {
            return Err(PdeSolver2DError::ZeroTimeSteps);
        }

        let nx = self.grid.nx();
        let ny = self.grid.ny();
        let nx_int = self.grid.nx_interior();
        let ny_int = self.grid.ny_interior();

        // Initialize full grid with terminal condition
        let mut u_full = vec![0.0; nx * ny];
        for i in 0..nx {
            for j in 0..ny {
                u_full[i * ny + j] = problem
                    .terminal_condition(self.grid.x().points()[i], self.grid.y().points()[j]);
            }
        }

        // Extract interior
        let mut u_int = vec![0.0; nx_int * ny_int];
        for ii in 0..nx_int {
            for jj in 0..ny_int {
                u_int[ii * ny_int + jj] = u_full[(ii + 1) * ny + (jj + 1)];
            }
        }

        // Time levels: T → 0
        let levels = self.stepper.time_levels(maturity);

        // Pre-allocate ADI scratch buffers once per solve and reuse across all
        // timesteps. The previous implementation allocated six Vecs per step,
        // costing O(n_steps) allocations on hot pricing paths.
        let mut buffers = AdiWorkBuffers::for_grid(&self.grid);

        for step in 0..n_steps {
            self.stepper.step_with_buffers(
                problem,
                &self.grid,
                &mut u_full,
                &mut u_int,
                levels[step],
                levels[step + 1],
                step,
                &mut buffers,
            )?;
        }

        // Final boundary fill
        fill_boundaries(problem, &self.grid, &mut u_full, &u_int, 0.0);

        Ok(PdeSolution2D {
            grid: self.grid.clone(),
            values: u_full,
            n_time_steps: n_steps,
        })
    }
}

/// Solution of a 2D PDE at `t = 0`.
///
/// Contains the full solution on the tensor-product grid (including boundaries)
/// and methods for interpolation and finite-difference Greeks.
#[derive(Debug, Clone)]
pub struct PdeSolution2D {
    /// Tensor-product grid.
    pub grid: Grid2D,
    /// Solution values at `t = 0` in row-major layout (`nx * ny`).
    pub values: Vec<f64>,
    /// Number of time steps used.
    pub n_time_steps: usize,
}

impl PdeSolution2D {
    /// Bilinear interpolation of the solution at `(x, y)`.
    pub fn interpolate(&self, x: f64, y: f64) -> f64 {
        self.grid.interpolate(&self.values, x, y)
    }

    /// Delta with respect to x (first derivative) at `(x, y)`.
    ///
    /// Computed via finite differences on the x-axis at the nearest y-grid
    /// level. The x-stencil is centred on the node *nearest* `x` rather than
    /// on the left node of the containing interval: a left-anchored forward
    /// difference evaluates the slope at the cell midpoint, which on a
    /// strike-concentrated grid can sit a full grid cell away from `x`.
    pub fn delta_x(&self, x: f64, y: f64) -> f64 {
        let ny = self.grid.ny();
        let x_pts = self.grid.x().points();
        let y_pts = self.grid.y().points();
        let n = x_pts.len();

        if n < 2 {
            return 0.0;
        }

        // Find nearest y-level
        let j = find_nearest(y_pts, y);

        // Node nearest x — the x-stencil is centred here.
        let i = find_nearest(x_pts, x);

        if i == 0 {
            let h = x_pts[1] - x_pts[0];
            if h.abs() < 1e-30 {
                return 0.0;
            }
            (self.values[ny + j] - self.values[j]) / h
        } else if i >= n - 1 {
            let h = x_pts[n - 1] - x_pts[n - 2];
            if h.abs() < 1e-30 {
                return 0.0;
            }
            (self.values[(n - 1) * ny + j] - self.values[(n - 2) * ny + j]) / h
        } else {
            // Second-order non-uniform central stencil centred on node i.
            let h_m = x_pts[i] - x_pts[i - 1];
            let h_p = x_pts[i + 1] - x_pts[i];
            let h_sum = h_m + h_p;
            if h_m.abs() < 1e-30 || h_p.abs() < 1e-30 {
                return 0.0;
            }
            -h_p / (h_m * h_sum) * self.values[(i - 1) * ny + j]
                + (h_p - h_m) / (h_m * h_p) * self.values[i * ny + j]
                + h_m / (h_p * h_sum) * self.values[(i + 1) * ny + j]
        }
    }

    /// Gamma with respect to x (second derivative) at `(x, y)`.
    ///
    /// Uses the three-point non-uniform second-derivative stencil centred on
    /// the x-grid node *nearest* `x` (rather than the left node of the
    /// containing interval) so the stencil stays within half a grid cell of
    /// `x`, avoiding a misstated peaked gamma on a strike-concentrated grid.
    pub fn gamma_x(&self, x: f64, y: f64) -> f64 {
        let ny = self.grid.ny();
        let x_pts = self.grid.x().points();
        let y_pts = self.grid.y().points();
        let n = x_pts.len();

        if n < 3 {
            return 0.0;
        }

        let j = find_nearest(y_pts, y);
        // Centre the three-point stencil on the node nearest x.
        let i = find_nearest(x_pts, x).clamp(1, n - 2);

        let h_m = x_pts[i] - x_pts[i - 1];
        let h_p = x_pts[i + 1] - x_pts[i];
        let h_sum = h_m + h_p;

        if h_sum.abs() < 1e-30 {
            return 0.0;
        }

        2.0 * (self.values[(i + 1) * ny + j] / h_p
            - self.values[i * ny + j] * (1.0 / h_m + 1.0 / h_p)
            + self.values[(i - 1) * ny + j] / h_m)
            / h_sum
    }
}

/// Errors during 2D solver construction or execution.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PdeSolver2DError {
    /// No grid was specified.
    #[error("2D PDE solver requires a grid")]
    MissingGrid,
    /// No stepper was specified.
    #[error("2D PDE solver requires a time stepper")]
    MissingStepper,
    /// The maturity passed to `solve` was not strictly positive.
    #[error("2D PDE solve requires a strictly positive maturity, got {maturity:e}")]
    NonPositiveMaturity {
        /// The offending maturity.
        maturity: f64,
    },
    /// The stepper was configured with zero time steps, so the backward time
    /// march never runs and the "solution" would be the bare terminal payoff.
    #[error("2D PDE solve requires at least one time step, got n_steps = 0")]
    ZeroTimeSteps,
    /// The ADI time-stepper failed — e.g. a convection-dominated grid outside
    /// the Modified Craig-Sneyd stable regime, or a degenerate tridiagonal
    /// solve.
    #[error(transparent)]
    Stepper(#[from] StepperError),
}

#[cfg(test)]
mod tests {
    use super::super::boundary::BoundaryCondition;
    use super::super::grid::Grid1D;
    use super::super::problem2d::PdeProblem2D;
    use super::*;

    /// 2D heat equation on [0,pi]^2 with Dirichlet BCs.
    struct Heat2D;

    impl PdeProblem2D for Heat2D {
        fn diffusion_xx(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            1.0
        }
        fn diffusion_yy(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            1.0
        }
        fn mixed_diffusion(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            0.0
        }
        fn convection_x(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            0.0
        }
        fn convection_y(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            0.0
        }
        fn reaction(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            0.0
        }
        fn terminal_condition(&self, x: f64, y: f64) -> f64 {
            x.sin() * y.sin()
        }
        fn boundary_x_lower(&self, _y: f64, _t: f64) -> BoundaryCondition {
            BoundaryCondition::Dirichlet(0.0)
        }
        fn boundary_x_upper(&self, _y: f64, _t: f64) -> BoundaryCondition {
            BoundaryCondition::Dirichlet(0.0)
        }
        fn boundary_y_lower(&self, _x: f64, _t: f64) -> BoundaryCondition {
            BoundaryCondition::Dirichlet(0.0)
        }
        fn boundary_y_upper(&self, _x: f64, _t: f64) -> BoundaryCondition {
            BoundaryCondition::Dirichlet(0.0)
        }
        fn is_time_homogeneous(&self) -> bool {
            true
        }
    }

    #[test]
    fn solver2d_heat_equation() {
        let pi = std::f64::consts::PI;
        let t_mat = 0.25;

        let gx = Grid1D::uniform(0.0, pi, 41).expect("valid");
        let gy = Grid1D::uniform(0.0, pi, 41).expect("valid");
        let grid = Grid2D::new(gx, gy);

        let solver = Solver2D::builder()
            .grid(grid)
            .craig_sneyd(200)
            .build()
            .expect("valid solver");

        let solution = solver
            .solve(&Heat2D, t_mat)
            .expect("pure-diffusion 2D heat solve is stable");
        let exact = (-2.0 * t_mat).exp();
        let computed = solution.interpolate(pi / 2.0, pi / 2.0);
        let error = (computed - exact).abs();
        assert!(
            error < 0.01,
            "Solver2D heat error = {error:.6e}, exact={exact:.6}, computed={computed:.6}"
        );
    }

    #[test]
    fn solver2d_builder_rejects_incomplete() {
        assert!(Solver2D::builder().build().is_err());
        let gx = Grid1D::uniform(0.0, 1.0, 5).expect("valid");
        let gy = Grid1D::uniform(0.0, 1.0, 5).expect("valid");
        assert!(Solver2D::builder()
            .grid(Grid2D::new(gx, gy))
            .build()
            .is_err());
    }

    /// Build a [`PdeSolution2D`] from a grid and an explicit value vector,
    /// bypassing the time march, so the x-Greek stencils can be pinned
    /// against an analytically known function.
    fn solution2d_from_values(grid: Grid2D, values: Vec<f64>) -> PdeSolution2D {
        assert_eq!(values.len(), grid.total());
        PdeSolution2D {
            grid,
            values,
            n_time_steps: 0,
        }
    }

    /// A strongly convection-dominated 2D problem (small diffusion, large
    /// convection) — used to exercise the MCS Péclet stability guard.
    struct ConvectionDominated2D;

    impl PdeProblem2D for ConvectionDominated2D {
        fn diffusion_xx(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            0.01
        }
        fn diffusion_yy(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            0.01
        }
        fn mixed_diffusion(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            0.0
        }
        fn convection_x(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            5.0
        }
        fn convection_y(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            5.0
        }
        fn reaction(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            0.0
        }
        fn terminal_condition(&self, x: f64, y: f64) -> f64 {
            x.sin() * y.sin()
        }
        fn boundary_x_lower(&self, _y: f64, _t: f64) -> BoundaryCondition {
            BoundaryCondition::Dirichlet(0.0)
        }
        fn boundary_x_upper(&self, _y: f64, _t: f64) -> BoundaryCondition {
            BoundaryCondition::Dirichlet(0.0)
        }
        fn boundary_y_lower(&self, _x: f64, _t: f64) -> BoundaryCondition {
            BoundaryCondition::Dirichlet(0.0)
        }
        fn boundary_y_upper(&self, _x: f64, _t: f64) -> BoundaryCondition {
            BoundaryCondition::Dirichlet(0.0)
        }
        fn is_time_homogeneous(&self) -> bool {
            true
        }
    }

    /// [P6-1] `Solver2D::solve` must propagate the MCS Péclet stability
    /// failure as `PdeSolver2DError::Stepper(StepperError::PecletViolation)`
    /// rather than returning a silently-divergent solution.
    #[test]
    fn solver2d_solve_propagates_peclet_violation() {
        let pi = std::f64::consts::PI;
        let gx = Grid1D::uniform(0.0, pi, 41).expect("valid grid");
        let gy = Grid1D::uniform(0.0, pi, 41).expect("valid grid");
        let solver = Solver2D::builder()
            .grid(Grid2D::new(gx, gy))
            .craig_sneyd(100)
            .build()
            .expect("valid solver");

        let result = solver.solve(&ConvectionDominated2D, 0.25);
        assert!(
            matches!(
                result,
                Err(PdeSolver2DError::Stepper(StepperError::PecletViolation { .. }))
            ),
            "a convection-dominated 2D solve must surface a PecletViolation, got {result:?}"
        );
    }

    /// [P6-6] `Solver2D::solve` must reject a non-positive maturity and a
    /// zero-step stepper instead of returning a degenerate "solution".
    #[test]
    fn solver2d_solve_rejects_invalid_maturity_and_zero_steps() {
        let pi = std::f64::consts::PI;
        let gx = Grid1D::uniform(0.0, pi, 21).expect("valid grid");
        let gy = Grid1D::uniform(0.0, pi, 21).expect("valid grid");

        // Non-positive maturity.
        let solver = Solver2D::builder()
            .grid(Grid2D::new(gx.clone(), gy.clone()))
            .craig_sneyd(50)
            .build()
            .expect("valid solver");
        assert!(
            matches!(
                solver.solve(&Heat2D, 0.0),
                Err(PdeSolver2DError::NonPositiveMaturity { .. })
            ),
            "maturity = 0 must be rejected"
        );
        assert!(
            matches!(
                solver.solve(&Heat2D, -1.0),
                Err(PdeSolver2DError::NonPositiveMaturity { .. })
            ),
            "negative maturity must be rejected"
        );

        // Zero time steps.
        let zero_step_solver = Solver2D::builder()
            .grid(Grid2D::new(gx, gy))
            .craig_sneyd(0)
            .build()
            .expect("valid solver");
        assert!(
            matches!(
                zero_step_solver.solve(&Heat2D, 1.0),
                Err(PdeSolver2DError::ZeroTimeSteps)
            ),
            "a zero-step stepper must be rejected"
        );
    }

    /// [P6-4] `gamma_x` / `delta_x` must centre the x-stencil on the x-grid
    /// node **nearest** `x`, not on the left node of the containing interval.
    ///
    /// Same failure mode as the 1D `gamma` test: `find_interval` returned the
    /// left node of `[x_{k-1}, x_k]`, placing the stencil up to a full grid
    /// cell below a query in the upper half of an interval. With
    /// `u(x, y) = exp(x)` (independent of y) the exact x-gamma and x-delta are
    /// both `exp(x)`.
    ///
    /// Two checks:
    ///   1. Coarse non-uniform grid: a query in the upper half of an interval
    ///      must be centred on the right (nearest) node — `gamma_x` is closer
    ///      to the curvature at `x_nearest` than at `x_left`.
    ///   2. Refined grid: the nearest-node `gamma_x` / `delta_x` converge to
    ///      `exp(x)`; a left-node-centred stencil keeps an O(cell) bias.
    #[test]
    fn gamma_x_and_delta_x_centre_stencil_on_nearest_node() {
        // (1) Coarse, non-uniform x-grid; small uniform y-grid.
        let gx =
            Grid1D::from_points(vec![0.0, 0.9, 2.0, 3.2, 4.5]).expect("valid non-uniform x-grid");
        let gy = Grid1D::uniform(0.0, 1.0, 3).expect("valid y-grid");
        let grid = Grid2D::new(gx.clone(), gy);

        let nx = grid.nx();
        let ny = grid.ny();
        let mut values = vec![0.0; nx * ny];
        for i in 0..nx {
            for j in 0..ny {
                // u depends on x only: u = exp(x).
                values[i * ny + j] = grid.x().points()[i].exp();
            }
        }
        let solution = solution2d_from_values(grid.clone(), values);

        // Query in the upper part of [x_2 = 2.0, x_3 = 3.2]: nearest node x_3.
        let x = 3.1_f64;
        let y = 0.5_f64;
        let nearest = gx.points()[3]; // 3.2
        let left = gx.points()[2]; // 2.0

        let gamma = solution.gamma_x(x, y);
        let err_nearest = (gamma - nearest.exp()).abs();
        let err_left = (gamma - left.exp()).abs();
        assert!(
            err_nearest < err_left,
            "gamma_x({x}) = {gamma:.4} should track the curvature at x_nearest=({nearest}) \
             ≈ {:.4}, not at x_left=({left}) ≈ {:.4}",
            nearest.exp(),
            left.exp(),
        );

        // (2) Refined uniform x-grid: the nearest-node stencils converge to
        // the analytic exp(x); a left-node-centred stencil would not.
        let gx_fine = Grid1D::uniform(0.0, 5.0, 401).expect("valid fine x-grid");
        let gy2 = Grid1D::uniform(0.0, 1.0, 3).expect("valid y-grid");
        let grid_fine = Grid2D::new(gx_fine.clone(), gy2);
        let nxf = grid_fine.nx();
        let nyf = grid_fine.ny();
        let mut vals_fine = vec![0.0; nxf * nyf];
        for i in 0..nxf {
            for j in 0..nyf {
                vals_fine[i * nyf + j] = grid_fine.x().points()[i].exp();
            }
        }
        let sol_fine = solution2d_from_values(grid_fine, vals_fine);

        let xf = 3.1_f64;
        let g_fine = sol_fine.gamma_x(xf, 0.5);
        let d_fine = sol_fine.delta_x(xf, 0.5);
        let g_rel = (g_fine - xf.exp()).abs() / xf.exp();
        let d_rel = (d_fine - xf.exp()).abs() / xf.exp();
        assert!(
            g_rel < 5e-3,
            "refined gamma_x({xf}) = {g_fine:.6} should converge to exp({xf}) = {:.6} \
             (rel err {g_rel:.3e})",
            xf.exp(),
        );
        assert!(
            d_rel < 5e-3,
            "refined delta_x({xf}) = {d_fine:.6} should converge to exp({xf}) = {:.6} \
             (rel err {d_rel:.3e})",
            xf.exp(),
        );
    }
}

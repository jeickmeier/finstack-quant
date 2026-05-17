//! Top-level 2D PDE solver with builder pattern.
//!
//! Combines a [`Grid2D`], a [`CraigSneydStepper`], and an optional exercise
//! constraint to solve a backward 2D PDE from terminal condition to `t = 0`.
//! Returns a [`PdeSolution2D`] with bilinear interpolation and finite-difference
//! Greeks.

use super::adi::{fill_boundaries, AdiWorkBuffers, CraigSneydStepper};
use super::grid::{find_interval, find_nearest};
use super::grid2d::Grid2D;
use super::problem2d::PdeProblem2D;

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
    pub fn solve(&self, problem: &dyn PdeProblem2D, maturity: f64) -> PdeSolution2D {
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
        let n_steps = self.stepper.n_steps();

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
            );
        }

        // Final boundary fill
        fill_boundaries(problem, &self.grid, &mut u_full, &u_int, 0.0);

        PdeSolution2D {
            grid: self.grid.clone(),
            values: u_full,
            n_time_steps: n_steps,
        }
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
    /// Computed via central finite differences on the x-axis at the
    /// nearest y-grid level.
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

        // Extract the x-line at this y-level
        let idx = find_interval(x_pts, x);

        if idx == 0 {
            let h = x_pts[1] - x_pts[0];
            if h.abs() < 1e-30 {
                return 0.0;
            }
            (self.values[ny + j] - self.values[j]) / h
        } else if idx >= n - 1 {
            let h = x_pts[n - 1] - x_pts[n - 2];
            if h.abs() < 1e-30 {
                return 0.0;
            }
            (self.values[(n - 1) * ny + j] - self.values[(n - 2) * ny + j]) / h
        } else {
            let h = x_pts[idx + 1] - x_pts[idx];
            if h.abs() < 1e-30 {
                return 0.0;
            }
            (self.values[(idx + 1) * ny + j] - self.values[idx * ny + j]) / h
        }
    }

    /// Gamma with respect to x (second derivative) at `(x, y)`.
    pub fn gamma_x(&self, x: f64, y: f64) -> f64 {
        let ny = self.grid.ny();
        let x_pts = self.grid.x().points();
        let y_pts = self.grid.y().points();
        let n = x_pts.len();

        if n < 3 {
            return 0.0;
        }

        let j = find_nearest(y_pts, y);
        let idx = find_interval(x_pts, x);
        let i = idx.clamp(1, n - 2);

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

/// Errors during 2D solver construction.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PdeSolver2DError {
    /// No grid was specified.
    #[error("2D PDE solver requires a grid")]
    MissingGrid,
    /// No stepper was specified.
    #[error("2D PDE solver requires a time stepper")]
    MissingStepper,
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

        let solution = solver.solve(&Heat2D, t_mat);
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
}

//! Two-dimensional ADI operator infrastructure.
//!
//! Provides directional tridiagonal operators for ADI splitting and an
//! explicit cross-derivative operator using the standard four-point stencil
//! on the tensor-product grid.
//!
//! Each ADI sweep applies a 1D tridiagonal solve along one dimension while
//! holding the other dimension fixed. The cross-derivative term is always
//! treated explicitly.

use super::boundary::BoundaryCondition;
use super::grid::Grid1D;
use super::grid2d::Grid2D;
use super::operator::TridiagOperator;
use super::problem2d::PdeProblem2D;

/// Assembled 2D operators for one time level, split by direction.
///
/// `op_x[j]` is the tridiagonal operator along the x-axis at y-level `j`
/// (interior y-index). `op_y[i]` is along the y-axis at x-level `i`.
/// The cross-derivative is stored as a flat vector of explicit contributions.
pub struct Operators2D {
    /// Tridiagonal operators along x, one per interior y-level.
    /// `op_x[j]` has size `nx_interior`.
    pub op_x: Vec<TridiagOperator>,
    /// Tridiagonal operators along y, one per interior x-level.
    /// `op_y[i]` has size `ny_interior`.
    pub op_y: Vec<TridiagOperator>,
    /// Cross-derivative contribution `a_xy * d²u/(dx dy)` evaluated
    /// at each interior point. Length `nx_interior * ny_interior`.
    pub cross_deriv: Vec<f64>,
    /// Number of interior x-points.
    pub nx_int: usize,
    /// Number of interior y-points.
    pub ny_int: usize,
}

impl Operators2D {
    /// Assemble all directional operators and the cross-derivative at time `t`.
    ///
    /// The reaction term `c(x, y, t) u` is split equally: half goes to the
    /// x-direction implicit solve, half to the y-direction. This avoids
    /// double-counting in the ADI splitting.
    pub fn assemble(problem: &dyn PdeProblem2D, grid: &Grid2D, t: f64) -> Self {
        let nx_int = grid.nx_interior();
        let ny_int = grid.ny_interior();
        let x_pts = grid.x().points();
        let y_pts = grid.y().points();

        // Assemble x-direction operators: one tridiag per interior y-level
        let mut op_x = Vec::with_capacity(ny_int);
        for jj in 0..ny_int {
            let j = jj + 1; // grid index
            let y = y_pts[j];
            let bc_lo = problem.boundary_x_lower(y, t);
            let bc_hi = problem.boundary_x_upper(y, t);
            let op = assemble_x_line(problem, grid.x(), y, t, bc_lo, bc_hi);
            op_x.push(op);
        }

        // Assemble y-direction operators: one tridiag per interior x-level
        let mut op_y = Vec::with_capacity(nx_int);
        for ii in 0..nx_int {
            let i = ii + 1;
            let x = x_pts[i];
            let bc_lo = problem.boundary_y_lower(x, t);
            let bc_hi = problem.boundary_y_upper(x, t);
            let op = assemble_y_line(problem, grid.y(), x, t, bc_lo, bc_hi);
            op_y.push(op);
        }

        // Cross-derivative at interior points
        let cross_deriv = compute_cross_derivative(problem, grid, t);

        Self {
            op_x,
            op_y,
            cross_deriv,
            nx_int,
            ny_int,
        }
    }
}

/// Assemble the x-direction tridiagonal operator at a fixed y-level.
///
/// Discretizes `a_xx * d²u/dx² + b_x * du/dx` with half-reaction.
fn assemble_x_line(
    problem: &dyn PdeProblem2D,
    x_grid: &Grid1D,
    y: f64,
    t: f64,
    bc_lower: BoundaryCondition,
    bc_upper: BoundaryCondition,
) -> TridiagOperator {
    let n = x_grid.n_interior();
    let pts = x_grid.points();

    let mut lower = vec![0.0; n];
    let mut main = vec![0.0; n];
    let mut upper = vec![0.0; n];
    let source = vec![0.0; n];

    for k in 0..n {
        let i = k + 1;
        let x = pts[i];
        let h_m = x_grid.h_left(i);
        let h_p = x_grid.h_right(i);
        let h_sum = h_m + h_p;

        let a = problem.diffusion_xx(x, y, t);
        let b = problem.convection_x(x, y, t);
        // Half-reaction goes into the x-direction operator
        let c_half = 0.5 * problem.reaction(x, y, t);

        lower[k] = 2.0 * a / (h_m * h_sum) - b * h_p / (h_m * h_sum);
        main[k] = -2.0 * a / (h_m * h_p) + b * (h_p - h_m) / (h_m * h_p) + c_half;
        upper[k] = 2.0 * a / (h_p * h_sum) + b * h_m / (h_p * h_sum);
    }

    TridiagOperator::from_parts(lower, main, upper, source, bc_lower, bc_upper, x_grid)
}

/// Assemble the y-direction tridiagonal operator at a fixed x-level.
///
/// Discretizes `a_yy * d²u/dy² + b_y * du/dy` with half-reaction.
fn assemble_y_line(
    problem: &dyn PdeProblem2D,
    y_grid: &Grid1D,
    x: f64,
    t: f64,
    bc_lower: BoundaryCondition,
    bc_upper: BoundaryCondition,
) -> TridiagOperator {
    let n = y_grid.n_interior();
    let pts = y_grid.points();

    let mut lower = vec![0.0; n];
    let mut main = vec![0.0; n];
    let mut upper = vec![0.0; n];
    let source = vec![0.0; n];

    for k in 0..n {
        let j = k + 1;
        let y = pts[j];
        let h_m = y_grid.h_left(j);
        let h_p = y_grid.h_right(j);
        let h_sum = h_m + h_p;

        let a = problem.diffusion_yy(x, y, t);
        let b = problem.convection_y(x, y, t);
        let c_half = 0.5 * problem.reaction(x, y, t);

        lower[k] = 2.0 * a / (h_m * h_sum) - b * h_p / (h_m * h_sum);
        main[k] = -2.0 * a / (h_m * h_p) + b * (h_p - h_m) / (h_m * h_p) + c_half;
        upper[k] = 2.0 * a / (h_p * h_sum) + b * h_m / (h_p * h_sum);
    }

    TridiagOperator::from_parts(lower, main, upper, source, bc_lower, bc_upper, y_grid)
}

/// Compute the explicit cross-derivative `a_xy * d²u/(dx dy)` applied to the
/// current solution at all interior points.
///
/// Uses the standard four-point stencil on the tensor-product grid:
/// ```text
/// d²u/(dx dy) ≈ [u(i+1,j+1) - u(i+1,j-1) - u(i-1,j+1) + u(i-1,j-1)]
///               / (2 * hx_i * 2 * hy_j)
/// ```
///
/// where `hx_i = 0.5 * (h_left + h_right)` at grid point `i`.
///
/// Returns a flat vector of length `nx_int * ny_int` (row-major by interior index).
fn compute_cross_derivative(problem: &dyn PdeProblem2D, grid: &Grid2D, t: f64) -> Vec<f64> {
    let nx_int = grid.nx_interior();
    let ny_int = grid.ny_interior();
    let x_pts = grid.x().points();
    let y_pts = grid.y().points();

    let mut cross = vec![0.0; nx_int * ny_int];

    for ii in 0..nx_int {
        let i = ii + 1;
        let x = x_pts[i];
        let hx = 0.5 * (grid.x().h_left(i) + grid.x().h_right(i));

        for jj in 0..ny_int {
            let j = jj + 1;
            let y = y_pts[j];
            let hy = 0.5 * (grid.y().h_left(j) + grid.y().h_right(j));

            let a_xy = problem.mixed_diffusion(x, y, t);
            // Store the coefficient divided by the stencil denominator.
            // The actual application (multiplying by solution values) happens
            // in the ADI stepper.
            cross[ii * ny_int + jj] = a_xy / (4.0 * hx * hy);
        }
    }

    cross
}

/// Apply the cross-derivative operator to a 2D solution vector.
///
/// `u_full` has length `nx * ny` (row-major, including boundaries).
/// `cross_coeffs` has length `nx_int * ny_int` from `compute_cross_derivative`
/// (private helper in this module).
///
/// Returns a flat vector of length `nx_int * ny_int` containing
/// `a_xy / (4 hx hy) * [u(i+1,j+1) - u(i+1,j-1) - u(i-1,j+1) + u(i-1,j-1)]`.
pub fn apply_cross_derivative(cross_coeffs: &[f64], u_full: &[f64], grid: &Grid2D) -> Vec<f64> {
    let nx_int = grid.nx_interior();
    let ny_int = grid.ny_interior();
    let ny = grid.ny();

    debug_assert_eq!(u_full.len(), grid.total());
    debug_assert_eq!(cross_coeffs.len(), nx_int * ny_int);

    let mut result = vec![0.0; nx_int * ny_int];

    for ii in 0..nx_int {
        let i = ii + 1; // grid index
        for jj in 0..ny_int {
            let j = jj + 1; // grid index

            // Four-point stencil using the full (boundary-inclusive) grid
            let u_pp = u_full[(i + 1) * ny + (j + 1)];
            let u_pm = u_full[(i + 1) * ny + (j - 1)];
            let u_mp = u_full[(i - 1) * ny + (j + 1)];
            let u_mm = u_full[(i - 1) * ny + (j - 1)];

            result[ii * ny_int + jj] = cross_coeffs[ii * ny_int + jj] * (u_pp - u_pm - u_mp + u_mm);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_derivative_of_product() {
        // u(x, y) = x * y → d²u/(dx dy) = 1 everywhere
        let gx = Grid1D::uniform(0.0, 2.0, 5).expect("valid");
        let gy = Grid1D::uniform(0.0, 3.0, 5).expect("valid");
        let grid = Grid2D::new(gx, gy);

        let nx = grid.nx();
        let ny = grid.ny();
        let mut u_full = vec![0.0; nx * ny];
        for i in 0..nx {
            for j in 0..ny {
                u_full[i * ny + j] = grid.x().points()[i] * grid.y().points()[j];
            }
        }

        // Cross coefficients: a_xy / (4 hx hy) where a_xy = 1
        let nx_int = grid.nx_interior();
        let ny_int = grid.ny_interior();
        let cross_coeffs: Vec<f64> = (0..nx_int * ny_int)
            .map(|flat| {
                let ii = flat / ny_int;
                let jj = flat % ny_int;
                let i = ii + 1;
                let j = jj + 1;
                let hx = 0.5 * (grid.x().h_left(i) + grid.x().h_right(i));
                let hy = 0.5 * (grid.y().h_left(j) + grid.y().h_right(j));
                1.0 / (4.0 * hx * hy) // a_xy = 1
            })
            .collect();

        let result = apply_cross_derivative(&cross_coeffs, &u_full, &grid);

        // d²(x*y)/(dx dy) = 1.0 at all interior points
        for val in &result {
            assert!(
                (val - 1.0).abs() < 1e-10,
                "cross derivative should be 1.0, got {val}"
            );
        }
    }

    /// Cross-derivative weights must use **per-cell** spacing `(h_left, h_right)`
    /// so that non-uniform grids stay consistent. The previous test only
    /// covered uniform grids; with two distinct non-uniform spacings on each
    /// axis, any off-by-one or mis-indexed `h_left/h_right` would be visible.
    #[test]
    fn cross_derivative_of_product_on_non_uniform_grid() {
        // Deliberately uneven grids so `h_left != h_right` at most interior nodes
        let gx = Grid1D::from_points(vec![0.0, 0.7, 1.5, 2.5, 4.0]).expect("valid x grid");
        let gy = Grid1D::from_points(vec![0.0, 0.4, 1.0, 2.0, 3.0]).expect("valid y grid");
        let grid = Grid2D::new(gx, gy);

        let nx = grid.nx();
        let ny = grid.ny();
        let mut u_full = vec![0.0; nx * ny];
        for i in 0..nx {
            for j in 0..ny {
                u_full[i * ny + j] = grid.x().points()[i] * grid.y().points()[j];
            }
        }

        let nx_int = grid.nx_interior();
        let ny_int = grid.ny_interior();
        let cross_coeffs: Vec<f64> = (0..nx_int * ny_int)
            .map(|flat| {
                let ii = flat / ny_int;
                let jj = flat % ny_int;
                let i = ii + 1;
                let j = jj + 1;
                let hx = 0.5 * (grid.x().h_left(i) + grid.x().h_right(i));
                let hy = 0.5 * (grid.y().h_left(j) + grid.y().h_right(j));
                1.0 / (4.0 * hx * hy)
            })
            .collect();

        let result = apply_cross_derivative(&cross_coeffs, &u_full, &grid);

        // d²(xy)/(dx dy) = 1 — must hold to machine epsilon on any grid because
        // the central-difference cross-derivative is exact for bilinear u.
        for (idx, val) in result.iter().enumerate() {
            assert!(
                (val - 1.0).abs() < 1e-10,
                "non-uniform cross-derivative at flat index {idx}: expected 1.0, got {val}"
            );
        }
    }
}

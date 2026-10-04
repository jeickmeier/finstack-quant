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
use super::operator::{node_stencil, TridiagOperator};
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

        let mut op_x = Vec::with_capacity(ny_int);
        for jj in 0..ny_int {
            let j = jj + 1; // grid index
            let y = y_pts[j];
            let bc_lo = problem.boundary_x_lower(y, t);
            let bc_hi = problem.boundary_x_upper(y, t);
            let op = assemble_line(grid.x(), bc_lo, bc_hi, |x| {
                (
                    problem.diffusion_xx(x, y, t),
                    problem.convection_x(x, y, t),
                    problem.reaction(x, y, t),
                    problem.source(x, y, t),
                )
            });
            op_x.push(op);
        }

        let mut op_y = Vec::with_capacity(nx_int);
        for ii in 0..nx_int {
            let i = ii + 1;
            let x = x_pts[i];
            let bc_lo = problem.boundary_y_lower(x, t);
            let bc_hi = problem.boundary_y_upper(x, t);
            let op = assemble_line(grid.y(), bc_lo, bc_hi, |y| {
                (
                    problem.diffusion_yy(x, y, t),
                    problem.convection_y(x, y, t),
                    problem.reaction(x, y, t),
                    problem.source(x, y, t),
                )
            });
            op_y.push(op);
        }

        let cross_deriv = compute_cross_derivative(problem, grid, t);

        Self {
            op_x,
            op_y,
            cross_deriv,
        }
    }
}

/// Assemble one directional tridiagonal operator along `line_grid`.
///
/// Discretizes `a * d²u/ds² + b * du/ds` with half of the reaction and source
/// terms. `coeffs(s)` returns `(diffusion, convection, reaction, source)` at
/// coordinate `s` along the line, with the other coordinate held fixed by the
/// caller.
fn assemble_line(
    line_grid: &Grid1D,
    bc_lower: BoundaryCondition,
    bc_upper: BoundaryCondition,
    coeffs: impl Fn(f64) -> (f64, f64, f64, f64),
) -> TridiagOperator {
    let n = line_grid.n_interior();
    let pts = line_grid.points();

    let mut lower = vec![0.0; n];
    let mut main = vec![0.0; n];
    let mut upper = vec![0.0; n];
    let mut source = vec![0.0; n];

    for k in 0..n {
        let i = k + 1;
        let h_m = line_grid.h_left(i);
        let h_p = line_grid.h_right(i);

        let (a, b, reaction, src) = coeffs(pts[i]);
        // Half-reaction goes into each directional operator
        let c_half = 0.5 * reaction;

        let (lo, mi, up) = node_stencil(a, b, h_m, h_p);
        lower[k] = lo;
        main[k] = mi + c_half;
        upper[k] = up;
        source[k] = 0.5 * src;
    }

    TridiagOperator::from_parts(lower, main, upper, source, bc_lower, bc_upper, line_grid)
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

/// Apply the cross-derivative operator to a 2D solution vector, writing
/// `a_xy / (4 hx hy) * [u(i+1,j+1) - u(i+1,j-1) - u(i-1,j+1) + u(i-1,j-1)]`
/// into `out`.
///
/// The Modified Craig-Sneyd ADI stepper applies this twice per timestep into
/// caller-owned scratch buffers.
///
/// # Arguments
///
/// * `out` - Mutable row-major interior buffer of length `nx_int * ny_int`
///   overwritten with the cross-derivative result.
/// * `cross_coeffs` - Row-major interior cross-derivative coefficients from
///   `compute_cross_derivative`, with the same required length as `out`.
/// * `u_full` - Row-major full-grid solution including boundaries, with length
///   `grid.total()`.
/// * `grid` - Two-dimensional grid defining full and interior dimensions.
pub(crate) fn apply_cross_derivative(
    out: &mut [f64],
    cross_coeffs: &[f64],
    u_full: &[f64],
    grid: &Grid2D,
) {
    let nx_int = grid.nx_interior();
    let ny_int = grid.ny_interior();
    let ny = grid.ny();

    debug_assert_eq!(u_full.len(), grid.total());
    debug_assert_eq!(cross_coeffs.len(), nx_int * ny_int);
    debug_assert_eq!(out.len(), nx_int * ny_int);

    for ii in 0..nx_int {
        let i = ii + 1; // grid index
        for jj in 0..ny_int {
            let j = jj + 1; // grid index

            // Four-point stencil using the full (boundary-inclusive) grid
            let u_pp = u_full[(i + 1) * ny + (j + 1)];
            let u_pm = u_full[(i + 1) * ny + (j - 1)];
            let u_mp = u_full[(i - 1) * ny + (j + 1)];
            let u_mm = u_full[(i - 1) * ny + (j - 1)];

            out[ii * ny_int + jj] = cross_coeffs[ii * ny_int + jj] * (u_pp - u_pm - u_mp + u_mm);
        }
    }
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

        let mut result = vec![0.0; nx_int * ny_int];
        apply_cross_derivative(&mut result, &cross_coeffs, &u_full, &grid);

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

        let mut result = vec![0.0; nx_int * ny_int];
        apply_cross_derivative(&mut result, &cross_coeffs, &u_full, &grid);

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

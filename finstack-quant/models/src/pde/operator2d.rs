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
use super::operator::{TridiagOperator, central_stencil, node_stencil};
use super::problem2d::PdeProblem2D;

/// The y-levels the ADI systems solve for.
///
/// These are the interior levels, preceded by the lower edge when that edge
/// carries [`BoundaryCondition::Degenerate`]. Solved vectors are row-major
/// with `count` entries per interior x-node; entry `jj` is grid level
/// `first + jj`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SolvedRows {
    /// Grid y-index of the first solved level: 0 with a degenerate lower
    /// edge, 1 otherwise.
    pub(super) first: usize,
    /// Number of solved levels.
    pub(super) count: usize,
}

impl SolvedRows {
    /// Solved y-levels of `problem` on `grid`.
    ///
    /// The kind of the lower y-edge condition may not depend on x or time,
    /// so it is read once ([`super::Solver2D::solve`] checks every x).
    pub(super) fn of(problem: &dyn PdeProblem2D, grid: &Grid2D) -> Self {
        let x = grid.x().points()[1];
        let first = usize::from(!matches!(
            problem.boundary_y_lower(x, 0.0),
            BoundaryCondition::Degenerate
        ));
        Self {
            first,
            count: grid.ny() - 1 - first,
        }
    }

    /// The interior levels only.
    #[cfg(test)]
    pub(super) fn interior(grid: &Grid2D) -> Self {
        Self {
            first: 1,
            count: grid.ny_interior(),
        }
    }
}

/// Weights of the second-order one-sided first derivative at the first of
/// three nodes whose spacings are `h1` then `h2`.
pub(super) fn forward_difference_weights(h1: f64, h2: f64) -> [f64; 3] {
    let h_sum = h1 + h2;
    [
        -(h1 + h_sum) / (h1 * h_sum),
        h_sum / (h1 * h2),
        -h1 / (h2 * h_sum),
    ]
}

/// Assembled 2D operators for one time level, split by direction.
///
/// `op_x[j]` is the tridiagonal operator along the x-axis at solved y-level
/// `j`. `op_y[i]` is along the y-axis at interior x-level `i`. The
/// cross-derivative is stored as a flat vector of explicit contributions.
#[derive(Debug, Clone)]
pub struct Operators2D {
    /// Tridiagonal operators along x, one per solved y-level.
    /// `op_x[j]` has size `nx_interior`.
    pub op_x: Vec<TridiagOperator>,
    /// Operators along y, one per interior x-level, each over the solved
    /// y-levels.
    pub op_y: Vec<TridiagOperator>,
    /// Cross-derivative contribution `a_xy * d²u/(dx dy)` evaluated at each
    /// solved point. Length `nx_interior` times the number of solved
    /// y-levels.
    pub cross_deriv: Vec<f64>,
    /// The y-levels the operators cover.
    rows: SolvedRows,
}

impl Operators2D {
    /// Assemble all directional operators and the cross-derivative at time `t`.
    ///
    /// The reaction term `c(x, y, t) u` is split equally: half goes to the
    /// x-direction implicit solve, half to the y-direction. This avoids
    /// double-counting in the ADI splitting.
    pub fn assemble(problem: &dyn PdeProblem2D, grid: &Grid2D, t: f64) -> Self {
        let nx_int = grid.nx_interior();
        let rows = SolvedRows::of(problem, grid);
        let x_pts = grid.x().points();
        let y_pts = grid.y().points();

        let mut op_x = Vec::with_capacity(rows.count);
        for jj in 0..rows.count {
            let y = y_pts[jj + rows.first];
            let bc_lo = problem.boundary_x_lower(y, t);
            let bc_hi = problem.boundary_x_upper(y, t);
            // Along a degenerate edge the x-diffusion may vanish, and a
            // one-sided convection would then smear the terminal kink.
            let stencil = if jj + rows.first == 0 {
                central_stencil
            } else {
                node_stencil
            };
            let op = assemble_line((grid.x(), 1), stencil, bc_lo, bc_hi, |x| {
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
            let op = assemble_line((grid.y(), rows.first), node_stencil, bc_lo, bc_hi, |y| {
                (
                    problem.diffusion_yy(x, y, t),
                    problem.convection_y(x, y, t),
                    problem.reaction(x, y, t),
                    problem.source(x, y, t),
                )
            });
            op_y.push(op);
        }

        let cross_deriv = compute_cross_derivative(problem, grid, rows, t);

        Self {
            op_x,
            op_y,
            cross_deriv,
            rows,
        }
    }

    /// Refresh the boundary RHS corrections for time-varying boundary values
    /// along all x- and y-lines at time `t`.
    pub(super) fn set_boundary_rhs(&mut self, problem: &dyn PdeProblem2D, grid: &Grid2D, t: f64) {
        let x_pts = grid.x().points();
        let y_pts = grid.y().points();
        for (jj, op) in self.op_x.iter_mut().enumerate() {
            let y = y_pts[jj + self.rows.first];
            op.set_boundary_rhs(
                problem.boundary_x_lower(y, t),
                problem.boundary_x_upper(y, t),
            );
        }
        for (ii, op) in self.op_y.iter_mut().enumerate() {
            let x = x_pts[ii + 1];
            op.set_boundary_rhs(
                problem.boundary_y_lower(x, t),
                problem.boundary_y_upper(x, t),
            );
        }
    }
}

/// Assemble one directional tridiagonal operator along `line_grid`.
///
/// Discretizes `a * d²u/ds² + b * du/ds` with half of the reaction and source
/// terms. `coeffs(s)` returns `(diffusion, convection, reaction, source)` at
/// coordinate `s` along the line, with the other coordinate held fixed by the
/// caller.
///
/// `first` is the grid index of the first solved node: 1 for interior nodes
/// only, 0 when the lower edge is [`BoundaryCondition::Degenerate`]. The edge
/// row then holds convection through the one-sided three-point difference
/// (the diffusion vanishes there), with its third weight outside the band.
/// `stencil` discretizes diffusion and convection at every other node.
fn assemble_line(
    (line_grid, first): (&Grid1D, usize),
    stencil: fn(f64, f64, f64, f64) -> (f64, f64, f64),
    bc_lower: BoundaryCondition,
    bc_upper: BoundaryCondition,
    coeffs: impl Fn(f64) -> (f64, f64, f64, f64),
) -> TridiagOperator {
    let n = line_grid.n() - 1 - first;
    let pts = line_grid.points();

    let mut lower = vec![0.0; n];
    let mut main = vec![0.0; n];
    let mut upper = vec![0.0; n];
    let mut source = vec![0.0; n];
    let mut edge_upper2 = 0.0;

    for k in 0..n {
        let i = k + first;
        if i == 0 {
            let (_, b, reaction, src) = coeffs(pts[0]);
            let [w0, w1, w2] =
                forward_difference_weights(line_grid.h_right(0), line_grid.h_right(1));
            main[k] = b * w0 + 0.5 * reaction;
            upper[k] = b * w1;
            edge_upper2 = b * w2;
            source[k] = 0.5 * src;
            continue;
        }
        let h_m = line_grid.h_left(i);
        let h_p = line_grid.h_right(i);

        let (a, b, reaction, src) = coeffs(pts[i]);
        // Half-reaction goes into each directional operator
        let c_half = 0.5 * reaction;

        let (lo, mi, up) = stencil(a, b, h_m, h_p);
        lower[k] = lo;
        main[k] = mi + c_half;
        upper[k] = up;
        source[k] = 0.5 * src;
    }

    let op = TridiagOperator::from_parts(lower, main, upper, source, bc_lower, bc_upper, line_grid);
    if first == 0 {
        op.with_lower_edge_coupling(edge_upper2)
    } else {
        op
    }
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
/// Returns a flat row-major vector over the solved points. A degenerate lower
/// edge has no mixed term, so its entries are zero.
fn compute_cross_derivative(
    problem: &dyn PdeProblem2D,
    grid: &Grid2D,
    rows: SolvedRows,
    t: f64,
) -> Vec<f64> {
    let nx_int = grid.nx_interior();
    let ny_int = rows.count;
    let x_pts = grid.x().points();
    let y_pts = grid.y().points();

    let mut cross = vec![0.0; nx_int * ny_int];

    for ii in 0..nx_int {
        let i = ii + 1;
        let x = x_pts[i];
        let hx = 0.5 * (grid.x().h_left(i) + grid.x().h_right(i));

        for jj in 0..ny_int {
            let j = jj + rows.first;
            if j == 0 {
                continue;
            }
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
/// * `out` - Mutable row-major buffer over the solved points (`nx_int` times
///   `rows.count`) overwritten with the cross-derivative result.
/// * `cross_coeffs` - Row-major cross-derivative coefficients from
///   `compute_cross_derivative`, with the same required length as `out`.
/// * `u_full` - Row-major full-grid solution including boundaries, with length
///   `grid.total()`.
/// * `grid` - Two-dimensional grid defining the full dimensions.
/// * `rows` - Solved y-levels that `out` and `cross_coeffs` cover; a
///   degenerate lower edge level receives zero.
pub(super) fn apply_cross_derivative(
    out: &mut [f64],
    cross_coeffs: &[f64],
    u_full: &[f64],
    grid: &Grid2D,
    rows: SolvedRows,
) {
    let nx_int = grid.nx_interior();
    let ny_int = rows.count;
    let ny = grid.ny();

    debug_assert_eq!(u_full.len(), grid.total());
    debug_assert_eq!(cross_coeffs.len(), nx_int * ny_int);
    debug_assert_eq!(out.len(), nx_int * ny_int);

    for ii in 0..nx_int {
        let i = ii + 1; // grid index
        for jj in 0..ny_int {
            let j = jj + rows.first; // grid index
            if j == 0 {
                out[ii * ny_int + jj] = 0.0;
                continue;
            }

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
    fn forward_difference_weights_are_exact_for_quadratics() {
        let (h1, h2) = (0.3, 0.5);
        let [w0, w1, w2] = forward_difference_weights(h1, h2);
        let y0 = 0.2;
        for (u, slope) in [
            ((|_| 1.0) as fn(f64) -> f64, 0.0),
            (|y| y, 1.0),
            (|y| y * y, 2.0 * y0),
        ] {
            let approx = w0 * u(y0) + w1 * u(y0 + h1) + w2 * u(y0 + h1 + h2);
            assert!((approx - slope).abs() < 1e-13, "{approx} != {slope}");
        }
    }

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
        apply_cross_derivative(
            &mut result,
            &cross_coeffs,
            &u_full,
            &grid,
            SolvedRows::interior(&grid),
        );

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
        apply_cross_derivative(
            &mut result,
            &cross_coeffs,
            &u_full,
            &grid,
            SolvedRows::interior(&grid),
        );

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

//! Tridiagonal spatial operator and Thomas algorithm.
//!
//! Discretizes a 1D PDE on a [`Grid1D`] into a tridiagonal matrix using
//! central finite differences on (possibly non-uniform) grids, with monotone
//! one-sided convection in drift-dominated cells. The Thomas algorithm solves
//! the resulting system in O(n) time.

use super::boundary::BoundaryCondition;
use super::error::PdeError;
use super::grid::Grid1D;
use super::problem::PdeProblem1D;

/// Relative threshold below which a Thomas-elimination pivot is treated as
/// degenerate. A pivot whose magnitude has collapsed to less than this
/// fraction of the terms that formed it has lost essentially all numerical
/// significance (catastrophic cancellation, or a genuine zero pivot); the
/// subsequent division would yield a silent `inf` / `NaN`. The threshold is
/// far above f64 machine epsilon (~2.2e-16), so a merely small — but
/// genuinely well-conditioned — pivot is not flagged.
const THOMAS_PIVOT_REL_EPS: f64 = 1e-12;

/// Check that a freshly computed Thomas pivot `d` (formed as the difference of
/// terms whose magnitudes are `term_a` and `term_b`) is non-degenerate.
///
/// Returns [`PdeError::DegeneratePivot`] if `d` is non-finite or its
/// magnitude has collapsed below [`THOMAS_PIVOT_REL_EPS`] times the row scale.
#[inline]
fn check_pivot(d: f64, term_a: f64, term_b: f64, row: usize, n: usize) -> Result<(), PdeError> {
    // Row scale: largest constituent magnitude, floored at 1.0 so that a row
    // whose terms are all genuinely tiny still requires a non-zero pivot.
    let row_scale = term_a.abs().max(term_b.abs()).max(1.0);
    if !d.is_finite() || d.abs() <= THOMAS_PIVOT_REL_EPS * row_scale {
        return Err(PdeError::DegeneratePivot {
            row,
            n,
            pivot: d,
            row_scale,
        });
    }
    Ok(())
}

/// Second-difference weights of `a * u''` at a node with spacings `h_m`
/// (below) and `h_p` (above).
fn diffusion_stencil(a: f64, h_m: f64, h_p: f64) -> (f64, f64, f64) {
    let h_sum = h_m + h_p;
    (
        2.0 * a / (h_m * h_sum),
        -2.0 * a / (h_m * h_p),
        2.0 * a / (h_p * h_sum),
    )
}

/// Second-order diffusion/convection stencil with central convection
/// whatever the cell Péclet number.
///
/// Used along a [`BoundaryCondition::Degenerate`] edge, where the diffusion
/// in the direction of the line can vanish. A one-sided convection there adds
/// `|b| * h / 2` of numerical diffusion to a line that has none, smearing a
/// payoff kink at first order in `h`; the implicit ADI sweeps are stable with
/// central convection.
pub(super) fn central_stencil(a: f64, b: f64, h_m: f64, h_p: f64) -> (f64, f64, f64) {
    let h_sum = h_m + h_p;
    let (mut lower, mut main, mut upper) = diffusion_stencil(a, h_m, h_p);
    lower -= b * h_p / (h_m * h_sum);
    main += b * (h_p - h_m) / (h_m * h_p);
    upper += b * h_m / (h_p * h_sum);
    (lower, main, upper)
}

/// Monotone diffusion/convection stencil shared by the 1D and ADI operators.
/// Central differences retain second-order accuracy where both off-diagonals
/// are non-negative; convection-dominated cells use the appropriate one-sided
/// difference so the spatial operator preserves non-negative payoffs.
/// The one-sided branch adds approximately `|b| * h / 2` numerical diffusion;
/// low-volatility prices require spatial refinement to control that bias.
pub(super) fn node_stencil(a: f64, b: f64, h_m: f64, h_p: f64) -> (f64, f64, f64) {
    let central_is_monotone = if b >= 0.0 {
        b * h_p <= 2.0 * a
    } else {
        -b * h_m <= 2.0 * a
    };
    if central_is_monotone {
        return central_stencil(a, b, h_m, h_p);
    }

    let (mut lower, mut main, mut upper) = diffusion_stencil(a, h_m, h_p);
    if b >= 0.0 {
        main -= b / h_p;
        upper += b / h_p;
    } else {
        main += b / h_m;
        lower -= b / h_m;
    }
    (lower, main, upper)
}

/// Tridiagonal matrix representing the spatial discretization of a 1D PDE.
///
/// For `n` interior grid points, the matrix is `n × n` with sub-diagonal,
/// main diagonal, and super-diagonal bands. Boundary conditions are incorporated
/// at assembly time by modifying the first/last rows and storing RHS corrections.
///
/// # Layout
///
/// All vectors have length `n` (number of interior points). `lower[0]` and
/// `upper[n-1]` are zero (no coupling beyond the boundary).
#[derive(Debug, Clone)]
pub struct TridiagOperator {
    /// Sub-diagonal: coupling from point `i` to `i-1`. `lower[0] = 0`.
    lower: Vec<f64>,
    /// Main diagonal coefficients.
    main: Vec<f64>,
    /// Super-diagonal: coupling from point `i` to `i+1`. `upper[n-1] = 0`.
    upper: Vec<f64>,
    /// Source term `f(x_i, t)` at each interior point.
    source: Vec<f64>,
    /// Stencil multiplier converting lower boundary value/slope `g(t)` into `bc_lower_rhs`.
    bc_lower_scale: f64,
    /// Stencil multiplier converting upper boundary value/slope `g(t)` into `bc_upper_rhs`.
    bc_upper_scale: f64,
    /// RHS correction from lower boundary at the first interior point.
    bc_lower_rhs: f64,
    /// RHS correction from upper boundary at the last interior point.
    bc_upper_rhs: f64,
    /// Coupling from row 0 to `x[2]`: the third weight of a one-sided
    /// difference on a [`BoundaryCondition::Degenerate`] lower edge row. Zero
    /// for every other operator, which is then strictly tridiagonal.
    lower_edge_upper2: f64,
    /// Number of interior points.
    n: usize,
}

impl TridiagOperator {
    /// Assemble the tridiagonal operator from PDE coefficients at time `t`.
    ///
    /// Discretizes `a(x,t) u'' + b(x,t) u' + c(x,t) u + f(x,t)` on the
    /// given grid. Central differences are second-order on smoothly varying
    /// grids; drift-dominated cells use first-order monotone convection.
    /// Boundary conditions modify the first and last rows.
    pub fn assemble(problem: &dyn PdeProblem1D, grid: &Grid1D, t: f64) -> Self {
        let n = grid.n_interior();
        let mut lower = vec![0.0; n];
        let mut main = vec![0.0; n];
        let mut upper = vec![0.0; n];
        let mut source = vec![0.0; n];

        for k in 0..n {
            let i = k + 1; // grid index
            let x = grid.points()[i];
            let h_m = grid.h_left(i);
            let h_p = grid.h_right(i);
            let a = problem.diffusion(x, t);
            let b = problem.convection(x, t);
            let c = problem.reaction(x, t);

            let (lo, mi, up) = node_stencil(a, b, h_m, h_p);
            lower[k] = lo;
            main[k] = mi + c;
            upper[k] = up;
            source[k] = problem.source(x, t);
        }

        Self::from_parts(
            lower,
            main,
            upper,
            source,
            problem.lower_boundary(t),
            problem.upper_boundary(t),
            grid,
        )
    }

    /// Construct a tridiagonal operator from pre-computed diagonals and boundary conditions.
    ///
    /// Used by 2D ADI assembly where each directional line is assembled
    /// separately and boundary conditions are applied per-line.
    pub fn from_parts(
        mut lower: Vec<f64>,
        mut main: Vec<f64>,
        mut upper: Vec<f64>,
        source: Vec<f64>,
        bc_lower: BoundaryCondition,
        bc_upper: BoundaryCondition,
        grid: &Grid1D,
    ) -> Self {
        let n = lower.len();
        debug_assert_eq!(main.len(), n);
        debug_assert_eq!(upper.len(), n);
        debug_assert_eq!(source.len(), n);

        let (bc_lower_scale, bc_lower_rhs) =
            apply_lower_boundary(bc_lower, &mut lower, &mut main, &mut upper, grid);
        let (bc_upper_scale, bc_upper_rhs) =
            apply_upper_boundary(bc_upper, &mut lower, &mut main, &mut upper, grid, n);

        Self {
            lower,
            main,
            upper,
            source,
            bc_lower_scale,
            bc_upper_scale,
            bc_lower_rhs,
            bc_upper_rhs,
            lower_edge_upper2: 0.0,
            n,
        }
    }

    /// Set the coupling from row 0 to `x[2]` of a lower edge row that holds a
    /// one-sided three-point difference.
    ///
    /// The row stays outside the tridiagonal band; [`Self::solve_thomas_into`]
    /// eliminates the entry against row 1 before the forward sweep.
    pub(super) fn with_lower_edge_coupling(mut self, upper2: f64) -> Self {
        debug_assert!(self.n >= 3, "a three-point edge row needs three unknowns");
        self.lower_edge_upper2 = upper2;
        self
    }

    /// `lower_edge_upper2 * x[2]`, or zero for a strictly tridiagonal operator.
    #[inline]
    fn lower_edge_term(&self, x: &[f64]) -> f64 {
        if self.lower_edge_upper2 == 0.0 {
            0.0
        } else {
            self.lower_edge_upper2 * x[2]
        }
    }

    /// Refresh the boundary RHS corrections for time-varying boundary values
    /// while reusing the cached interior tridiagonal matrix.
    pub(super) fn set_boundary_rhs(
        &mut self,
        bc_lower: BoundaryCondition,
        bc_upper: BoundaryCondition,
    ) {
        self.bc_lower_rhs = self.bc_lower_scale * boundary_scalar(bc_lower);
        self.bc_upper_rhs = self.bc_upper_scale * boundary_scalar(bc_upper);
    }

    /// Compute `y = A * x + source` for interior points, writing into `out`.
    ///
    /// Boundary contributions are added from the stored corrections.
    ///
    /// # Arguments
    ///
    /// * `x` - Interior solution values, length `n`.
    /// * `out` - Pre-allocated output slice of length `n`, overwritten with
    ///   `A * x + source` plus the boundary corrections.
    pub fn apply(&self, x: &[f64], out: &mut [f64]) {
        debug_assert_eq!(x.len(), self.n);
        debug_assert_eq!(out.len(), self.n);

        if self.n == 0 {
            return;
        }

        out[0] = self.main[0] * x[0] + self.source[0] + self.bc_lower_rhs + self.lower_edge_term(x);
        if self.n > 1 {
            out[0] += self.upper[0] * x[1];
        }

        for i in 1..self.n.saturating_sub(1) {
            out[i] = self.lower[i] * x[i - 1]
                + self.main[i] * x[i]
                + self.upper[i] * x[i + 1]
                + self.source[i];
        }

        if self.n > 1 {
            let last = self.n - 1;
            out[last] = self.lower[last] * x[last - 1]
                + self.main[last] * x[last]
                + self.source[last]
                + self.bc_upper_rhs;
        } else {
            // n == 1: single interior point gets both boundary corrections
            out[0] += self.bc_upper_rhs;
        }
    }

    /// Solve `(I - alpha * A) * x_new = rhs` using the Thomas algorithm.
    ///
    /// This is the implicit step in the theta scheme, where `alpha = theta * dt`.
    /// The system is tridiagonal and solved in O(n) with no heap allocation
    /// beyond the output vector.
    ///
    /// # Arguments
    ///
    /// * `alpha` — scalar multiplier (typically `theta * dt`)
    /// * `rhs` — right-hand side vector (length `n`)
    ///
    /// Returns the solution vector of length `n`, or
    /// [`PdeError::DegeneratePivot`] if the forward elimination hits a
    /// degenerate pivot (a sign that `(I - alpha*A)` has lost diagonal
    /// dominance, e.g. via a boundary-condition modification). Without the
    /// guard such a pivot would yield a silent `inf` / `NaN` solution.
    pub fn solve_thomas(&self, alpha: f64, rhs: &[f64]) -> Result<Vec<f64>, PdeError> {
        let mut x = vec![0.0; self.n];
        let mut scratch = vec![0.0; self.n];
        self.solve_thomas_into(alpha, rhs, &mut x, &mut scratch)?;
        Ok(x)
    }

    /// Solve `(I - alpha * A) * x_new = rhs` using the Thomas algorithm,
    /// writing the result into `out`.
    ///
    /// Like [`solve_thomas`](Self::solve_thomas) but allocation-free: `out`
    /// holds the forward-eliminated right-hand side before back-substitution
    /// and `scratch` holds the modified main diagonal. Returns
    /// [`PdeError::DegeneratePivot`] on a degenerate pivot.
    ///
    /// # Arguments
    ///
    /// * `alpha` — scalar multiplier (typically `theta * dt`)
    /// * `rhs` — right-hand side vector (length `n`)
    /// * `out` — solution buffer (length `n`), overwritten
    /// * `scratch` — work buffer (length `n`), overwritten
    pub fn solve_thomas_into(
        &self,
        alpha: f64,
        rhs: &[f64],
        out: &mut [f64],
        scratch: &mut [f64],
    ) -> Result<(), PdeError> {
        debug_assert_eq!(rhs.len(), self.n);
        debug_assert_eq!(out.len(), self.n);
        debug_assert_eq!(scratch.len(), self.n);

        if self.n == 0 {
            return Ok(());
        }

        let n = self.n;
        let d = scratch;

        // Row 0 of `(I - alpha*A)`: diagonal, super-diagonal and right-hand side.
        let mut upper0 = -alpha * self.upper.first().copied().unwrap_or(0.0);
        d[0] = 1.0 - alpha * self.main[0];
        out[0] = rhs[0];
        if self.lower_edge_upper2 != 0.0 {
            // A one-sided edge row also couples to x[2]. Subtract the multiple
            // of row 1 that cancels that entry, leaving a tridiagonal system.
            check_pivot(self.upper[1], self.lower_edge_upper2, 0.0, 1, n)?;
            let factor = self.lower_edge_upper2 / self.upper[1];
            d[0] += factor * alpha * self.lower[1];
            upper0 -= factor * (1.0 - alpha * self.main[1]);
            out[0] -= factor * rhs[1];
        }
        check_pivot(d[0], 1.0, alpha * self.main[0], 0, n)?;

        for i in 1..n {
            let a_i = -alpha * self.lower[i];
            let c_prev = if i == 1 {
                upper0
            } else {
                -alpha * self.upper[i - 1]
            };
            let w = a_i / d[i - 1];
            let diag = 1.0 - alpha * self.main[i];
            let off = w * c_prev;
            d[i] = diag - off;
            check_pivot(d[i], diag, off, i, n)?;
            out[i] = rhs[i] - w * out[i - 1];
        }

        out[n - 1] /= d[n - 1];
        for i in (0..n - 1).rev() {
            let c_i = if i == 0 {
                upper0
            } else {
                -alpha * self.upper[i]
            };
            out[i] = (out[i] - c_i * out[i + 1]) / d[i];
        }
        Ok(())
    }

    /// Compute the explicit-side RHS for a theta step:
    /// `y = (I + beta * A) * x + beta * (source + bc_corrections)`
    ///
    /// where `beta = (1 - theta) * dt`. This is the explicit contribution
    /// in the Crank-Nicolson or other theta schemes.
    pub fn apply_explicit(&self, beta: f64, x: &[f64]) -> Vec<f64> {
        debug_assert_eq!(x.len(), self.n);
        let mut y = vec![0.0; self.n];

        if self.n == 0 {
            return y;
        }

        y[0] = x[0]
            + beta
                * (self.main[0] * x[0]
                    + self.source[0]
                    + self.bc_lower_rhs
                    + self.lower_edge_term(x));
        if self.n > 1 {
            y[0] += beta * self.upper[0] * x[1];
        }

        for i in 1..self.n.saturating_sub(1) {
            y[i] = x[i]
                + beta
                    * (self.lower[i] * x[i - 1]
                        + self.main[i] * x[i]
                        + self.upper[i] * x[i + 1]
                        + self.source[i]);
        }

        if self.n > 1 {
            let last = self.n - 1;
            y[last] = x[last]
                + beta
                    * (self.lower[last] * x[last - 1]
                        + self.main[last] * x[last]
                        + self.source[last]
                        + self.bc_upper_rhs);
        } else {
            y[0] += beta * self.bc_upper_rhs;
        }

        y
    }

    /// Add implicit-side boundary and source corrections to a RHS vector.
    ///
    /// Adds `alpha * (source + bc_corrections)` to `rhs`, where
    /// `alpha = theta * dt`. Used by the theta stepper for the implicit
    /// contribution of source/boundary terms.
    pub fn add_implicit_corrections(&self, alpha: f64, rhs: &mut [f64]) {
        debug_assert_eq!(rhs.len(), self.n);

        if self.n == 0 {
            return;
        }

        for (source_val, rhs_val) in self.source.iter().zip(rhs.iter_mut()) {
            *rhs_val += alpha * source_val;
        }

        rhs[0] += alpha * self.bc_lower_rhs;
        if self.n > 1 {
            rhs[self.n - 1] += alpha * self.bc_upper_rhs;
        } else {
            rhs[0] += alpha * self.bc_upper_rhs;
        }
    }
}

fn boundary_scalar(bc: BoundaryCondition) -> f64 {
    match bc {
        BoundaryCondition::Dirichlet(g) | BoundaryCondition::Neumann(g) => g,
        BoundaryCondition::Linear
        | BoundaryCondition::LinearInExp
        | BoundaryCondition::Degenerate => 0.0,
    }
}

/// Apply Dirichlet/Neumann/Linear boundary condition at the lower edge.
///
/// Returns `(scale, rhs)` for the first interior point.
fn apply_lower_boundary(
    bc: BoundaryCondition,
    lower: &mut [f64],
    main: &mut [f64],
    upper: &mut [f64],
    grid: &Grid1D,
) -> (f64, f64) {
    let scale = match bc {
        BoundaryCondition::Dirichlet(_) => {
            // The stencil at the first interior point (k=0) has coupling lower[0]
            // to the boundary. Since u[0] = g, shift to RHS: rhs += lower[0] * g
            let scale = lower[0];
            lower[0] = 0.0; // no coupling in the tridiagonal system
            scale
        }
        BoundaryCondition::Neumann(_) => {
            // du/dx at x_min = g, imposed one-sided at the boundary node:
            // (u[1] - u[0]) / h_1 = g  →  u[0] = u[1] - h_1*g,
            // where u[1] is the first interior unknown and h_1 = x[1] - x[0].
            // Substitute into the first interior row:
            //   lower[0]*u[0] → lower[0]*(u[1] - h_1*g)
            //   main[0] += lower[0];  affine correction = -lower[0]*h_1*g.
            // This is the SAME discretization `boundary_value`/
            // `fill_boundaries` use to reconstruct u[0] from the solved
            // interior — using a central ghost form here (as previously)
            // imposes the derivative at a different location and is wrong on
            // non-uniform grids (see
            // `neumann_preserves_linear_profile_on_nonuniform_grid`).
            let h = grid.h_left(1); // grid[1] - grid[0]
            let scale = -lower[0] * h;
            main[0] += lower[0];
            lower[0] = 0.0;
            scale
        }
        BoundaryCondition::Linear | BoundaryCondition::LinearInExp => {
            if upper.len() > 1 {
                // Continue the slope between the first two interior nodes
                // over the boundary cell, whose width need not match theirs.
                let ratio = bc.extrapolation_ratio(grid.h_left(1), grid.h_right(1), true);
                main[0] += (1.0 + ratio) * lower[0];
                upper[0] -= ratio * lower[0];
            } else {
                main[0] += lower[0];
            }
            lower[0] = 0.0;
            0.0
        }
        // Row 0 is the edge node's own equation, supplied by the caller: it
        // has no boundary value to eliminate.
        BoundaryCondition::Degenerate => 0.0,
    };
    (scale, scale * boundary_scalar(bc))
}

/// Apply Dirichlet/Neumann/Linear boundary condition at the upper edge.
///
/// Returns `(scale, rhs)` for the last interior point.
fn apply_upper_boundary(
    bc: BoundaryCondition,
    lower: &mut [f64],
    main: &mut [f64],
    upper: &mut [f64],
    grid: &Grid1D,
    n: usize,
) -> (f64, f64) {
    let last = n - 1;
    let scale = match bc {
        BoundaryCondition::Dirichlet(_) => {
            let scale = upper[last];
            upper[last] = 0.0;
            scale
        }
        BoundaryCondition::Neumann(_) => {
            // du/dx at x_max = g, imposed one-sided at the boundary node:
            // u[n_grid-1] = u_last + h_n*g, h_n = grid[n-1] - grid[n-2].
            // Mirrors the lower-edge elimination and matches the
            // reconstruction in `boundary_value`/`fill_boundaries`.
            let h = grid.h_right(grid.n() - 2); // grid[n-1] - grid[n-2]
            let scale = upper[last] * h;
            main[last] += upper[last];
            upper[last] = 0.0;
            scale
        }
        BoundaryCondition::Linear | BoundaryCondition::LinearInExp => {
            if last > 0 {
                let interior = grid.n() - 2;
                let ratio =
                    bc.extrapolation_ratio(grid.h_right(interior), grid.h_left(interior), false);
                main[last] += (1.0 + ratio) * upper[last];
                lower[last] -= ratio * upper[last];
            } else {
                main[last] += upper[last];
            }
            upper[last] = 0.0;
            0.0
        }
        // Rejected by the solvers before assembly; the last row is left as
        // supplied.
        BoundaryCondition::Degenerate => 0.0,
    };
    (scale, scale * boundary_scalar(bc))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple constant-coefficient PDE: u_t = u_xx (heat equation)
    struct HeatEquation;

    impl PdeProblem1D for HeatEquation {
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
            x * x
        }
        fn lower_boundary(&self, _t: f64) -> BoundaryCondition {
            BoundaryCondition::Dirichlet(0.0)
        }
        fn upper_boundary(&self, _t: f64) -> BoundaryCondition {
            BoundaryCondition::Dirichlet(1.0)
        }
    }

    /// A lower edge row with a third weight is solved by eliminating that
    /// weight against row 1: `solve_thomas` must invert exactly what `apply`
    /// applies.
    #[test]
    fn thomas_inverts_operator_with_lower_edge_coupling() {
        let grid = Grid1D::from_points(vec![0.0, 0.1, 0.25, 0.45, 0.7, 1.0]).expect("grid");
        let n = grid.n() - 1;
        let lower = vec![0.0, 3.0, 2.5, 2.0, 1.5];
        let main = vec![-4.0, -7.0, -6.0, -5.0, -4.0];
        let upper = vec![5.0, 3.5, 3.0, 2.5, 2.0];
        let op = TridiagOperator::from_parts(
            lower,
            main,
            upper,
            vec![0.0; n],
            BoundaryCondition::Degenerate,
            BoundaryCondition::Dirichlet(0.0),
            &grid,
        )
        .with_lower_edge_coupling(-1.25);

        let x = [1.0, -0.5, 2.0, 0.25, -1.5];
        let mut applied = vec![0.0; n];
        op.apply(&x, &mut applied);
        // Row 0 carries the entry outside the band.
        assert!((applied[0] - (-4.0 * 1.0 + 5.0 * -0.5 - 1.25 * 2.0)).abs() < 1e-14);

        let alpha = 0.3;
        let rhs: Vec<f64> = x
            .iter()
            .zip(&applied)
            .map(|(x, ax)| x - alpha * ax)
            .collect();
        let solved = op.solve_thomas(alpha, &rhs).expect("edge-row solve");
        for (solved, expected) in solved.iter().zip(&x) {
            assert!((solved - expected).abs() < 1e-12, "{solved} != {expected}");
        }
    }

    /// The upwind branch must be untouched by the stencil split, and the
    /// central stencil must differentiate a quadratic exactly on a
    /// non-uniform grid even with no diffusion.
    #[test]
    fn central_stencil_is_exact_for_quadratics_without_diffusion() {
        let (h_m, h_p, b) = (0.2, 0.5, 1.7);
        let (lower, main, upper) = central_stencil(0.0, b, h_m, h_p);
        let u = |x: f64| 3.0 - 2.0 * x + 4.0 * x * x;
        let x0 = 0.9;
        let approx = lower * u(x0 - h_m) + main * u(x0) + upper * u(x0 + h_p);
        assert!((approx - b * (-2.0 + 8.0 * x0)).abs() < 1e-12);
        // With no diffusion the monotone stencil is one-sided instead.
        assert_eq!(node_stencil(0.0, b, h_m, h_p), (0.0, -b / h_p, b / h_p));
    }

    #[test]
    fn operator_assembly_size() {
        let grid = Grid1D::uniform(0.0, 1.0, 11).expect("valid grid");
        let op = TridiagOperator::assemble(&HeatEquation, &grid, 0.0);
        assert_eq!(op.n, 9); // 11 points - 2 boundaries
    }

    #[test]
    fn thomas_solve_identity() {
        // With alpha=0, solve (I)*x = rhs → x = rhs
        let grid = Grid1D::uniform(0.0, 1.0, 5).expect("valid grid");
        let op = TridiagOperator::assemble(&HeatEquation, &grid, 0.0);
        let rhs = vec![1.0, 2.0, 3.0];
        let x = op
            .solve_thomas(0.0, &rhs)
            .expect("alpha=0 gives the identity — pivots are all 1");
        for (xi, ri) in x.iter().zip(rhs.iter()) {
            assert!((xi - ri).abs() < 1e-14);
        }
    }

    /// [P6-5] `solve_thomas` must reject a degenerate pivot with
    /// [`PdeError::DegeneratePivot`] rather than dividing by it and
    /// returning a silent `inf` / `NaN` solution.
    ///
    /// A `(I - alpha*A)` system whose first pivot `d[0] = 1 - alpha*main[0]`
    /// is engineered to vanish (`alpha * main[0] == 1`) is the simplest
    /// degenerate case. The constructor below builds an operator directly
    /// from diagonals so the pivot can be forced to zero.
    #[test]
    fn thomas_rejects_degenerate_pivot() {
        // 3x3 tridiagonal operator. With alpha = 1 and main[0] = 1, the first
        // pivot d[0] = 1 - 1*1 = 0 — a genuine zero pivot.
        let grid = Grid1D::uniform(0.0, 1.0, 5).expect("valid grid");
        let op = TridiagOperator::from_parts(
            vec![0.0, 0.1, 0.1],
            vec![1.0, 2.0, 2.0],
            vec![0.1, 0.1, 0.0],
            vec![0.0, 0.0, 0.0],
            BoundaryCondition::Dirichlet(0.0),
            BoundaryCondition::Dirichlet(0.0),
            &grid,
        );
        let rhs = vec![1.0, 1.0, 1.0];

        let result = op.solve_thomas(1.0, &rhs);
        match result {
            Err(PdeError::DegeneratePivot { row, pivot, .. }) => {
                assert_eq!(row, 0, "the engineered zero pivot is at row 0");
                assert!(
                    pivot.abs() < 1e-12,
                    "the reported pivot must be (near) zero, got {pivot}"
                );
            }
            other => panic!("expected DegeneratePivot, got {other:?}"),
        }

        // The buffer variant must reject it identically.
        let mut out = vec![0.0; 3];
        let mut scratch = vec![0.0; 3];
        assert!(
            matches!(
                op.solve_thomas_into(1.0, &rhs, &mut out, &mut scratch),
                Err(PdeError::DegeneratePivot { .. })
            ),
            "solve_thomas_into must also reject the degenerate pivot"
        );
    }

    /// A well-conditioned diagonally-dominant system must still solve cleanly
    /// — the pivot guard must not false-positive on a healthy operator.
    #[test]
    fn thomas_accepts_well_conditioned_system() {
        let grid = Grid1D::uniform(0.0, 1.0, 5).expect("valid grid");
        let op = TridiagOperator::assemble(&HeatEquation, &grid, 0.0);
        // alpha small and positive → (I - alpha*A) is strongly diagonally
        // dominant, pivots ≈ 1.
        let rhs = vec![0.3, 0.5, 0.7];
        let x = op
            .solve_thomas(0.01, &rhs)
            .expect("a diagonally-dominant implicit operator must solve cleanly");
        assert_eq!(x.len(), 3);
        for xi in &x {
            assert!(xi.is_finite(), "solution must be finite");
        }
    }
}

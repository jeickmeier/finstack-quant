//! Tridiagonal spatial operator and Thomas algorithm.
//!
//! Discretizes a 1D PDE on a [`Grid1D`] into a tridiagonal matrix using
//! second-order accurate finite difference stencils on (possibly non-uniform)
//! grids. The Thomas algorithm solves the resulting system in O(n) time.

use super::boundary::BoundaryCondition;
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

/// Error raised by the tridiagonal Thomas solver.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ThomasError {
    /// A pivot in the Thomas forward elimination became degenerate — either
    /// non-finite, or negligibly small relative to the terms that formed it.
    ///
    /// The Thomas algorithm divides by each pivot; a degenerate pivot makes
    /// the solve produce silent `inf` / `NaN`. For the implicit operator
    /// `(I - alpha * A)` this signals that diagonal dominance has been
    /// eroded — most often by a Neumann / Linear boundary-condition
    /// modification on a near-degenerate row — so the solve cannot be trusted.
    #[error(
        "Thomas solver: degenerate pivot at row {row} of {n}: |pivot| = {pivot:e} \
         is negligible relative to the row scale {row_scale:e} (or non-finite); \
         the implicit operator (I - alpha*A) has lost diagonal dominance — \
         check the boundary conditions or grid"
    )]
    DegeneratePivot {
        /// Row index of the degenerate pivot.
        row: usize,
        /// Total number of interior rows.
        n: usize,
        /// The degenerate pivot value.
        pivot: f64,
        /// Magnitude of the terms that formed the pivot.
        row_scale: f64,
    },
}

/// Check that a freshly computed Thomas pivot `d` (formed as the difference of
/// terms whose magnitudes are `term_a` and `term_b`) is non-degenerate.
///
/// Returns [`ThomasError::DegeneratePivot`] if `d` is non-finite or its
/// magnitude has collapsed below [`THOMAS_PIVOT_REL_EPS`] times the row scale.
#[inline]
fn check_pivot(d: f64, term_a: f64, term_b: f64, row: usize, n: usize) -> Result<(), ThomasError> {
    // Row scale: largest constituent magnitude, floored at 1.0 so that a row
    // whose terms are all genuinely tiny still requires a non-zero pivot.
    let row_scale = term_a.abs().max(term_b.abs()).max(1.0);
    if !d.is_finite() || d.abs() <= THOMAS_PIVOT_REL_EPS * row_scale {
        return Err(ThomasError::DegeneratePivot {
            row,
            n,
            pivot: d,
            row_scale,
        });
    }
    Ok(())
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
    /// RHS correction from lower boundary at the first interior point.
    bc_lower_rhs: f64,
    /// RHS correction from upper boundary at the last interior point.
    bc_upper_rhs: f64,
    /// Number of interior points.
    n: usize,
}

impl TridiagOperator {
    /// Assemble the tridiagonal operator from PDE coefficients at time `t`.
    ///
    /// Discretizes `a(x,t) u'' + b(x,t) u' + c(x,t) u + f(x,t)` on the
    /// given grid using second-order accurate stencils for non-uniform spacing.
    /// Boundary conditions modify the first and last rows.
    pub fn assemble(problem: &dyn PdeProblem1D, grid: &Grid1D, t: f64) -> Self {
        let n = grid.n_interior();
        let mut lower = vec![0.0; n];
        let mut main = vec![0.0; n];
        let mut upper = vec![0.0; n];
        let mut source = vec![0.0; n];

        // Assemble interior stencils (grid indices 1..n_grid-1 map to operator indices 0..n-1)
        for k in 0..n {
            let i = k + 1; // grid index
            let x = grid.points()[i];
            let h_m = grid.h_left(i);
            let h_p = grid.h_right(i);
            let h_sum = h_m + h_p;

            let a = problem.diffusion(x, t);
            let b = problem.convection(x, t);
            let c = problem.reaction(x, t);

            // Second derivative stencil: a * u''
            // u[i-1]: 2a / (h_m * h_sum)
            // u[i]:  -2a / (h_m * h_p)
            // u[i+1]: 2a / (h_p * h_sum)

            // First derivative stencil (central, non-uniform): b * u'
            // u[i-1]: -b * h_p / (h_m * h_sum)
            // u[i]:    b * (h_p - h_m) / (h_m * h_p)
            // u[i+1]:  b * h_m / (h_p * h_sum)

            lower[k] = 2.0 * a / (h_m * h_sum) - b * h_p / (h_m * h_sum);
            main[k] = -2.0 * a / (h_m * h_p) + b * (h_p - h_m) / (h_m * h_p) + c;
            upper[k] = 2.0 * a / (h_p * h_sum) + b * h_m / (h_p * h_sum);
            source[k] = problem.source(x, t);
        }

        // Apply boundary conditions
        let bc_lower = problem.lower_boundary(t);
        let bc_upper = problem.upper_boundary(t);

        let bc_lower_rhs = apply_lower_boundary(bc_lower, &mut lower, &mut main, &mut upper, grid);
        let bc_upper_rhs =
            apply_upper_boundary(bc_upper, &mut lower, &mut main, &mut upper, grid, n);

        Self {
            lower,
            main,
            upper,
            source,
            bc_lower_rhs,
            bc_upper_rhs,
            n,
        }
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

        let bc_lower_rhs = apply_lower_boundary(bc_lower, &mut lower, &mut main, &mut upper, grid);
        let bc_upper_rhs =
            apply_upper_boundary(bc_upper, &mut lower, &mut main, &mut upper, grid, n);

        Self {
            lower,
            main,
            upper,
            source,
            bc_lower_rhs,
            bc_upper_rhs,
            n,
        }
    }

    /// Number of interior points this operator covers.
    #[inline]
    pub fn n(&self) -> usize {
        self.n
    }

    /// Compute `y = A * x + source` for interior points.
    ///
    /// `x` has length `n` (interior values only). Boundary contributions are
    /// added from the stored corrections.
    pub fn apply(&self, x: &[f64]) -> Vec<f64> {
        debug_assert_eq!(x.len(), self.n);
        let mut y = vec![0.0; self.n];

        if self.n == 0 {
            return y;
        }

        // First row
        y[0] = self.main[0] * x[0] + self.source[0] + self.bc_lower_rhs;
        if self.n > 1 {
            y[0] += self.upper[0] * x[1];
        }

        // Interior rows
        for i in 1..self.n.saturating_sub(1) {
            y[i] = self.lower[i] * x[i - 1]
                + self.main[i] * x[i]
                + self.upper[i] * x[i + 1]
                + self.source[i];
        }

        // Last row
        if self.n > 1 {
            let last = self.n - 1;
            y[last] = self.lower[last] * x[last - 1]
                + self.main[last] * x[last]
                + self.source[last]
                + self.bc_upper_rhs;
        } else {
            // n == 1: single interior point gets both boundary corrections
            y[0] += self.bc_upper_rhs;
        }

        y
    }

    /// Compute `y = A * x + source` for interior points, writing into `out`.
    ///
    /// Like [`apply`](Self::apply) but writes into a pre-allocated slice
    /// instead of allocating.
    pub fn apply_into(&self, x: &[f64], out: &mut [f64]) {
        debug_assert_eq!(x.len(), self.n);
        debug_assert_eq!(out.len(), self.n);

        if self.n == 0 {
            return;
        }

        // First row
        out[0] = self.main[0] * x[0] + self.source[0] + self.bc_lower_rhs;
        if self.n > 1 {
            out[0] += self.upper[0] * x[1];
        }

        // Interior rows
        for i in 1..self.n.saturating_sub(1) {
            out[i] = self.lower[i] * x[i - 1]
                + self.main[i] * x[i]
                + self.upper[i] * x[i + 1]
                + self.source[i];
        }

        // Last row
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
    /// [`ThomasError::DegeneratePivot`] if the forward elimination hits a
    /// degenerate pivot (a sign that `(I - alpha*A)` has lost diagonal
    /// dominance, e.g. via a boundary-condition modification). Without the
    /// guard such a pivot would yield a silent `inf` / `NaN` solution.
    pub fn solve_thomas(&self, alpha: f64, rhs: &[f64]) -> Result<Vec<f64>, ThomasError> {
        debug_assert_eq!(rhs.len(), self.n);

        if self.n == 0 {
            return Ok(vec![]);
        }

        let n = self.n;
        let mut x = vec![0.0; n];

        // Build the modified system: (I - alpha * A)
        // Diagonal:      d[i] = 1 - alpha * main[i]
        // Sub-diagonal:  a[i] = -alpha * lower[i]
        // Super-diagonal: c[i] = -alpha * upper[i]

        // Forward elimination (in-place using temporary vectors)
        let mut d = vec![0.0; n]; // modified diagonal
        let mut r = vec![0.0; n]; // modified RHS

        d[0] = 1.0 - alpha * self.main[0];
        // The first pivot is formed from `1` and `alpha * main[0]`.
        check_pivot(d[0], 1.0, alpha * self.main[0], 0, n)?;
        r[0] = rhs[0];

        for i in 1..n {
            let a_i = -alpha * self.lower[i];
            let c_prev = -alpha * self.upper[i - 1];
            let w = a_i / d[i - 1];
            let diag = 1.0 - alpha * self.main[i];
            let off = w * c_prev;
            d[i] = diag - off;
            check_pivot(d[i], diag, off, i, n)?;
            r[i] = rhs[i] - w * r[i - 1];
        }

        // Back-substitution (every pivot has been verified non-degenerate).
        x[n - 1] = r[n - 1] / d[n - 1];
        for i in (0..n - 1).rev() {
            let c_i = -alpha * self.upper[i];
            x[i] = (r[i] - c_i * x[i + 1]) / d[i];
        }

        Ok(x)
    }

    /// Solve `(I - alpha * A) * x_new = rhs` using the Thomas algorithm,
    /// writing the result into `out`.
    ///
    /// Like [`solve_thomas`](Self::solve_thomas) but avoids allocating the
    /// output vector. Returns [`ThomasError::DegeneratePivot`] on a
    /// degenerate pivot.
    pub fn solve_thomas_into(
        &self,
        alpha: f64,
        rhs: &[f64],
        out: &mut [f64],
    ) -> Result<(), ThomasError> {
        debug_assert_eq!(rhs.len(), self.n);
        debug_assert_eq!(out.len(), self.n);

        if self.n == 0 {
            return Ok(());
        }

        let n = self.n;
        let mut d = vec![0.0; n];
        let mut r = vec![0.0; n];

        d[0] = 1.0 - alpha * self.main[0];
        check_pivot(d[0], 1.0, alpha * self.main[0], 0, n)?;
        r[0] = rhs[0];

        for i in 1..n {
            let a_i = -alpha * self.lower[i];
            let c_prev = -alpha * self.upper[i - 1];
            let w = a_i / d[i - 1];
            let diag = 1.0 - alpha * self.main[i];
            let off = w * c_prev;
            d[i] = diag - off;
            check_pivot(d[i], diag, off, i, n)?;
            r[i] = rhs[i] - w * r[i - 1];
        }

        out[n - 1] = r[n - 1] / d[n - 1];
        for i in (0..n - 1).rev() {
            let c_i = -alpha * self.upper[i];
            out[i] = (r[i] - c_i * out[i + 1]) / d[i];
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

        // First row
        y[0] = x[0] + beta * (self.main[0] * x[0] + self.source[0] + self.bc_lower_rhs);
        if self.n > 1 {
            y[0] += beta * self.upper[0] * x[1];
        }

        // Interior rows
        for i in 1..self.n.saturating_sub(1) {
            y[i] = x[i]
                + beta
                    * (self.lower[i] * x[i - 1]
                        + self.main[i] * x[i]
                        + self.upper[i] * x[i + 1]
                        + self.source[i]);
        }

        // Last row
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

        // Add source terms
        for (source_val, rhs_val) in self.source.iter().zip(rhs.iter_mut()) {
            *rhs_val += alpha * source_val;
        }

        // Add boundary corrections
        rhs[0] += alpha * self.bc_lower_rhs;
        if self.n > 1 {
            rhs[self.n - 1] += alpha * self.bc_upper_rhs;
        } else {
            rhs[0] += alpha * self.bc_upper_rhs;
        }
    }
}

/// Apply Dirichlet/Neumann/Linear boundary condition at the lower edge.
///
/// Returns the RHS correction for the first interior point.
fn apply_lower_boundary(
    bc: BoundaryCondition,
    lower: &mut [f64],
    main: &mut [f64],
    upper: &mut [f64],
    grid: &Grid1D,
) -> f64 {
    match bc {
        BoundaryCondition::Dirichlet(g) => {
            // The stencil at the first interior point (k=0) has coupling lower[0]
            // to the boundary. Since u[0] = g, shift to RHS: rhs += lower[0] * g
            let correction = lower[0] * g;
            lower[0] = 0.0; // no coupling in the tridiagonal system
            correction
        }
        BoundaryCondition::Neumann(g) => {
            // du/dx at x_min = g
            // Ghost node: u[-1] ≈ u[1] - 2*h_1*g, where h_1 = x[1] - x[0]
            // Substitute into the stencil for the first interior point:
            // lower[0]*u[-1] → lower[0]*(u[1] - 2*h_1*g)
            // Main diagonal gets: main[0] stays same
            // Upper gets: upper[0] += lower[0]
            // RHS correction: -lower[0] * 2 * h_1 * g
            let h = grid.h_left(1); // grid[1] - grid[0]
            let correction = -lower[0] * 2.0 * h * g;
            upper[0] += lower[0];
            lower[0] = 0.0;
            correction
        }
        BoundaryCondition::Linear => {
            // d²u/dx² = 0 at boundary → u[0] = 2*u[1] - u[2]
            // Substitute into stencil for first interior point (k=0, grid i=1):
            // lower[0]*u[0] → lower[0]*(2*u[1] - u[2])
            // main[0] += 2*lower[0]
            // upper[0] -= lower[0]
            main[0] += 2.0 * lower[0];
            if upper.len() > 1 {
                upper[0] -= lower[0];
            }
            lower[0] = 0.0;
            0.0
        }
    }
}

/// Apply Dirichlet/Neumann/Linear boundary condition at the upper edge.
///
/// Returns the RHS correction for the last interior point.
fn apply_upper_boundary(
    bc: BoundaryCondition,
    lower: &mut [f64],
    main: &mut [f64],
    upper: &mut [f64],
    grid: &Grid1D,
    n: usize,
) -> f64 {
    let last = n - 1;
    match bc {
        BoundaryCondition::Dirichlet(g) => {
            let correction = upper[last] * g;
            upper[last] = 0.0;
            correction
        }
        BoundaryCondition::Neumann(g) => {
            // du/dx at x_max = g
            // Ghost node: u[n+1] ≈ u[n-1] + 2*h_n*g
            let h = grid.h_right(grid.n() - 2); // grid[n-1] - grid[n-2]
            let correction = upper[last] * 2.0 * h * g;
            lower[last] += upper[last];
            upper[last] = 0.0;
            correction
        }
        BoundaryCondition::Linear => {
            // d²u/dx² = 0 → u[n] = 2*u[n-1] - u[n-2]
            main[last] += 2.0 * upper[last];
            if last > 0 {
                lower[last] -= upper[last];
            }
            upper[last] = 0.0;
            0.0
        }
    }
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

    #[test]
    fn operator_assembly_size() {
        let grid = Grid1D::uniform(0.0, 1.0, 11).expect("valid grid");
        let op = TridiagOperator::assemble(&HeatEquation, &grid, 0.0);
        assert_eq!(op.n(), 9); // 11 points - 2 boundaries
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
    /// [`ThomasError::DegeneratePivot`] rather than dividing by it and
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
            Err(ThomasError::DegeneratePivot { row, pivot, .. }) => {
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
        assert!(
            matches!(
                op.solve_thomas_into(1.0, &rhs, &mut out),
                Err(ThomasError::DegeneratePivot { .. })
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

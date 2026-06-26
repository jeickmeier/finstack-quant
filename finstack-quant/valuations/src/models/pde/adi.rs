//! Modified Craig-Sneyd (MCS) ADI time-stepping for 2D PDEs.
//!
//! Implements the Modified Craig-Sneyd (MCS) scheme of In 't Hout & Welfert
//! (2009) for solving 2D convection-diffusion-reaction PDEs with a mixed
//! (cross-derivative) term. Each time step splits into fractional steps along
//! each spatial direction; the cross-derivative term is treated explicitly.
//!
//! # Algorithm
//!
//! Write the spatial operator as `F = F_0 + F_1 + F_2`, where `F_0` is the
//! mixed (cross-derivative) term `A_xy`, `F_1` is the x-direction term `A_x`,
//! and `F_2` is the y-direction term `A_y`. With a fixed parameter `theta`,
//! one MCS step from `u^n` (at `t_n`) to `u^{n+1}` (at `t_{n+1}`) is
//! (In 't Hout & Welfert 2009; In 't Hout & Mishra 2010, eq. 1.4):
//!
//! ```text
//! Y_0   = u^n + dt * F(t_n, u^n)                                  [predictor]
//! Y_j   = Y_{j-1} + theta*dt * (F_j(t_{n+1},Y_j) - F_j(t_n,u^n))   j = 1,2
//! Yhat0 = Y_0 + theta*dt * (F_0(t_{n+1},Y_2) - F_0(t_n,u^n))       [mixed corr.]
//! Ytld0 = Yhat0 + (1/2 - theta)*dt * (F(t_{n+1},Y_2) - F(t_n,u^n)) [MCS corr.]
//! Ytld_j = Ytld_{j-1} + theta*dt * (F_j(t_{n+1},Ytld_j) - F_j(t_n,u^n))  j = 1,2
//! u^{n+1} = Ytld_2
//! ```
//!
//! The `Y_j` lines (first predictor plus two implicit unidirectional
//! correctors) alone are the first-order Douglas scheme. The `Yhat0`
//! mixed-term corrector plus the second pair of implicit sweeps `Ytld_j`
//! upgrade Douglas to the second-order MCS scheme. With `theta = 1/2` the
//! `Ytld0` line vanishes and MCS reduces to the plain Craig-Sneyd (CS) scheme.
//!
//! # Stability and the choice of `theta`
//!
//! The corrector-less Douglas scheme is unconditionally stable only for
//! `theta >= 1/2`; at smaller `theta` it is inadmissible even for pure
//! diffusion. The MCS corrector lowers the admissible bound: the MCS scheme is
//! unconditionally stable (von Neumann) for `theta >= 1/3` in the pure
//! 2D-diffusion case, and for `theta >= 2/5` in the general
//! convection-diffusion case (In 't Hout & Mishra 2010). This stepper uses
//! `theta = 1/3`: it is second-order accurate and is the standard literature
//! choice for the Heston PDE (In 't Hout & Foulon 2010). The `2/5` bound is a
//! worst-case convection-diffusion result; for the Heston PDE `1/3` remains
//! stable in practice because the Rannacher start damps the non-smooth payoff,
//! the Péclet numbers are modest (the convection terms `r - q - v/2` and
//! `kappa(theta - v)` are mild), and the mixed term is diffusion-like.
//!
//! The implicit sweeps are tridiagonal solves reusing the 1D Thomas algorithm.
//!
//! # References
//!
//! - In 't Hout, K. J. & Welfert, B. D. (2009). "Unconditional stability of
//!   second-order ADI schemes applied to multi-dimensional diffusion equations
//!   with mixed derivative terms." *Applied Numerical Mathematics*, 59(3-4).
//! - In 't Hout, K. J. & Mishra, C. (2010). "Stability of the Modified
//!   Craig-Sneyd scheme for two-dimensional convection-diffusion equations
//!   with mixed derivative term." (arXiv:1011.6528) — gives the MCS scheme in
//!   the form (eq. 1.4) reproduced above.
//! - Craig, I. J. D. & Sneyd, A. D. (1988). "An alternating-direction implicit
//!   scheme for parabolic equations with mixed derivatives."

use super::boundary::BoundaryCondition;
use super::grid2d::Grid2D;
use super::operator2d::{apply_cross_derivative_into, Operators2D};
use super::problem2d::PdeProblem2D;
use super::stepper::StepperError;

/// Reusable per-step scratch buffers for the Modified Craig-Sneyd ADI stepper.
///
/// Sized to the largest grid the caller intends to step. Reusing this across
/// timesteps (and across grids of identical shape) lets `Solver2D` skip the
/// per-call allocations the previous implementation performed every call to
/// [`CraigSneydStepper::step_with_buffers`].
#[derive(Clone, Default)]
pub struct AdiWorkBuffers {
    x_line: Vec<f64>,
    y_line: Vec<f64>,
    line_out: Vec<f64>,
    rhs_buf: Vec<f64>,
    /// Predictor `Y_0` (interior).
    y0: Vec<f64>,
    /// First x-sweep result `Y_1` (interior).
    y1: Vec<f64>,
    /// First y-sweep result `Y_2` (interior).
    y2: Vec<f64>,
    /// MCS corrector iterate `Ytld` (interior): `Ytld_0`, then `Ytld_1`.
    ytld: Vec<f64>,
    /// `F_1(t_n, u^n) = A_x * u^n` (interior).
    ax_u: Vec<f64>,
    /// `F_2(t_n, u^n) = A_y * u^n` (interior).
    ay_u: Vec<f64>,
    /// `F_1(t_{n+1}, Y_2) = A_x * Y_2` (interior), for the MCS corrector.
    ax_y2: Vec<f64>,
    /// `F_2(t_{n+1}, Y_2) = A_y * Y_2` (interior), for the MCS corrector.
    ay_y2: Vec<f64>,
    /// `F_0(t_n, u^n)`: cross-derivative term at `u^n` (interior).
    cross: Vec<f64>,
    /// `F_0(t_{n+1}, Y_2)`: cross-derivative term at `Y_2` (interior).
    cross_y2: Vec<f64>,
}

impl AdiWorkBuffers {
    /// Pre-allocate buffers sized to a particular grid.
    pub fn for_grid(grid: &Grid2D) -> Self {
        let nx_int = grid.nx_interior();
        let ny_int = grid.ny_interior();
        let interior = nx_int * ny_int;
        let line_max = nx_int.max(ny_int);
        Self {
            x_line: vec![0.0; nx_int],
            y_line: vec![0.0; ny_int],
            line_out: vec![0.0; line_max],
            rhs_buf: vec![0.0; line_max],
            y0: vec![0.0; interior],
            y1: vec![0.0; interior],
            y2: vec![0.0; interior],
            ytld: vec![0.0; interior],
            ax_u: vec![0.0; interior],
            ay_u: vec![0.0; interior],
            ax_y2: vec![0.0; interior],
            ay_y2: vec![0.0; interior],
            cross: vec![0.0; interior],
            cross_y2: vec![0.0; interior],
        }
    }

    /// Resize all buffers in place if the grid shape changed.
    fn resize_for(&mut self, grid: &Grid2D) {
        let nx_int = grid.nx_interior();
        let ny_int = grid.ny_interior();
        let interior = nx_int * ny_int;
        let line_max = nx_int.max(ny_int);
        if self.x_line.len() != nx_int {
            self.x_line.resize(nx_int, 0.0);
        }
        if self.y_line.len() != ny_int {
            self.y_line.resize(ny_int, 0.0);
        }
        if self.line_out.len() != line_max {
            self.line_out.resize(line_max, 0.0);
        }
        if self.rhs_buf.len() != line_max {
            self.rhs_buf.resize(line_max, 0.0);
        }
        if self.y0.len() != interior {
            self.y0.resize(interior, 0.0);
        }
        if self.y1.len() != interior {
            self.y1.resize(interior, 0.0);
        }
        if self.y2.len() != interior {
            self.y2.resize(interior, 0.0);
        }
        if self.ytld.len() != interior {
            self.ytld.resize(interior, 0.0);
        }
        if self.ax_u.len() != interior {
            self.ax_u.resize(interior, 0.0);
        }
        if self.ay_u.len() != interior {
            self.ay_u.resize(interior, 0.0);
        }
        if self.ax_y2.len() != interior {
            self.ax_y2.resize(interior, 0.0);
        }
        if self.ay_y2.len() != interior {
            self.ay_y2.resize(interior, 0.0);
        }
        if self.cross.len() != interior {
            self.cross.resize(interior, 0.0);
        }
        if self.cross_y2.len() != interior {
            self.cross_y2.resize(interior, 0.0);
        }
    }
}

/// Default MCS `theta`: second-order accurate and unconditionally stable for
/// `theta >= 1/3` (pure diffusion) / `theta >= 2/5` (general
/// convection-diffusion), per In 't Hout & Mishra (2010). `1/3` is the
/// standard choice for the Heston PDE.
const MCS_THETA: f64 = 1.0 / 3.0;

/// Cell-Péclet ceiling for the 2D MCS ADI stepper.
///
/// The cell Péclet number `Pe = |b| * h / (2 * a)` measures local
/// convection-vs-diffusion. MCS at `theta = 1/3` is unconditionally stable
/// for *pure 2D diffusion*, but for the *general convection-diffusion* case
/// the von Neumann stability bound rises to `theta >= 2/5` (In 't Hout &
/// Mishra 2010). In the strongly convection-dominated regime — large Heston
/// `kappa`, very wide variance grids, or coarse spacing — the `theta = 1/3`
/// scheme leaves its proven-stable envelope and can diverge silently.
///
/// `4.0` is chosen empirically. Representative production Heston grids
/// (ATM 1y, put-call parity, strong-correlation reconciliation cases) peak at
/// `Pe ≈ 1` — comfortably below this ceiling. A pathological large-`kappa`
/// configuration (`kappa = 10`) reaches `Pe ≈ 5` and `kappa = 20` reaches
/// `Pe ≈ 10`; both are flagged. The ceiling therefore sits well above the
/// convection-dominated onset (`Pe ~ 1`, where a central-difference
/// off-diagonal first changes sign) yet below the genuinely pathological
/// regime, so it never false-positives a normal Heston solve while catching
/// the cases the scheme cannot reliably handle.
const MCS_PECLET_MAX: f64 = 4.0;

/// Check the 2D grid is not so convection-dominated that the `theta = 1/3`
/// MCS scheme leaves its reliably-stable regime.
///
/// Scans every interior node and computes the cell Péclet number in each
/// direction, `Pe = |b| * h / (2 * a)`, using the *larger* of the two
/// neighbour spacings (the conservative cell width — it is the spacing that
/// governs whether a central-difference off-diagonal flips sign). If the
/// largest `Pe` exceeds [`MCS_PECLET_MAX`] the step is rejected with
/// [`StepperError::PecletViolation`] rather than risking silent divergence.
///
/// A node whose diffusion `a` is exactly zero while convection is non-zero
/// is treated as infinitely convection-dominated and flagged.
fn check_peclet(problem: &dyn PdeProblem2D, grid: &Grid2D, t: f64) -> Result<(), StepperError> {
    let x_pts = grid.x().points();
    let y_pts = grid.y().points();

    let mut worst: Option<(&'static str, f64, f64, f64, f64)> = None;
    let mut consider = |dir: &'static str, b: f64, a: f64, h: f64| {
        let b = b.abs();
        if b == 0.0 {
            return; // no convection here — no Péclet constraint
        }
        // Pe = |b| h / (2a); a == 0 with b != 0 is infinite Péclet.
        let pe = if a > 0.0 {
            b * h / (2.0 * a)
        } else {
            f64::INFINITY
        };
        if worst.map(|(_, w, ..)| pe > w).unwrap_or(true) {
            worst = Some((dir, pe, b, a, h));
        }
    };

    // Iterate the interior nodes (grid indices 1..n-1). `x` / `y` come from
    // the point iterators; the index is used only for the spacing helpers.
    for (i, &x) in x_pts.iter().enumerate().take(grid.nx() - 1).skip(1) {
        let hx = grid.x().h_left(i).max(grid.x().h_right(i));
        for (j, &y) in y_pts.iter().enumerate().take(grid.ny() - 1).skip(1) {
            let hy = grid.y().h_left(j).max(grid.y().h_right(j));
            consider(
                "x",
                problem.convection_x(x, y, t),
                problem.diffusion_xx(x, y, t),
                hx,
            );
            consider(
                "y",
                problem.convection_y(x, y, t),
                problem.diffusion_yy(x, y, t),
                hy,
            );
        }
    }

    if let Some((direction, peclet, convection, diffusion, spacing)) = worst {
        if peclet > MCS_PECLET_MAX {
            return Err(StepperError::PecletViolation {
                direction,
                peclet,
                pe_max: MCS_PECLET_MAX,
                convection,
                diffusion,
                spacing,
            });
        }
    }
    Ok(())
}

/// Modified Craig-Sneyd (MCS) ADI time stepper for 2D problems.
///
/// Uses `theta = 1/3` (MCS theta). Optionally applies Rannacher-style
/// smoothing: fully-implicit (`theta = 1.0`) steps near the terminal condition
/// to damp the non-smooth payoff, then `theta = 1/3` for the remaining steps.
///
/// The type name is retained for API stability; the implemented scheme is the
/// Modified Craig-Sneyd scheme (plain Craig-Sneyd is the `theta = 1/2` special
/// case — see the module documentation).
pub struct CraigSneydStepper {
    /// MCS `theta` parameter for the implicit weight.
    theta: f64,
    /// Number of initial fully-implicit steps (Rannacher smoothing).
    implicit_start_steps: usize,
    /// Total number of time steps.
    n_steps: usize,
    /// Whether to apply the MCS corrector stages (`Yhat0` / `Ytld_j`).
    ///
    /// Always `true` in production: the full Modified Craig-Sneyd scheme.
    /// A test-only constructor sets this `false` to obtain the bare Douglas
    /// scheme, used to demonstrate that the corrector is what makes
    /// `theta = 1/3` an admissible, second-order-accurate scheme (bare Douglas
    /// is only unconditionally stable for `theta >= 1/2`).
    apply_mcs_corrector: bool,
}

impl CraigSneydStepper {
    /// Modified Craig-Sneyd ADI stepper with the standard `theta = 1/3`.
    pub fn new(n_steps: usize) -> Self {
        Self {
            theta: MCS_THETA,
            implicit_start_steps: 0,
            n_steps,
            apply_mcs_corrector: true,
        }
    }

    /// Modified Craig-Sneyd with Rannacher-style smoothing: use `theta = 1.0`
    /// (fully implicit) for the first `implicit_start` steps, then switch to
    /// `theta = 1/3`.
    pub fn with_rannacher(implicit_start: usize, n_steps: usize) -> Self {
        Self {
            theta: MCS_THETA,
            implicit_start_steps: implicit_start,
            n_steps,
            apply_mcs_corrector: true,
        }
    }

    /// Test-only: bare Douglas scheme (predictor + two implicit unidirectional
    /// correctors, no MCS corrector) with a caller-chosen `theta`.
    ///
    /// The corrector-less Douglas scheme is unconditionally stable only for
    /// `theta >= 1/2`; at `theta = 1/3` it is an inadmissible (unstable) scheme
    /// even for pure diffusion. Tests use both values: `theta = 1/3` to show
    /// that `theta = 1/3` is inadmissible *without* the MCS corrector, and
    /// `theta = 1/2` (a legitimate scheme) as an accuracy baseline for MCS.
    #[cfg(test)]
    pub(super) fn douglas_for_test(theta: f64, n_steps: usize) -> Self {
        Self {
            theta,
            implicit_start_steps: 0,
            n_steps,
            apply_mcs_corrector: false,
        }
    }

    /// Total number of time steps.
    pub fn n_steps(&self) -> usize {
        self.n_steps
    }

    /// Generate time levels from maturity backward to 0.
    pub fn time_levels(&self, maturity: f64) -> Vec<f64> {
        let n = self.n_steps;
        let dt = maturity / n as f64;
        (0..=n).map(|i| maturity - i as f64 * dt).collect()
    }

    /// Execute one Modified Craig-Sneyd ADI step from `t_from` to `t_to`
    /// (backward).
    ///
    /// Allocates fresh work buffers on every call; prefer
    /// [`CraigSneydStepper::step_with_buffers`] when stepping in a loop.
    ///
    /// `u_full` is the full (boundary-inclusive) solution of length `nx * ny`.
    /// `u_int` is the interior solution of length `nx_int * ny_int` (row-major).
    /// Both are updated in place.
    ///
    /// Returns a [`StepperError`] if the step cannot be taken reliably — a
    /// non-positive `dt`, a convection-dominated grid outside the MCS
    /// stable regime, or a degenerate tridiagonal solve.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &self,
        problem: &dyn PdeProblem2D,
        grid: &Grid2D,
        u_full: &mut [f64],
        u_int: &mut [f64],
        t_from: f64,
        t_to: f64,
        step_index: usize,
    ) -> Result<(), StepperError> {
        let mut buffers = AdiWorkBuffers::for_grid(grid);
        self.step_with_buffers(
            problem,
            grid,
            u_full,
            u_int,
            t_from,
            t_to,
            step_index,
            &mut buffers,
        )
    }

    /// Like [`CraigSneydStepper::step`], but reuses caller-owned scratch
    /// buffers instead of allocating fresh `Vec`s on every call.
    #[allow(clippy::too_many_arguments)]
    pub fn step_with_buffers(
        &self,
        problem: &dyn PdeProblem2D,
        grid: &Grid2D,
        u_full: &mut [f64],
        u_int: &mut [f64],
        t_from: f64,
        t_to: f64,
        step_index: usize,
        buffers: &mut AdiWorkBuffers,
    ) -> Result<(), StepperError> {
        let dt = t_from - t_to;
        // A non-positive or non-finite dt would otherwise propagate as silent
        // NaN / inf in release builds (the previous `debug_assert` was
        // compiled out). The `is_finite` check also rejects a NaN dt.
        if !dt.is_finite() || dt <= 0.0 {
            return Err(StepperError::NonPositiveStep { dt, t_from, t_to });
        }

        // Convection-dominated guard: the `theta = 1/3` MCS scheme is reliably
        // stable only when the cell Péclet number is bounded. The 1D path
        // enforces a CFL bound; the 2D path enforces this Péclet bound.
        // Evaluated at t_from (coefficients are time-homogeneous for the
        // Heston PDE, the production user of this stepper).
        check_peclet(problem, grid, t_from)?;

        let theta = if step_index < self.implicit_start_steps {
            1.0
        } else {
            self.theta
        };

        let nx_int = grid.nx_interior();
        let ny_int = grid.ny_interior();

        let interior = nx_int * ny_int;

        buffers.resize_for(grid);
        let AdiWorkBuffers {
            x_line,
            y_line,
            line_out,
            rhs_buf,
            y0,
            y1,
            y2,
            ytld,
            ax_u,
            ay_u,
            ax_y2,
            ay_y2,
            cross,
            cross_y2,
        } = buffers;

        // Assemble operators at t_from (for the explicit evaluation of F at u^n).
        let ops = Operators2D::assemble(problem, grid, t_from);

        // F_0(t_n, u^n): the mixed (cross-derivative) term applied to u^n.
        apply_cross_derivative_into(cross, &ops.cross_deriv, u_full, grid);

        // F_1(t_n, u^n) = A_x * u^n  (for each y-line).
        apply_x_operator(&ops, u_int, nx_int, ny_int, x_line, line_out, ax_u);
        // F_2(t_n, u^n) = A_y * u^n  (for each x-line).
        apply_y_operator(&ops, u_int, nx_int, ny_int, y_line, line_out, ay_u);

        // --- Predictor: Y_0 = u^n + dt * F(t_n, u^n) ---
        // The reaction term c*u is split into the directional operators
        // (c/2 in A_x, c/2 in A_y), so ax_u + ay_u already carries the full
        // reaction contribution; cross carries the mixed term.
        for idx in 0..interior {
            y0[idx] = u_int[idx] + dt * (ax_u[idx] + ay_u[idx] + cross[idx]);
        }

        // Operators for the implicit side (evaluated at t_{n+1} = t_to).
        let ops_impl = if problem.is_time_homogeneous() {
            ops
        } else {
            Operators2D::assemble(problem, grid, t_to)
        };

        let alpha = theta * dt;

        // --- CS corrector 1: implicit x-sweep ---
        // Y_1 = Y_0 + theta*dt * (F_1(t_{n+1}, Y_1) - F_1(t_n, u^n))
        // <=> (I - theta*dt*A_x) Y_1 = Y_0 - theta*dt*ax_u  (+ implicit corr.)
        implicit_x_sweep(
            &ops_impl, alpha, y0, ax_u, nx_int, ny_int, rhs_buf, line_out, y1,
        )?;

        // --- CS corrector 2: implicit y-sweep ---
        // Y_2 = Y_1 + theta*dt * (F_2(t_{n+1}, Y_2) - F_2(t_n, u^n))
        implicit_y_sweep(
            &ops_impl, alpha, y1, ay_u, nx_int, ny_int, rhs_buf, line_out, y2,
        )?;

        if !self.apply_mcs_corrector {
            // Bare Douglas scheme (test path): u^{n+1} = Y_2.
            u_int.copy_from_slice(y2);
            fill_boundaries(problem, grid, u_full, u_int, t_to);
            return Ok(());
        }

        // Reconstruct the boundary-inclusive Y_2 in u_full so that the mixed
        // (cross-derivative) corrector can use the four-point stencil.
        fill_boundaries(problem, grid, u_full, y2, t_to);

        // --- MCS mixed-term corrector ---
        // F_0(t_{n+1}, Y_2): mixed term applied to Y_2.
        apply_cross_derivative_into(cross_y2, &ops_impl.cross_deriv, u_full, grid);
        // F_1(t_{n+1}, Y_2) = A_x * Y_2 and F_2(t_{n+1}, Y_2) = A_y * Y_2,
        // needed for the full-operator difference in the Ytld_0 line.
        apply_x_operator(&ops_impl, y2, nx_int, ny_int, x_line, line_out, ax_y2);
        apply_y_operator(&ops_impl, y2, nx_int, ny_int, y_line, line_out, ay_y2);

        // Yhat_0 = Y_0 + theta*dt * (F_0(t_{n+1},Y_2) - F_0(t_n,u^n))
        // Ytld_0 = Yhat_0 + (1/2 - theta)*dt * (F(t_{n+1},Y_2) - F(t_n,u^n))
        // with F = F_0 + F_1 + F_2. Both corrections are folded into `ytld`.
        let half_minus_theta = 0.5 - theta;
        for idx in 0..interior {
            let f_old = cross[idx] + ax_u[idx] + ay_u[idx];
            let f_new = cross_y2[idx] + ax_y2[idx] + ay_y2[idx];
            ytld[idx] = y0[idx]
                + theta * dt * (cross_y2[idx] - cross[idx])
                + half_minus_theta * dt * (f_new - f_old);
        }

        // --- MCS corrector 1: second implicit x-sweep ---
        // Ytld_1 = Ytld_0 + theta*dt * (F_1(t_{n+1},Ytld_1) - F_1(t_n,u^n))
        implicit_x_sweep(
            &ops_impl, alpha, ytld, ax_u, nx_int, ny_int, rhs_buf, line_out, y1,
        )?;

        // --- MCS corrector 2: second implicit y-sweep ---
        // Ytld_2 = Ytld_1 + theta*dt * (F_2(t_{n+1},Ytld_2) - F_2(t_n,u^n))
        implicit_y_sweep(
            &ops_impl, alpha, y1, ay_u, nx_int, ny_int, rhs_buf, line_out, ytld,
        )?;

        // u^{n+1} = Ytld_2.
        u_int.copy_from_slice(ytld);
        fill_boundaries(problem, grid, u_full, u_int, t_to);
        Ok(())
    }
}

/// Apply the x-direction operator `A_x` (plus source/boundary corrections) to
/// an interior solution `u_int`, writing `A_x * u_int` into `out`.
///
/// `u_int` and `out` are row-major interior vectors of length `nx_int*ny_int`;
/// `x_line` (length `nx_int`) and `line_out` (length `>= nx_int`) are scratch.
fn apply_x_operator(
    ops: &Operators2D,
    u_int: &[f64],
    nx_int: usize,
    ny_int: usize,
    x_line: &mut [f64],
    line_out: &mut [f64],
    out: &mut [f64],
) {
    for jj in 0..ny_int {
        for ii in 0..nx_int {
            x_line[ii] = u_int[ii * ny_int + jj];
        }
        ops.op_x[jj].apply_into(x_line, &mut line_out[..nx_int]);
        for ii in 0..nx_int {
            out[ii * ny_int + jj] = line_out[ii];
        }
    }
}

/// Apply the y-direction operator `A_y` (plus source/boundary corrections) to
/// an interior solution `u_int`, writing `A_y * u_int` into `out`.
fn apply_y_operator(
    ops: &Operators2D,
    u_int: &[f64],
    nx_int: usize,
    ny_int: usize,
    y_line: &mut [f64],
    line_out: &mut [f64],
    out: &mut [f64],
) {
    for ii in 0..nx_int {
        for jj in 0..ny_int {
            y_line[jj] = u_int[ii * ny_int + jj];
        }
        ops.op_y[ii].apply_into(y_line, &mut line_out[..ny_int]);
        for jj in 0..ny_int {
            out[ii * ny_int + jj] = line_out[jj];
        }
    }
}

/// One implicit x-sweep `(I - alpha*A_x) out = rhs - alpha*ax_u (+ corr.)`.
///
/// `rhs` is the previous iterate (`Y_0` or `Ytld_0`), `ax_u` is `A_x` applied
/// to the explicit-side solution `u^n`. Both are row-major interior vectors;
/// `rhs_buf`/`line_out` (length `>= nx_int`) are scratch. Result into `out`.
///
/// Returns [`StepperError::ThomasFailure`] if any per-line tridiagonal solve
/// hits a degenerate pivot.
#[allow(clippy::too_many_arguments)]
fn implicit_x_sweep(
    ops: &Operators2D,
    alpha: f64,
    rhs: &[f64],
    ax_u: &[f64],
    nx_int: usize,
    ny_int: usize,
    rhs_buf: &mut [f64],
    line_out: &mut [f64],
    out: &mut [f64],
) -> Result<(), StepperError> {
    for jj in 0..ny_int {
        for ii in 0..nx_int {
            rhs_buf[ii] = rhs[ii * ny_int + jj] - alpha * ax_u[ii * ny_int + jj];
        }
        ops.op_x[jj].add_implicit_corrections(alpha, &mut rhs_buf[..nx_int]);
        ops.op_x[jj].solve_thomas_into(alpha, &rhs_buf[..nx_int], &mut line_out[..nx_int])?;
        for ii in 0..nx_int {
            out[ii * ny_int + jj] = line_out[ii];
        }
    }
    Ok(())
}

/// One implicit y-sweep `(I - alpha*A_y) out = rhs - alpha*ay_u (+ corr.)`.
///
/// Mirror of [`implicit_x_sweep`] along the y-direction.
#[allow(clippy::too_many_arguments)]
fn implicit_y_sweep(
    ops: &Operators2D,
    alpha: f64,
    rhs: &[f64],
    ay_u: &[f64],
    nx_int: usize,
    ny_int: usize,
    rhs_buf: &mut [f64],
    line_out: &mut [f64],
    out: &mut [f64],
) -> Result<(), StepperError> {
    for ii in 0..nx_int {
        for jj in 0..ny_int {
            rhs_buf[jj] = rhs[ii * ny_int + jj] - alpha * ay_u[ii * ny_int + jj];
        }
        ops.op_y[ii].add_implicit_corrections(alpha, &mut rhs_buf[..ny_int]);
        ops.op_y[ii].solve_thomas_into(alpha, &rhs_buf[..ny_int], &mut line_out[..ny_int])?;
        for jj in 0..ny_int {
            out[ii * ny_int + jj] = line_out[jj];
        }
    }
    Ok(())
}

/// Fill boundary values in the full grid from boundary conditions and the
/// interior solution.
///
/// `u_full` has length `nx * ny`, `u_int` has length `nx_int * ny_int`.
pub fn fill_boundaries(
    problem: &dyn PdeProblem2D,
    grid: &Grid2D,
    u_full: &mut [f64],
    u_int: &[f64],
    t: f64,
) {
    let nx = grid.nx();
    let ny = grid.ny();
    let nx_int = grid.nx_interior();
    let ny_int = grid.ny_interior();
    let x_pts = grid.x().points();
    let y_pts = grid.y().points();

    // Copy interior into full grid
    for ii in 0..nx_int {
        for jj in 0..ny_int {
            u_full[(ii + 1) * ny + (jj + 1)] = u_int[ii * ny_int + jj];
        }
    }

    // x-boundaries (left and right edges): all y-values
    for j in 0..ny {
        let y = y_pts[j];
        // Lower x-boundary (i = 0)
        u_full[j] = boundary_value_2d(
            problem.boundary_x_lower(y, t),
            u_full,
            grid,
            0,
            j,
            true,
            true,
        );
        // Upper x-boundary (i = nx-1)
        u_full[(nx - 1) * ny + j] = boundary_value_2d(
            problem.boundary_x_upper(y, t),
            u_full,
            grid,
            nx - 1,
            j,
            true,
            false,
        );
    }

    // y-boundaries (bottom and top edges): interior x-values only
    // (corners already set by x-boundary pass)
    for i in 1..nx - 1 {
        let x = x_pts[i];
        // Lower y-boundary (j = 0)
        u_full[i * ny] = boundary_value_2d(
            problem.boundary_y_lower(x, t),
            u_full,
            grid,
            i,
            0,
            false,
            true,
        );
        // Upper y-boundary (j = ny-1)
        u_full[i * ny + ny - 1] = boundary_value_2d(
            problem.boundary_y_upper(x, t),
            u_full,
            grid,
            i,
            ny - 1,
            false,
            false,
        );
    }
}

/// Extract a boundary value from a boundary condition.
///
/// For Dirichlet, returns the fixed value. For Neumann, extrapolates using
/// the derivative value. For Linear, extrapolates from two interior points
/// (vanishing second derivative). `is_x_dir` indicates which direction the
/// boundary is on; `is_lower` indicates lower vs upper edge.
fn boundary_value_2d(
    bc: BoundaryCondition,
    u_full: &[f64],
    grid: &Grid2D,
    i: usize,
    j: usize,
    is_x_dir: bool,
    is_lower: bool,
) -> f64 {
    let ny = grid.ny();
    match bc {
        BoundaryCondition::Dirichlet(g) => g,
        BoundaryCondition::Neumann(g) => {
            // du/dn = g: first-order extrapolation using the derivative value
            if is_x_dir {
                if is_lower {
                    let h = grid.x().h_left(1);
                    let u1 = u_full[ny + j];
                    u1 - h * g
                } else {
                    let h = grid.x().h_right(grid.nx() - 2);
                    let u1 = u_full[(i - 1) * ny + j];
                    u1 + h * g
                }
            } else if is_lower {
                let h = grid.y().h_left(1);
                let u1 = u_full[i * ny + 1];
                u1 - h * g
            } else {
                let h = grid.y().h_right(grid.ny() - 2);
                let u1 = u_full[i * ny + (j - 1)];
                u1 + h * g
            }
        }
        BoundaryCondition::Linear => {
            // d²u/dx² = 0: linear extrapolation from two interior neighbors
            if is_x_dir {
                if is_lower {
                    // i=0: extrapolate from i=1, i=2
                    let u1 = u_full[ny + j];
                    let u2 = u_full[2 * ny + j];
                    2.0 * u1 - u2
                } else {
                    // i=nx-1: extrapolate from i=nx-2, i=nx-3
                    let u1 = u_full[(i - 1) * ny + j];
                    let u2 = u_full[(i - 2) * ny + j];
                    2.0 * u1 - u2
                }
            } else if is_lower {
                // j=0: extrapolate from j=1, j=2
                let u1 = u_full[i * ny + 1];
                let u2 = u_full[i * ny + 2];
                2.0 * u1 - u2
            } else {
                // j=ny-1: extrapolate from j=ny-2, j=ny-3
                let u1 = u_full[i * ny + (j - 1)];
                let u2 = u_full[i * ny + (j - 2)];
                2.0 * u1 - u2
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::boundary::BoundaryCondition;
    use super::super::grid::Grid1D;
    use super::super::grid2d::Grid2D;
    use super::super::problem2d::PdeProblem2D;
    use super::*;

    /// 2D heat equation: u_t = u_xx + u_yy on [0,pi] x [0,pi]
    /// Terminal: u(x,y,T) = sin(x) * sin(y)
    /// Exact: u(x,y,t) = exp(-2*(T-t)) * sin(x) * sin(y)
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
    fn heat_2d_adi_converges() {
        let t_mat = 0.25;
        let problem = Heat2D;
        let pi = std::f64::consts::PI;

        let n_space = 41;
        let n_time = 200;

        let gx = Grid1D::uniform(0.0, pi, n_space).expect("valid grid");
        let gy = Grid1D::uniform(0.0, pi, n_space).expect("valid grid");
        let grid = Grid2D::new(gx, gy);

        let nx = grid.nx();
        let ny = grid.ny();
        let nx_int = grid.nx_interior();
        let ny_int = grid.ny_interior();

        // Initialize full solution with terminal condition
        let mut u_full = vec![0.0; nx * ny];
        for i in 0..nx {
            for j in 0..ny {
                u_full[i * ny + j] =
                    problem.terminal_condition(grid.x().points()[i], grid.y().points()[j]);
            }
        }

        // Extract interior
        let mut u_int = vec![0.0; nx_int * ny_int];
        for ii in 0..nx_int {
            for jj in 0..ny_int {
                u_int[ii * ny_int + jj] = u_full[(ii + 1) * ny + (jj + 1)];
            }
        }

        let stepper = CraigSneydStepper::new(n_time);
        let levels = stepper.time_levels(t_mat);

        for step in 0..n_time {
            stepper
                .step(
                    &problem,
                    &grid,
                    &mut u_full,
                    &mut u_int,
                    levels[step],
                    levels[step + 1],
                    step,
                )
                .expect("pure-diffusion 2D heat step is stable");
        }

        // Check at (pi/2, pi/2)
        let exact = (-2.0 * t_mat).exp() * (pi / 2.0).sin() * (pi / 2.0).sin();
        let computed = grid.interpolate(&u_full, pi / 2.0, pi / 2.0);
        let error = (computed - exact).abs();
        assert!(
            error < 0.01,
            "2D heat CS error = {error:.6e}, exact = {exact:.6}, computed = {computed:.6}"
        );
    }

    /// A convection-dominated 2D problem: small diffusion, large convection.
    /// The convection magnitude `conv` is a tunable knob so a test can dial
    /// the cell Péclet number across [`MCS_PECLET_MAX`].
    struct ConvectionDominated2D {
        /// Diffusion coefficient on both axes.
        diff: f64,
        /// Convection coefficient on both axes.
        conv: f64,
    }

    impl PdeProblem2D for ConvectionDominated2D {
        fn diffusion_xx(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            self.diff
        }
        fn diffusion_yy(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            self.diff
        }
        fn mixed_diffusion(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            0.0
        }
        fn convection_x(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            self.conv
        }
        fn convection_y(&self, _x: f64, _y: f64, _t: f64) -> f64 {
            self.conv
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

    /// [P6-1] The 2D MCS stepper must reject a strongly convection-dominated
    /// grid with [`StepperError::PecletViolation`] rather than running the
    /// `theta = 1/3` scheme silently outside its proven-stable regime.
    ///
    /// Failure mode being guarded: unlike the 1D path (which enforces a CFL
    /// bound), the 2D MCS stepper hard-coded `theta = 1/3` with no stability
    /// guard. MCS at `theta = 1/3` is unconditionally stable for *pure
    /// diffusion* but only for `theta >= 2/5` in the general
    /// convection-diffusion case (In 't Hout & Mishra 2010); a
    /// convection-dominated grid can therefore diverge silently to inf / NaN.
    ///
    /// On a 41x41 grid over `[0, pi]^2` (spacing h ≈ pi/40 ≈ 0.0785) with
    /// diffusion `a = 0.01` and convection `b = 5.0`, the cell Péclet number
    /// is `Pe = |b|*h/(2a) ≈ 5*0.0785/0.02 ≈ 19.6` — far above the
    /// `MCS_PECLET_MAX = 4.0` ceiling. The step must error.
    #[test]
    fn mcs_step_rejects_convection_dominated_grid() {
        let pi = std::f64::consts::PI;
        let problem = ConvectionDominated2D {
            diff: 0.01,
            conv: 5.0,
        };
        let gx = Grid1D::uniform(0.0, pi, 41).expect("valid grid");
        let gy = Grid1D::uniform(0.0, pi, 41).expect("valid grid");
        let grid = Grid2D::new(gx, gy);

        // The Péclet check itself flags the grid.
        match check_peclet(&problem, &grid, 0.0) {
            Err(StepperError::PecletViolation {
                peclet,
                pe_max,
                convection,
                diffusion,
                ..
            }) => {
                assert!(
                    (pe_max - MCS_PECLET_MAX).abs() < 1e-12,
                    "the error must cite the MCS_PECLET_MAX ceiling"
                );
                assert!(
                    peclet > MCS_PECLET_MAX,
                    "the reported Péclet {peclet:e} must exceed the ceiling {pe_max}"
                );
                assert!(
                    (convection - 5.0).abs() < 1e-12 && (diffusion - 0.01).abs() < 1e-12,
                    "the error must cite the offending convection / diffusion coefficients"
                );
            }
            other => panic!("expected PecletViolation from check_peclet, got {other:?}"),
        }

        // A full MCS step must surface the same error.
        let nx = grid.nx();
        let ny = grid.ny();
        let nx_int = grid.nx_interior();
        let ny_int = grid.ny_interior();
        let mut u_full = vec![0.0; nx * ny];
        for i in 0..nx {
            for j in 0..ny {
                u_full[i * ny + j] =
                    problem.terminal_condition(grid.x().points()[i], grid.y().points()[j]);
            }
        }
        let mut u_int = vec![0.0; nx_int * ny_int];
        for ii in 0..nx_int {
            for jj in 0..ny_int {
                u_int[ii * ny_int + jj] = u_full[(ii + 1) * ny + (jj + 1)];
            }
        }
        let stepper = CraigSneydStepper::new(100);
        let result = stepper.step(&problem, &grid, &mut u_full, &mut u_int, 0.25, 0.245, 0);
        assert!(
            matches!(result, Err(StepperError::PecletViolation { .. })),
            "a convection-dominated MCS step must be rejected, got {result:?}"
        );
    }

    /// [P6-1] The Péclet guard must NOT false-positive a benign grid: pure
    /// 2D diffusion (zero convection — MCS at `theta = 1/3` is
    /// unconditionally stable here) and a mildly convective grid well within
    /// the ceiling both pass.
    #[test]
    fn mcs_peclet_guard_accepts_diffusion_dominated_grids() {
        let pi = std::f64::consts::PI;
        let gx = Grid1D::uniform(0.0, pi, 41).expect("valid grid");
        let gy = Grid1D::uniform(0.0, pi, 41).expect("valid grid");
        let grid = Grid2D::new(gx, gy);

        // Pure diffusion: zero convection → no Péclet constraint at all.
        assert!(
            check_peclet(&Heat2D, &grid, 0.0).is_ok(),
            "pure-diffusion grid must pass the Péclet guard"
        );

        // Mild convection: with a = 1.0, b = 1.0, h ≈ 0.0785 the cell Péclet
        // is ≈ 0.04 — far inside the ceiling.
        let mild = ConvectionDominated2D {
            diff: 1.0,
            conv: 1.0,
        };
        assert!(
            check_peclet(&mild, &grid, 0.0).is_ok(),
            "a mildly convective, diffusion-dominated grid must pass the Péclet guard"
        );
    }

    /// [P6-6] The 2D MCS stepper must reject a non-positive time step with
    /// [`StepperError::NonPositiveStep`] rather than producing silent NaN
    /// (the old `debug_assert!(dt > 0.0)` was compiled out in release).
    #[test]
    fn mcs_step_rejects_non_positive_dt() {
        let pi = std::f64::consts::PI;
        let gx = Grid1D::uniform(0.0, pi, 11).expect("valid grid");
        let gy = Grid1D::uniform(0.0, pi, 11).expect("valid grid");
        let grid = Grid2D::new(gx, gy);
        let nx = grid.nx();
        let ny = grid.ny();
        let nx_int = grid.nx_interior();
        let ny_int = grid.ny_interior();
        let mut u_full = vec![1.0; nx * ny];
        let mut u_int = vec![1.0; nx_int * ny_int];

        let stepper = CraigSneydStepper::new(10);
        // dt = 0: t_from == t_to.
        let zero = stepper.step(&Heat2D, &grid, &mut u_full, &mut u_int, 0.3, 0.3, 0);
        assert!(
            matches!(zero, Err(StepperError::NonPositiveStep { dt, .. }) if dt == 0.0),
            "dt = 0 must be rejected as NonPositiveStep, got {zero:?}"
        );
        // dt < 0: t_from < t_to.
        let neg = stepper.step(&Heat2D, &grid, &mut u_full, &mut u_int, 0.1, 0.4, 0);
        assert!(
            matches!(neg, Err(StepperError::NonPositiveStep { dt, .. }) if dt < 0.0),
            "dt < 0 must be rejected as NonPositiveStep, got {neg:?}"
        );
    }
}

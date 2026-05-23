//! PDE problem trait defining the 1D convection-diffusion-reaction equation.
//!
//! The standard form is:
//! ```text
//! du/dt = a(x,t) d²u/dx² + b(x,t) du/dx + c(x,t) u + f(x,t)
//! ```
//!
//! Working in log-spot coordinates (`x = ln S`) simplifies coefficients
//! for Black-Scholes-type problems and maps relative price moves to
//! uniform grid spacing.

use super::boundary::BoundaryCondition;

/// Defines a 1D PDE problem in convection-diffusion-reaction form.
///
/// Implementors specify the PDE coefficients, terminal condition (payoff),
/// and boundary conditions. The solver evaluates these at each grid node
/// and time step to assemble and advance the finite difference system.
///
/// # Coordinate Convention
///
/// Prefer log-spot coordinates (`x = ln S`) so that:
/// - Diffusion `a = 0.5 σ²` is constant for flat-vol Black-Scholes
/// - Convection `b = r - q - 0.5 σ²`
/// - Grid spacing maps to relative price moves
/// - Payoff kinks at strike `K` map to `ln K`
pub trait PdeProblem1D {
    /// Diffusion coefficient `a(x, t)` — multiplies `d²u/dx²`.
    ///
    /// For Black-Scholes in log-spot: `0.5 * σ²`.
    fn diffusion(&self, x: f64, t: f64) -> f64;

    /// Convection coefficient `b(x, t)` — multiplies `du/dx`.
    ///
    /// For Black-Scholes in log-spot: `r - q - 0.5 * σ²`.
    fn convection(&self, x: f64, t: f64) -> f64;

    /// Reaction coefficient `c(x, t)` — multiplies `u`.
    ///
    /// For Black-Scholes: `-r` (discounting term).
    fn reaction(&self, x: f64, t: f64) -> f64;

    /// Source term `f(x, t)` — additive forcing.
    ///
    /// Zero for standard option pricing. Override for inhomogeneous PDEs.
    fn source(&self, _x: f64, _t: f64) -> f64 {
        0.0
    }

    /// Terminal condition `u(x, T) = payoff(x)`.
    ///
    /// Evaluated at maturity to initialize the backward time-stepping.
    fn terminal_condition(&self, x: f64) -> f64;

    /// Boundary condition at the lower edge `x = x_min` at time `t`.
    fn lower_boundary(&self, t: f64) -> BoundaryCondition;

    /// Boundary condition at the upper edge `x = x_max` at time `t`.
    fn upper_boundary(&self, t: f64) -> BoundaryCondition;

    /// Hint: are all coefficients independent of time?
    ///
    /// When `true`, the solver caches the tridiagonal operator and reuses
    /// it at every time step, avoiding redundant assembly.
    fn is_time_homogeneous(&self) -> bool {
        false
    }
}

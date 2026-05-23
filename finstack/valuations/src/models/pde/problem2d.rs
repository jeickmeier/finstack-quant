//! PDE problem trait defining a 2D convection-diffusion-reaction equation.
//!
//! The standard form is:
//! ```text
//! du/dt = a_xx(x,y,t) d²u/dx² + a_yy(x,y,t) d²u/dy²
//!       + a_xy(x,y,t) d²u/(dx dy)
//!       + b_x(x,y,t) du/dx + b_y(x,y,t) du/dy
//!       + c(x,y,t) u + f(x,y,t)
//! ```
//!
//! For the Heston model in log-spot (`x = ln S`) and variance (`y = v`):
//! - `a_xx = 0.5 v`, `a_yy = 0.5 σ_v² v`, `a_xy = ρ σ_v v`
//! - `b_x = r - q - 0.5 v`, `b_y = κ(θ - v)`
//! - `c = -r`

use super::boundary::BoundaryCondition;

/// Defines a 2D PDE problem in convection-diffusion-reaction form.
///
/// Implementors specify PDE coefficients, terminal condition, and boundary
/// conditions on all four edges. The ADI solver evaluates these at each
/// grid node and time step to assemble directional operators.
///
/// # Coordinate Convention
///
/// - x-axis: typically log-spot (`x = ln S`)
/// - y-axis: typically variance (`v`) or log-variance
pub trait PdeProblem2D {
    /// Diffusion in the x-direction: coefficient of `d²u/dx²`.
    fn diffusion_xx(&self, x: f64, y: f64, t: f64) -> f64;

    /// Diffusion in the y-direction: coefficient of `d²u/dy²`.
    fn diffusion_yy(&self, x: f64, y: f64, t: f64) -> f64;

    /// Mixed derivative coefficient: `d²u/(dx dy)`.
    ///
    /// Treated explicitly in ADI splitting.
    fn mixed_diffusion(&self, x: f64, y: f64, t: f64) -> f64;

    /// Convection in the x-direction: coefficient of `du/dx`.
    fn convection_x(&self, x: f64, y: f64, t: f64) -> f64;

    /// Convection in the y-direction: coefficient of `du/dy`.
    fn convection_y(&self, x: f64, y: f64, t: f64) -> f64;

    /// Reaction coefficient: multiplies `u`.
    fn reaction(&self, x: f64, y: f64, t: f64) -> f64;

    /// Source term (additive forcing). Default zero.
    fn source(&self, _x: f64, _y: f64, _t: f64) -> f64 {
        0.0
    }

    /// Terminal condition `u(x, y, T) = payoff(x, y)`.
    fn terminal_condition(&self, x: f64, y: f64) -> f64;

    /// Boundary condition at x = x_min (lower x-edge).
    fn boundary_x_lower(&self, y: f64, t: f64) -> BoundaryCondition;

    /// Boundary condition at x = x_max (upper x-edge).
    fn boundary_x_upper(&self, y: f64, t: f64) -> BoundaryCondition;

    /// Boundary condition at y = y_min (lower y-edge).
    fn boundary_y_lower(&self, x: f64, t: f64) -> BoundaryCondition;

    /// Boundary condition at y = y_max (upper y-edge).
    fn boundary_y_upper(&self, x: f64, t: f64) -> BoundaryCondition;

    /// Hint: are all coefficients independent of time?
    ///
    /// When `true`, directional operators can be cached.
    fn is_time_homogeneous(&self) -> bool {
        false
    }
}

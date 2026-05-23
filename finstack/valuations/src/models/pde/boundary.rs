//! Boundary condition types for PDE solvers.
//!
//! Defines the standard boundary conditions used at grid edges:
//! Dirichlet (known value), Neumann (known derivative), and Linear
//! (vanishing second derivative, standard far-field for options).

/// Boundary condition at a grid edge.
///
/// Applied at each time step to the lower (`x_min`) and upper (`x_max`)
/// boundaries of the spatial domain.
#[derive(Debug, Clone, Copy)]
pub enum BoundaryCondition {
    /// Fixed value: `u(x_boundary, t) = value`.
    ///
    /// The boundary value participates in the interior stencil by shifting
    /// known terms to the right-hand side of the linear system.
    Dirichlet(f64),

    /// Fixed first derivative: `du/dx(x_boundary, t) = value`.
    ///
    /// Implemented via ghost-node elimination, modifying the stencil
    /// at the adjacent interior point.
    Neumann(f64),

    /// Vanishing second derivative: `d²u/dx² = 0` at the boundary.
    ///
    /// Equivalent to linear extrapolation from the interior. Standard
    /// far-field condition for option pricing where gamma vanishes
    /// far from the strike.
    Linear,
}

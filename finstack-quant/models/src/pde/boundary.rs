//! Boundary condition types for PDE solvers.
//!
//! Defines the standard boundary conditions used at grid edges:
//! Dirichlet (known value), Neumann (known derivative), and extrapolation
//! that is affine in the grid coordinate or its exponential.

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
    /// Extrapolates the slope of the two nearest interior nodes using the
    /// actual cell widths, including on nonuniform grids. With only one
    /// interior node, the extension is constant. This represents vanishing
    /// option gamma only when the grid coordinate is spot itself.
    Linear,

    /// Affine dependence on the exponential coordinate: `u(x) = a + b exp(x)`.
    ///
    /// On a log-spot grid `x = ln(S)`, this imposes vanishing spot gamma,
    /// `d²u/dS² = 0`, equivalently `d²u/dx² = du/dx`. Extrapolation uses
    /// the exact exponential cell widths, including on nonuniform grids.
    /// With only one interior node the extension is constant.
    LinearInExp,

    /// The PDE itself holds on the edge, which is solved with the interior.
    ///
    /// For an edge where the diffusion normal to the edge and the mixed
    /// derivative coefficient both vanish and the convection normal to the
    /// edge points into the domain, the equation needs no boundary data: the
    /// characteristics leave through the edge. The edge nodes are unknowns of
    /// the linear systems, and their normal derivative is the second-order
    /// one-sided three-point difference (In 't Hout & Foulon 2010 use this at
    /// zero variance in the Heston equation).
    ///
    /// Supported on the lower y-edge of a 2D problem only, where the
    /// condition must hold for every x and every time. A solver given this
    /// condition on any other edge, or on an edge where the coefficient
    /// requirements fail, returns
    /// [`PdeError::UnsupportedBoundary`](super::PdeError::UnsupportedBoundary).
    Degenerate,
}

impl BoundaryCondition {
    /// Ratio of boundary-cell width to interior-cell width in the coordinate
    /// in which a linear extrapolation is imposed.
    pub(super) fn extrapolation_ratio(
        self,
        boundary_width: f64,
        interior_width: f64,
        is_lower: bool,
    ) -> f64 {
        if matches!(self, Self::LinearInExp) {
            if is_lower {
                -(-boundary_width).exp_m1() / interior_width.exp_m1()
            } else {
                boundary_width.exp_m1() / -(-interior_width).exp_m1()
            }
        } else {
            boundary_width / interior_width
        }
    }
}

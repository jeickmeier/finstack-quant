//! Unified error type for 1D and 2D finite-difference PDE solvers.

/// Errors returned by finite-difference grids, operators, steppers, and solvers.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PdeError {
    /// Grid requires more points.
    #[error("Grid needs at least {min} points, got {n}")]
    TooFewPoints {
        /// Number of points provided.
        n: usize,
        /// Minimum required.
        min: usize,
    },

    /// Invalid domain bounds.
    #[error("Invalid bounds: x_min={x_min} must be less than x_max={x_max}")]
    InvalidBounds {
        /// Lower bound.
        x_min: f64,
        /// Upper bound.
        x_max: f64,
    },

    /// Grid points are not strictly increasing.
    #[error("Grid points must be strictly increasing")]
    NotMonotonic,

    /// Invalid grid construction parameter.
    #[error("Invalid parameter '{name}'={value}: {reason}")]
    InvalidParameter {
        /// Parameter name.
        name: &'static str,
        /// Parameter value.
        value: f64,
        /// Reason it is invalid.
        reason: &'static str,
    },

    /// A `sinh_concentrated` grid degenerated to (effectively) uniform.
    ///
    /// The sinh parameter range collapsed below numerical resolution, so the
    /// transformation is effectively linear and the requested concentration
    /// near `center` is entirely lost. Use a smaller `intensity`, place
    /// `center` inside the domain, or call [`Grid1D::uniform`](super::Grid1D::uniform)
    /// explicitly.
    #[error(
        "sinh_concentrated grid degenerated to uniform: intensity={intensity} with \
         center={center} on domain [{x_min}, {x_max}] collapses the sinh range — \
         the requested strike concentration is lost; use a smaller intensity, a \
         center inside the domain, or Grid1D::uniform"
    )]
    DegenerateConcentration {
        /// The `intensity` argument that produced the degenerate grid.
        intensity: f64,
        /// The `center` argument.
        center: f64,
        /// Lower domain bound.
        x_min: f64,
        /// Upper domain bound.
        x_max: f64,
    },

    /// A pivot in the Thomas forward elimination became degenerate — either
    /// non-finite, or negligibly small relative to the terms that formed it.
    ///
    /// For the implicit operator `(I - alpha * A)` this signals that diagonal
    /// dominance has been eroded — most often by a Neumann / Linear boundary-
    /// condition modification on a near-degenerate row — so the solve cannot
    /// be trusted.
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

    /// An explicit or under-damped (`theta < 0.5`) theta scheme was asked to
    /// take a time step larger than the CFL / von-Neumann stability limit
    /// `dt <= min_i(dx_i)^2 / (2 * max_i|a_i|)`.
    #[error(
        "CFL stability violated: explicit/under-damped theta scheme (theta={theta}) \
         requires dt <= dx_min^2 / (2*max|a|) = {cfl_max_dt:e}, but dt = {dt:e} \
         (limiting grid spacing dx_min = {dx_min:e}, max diffusion |a| = {max_diffusion:e}); \
         use more time steps, a coarser/uniform grid, or an implicit/Crank-Nicolson scheme (theta >= 0.5)"
    )]
    CflViolation {
        /// Theta parameter of the offending scheme (`< 0.5`).
        theta: f64,
        /// The time-step size requested by the caller.
        dt: f64,
        /// The largest stable time step permitted by the CFL condition.
        cfl_max_dt: f64,
        /// Smallest grid spacing on the (possibly non-uniform) grid.
        dx_min: f64,
        /// Largest absolute diffusion coefficient over the interior grid.
        max_diffusion: f64,
    },

    /// A non-positive or non-finite time step `dt = t_from - t_to` was requested.
    #[error(
        "invalid time step: dt = {dt:e} must be strictly positive (t_from={t_from:e}, t_to={t_to:e})"
    )]
    NonPositiveStep {
        /// The non-positive step size.
        dt: f64,
        /// Step start time (closer to maturity).
        t_from: f64,
        /// Step end time (closer to t=0).
        t_to: f64,
    },

    /// Payoffs do not cover exactly the solver's interior spatial nodes.
    #[error("exercise payoff length {actual} does not match {expected} interior nodes")]
    PayoffLength {
        /// Number of interior solution values.
        expected: usize,
        /// Number of supplied intrinsic values.
        actual: usize,
    },

    /// A payoff or continuation value is not finite.
    #[error("non-finite exercise {kind} at interior node {index}")]
    NonFiniteExerciseValue {
        /// Whether the invalid value is a payoff or continuation value.
        kind: &'static str,
        /// Index into the interior-node value vector.
        index: usize,
    },

    /// The penalty factor is not finite and positive, or no iteration is requested.
    #[error("exercise requires a positive finite penalty factor and at least one iteration")]
    InvalidPenalty,

    /// The exercise time-step interval is not finite and strictly positive.
    #[error("exercise time step must be positive and finite, got {dt}")]
    InvalidExerciseTimeStep {
        /// Rejected step width in model years.
        dt: f64,
    },

    /// No grid was specified in the solver builder.
    #[error("PDE solver requires a spatial grid")]
    MissingGrid,

    /// No time stepper was specified in the solver builder.
    #[error("PDE solver requires a time stepper")]
    MissingStepper,

    /// The maturity passed to `solve` was not strictly positive.
    #[error("PDE solve requires a strictly positive maturity, got {maturity:e}")]
    NonPositiveMaturity {
        /// The offending maturity.
        maturity: f64,
    },

    /// A [`BoundaryCondition::Degenerate`](super::BoundaryCondition::Degenerate)
    /// edge was requested where the solver cannot impose it.
    #[error("degenerate boundary condition rejected on the {edge} edge: {reason}")]
    UnsupportedBoundary {
        /// Edge that carried the condition.
        edge: &'static str,
        /// Why the condition cannot be imposed there.
        reason: &'static str,
    },

    /// The stepper was configured with zero time steps.
    #[error("PDE solve requires at least one time step, got n_steps = 0")]
    ZeroTimeSteps,

    /// A Bermudan exercise time is invalid or absent from the solver time grid.
    #[error(
        "Bermudan exercise time {time} must be finite, within maturity, and aligned with the PDE time grid"
    )]
    InvalidExerciseTime {
        /// Rejected year fraction from valuation.
        time: f64,
    },
}

impl From<PdeError> for finstack_quant_core::Error {
    fn from(err: PdeError) -> Self {
        match err {
            PdeError::DegeneratePivot { .. } | PdeError::CflViolation { .. } => {
                Self::Internal(err.to_string())
            }
            other => Self::Validation(other.to_string()),
        }
    }
}

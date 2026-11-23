//! Concrete interpolation strategy implementations.
//!
//! Provides strategy types for Linear, LogLinear, CubicHermite, MonotoneConvex,
//! and specialized forward-rate interpolation. The public strategies are kept
//! here because curve builders and calibration code select them by
//! [`InterpStyle`](super::types::InterpStyle); less-common strategies should
//! remain documented and covered by tests before being exposed through bindings.

pub(crate) mod cubic_hermite;
pub(crate) mod linear;
pub(crate) mod monotone_convex;
mod numerics;
pub(crate) mod piecewise_quadratic;

pub(crate) use cubic_hermite::CubicHermiteStrategy;
pub(crate) use linear::{LinearStrategy, LogLinearStrategy};
pub(crate) use monotone_convex::MonotoneConvexStrategy;
pub(crate) use piecewise_quadratic::PiecewiseQuadraticForwardStrategy;

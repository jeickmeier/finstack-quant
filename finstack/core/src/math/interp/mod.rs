//! Interpolation algorithms for yield curves and term structures.
//!
//! Provides multiple interpolation schemes for constructing smooth, arbitrage-free
//! curves from discrete market quotes. Implementations are optimized for financial
//! use cases with focus on monotonicity and positive forward rates.
//!
//! # Interpolation Methods
//!
//! - [`LinearDf`]: Linear in discount factors (simple but may create arbitrage)
//! - [`LogLinearDf`]: Linear in log(DF), constant zero rates
//! - [`MonotoneConvex`]: Hagan-West smooth, monotone, no-arbitrage
//! - [`CubicHermite`]: PCHIP shape-preserving cubic
//! - [`PiecewiseQuadraticForward`]: Natural cubic in log DF (C² forwards)
//!
//! # Arbitrage Considerations
//!
//! Not all methods guarantee positive forward rates:
//! - **Linear**: May produce negative forwards ❌
//! - **LogLinear**: Guarantees positive forwards ✅
//! - **MonotoneConvex**: Guarantees positive forwards ✅
//! - **CubicHermite**: Shape-preserving but requires monotone input ⚠️
//!
//! # References
//!
//! - Hagan, P. S., & West, G. (2006). "Interpolation Methods for Curve Construction."
//!   *Applied Mathematical Finance*, 13(2), 89-129. See
//!   [Hagan-West Monotone Convex](../../../../docs/REFERENCES.md#hagan-west-monotone-convex).
//! - Hagan, P. S., & West, G. (2008). "Methods for Constructing a Yield Curve."
//!   *Wilmott Magazine*, May 2008.
//! - Fritsch, F. N., & Carlson, R. E. (1980). "Monotone Piecewise Cubic
//!   Interpolation." See
//!   [Fritsch-Carlson 1980](../../../../docs/REFERENCES.md#fritsch-carlson-1980).
//! - de Boor, C. *A Practical Guide to Splines*. See
//!   [De Boor Splines](../../../../docs/REFERENCES.md#de-boor-splines).

/// Generic interpolator container with strategy pattern.
mod generic;
/// Concrete strategy implementations.
pub(crate) mod strategies;
/// Traits for interpolation.
mod traits;
/// Types, public aliases, and factory for interpolation.
pub(crate) mod types;
/// Shared helpers (validation and search).
pub(crate) mod utils;

// Re-exports for ergonomic access
pub use generic::Interpolator;
pub use strategies::{
    CubicHermiteStrategy, LinearStrategy, LogLinearStrategy, MonotoneConvexStrategy,
    PiecewiseQuadraticForwardStrategy, DEFAULT_MONOTONE_CONVEX_EPSILON,
};
pub use traits::{InterpFn, InterpolationStrategy};
pub use types::{ExtrapolationPolicy, InterpStyle, ValidationPolicy, DERIVATIVE_EPSILON};
/// Validate knot ordering and finiteness (internal helper used by curve builders).
pub use utils::validate_knots;

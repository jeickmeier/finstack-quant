//! Correlation matrix utilities shared across credit, rates, and portfolio
//! analytics.
//!
//! # Components
//!
//! - [`Error`]: Structured validation diagnostics
//! - [`nearest_correlation()`][]: Higham (2002)
//!   alternating-projection PSD repair
//! - [`validate_correlation_matrix`]: Core's canonical correlation validation
//!   with located [`Error`] variants (`DiagonalNotOne`, `OutOfBounds`, …)

mod nearest_correlation;

pub use finstack_quant_core::math::linalg::{
    validate_correlation_matrix, CorrelationError as Error,
};
pub use nearest_correlation::{nearest_correlation, NearestCorrelationOpts};

/// Convenience result type for detailed correlation operations.
pub type Result<T> = std::result::Result<T, Error>;

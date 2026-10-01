//! WASM bindings for `finstack-quant-analytics`.
//!
//! The primary entry point exposed to JS is the [`JsPerformance`] class
//! (exported to JS as `Performance`). The bound analytics — returns/risk
//! metrics, benchmark comparisons, basic factor models — are reachable as
//! `Performance` methods, including the ticker-major periodic-return panel.
//! The free functions are [`constrained_least_squares`] (exported as
//! `constrainedLeastSquares`), a standalone numeric regression building
//! block for factor-Brinson attribution, and the scalar metrics [`sharpe`],
//! [`sortino`], [`volatility`] and [`max_drawdown`] (`maxDrawdown`) over one
//! return series; none depends on `Performance`.

mod performance;
mod performance_io;
mod regression;
mod scalar;
mod support;

pub use performance::JsPerformance;
pub use regression::constrained_least_squares;
pub use scalar::{max_drawdown, sharpe, sortino, volatility};

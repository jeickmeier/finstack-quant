//! Shared metric calculators that work across instrument types.
//!
//! These calculators rely on trait-based instrument metadata (e.g., `expiry()`,
//! `effective_start_date()`) and `market_dependencies()` to avoid per-instrument boilerplate.

pub(crate) mod df;

//! Comprehensive IR Future instrument test suite.
//!
//! Organized into logical modules covering:
//! - Construction and builders
//! - Pricing scenarios
//! - Metrics (PV, DV01, Theta, Bucketed DV01)
//! - Position handling
//! - Edge cases and error handling
//! - Market standard validations

mod test_construction;
mod test_convexity;
mod test_edge_cases;
mod test_market_standard;
mod test_metrics;
mod test_position;
mod test_pricing;
mod utils;

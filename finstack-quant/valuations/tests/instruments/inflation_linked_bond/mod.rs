//! Comprehensive test suite for Inflation-Linked Bonds (ILB)
//!
//! This module provides market-standard unit tests organized by functionality:
//! - Construction and parameter validation
//! - Index ratio calculations (various methods and interpolations)
//! - Cashflow generation and schedules
//! - Pricing and NPV calculations
//! - Real yield and breakeven inflation
//! - Duration and DV01 sensitivities
//! - Metrics integration
//! - Edge cases and error handling

mod common;
mod test_cashflows;
mod test_construction;
mod test_duration;
mod test_edge_cases;
mod test_index_ratio;
mod test_metrics;
mod test_pricing;
mod test_yields;

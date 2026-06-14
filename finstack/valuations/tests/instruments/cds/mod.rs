//! CDS instrument tests module.
//!
//! Comprehensive test suite organized by functionality:
//! - `construction`: CDS creation, builders, and conventions
//! - `pricing`: Core pricing logic (legs, NPV, schedules)
//! - `metrics`: All metrics calculations (CS01, DV01, etc.)
//! - `market_validation`: ISDA standards and market benchmarks
//! - `edge_cases`: Boundary conditions and error handling
//! - `credit_derivatives_edge_cases`: Recovery boundary, survival floor tests
//! - `conventions`: Calendar and settlement delay validation

mod test_cds_construction;
mod test_cds_conventions;
mod test_cds_edge_cases;
mod test_cds_market_validation;
mod test_cds_metrics;
mod test_cds_pricing;
mod test_credit_derivatives_edge_cases;
mod test_par_spread_roundtrip;

mod test_cds_pricing_2;
mod test_upfront;

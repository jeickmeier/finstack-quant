//! Comprehensive unit tests for CDS Tranche instrument.
//!
//! Test organization:
//! - `helpers`: Shared test fixtures and utilities
//! - `config_tests`: Configuration and parameter validation
//! - `pricing_tests`: Core pricing functionality (PV, legs, schedules)
//! - `risk_metrics_tests`: Risk sensitivities (CS01, correlation delta, JTD)
//! - `expected_loss_tests`: Expected loss calculations (homogeneous & heterogeneous)
//! - `numerical_stability_tests`: Boundary conditions and extreme values
//! - `market_standards_tests`: Market convention and methodology validation
//! - `metrics_calculator_tests`: Metric framework integration tests

mod config_tests;
mod expected_loss_tests;
mod helpers;
mod market_standards_tests;
mod metrics_calculator_tests;
mod numerical_stability_tests;
mod pricing_tests;
mod risk_metrics_tests;

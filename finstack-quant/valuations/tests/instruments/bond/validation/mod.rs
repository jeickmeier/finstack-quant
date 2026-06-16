//! Market validation tests for bond calculations.
//!
//! These tests validate calculations against published industry standards:
//! - Textbook examples (Fabozzi, Hull)
//! - Market calculator outputs (Bloomberg, FactSet)
//! - Cross-metric relationships
//!
//! Includes:
//! - `daycount_consistency`: Verifies consistent day count usage across metrics
//! - `market_benchmarks`: External benchmark validation
//! - `metric_relationships`: Cross-metric consistency checks
//! - `settlement_conventions`: PV anchoring and quote-date consistency

mod daycount_consistency;
mod market_benchmarks;
mod metric_relationships;
mod settlement_conventions;
mod tree_calibration;

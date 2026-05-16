//! Calibration test suite.
//!
//! All tests in this module target the plan-driven calibration API
//! (`finstack_valuations::calibration`).
//!
//! ## Test Organization
//!
//! - `bootstrap` - Determinism and smoke tests for curve bootstrapping
//! - `repricing` - Repricing accuracy tests for calibrated curves
//! - `config` - Configuration helpers and validation rules
//! - `finstack_config` - Finstack-specific config integration
//! - `serialization` - Serde roundtrip tests for calibration types
//! - `builder` - Simple calibration builder API tests
//! - `hazard_curve` - Hazard/credit curve calibration
//! - `inflation` - Inflation curve calibration and conventions
//! - `swaption_vol` - Swaption volatility surface calibration
//! - `base_correlation` - Base correlation surface calibration
//! - `failure_modes` - Engine error handling and failure scenarios
//! - `explainability` - Explanation trace generation
//! - `validation` - Curve and surface validation tests
//! - `quote_construction` - All quote types instrument construction verification
//! - `bloomberg_accuracy` - Bloomberg benchmark accuracy tests
//! - `v2_engine_smoke` - V2 API smoke tests

mod base_correlation;
mod bloomberg_accuracy;
mod bootstrap;
mod builder;
mod config;
mod diagnostics;
mod explainability;
mod failure_modes;
mod finstack_config;
mod hazard_curve;
mod inflation;
mod parametric;
mod quote_construction;
mod reference_envelopes;
mod repricing;
mod serialization;
mod swaption_vol;
mod v2_engine_smoke;
mod validation;

mod term_structures;

pub(crate) mod tolerances;

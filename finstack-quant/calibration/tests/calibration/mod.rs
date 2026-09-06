//! Calibration test suite.
//!
//! All tests in this module target the plan-driven calibration API
//! (`finstack_quant_calibration`).
//!
//! ## Test Organization
//!
//! - `bootstrap` - Determinism and smoke tests for curve bootstrapping
//! - `hazard_curve` - Quote-space credit recalibration and replay invariants
//! - `repricing` - Repricing accuracy tests for calibrated curves
//! - `config` - Configuration helpers and validation rules
//! - `finstack_config` - Finstack Quant-specific config integration
//! - `serialization` - Serde roundtrip tests for calibration types
//! - `builder` - Simple calibration builder API tests
//! - `hazard_curve` - Hazard/credit curve calibration
//! - `inflation` - Inflation curve calibration and conventions
//! - `swaption_vol` - Swaption volatility surface calibration
//! - `svi_surface` - SVI equity volatility surface calibration
//! - `base_correlation` - Base correlation surface calibration
//! - `failure_modes` - Engine error handling and failure scenarios
//! - `explainability` - Explanation trace generation
//! - `validation` - Curve and surface validation tests
//! - `quote_construction` - All quote types instrument construction verification
//! - `bloomberg_accuracy` - Bloomberg benchmark accuracy tests
//! - `engine_smoke` - calibration engine smoke tests

mod base_correlation;
mod bloomberg_accuracy;
mod bootstrap;
mod builder;
mod config;
mod diagnostics;
mod engine_smoke;
mod explainability;
mod failure_modes;
mod finstack_config;
mod hull_white_regressions;
mod inflation;
mod market_quote;
mod parametric;
mod quote_schemas;
mod reference_envelopes;
mod schema_parity;
mod serialization;
mod svi_surface;
mod swaption_vol;
mod validation;

mod term_structures;

pub(crate) mod tolerances;

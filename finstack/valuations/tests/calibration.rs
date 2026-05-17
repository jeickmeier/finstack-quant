//! Calibration test suite entry point.
//!
//! This module consolidates tests for:
//!
//! - **bootstrap**: Curve bootstrapping determinism and smoke tests
//! - **repricing**: Repricing accuracy for calibrated curves
//! - **config**: Configuration helpers and validation rules
//! - **finstack_config**: Finstack-specific config integration
//! - **serialization**: Serde roundtrip tests for calibration types
//! - **builder**: Simple calibration builder API tests
//! - **hazard_curve**: Hazard/credit curve calibration
//! - **inflation**: Inflation curve calibration and conventions
//! - **swaption_vol**: Swaption volatility surface calibration
//! - **svi_surface**: SVI equity volatility surface calibration
//! - **base_correlation**: Base correlation surface calibration
//! - **failure_modes**: Engine error handling and failure scenarios
//! - **explainability**: Explanation trace generation
//! - **validation**: Curve and surface validation tests
//! - **quote_construction**: All-types calibration quote construction verification
//! - **bloomberg_accuracy**: Bloomberg benchmark accuracy tests
//! - **v2_engine_smoke**: V2 API smoke tests
//! - **term_structures/**: Independent term structure property tests
//!
//! Run all calibration tests:
//! ```bash
//! cargo test --test calibration
//! ```

// ============================================================================
// Shared Test Utilities
// ============================================================================

/// Common test utilities: fixtures, tolerances, assertions, builders
#[path = "common/mod.rs"]
mod common;

#[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
mod finstack_test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_utils.rs"
    ));
}

// ============================================================================
// Calibration Tests
// ============================================================================

#[path = "calibration/mod.rs"]
mod calibration;

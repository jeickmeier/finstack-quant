//! Shared calibration abstractions for factor-model implementations.

/// Common shape for factor-model calibrators.
///
/// The trait is intentionally small: each asset class can choose its own
/// configuration, input panel, model artifact, and diagnostics type while still
/// presenting the same `calibrate(inputs) -> model` workflow.
pub trait FactorCalibrator {
    /// Calibrator configuration type.
    type Config;
    /// Input data required to run calibration.
    type Inputs;
    /// Calibrated model artifact produced by the calibrator.
    type Model;
    /// Diagnostics embedded in or associated with the calibrated model.
    type Diagnostics;

    /// Borrow the calibrator configuration.
    fn config(&self) -> &Self::Config;

    /// Run calibration and return the calibrated model artifact.
    ///
    /// # Errors
    ///
    /// Returns a core error when inputs are structurally invalid or calibration
    /// cannot produce a valid model artifact.
    fn calibrate(&self, inputs: Self::Inputs) -> Result<Self::Model, finstack_core::Error>;

    /// Borrow diagnostics from a calibrated model artifact.
    fn diagnostics(model: &Self::Model) -> &Self::Diagnostics;
}

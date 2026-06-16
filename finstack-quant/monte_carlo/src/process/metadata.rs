//! Process metadata extraction for captured-path datasets.
//!
//! [`ProcessMetadata`] lets a stochastic process describe its own parameter set
//! and state-vector layout for downstream consumers of
//! [`crate::paths::PathDataset`]. This metadata is optional for plain pricing but
//! important when captured paths need to be interpreted outside the engine.

use crate::paths::ProcessParams;

/// Trait for extracting captured-path metadata from a process.
///
/// Implementations should return a [`ProcessParams`] snapshot that is stable
/// enough for visualization, diagnostics, and serialization. In particular:
///
/// - `process_type` should identify the model family, such as `"GBM"` or `"Heston"`.
/// - `parameters` should use clear, documented keys such as `r`, `q`, `sigma`,
///   `kappa`, or `theta`.
/// - `factor_names` should describe the order of entries in captured state vectors.
/// - `correlation`, when present, should be a row-major square matrix aligned with
///   `factor_names`.
pub trait ProcessMetadata {
    /// Return metadata describing the process configuration and state layout.
    fn metadata(&self) -> ProcessParams;
}

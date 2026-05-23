//! Credit factor-model calibration, decomposition, and sensitivity matrix
//! primitives.
//!
//! This crate was carved out of `finstack-valuations` to shrink the umbrella
//! crate's edit-rebuild loop. Callers import directly from
//! `finstack_factor_model::*`; there is no longer a re-export façade in
//! `finstack-valuations`.
//!
//! # Modules
//!
//! - [`credit_calibration`]: Sequential "peel-the-onion" calibrator producing
//!   a [`finstack_core::factor_model::credit_hierarchy::CreditFactorModel`]
//!   from sparse issuer-spread history.
//! - [`credit_decomposition`]: Pure decomposition of issuer spreads into
//!   hierarchy-level factor values.
//! - [`sensitivity_matrix`]: Positions × factors sensitivity matrix layout.
//!
//! Engines that take `&dyn Instrument` (delta and full-repricing engines)
//! live in `finstack-portfolio::sensitivity` because they depend on the
//! instrument trait surface.

#![deny(missing_docs)]

pub mod credit_calibration;
pub mod credit_decomposition;
pub mod sensitivity_matrix;

pub use credit_calibration::{
    BetaShrinkage, BucketSizeThresholds, CovarianceStrategy, CreditCalibrationConfig,
    CreditCalibrationInputs, CreditCalibrator, GenericFactorSeries, HistoryPanel, IssuerTagPanel,
    PanelSpace, VolModelChoice,
};
pub use credit_decomposition::{
    decompose_levels, decompose_period, DecompositionError, LevelValuesAtDate, LevelValuesDelta,
    LevelsAtDate, PeriodDecomposition,
};
pub use sensitivity_matrix::SensitivityMatrix;

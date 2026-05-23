//! Factor-model integration helpers — re-export façade.
//!
//! Preserves the historical `finstack_valuations::factor_model::*` public
//! surface for the **instrument-free** pieces. The actual implementations live
//! in:
//!
//! - [`finstack_factor_model`]: calibration, decomposition, and
//!   `SensitivityMatrix` data type.
//! - [`crate::instruments::dependencies_flatten`]: market-dependency flattening
//!   helper (re-exported here as `decompose` for binding stability).
//!
//! The **instrument-dependent** factor sensitivity engines
//! (`DeltaBasedEngine`, `FullRepricingEngine`, `ScenarioGrid`,
//! `FactorSensitivityEngine`, position parsing, JSON façade) have moved to
//! `finstack_portfolio::sensitivity`. Callers should import them from there.

pub use finstack_factor_model::credit_calibration::{
    BetaShrinkage, BucketSizeThresholds, CovarianceStrategy, CreditCalibrationConfig,
    CreditCalibrationInputs, CreditCalibrator, GenericFactorSeries, HistoryPanel, IssuerTagPanel,
    PanelSpace, VolModelChoice,
};
pub use finstack_factor_model::credit_decomposition::{
    decompose_levels, decompose_period, DecompositionError, LevelValuesAtDate, LevelValuesDelta,
    LevelsAtDate, PeriodDecomposition,
};
pub use finstack_factor_model::sensitivity_matrix::SensitivityMatrix;
pub use finstack_factor_model::{credit_calibration, credit_decomposition};

pub use crate::instruments::dependencies_flatten::decompose;

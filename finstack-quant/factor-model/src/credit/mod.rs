//! Credit factor hierarchy artifacts, calibration, and decomposition.

/// Credit hierarchy calibration from issuer spread histories.
pub mod calibration;
/// Credit factor decomposition across hierarchy levels.
pub mod decomposition;
/// Credit factor hierarchy artifact types.
pub mod hierarchy;
mod peel;

pub use calibration::{
    BetaShrinkage, BucketSizeThresholds, CovarianceStrategy, CreditCalibrationConfig,
    CreditCalibrationInputs, CreditCalibrator, GenericFactorSeries, HistoryPanel, IssuerTagPanel,
    PanelSpace, VolModelChoice,
};
pub use decomposition::{
    decompose_levels, decompose_period, DecompositionError, LevelValuesAtDate, LevelValuesDelta,
    LevelsAtDate, PeriodDecomposition,
};
pub use hierarchy::*;

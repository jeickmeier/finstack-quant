//! Factor-model integration helpers for the valuations crate.

pub mod credit_calibration;
pub mod credit_decomposition;
mod decompose;
mod json;
mod positions;
pub mod sensitivity;

pub use credit_calibration::{
    BetaShrinkage, BucketSizeThresholds, CovarianceStrategy, CreditCalibrationConfig,
    CreditCalibrationInputs, CreditCalibrator, GenericFactorSeries, HistoryPanel, IssuerTagPanel,
    PanelSpace, VolModelChoice,
};
pub use credit_decomposition::{
    decompose_levels, decompose_period, DecompositionError, LevelValuesAtDate, LevelValuesDelta,
    LevelsAtDate, PeriodDecomposition,
};
pub use decompose::decompose;
pub use json::{
    compute_factor_sensitivities_from_json, compute_factor_sensitivities_json,
    compute_pnl_profiles_from_json, compute_pnl_profiles_json, parse_bump_config_json,
    parse_factor_definitions_json, FactorPnlProfileJson, SensitivityMatrixJson,
    DEFAULT_PNL_SCENARIO_POINTS,
};
pub use positions::{parse_positions_json, pricing_positions, ParsedPosition};
pub use sensitivity::{
    mapping_to_market_bumps, DeltaBasedEngine, FactorPnlProfile, FactorSensitivityEngine,
    FullRepricingEngine, ScenarioGrid, SensitivityMatrix,
};

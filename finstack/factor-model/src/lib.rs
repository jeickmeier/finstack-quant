//! Canonical factor-modelling primitives, matching, credit calibration,
//! and sensitivity matrix for finstack.
//!
//! Multi-asset factor modelling is the first-class concept of this crate.
//! Credit hierarchy calibration is one current implementation; rates,
//! equity, volatility, commodity, and inflation factors are expressed
//! through generic [`FactorType`] and [`FactorDefinition`].

#![deny(missing_docs)]

/// Shared calibration abstractions for factor-model implementations.
pub mod calibration;
/// Factor-model run configuration, risk measures, and bump sizing.
pub mod config;
/// Factor covariance matrix storage and validation.
pub mod covariance;
/// Credit factor hierarchy artifacts, calibration, and decomposition.
pub mod credit;
/// Factor-model specific error and unmatched-dependency policy types.
pub mod error;
/// Matching primitives and built-in matcher components.
pub mod matching;
mod parse;
/// Generic factor identifiers, definitions, and market dependencies.
pub mod primitives;
/// Positions × factors sensitivity matrix storage.
pub mod sensitivity_matrix;

pub use calibration::FactorCalibrator;
pub use config::{BumpSizeConfig, FactorBumpUnit, FactorModelConfig, PricingMode, RiskMeasure};
pub use covariance::FactorCovarianceMatrix;
pub use credit::{
    decompose_levels, decompose_period, BetaShrinkage, BucketSizeThresholds, CovarianceStrategy,
    CreditCalibrationConfig, CreditCalibrationInputs, CreditCalibrator, DecompositionError,
    GenericFactorSeries, HistoryPanel, IssuerTagPanel, LevelValuesAtDate, LevelValuesDelta,
    LevelsAtDate, PanelSpace, PeriodDecomposition, VolModelChoice,
};
pub use error::{FactorModelError, UnmatchedPolicy};
pub use finstack_core::{Error, InputError, Result};
pub use matching::{
    bucket_factor_id, AttributeFilter, CascadeMatcher, CreditHierarchicalConfig, DependencyFilter,
    FactorMatchEntry, FactorMatchError, FactorMatcher, FactorNode, HierarchicalConfig,
    HierarchicalMatcher, MappingRule, MappingTableMatcher, MatchingConfig,
    CREDIT_GENERIC_FACTOR_ID, ISSUER_ID_META_KEY,
};
pub use primitives::{
    CurveType, DependencyType, FactorDefinition, FactorId, FactorType, MarketDependency,
    MarketMapping,
};
pub use sensitivity_matrix::SensitivityMatrix;

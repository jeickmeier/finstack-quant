//! Canonical factor-modelling primitives, matching, credit calibration,
//! and sensitivity matrix for finstack_quant.
//!
//! Multi-asset factor modelling is the first-class concept of this crate.
//! Credit hierarchy calibration is one current implementation; rates,
//! equity, volatility, commodity, and inflation factors are expressed
//! through generic [`FactorType`] and [`FactorDefinition`].
//!
//! # Module Guide
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`primitives`] | `FactorId`, `FactorType`, `FactorDefinition`, market dependencies |
//! | [`matching`] | Mapping-table, cascade, and hierarchy matchers for dependency-to-factor resolution |
//! | [`credit`] | Credit hierarchy artifacts, calibration, and spread decomposition |
//! | [`config`] | `FactorModelConfig`, `RiskMeasure`, `PricingMode`, bump sizing |
//! | [`covariance`] | `FactorCovarianceMatrix` with symmetry and PSD validation |
//! | [`error`] | `FactorModelError` and `UnmatchedPolicy` |
//! | [`sensitivity_matrix`] | `SensitivityMatrix`: positions × factors dense layout |
//!
//! # Quick Start
//!
//! ```rust
//! use finstack_quant_factor_model::{
//!     FactorDefinition, FactorId, FactorType, MarketMapping,
//! };
//! use finstack_quant_core::market_data::bumps::BumpUnits;
//! use finstack_quant_core::types::CurveId;
//!
//! let def = FactorDefinition {
//!     id: FactorId::new("USD_10Y_SWAP"),
//!     factor_type: FactorType::Rates,
//!     market_mapping: MarketMapping::CurveParallel {
//!         curve_ids: vec![CurveId::new("USD-SOIS")],
//!         units: BumpUnits::RateBp,
//!     },
//!     description: Some("USD 10Y swap rate".to_string()),
//! };
//! assert_eq!(def.factor_type, FactorType::Rates);
//! ```
//!
//! # Conventions
//!
//! - Factor identifiers (`FactorId`) are string-backed and case-sensitive.
//! - Covariance entries are annualized (co)variances in each factor's canonical
//!   bump unit (bps for rates/credit, % for equity/commodity/FX, vol points for
//!   volatility). See [`FactorCovarianceMatrix`] for the units contract.
//! - Credit decomposition enforces the reconciliation invariant to absolute
//!   tolerance `1e-10`.
//! - Pricing engines that consume `FactorModelConfig` live in
//!   `finstack-quant-portfolio::sensitivity` because they depend on the
//!   instrument trait surface.

#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
#![doc(test(attr(allow(clippy::expect_used))))]

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

pub use config::{BumpSizeConfig, FactorBumpUnit, FactorModelConfig, PricingMode, RiskMeasure};
pub use covariance::FactorCovarianceMatrix;
pub use error::{FactorModelError, UnmatchedPolicy};
pub use finstack_quant_core::{Error, Result};
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

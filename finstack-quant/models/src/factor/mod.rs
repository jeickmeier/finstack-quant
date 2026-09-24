//! Canonical factor-modelling primitives, matching, credit calibration,
//! and sensitivity matrix for finstack_quant.
//!
//! Multi-asset factor modelling is the first-class concept of this module.
//! Credit hierarchy calibration is implemented for credit factors; rates,
//! equity, volatility, commodity, and inflation factors are expressed
//! through generic [`FactorType`] and [`FactorDefinition`].
//!
//! # Public API
//!
//! Generic configuration, covariance, envelope, factor, dependency, and
//! sensitivity types are exported at the module root. Public submodules are:
//!
//! - [`matching`] for dependency-to-factor matching.
//! - [`credit`] for credit hierarchy artifacts, calibration, and decomposition.
//! - [`schema`] for versioned JSON Schema contracts.
//!
//! # Quick Start
//!
//! ```rust
//! use finstack_quant_models::factor::{
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
//!   bump unit (bp for rates/credit, % for equity/commodity/FX, vol points for
//!   volatility). See [`FactorCovarianceMatrix`] for the units contract.
//! - Credit callers pass decimal spreads; internals, histories, and `Σ` are
//!   bp. `decompose_period` is algebraic differencing (it does not enforce
//!   a numerical tolerance).
//! - Pricing engines that consume `FactorModelConfig` live in
//!   `finstack-quant-portfolio::sensitivity` because they depend on the
//!   instrument trait surface.
//!
//! # References
//!
//! - Factor models and exposure-based risk: `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
//! - Euler capital allocation: `docs/REFERENCES.md#tasche-2008-capital-allocation`

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
mod config;
/// Factor covariance matrix storage and validation.
mod covariance;
/// Credit factor hierarchy artifacts, calibration, and decomposition.
pub mod credit;
/// Versioned persistence envelope for factor-model configuration.
mod envelope;
/// Matching primitives and built-in matcher components.
pub mod matching;
/// Generic factor identifiers, definitions, and market dependencies.
mod primitives;
/// Product-independent factor and position risk kernels.
pub mod risk;
/// JSON Schema generation helpers for factor-model contracts.
#[cfg(feature = "json-schema")]
pub mod schema;
/// Positions × factors sensitivity matrix storage.
mod sensitivity_matrix;

pub use config::{BumpSizeConfig, FactorModelConfig, PricingMode, RiskMeasure, UnmatchedPolicy};
pub use covariance::FactorCovarianceMatrix;
pub use envelope::{
    FactorModelConfigEnvelope, FactorModelConfigSchema, FACTOR_MODEL_CONFIG_CONTRACT,
};
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

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`. Without it those samples
/// are dead text and rot silently on any API change.
#[cfg(doctest)]
#[doc = include_str!("README.md")]
struct ReadmeDoctests;

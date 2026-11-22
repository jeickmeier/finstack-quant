//! Credit factor hierarchy artifact types (serde-first data model).
//!
//! This module defines the canonical calibration artifact for the credit
//! factor hierarchy. The central type is
//! [`CreditFactorModel`](crate::credit::hierarchy::CreditFactorModel), a
//! fully self-contained JSON artifact produced by offline calibration and
//! consumed at runtime by attribution, risk, and vol-forecast pipelines.
//!
//! # Schema version
//!
//! [`CreditFactorModelSchema::CURRENT`](crate::credit::hierarchy::CreditFactorModelSchema::CURRENT)
//! is the exact
//! `"finstack_quant.credit_factor_model/1"` marker stored in the model's
//! `schema` field. Consumers must check this field before trusting any other
//! field.
//!
//! # Usage
//!
//! ```rust
//! use finstack_quant_factor::credit::hierarchy::{
//!     CreditFactorModel, CreditFactorModelSchema,
//! };
//!
//! // Deserialize from JSON, then validate internal consistency.
//! let json = r#"{
//!   "schema": "finstack_quant.credit_factor_model/1",
//!   "as_of": "2024-03-29",
//!   "calibration_window": { "start": "2022-03-29", "end": "2024-03-29" },
//!   "policy": "globally_off",
//!   "generic_factor": { "name": "CDX IG", "series_id": "cdx.ig.5y" },
//!   "hierarchy": { "levels": ["rating", "region", "sector"] },
//!   "panel_frequency": "monthly",
//!   "use_returns_or_levels": "returns",
//!   "bucket_weighting": "equal",
//!   "config": {
//!     "factors": [],
//!     "covariance": { "n": 0, "factor_ids": [], "data": [] },
//!     "matching": { "mapping_table": [] }
//!   },
//!   "issuer_betas": [],
//!   "anchor_state": { "pc": 0.0, "by_level": [] },
//!   "static_correlation": { "factor_ids": [], "data": [] },
//!   "vol_state": { "factors": {}, "idiosyncratic": {} },
//!   "factor_histories": null,
//!   "diagnostics": {
//!     "mode_counts": {},
//!     "bucket_sizes_per_level": [],
//!     "fold_ups": [],
//!     "r_squared_histogram": null,
//!     "tag_taxonomy": {}
//!   }
//! }"#;
//!
//! let model = CreditFactorModel::from_json(json).expect("valid artifact");
//! assert_eq!(model.schema, CreditFactorModelSchema::CURRENT);
//! ```
//!
//! # Design notes
//!
//! - Stable artifact structs use `#[serde(deny_unknown_fields)]` to catch schema
//!   drift early. `CreditCalibrationDiagnostics` is the explicitly open extension
//!   object for additive diagnostic fields.
//! - All keyed maps use `BTreeMap` for deterministic serialization order.

mod correlation;
mod model;
mod spec;

#[cfg(test)]
mod tests;

pub use correlation::FactorCorrelationMatrix;
pub use model::{
    CreditCalibrationDiagnostics, CreditFactorModel, CreditFactorModelSchema, DateRange,
    FactorHistories, FoldUpRecord, GenericFactorSpec, LevelAnchor, LevelsAtAnchor, VolModel,
    VolState,
};
pub use spec::{
    AdderVolSource, CreditHierarchySpec, FitQuality, HierarchyDimension, IssuerBetaMode,
    IssuerBetaOverride, IssuerBetaPolicy, IssuerBetaRow, IssuerBetas, IssuerTags, dimension_key,
};

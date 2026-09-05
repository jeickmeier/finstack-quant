//! Deterministic calibrator that produces a
//! [`CreditFactorModel`][crate::factor::credit::hierarchy::CreditFactorModel] artifact from
//! sparse issuer-spread history.
//!
//! # Algorithm overview
//!
//! The calibration is a sequential "peel-the-onion" identical in structure to
//! [`crate::factor::credit::decomposition::decompose_levels`] but operating
//! on a *time series* of issuer spreads rather than a single snapshot:
//!
//! 1. Classify each issuer as `IssuerBeta` or `BucketOnly` based on the
//!    [`IssuerBetaPolicy`][crate::factor::credit::hierarchy::IssuerBetaPolicy] and
//!    per-issuer [`IssuerBetaOverride`][crate::factor::credit::hierarchy::IssuerBetaOverride].
//! 2. Optionally convert the spread panel to a return panel (default).
//! 3. Inventory hierarchy buckets and fold up under-populated buckets.
//! 4. Regress each issuer's residual on the generic factor (PC peel).
//! 5. Peel hierarchy levels one at a time: cross-sectional bucket means become
//!    bucket factor returns, and `IssuerBeta` issuers fit a per-level β.
//! 6. After the last level, the remaining residual is the issuer adder.
//! 7. Anchor every factor's level value at `as_of` using the same peeling logic
//!    on a single observation in level space.
//! 8. Estimate per-factor variance via the configured vol model (sample or RiskMetrics EWMA).
//! 9. Assemble correlation and covariance per
//!    [`CovarianceStrategy`][crate::factor::credit::calibration::CovarianceStrategy]:
//!    `Diagonal` → identity ρ, Σ = diag(σ²); `Ridge` → sample ρ (PSD-repaired
//!    if needed), Σ = D·ρ·D + α·I; `FullSampleRepaired` → sample ρ repaired
//!    to PSD, Σ = D·ρ_repaired·D; `LedoitWolf` → Σ and ρ from the Ledoit-Wolf
//!    identity-target shrinkage estimator over complete-case observations.
//! 10. Assemble [`crate::factor::FactorModelConfig`] with `MatchingConfig::CreditHierarchical`.
//! 11. Build [`CalibrationDiagnostics`][crate::factor::credit::hierarchy::CalibrationDiagnostics]
//!     from the bookkeeping above.
//! 12. Return the assembled
//!     [`CreditFactorModel`][crate::factor::credit::hierarchy::CreditFactorModel] after a final
//!     [`CreditFactorModel::validate`][crate::factor::credit::hierarchy::CreditFactorModel::validate]
//!     check.
//!
//! # Determinism
//!
//! Every keyed map is a [`std::collections::BTreeMap`] and every iteration order is stable. Two
//! calibrations with the same inputs serialize to byte-identical JSON.
//!
//! # Anchoring
//!
//! The anchoring step (step 7) implements the same math as
//! [`decompose_levels`][crate::factor::credit::decomposition::decompose_levels]
//! but is called via a private helper because we don't yet have a fully
//! assembled [`CreditFactorModel`][crate::factor::credit::hierarchy::CreditFactorModel]
//! at that point in the pipeline.
//!
//! # Determinism note on OLS
//!
//! The OLS slope `β = Cov(y, x) / Var(x)` is delegated to
//! `finstack_quant_analytics::beta`, which implements the same math
//! and is deterministic for the same input slice.

mod assemble;
mod calibrator;
mod config;
mod inputs;
mod inventory;
mod panel;
mod peel_fit;
mod statistics;
mod validation;

pub use calibrator::CreditCalibrator;
pub use config::{
    BetaShrinkage, BucketSizeThresholds, BucketWeighting, CovarianceStrategy,
    CreditCalibrationConfig, PanelFrequency, PanelSpace, VolModelChoice,
};
pub use inputs::{CreditCalibrationInputs, GenericFactorSeries, HistoryPanel, IssuerTagPanel};

#[cfg(test)]
mod tests;

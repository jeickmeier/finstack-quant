//! Canonical model-version strings for audit trails and calibration reports.
//!
//! # Why centralize
//!
//! Each calibration target previously emitted its own hard-coded
//! `CalibrationReport::model_version` string, so any version bump touched
//! several unrelated files and the strings were visible only to reviewers who
//! happened to grep for them.
//!
//! Centralizing them here means:
//!
//! * A reviewer can see the full set of active model versions at a glance
//!   when auditing the calibration pipeline.
//! * A version bump touches one file.
//! * Consumers that persist the `model_version` field to audit logs see
//!   consistent, drift-free identifiers regardless of which entry point
//!   produced the report.
//!
//! # Naming convention
//!
//! Each constant follows the pattern:
//!
//! ```text
//! <ModelName> v<semver> [(implementation notes in parentheses)]
//! ```
//!
//! where `<semver>` is a conceptual version tracking the calibration
//! methodology (not the crate version), and the parenthetical notes call
//! out non-default algorithm choices (e.g. "Jamshidian decomposition,
//! vega-weighted, multi-start"). Bump the version whenever a change to
//! the calibration math would plausibly move prices under otherwise
//! identical inputs.

/// Hull-White 1F calibration (Jamshidian decomposition, vega-weighted
/// residuals, Halton multi-start).
pub const HULL_WHITE_1F: &str =
    "Hull-White 1F (Jamshidian decomposition, vega-weighted, multi-start)";

/// Multi-curve OIS discount curve bootstrap, produced by the default
/// target at `finstack_quant_valuations::calibration::targets::discount`.
pub const MULTI_CURVE_OIS_DISCOUNT: &str = "Multi-curve OIS discounting v1.0";

/// ISDA Standard Model v1.8.2, used by the CDS hazard-curve bootstrap in
/// `finstack_quant_valuations::calibration::targets::hazard`. The version
/// mirrors the ISDA reference implementation revision.
pub const ISDA_STANDARD_MODEL: &str = "ISDA Standard Model v1.8.2";

/// Student-t copula calibration (CDX tranches, DeMarta-McNeil
/// parameterization, runtime-generated Gauss-Laguerre quadrature).
pub const STUDENT_T_COPULA: &str = "Student-t Copula Calibration v1.0";

/// SVI volatility surface calibration with Gatheral total-variance
/// interpolation across expiries. The "v1.1" bump distinguishes this
/// interpolation from the earlier parameter-space linear interpolation
/// which admitted calendar-spread arbitrage.
pub const SVI_SURFACE: &str = "SVI v1.1 (Gatheral total-variance interpolation)";

#[cfg(test)]
mod tests {
    use super::*;

    /// The version strings must remain non-empty and unique so
    /// `CalibrationReport::model_version` is never ambiguous about
    /// which model produced a given report.
    #[test]
    fn all_model_versions_are_nonempty_and_distinct() {
        let all = [
            HULL_WHITE_1F,
            MULTI_CURVE_OIS_DISCOUNT,
            ISDA_STANDARD_MODEL,
            STUDENT_T_COPULA,
            SVI_SURFACE,
        ];
        for v in all {
            assert!(!v.is_empty(), "model-version string must be non-empty");
        }
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(
                    all[i], all[j],
                    "model-version strings must be distinct ({} vs {})",
                    all[i], all[j]
                );
            }
        }
    }
}

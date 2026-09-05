//! Factor-model run configuration, risk measures, and bump sizing.
//!
//! [`FactorModelConfig`] is the top-level configuration consumed by pricing
//! engines in `finstack-quant-portfolio::sensitivity`. It bundles factor
//! definitions, a matching config, an optional covariance matrix, and
//! sensitivity extraction settings ([`PricingMode`], [`RiskMeasure`],
//! [`BumpSizeConfig`]).

use super::covariance::FactorCovarianceMatrix;
use super::matching::MatchingConfig;
use super::primitives::definition::FactorDefinition;
use super::primitives::factor_types::{FactorId, FactorType};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Policy for handling dependencies that do not match any factor.
///
/// Serializes in `snake_case`, matching the crate-wide wire convention and
/// this type's own `Display` representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UnmatchedPolicy {
    /// Fail immediately when any dependency is unmatched.
    ///
    /// Use this in production risk runs where dropping unmapped risk would be a
    /// control failure.
    Strict,
    /// Roll unmatched risk into a residual bucket.
    ///
    /// Use this when the engine should preserve total exposure while making the
    /// unmatched component explicit as residual risk.
    #[default]
    Residual,
    /// Continue but surface a warning to the caller.
    ///
    /// Suitable for exploratory workflows where visibility matters but a hard
    /// failure would be too disruptive.
    Warn,
}

impl fmt::Display for UnmatchedPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Strict => write!(f, "strict"),
            Self::Residual => write!(f, "residual"),
            Self::Warn => write!(f, "warn"),
        }
    }
}

/// Strategy used when extracting factor sensitivities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PricingMode {
    /// Use central finite differences to approximate linear deltas.
    ///
    /// This is the lightweight choice when a downstream engine can reprice under
    /// small symmetric bumps and the risk report only needs first-order factor
    /// sensitivities.
    DeltaBased,
    /// Reprice across a scenario grid and derive deltas from the P&L profile.
    ///
    /// Use this when the portfolio workflow needs richer scenario behavior than
    /// a single small bump can capture, at the cost of more repricing work.
    FullRepricing,
}

impl fmt::Display for PricingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeltaBased => write!(f, "delta_based"),
            Self::FullRepricing => write!(f, "full_repricing"),
        }
    }
}

/// Risk measure used when aggregating factor exposures.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
#[non_exhaustive]
pub enum RiskMeasure {
    /// Aggregate exposures using factor covariance and portfolio variance.
    #[default]
    Variance,
    /// Aggregate exposures using portfolio volatility.
    Volatility,
    /// Aggregate exposures using Value at Risk at a fixed one-sided loss confidence level.
    ///
    /// This assumes the downstream aggregation engine is interpreting the factor
    /// model as a parametric, one-period loss distribution rather than a full
    /// historical or Monte Carlo simulation.
    ///
    /// # Sign convention
    ///
    /// VaR is reported as a **negative** number on the P&L axis: for a long-risk
    /// portfolio, `total_risk` at 99% is approximately `-sigma * z_{0.99}`.
    /// Factor contributions carry the same sign as the total. Downstream
    /// aggregators and visualizations rely on this convention.
    #[serde(rename = "var")]
    VaR {
        /// Confidence level in the open interval `(0.5, 1)`.
        #[cfg_attr(feature = "json-schema", schemars(extend("exclusiveMinimum" = 0.5, "exclusiveMaximum" = 1.0)))]
        confidence: f64,
    },
    /// Aggregate exposures using expected shortfall at a fixed one-sided loss confidence level.
    ///
    /// As with [`Self::VaR`], this is intended for parametric factor-model
    /// aggregation rather than full-path simulation, and ES is reported as a
    /// **negative** number using the P&L sign convention.
    ExpectedShortfall {
        /// Confidence level in the open interval `(0.5, 1)`.
        #[cfg_attr(feature = "json-schema", schemars(extend("exclusiveMinimum" = 0.5, "exclusiveMaximum" = 1.0)))]
        confidence: f64,
    },
}

impl RiskMeasure {
    /// Validate any embedded confidence levels before downstream risk calculations use them.
    ///
    /// Variance and volatility carry no additional parameters. Parametric VaR
    /// and expected shortfall require a finite one-sided loss confidence in
    /// `(0.5, 1)`; their negative P&L sign convention is described on the enum
    /// variants.
    ///
    /// # Errors
    ///
    /// Returns an error when a VaR or expected-shortfall confidence is NaN,
    /// infinite, less than or equal to `0.5`, or greater than or equal to `1`.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        match self {
            Self::Variance | Self::Volatility => Ok(()),
            Self::VaR { confidence } | Self::ExpectedShortfall { confidence } => {
                validate_confidence(*confidence)
            }
        }
    }
}

fn validate_confidence(confidence: f64) -> finstack_quant_core::Result<()> {
    if confidence.is_finite() && confidence > 0.5 && confidence < 1.0 {
        Ok(())
    } else {
        Err(finstack_quant_core::Error::Validation(format!(
            "RiskMeasure confidence must be in the open interval (0.5, 1), got {confidence}"
        )))
    }
}

impl<'de> Deserialize<'de> for RiskMeasure {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum RiskMeasureSerde {
            Variance,
            Volatility,
            #[serde(rename = "var")]
            VaR {
                confidence: f64,
            },
            ExpectedShortfall {
                confidence: f64,
            },
        }

        let measure = match RiskMeasureSerde::deserialize(deserializer)? {
            RiskMeasureSerde::Variance => Self::Variance,
            RiskMeasureSerde::Volatility => Self::Volatility,
            RiskMeasureSerde::VaR { confidence } => Self::VaR { confidence },
            RiskMeasureSerde::ExpectedShortfall { confidence } => {
                Self::ExpectedShortfall { confidence }
            }
        };

        measure.validate().map_err(serde::de::Error::custom)?;
        Ok(measure)
    }
}

/// Per-factor-type bump magnitudes for finite-difference sensitivity engines.
///
/// Unknown fields are rejected on deserialization: every field here has a
/// serde default, so a typo'd key (e.g. `"credit_bp"`) would otherwise be
/// silently dropped and the bump would silently revert to 1.0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BumpSizeConfig {
    /// Default rates bump in basis points.
    #[serde(default = "default_one")]
    pub rates_bp: f64,
    /// Default credit bump in basis points.
    #[serde(default = "default_one")]
    pub credit_bp: f64,
    /// Default equity spot bump in percent.
    #[serde(default = "default_one")]
    pub equity_pct: f64,
    /// Default FX spot bump in percent.
    #[serde(default = "default_one")]
    pub fx_pct: f64,
    /// Default volatility bump in vol points (`1.0` = one vol point =
    /// `0.01` absolute vol).
    #[serde(default = "default_one")]
    pub vol_points: f64,
    /// Per-factor overrides that take precedence over factor-type defaults.
    #[serde(default)]
    pub overrides: BTreeMap<FactorId, f64>,
}

fn default_one() -> f64 {
    1.0
}

impl Default for BumpSizeConfig {
    fn default() -> Self {
        Self {
            rates_bp: 1.0,
            credit_bp: 1.0,
            equity_pct: 1.0,
            fx_pct: 1.0,
            vol_points: 1.0,
            overrides: BTreeMap::new(),
        }
    }
}

impl BumpSizeConfig {
    /// Return the configured bump size for `factor_id`, checking overrides first.
    ///
    /// The returned `f64` is in the *canonical* units for the factor type:
    /// basis points for rates/credit/inflation, percent for equity/commodity/FX,
    /// vol points for volatility. Callers that cannot statically
    /// know the unit should use [`Self::bump_size_with_unit_for_factor`]
    /// instead — same numeric, but the unit flows through as a
    /// [`FactorBumpUnit`] tag.
    #[must_use]
    pub fn bump_size_for_factor(&self, factor_id: &FactorId, factor_type: &FactorType) -> f64 {
        if let Some(&size) = self.overrides.get(factor_id) {
            return size;
        }

        match factor_type {
            FactorType::Rates | FactorType::Inflation | FactorType::Custom(_) => self.rates_bp,
            FactorType::Credit => self.credit_bp,
            FactorType::Equity | FactorType::Commodity => self.equity_pct,
            FactorType::Fx => self.fx_pct,
            FactorType::Volatility => self.vol_points,
        }
    }

    /// Return the configured bump size along with its [`FactorBumpUnit`].
    ///
    /// A bare-`f64` return would obscure that the unit depends on
    /// `factor_type` — a numeric value of `1.0` is 1 bp for a rates
    /// factor but 1 % for an equity factor, and mixing the two up
    /// silently produces a 100× error. This method carries the unit
    /// alongside the magnitude so downstream bump-construction code
    /// can validate or convert explicitly.
    ///
    /// Per-factor `overrides` inherit the factor-type's canonical unit —
    /// if a user wants a non-canonical interpretation (e.g. an absolute
    /// shift on a rates factor), introduce a new factor with a different
    /// type or a `MarketMapping` that encodes the desired `BumpUnits`.
    ///
    /// # Arguments
    ///
    /// * `factor_id` - Stable identifier of the market or statistical factor.
    /// * `factor_type` - Factor classification controlling shock and aggregation semantics.
    #[must_use]
    pub fn bump_size_with_unit_for_factor(
        &self,
        factor_id: &FactorId,
        factor_type: &FactorType,
    ) -> (f64, FactorBumpUnit) {
        let size = self.bump_size_for_factor(factor_id, factor_type);
        (size, FactorBumpUnit::canonical_for(factor_type))
    }
}

/// Unit semantics for a factor bump magnitude, carried alongside the
/// numeric value returned by
/// [`BumpSizeConfig::bump_size_with_unit_for_factor`].
///
/// `BumpSizeConfig` itself encodes units only implicitly in the field
/// name (`rates_bp`, `equity_pct`, `vol_points`), which previously let
/// a caller thread a rates-bp magnitude into the `EquitySpot` path
/// (which assumes percent) and silently produce a 100× scaling error.
/// `FactorBumpUnit` makes the interpretation explicit and lets
/// downstream code validate against or convert to the mapping's
/// expected unit.
///
/// The variants intentionally mirror [`finstack_quant_core::market_data::bumps::BumpUnits`]
/// plus `VolPoint` and `Absolute`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FactorBumpUnit {
    /// Absolute dimensionless shift applied as-is (`0.01` means +0.01 on the
    /// quoted quantity). Not the canonical unit for any factor type; kept
    /// for callers that pre-convert magnitudes themselves.
    Absolute,
    /// Basis-point shift; `1.0` means 1 bp = 0.0001 fractional.
    BasisPoint,
    /// Percent shift; `1.0` means 1 % = 0.01 fractional.
    Percent,
    /// Vol-point shift; `1.0` means one vol point = 0.01 absolute vol.
    ///
    /// This matches [`BumpSizeConfig::vol_points`] (default `1.0` = one vol
    /// point). Treating that magnitude as an `Absolute` shift would apply a
    /// 100× oversized vol bump.
    VolPoint,
    /// Direct fractional shift; `0.01` means 1 %.
    Fraction,
    /// Multiplicative factor on the base; `1.10` means +10 %.
    Multiplier,
}

impl FactorBumpUnit {
    /// Canonical unit for a given [`FactorType`].
    ///
    /// * Rates / Credit / Inflation / Custom → `BasisPoint` (matches
    ///   `BumpSizeConfig::rates_bp`, `credit_bp`).
    /// * Equity / Commodity / FX → `Percent` (matches
    ///   `BumpSizeConfig::equity_pct`, `fx_pct`).
    /// * Volatility → `VolPoint` (matches `BumpSizeConfig::vol_points`;
    ///   `1.0` = one vol point = `0.01` absolute vol).
    #[must_use]
    pub fn canonical_for(factor_type: &FactorType) -> Self {
        match factor_type {
            FactorType::Rates
            | FactorType::Credit
            | FactorType::Inflation
            | FactorType::Custom(_) => FactorBumpUnit::BasisPoint,
            FactorType::Equity | FactorType::Commodity | FactorType::Fx => FactorBumpUnit::Percent,
            FactorType::Volatility => FactorBumpUnit::VolPoint,
        }
    }

    /// Convert a magnitude in this unit to a plain fraction (dimensionless
    /// proportion of the base). Useful when a consumer only knows how to
    /// apply fractional shifts, e.g. an equity-spot multiplier of
    /// `1.0 + fraction`.
    ///
    /// `Multiplier` is returned unchanged — the fractional form doesn't
    /// capture a multiplicative shock; callers that want that branch
    /// should match on the variant explicitly.
    #[must_use]
    // Multiplier passthrough is semantically distinct from Absolute/Fraction
    // even though the arm bodies coincide; keep the arms explicit.
    #[allow(clippy::match_same_arms)]
    pub fn to_fraction(self, value: f64) -> f64 {
        match self {
            FactorBumpUnit::Absolute | FactorBumpUnit::Fraction => value,
            FactorBumpUnit::BasisPoint => value * 1e-4,
            // One percent and one vol point both convert at 1e-2: a vol
            // point is 0.01 of absolute vol just as a percent is 0.01 of
            // the base.
            FactorBumpUnit::Percent | FactorBumpUnit::VolPoint => value * 1e-2,
            // Multiplier is not a linear shift; expose as-is for callers
            // that know to build a multiplicative bump spec.
            FactorBumpUnit::Multiplier => value,
        }
    }
}

/// Serializable configuration bundle for constructing a factor-model workflow.
///
/// The `factors` vector defines the canonical factor ordering. The covariance
/// matrix must use the same factor IDs and ordering, and the matching
/// configuration is expected to emit exposures against that same universe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FactorModelConfig {
    /// Factor definitions spanning the model universe.
    pub factors: Vec<FactorDefinition>,
    /// Covariance matrix aligned to `factors`.
    pub covariance: FactorCovarianceMatrix,
    /// Declarative dependency-to-factor matching configuration.
    pub matching: MatchingConfig,
    /// Sensitivity extraction strategy used by the analysis pipeline.
    pub pricing_mode: PricingMode,
    /// Risk measure used when aggregating factor sensitivities.
    #[serde(default)]
    pub risk_measure: RiskMeasure,
    /// Optional finite-difference bump overrides for sensitivity engines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bump_size: Option<BumpSizeConfig>,
    /// Policy used when a dependency does not map to a configured factor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unmatched_policy: Option<UnmatchedPolicy>,
}

impl FactorModelConfig {
    /// Validate factor ordering, matching rules, and the selected risk measure.
    ///
    /// # Errors
    ///
    /// Returns a validation error when covariance axes do not exactly match the
    /// ordered factor definitions, matching rules emit undeclared factor IDs,
    /// issuer rows are duplicated, or a confidence-bearing risk measure is
    /// outside its accepted range.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let factor_ids: Vec<&FactorId> = self.factors.iter().map(|factor| &factor.id).collect();
        let covariance_ids: Vec<&FactorId> = self.covariance.factor_ids().iter().collect();
        if factor_ids != covariance_ids {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FactorModelConfig: covariance factor ids must exactly match factors in order; \
                 factors={factor_ids:?}, covariance={covariance_ids:?}"
            )));
        }
        self.risk_measure.validate()?;
        self.validate_matching_factor_ids()
    }

    /// Validates that every factor identifier the matcher can emit is also
    /// present in `factors`.
    ///
    /// This is the static counterpart to runtime "missing factor" failures:
    /// catching the misalignment at config-load time avoids surprises during
    /// portfolio analysis. Cascades and credit hierarchies are walked
    /// recursively so every reachable factor ID is checked.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when any factor ID emitted by the
    /// matching config is not declared in [`Self::factors`].
    ///
    /// # Limitations
    ///
    /// This validation only enumerates factor IDs for issuers known to the calibrated
    /// `issuer_betas`. If a runtime issuer with full tags is treated as `BucketOnly`,
    /// its bucket factor IDs are not checked here.
    pub fn validate_matching_factor_ids(&self) -> finstack_quant_core::Result<()> {
        use std::collections::BTreeSet;
        let known: BTreeSet<&FactorId> = self.factors.iter().map(|f| &f.id).collect();
        for fid in self.matching.enumerate_factor_ids() {
            if !known.contains(&fid) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FactorModelConfig: matcher references factor_id {fid:?} not present in factors"
                )));
            }
        }
        check_no_duplicate_issuer_rows(&self.matching)?;
        Ok(())
    }
}

/// Reject matching configs with more than one beta row for the same issuer —
/// within a single credit-hierarchical config **or across cascade members**.
///
/// Row lookup (binary search) resolves within-config duplicates arbitrarily,
/// and the idiosyncratic-variance collector lets a later cascade member's row
/// silently overwrite an earlier one's adder variance, so either form of
/// duplication can source betas and idiosyncratic variance from two
/// different rows for the same issuer.
fn check_no_duplicate_issuer_rows(matching: &MatchingConfig) -> finstack_quant_core::Result<()> {
    use std::collections::BTreeSet;
    fn walk<'a>(
        matching: &'a MatchingConfig,
        seen: &mut BTreeSet<&'a str>,
    ) -> finstack_quant_core::Result<()> {
        match matching {
            MatchingConfig::Cascade(configs) => {
                configs.iter().try_for_each(|config| walk(config, seen))
            }
            MatchingConfig::CreditHierarchical(config) => {
                for row in &config.issuer_betas {
                    if !seen.insert(row.issuer_id.as_str()) {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "FactorModelConfig: duplicate issuer_id {:?} in credit \
                             hierarchical matching config (within one config or \
                             across cascade members)",
                            row.issuer_id.as_str()
                        )));
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    let mut seen = BTreeSet::new();
    walk(matching, &mut seen)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::factor::{
        FactorCovarianceMatrix, FactorDefinition, FactorType, MarketMapping, MatchingConfig,
        UnmatchedPolicy,
    };
    use finstack_quant_core::market_data::bumps::BumpUnits;
    use finstack_quant_core::types::CurveId;

    #[test]
    fn test_unmatched_policy_default() {
        assert_eq!(UnmatchedPolicy::default(), UnmatchedPolicy::Residual);
    }

    #[test]
    fn test_unmatched_policy_serde() {
        let policy = UnmatchedPolicy::Strict;
        let json_result = serde_json::to_string(&policy);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let back_result: Result<UnmatchedPolicy, _> = serde_json::from_str(&json);
        assert!(back_result.is_ok());
        let Ok(back) = back_result else {
            return;
        };
        assert_eq!(policy, back);
    }

    #[test]
    fn unmatched_policy_serializes_snake_case_and_rejects_pascal_case() {
        let json = serde_json::to_string(&UnmatchedPolicy::Strict).unwrap_or_default();
        assert_eq!(json, "\"strict\"");
        assert!(serde_json::from_str::<UnmatchedPolicy>("\"Residual\"").is_err());
    }

    #[test]
    fn test_risk_measure_serde_roundtrip_for_all_variants() {
        let cases = [
            (RiskMeasure::Variance, "\"variance\""),
            (RiskMeasure::Volatility, "\"volatility\""),
            (
                RiskMeasure::VaR { confidence: 0.99 },
                r#"{"var":{"confidence":0.99}}"#,
            ),
            (
                RiskMeasure::ExpectedShortfall { confidence: 0.975 },
                r#"{"expected_shortfall":{"confidence":0.975}}"#,
            ),
        ];

        for (measure, expected_json) in cases {
            let json_result = serde_json::to_string(&measure);
            assert!(json_result.is_ok());
            let Ok(json) = json_result else {
                return;
            };

            assert_eq!(json, expected_json);

            let back_result: Result<RiskMeasure, _> = serde_json::from_str(&json);
            assert!(back_result.is_ok());
            let Ok(back) = back_result else {
                return;
            };

            assert_eq!(measure, back);
        }
    }

    #[test]
    fn test_risk_measure_default_is_variance() {
        assert_eq!(RiskMeasure::default(), RiskMeasure::Variance);
    }

    #[test]
    fn test_risk_measure_validate_rejects_invalid_confidence() {
        let invalid_measures = [
            RiskMeasure::VaR { confidence: 0.1 },
            RiskMeasure::VaR { confidence: 0.5 },
            RiskMeasure::VaR { confidence: 0.0 },
            RiskMeasure::VaR { confidence: 1.0 },
            RiskMeasure::ExpectedShortfall { confidence: 0.25 },
            RiskMeasure::ExpectedShortfall { confidence: 0.5 },
            RiskMeasure::ExpectedShortfall { confidence: -0.1 },
            RiskMeasure::ExpectedShortfall { confidence: 1.1 },
        ];

        for measure in invalid_measures {
            assert!(measure.validate().is_err());
        }
    }

    #[test]
    fn test_risk_measure_serde_rejects_invalid_confidence() {
        let invalid_payloads = [
            r#"{"var":{"confidence":0.1}}"#,
            r#"{"var":{"confidence":0.5}}"#,
            r#"{"var":{"confidence":0.0}}"#,
            r#"{"var":{"confidence":1.0}}"#,
            r#"{"expected_shortfall":{"confidence":0.25}}"#,
            r#"{"expected_shortfall":{"confidence":0.5}}"#,
            r#"{"expected_shortfall":{"confidence":-0.1}}"#,
            r#"{"expected_shortfall":{"confidence":1.1}}"#,
        ];

        for payload in invalid_payloads {
            let result: Result<RiskMeasure, _> = serde_json::from_str(payload);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_pricing_mode_serde() {
        let mode = PricingMode::DeltaBased;
        let json_result = serde_json::to_string(&mode);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let back_result: Result<PricingMode, _> = serde_json::from_str(&json);
        assert!(back_result.is_ok());
        let Ok(back) = back_result else {
            return;
        };

        assert_eq!(mode, back);
    }

    #[test]
    fn test_bump_size_config_defaults() {
        let config = BumpSizeConfig::default();
        assert!((config.rates_bp - 1.0).abs() < 1e-12);
        assert!((config.credit_bp - 1.0).abs() < 1e-12);
        assert!((config.equity_pct - 1.0).abs() < 1e-12);
        assert!((config.fx_pct - 1.0).abs() < 1e-12);
        assert!((config.vol_points - 1.0).abs() < 1e-12);
        assert!(config.overrides.is_empty());
    }

    #[test]
    fn test_bump_size_for_factor_override() {
        let mut config = BumpSizeConfig::default();
        config.overrides.insert(FactorId::new("USD-Rates"), 0.5);

        let overridden =
            config.bump_size_for_factor(&FactorId::new("USD-Rates"), &FactorType::Rates);
        assert!((overridden - 0.5).abs() < 1e-12);

        let fallback = config.bump_size_for_factor(&FactorId::new("EUR-Rates"), &FactorType::Rates);
        assert!((fallback - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_bump_size_config_serde() {
        let config = BumpSizeConfig::default();
        let json_result = serde_json::to_string(&config);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let back_result: Result<BumpSizeConfig, _> = serde_json::from_str(&json);
        assert!(back_result.is_ok());
        let Ok(back) = back_result else {
            return;
        };

        assert!((config.rates_bp - back.rates_bp).abs() < 1e-12);
        assert!((config.credit_bp - back.credit_bp).abs() < 1e-12);
        assert!((config.equity_pct - back.equity_pct).abs() < 1e-12);
        assert!((config.fx_pct - back.fx_pct).abs() < 1e-12);
        assert!((config.vol_points - back.vol_points).abs() < 1e-12);
        assert_eq!(config.overrides, back.overrides);
    }

    #[test]
    fn test_factor_model_config_serde_roundtrip() {
        let config = FactorModelConfig {
            factors: vec![FactorDefinition {
                id: FactorId::new("Rates"),
                factor_type: FactorType::Rates,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![CurveId::new("USD-OIS")],
                    units: BumpUnits::RateBp,
                },
                description: None,
            }],
            covariance: {
                let covariance_result =
                    FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04]);
                assert!(covariance_result.is_ok());
                let Ok(covariance) = covariance_result else {
                    return;
                };
                covariance
            },
            matching: MatchingConfig::MappingTable(vec![]),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: RiskMeasure::Variance,
            bump_size: None,
            unmatched_policy: Some(UnmatchedPolicy::Residual),
        };

        let json_result = serde_json::to_string_pretty(&config);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };
        let back_result: Result<FactorModelConfig, _> = serde_json::from_str(&json);
        assert!(back_result.is_ok());
        let Ok(back) = back_result else {
            return;
        };

        assert_eq!(back.factors.len(), 1);
        assert_eq!(back.pricing_mode, PricingMode::DeltaBased);
        assert_eq!(back.risk_measure, RiskMeasure::Variance);
        assert_eq!(back.unmatched_policy, Some(UnmatchedPolicy::Residual));
    }

    #[test]
    fn test_factor_model_config_deserialize_uses_defaults_for_omitted_optionals() {
        let original = FactorModelConfig {
            factors: vec![FactorDefinition {
                id: FactorId::new("Rates"),
                factor_type: FactorType::Rates,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![CurveId::new("USD-OIS")],
                    units: BumpUnits::RateBp,
                },
                description: None,
            }],
            covariance: {
                let covariance_result =
                    FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04]);
                assert!(covariance_result.is_ok());
                let Ok(covariance) = covariance_result else {
                    return;
                };
                covariance
            },
            matching: MatchingConfig::MappingTable(vec![]),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: RiskMeasure::Variance,
            bump_size: None,
            unmatched_policy: None,
        };

        let value_result = serde_json::to_value(original);
        assert!(value_result.is_ok());
        let Ok(mut value) = value_result else {
            return;
        };
        assert!(value.is_object());
        let Some(object) = value.as_object_mut() else {
            return;
        };
        object.remove("risk_measure");
        object.remove("bump_size");
        object.remove("unmatched_policy");

        let config_result: Result<FactorModelConfig, _> = serde_json::from_value(value);
        assert!(config_result.is_ok());
        let Ok(config) = config_result else {
            return;
        };

        assert_eq!(config.risk_measure, RiskMeasure::Variance);
        assert_eq!(config.bump_size, None);
        assert_eq!(config.unmatched_policy, None);
    }

    // a vol bump of 1.0 vol point must convert to 0.01
    // absolute vol, not 1.0 (a 100x oversized bump).
    #[test]
    fn vol_point_unit_converts_one_point_to_one_percent() {
        let (size, unit) = BumpSizeConfig::default()
            .bump_size_with_unit_for_factor(&FactorId::new("VOL-1"), &FactorType::Volatility);
        assert!((size - 1.0).abs() < 1e-12);
        assert_eq!(unit, FactorBumpUnit::VolPoint);
        assert!(
            (unit.to_fraction(size) - 0.01).abs() < 1e-15,
            "one vol point must be 0.01 absolute vol"
        );
    }

    #[test]
    fn bump_size_config_rejects_unknown_fields() {
        let json = r#"{"credit_bps": 5.0}"#; // schema-rejection-test
        let result: Result<BumpSizeConfig, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn credit_hierarchical_config_rejects_unknown_factor_id() {
        use crate::factor::credit::hierarchy::{
            AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
            IssuerBetas, IssuerTags,
        };
        use crate::factor::matching::{CreditHierarchicalConfig, DependencyFilter};
        use finstack_quant_core::types::IssuerId;
        use std::collections::BTreeMap;

        let mut tags = BTreeMap::new();
        tags.insert("rating".to_owned(), "IG".to_owned());
        let row = IssuerBetaRow {
            issuer_id: IssuerId::new("ISSUER-A"),
            tags: IssuerTags(tags),
            mode: IssuerBetaMode::IssuerBeta,
            betas: IssuerBetas {
                pc: 0.9,
                levels: vec![0.85],
            },
            adder_at_anchor: 0.0,
            adder_vol_annualized: 0.01,
            adder_vol_source: AdderVolSource::Default,
            fit_quality: None,
            level_fit_quality: vec![],
            spread_duration: 1.0,
        };
        let credit_config = CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating],
            },
            issuer_betas: vec![row],
            require_issuer_id: false,
        };

        // Build a FactorModelConfig where `factors` only knows about
        // `credit::generic` but NOT the `credit::level0::Rating::IG` bucket
        // factor that the matcher will try to emit.
        let factor_id = FactorId::new("credit::generic");
        let factors = vec![FactorDefinition {
            id: factor_id.clone(),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("CDX.IG")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }];
        let covariance = FactorCovarianceMatrix::new(vec![factor_id], vec![0.04]).unwrap();

        let config = FactorModelConfig {
            factors,
            covariance,
            matching: MatchingConfig::CreditHierarchical(credit_config),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: RiskMeasure::Variance,
            bump_size: None,
            unmatched_policy: None,
        };

        let result = config.validate_matching_factor_ids();
        assert!(
            result.is_err(),
            "validation must reject matcher referencing unknown factor IDs"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("credit::level0::Rating::IG"),
            "error must name the missing factor: {msg}"
        );
    }

    #[test]
    fn validate_matching_factor_ids_accepts_aligned_config() {
        // Single MappingTable rule referencing a factor that does exist.
        let factor_id = FactorId::new("Rates");
        let factors = vec![FactorDefinition {
            id: factor_id.clone(),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }];
        let covariance = FactorCovarianceMatrix::new(vec![factor_id.clone()], vec![0.04]).unwrap();
        let config = FactorModelConfig {
            factors,
            covariance,
            matching: MatchingConfig::MappingTable(vec![crate::factor::matching::MappingRule {
                dependency_filter: crate::factor::matching::DependencyFilter::default(),
                attribute_filter: crate::factor::matching::AttributeFilter::default(),
                factor_id,
            }]),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: RiskMeasure::Variance,
            bump_size: None,
            unmatched_policy: None,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_rejects_covariance_axis_order_mismatch() {
        let factor_id = FactorId::new("Rates");
        let config = FactorModelConfig {
            factors: vec![FactorDefinition {
                id: factor_id,
                factor_type: FactorType::Rates,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![CurveId::new("USD-OIS")],
                    units: BumpUnits::RateBp,
                },
                description: None,
            }],
            covariance: FactorCovarianceMatrix::new(vec![FactorId::new("Other")], vec![0.04])
                .unwrap(),
            matching: MatchingConfig::MappingTable(Vec::new()),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: RiskMeasure::Variance,
            bump_size: None,
            unmatched_policy: None,
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_matching_factor_ids_rejects_duplicate_issuer_rows() {
        use crate::factor::credit::hierarchy::{
            AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
            IssuerBetas, IssuerTags,
        };
        use crate::factor::matching::CreditHierarchicalConfig;
        use finstack_quant_core::types::IssuerId;
        use std::collections::BTreeMap;

        let row = |adder_vol: f64| IssuerBetaRow {
            issuer_id: IssuerId::new("ACME"),
            tags: IssuerTags(BTreeMap::from([("rating".to_string(), "IG".to_string())])),
            mode: IssuerBetaMode::BucketOnly,
            betas: IssuerBetas {
                pc: 1.0,
                levels: vec![1.0],
            },
            adder_at_anchor: 0.0,
            adder_vol_annualized: adder_vol,
            adder_vol_source: AdderVolSource::Default,
            fit_quality: None,
            level_fit_quality: vec![],
            spread_duration: 1.0,
        };
        let generic = FactorId::new("credit::generic");
        let bucket = FactorId::new("credit::level0::Rating::IG");
        let factors: Vec<FactorDefinition> = [&generic, &bucket]
            .into_iter()
            .map(|id| FactorDefinition {
                id: id.clone(),
                factor_type: FactorType::Credit,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![],
                    units: BumpUnits::RateBp,
                },
                description: None,
            })
            .collect();
        let covariance =
            FactorCovarianceMatrix::new(vec![generic, bucket], vec![1.0, 0.0, 0.0, 1.0]).unwrap();
        // Two rows for the same issuer: lookup and idiosyncratic-variance
        // consumers could silently pick different rows.
        let config = FactorModelConfig {
            factors,
            covariance,
            matching: MatchingConfig::CreditHierarchical(CreditHierarchicalConfig {
                dependency_filter: Default::default(),
                hierarchy: CreditHierarchySpec {
                    levels: vec![HierarchyDimension::Rating],
                },
                issuer_betas: vec![row(20.0), row(200.0)],
                require_issuer_id: false,
            }),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: RiskMeasure::Variance,
            bump_size: None,
            unmatched_policy: None,
        };
        let err = config
            .validate_matching_factor_ids()
            .expect_err("duplicate issuer rows in the matching config must be rejected");
        assert!(
            err.to_string().contains("ACME"),
            "error must name the duplicated issuer: {err}"
        );
    }
    #[test]
    fn validate_matching_factor_ids_rejects_duplicates_across_cascade_members() {
        use crate::factor::credit::hierarchy::{
            AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
            IssuerBetas, IssuerTags,
        };
        use crate::factor::matching::CreditHierarchicalConfig;
        use finstack_quant_core::types::IssuerId;
        use std::collections::BTreeMap;

        let row = || IssuerBetaRow {
            issuer_id: IssuerId::new("ACME"),
            tags: IssuerTags(BTreeMap::from([("rating".to_string(), "IG".to_string())])),
            mode: IssuerBetaMode::BucketOnly,
            betas: IssuerBetas {
                pc: 1.0,
                levels: vec![1.0],
            },
            adder_at_anchor: 0.0,
            adder_vol_annualized: 10.0,
            adder_vol_source: AdderVolSource::Default,
            fit_quality: None,
            level_fit_quality: vec![],
            spread_duration: 1.0,
        };
        let member = |r: IssuerBetaRow| {
            MatchingConfig::CreditHierarchical(CreditHierarchicalConfig {
                dependency_filter: Default::default(),
                hierarchy: CreditHierarchySpec {
                    levels: vec![HierarchyDimension::Rating],
                },
                issuer_betas: vec![r],
                require_issuer_id: false,
            })
        };
        let generic = FactorId::new("credit::generic");
        let bucket = FactorId::new("credit::level0::Rating::IG");
        let factors: Vec<FactorDefinition> = [&generic, &bucket]
            .into_iter()
            .map(|id| FactorDefinition {
                id: id.clone(),
                factor_type: FactorType::Credit,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![],
                    units: BumpUnits::RateBp,
                },
                description: None,
            })
            .collect();
        // Same issuer appears in two different cascade members: the
        // idiosyncratic-variance collector would let the later member
        // silently overwrite the earlier one's adder variance.
        let config = FactorModelConfig {
            factors,
            covariance: FactorCovarianceMatrix::new(
                vec![generic, bucket],
                vec![1.0, 0.0, 0.0, 1.0],
            )
            .unwrap(),
            matching: MatchingConfig::Cascade(vec![member(row()), member(row())]),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: RiskMeasure::Variance,
            bump_size: None,
            unmatched_policy: None,
        };
        let err = config
            .validate_matching_factor_ids()
            .expect_err("same issuer in two cascade members must be rejected");
        assert!(err.to_string().contains("ACME"), "must name issuer: {err}");
    }
}

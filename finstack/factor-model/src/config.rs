use super::covariance::FactorCovarianceMatrix;
use super::matching::MatchingConfig;
use super::primitives::definition::FactorDefinition;
use super::primitives::factor_types::{FactorId, FactorType};
use super::UnmatchedPolicy;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

/// Strategy used when extracting factor sensitivities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

impl crate::parse::NormalizedEnum for PricingMode {
    const VARIANTS: &'static [(&'static str, Self)] = &[
        ("delta_based", Self::DeltaBased),
        ("deltabased", Self::DeltaBased),
        ("full_repricing", Self::FullRepricing),
        ("fullrepricing", Self::FullRepricing),
    ];
}

impl FromStr for PricingMode {
    type Err = finstack_core::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parse::parse_normalized_enum(s)
            .map_err(|_| finstack_core::InputError::Invalid.into())
    }
}

/// Risk measure used when aggregating factor exposures.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
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
        confidence: f64,
    },
    /// Aggregate exposures using expected shortfall at a fixed one-sided loss confidence level.
    ///
    /// As with [`Self::VaR`], this is intended for parametric factor-model
    /// aggregation rather than full-path simulation, and ES is reported as a
    /// **negative** number using the P&L sign convention.
    ExpectedShortfall {
        /// Confidence level in the open interval `(0.5, 1)`.
        confidence: f64,
    },
}

impl RiskMeasure {
    /// Validate any embedded confidence levels before downstream risk calculations use them.
    pub fn validate(&self) -> finstack_core::Result<()> {
        match self {
            Self::Variance | Self::Volatility => Ok(()),
            Self::VaR { confidence } | Self::ExpectedShortfall { confidence } => {
                validate_confidence(*confidence)
            }
        }
    }
}

fn validate_confidence(confidence: f64) -> finstack_core::Result<()> {
    if confidence.is_finite() && confidence > 0.5 && confidence < 1.0 {
        Ok(())
    } else {
        Err(finstack_core::Error::Validation(format!(
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Default volatility bump in absolute vol points.
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
    /// absolute vol points for volatility. Callers that cannot statically
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
            FactorType::FX => self.fx_pct,
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
/// The variants intentionally mirror [`finstack_core::market_data::bumps::BumpUnits`]
/// plus `Absolute` (used by vol-point shifts where the magnitude is
/// already a dimensionless number of vol points, not a fraction or
/// percent of something).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FactorBumpUnit {
    /// Absolute dimensionless shift — e.g. vol points (`0.01` = one vol point).
    Absolute,
    /// Basis-point shift; `1.0` means 1 bp = 0.0001 fractional.
    BasisPoint,
    /// Percent shift; `1.0` means 1 % = 0.01 fractional.
    Percent,
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
    /// * Volatility → `Absolute` (vol points).
    #[must_use]
    pub fn canonical_for(factor_type: &FactorType) -> Self {
        match factor_type {
            FactorType::Rates
            | FactorType::Credit
            | FactorType::Inflation
            | FactorType::Custom(_) => FactorBumpUnit::BasisPoint,
            FactorType::Equity | FactorType::Commodity | FactorType::FX => FactorBumpUnit::Percent,
            FactorType::Volatility => FactorBumpUnit::Absolute,
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
    pub fn to_fraction(self, value: f64) -> f64 {
        match self {
            FactorBumpUnit::Absolute | FactorBumpUnit::Fraction => value,
            FactorBumpUnit::BasisPoint => value * 1e-4,
            FactorBumpUnit::Percent => value * 1e-2,
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
    /// Returns [`finstack_core::Error::Validation`] when any factor ID emitted by the
    /// matching config is not declared in [`Self::factors`].
    ///
    /// # Limitations
    ///
    /// This validation only enumerates factor IDs for issuers known to the calibrated
    /// `issuer_betas`. If a runtime issuer with full tags is treated as `BucketOnly`,
    /// its bucket factor IDs are not checked here.
    pub fn validate_matching_factor_ids(&self) -> finstack_core::Result<()> {
        use std::collections::BTreeSet;
        let known: BTreeSet<&FactorId> = self.factors.iter().map(|f| &f.id).collect();
        for fid in self.matching.enumerate_factor_ids() {
            if !known.contains(&fid) {
                return Err(finstack_core::Error::Validation(format!(
                    "FactorModelConfig: matcher references factor_id {fid:?} not present in factors"
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_parses_to(label: &str, expected: PricingMode) {
        assert!(matches!(label.parse::<PricingMode>(), Ok(value) if value == expected));
    }
    use crate::{
        FactorCovarianceMatrix, FactorDefinition, FactorType, MarketMapping, MatchingConfig,
        UnmatchedPolicy,
    };
    use finstack_core::market_data::bumps::BumpUnits;
    use finstack_core::types::CurveId;

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
    fn test_bump_size_config_remains_equality_comparable() {
        assert_eq!(BumpSizeConfig::default(), BumpSizeConfig::default());
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

    #[test]
    fn test_pricing_mode_fromstr_display_roundtrip() {
        for (input, expected) in [
            ("delta_based", PricingMode::DeltaBased),
            ("deltabased", PricingMode::DeltaBased),
            ("full_repricing", PricingMode::FullRepricing),
            ("fullrepricing", PricingMode::FullRepricing),
        ] {
            assert_parses_to(input, expected);
        }

        for variant in [PricingMode::DeltaBased, PricingMode::FullRepricing] {
            let display = variant.to_string();
            assert!(matches!(display.parse::<PricingMode>(), Ok(value) if value == variant));
        }
    }

    #[test]
    fn test_pricing_mode_fromstr_rejects_unknown() {
        assert!("unknown".parse::<PricingMode>().is_err());
    }

    // ------------------------------------------------------------------
    // PR-2 test: matcher-emitted factor IDs must exist in `factors`
    // ------------------------------------------------------------------
    #[test]
    fn credit_hierarchical_config_rejects_unknown_factor_id() {
        use crate::credit::hierarchy::{
            AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
            IssuerBetas, IssuerTags,
        };
        use crate::matching::{CreditHierarchicalConfig, DependencyFilter};
        use finstack_core::types::IssuerId;
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
        };
        let credit_config = CreditHierarchicalConfig {
            dependency_filter: DependencyFilter::default(),
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating],
            },
            issuer_betas: vec![row],
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
            matching: MatchingConfig::MappingTable(vec![crate::matching::MappingRule {
                dependency_filter: crate::matching::DependencyFilter::default(),
                attribute_filter: crate::matching::AttributeFilter::default(),
                factor_id,
            }]),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: RiskMeasure::Variance,
            bump_size: None,
            unmatched_policy: None,
        };
        assert!(config.validate_matching_factor_ids().is_ok());
    }
}

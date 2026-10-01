//! Eligible collateral specifications and haircuts.
//!
//! Defines collateral eligibility criteria and haircut schedules following
//! BCBS-IOSCO standards for non-centrally cleared derivatives and GMRA
//! conventions for repos.

use std::fmt;

use crate::registry::margin_registry_from_config;
use crate::registry::{embedded_registry, embedded_registry_or_panic, AssetClassDefault};
use crate::types::serde_validation;
use finstack_quant_core::config::FinstackConfig;
use serde::{Deserialize, Serialize};

/// Collateral asset classes per BCBS-IOSCO standards.
///
/// Asset classes determine baseline haircuts and eligibility criteria.
/// The BCBS-IOSCO framework specifies minimum haircuts by asset class.
///
/// # Reference
///
/// BCBS-IOSCO "Margin requirements for non-centrally cleared derivatives" (2020)
/// Annex A: Standardized haircut schedule `docs/REFERENCES.md#bcbs-iosco-uncleared-margin`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CollateralAssetClass {
    /// Cash in eligible currency
    #[default]
    Cash,
    /// Sovereign government bonds accepted as collateral.
    GovernmentBonds,
    /// Debt issued or guaranteed by eligible agencies.
    AgencyBonds,
    /// Covered bonds meeting the applicable eligibility criteria.
    CoveredBonds,
    /// Investment-grade corporate bonds accepted under the CSA.
    CorporateBonds,
    /// Eligible listed equity collateral.
    Equity,
    /// Eligible gold collateral.
    Gold,
    /// Eligible mutual funds or exchange-traded funds.
    MutualFunds,
}

impl CollateralAssetClass {
    fn defaults_or_panic(&self) -> AssetClassDefault {
        embedded_registry_or_panic().collateral_asset_class_defaults[self].clone()
    }

    fn default_entry(&self, field_name: &str) -> finstack_quant_core::Result<AssetClassDefault> {
        let registry = embedded_registry()
            .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
        registry
            .collateral_asset_class_defaults
            .get(self)
            .cloned()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "No {field_name} configured for collateral asset class '{}'",
                    self
                ))
            })
    }

    /// Normalized string identifier for this asset class.
    pub fn as_str(&self) -> &str {
        match self {
            CollateralAssetClass::Cash => "cash",
            CollateralAssetClass::GovernmentBonds => "government_bonds",
            CollateralAssetClass::AgencyBonds => "agency_bonds",
            CollateralAssetClass::CoveredBonds => "covered_bonds",
            CollateralAssetClass::CorporateBonds => "corporate_bonds",
            CollateralAssetClass::Equity => "equity",
            CollateralAssetClass::Gold => "gold",
            CollateralAssetClass::MutualFunds => "mutual_funds",
        }
    }
}

impl fmt::Display for CollateralAssetClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for CollateralAssetClass {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cash" => Ok(CollateralAssetClass::Cash),
            "government_bonds" => Ok(CollateralAssetClass::GovernmentBonds),
            "agency_bonds" => Ok(CollateralAssetClass::AgencyBonds),
            "covered_bonds" => Ok(CollateralAssetClass::CoveredBonds),
            "corporate_bonds" => Ok(CollateralAssetClass::CorporateBonds),
            "equity" => Ok(CollateralAssetClass::Equity),
            "gold" => Ok(CollateralAssetClass::Gold),
            "mutual_funds" => Ok(CollateralAssetClass::MutualFunds),
            other => Err(format!("Unknown collateral asset class: {other}")),
        }
    }
}

impl CollateralAssetClass {
    /// Get the BCBS-IOSCO standard haircut for this asset class.
    ///
    /// These are baseline haircuts; actual haircuts may vary based on
    /// rating, maturity, and bilateral agreement.
    ///
    /// # Returns
    ///
    /// Haircut as a decimal (e.g., 0.02 = 2%)
    ///
    /// The returned value is the embedded baseline for the asset class, before
    /// maturity, rating, concentration, wrong-way-risk, or bilateral CSA
    /// adjustments.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot load a default
    /// entry for this asset class.
    pub fn standard_haircut(&self) -> finstack_quant_core::Result<f64> {
        Ok(self.default_entry("standard haircut")?.standard_haircut)
    }

    /// Get the FX haircut add-on for currency mismatch.
    ///
    /// Per BCBS-IOSCO, an 8% add-on applies when collateral currency
    /// differs from the settlement currency of the derivative.
    ///
    /// The result is a decimal multiplier applied only after the caller has
    /// determined that the currencies differ; this method does not compare
    /// currencies itself.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot load a default
    /// entry for this asset class.
    pub fn fx_addon(&self) -> finstack_quant_core::Result<f64> {
        Ok(self.default_entry("FX addon")?.fx_addon)
    }
}

/// Maturity constraints for eligible collateral.
///
/// Some CSAs restrict collateral based on remaining maturity to limit
/// duration risk in the collateral portfolio.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MaturityConstraints {
    /// Minimum remaining years to maturity (if any)
    #[serde(
        default,
        deserialize_with = "serde_validation::min_remaining_years::deserialize",
        serialize_with = "serde_validation::min_remaining_years::serialize"
    )]
    #[cfg_attr(feature = "json-schema", schemars(range(min = 0.0)))]
    pub min_remaining_years: Option<f64>,
    /// Maximum remaining years to maturity (if any)
    #[serde(
        default,
        deserialize_with = "serde_validation::max_remaining_years::deserialize",
        serialize_with = "serde_validation::max_remaining_years::serialize"
    )]
    #[cfg_attr(feature = "json-schema", schemars(range(min = 0.0)))]
    pub max_remaining_years: Option<f64>,
}

impl MaturityConstraints {
    /// Create constraints with maximum maturity only.
    #[must_use]
    pub fn max_maturity(years: f64) -> Self {
        Self {
            min_remaining_years: None,
            max_remaining_years: Some(years),
        }
    }

    /// Check if a given remaining maturity satisfies the constraints.
    ///
    /// Boundary comparisons use a small tolerance (~1.4 minutes of a
    /// year) to avoid IEEE 754 rounding error rejecting an instrument
    /// that exactly matches the registry boundary. The tolerance is
    /// tight enough that daily-bucketed schedules (the typical case)
    /// never see boundary leak: 2.7e-6 years × 365 days/year ≈ 0.001
    /// days = ~1.4 minutes.
    #[must_use]
    pub fn is_satisfied(&self, remaining_years: f64) -> bool {
        // 2.7e-6 years ≈ 1.4 minutes — well below any operational
        // boundary used in eligibility schedules, comfortably above
        // f64 round-trip error from `time::Duration` ↔ year fractions.
        const TOLERANCE_YEARS: f64 = 2.7e-6;
        if let Some(min) = self.min_remaining_years {
            if remaining_years < min - TOLERANCE_YEARS {
                return false;
            }
        }
        if let Some(max) = self.max_remaining_years {
            if remaining_years > max + TOLERANCE_YEARS {
                return false;
            }
        }
        true
    }
}

/// Single collateral eligibility entry.
///
/// Defines eligibility criteria and haircut for a specific type of collateral.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CollateralEligibility {
    /// Asset class
    pub asset_class: CollateralAssetClass,

    /// Minimum credit rating requirement (e.g., "A-", "BBB")
    ///
    /// If None, no rating constraint applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_rating: Option<String>,

    /// Remaining maturity constraints
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maturity_constraints: Option<MaturityConstraints>,

    /// Haircut as decimal (e.g., 0.02 = 2%)
    #[serde(
        deserialize_with = "serde_validation::haircut::deserialize",
        serialize_with = "serde_validation::haircut::serialize"
    )]
    #[cfg_attr(feature = "json-schema", schemars(range(min = 0.0, max = 1.0)))]
    pub haircut: f64,

    /// Additional FX haircut for currency mismatch (decimal)
    ///
    /// Applied when collateral currency differs from settlement currency.
    #[serde(
        default,
        deserialize_with = "serde_validation::fx_haircut_addon::deserialize",
        serialize_with = "serde_validation::fx_haircut_addon::serialize"
    )]
    #[cfg_attr(feature = "json-schema", schemars(range(min = 0.0, max = 1.0)))]
    pub fx_haircut_addon: f64,

    /// Concentration limit as fraction of total collateral (optional)
    ///
    /// E.g., 0.30 means max 30% of collateral can be this type.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "serde_validation::concentration_limit::deserialize",
        serialize_with = "serde_validation::concentration_limit::serialize"
    )]
    #[cfg_attr(feature = "json-schema", schemars(range(min = 0.0, max = 1.0)))]
    pub concentration_limit: Option<f64>,
}

impl CollateralEligibility {
    /// Create a cash eligibility entry.
    #[must_use]
    pub fn cash() -> Self {
        let defaults = CollateralAssetClass::Cash.defaults_or_panic();
        Self {
            asset_class: CollateralAssetClass::Cash,
            min_rating: None,
            maturity_constraints: None,
            haircut: defaults.standard_haircut,
            fx_haircut_addon: defaults.fx_addon,
            concentration_limit: None,
        }
    }

    /// Create a government bonds eligibility entry with standard BCBS haircuts.
    #[must_use]
    pub fn government_bonds(haircut: f64) -> Self {
        let defaults = CollateralAssetClass::GovernmentBonds.defaults_or_panic();
        Self {
            asset_class: CollateralAssetClass::GovernmentBonds,
            min_rating: Some("A-".to_string()),
            maturity_constraints: None,
            haircut,
            fx_haircut_addon: defaults.fx_addon,
            concentration_limit: None,
        }
    }

    /// Create a corporate bonds eligibility entry.
    #[must_use]
    pub fn corporate_bonds(haircut: f64, min_rating: &str) -> Self {
        let defaults = CollateralAssetClass::CorporateBonds.defaults_or_panic();
        Self {
            asset_class: CollateralAssetClass::CorporateBonds,
            min_rating: Some(min_rating.to_string()),
            maturity_constraints: None,
            haircut,
            fx_haircut_addon: defaults.fx_addon,
            concentration_limit: defaults.concentration_limit,
        }
    }

    /// Calculate total haircut including FX add-on if applicable.
    #[must_use]
    pub fn total_haircut(&self, currency_mismatch: bool) -> f64 {
        if currency_mismatch {
            self.haircut + self.fx_haircut_addon
        } else {
            self.haircut
        }
    }
}

#[cfg(test)]
mod collateral_maturity_tests {
    use super::*;

    #[test]
    fn treasury_haircut_uses_remaining_maturity_bucket() {
        let schedule =
            EligibleCollateralSchedule::bcbs_standard().expect("registry schedule should load");

        assert_eq!(
            schedule.haircut_for_maturity(&CollateralAssetClass::GovernmentBonds, 0.5),
            Some(0.005)
        );
        assert_eq!(
            schedule.haircut_for_maturity(&CollateralAssetClass::GovernmentBonds, 3.0),
            Some(0.02)
        );
        assert_eq!(
            schedule.haircut_for_maturity(&CollateralAssetClass::GovernmentBonds, 30.0),
            Some(0.04)
        );
    }
}

/// Eligible collateral schedule with haircuts.
///
/// Defines the complete set of collateral types accepted under a CSA
/// or margin agreement, along with associated haircuts and constraints.
///
/// # Example
///
/// ```
/// use finstack_quant_margin::{CollateralEligibility, EligibleCollateralSchedule};
///
/// // Start from a standard schedule (BCBS-IOSCO compliant)
/// let schedule = EligibleCollateralSchedule::bcbs_standard()?;
/// # let _ = schedule;
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EligibleCollateralSchedule {
    /// List of eligible collateral types with haircuts
    pub eligible: Vec<CollateralEligibility>,

    /// Default haircut for unlisted collateral (if accepted)
    ///
    /// If None, only explicitly listed collateral types are accepted.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "serde_validation::default_haircut::deserialize",
        serialize_with = "serde_validation::default_haircut::serialize"
    )]
    #[cfg_attr(feature = "json-schema", schemars(range(min = 0.0, max = 1.0)))]
    pub default_haircut: Option<f64>,

    /// Whether rehypothecation of posted collateral is permitted
    ///
    /// For IM under BCBS-IOSCO rules, rehypothecation is prohibited.
    /// For VM, rehypothecation may be permitted by bilateral agreement.
    #[serde(default)]
    pub rehypothecation_allowed: bool,
}

impl Default for EligibleCollateralSchedule {
    fn default() -> Self {
        Self {
            eligible: vec![CollateralEligibility::cash()],
            default_haircut: None,
            rehypothecation_allowed: false,
        }
    }
}

impl EligibleCollateralSchedule {
    fn from_embedded_registry(schedule_id: &str) -> finstack_quant_core::Result<Self> {
        let registry = embedded_registry()?;
        registry
            .collateral_schedules
            .get(schedule_id)
            .cloned()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "collateral schedule '{schedule_id}' not found in registry"
                ))
            })
    }

    /// Create a schedule accepting only cash.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn cash_only() -> finstack_quant_core::Result<Self> {
        Self::from_embedded_registry("cash_only")
    }

    /// Create a standard BCBS-IOSCO compliant schedule.
    ///
    /// Includes cash and government bonds with standard haircuts.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn bcbs_standard() -> finstack_quant_core::Result<Self> {
        Self::from_embedded_registry("bcbs_standard")
    }

    /// Create a standard repo collateral schedule (US Treasuries).
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn us_treasuries() -> finstack_quant_core::Result<Self> {
        Self::from_embedded_registry("us_treasuries")
    }

    /// Load a named schedule from a provided config (with overrides).
    ///
    /// Configured registry overrides take precedence over the embedded margin
    /// registry. The returned schedule is cloned from the registry, so later
    /// changes to `cfg` do not mutate it.
    ///
    /// # Errors
    ///
    /// Returns an error if registry configuration cannot be parsed or validated,
    /// or `schedule_id` does not name a configured or embedded schedule.
    pub fn from_finstack_config(
        cfg: &FinstackConfig,
        schedule_id: &str,
    ) -> finstack_quant_core::Result<Self> {
        let registry = margin_registry_from_config(cfg)?;
        registry
            .collateral_schedules
            .get(schedule_id)
            .cloned()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "collateral schedule '{schedule_id}' not found"
                ))
            })
    }

    /// Find the applicable haircut for a given asset class.
    ///
    /// Returns the first matching eligibility entry's haircut, or the default
    /// haircut if no specific match is found.
    #[must_use]
    pub fn haircut_for(&self, asset_class: &CollateralAssetClass) -> Option<f64> {
        self.eligible
            .iter()
            .find(|e| &e.asset_class == asset_class)
            .map(|e| e.haircut)
            .or(self.default_haircut)
    }

    /// Find the applicable haircut for a given asset class and remaining maturity.
    ///
    /// Entries with maturity constraints are matched against `remaining_years`;
    /// unconstrained entries for the asset class remain eligible. Returns the
    /// schedule default only when no explicit entry for the asset class matches.
    #[must_use]
    pub fn haircut_for_maturity(
        &self,
        asset_class: &CollateralAssetClass,
        remaining_years: f64,
    ) -> Option<f64> {
        self.eligible
            .iter()
            .find(|e| {
                &e.asset_class == asset_class
                    && e.maturity_constraints
                        .as_ref()
                        .is_none_or(|c| c.is_satisfied(remaining_years))
            })
            .map(|e| e.haircut)
            .or(self.default_haircut)
    }

    /// Check if an asset class is eligible.
    #[must_use]
    pub fn is_eligible(&self, asset_class: &CollateralAssetClass) -> bool {
        self.eligible.iter().any(|e| &e.asset_class == asset_class)
            || self.default_haircut.is_some()
    }

    /// Validate a proposed collateral portfolio against concentration limits.
    ///
    /// Each entry in `allocations` maps an asset class to the proposed amount.
    /// Returns a list of asset classes that breach their concentration limit.
    #[must_use]
    pub fn check_concentration_limits(
        &self,
        allocations: &[(CollateralAssetClass, f64)],
    ) -> Vec<ConcentrationBreach> {
        let total: f64 = allocations.iter().map(|(_, amt)| *amt).sum();
        if total <= 0.0 {
            return Vec::new();
        }

        let mut breaches = Vec::new();
        for (asset_class, amount) in allocations {
            let fraction = amount / total;
            if let Some(entry) = self.eligible.iter().find(|e| &e.asset_class == asset_class) {
                if let Some(limit) = entry.concentration_limit {
                    if fraction > limit {
                        breaches.push(ConcentrationBreach {
                            asset_class: asset_class.clone(),
                            fraction,
                            limit,
                            excess: fraction - limit,
                        });
                    }
                }
            }
        }
        breaches
    }
}

/// A breach of a collateral concentration limit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ConcentrationBreach {
    /// Asset class that breached
    pub asset_class: CollateralAssetClass,
    /// Actual fraction of total collateral
    pub fraction: f64,
    /// Allowed concentration limit
    pub limit: f64,
    /// Excess fraction above limit
    pub excess: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collateral_asset_class_haircuts() {
        assert_eq!(
            CollateralAssetClass::Cash
                .standard_haircut()
                .expect("standard class should have haircut"),
            0.0
        );
        assert_eq!(
            CollateralAssetClass::Equity
                .standard_haircut()
                .expect("standard class should have haircut"),
            0.15
        );
        assert_eq!(
            CollateralAssetClass::Gold
                .standard_haircut()
                .expect("standard class should have haircut"),
            0.15
        );
    }

    #[test]
    fn maturity_constraints_work() {
        let constraints = MaturityConstraints {
            min_remaining_years: Some(1.0),
            max_remaining_years: Some(5.0),
        };
        assert!(!constraints.is_satisfied(0.5)); // Too short
        assert!(constraints.is_satisfied(2.0)); // Within range
        assert!(!constraints.is_satisfied(6.0)); // Too long
    }

    #[test]
    fn total_haircut_includes_fx_addon() {
        let eligibility = CollateralEligibility::government_bonds(0.02);
        assert_eq!(eligibility.total_haircut(false), 0.02);
        assert_eq!(eligibility.total_haircut(true), 0.10); // 0.02 + 0.08
    }

    #[test]
    fn schedule_finds_haircut() {
        let schedule = EligibleCollateralSchedule::cash_only().expect("registry should load");
        assert_eq!(schedule.haircut_for(&CollateralAssetClass::Cash), Some(0.0));
        assert_eq!(schedule.haircut_for(&CollateralAssetClass::Equity), None);
    }

    #[test]
    fn bcbs_standard_schedule() {
        let schedule = EligibleCollateralSchedule::bcbs_standard().expect("registry should load");
        assert!(schedule.is_eligible(&CollateralAssetClass::Cash));
        assert!(schedule.is_eligible(&CollateralAssetClass::GovernmentBonds));
        assert!(!schedule.is_eligible(&CollateralAssetClass::Equity));
        assert!(!schedule.rehypothecation_allowed);
    }

    #[test]
    fn concentration_limit_no_breach() {
        let schedule = EligibleCollateralSchedule {
            eligible: vec![
                CollateralEligibility::cash(),
                CollateralEligibility::corporate_bonds(0.04, "BBB"),
            ],
            default_haircut: None,
            rehypothecation_allowed: false,
        };

        let allocations = vec![
            (CollateralAssetClass::Cash, 80.0),
            (CollateralAssetClass::CorporateBonds, 20.0),
        ];
        let breaches = schedule.check_concentration_limits(&allocations);
        assert!(breaches.is_empty());
    }

    #[test]
    fn concentration_limit_breach() {
        let schedule = EligibleCollateralSchedule {
            eligible: vec![
                CollateralEligibility::cash(),
                CollateralEligibility::corporate_bonds(0.04, "BBB"),
            ],
            default_haircut: None,
            rehypothecation_allowed: false,
        };

        let allocations = vec![
            (CollateralAssetClass::Cash, 60.0),
            (CollateralAssetClass::CorporateBonds, 40.0),
        ];
        let breaches = schedule.check_concentration_limits(&allocations);
        assert_eq!(breaches.len(), 1);
        assert_eq!(
            breaches[0].asset_class,
            CollateralAssetClass::CorporateBonds
        );
        assert!((breaches[0].fraction - 0.40).abs() < 0.001);
        assert!((breaches[0].limit - 0.30).abs() < 0.001);
    }
}

//! OTC derivative margin specification.
//!
//! Shared margin specification for CSA-governed OTC derivatives
//! including IRS, CDS, CDS Index, and TRS.

use super::csa::CsaSpec;
use super::enums::{ClearingStatus, ImMethodology, MarginTenor};
use super::simm_types::SimmCreditSector;
use crate::registry::{embedded_registry, margin_registry_from_config};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

/// Explicit ISDA SIMM credit risk-class and bucket assignment.
///
/// Corporate, sovereign, and index credit exposures belong to credit
/// qualifying, including high-yield sectors represented by SIMM buckets 7-12.
/// Credit non-qualifying is reserved for securitizations and other exposures
/// governed by the non-qualifying risk class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "risk_class", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum SimmCreditClassification {
    /// Credit-qualifying exposure assigned to an ISDA SIMM sector bucket.
    Qualifying {
        /// Sector bucket used for credit-qualifying delta aggregation.
        sector: SimmCreditSector,
    },
    /// Credit non-qualifying exposure, typically a securitization.
    NonQualifying,
}

impl<'de> serde::Deserialize<'de> for SimmCreditClassification {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum RiskClass {
            Qualifying,
            NonQualifying,
        }

        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            risk_class: RiskClass,
            #[serde(default)]
            sector: Option<SimmCreditSector>,
        }

        let wire = <Wire as serde::Deserialize>::deserialize(deserializer)?;
        match (wire.risk_class, wire.sector) {
            (RiskClass::Qualifying, Some(sector)) => Ok(Self::Qualifying { sector }),
            (RiskClass::Qualifying, None) => Err(serde::de::Error::missing_field("sector")),
            (RiskClass::NonQualifying, None) => Ok(Self::NonQualifying),
            (RiskClass::NonQualifying, Some(_)) => Err(serde::de::Error::custom(
                "non-qualifying SIMM credit classification must not include sector",
            )),
        }
    }
}

/// OTC derivative margin specification (ISDA CSA compliant).
///
/// This is the standard margin specification for bilateral and cleared
/// OTC derivatives. It combines CSA terms with clearing-specific parameters.
///
/// # Usage
///
/// Attach this to any OTC derivative instrument that requires margining:
/// - Interest Rate Swaps (IRS)
/// - Credit Default Swaps (CDS)
/// - CDS Indices
/// - Total Return Swaps (TRS)
///
/// # Example
///
/// ```
/// use finstack_quant_margin::{
///     OtcMarginSpec, CsaSpec, SimmCreditClassification, SimmCreditSector,
/// };
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// // Bilateral (uncleared) derivative
/// let bilateral_spec = OtcMarginSpec::bilateral_simm(CsaSpec::usd_regulatory()?);
/// let credit_spec = bilateral_spec.with_simm_credit_classification(
///     SimmCreditClassification::Qualifying {
///         sector: SimmCreditSector::Financial,
///     },
/// );
///
/// // Cleared derivative
/// let cleared_spec = OtcMarginSpec::cleared("LCH", finstack_quant_core::currency::Currency::USD)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OtcMarginSpec {
    /// Full CSA specification (for bilateral trades)
    ///
    /// For cleared trades, this represents the terms with the CCP.
    pub csa: CsaSpec,

    /// Clearing status: bilateral or cleared through CCP
    pub clearing_status: ClearingStatus,

    /// Initial margin calculation methodology
    ///
    /// - Bilateral: SIMM or Schedule
    /// - Cleared: ClearingHouse (CCP-specific)
    pub im_methodology: ImMethodology,

    /// Explicit SIMM credit classification for credit-sensitive instruments.
    ///
    /// Required when a credit product uses `ImMethodology::Simm`; leave `None`
    /// for non-credit instruments and non-SIMM margin methodologies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simm_credit_classification: Option<SimmCreditClassification>,

    /// Variation margin exchange frequency
    pub vm_frequency: MarginTenor,

    /// Settlement lag for margin transfers (business days)
    pub settlement_lag: u32,
}

impl OtcMarginSpec {
    /// Create a bilateral margin spec using ISDA SIMM.
    ///
    /// This is the standard configuration for large dealer-to-dealer
    /// or dealer-to-client bilateral trades.
    #[must_use]
    pub fn bilateral_simm(csa: CsaSpec) -> Self {
        Self {
            csa,
            clearing_status: ClearingStatus::Bilateral,
            im_methodology: ImMethodology::Simm,
            simm_credit_classification: None,
            vm_frequency: MarginTenor::Daily,
            settlement_lag: 1,
        }
    }

    /// Create a bilateral margin spec using schedule-based IM.
    ///
    /// Used when SIMM is not implemented or for smaller counterparties.
    #[must_use]
    pub fn bilateral_schedule(csa: CsaSpec) -> Self {
        Self {
            csa,
            clearing_status: ClearingStatus::Bilateral,
            im_methodology: ImMethodology::Schedule,
            simm_credit_classification: None,
            vm_frequency: MarginTenor::Daily,
            settlement_lag: 1,
        }
    }

    /// Create a margin spec for cleared derivatives.
    ///
    /// # Arguments
    ///
    /// * `ccp` - Clearing house identifier (e.g., "LCH", "CME", "ICE")
    /// * `currency` - Settlement currency
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn cleared(ccp: impl Into<String>, currency: Currency) -> Result<Self> {
        let registry = embedded_registry()?;
        let eligible_collateral = registry
            .collateral_schedules
            .get("bcbs_standard")
            .cloned()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "collateral schedule 'bcbs_standard' not found in registry".to_string(),
                )
            })?;
        Self::build_cleared(ccp.into(), currency, registry, eligible_collateral)
    }

    /// Shared construction path for `cleared` and `cleared_from_config`.
    ///
    /// Takes an already-resolved registry + eligible-collateral schedule and
    /// assembles the CSA and spec. Keeping this as a `fn` (not a method) makes
    /// the call from registry-owning contexts explicit.
    fn build_cleared(
        ccp_name: String,
        currency: Currency,
        registry: &crate::registry::MarginRegistry,
        eligible_collateral: super::collateral::EligibleCollateralSchedule,
    ) -> Result<Self> {
        let mut vm_params = registry.defaults.vm.to_vm_params(currency)?;
        vm_params.rounding = Money::new(registry.defaults.cleared_settlement.rounding, currency)?;
        vm_params.settlement_lag = registry.defaults.cleared_settlement.settlement_lag;

        let csa = CsaSpec {
            id: format!("{}-CCP-CSA", ccp_name),
            base_currency: currency,
            calendar_id: super::default_margin_calendar(currency).to_string(),
            vm_params,
            im_params: Some(
                registry
                    .defaults
                    .im
                    .cleared
                    .to_im_params(ImMethodology::ClearingHouse, currency)?,
            ),
            eligible_collateral,
            call_timing: registry.defaults.timing.ccp.clone(),
            collateral_curve_id: finstack_quant_core::types::CurveId::new(format!(
                "{}-OIS",
                currency
            )),
        };

        Ok(Self {
            csa,
            clearing_status: ClearingStatus::Cleared { ccp: ccp_name },
            im_methodology: ImMethodology::ClearingHouse,
            simm_credit_classification: None,
            vm_frequency: MarginTenor::Daily,
            settlement_lag: registry.defaults.cleared_settlement.settlement_lag,
        })
    }

    /// Create a USD bilateral spec with standard regulatory terms.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn usd_bilateral() -> Result<Self> {
        Ok(Self::bilateral_simm(CsaSpec::usd_regulatory()?))
    }

    /// Create a EUR bilateral spec with standard regulatory terms.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn eur_bilateral() -> Result<Self> {
        Ok(Self::bilateral_simm(CsaSpec::eur_regulatory()?))
    }

    /// Create a margin spec for cleared derivatives using overrides from a config.
    pub fn cleared_from_config(
        ccp: impl Into<String>,
        currency: Currency,
        cfg: &FinstackConfig,
    ) -> Result<Self> {
        let registry = margin_registry_from_config(cfg)?;
        let eligible_collateral =
            super::collateral::EligibleCollateralSchedule::from_finstack_config(
                cfg,
                "bcbs_standard",
            )?;
        Self::build_cleared(ccp.into(), currency, &registry, eligible_collateral)
    }

    /// Attach an explicit SIMM credit risk-class and sector assignment.
    ///
    /// # Arguments
    ///
    /// * `classification` - Canonical qualifying-sector or non-qualifying
    ///   assignment used when credit delta is generated for the instrument.
    #[must_use]
    pub fn with_simm_credit_classification(
        mut self,
        classification: SimmCreditClassification,
    ) -> Self {
        self.simm_credit_classification = Some(classification);
        self
    }

    /// Validate margin terms required by credit-sensitive instruments.
    ///
    /// # Errors
    ///
    /// Returns a validation error when SIMM is selected without an explicit
    /// credit risk-class and sector classification.
    pub fn validate_for_credit(&self) -> Result<()> {
        if self.im_methodology == ImMethodology::Simm && self.simm_credit_classification.is_none() {
            return Err(finstack_quant_core::Error::Validation(
                "SIMM credit products require simm_credit_classification".to_string(),
            ));
        }
        Ok(())
    }

    /// Check if this is a cleared trade.
    #[must_use]
    pub fn is_cleared(&self) -> bool {
        matches!(self.clearing_status, ClearingStatus::Cleared { .. })
    }

    /// Check if this is a bilateral trade.
    #[must_use]
    pub fn is_bilateral(&self) -> bool {
        matches!(self.clearing_status, ClearingStatus::Bilateral)
    }

    /// Get the CCP name if cleared.
    #[must_use]
    pub fn ccp(&self) -> Option<&str> {
        match &self.clearing_status {
            ClearingStatus::Cleared { ccp } => Some(ccp.as_str()),
            ClearingStatus::Bilateral => None,
        }
    }

    /// Get the base currency for margin calculations.
    #[must_use]
    pub fn base_currency(&self) -> Currency {
        self.csa.base_currency
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::money::Money;

    #[test]
    fn bilateral_simm_spec() {
        let spec = OtcMarginSpec::usd_bilateral().expect("registry should load");
        assert!(spec.is_bilateral());
        assert!(!spec.is_cleared());
        assert_eq!(spec.im_methodology, ImMethodology::Simm);
        assert_eq!(spec.vm_frequency, MarginTenor::Daily);
        assert!(spec.simm_credit_classification.is_none());
        assert!(spec.ccp().is_none());
    }

    #[test]
    fn cleared_spec() {
        let spec = OtcMarginSpec::cleared("LCH", Currency::USD).expect("registry should load");
        assert!(spec.is_cleared());
        assert!(!spec.is_bilateral());
        assert_eq!(spec.im_methodology, ImMethodology::ClearingHouse);
        assert_eq!(spec.ccp(), Some("LCH"));
        assert_eq!(spec.settlement_lag, 0);
    }

    #[test]
    fn ice_clear_credit_spec() {
        let spec = OtcMarginSpec::cleared("ICE", Currency::USD).expect("registry should load");
        assert!(spec.is_cleared());
        assert_eq!(spec.ccp(), Some("ICE"));
        assert_eq!(spec.base_currency(), Currency::USD);
    }

    #[test]
    fn csa_thresholds() {
        let spec = OtcMarginSpec::cleared("CME", Currency::EUR).expect("registry should load");
        assert_eq!(
            spec.csa.vm_params.threshold,
            Money::from((0_i64, Currency::EUR))
        );
    }

    #[test]
    fn simm_credit_classification_has_explicit_tagged_wire_shape() {
        let spec = OtcMarginSpec::usd_bilateral()
            .expect("registry should load")
            .with_simm_credit_classification(SimmCreditClassification::Qualifying {
                sector: SimmCreditSector::Financial,
            });

        let json = serde_json::to_value(&spec).expect("serialize spec");
        assert_eq!(
            json["simm_credit_classification"],
            serde_json::json!({"risk_class": "qualifying", "sector": "financial"})
        );
        let roundtrip: OtcMarginSpec = serde_json::from_value(json).expect("deserialize spec");
        assert_eq!(roundtrip, spec);
    }

    #[test]
    fn non_qualifying_classification_does_not_accept_a_sector() {
        let payload = serde_json::json!({
            "risk_class": "non_qualifying",
            "sector": "financial"
        });

        assert!(serde_json::from_value::<SimmCreditClassification>(payload).is_err());
    }

    #[test]
    fn qualifying_classification_requires_a_sector() {
        let payload = serde_json::json!({"risk_class": "qualifying"});

        assert!(serde_json::from_value::<SimmCreditClassification>(payload).is_err());
    }

    #[test]
    fn simm_credit_products_require_classification() {
        let spec = OtcMarginSpec::usd_bilateral().expect("registry should load");

        assert!(spec.validate_for_credit().is_err());
    }

    #[test]
    fn classified_simm_credit_product_validates() {
        let spec = OtcMarginSpec::usd_bilateral()
            .expect("registry should load")
            .with_simm_credit_classification(SimmCreditClassification::Qualifying {
                sector: SimmCreditSector::Financial,
            });

        assert!(spec.validate_for_credit().is_ok());
    }
}

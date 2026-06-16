//! OTC derivative margin specification.
//!
//! Shared margin specification for CSA-governed OTC derivatives
//! including IRS, CDS, CDS Index, and TRS.

use super::csa::CsaSpec;
use super::enums::{ClearingStatus, ImMethodology, MarginTenor};
use crate::registry::{embedded_registry, margin_registry_from_config};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

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
/// ```ignore
/// use finstack_quant_margin::{OtcMarginSpec, CsaSpec, ClearingStatus, ImMethodology, MarginTenor};
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// // Bilateral (uncleared) derivative
/// let bilateral_spec = OtcMarginSpec::bilateral_simm(CsaSpec::usd_regulatory()?);
///
/// // Cleared derivative
/// let cleared_spec = OtcMarginSpec::cleared("LCH", finstack_quant_core::currency::Currency::USD)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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
        Ok(Self::build_cleared(
            ccp.into(),
            currency,
            registry,
            eligible_collateral,
        ))
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
    ) -> Self {
        let mut vm_params = registry.defaults.vm.to_vm_params(currency);
        vm_params.rounding = Money::new(registry.defaults.cleared_settlement.rounding, currency);
        vm_params.settlement_lag = registry.defaults.cleared_settlement.settlement_lag;

        let csa = CsaSpec {
            id: format!("{}-CCP-CSA", ccp_name),
            base_currency: currency,
            vm_params,
            im_params: Some(
                registry
                    .defaults
                    .im
                    .cleared
                    .to_im_params(ImMethodology::ClearingHouse, currency),
            ),
            eligible_collateral,
            call_timing: registry.defaults.timing.ccp.clone(),
            collateral_curve_id: finstack_quant_core::types::CurveId::new(format!(
                "{}-OIS",
                currency
            )),
        };

        Self {
            csa,
            clearing_status: ClearingStatus::Cleared { ccp: ccp_name },
            im_methodology: ImMethodology::ClearingHouse,
            vm_frequency: MarginTenor::Daily,
            settlement_lag: registry.defaults.cleared_settlement.settlement_lag,
        }
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
        Ok(Self::build_cleared(
            ccp.into(),
            currency,
            &registry,
            eligible_collateral,
        ))
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

impl Default for OtcMarginSpec {
    #[allow(clippy::expect_used)] // Embedded margin registry is a compile-time asset.
    fn default() -> Self {
        Self::usd_bilateral().expect("embedded margin registry is a compile-time asset")
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
        assert_eq!(spec.csa.vm_params.threshold, Money::new(0.0, Currency::EUR));
    }
}

//! Credit Support Annex (CSA) specification.
//!
//! Defines the CSA agreement terms that govern collateral exchange for
//! OTC derivatives under ISDA documentation.

use super::collateral::EligibleCollateralSchedule;
use super::enums::ImMethodology;
use super::thresholds::{ImParameters, VmParameters};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;

use crate::registry::{embedded_registry, margin_registry_from_config};
use finstack_quant_core::config::FinstackConfig;

/// Margin call timing parameters.
///
/// Specifies the operational timing for margin calls including
/// notification and dispute resolution windows.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarginCallTiming {
    /// Notification deadline (hours after valuation, e.g., 13:00 local time)
    pub notification_deadline_hours: u8,

    /// Response deadline (hours after notification)
    pub response_deadline_hours: u8,

    /// Dispute resolution window (business days)
    pub dispute_resolution_days: u8,

    /// Grace period for collateral delivery (business days)
    pub delivery_grace_days: u8,
}

impl Default for MarginCallTiming {
    fn default() -> Self {
        crate::registry::embedded_registry_or_panic()
            .defaults
            .timing
            .standard
            .clone()
    }
}

impl MarginCallTiming {
    /// Standard timing for regulatory VM CSA.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn regulatory_standard() -> Result<Self> {
        let registry = embedded_registry()?;
        Ok(registry.defaults.timing.regulatory_vm.clone())
    }
}

/// Credit Support Annex specification (ISDA standard).
///
/// The CSA governs the exchange of collateral between counterparties
/// for OTC derivatives. This specification captures all key commercial
/// terms needed for margin calculation and management.
///
/// # ISDA Documentation
///
/// This type represents terms from:
/// - ISDA 2016 Credit Support Annex for Variation Margin (VM CSA)
/// - ISDA 2018 Credit Support Annex for Initial Margin (IM CSA)
///
/// # References
///
/// - ISDA 2016 VM CSA: `docs/REFERENCES.md#isda-vm-csa-2016`
/// - ISDA 2018 IM CSA: `docs/REFERENCES.md#isda-im-csa-2018`
/// - BCBS-IOSCO uncleared margin framework: `docs/REFERENCES.md#bcbs-iosco-uncleared-margin`
///
/// # Example
///
/// ```ignore
/// use finstack_quant_margin::{
///     CsaSpec, VmParameters, ImParameters, EligibleCollateralSchedule,
///     MarginCallTiming, ImMethodology, MarginTenor,
/// };
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::money::Money;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let csa = CsaSpec {
///     id: "USD-CSA-2024".to_string(),
///     base_currency: Currency::USD,
///     vm_params: VmParameters::regulatory_standard(Currency::USD)?,
///     im_params: Some(ImParameters::simm_standard(Currency::USD)?),
///     eligible_collateral: EligibleCollateralSchedule::bcbs_standard()?,
///     call_timing: MarginCallTiming::regulatory_standard()?,
///     collateral_curve_id: "USD-OIS".into(),
/// };
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CsaSpec {
    /// CSA identifier (e.g., "USD-CSA-STANDARD", "COUNTERPARTY-XYZ-CSA")
    pub id: String,

    /// Base currency for margin calculations.
    ///
    /// All exposures and collateral values are converted to this currency
    /// for netting and comparison with thresholds.
    pub base_currency: Currency,

    /// Variation margin parameters.
    ///
    /// Governs daily mark-to-market collateral exchange.
    pub vm_params: VmParameters,

    /// Initial margin parameters (optional).
    ///
    /// If None, no IM is exchanged (either not in scope for regulations
    /// or trade is cleared).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub im_params: Option<ImParameters>,

    /// Eligible collateral schedule.
    ///
    /// Defines what collateral types are acceptable and associated haircuts.
    pub eligible_collateral: EligibleCollateralSchedule,

    /// Margin call timing parameters.
    pub call_timing: MarginCallTiming,

    /// Discount curve ID for collateral valuation.
    ///
    /// Cash collateral is typically discounted at OIS/RFR rates.
    /// This curve should match the CSA's collateral interest rate.
    pub collateral_curve_id: CurveId,
}

impl CsaSpec {
    /// Create a standard regulatory CSA for USD derivatives.
    ///
    /// This represents post-2016 regulatory compliant terms with:
    /// - Zero VM threshold
    /// - Daily margin exchange
    /// - SIMM for IM
    /// - Cash and government bonds as eligible collateral
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn usd_regulatory() -> Result<Self> {
        Self::regulatory_for_currency(Currency::USD, "USD-REGULATORY-CSA", "USD-OIS")
    }

    /// Create a standard regulatory CSA for EUR derivatives.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn eur_regulatory() -> Result<Self> {
        Self::regulatory_for_currency(Currency::EUR, "EUR-REGULATORY-CSA", "EUR-ESTR")
    }

    fn regulatory_for_currency(
        currency: Currency,
        id: &str,
        collateral_curve: &str,
    ) -> Result<Self> {
        Self::regulatory_inner(None, currency, id, collateral_curve)
    }

    /// Create a CSA using overrides resolved from a config.
    pub fn regulatory_from_config(
        cfg: &FinstackConfig,
        currency: Currency,
        id: &str,
        collateral_curve: &str,
    ) -> Result<Self> {
        Self::regulatory_inner(Some(cfg), currency, id, collateral_curve)
    }

    /// Shared construction path for the `*_regulatory` and `regulatory_from_config`
    /// constructors. Passing `Some(cfg)` selects the config-driven registry;
    /// `None` uses the embedded defaults.
    fn regulatory_inner(
        cfg: Option<&FinstackConfig>,
        currency: Currency,
        id: &str,
        collateral_curve: &str,
    ) -> Result<Self> {
        let (vm_params, im_params, eligible_collateral, call_timing) = match cfg {
            Some(cfg) => {
                let registry = margin_registry_from_config(cfg)?;
                (
                    VmParameters::from_finstack_config(cfg, currency)?,
                    ImParameters::from_finstack_config(cfg, ImMethodology::Simm, currency)?,
                    EligibleCollateralSchedule::from_finstack_config(cfg, "bcbs_standard")?,
                    registry.defaults.timing.regulatory_vm,
                )
            }
            None => (
                VmParameters::regulatory_standard(currency)?,
                ImParameters::simm_standard(currency)?,
                EligibleCollateralSchedule::bcbs_standard()?,
                MarginCallTiming::regulatory_standard()?,
            ),
        };
        Ok(Self {
            id: id.to_string(),
            base_currency: currency,
            vm_params,
            im_params: Some(im_params),
            eligible_collateral,
            call_timing,
            collateral_curve_id: CurveId::new(collateral_curve),
        })
    }

    /// Check if this CSA requires initial margin.
    #[must_use]
    pub fn requires_im(&self) -> bool {
        self.im_params.is_some()
    }

    /// Get the VM threshold amount.
    #[must_use]
    pub fn vm_threshold(&self) -> &finstack_quant_core::money::Money {
        &self.vm_params.threshold
    }

    /// Get the IM threshold amount (if IM is required).
    #[must_use]
    pub fn im_threshold(&self) -> Option<&finstack_quant_core::money::Money> {
        self.im_params.as_ref().map(|p| &p.threshold)
    }
}

impl Default for CsaSpec {
    fn default() -> Self {
        // Touch the embedded registry first via the consolidated panic
        // helper so any compile-time-asset failure surfaces with the
        // standard message; `usd_regulatory()` would also fail at the
        // same point but with a less specific error chain.
        let _ = crate::registry::embedded_registry_or_panic();
        #[allow(clippy::expect_used)] // proven reachable only on a build-pipeline regression
        Self::usd_regulatory()
            .expect("usd_regulatory(): embedded margin registry must already be loaded")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::money::Money;

    #[test]
    fn usd_regulatory_csa() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load");
        assert_eq!(csa.base_currency, Currency::USD);
        assert_eq!(csa.vm_params.threshold, Money::new(0.0, Currency::USD));
        assert!(csa.requires_im());
    }

    #[test]
    fn eur_regulatory_csa() {
        let csa = CsaSpec::eur_regulatory().expect("registry should load");
        assert_eq!(csa.base_currency, Currency::EUR);
        assert_eq!(csa.collateral_curve_id.as_str(), "EUR-ESTR");
    }

    #[test]
    fn margin_call_timing_defaults() {
        let timing = MarginCallTiming::default();
        assert_eq!(timing.notification_deadline_hours, 13);
        assert_eq!(timing.dispute_resolution_days, 2);
    }
}

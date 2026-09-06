//! Credit Support Annex (CSA) specification.
//!
//! Defines the CSA agreement terms that govern collateral exchange for
//! OTC derivatives under ISDA documentation.

use super::collateral::EligibleCollateralSchedule;
use super::enums::ImMethodology;
use super::serde_validation;
use super::thresholds::{ImParameters, VmParameters};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;

use crate::registry::{embedded_registry, margin_registry_from_config};
use finstack_quant_core::config::FinstackConfig;

/// Margin call timing parameters.
///
/// Specifies the operational timing for margin calls including
/// notification and dispute resolution windows.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MarginCallTiming {
    /// Notification deadline (hours after valuation, e.g., 13:00 local time)
    #[serde(
        deserialize_with = "serde_validation::notification_deadline_hours::deserialize",
        serialize_with = "serde_validation::notification_deadline_hours::serialize"
    )]
    #[cfg_attr(feature = "json-schema", schemars(range(max = 23)))]
    pub notification_deadline_hours: u8,

    /// Response deadline (hours after notification)
    pub response_deadline_hours: u8,

    /// Dispute resolution window (business days)
    pub dispute_resolution_days: u8,

    /// Grace period for collateral delivery (business days)
    pub delivery_grace_days: u8,
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
/// ```
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
///     calendar_id: "usny".into(),
/// };
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CsaSpec {
    /// CSA identifier (e.g., "USD-CSA-STANDARD", "COUNTERPARTY-XYZ-CSA")
    pub id: String,

    /// Base currency for margin calculations.
    ///
    /// All exposures and collateral values are converted to this currency
    /// for netting and comparison with thresholds.
    pub base_currency: Currency,

    /// Contractual business-day calendar for calls and settlements.
    pub calendar_id: String,

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

    /// Create a standard regulatory CSA for any currency from the embedded
    /// registry: zero VM threshold, daily exchange, SIMM IM, BCBS-IOSCO
    /// eligible collateral and the currency's default margin calendar.
    ///
    /// # Arguments
    ///
    /// * `currency` - Base currency for thresholds, MTA and collateral values.
    /// * `id` - CSA identifier used in margin lookups; must be non-empty.
    /// * `collateral_curve` - Discount curve id for collateral valuation
    ///   (typically the currency's OIS/RFR curve, e.g. `"GBP-SONIA"`).
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn regulatory_for_currency(
        currency: Currency,
        id: &str,
        collateral_curve: &str,
    ) -> Result<Self> {
        Self::regulatory_inner(None, currency, id, collateral_curve)
    }

    /// Return a copy with bilateral (legacy, non-zero) VM threshold terms.
    ///
    /// Rounding and independent amount keep the registry defaults of
    /// [`VmParameters::with_threshold`] when `None`; frequency and settlement
    /// lag are unchanged from the current spec.
    ///
    /// # Arguments
    ///
    /// * `threshold` - VM threshold below which no margin is exchanged, in
    ///   the CSA base currency.
    /// * `mta` - Minimum transfer amount in the CSA base currency.
    /// * `rounding` - Optional transfer rounding increment in the base
    ///   currency; `None` keeps the default (10,000).
    /// * `independent_amount` - Optional independent amount in the base
    ///   currency; `None` keeps zero.
    ///
    /// # Errors
    ///
    /// Returns a validation error if any supplied amount is not in the CSA
    /// base currency.
    pub fn with_vm_threshold(
        mut self,
        threshold: Money,
        mta: Money,
        rounding: Option<Money>,
        independent_amount: Option<Money>,
    ) -> Result<Self> {
        for (name, money) in [
            ("threshold", Some(threshold)),
            ("mta", Some(mta)),
            ("rounding", rounding),
            ("independent_amount", independent_amount),
        ] {
            if let Some(money) = money {
                if money.currency() != self.base_currency {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "CSA '{}' VM {name} currency mismatch: expected {}, got {}",
                        self.id,
                        self.base_currency,
                        money.currency()
                    )));
                }
            }
        }
        let frequency = self.vm_params.frequency;
        let settlement_lag = self.vm_params.settlement_lag;
        let mut vm = VmParameters::with_threshold(threshold, mta);
        if let Some(rounding) = rounding {
            vm.rounding = rounding;
        }
        if let Some(ia) = independent_amount {
            vm.independent_amount = ia;
        }
        vm.frequency = frequency;
        vm.settlement_lag = settlement_lag;
        self.vm_params = vm;
        Ok(self)
    }

    /// Return a copy with explicit initial-margin terms.
    ///
    /// Starts from [`ImParameters::for_methodology`] for the CSA base
    /// currency and overrides the four negotiable terms.
    ///
    /// # Arguments
    ///
    /// * `methodology` - IM calculation regime (SIMM, schedule, ...).
    /// * `mpor_days` - Margin period of risk in business days; must be positive.
    /// * `threshold` - IM threshold in the CSA base currency.
    /// * `mta` - IM minimum transfer amount in the CSA base currency.
    /// * `segregated` - Whether IM must be held with a third-party custodian.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `mpor_days` is zero, an amount is not in
    /// the CSA base currency, or the embedded registry cannot be loaded.
    pub fn with_im(
        mut self,
        methodology: ImMethodology,
        mpor_days: u32,
        threshold: Money,
        mta: Money,
        segregated: bool,
    ) -> Result<Self> {
        if mpor_days == 0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CSA '{}' IM mpor_days must be positive",
                self.id
            )));
        }
        for (name, money) in [("threshold", threshold), ("mta", mta)] {
            if money.currency() != self.base_currency {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CSA '{}' IM {name} currency mismatch: expected {}, got {}",
                    self.id,
                    self.base_currency,
                    money.currency()
                )));
            }
        }
        let mut im = ImParameters::for_methodology(methodology, self.base_currency)?;
        im.mpor_days = mpor_days;
        im.threshold = threshold;
        im.mta = mta;
        im.segregated = segregated;
        self.im_params = Some(im);
        Ok(self)
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
            calendar_id: super::default_margin_calendar(currency).to_string(),
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

    /// Validate CSA identifiers and the contractual holiday calendar.
    pub fn validate(&self) -> Result<()> {
        self.vm_params.validate(self.base_currency)?;
        if let Some(im) = &self.im_params {
            for (name, amount) in [("threshold", im.threshold), ("MTA", im.mta)] {
                if amount.currency() != self.base_currency
                    || !amount.amount().is_finite()
                    || amount.amount() < 0.0
                {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "IM {name} must be finite, nonnegative and denominated in {}",
                        self.base_currency
                    )));
                }
            }
            if im.mpor_days == 0 {
                return Err(finstack_quant_core::Error::Validation(
                    "IM MPOR must be positive".into(),
                ));
            }
        }
        if self.id.trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "CSA id must not be empty".into(),
            ));
        }
        if finstack_quant_core::dates::calendar_by_id(&self.calendar_id).is_none() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CSA '{}' calendar '{}' is not registered",
                self.id, self.calendar_id
            )));
        }
        if self.collateral_curve_id.as_str().trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CSA '{}' collateral curve id must not be empty",
                self.id
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usd_regulatory_csa() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load");
        assert_eq!(csa.base_currency, Currency::USD);
        assert_eq!(csa.vm_params.threshold, Money::from((0_i64, Currency::USD)));
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
        let timing = crate::registry::embedded_registry()
            .expect("registry should load")
            .defaults
            .timing
            .standard
            .clone();
        assert_eq!(timing.notification_deadline_hours, 13);
        assert_eq!(timing.dispute_resolution_days, 2);
    }

    #[test]
    fn with_vm_threshold_and_with_im_override_terms_in_base_currency() {
        let csa = CsaSpec::usd_regulatory()
            .expect("registry should load")
            .with_vm_threshold(
                Money::from((300_000_i64, Currency::USD)),
                Money::from((50_000_i64, Currency::USD)),
                None,
                Some(Money::from((100_000_i64, Currency::USD))),
            )
            .expect("same-currency terms")
            .with_im(
                ImMethodology::Schedule,
                5,
                Money::from((1_000_000_i64, Currency::USD)),
                Money::from((0_i64, Currency::USD)),
                true,
            )
            .expect("same-currency IM terms");
        assert_eq!(csa.vm_threshold().amount(), 300_000.0);
        assert_eq!(csa.vm_params.independent_amount.amount(), 100_000.0);
        assert_eq!(csa.vm_params.rounding.amount(), 10_000.0);
        let im = csa.im_params.as_ref().expect("im");
        assert_eq!(im.methodology, ImMethodology::Schedule);
        assert_eq!(im.mpor_days, 5);
        assert!(im.segregated);

        let mismatch = CsaSpec::usd_regulatory()
            .expect("registry should load")
            .with_vm_threshold(
                Money::from((1_i64, Currency::EUR)),
                Money::from((0_i64, Currency::USD)),
                None,
                None,
            );
        assert!(mismatch.is_err());
    }

    #[test]
    fn unmapped_currency_uses_registered_weekends_calendar() {
        let csa = CsaSpec::regulatory_for_currency(Currency::NZD, "NZD-CSA", "NZD-OIS")
            .expect("registry should load");
        assert_eq!(csa.calendar_id, "weekends_only");
        csa.validate().expect("fallback calendar should resolve");
    }
}

//! Variation margin and initial margin parameter specifications.
//!
//! Defines the threshold, MTA (Minimum Transfer Amount), and other parameters
//! that govern margin call mechanics.

use super::enums::{ImMethodology, MarginTenor};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

use crate::registry::embedded_registry;
use crate::registry::margin_registry_from_config;
use finstack_quant_core::config::FinstackConfig;

/// Variation margin parameters.
///
/// These parameters govern the daily (or periodic) exchange of variation margin
/// under a CSA agreement. VM is exchanged to eliminate mark-to-market exposure.
///
/// # ISDA CSA Standard Terms
///
/// The 2016 VM CSA introduced standardized terms for variation margin:
/// - Zero threshold for in-scope entities
/// - Daily exchange with T+1 settlement
/// - Cash or highly liquid securities as collateral
///
/// # Example
///
/// ```ignore
/// use finstack_quant_margin::{MarginTenor, VmParameters};
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::money::Money;
///
/// let vm_params = VmParameters {
///     threshold: Money::new(10_000_000.0, Currency::USD),
///     mta: Money::new(500_000.0, Currency::USD),
///     rounding: Money::new(10_000.0, Currency::USD),
///     independent_amount: Money::new(0.0, Currency::USD),
///     frequency: MarginTenor::Daily,
///     settlement_lag: 1,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VmParameters {
    /// Threshold amount below which no margin is exchanged.
    ///
    /// Under BCBS-IOSCO rules for covered entities, VM threshold must be zero.
    /// Legacy bilateral CSAs may have non-zero thresholds.
    pub threshold: Money,

    /// Minimum Transfer Amount (MTA).
    ///
    /// Margin calls below MTA are not made. BCBS-IOSCO permits combined
    /// IM+VM MTA up to €500,000 equivalent.
    pub mta: Money,

    /// Rounding increment for margin amounts.
    ///
    /// Margin calls are typically rounded to the nearest multiple of this amount.
    pub rounding: Money,

    /// Independent Amount (IA) / Additional Margin.
    ///
    /// Fixed collateral amount required regardless of exposure.
    /// Often used for credit enhancement or as a buffer.
    pub independent_amount: Money,

    /// Margin call frequency.
    ///
    /// Under BCBS-IOSCO, daily margin exchange is required.
    pub frequency: MarginTenor,

    /// Settlement lag in business days (T+n).
    ///
    /// Standard is T+1 for VM under 2016 VM CSA.
    pub settlement_lag: u32,
}

impl VmParameters {
    /// Create VM parameters with zero threshold (regulatory standard).
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn regulatory_standard(currency: Currency) -> Result<Self> {
        let registry = embedded_registry()?;
        Ok(Self::from_defaults(currency, &registry.defaults.vm))
    }

    /// Create VM parameters with a threshold (bilateral thresholds).
    #[must_use]
    pub fn with_threshold(threshold: Money, mta: Money) -> Self {
        let currency = threshold.currency();
        Self {
            threshold,
            mta,
            rounding: Money::new(10_000.0, currency),
            independent_amount: Money::new(0.0, currency),
            frequency: MarginTenor::Daily,
            settlement_lag: 1,
        }
    }

    /// Compute the required credit support amount **before** netting
    /// against currently posted collateral, rounding, and the MTA cutoff.
    ///
    /// # ISDA CSA Formula (symmetric, bilateral)
    ///
    /// ```text
    /// Excess              = sign(Exposure) × max(0, |Exposure| − Threshold)
    /// Required Support    = Excess + Independent_Amount
    /// ```
    ///
    /// This is the single source of truth used by both
    /// [`Self::calculate_margin_call`] and
    /// [`crate::calculators::VmCalculator::calculate`] to avoid silent
    /// formula drift between the net-exposure reporting field and the
    /// margin-call amount.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] if `exposure.currency()`
    /// does not match the threshold currency.
    pub fn required_credit_support(&self, exposure: Money) -> Result<Money> {
        if exposure.currency() != self.threshold.currency() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "VM exposure currency mismatch: expected {}, got {}",
                self.threshold.currency(),
                exposure.currency()
            )));
        }
        let currency = exposure.currency();
        let exp = exposure.amount();
        let threshold = self.threshold.amount();
        let ia = self.independent_amount.amount();

        // Reject NaN/infinity at the boundary so a corrupted overlay or
        // upstream miscalculation cannot silently propagate non-finite
        // margin numbers into a CSA call.
        if !exp.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "VM exposure must be finite, got {exp}"
            )));
        }
        if !threshold.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "VM threshold must be finite, got {threshold}"
            )));
        }
        if !ia.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "VM independent amount must be finite, got {ia}"
            )));
        }

        let abs_excess = (exp.abs() - threshold).max(0.0);
        let signed_excess = if abs_excess == 0.0 {
            0.0
        } else {
            exp.signum() * abs_excess
        };
        Ok(Money::new(signed_excess + ia, currency))
    }

    /// Calculate the credit support amount (margin to be delivered/returned).
    ///
    /// # ISDA CSA Formula
    ///
    /// Bilateral netting uses a **symmetric** threshold around signed exposure:
    ///
    /// ```text
    /// Excess = sign(Exposure) × max(0, |Exposure| − Threshold)
    /// Credit Support Amount = Excess + Independent_Amount − Current_Collateral
    /// ```
    ///
    /// When `Credit Support Amount` is negative and `Exposure ≥ 0`, the magnitude is
    /// **excess collateral returned** to the counterparty. When `Exposure < 0`, a
    /// negative amount means **delivery** of margin to the counterparty (implemented
    /// in [`crate::calculators::VmCalculator`]).
    ///
    /// Calls where the **rounded** credit support amount is strictly below the
    /// Minimum Transfer Amount return zero.
    ///
    /// # Arguments
    ///
    /// * `exposure` - Signed mark-to-market (positive = counterparty owes us)
    /// * `current_collateral` - Value of currently posted collateral
    ///
    /// # Returns
    ///
    /// Signed credit support amount (positive = collect from them; negative = return
    /// excess when `Exposure ≥ 0`, or post margin when `Exposure < 0`). Zero if
    /// below MTA after rounding.
    pub fn calculate_margin_call(
        &self,
        exposure: Money,
        current_collateral: Money,
    ) -> Result<Money> {
        if current_collateral.currency() != self.threshold.currency() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "VM collateral currency mismatch: expected {}, got {}",
                self.threshold.currency(),
                current_collateral.currency()
            )));
        }

        let required = self.required_credit_support(exposure)?;
        let currency = required.currency();
        let credit_support_amount = required.amount() - current_collateral.amount();
        let mta_amount = self.mta.amount();
        if credit_support_amount.abs() < mta_amount {
            return Ok(Money::new(0.0, currency));
        }

        let rounded = self.round_transfer_amount(
            Money::new(credit_support_amount, currency),
            exposure.amount(),
        );
        Ok(rounded)
    }

    /// Round a transfer amount according to ISDA delivery/return elections.
    fn round_transfer_amount(&self, amount: Money, exposure_amount: f64) -> Money {
        let rounding = self.rounding.amount();
        if rounding <= 0.0 {
            return amount;
        }
        let raw = amount.amount();
        if raw == 0.0 {
            return amount;
        }

        let units = raw.abs() / rounding;
        let rounded_abs = if raw > 0.0 || exposure_amount < 0.0 {
            units.ceil() * rounding
        } else {
            units.floor() * rounding
        };
        Money::new(raw.signum() * rounded_abs, amount.currency())
    }

    /// Build from defaults resolved via a `FinstackConfig`.
    pub fn from_finstack_config(cfg: &FinstackConfig, currency: Currency) -> Result<Self> {
        let registry = margin_registry_from_config(cfg)?;
        Ok(Self::from_defaults(currency, &registry.defaults.vm))
    }

    fn from_defaults(currency: Currency, defaults: &crate::registry::VmDefaults) -> Self {
        defaults.to_vm_params(currency)
    }
}

impl Default for VmParameters {
    #[allow(clippy::expect_used)] // Embedded margin registry is a compile-time asset.
    fn default() -> Self {
        Self::regulatory_standard(Currency::USD)
            .expect("embedded margin registry is a compile-time asset")
    }
}

/// Initial margin parameters.
///
/// Initial margin is collateral posted to cover potential future exposure (PFE)
/// during the close-out period following a default. IM is required for
/// non-centrally cleared derivatives under BCBS-IOSCO rules.
///
/// # Margin Period of Risk (MPOR)
///
/// The MPOR determines the horizon over which PFE is calculated:
/// - Standard: 10 business days for bilateral derivatives
/// - Reduced: 5 days for certain liquid products
///
/// # Example
///
/// ```ignore
/// use finstack_quant_margin::{ImMethodology, ImParameters};
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::money::Money;
///
/// let im_params = ImParameters {
///     methodology: ImMethodology::Simm,
///     mpor_days: 10,
///     threshold: Money::new(50_000_000.0, Currency::USD),
///     mta: Money::new(0.0, Currency::USD), // Combined with VM MTA
///     segregated: true,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImParameters {
    /// IM calculation methodology.
    ///
    /// Options include SIMM, regulatory schedule, or CCP methodology.
    pub methodology: ImMethodology,

    /// Margin Period of Risk in business days.
    ///
    /// Standard is 10 days under BCBS-IOSCO. CCPs may use shorter periods.
    pub mpor_days: u32,

    /// IM threshold (aggregate group level).
    ///
    /// BCBS-IOSCO permits €50M aggregate threshold at group level.
    /// Many large dealers operate with zero threshold by agreement.
    pub threshold: Money,

    /// Minimum Transfer Amount for IM.
    ///
    /// Combined IM+VM MTA must not exceed €500,000 under BCBS-IOSCO.
    pub mta: Money,

    /// Whether IM must be held in a segregated account.
    ///
    /// Under BCBS-IOSCO, IM must be segregated with a third-party custodian
    /// to protect it in case of the collecting party's insolvency.
    pub segregated: bool,
}

impl ImParameters {
    /// Canonical constructor: build IM parameters for a given methodology
    /// from the embedded margin registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn for_methodology(methodology: ImMethodology, currency: Currency) -> Result<Self> {
        let registry = embedded_registry()?;
        Ok(Self::from_registry_defaults(
            methodology,
            currency,
            registry,
        ))
    }

    /// Create IM parameters using ISDA SIMM methodology.
    ///
    /// Thin convenience wrapper around [`Self::for_methodology`].
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn simm_standard(currency: Currency) -> Result<Self> {
        Self::for_methodology(ImMethodology::Simm, currency)
    }

    /// Create IM parameters using schedule-based methodology.
    ///
    /// Thin convenience wrapper around [`Self::for_methodology`].
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn schedule_based(currency: Currency) -> Result<Self> {
        Self::for_methodology(ImMethodology::Schedule, currency)
    }

    /// Create IM parameters for cleared trades (CCP methodology).
    ///
    /// Thin convenience wrapper around [`Self::for_methodology`].
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn cleared(currency: Currency) -> Result<Self> {
        Self::for_methodology(ImMethodology::ClearingHouse, currency)
    }

    /// Create IM parameters for repos using haircut methodology.
    ///
    /// Thin convenience wrapper around [`Self::for_methodology`].
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn repo_haircut(currency: Currency) -> Result<Self> {
        Self::for_methodology(ImMethodology::Haircut, currency)
    }

    /// Create IM parameters using defaults resolved from a config.
    ///
    /// # Errors
    ///
    /// Returns an error if the margin registry cannot be loaded from `cfg`.
    pub fn from_finstack_config(
        cfg: &FinstackConfig,
        methodology: ImMethodology,
        currency: Currency,
    ) -> Result<Self> {
        let registry = margin_registry_from_config(cfg)?;
        Ok(Self::from_registry_defaults(
            methodology,
            currency,
            &registry,
        ))
    }

    /// Shared construction path for the methodology-keyed constructors.
    ///
    /// [`ImMethodology::InternalModel`] reuses the SIMM defaults because the
    /// internal-model calculator is a stub — when a dedicated defaults block
    /// is added to the registry, update this match arm.
    fn from_registry_defaults(
        methodology: ImMethodology,
        currency: Currency,
        registry: &crate::registry::MarginRegistry,
    ) -> Self {
        let defaults = match methodology {
            ImMethodology::Simm | ImMethodology::InternalModel => &registry.defaults.im.simm,
            ImMethodology::Schedule => &registry.defaults.im.schedule,
            ImMethodology::ClearingHouse => &registry.defaults.im.cleared,
            ImMethodology::Haircut => &registry.defaults.im.repo_haircut,
        };
        defaults.to_im_params(methodology, currency)
    }
}

impl Default for ImParameters {
    #[allow(clippy::expect_used)] // Embedded margin registry is a compile-time asset.
    fn default() -> Self {
        Self::simm_standard(Currency::USD)
            .expect("embedded margin registry is a compile-time asset")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_params_regulatory_standard() {
        let params =
            VmParameters::regulatory_standard(Currency::USD).expect("registry should load");
        assert_eq!(params.threshold, Money::new(0.0, Currency::USD));
        assert_eq!(params.frequency, MarginTenor::Daily);
        assert_eq!(params.settlement_lag, 1);
    }

    #[test]
    fn vm_margin_call_calculation() {
        let params = VmParameters {
            threshold: Money::new(1_000_000.0, Currency::USD),
            mta: Money::new(100_000.0, Currency::USD),
            rounding: Money::new(10_000.0, Currency::USD),
            independent_amount: Money::new(0.0, Currency::USD),
            frequency: MarginTenor::Daily,
            settlement_lag: 1,
        };

        // Exposure below threshold: no margin call
        let exposure = Money::new(500_000.0, Currency::USD);
        let collateral = Money::new(0.0, Currency::USD);
        let call = params
            .calculate_margin_call(exposure, collateral)
            .expect("matching currencies should succeed");
        assert_eq!(call, Money::new(0.0, Currency::USD));

        // Exposure above threshold: margin call
        let exposure = Money::new(2_000_000.0, Currency::USD);
        let call = params
            .calculate_margin_call(exposure, collateral)
            .expect("matching currencies should succeed");
        assert_eq!(call, Money::new(1_000_000.0, Currency::USD)); // 2M - 1M threshold

        // Amount below MTA: no call
        let exposure = Money::new(1_050_000.0, Currency::USD);
        let call = params
            .calculate_margin_call(exposure, collateral)
            .expect("matching currencies should succeed");
        assert_eq!(call, Money::new(0.0, Currency::USD)); // 50K < 100K MTA
    }

    #[test]
    fn vm_margin_call_mta_applies_before_rounding() {
        let params = VmParameters {
            threshold: Money::new(0.0, Currency::USD),
            mta: Money::new(150_000.0, Currency::USD),
            rounding: Money::new(10_000.0, Currency::USD),
            independent_amount: Money::new(0.0, Currency::USD),
            frequency: MarginTenor::Daily,
            settlement_lag: 1,
        };

        // Raw excess 147k is below MTA, so no call should be made even
        // though the rounded amount would reach the 150k threshold.
        let exposure = Money::new(147_000.0, Currency::USD);
        let collateral = Money::new(0.0, Currency::USD);
        let call = params
            .calculate_margin_call(exposure, collateral)
            .expect("matching currencies should succeed");
        assert_eq!(call, Money::new(0.0, Currency::USD));
    }

    #[test]
    fn vm_margin_call_rounds_delivery_up_and_return_down() {
        let params = VmParameters {
            threshold: Money::new(0.0, Currency::USD),
            mta: Money::new(100_000.0, Currency::USD),
            rounding: Money::new(10_000.0, Currency::USD),
            independent_amount: Money::new(0.0, Currency::USD),
            frequency: MarginTenor::Daily,
            settlement_lag: 1,
        };

        let delivery_to_us = params
            .calculate_margin_call(
                Money::new(151_001.0, Currency::USD),
                Money::new(0.0, Currency::USD),
            )
            .expect("delivery call should calculate");
        assert_eq!(delivery_to_us, Money::new(160_000.0, Currency::USD));

        let return_to_them = params
            .calculate_margin_call(
                Money::new(0.0, Currency::USD),
                Money::new(151_001.0, Currency::USD),
            )
            .expect("return call should calculate");
        assert_eq!(return_to_them, Money::new(-150_000.0, Currency::USD));

        let delivery_to_them = params
            .calculate_margin_call(
                Money::new(-151_001.0, Currency::USD),
                Money::new(0.0, Currency::USD),
            )
            .expect("negative exposure delivery call should calculate");
        assert_eq!(delivery_to_them, Money::new(-160_000.0, Currency::USD));
    }

    #[test]
    fn vm_margin_call_negative_exposure_bilateral() {
        let params = VmParameters {
            threshold: Money::new(1_000_000.0, Currency::USD),
            mta: Money::new(100_000.0, Currency::USD),
            rounding: Money::new(10_000.0, Currency::USD),
            independent_amount: Money::new(0.0, Currency::USD),
            frequency: MarginTenor::Daily,
            settlement_lag: 1,
        };

        // We owe the counterparty: symmetric excess applies to |Exposure| − Threshold.
        let exposure = Money::new(-2_000_000.0, Currency::USD);
        let collateral = Money::new(0.0, Currency::USD);
        let call = params
            .calculate_margin_call(exposure, collateral)
            .expect("matching currencies should succeed");
        assert_eq!(call, Money::new(-1_000_000.0, Currency::USD)); // −(2M − 1M)
    }

    #[test]
    fn im_params_simm_standard() {
        let params = ImParameters::simm_standard(Currency::EUR).expect("registry should load");
        assert_eq!(params.methodology, ImMethodology::Simm);
        assert_eq!(params.mpor_days, 10);
        assert!(params.segregated);
    }

    #[test]
    fn im_params_cleared() {
        let params = ImParameters::cleared(Currency::USD).expect("registry should load");
        assert_eq!(params.methodology, ImMethodology::ClearingHouse);
        assert_eq!(params.mpor_days, 5);
        assert!(!params.segregated);
        assert_eq!(params.threshold, Money::new(0.0, Currency::USD));
    }
}

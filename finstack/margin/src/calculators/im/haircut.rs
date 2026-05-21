//! Haircut-based initial margin calculator.
//!
//! Standard methodology for repos and securities financing transactions
//! where IM is calculated as a percentage of collateral value.

use crate::calculators::traits::{ImCalculator, ImResult};
use crate::traits::Marginable;
use crate::types::{CollateralAssetClass, EligibleCollateralSchedule, ImMethodology};
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::Result;

/// Haircut-based initial margin calculator.
///
/// Calculates IM based on collateral value and asset-class-specific haircuts.
/// This is the standard methodology for repos, securities lending, and
/// other secured financing transactions.
///
/// # Formula
///
/// ```text
/// IM = Collateral_Value × Haircut
/// Total_Haircut = Base_Haircut + FX_Addon (if currency mismatch)
/// ```
///
/// # BCBS-IOSCO Haircut Schedule
///
/// Standard haircuts by asset class:
/// - Cash: 0% (8% FX addon if currency mismatch)
/// - Government bonds ≤1yr: 0.5%
/// - Government bonds 1-5yr: 2%
/// - Government bonds >5yr: 4%
/// - Corporate bonds IG: 2-8%
/// - Equity: 15%
///
/// # Example
///
/// ```ignore
/// use finstack_margin::{EligibleCollateralSchedule, HaircutImCalculator, ImCalculator, Marginable};
/// use finstack_core::dates::Date;
/// use finstack_core::market_data::context::MarketContext;
/// use time::macros::date;
///
/// # fn main() -> finstack_core::Result<()> {
/// let schedule = EligibleCollateralSchedule::us_treasuries()?;
/// let calc = HaircutImCalculator::new(schedule);
///
/// # let repo: &dyn Marginable = todo!("provide a marginable secured financing instrument");
/// # let context = MarketContext::new();
/// # let as_of: Date = date!(2025-01-01);
/// let im = calc.calculate(repo, &context, as_of)?;
/// println!("Haircut IM: {}", im.amount);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct HaircutImCalculator {
    /// Eligible collateral schedule with haircuts.
    eligible_collateral: EligibleCollateralSchedule,

    /// Default collateral asset class to assume.
    default_asset_class: CollateralAssetClass,

    /// Posted collateral currency, if different from the exposure
    /// currency. The FX add-on is applied iff this is set *and* differs
    /// from the instrument's MTM currency at calculation time. Replaces
    /// the previous two-flag (`apply_fx_addon`, `currency_mismatch`)
    /// builder state with a single explicit value: there is no way to
    /// configure "apply FX add-on but currencies match" by accident.
    posted_collateral_currency: Option<finstack_core::currency::Currency>,
}

impl HaircutImCalculator {
    /// Create a new haircut calculator with the given collateral schedule.
    #[must_use]
    pub fn new(eligible_collateral: EligibleCollateralSchedule) -> Self {
        Self {
            eligible_collateral,
            default_asset_class: CollateralAssetClass::GovernmentBonds,
            posted_collateral_currency: None,
        }
    }

    /// Create a calculator for US Treasury collateral.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn us_treasuries() -> Result<Self> {
        Ok(Self::new(EligibleCollateralSchedule::us_treasuries()?))
    }

    /// Create a calculator with BCBS-IOSCO standard haircuts.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded margin registry cannot be loaded.
    pub fn bcbs_standard() -> Result<Self> {
        Ok(Self::new(EligibleCollateralSchedule::bcbs_standard()?))
    }

    /// Declare the posted collateral currency. The FX add-on is applied
    /// at calculation time when this currency differs from the
    /// instrument's MTM currency.
    #[must_use]
    pub fn with_posted_collateral_currency(
        mut self,
        currency: finstack_core::currency::Currency,
    ) -> Self {
        self.posted_collateral_currency = Some(currency);
        self
    }

    /// Set the default asset class.
    #[must_use]
    pub fn with_default_asset_class(mut self, asset_class: CollateralAssetClass) -> Self {
        self.default_asset_class = asset_class;
        self
    }

    /// Calculate haircut IM for a given collateral value and asset class.
    ///
    /// `currency_mismatch` is the explicit caller-provided indicator
    /// that the posted collateral currency differs from the exposure
    /// currency; the FX add-on is applied iff true.
    pub fn calculate_for_collateral(
        &self,
        collateral_value: Money,
        asset_class: &CollateralAssetClass,
        currency_mismatch: bool,
    ) -> Result<Money> {
        let haircut = match self.eligible_collateral.haircut_for(asset_class) {
            Some(h) => h,
            None => asset_class.standard_haircut()?,
        };

        let total_haircut = if currency_mismatch {
            haircut + asset_class.fx_addon()?
        } else {
            haircut
        };

        Ok(collateral_value * total_haircut)
    }

    /// Get the haircut for an asset class.
    pub fn haircut_for(&self, asset_class: &CollateralAssetClass) -> Result<f64> {
        match self.eligible_collateral.haircut_for(asset_class) {
            Some(h) => Ok(h),
            None => asset_class.standard_haircut(),
        }
    }
}

impl ImCalculator for HaircutImCalculator {
    fn calculate(
        &self,
        instrument: &dyn Marginable,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<ImResult> {
        let mtm = instrument.mtm_for_vm(context, as_of)?;
        let collateral_value = Money::new(mtm.amount().abs(), mtm.currency());

        // Derive the FX-addon flag from the actual currency pair instead
        // of carrying it as builder state.
        let currency_mismatch = self
            .posted_collateral_currency
            .is_some_and(|c| c != mtm.currency());

        let im_amount = self.calculate_for_collateral(
            collateral_value,
            &self.default_asset_class,
            currency_mismatch,
        )?;

        let mut breakdown = finstack_core::HashMap::default();
        breakdown.insert(self.default_asset_class.to_string(), im_amount);

        Ok(ImResult::with_breakdown(
            im_amount,
            ImMethodology::Haircut,
            as_of,
            2, // Short MPOR for repos
            breakdown,
        ))
    }

    fn methodology(&self) -> ImMethodology {
        ImMethodology::Haircut
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;

    #[test]
    fn haircut_calculation() {
        let calc = HaircutImCalculator::us_treasuries().expect("registry should load");

        let collateral = Money::new(10_000_000.0, Currency::USD);
        let im = calc
            .calculate_for_collateral(collateral, &CollateralAssetClass::GovernmentBonds, false)
            .expect("calculation ok");

        // Should apply ~1-2% haircut
        assert!(im.amount() > 0.0);
        assert!(im.amount() < 500_000.0); // Less than 5%
    }

    #[test]
    fn fx_addon_applied() {
        let calc = HaircutImCalculator::bcbs_standard().expect("registry should load");

        let collateral = Money::new(10_000_000.0, Currency::USD);

        let im_no_fx = calc
            .calculate_for_collateral(collateral, &CollateralAssetClass::Cash, false)
            .expect("calculation ok");

        let im_with_fx = calc
            .calculate_for_collateral(collateral, &CollateralAssetClass::Cash, true)
            .expect("calculation ok");

        // Cash with FX mismatch should have 8% haircut
        assert_eq!(im_no_fx.amount(), 0.0); // Cash has 0% haircut
        assert_eq!(im_with_fx.amount(), 800_000.0); // 8% FX addon
    }

    #[test]
    fn calculate_respects_currency_mismatch_runtime_flag() {
        use crate::traits::Marginable;
        use finstack_core::market_data::context::MarketContext;
        use time::macros::date;

        struct TestMarginable {
            value: Money,
        }
        impl Marginable for TestMarginable {
            fn id(&self) -> &str {
                "TEST"
            }
            fn margin_spec(&self) -> Option<&crate::OtcMarginSpec> {
                None
            }
            fn netting_set_id(&self) -> Option<crate::NettingSetId> {
                None
            }
            fn simm_sensitivities(
                &self,
                _m: &MarketContext,
                _a: Date,
            ) -> Result<crate::SimmSensitivities> {
                Ok(crate::SimmSensitivities::new(self.value.currency()))
            }
            fn mtm_for_vm(&self, _m: &MarketContext, _a: Date) -> Result<Money> {
                Ok(self.value)
            }
        }

        let instrument = TestMarginable {
            value: Money::new(10_000_000.0, Currency::USD),
        };
        let context = MarketContext::new();
        let as_of: Date = date!(2025 - 01 - 01);

        // No posted-collateral-currency declared → no FX addon, even
        // for an FX-sensitive asset class like cash.
        let calc_no_decl = HaircutImCalculator::bcbs_standard()
            .expect("registry should load")
            .with_default_asset_class(CollateralAssetClass::Cash);
        let im_same_ccy = calc_no_decl
            .calculate(&instrument, &context, as_of)
            .expect("calculation ok");
        assert_eq!(
            im_same_ccy.amount.amount(),
            0.0,
            "no posted-collateral-currency declared → no FX addon"
        );

        // Posted collateral currency matches MTM → no FX addon.
        let calc_match = HaircutImCalculator::bcbs_standard()
            .expect("registry should load")
            .with_default_asset_class(CollateralAssetClass::Cash)
            .with_posted_collateral_currency(Currency::USD);
        let im_match = calc_match
            .calculate(&instrument, &context, as_of)
            .expect("calculation ok");
        assert_eq!(
            im_match.amount.amount(),
            0.0,
            "posted USD == MTM USD → no FX addon"
        );

        // Posted collateral currency differs → FX addon applied.
        let calc_mismatch = HaircutImCalculator::bcbs_standard()
            .expect("registry should load")
            .with_default_asset_class(CollateralAssetClass::Cash)
            .with_posted_collateral_currency(Currency::EUR);
        let im_mismatch = calc_mismatch
            .calculate(&instrument, &context, as_of)
            .expect("calculation ok");
        assert_eq!(
            im_mismatch.amount.amount(),
            800_000.0,
            "posted EUR ≠ MTM USD → 8% FX addon applied to cash"
        );
    }

    #[test]
    fn default_haircuts() {
        assert_eq!(
            CollateralAssetClass::Cash
                .standard_haircut()
                .expect("default class should be configured"),
            0.0
        );
        assert_eq!(
            CollateralAssetClass::Equity
                .standard_haircut()
                .expect("default class should be configured"),
            0.15
        );
        assert_eq!(
            CollateralAssetClass::Gold
                .standard_haircut()
                .expect("default class should be configured"),
            0.15
        );
    }
}

//! Haircut-based initial margin calculator.
//!
//! Standard methodology for repos and securities financing transactions
//! where IM is calculated as a percentage of collateral value.

use crate::calculators::traits::{ImCalculator, ImResult};
use crate::traits::Marginable;
use crate::types::{CollateralAssetClass, EligibleCollateralSchedule, ImMethodology};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::{types::CreditRating, Result};

/// Margin period of risk in business days used for haircut-based repo IM.
pub const HAIRCUT_MPOR_DAYS: u32 = 2;

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
/// ```no_run
/// use finstack_quant_margin::{EligibleCollateralSchedule, HaircutImCalculator, ImCalculator, Marginable};
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use time::macros::date;
///
/// # fn main() -> finstack_quant_core::Result<()> {
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

    /// Posted collateral currency. The FX add-on applies only when this is
    /// set and differs from the instrument's MTM currency.
    posted_collateral_currency: Option<finstack_quant_core::currency::Currency>,
    remaining_years: Option<f64>,
    rating: Option<CreditRating>,
}

impl HaircutImCalculator {
    /// Create a new haircut calculator with the given collateral schedule.
    #[must_use]
    pub fn new(eligible_collateral: EligibleCollateralSchedule) -> Self {
        Self {
            eligible_collateral,
            default_asset_class: CollateralAssetClass::GovernmentBonds,
            posted_collateral_currency: None,
            remaining_years: None,
            rating: None,
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
        currency: finstack_quant_core::currency::Currency,
    ) -> Self {
        self.posted_collateral_currency = Some(currency);
        self
    }

    /// Supply collateral terms used to enforce contractual eligibility.
    ///
    /// # Arguments
    ///
    /// * `remaining_years` - Finite nonnegative residual maturity in years.
    /// * `rating` - Current collateral credit rating; absence is accepted only for entries without a rating floor.
    ///
    /// # Errors
    ///
    /// Rejects negative or non-finite maturity.
    pub fn with_collateral_terms(
        mut self,
        remaining_years: f64,
        rating: Option<CreditRating>,
    ) -> Result<Self> {
        if !remaining_years.is_finite() || remaining_years < 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "collateral maturity must be finite and nonnegative".into(),
            ));
        }
        self.remaining_years = Some(remaining_years);
        self.rating = rating;
        Ok(self)
    }

    fn matching_entry(
        &self,
        asset_class: &CollateralAssetClass,
    ) -> Result<&crate::types::CollateralEligibility> {
        for entry in &self.eligible_collateral.eligible {
            if &entry.asset_class != asset_class {
                continue;
            }
            if let Some(constraints) = &entry.maturity_constraints {
                if !self
                    .remaining_years
                    .is_some_and(|years| constraints.is_satisfied(years))
                {
                    continue;
                }
            }
            if let Some(min_rating) = &entry.min_rating {
                let floor: CreditRating = min_rating.parse()?;
                if !self
                    .rating
                    .is_some_and(|rating| rating != CreditRating::NR && rating <= floor)
                {
                    continue;
                }
            }
            if !entry.haircut.is_finite()
                || !(0.0..=1.0).contains(&entry.haircut)
                || !entry.fx_haircut_addon.is_finite()
                || !(0.0..=1.0).contains(&entry.fx_haircut_addon)
            {
                return Err(finstack_quant_core::Error::Validation(
                    "invalid contractual haircut".into(),
                ));
            }
            return Ok(entry);
        }
        Err(finstack_quant_core::Error::Validation(format!(
            "collateral {asset_class} has no eligible entry for the supplied maturity and rating"
        )))
    }

    /// Set the default asset class.
    #[must_use]
    pub fn with_default_asset_class(mut self, asset_class: CollateralAssetClass) -> Self {
        self.default_asset_class = asset_class;
        self
    }

    /// Eligible-collateral schedule the haircuts are read from.
    #[must_use]
    pub fn eligible_collateral(&self) -> &EligibleCollateralSchedule {
        &self.eligible_collateral
    }

    /// Default collateral asset class assumed by trait-based calculations.
    #[must_use]
    pub fn default_asset_class(&self) -> &CollateralAssetClass {
        &self.default_asset_class
    }

    /// Posted-collateral currency used to detect an FX mismatch, if declared.
    #[must_use]
    pub fn posted_collateral_currency(&self) -> Option<finstack_quant_core::currency::Currency> {
        self.posted_collateral_currency
    }

    /// Margin period of risk stamped on every result ([`HAIRCUT_MPOR_DAYS`]).
    #[must_use]
    pub fn mpor_days(&self) -> u32 {
        HAIRCUT_MPOR_DAYS
    }

    /// Calculate haircut IM for a given collateral value and asset class.
    ///
    /// # Arguments
    ///
    /// * `collateral_value` - Collateral market value.
    /// * `asset_class` - Collateral asset class used for haircut lookup and
    ///   the result breakdown key.
    /// * `currency_mismatch` - Explicit caller-provided indicator that the
    ///   posted collateral currency differs from the exposure currency; the
    ///   asset-class FX add-on is applied iff true.
    /// * `as_of` - Calculation date stored on the returned result.
    ///
    /// # Returns
    ///
    /// An [`ImResult`] with methodology [`ImMethodology::Haircut`], the
    /// canonical repo haircut MPOR, and one breakdown entry keyed by
    /// `asset_class`.
    ///
    /// # Errors
    ///
    /// Returns an error for negative collateral, missing maturity or rating required
    /// by the schedule, ineligible collateral, or invalid contractual haircuts.
    pub fn calculate_for_collateral(
        &self,
        collateral_value: Money,
        asset_class: &CollateralAssetClass,
        currency_mismatch: bool,
        as_of: Date,
    ) -> Result<ImResult> {
        if !collateral_value.amount().is_finite() || collateral_value.amount() < 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "collateral value must be finite and nonnegative".into(),
            ));
        }
        let entry = self.matching_entry(asset_class)?;
        let total_haircut = entry.total_haircut(currency_mismatch);

        let amount = collateral_value * total_haircut;
        Ok(Self::result_from_amount(
            amount,
            as_of,
            asset_class.to_string(),
        ))
    }

    /// Get the haircut for an asset class.
    pub fn haircut_for(&self, asset_class: &CollateralAssetClass) -> Result<f64> {
        Ok(self.matching_entry(asset_class)?.haircut)
    }

    fn result_from_amount(
        amount: Money,
        as_of: Date,
        breakdown_key: impl Into<String>,
    ) -> ImResult {
        let mut breakdown = finstack_quant_core::HashMap::default();
        breakdown.insert(breakdown_key.into(), amount);
        ImResult::with_breakdown(
            amount,
            ImMethodology::Haircut,
            as_of,
            HAIRCUT_MPOR_DAYS,
            breakdown,
        )
    }
}

impl ImCalculator for HaircutImCalculator {
    fn calculate(
        &self,
        instrument: &dyn Marginable,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<ImResult> {
        let collateral_value = instrument
            .im_exposure_base(context, as_of)?
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "Haircut IM requires an explicit im_exposure_base for instrument '{}'",
                    instrument.id()
                ))
            })?;

        let currency_mismatch = self
            .posted_collateral_currency
            .is_some_and(|c| c != collateral_value.currency());

        self.calculate_for_collateral(
            collateral_value,
            &self.default_asset_class,
            currency_mismatch,
            as_of,
        )
    }

    fn methodology(&self) -> ImMethodology {
        ImMethodology::Haircut
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    fn test_date() -> Date {
        Date::from_calendar_date(2024, time::Month::January, 1).expect("valid date")
    }

    #[test]
    fn haircut_calculation() {
        let calc = HaircutImCalculator::us_treasuries()
            .expect("registry should load")
            .with_collateral_terms(0.5, Some(CreditRating::AAA))
            .expect("terms");

        let collateral = Money::from((10_000_000_i64, Currency::USD));
        let im = calc
            .calculate_for_collateral(
                collateral,
                &CollateralAssetClass::GovernmentBonds,
                false,
                test_date(),
            )
            .expect("calculation ok");

        // Should apply ~1-2% haircut
        assert!(im.amount.amount() > 0.0);
        assert!(im.amount.amount() < 500_000.0); // Less than 5%
    }

    #[test]
    fn fx_addon_applied() {
        let calc = HaircutImCalculator::bcbs_standard()
            .expect("registry should load")
            .with_collateral_terms(0.5, Some(CreditRating::AAA))
            .expect("terms");

        let collateral = Money::from((10_000_000_i64, Currency::USD));

        let im_no_fx = calc
            .calculate_for_collateral(collateral, &CollateralAssetClass::Cash, false, test_date())
            .expect("calculation ok");

        let im_with_fx = calc
            .calculate_for_collateral(collateral, &CollateralAssetClass::Cash, true, test_date())
            .expect("calculation ok");

        // Cash with FX mismatch should have 8% haircut
        assert_eq!(im_no_fx.amount.amount(), 0.0); // Cash has 0% haircut
        assert_eq!(im_with_fx.amount.amount(), 800_000.0); // 8% FX addon
    }

    #[test]
    fn calculate_respects_currency_mismatch_runtime_flag() {
        use crate::traits::Marginable;
        use finstack_quant_core::market_data::context::MarketContext;
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
            fn im_exposure_base(
                &self,
                _market: &MarketContext,
                _as_of: Date,
            ) -> Result<Option<Money>> {
                Ok(Some(self.value))
            }
        }

        let instrument = TestMarginable {
            value: Money::from((10_000_000_i64, Currency::USD)),
        };
        let context = MarketContext::new();
        let as_of: Date = date!(2025 - 01 - 01);

        // No posted-collateral-currency declared → no FX addon, even
        // for an FX-sensitive asset class like cash.
        let calc_no_decl = HaircutImCalculator::bcbs_standard()
            .expect("registry should load")
            .with_default_asset_class(CollateralAssetClass::Cash);
        let im_same_currency = calc_no_decl
            .calculate(&instrument, &context, as_of)
            .expect("calculation ok");
        assert_eq!(
            im_same_currency.amount.amount(),
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
    fn calculate_uses_im_exposure_base_not_net_mtm() {
        use crate::traits::Marginable;
        use finstack_quant_core::market_data::context::MarketContext;
        use time::macros::date;

        struct TestRepo {
            mtm: Money,
            exposure_base: Money,
        }
        impl Marginable for TestRepo {
            fn id(&self) -> &str {
                "REPO"
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
                Ok(crate::SimmSensitivities::new(self.exposure_base.currency()))
            }
            fn mtm_for_vm(&self, _m: &MarketContext, _a: Date) -> Result<Money> {
                Ok(self.mtm)
            }
            fn im_exposure_base(
                &self,
                _market: &MarketContext,
                _as_of: Date,
            ) -> Result<Option<Money>> {
                Ok(Some(self.exposure_base))
            }
        }

        let repo = TestRepo {
            mtm: Money::from((0_i64, Currency::USD)),
            exposure_base: Money::from((100_000_000_i64, Currency::USD)),
        };
        let calc = HaircutImCalculator::bcbs_standard()
            .expect("registry should load")
            .with_collateral_terms(0.5, Some(CreditRating::AAA))
            .expect("terms");
        let result = calc
            .calculate(&repo, &MarketContext::new(), date!(2025 - 01 - 01))
            .expect("haircut IM should calculate from exposure base");

        assert_eq!(result.amount, Money::from((500_000_i64, Currency::USD)));
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

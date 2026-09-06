//! BCBS-IOSCO regulatory schedule-based IM calculator.
//!
//! Fallback methodology using grid-based rates applied to notional amounts.
//! Simpler but typically more conservative than SIMM.
//!
//! # Error Handling
//!
//! The constructors
//! [`crate::calculators::im::schedule::RegulatorySchedule::bcbs_iosco()`] and
//! [`crate::calculators::im::schedule::ScheduleImCalculator::bcbs_standard()`]
//! return `Result` rather than panicking,
//! allowing callers to handle missing registry data gracefully.

use crate::calculators::traits::{ImCalculator, ImResult};
use crate::registry::{embedded_registry, margin_registry_from_config};
use crate::traits::Marginable;
use crate::types::ImMethodology;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::neumaier_sum;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;
use std::borrow::Cow;

/// Asset class for schedule-based IM calculation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScheduleAssetClass {
    /// Interest rate derivatives
    InterestRate,
    /// Credit derivatives
    Credit,
    /// Equity derivatives
    Equity,
    /// Commodity derivatives
    Commodity,
    /// Foreign exchange derivatives
    Fx,
    /// Other derivatives
    Other,
    /// Custom user-defined asset class (from JSON)
    Custom(String),
}

impl ScheduleAssetClass {
    /// Canonical string identifier for this asset class.
    pub fn as_str(&self) -> Cow<'_, str> {
        match self {
            ScheduleAssetClass::InterestRate => Cow::Borrowed("interest_rate"),
            ScheduleAssetClass::Credit => Cow::Borrowed("credit"),
            ScheduleAssetClass::Equity => Cow::Borrowed("equity"),
            ScheduleAssetClass::Commodity => Cow::Borrowed("commodity"),
            ScheduleAssetClass::Fx => Cow::Borrowed("fx"),
            ScheduleAssetClass::Other => Cow::Borrowed("other"),
            ScheduleAssetClass::Custom(name) => Cow::Owned(format!("custom_{name}")),
        }
    }
}

impl serde::Serialize for ScheduleAssetClass {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str().as_ref())
    }
}

impl<'de> serde::Deserialize<'de> for ScheduleAssetClass {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for ScheduleAssetClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ScheduleAssetClass {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "interest_rate" => Ok(ScheduleAssetClass::InterestRate),
            "credit" => Ok(ScheduleAssetClass::Credit),
            "equity" => Ok(ScheduleAssetClass::Equity),
            "commodity" => Ok(ScheduleAssetClass::Commodity),
            "fx" => Ok(ScheduleAssetClass::Fx),
            "other" => Ok(ScheduleAssetClass::Other),
            custom if custom.starts_with("custom_") => {
                let name = &custom["custom_".len()..];
                if name.is_empty()
                    || !name.chars().all(|character| {
                        character.is_ascii_lowercase()
                            || character.is_ascii_digit()
                            || character == '_'
                    })
                {
                    return Err(format!("invalid custom schedule asset class {s:?}"));
                }
                Ok(ScheduleAssetClass::Custom(name.to_string()))
            }
            _ => Err(format!("unknown schedule asset class {s:?}")),
        }
    }
}

/// BCBS-IOSCO regulatory schedule for IM calculation.
///
/// Stores the schedule-grid rates used by the regulatory fallback methodology
/// for uncleared derivatives. Rates are decimals, so `0.04` means 4% of the
/// regulatory notional or other proxy exposure base.
///
/// # References
///
/// - BCBS-IOSCO uncleared margin framework: `docs/REFERENCES.md#bcbs-iosco-uncleared-margin`
#[derive(Debug, Clone)]
pub struct RegulatorySchedule {
    /// IM rates by asset class and maturity bucket
    pub rates: HashMap<(ScheduleAssetClass, MaturityBucket), f64>,
    /// Short/medium bucket boundary in years.
    pub short_to_medium: f64,
    /// Medium/long bucket boundary in years.
    pub medium_to_long: f64,
    /// Default rate when no explicit bucket is available.
    pub default_rate: f64,
}

/// Maturity bucket for schedule IM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaturityBucket {
    /// Less than 2 years
    Short,
    /// 2-5 years
    Medium,
    /// Greater than 5 years
    Long,
}

/// Default schedule IM registry id for the BCBS-IOSCO grid (`schedule_im.v1.json`).
pub const BCBS_IOSCO_SCHEDULE_ID: &str = "bcbs_iosco";

/// Look up a schedule IM grid by id in `registry`.
fn schedule_entry<'a>(
    registry: &'a crate::registry::MarginRegistry,
    schedule_id: &str,
) -> Result<&'a crate::registry::ScheduleImSchedule> {
    registry.schedule_im.get(schedule_id).ok_or_else(|| {
        finstack_quant_core::InputError::NotFound {
            id: format!("schedule_im '{schedule_id}'"),
        }
        .into()
    })
}

impl RegulatorySchedule {
    /// BCBS-IOSCO standard schedule loaded from the embedded registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded registry cannot be loaded or if the
    /// [`BCBS_IOSCO_SCHEDULE_ID`] schedule entry is missing.
    pub fn bcbs_iosco() -> Result<Self> {
        Self::from_registry_id(BCBS_IOSCO_SCHEDULE_ID)
    }

    /// Load a named schedule IM grid from the embedded registry (no hard-coded rates).
    ///
    /// # Errors
    ///
    /// Returns an error if the registry cannot be loaded or `schedule_id` is absent.
    pub fn from_registry_id(schedule_id: &str) -> Result<Self> {
        Ok(Self::from_registry(
            schedule_entry(embedded_registry()?, schedule_id)?.clone(),
        ))
    }

    /// Build from a registry entry.
    #[must_use]
    pub fn from_registry(entry: crate::registry::ScheduleImSchedule) -> Self {
        Self {
            rates: entry.rates,
            short_to_medium: entry.boundaries.short_to_medium,
            medium_to_long: entry.boundaries.medium_to_long,
            default_rate: entry.default_rate,
        }
    }

    /// Get the IM rate for an asset class and maturity.
    ///
    /// # Arguments
    ///
    /// * `asset_class` - Regulatory schedule class; absent entries use the contractual default rate.
    /// * `maturity_years` - Finite nonnegative residual maturity in years.
    ///
    /// # Errors
    ///
    /// Rejects invalid maturity or a selected rate outside the finite [0, 1] range.
    pub fn rate(&self, asset_class: ScheduleAssetClass, maturity_years: f64) -> Result<f64> {
        validate_maturity(maturity_years)?;
        let bucket = if maturity_years < self.short_to_medium {
            MaturityBucket::Short
        } else if maturity_years < self.medium_to_long {
            MaturityBucket::Medium
        } else {
            MaturityBucket::Long
        };

        let rate = *self
            .rates
            .get(&(asset_class, bucket))
            .unwrap_or(&self.default_rate);
        if !rate.is_finite() || !(0.0..=1.0).contains(&rate) {
            return Err(finstack_quant_core::Error::Validation(
                "schedule IM rate must be finite and in [0, 1]".into(),
            ));
        }
        Ok(rate)
    }
}

/// Schedule-based IM calculator.
///
/// Implements the BCBS-IOSCO schedule fallback for uncleared derivatives.
/// The schedule itself is a percentage grid keyed by asset class and maturity
/// bucket, with rates stored in decimal form.
///
/// There are two distinct entry points:
/// - [`Self::calculate_for_notional`] applies the schedule to an explicit
///   notional amount supplied by the caller.
/// - [`ImCalculator::calculate`] requires [`Marginable::im_exposure_base`] to
///   supply a regulatory notional or approved exposure base. It fails closed
///   when that input is absent.
///
/// # Formula
///
/// ```text
/// Explicit notional helper:
/// IM = |Notional| × Schedule_Rate(asset_class, maturity)
///
/// Trait-based path:
/// IM = |Explicit_IM_Exposure_Base| × Schedule_Rate(default_asset_class, default_maturity)
/// ```
///
/// # Conventions
///
/// - `Schedule_Rate` is a decimal fraction, not basis points.
/// - Maturity is supplied as a year fraction.
/// - The embedded BCBS-IOSCO grid uses the schedule boundaries carried in the
///   registry entry rather than hard-coded bucket cutoffs.
///
/// # Example
///
/// ```no_run
/// use finstack_quant_margin::{ImCalculator, Marginable, ScheduleImCalculator};
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use time::macros::date;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let calc = ScheduleImCalculator::bcbs_standard()?;
/// # let swap: &dyn Marginable = todo!("provide a marginable instrument");
/// # let context = MarketContext::new();
/// # let as_of: Date = date!(2025-01-01);
/// let im = calc.calculate(swap, &context, as_of)?;
/// # let _ = im;
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - BCBS-IOSCO uncleared margin framework: `docs/REFERENCES.md#bcbs-iosco-uncleared-margin`
#[derive(Debug, Clone)]
pub struct ScheduleImCalculator {
    /// Regulatory schedule
    pub schedule: RegulatorySchedule,
    /// Default asset class to assume
    pub default_asset_class: ScheduleAssetClass,
    /// Default maturity in years
    pub default_maturity_years: f64,
    /// Margin period of risk in business days
    pub mpor_days: u32,
}

impl ScheduleImCalculator {
    /// Create calculator with the embedded BCBS-IOSCO standard schedule.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded registry cannot be loaded or if the
    /// [`BCBS_IOSCO_SCHEDULE_ID`] schedule entry is missing.
    pub fn bcbs_standard() -> Result<Self> {
        Self::from_registry_id(BCBS_IOSCO_SCHEDULE_ID)
    }

    /// Create calculator from a schedule grid id in the embedded or merged registry.
    ///
    /// # Arguments
    ///
    /// * `schedule_id` - Registry identifier such as [`BCBS_IOSCO_SCHEDULE_ID`]
    ///
    /// # Errors
    ///
    /// Returns an error if the registry cannot be loaded or `schedule_id` is not found.
    pub fn from_registry_id(schedule_id: &str) -> Result<Self> {
        Ok(Self::from_entry(schedule_entry(
            embedded_registry()?,
            schedule_id,
        )?))
    }

    fn from_entry(entry: &crate::registry::ScheduleImSchedule) -> Self {
        Self {
            schedule: RegulatorySchedule::from_registry(entry.clone()),
            default_asset_class: entry.default_asset_class.clone(),
            default_maturity_years: entry.default_maturity_years,
            mpor_days: entry.mpor_days,
        }
    }

    /// Create calculator resolved from a provided `FinstackConfig`.
    ///
    /// Loads the schedule entry identified by [`BCBS_IOSCO_SCHEDULE_ID`] after
    /// applying any margin-registry overlay in the config.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry cannot be loaded from the config or if the
    /// "bcbs_iosco" schedule entry is missing.
    pub fn from_finstack_config(cfg: &finstack_quant_core::config::FinstackConfig) -> Result<Self> {
        let registry = margin_registry_from_config(cfg)?;
        Ok(Self::from_entry(schedule_entry(
            &registry,
            BCBS_IOSCO_SCHEDULE_ID,
        )?))
    }

    /// Set the default asset class used by [`ImCalculator::calculate`].
    ///
    /// # Arguments
    ///
    /// * `asset_class` - Asset class used when the trait-based fallback path
    ///   cannot infer a more specific regulatory schedule bucket
    ///
    /// # Returns
    ///
    /// The updated calculator.
    #[must_use]
    pub fn with_asset_class(mut self, asset_class: ScheduleAssetClass) -> Self {
        self.default_asset_class = asset_class;
        self
    }

    /// Set the default maturity used by [`ImCalculator::calculate`].
    ///
    /// # Arguments
    ///
    /// * `years` - Finite nonnegative residual maturity expressed as a year fraction
    ///
    /// # Returns
    ///
    /// The updated calculator.
    ///
    /// # Errors
    ///
    /// Rejects negative or non-finite maturity.
    pub fn with_maturity(mut self, years: f64) -> Result<Self> {
        validate_maturity(years)?;
        self.default_maturity_years = years;
        Ok(self)
    }

    /// Calculate gross schedule IM from an explicit notional amount.
    ///
    /// # Arguments
    ///
    /// * `notional` - Regulatory notional or caller-supplied exposure base in
    ///   the reporting currency. The calculation uses `abs(notional)`.
    /// * `asset_class` - Schedule asset class used for the rate lookup and
    ///   result breakdown key.
    /// * `maturity_years` - Finite nonnegative residual maturity as a year fraction.
    /// * `as_of` - Calculation date stored on the returned result.
    ///
    /// # Returns
    ///
    /// An [`ImResult`] with methodology [`ImMethodology::Schedule`], the
    /// calculator's registry-backed MPOR, and one breakdown entry keyed by
    /// `asset_class`. The amount is `|notional| × rate`, with the rate taken
    /// from the configured schedule grid. This is the **gross** IM — no netting
    /// benefit is applied. For portfolios with offsetting positions use
    /// [`calculate_netting_set_with_ngr`](Self::calculate_netting_set_with_ngr)
    /// which implements the BCBS-IOSCO NGR reduction factor.
    pub fn calculate_for_notional(
        &self,
        notional: Money,
        asset_class: ScheduleAssetClass,
        maturity_years: f64,
        as_of: Date,
    ) -> Result<ImResult> {
        let rate = self.schedule.rate(asset_class.clone(), maturity_years)?;
        let amount = Money::new(notional.amount().abs() * rate, notional.currency())?;
        Ok(self.result_from_amount(amount, as_of, asset_class.to_string()))
    }

    /// Calculate trade-specific gross schedule IM, then apply one netting-set NGR.
    ///
    /// # Arguments
    ///
    /// * `positions` - Tuples of signed MtM, gross notional, schedule asset class,
    ///   and finite nonnegative residual maturity in years. All money must share
    ///   one reporting currency. Each trade receives its own schedule rate.
    /// * `as_of` - Calculation date stored on the result.
    ///
    /// # Returns
    ///
    /// No result for an empty set or zero gross notional. Otherwise the sum of
    /// trade gross IM multiplied by `0.4 + 0.6 * NGR`, where NGR is positive net
    /// MtM divided by gross positive MtM (zero when gross positive MtM is zero).
    ///
    /// # Errors
    ///
    /// Rejects mixed currencies, invalid maturities or non-finite amounts.
    pub fn calculate_netting_set_with_ngr(
        &self,
        positions: &[(Money, Money, ScheduleAssetClass, f64)],
        as_of: Date,
    ) -> Result<Option<ImResult>> {
        let Some(first) = positions.first() else {
            return Ok(None);
        };
        let currency = first.0.currency();
        let mut gross_by_class = HashMap::default();
        let mut gross_notional = 0.0;
        for (mtm, notional, asset_class, maturity) in positions {
            if mtm.currency() != currency
                || notional.currency() != currency
                || !mtm.amount().is_finite()
                || !notional.amount().is_finite()
                || !maturity.is_finite()
                || *maturity < 0.0
            {
                return Err(finstack_quant_core::Error::Validation("schedule IM requires finite amounts in one currency and nonnegative finite maturities".into()));
            }
            gross_notional += notional.amount().abs();
            let gross =
                notional.amount().abs() * self.schedule.rate(asset_class.clone(), *maturity)?;
            *gross_by_class
                .entry(Self::ngr_breakdown_key(asset_class))
                .or_insert(0.0) += gross;
        }
        if gross_notional == 0.0 {
            return Ok(None);
        }
        let signed_mtm = neumaier_sum(positions.iter().map(|p| p.0.amount()));
        let positive_mtm = neumaier_sum(positions.iter().map(|p| p.0.amount().max(0.0)));
        let ngr = if positive_mtm == 0.0 {
            0.0
        } else {
            (signed_mtm.max(0.0) / positive_mtm).clamp(0.0, 1.0)
        };
        let reduction = 0.4 + 0.6 * ngr;
        let mut breakdown = HashMap::default();
        for (key, gross) in gross_by_class {
            breakdown.insert(key, Money::new(gross * reduction, currency)?);
        }
        let amount = Money::new(
            neumaier_sum(breakdown.values().map(|m| m.amount())),
            currency,
        )?;
        Ok(Some(ImResult::with_breakdown(
            amount,
            ImMethodology::Schedule,
            as_of,
            self.mpor_days,
            breakdown,
        )))
    }

    /// Canonical schedule IM breakdown key for NGR-reduced netting-set results.
    ///
    /// # Arguments
    ///
    /// * `asset_class` - SIMM asset class selecting the applicable schedule or risk weights.
    #[must_use]
    pub fn ngr_breakdown_key(asset_class: &ScheduleAssetClass) -> String {
        format!("{asset_class}_ngr")
    }

    /// Get the decimal schedule rate for an asset class and maturity.
    ///
    /// # Arguments
    ///
    /// * `asset_class` - Regulatory schedule asset class
    /// * `maturity_years` - Remaining maturity as a year fraction
    ///
    /// # Returns
    ///
    /// A decimal rate such as `0.01` for 1%.
    pub fn rate(&self, asset_class: ScheduleAssetClass, maturity_years: f64) -> Result<f64> {
        self.schedule.rate(asset_class, maturity_years)
    }

    fn result_from_amount(
        &self,
        amount: Money,
        as_of: Date,
        breakdown_key: impl Into<String>,
    ) -> ImResult {
        let mut breakdown = HashMap::default();
        breakdown.insert(breakdown_key.into(), amount);
        ImResult::with_breakdown(
            amount,
            ImMethodology::Schedule,
            as_of,
            self.mpor_days,
            breakdown,
        )
    }
}

impl ImCalculator for ScheduleImCalculator {
    fn calculate(
        &self,
        instrument: &dyn Marginable,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<ImResult> {
        let notional = super::require_im_exposure_base(
            "Schedule",
            instrument,
            context,
            as_of,
            "a regulatory notional",
        )?;

        self.calculate_for_notional(
            notional,
            self.default_asset_class.clone(),
            self.default_maturity_years,
            as_of,
        )
    }

    fn methodology(&self) -> ImMethodology {
        ImMethodology::Schedule
    }
}

fn validate_maturity(years: f64) -> Result<()> {
    if !years.is_finite() || years < 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "schedule IM maturity must be finite and nonnegative".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    fn test_date() -> Date {
        Date::from_calendar_date(2024, time::Month::January, 1).expect("valid date")
    }

    fn ir_positions(pairs: &[(Money, Money)]) -> Vec<(Money, Money, ScheduleAssetClass, f64)> {
        pairs
            .iter()
            .copied()
            .map(|(mtm, notional)| (mtm, notional, ScheduleAssetClass::InterestRate, 5.0))
            .collect()
    }

    #[test]
    fn bcbs_schedule_rates() {
        let schedule = RegulatorySchedule::bcbs_iosco()
            .expect("bcbs_iosco schedule should load from embedded registry");

        // Interest rate
        assert_eq!(
            schedule
                .rate(ScheduleAssetClass::InterestRate, 1.0)
                .expect("rate"),
            0.01
        );
        assert_eq!(
            schedule
                .rate(ScheduleAssetClass::InterestRate, 3.0)
                .expect("rate"),
            0.02
        );
        assert_eq!(
            schedule
                .rate(ScheduleAssetClass::InterestRate, 10.0)
                .expect("rate"),
            0.04
        );

        assert_eq!(
            schedule
                .rate(ScheduleAssetClass::Credit, 1.0)
                .expect("rate"),
            0.02
        );
        assert_eq!(
            schedule
                .rate(ScheduleAssetClass::Credit, 10.0)
                .expect("rate"),
            0.10
        );

        // Equity (constant)
        assert_eq!(
            schedule
                .rate(ScheduleAssetClass::Equity, 1.0)
                .expect("rate"),
            0.15
        );
        assert_eq!(
            schedule
                .rate(ScheduleAssetClass::Equity, 10.0)
                .expect("rate"),
            0.15
        );
    }

    #[test]
    fn schedule_im_calculation() {
        let calc = ScheduleImCalculator::bcbs_standard()
            .expect("bcbs_standard calculator should load from embedded registry");

        let notional = Money::from((100_000_000_i64, Currency::USD));
        let im = calc
            .calculate_for_notional(notional, ScheduleAssetClass::InterestRate, 5.0, test_date())
            .expect("IM");

        // 5y IR uses long bucket (4%) since maturity >= 5.0
        assert_eq!(im.amount.amount(), 4_000_000.0);
    }

    #[test]
    fn trait_path_fails_closed_without_explicit_exposure_base() {
        #[derive(Clone)]
        struct AtMarketInstrument {
            id: String,
            value: Money,
        }

        impl Marginable for AtMarketInstrument {
            fn id(&self) -> &str {
                &self.id
            }

            fn margin_spec(&self) -> Option<&crate::OtcMarginSpec> {
                None
            }

            fn netting_set_id(&self) -> Option<crate::NettingSetId> {
                None
            }

            fn simm_sensitivities(
                &self,
                _market: &MarketContext,
                _as_of: Date,
            ) -> Result<crate::SimmSensitivities> {
                Ok(crate::SimmSensitivities::new(self.value.currency()))
            }

            fn mtm_for_vm(&self, _market: &MarketContext, _as_of: Date) -> Result<Money> {
                Ok(self.value)
            }
        }

        let calc = ScheduleImCalculator::bcbs_standard()
            .expect("bcbs_standard calculator should load from embedded registry");
        let instrument = AtMarketInstrument {
            id: "ATM-SWAP".to_string(),
            value: Money::from((0_i64, Currency::USD)),
        };
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2024, time::Month::January, 1).expect("valid date");

        let err = calc
            .calculate(&instrument, &market, as_of)
            .expect_err("schedule IM must not use zero MtM as a notional proxy");

        assert!(
            err.to_string().contains("exposure base"),
            "expected missing exposure-base error, got {err}"
        );
    }

    #[test]
    fn credit_schedule_im() {
        let calc = ScheduleImCalculator::bcbs_standard()
            .expect("bcbs_standard calculator should load from embedded registry")
            .with_asset_class(ScheduleAssetClass::Credit)
            .with_maturity(7.0)
            .expect("maturity");

        let notional = Money::from((50_000_000_i64, Currency::USD));
        let im = calc
            .calculate_for_notional(notional, ScheduleAssetClass::Credit, 7.0, test_date())
            .expect("IM");

        // 7y credit uses long bucket (10%)
        assert_eq!(im.amount.amount(), 5_000_000.0);
    }

    #[test]
    fn bcbs_constructors_return_ok() {
        assert!(
            RegulatorySchedule::bcbs_iosco().is_ok(),
            "RegulatorySchedule::bcbs_iosco() should return Ok"
        );
        assert!(
            ScheduleImCalculator::bcbs_standard().is_ok(),
            "ScheduleImCalculator::bcbs_standard() should return Ok"
        );
    }

    /// Perfectly-offset netting set (Σ MtM = 0 → NGR = 0): IM reduces
    /// to 40% of the gross value. BCBS-IOSCO: `0.4 + 0.6·NGR = 0.4`.
    #[test]
    fn ngr_fully_offset_book_gives_40pct_of_gross() {
        let calc = ScheduleImCalculator::bcbs_standard()
            .expect("bcbs_standard loads")
            .with_asset_class(ScheduleAssetClass::InterestRate);

        // Two perfectly-offsetting positions of ±10M MtM, each 100M notional.
        let positions = [
            (
                Money::new(10.0e6, Currency::USD).expect("valid money fixture"),
                Money::new(100.0e6, Currency::USD).expect("valid money fixture"),
            ),
            (
                Money::new(-10.0e6, Currency::USD).expect("valid money fixture"),
                Money::new(100.0e6, Currency::USD).expect("valid money fixture"),
            ),
        ];
        let im = calc
            .calculate_netting_set_with_ngr(&ir_positions(&positions), test_date())
            .expect("valid netting-set IM fixture")
            .expect("NGR computable");

        // Gross = 200M, 5Y IR rate = 4%, reduction = 0.4.
        let expected = 200.0e6 * 0.04 * 0.4;
        assert!(
            (im.amount.amount() - expected).abs() < 1e-6,
            "fully-offset NGR gives 40% reduction, expected {expected}, got {}",
            im.amount.amount()
        );
    }

    /// Fully-directional book (no offsets, all MtMs same sign → NGR=1):
    /// IM equals gross formula `Gross × Rate`. Reduction = 1.0.
    #[test]
    fn ngr_fully_directional_book_gives_gross_im() {
        let calc = ScheduleImCalculator::bcbs_standard()
            .expect("bcbs_standard loads")
            .with_asset_class(ScheduleAssetClass::InterestRate);

        let positions = [
            (
                Money::new(10.0e6, Currency::USD).expect("valid money fixture"),
                Money::new(100.0e6, Currency::USD).expect("valid money fixture"),
            ),
            (
                Money::new(5.0e6, Currency::USD).expect("valid money fixture"),
                Money::new(50.0e6, Currency::USD).expect("valid money fixture"),
            ),
        ];
        let im = calc
            .calculate_netting_set_with_ngr(&ir_positions(&positions), test_date())
            .expect("valid netting-set IM fixture")
            .expect("NGR computable");

        // Σ MtM = 15M, Σ|MtM| = 15M → NGR = 1.0. Gross = 150M; reduction = 1.0.
        let expected = 150.0e6 * 0.04 * 1.0;
        assert!(
            (im.amount.amount() - expected).abs() < 1e-6,
            "NGR = 1 reduces to gross formula, expected {expected}, got {}",
            im.amount.amount()
        );
    }

    /// Partially-offset book: NGR ∈ (0, 1); the reduction factor
    /// interpolates linearly. For Σ MtM = 5M, Σ positive MtM = 10M → NGR = 1/2.
    #[test]
    fn ngr_partial_offset_interpolates_reduction_factor() {
        let calc = ScheduleImCalculator::bcbs_standard()
            .expect("bcbs_standard loads")
            .with_asset_class(ScheduleAssetClass::InterestRate);

        let positions = [
            (
                Money::new(10.0e6, Currency::USD).expect("valid money fixture"),
                Money::new(100.0e6, Currency::USD).expect("valid money fixture"),
            ),
            (
                Money::new(-5.0e6, Currency::USD).expect("valid money fixture"),
                Money::new(50.0e6, Currency::USD).expect("valid money fixture"),
            ),
        ];
        let im = calc
            .calculate_netting_set_with_ngr(&ir_positions(&positions), test_date())
            .expect("valid netting-set IM fixture")
            .expect("NGR computable");

        let ngr = 5.0 / 10.0; // 1/2
        let reduction = 0.4 + 0.6 * ngr; // 0.7
        let expected = 150.0e6 * 0.04 * reduction;
        assert!(
            (im.amount.amount() - expected).abs() < 1e-6,
            "partial NGR = 1/2 -> reduction 0.7, expected {expected}, got {}",
            im.amount.amount()
        );
    }

    #[test]
    fn ngr_all_negative_book_uses_zero_ngr_floor() {
        let calc = ScheduleImCalculator::bcbs_standard()
            .expect("bcbs_standard loads")
            .with_asset_class(ScheduleAssetClass::InterestRate);

        let positions = [
            (
                Money::new(-10.0e6, Currency::USD).expect("valid money fixture"),
                Money::new(100.0e6, Currency::USD).expect("valid money fixture"),
            ),
            (
                Money::new(-5.0e6, Currency::USD).expect("valid money fixture"),
                Money::new(50.0e6, Currency::USD).expect("valid money fixture"),
            ),
        ];
        let im = calc
            .calculate_netting_set_with_ngr(&ir_positions(&positions), test_date())
            .expect("valid netting-set IM fixture")
            .expect("NGR computable");

        let expected = 150.0e6 * 0.04 * 0.4;
        assert!(
            (im.amount.amount() - expected).abs() < 1e-6,
            "all-negative book should use NGR=0 floor, expected {expected}, got {}",
            im.amount.amount()
        );
    }

    /// Empty or currency-mixed netting sets return None.
    #[test]
    fn ngr_empty_and_mixed_currency_returns_none() {
        let calc = ScheduleImCalculator::bcbs_standard()
            .expect("bcbs_standard loads")
            .with_asset_class(ScheduleAssetClass::InterestRate);

        assert!(calc
            .calculate_netting_set_with_ngr(&[], test_date())
            .expect("valid netting-set IM fixture")
            .is_none());

        let mixed = [
            (
                Money::new(10.0e6, Currency::USD).expect("valid money fixture"),
                Money::new(100.0e6, Currency::USD).expect("valid money fixture"),
            ),
            (
                Money::new(5.0e6, Currency::EUR).expect("valid money fixture"),
                Money::new(50.0e6, Currency::EUR).expect("valid money fixture"),
            ),
        ];
        assert!(calc
            .calculate_netting_set_with_ngr(&ir_positions(&mixed), test_date())
            .is_err());
    }

    #[test]
    fn from_registry_id_matches_bcbs_iosco() {
        let via_named = RegulatorySchedule::from_registry_id(BCBS_IOSCO_SCHEDULE_ID)
            .expect("named schedule should load");
        let via_default = RegulatorySchedule::bcbs_iosco().expect("bcbs_iosco should load");
        assert_eq!(via_named.default_rate, via_default.default_rate);
        assert_eq!(via_named.rates.len(), via_default.rates.len());
    }
}

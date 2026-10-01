//! Deposit instrument types and trait implementations.
//!
//! Defines the `Deposit` instrument with explicit trait implementations
//! mirroring the modern instrument style used elsewhere in valuations.
//! Pricing logic is implemented as instance methods on the instrument struct.
//!
//! # Market Conventions
//!
//! Money-market deposits settle on business days with currency-specific spot lags:
//! - **USD/EUR/JPY**: T+2 settlement (two business days after trade date)
//! - **GBP**: T+0 settlement (same day)
//!
//! The instrument supports optional spot lag and business day convention fields
//! to properly compute settlement dates when building cashflow schedules.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::calendar_by_id;
use finstack_quant_core::dates::{
    adjust, BusinessDayConvention, Date, DateExt, DayCount, HolidayCalendar,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};
use rust_decimal::Decimal;
use time::macros::date;

use crate::impl_instrument_base;
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::market::conventions::ConventionRegistry;
use finstack_quant_core::types::IndexId;

/// Simple deposit instrument with optional quoted rate.
///
/// Represents a single-period deposit where principal is exchanged
/// at start and principal plus interest at maturity.
///
/// # Market Convention Fields
///
/// The instrument supports optional settlement convention fields for proper
/// business-day adjusted cashflow generation:
///
/// - `business_day_convention`: Business day convention for date adjustment (default: ModifiedFollowing)
/// - `calendar_id`: Holiday calendar identifier for business day logic (e.g., "nyse", "target")
///
/// `start_date` is always the accrual start (spot) date. Callers holding a
/// trade date compute the spot date before building (see
/// [`Deposit::from_conventions`]). When `calendar_id` is set, `start_date` and
/// `maturity` are adjusted by the business day convention.
#[derive(
    Clone,
    Debug,
    PartialEq,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Deposit {
    /// Unique identifier for the deposit.
    pub id: InstrumentId,
    /// Principal amount of the deposit.
    pub notional: Money,
    /// Start date of the deposit period.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start_date: Date,
    /// Maturity date of the deposit period.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Day count convention for interest accrual.
    pub day_count: DayCount,

    /// Optional contractual simple rate r (annualised decimal, 0.045 = 4.5%) for the deposit.
    ///
    /// Note: `cashflow_schedule()` requires `fixed_rate` to be set. Leaving it as `None`
    /// is only appropriate if the caller never requests cashflow generation/PV from
    /// this instrument (e.g., constructing placeholders).
    #[builder(optional)]
    #[serde(default, with = "finstack_quant_core::wire::optional_decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DecimalWire>")
    )]
    pub fixed_rate: Option<Decimal>,
    /// Discount curve id used for valuation and par extraction.
    pub discount_curve_id: CurveId,
    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,

    /// Business day convention for date adjustments.
    ///
    /// Used to adjust the effective start/end dates to valid business days.
    /// Default: `ModifiedFollowing` (standard money market convention).
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,

    /// Optional holiday calendar identifier for business day logic.
    ///
    /// Examples: "nyse", "target", "london", "tokyo".
    /// When set, enables calendar-aware spot date and accrual date adjustments.
    #[builder(optional)]
    pub calendar_id: Option<CalendarId>,
}

/// Parameters for building a deposit from registered rate-index conventions.
#[derive(Debug, Clone)]
pub struct ConventionDepositParams<'a> {
    /// Unique deposit identifier.
    pub id: InstrumentId,
    /// Deposit notional.
    pub notional: Money,
    /// Trade date used as the raw start date before spot-lag adjustment.
    pub trade_date: Date,
    /// Deposit maturity date.
    pub maturity: Date,
    /// Contractual simple annualized rate in decimal form (0.045 = 4.5%).
    pub fixed_rate: f64,
    /// Rate index used to resolve market conventions.
    pub index_id: &'a str,
    /// Discount curve used for valuation and par extraction.
    pub discount_curve_id: &'a str,
    /// Scenario-selection and tagging attributes.
    pub attributes: Attributes,
}

impl Deposit {
    /// Create a canonical example deposit for testing and documentation.
    ///
    /// Returns a 6-month USD deposit with 4.5% quoted rate accruing from its
    /// spot date (2024-01-03, T+2 weekdays from a 2024-01-01 trade) with the
    /// ModifiedFollowing business day convention.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new("DEP-USD-6M"))
            .notional(Money::from((100_000_i64, Currency::USD)))
            .start_date(date!(2024 - 01 - 03))
            .maturity(date!(2024 - 07 - 01))
            .day_count(DayCount::Act360)
            .fixed_rate_opt(Decimal::try_from(0.045).ok())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Attributes::new())
            .business_day_convention(BusinessDayConvention::ModifiedFollowing)
            .build()
    }

    /// Create a deposit using market conventions resolved from `ConventionRegistry`.
    ///
    /// This constructor is the preferred shortcut for standard money-market
    /// deposits when the caller knows the trade date, maturity, and quoted rate.
    /// The accrual start date is the spot date: `trade_date` plus the index's
    /// `market_settlement_days` business days on its market calendar.
    ///
    /// # Arguments
    ///
    /// * `params` - Deposit identity, notional, trade date, maturity, decimal
    ///   simple rate (0.045 = 4.5%), rate index id used to resolve conventions,
    ///   discount curve id and attributes.
    ///
    /// # Errors
    ///
    /// Returns an error if the global `ConventionRegistry` is unavailable or if
    /// the requested `index_id` is not present in the registry.
    pub fn from_conventions(
        params: ConventionDepositParams<'_>,
    ) -> finstack_quant_core::Result<Self> {
        let ConventionDepositParams {
            id,
            notional,
            trade_date,
            maturity,
            fixed_rate,
            index_id,
            discount_curve_id,
            attributes,
        } = params;

        let registry = ConventionRegistry::try_global().map_err(|_| {
            finstack_quant_core::Error::Validation("ConventionRegistry not initialized.".into())
        })?;
        let conv = registry.require_rate_index(&IndexId::new(index_id))?;
        let calendar = calendar_by_id(&conv.market_calendar_id).ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "rate index '{index_id}' references unknown market_calendar_id '{}'",
                conv.market_calendar_id
            ))
        })?;
        let spot_date = trade_date.add_business_days(conv.market_settlement_days, calendar)?;

        let deposit = Self::builder()
            .id(id)
            .notional(notional)
            .start_date(spot_date)
            .maturity(maturity)
            .day_count(conv.day_count)
            .fixed_rate_opt(Some(finstack_quant_core::decimal::f64_to_decimal(
                fixed_rate,
            )?))
            .discount_curve_id(CurveId::new(discount_curve_id))
            .attributes(attributes)
            .business_day_convention(conv.market_business_day_convention)
            .calendar_id_opt(Some(conv.market_calendar_id.clone().into()))
            .build()?;

        deposit.validate()?;
        Ok(deposit)
    }

    /// Calculate the raw (unrounded) trade NPV of this deposit.
    ///
    /// Unlike holder-view [`crate::instruments::Instrument::value`], this
    /// includes an initial exchange dated exactly on `as_of`. Truly past
    /// cashflows remain excluded.
    pub fn npv_raw(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::common_impl::helpers::schedule_trade_pv_raw(
            self,
            market,
            as_of,
            &self.discount_curve_id,
        )
    }
}

// Explicit Instrument trait implementation (replaces macro for better IDE visibility)
impl crate::instruments::common_impl::traits::Instrument for Deposit {
    impl_instrument_base!(crate::pricer::InstrumentType::Deposit);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        self.validate()?;
        crate::instruments::common_impl::helpers::schedule_pv(
            self,
            curves,
            as_of,
            &self.discount_curve_id,
        )
    }

    fn base_value_raw(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::common_impl::helpers::schedule_pv_raw(
            self,
            curves,
            as_of,
            &self.discount_curve_id,
        )
    }

    fn base_value_raw_with_currency(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<(f64, finstack_quant_core::currency::Currency)> {
        Ok((
            crate::instruments::common_impl::helpers::schedule_pv_raw(
                self,
                curves,
                as_of,
                &self.discount_curve_id,
            )?,
            self.notional.currency(),
        ))
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        Ok(deps)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        self.effective_end_date().ok()
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        self.effective_start_date().ok()
    }

    crate::impl_focused_pricing_overrides!();
}

/// Minimum reasonable deposit rate (-10% = -1000 bp).
/// Rates below this are likely data errors or misconfigured instruments.
const MIN_REASONABLE_RATE: f64 = -0.10;

/// Maximum reasonable deposit rate (100% = 10000 bp).
/// Rates above this are likely data errors or misconfigured instruments.
const MAX_REASONABLE_RATE: f64 = 1.0;

impl Deposit {
    /// Validate the deposit parameters.
    ///
    /// Checks that:
    /// - End date is after start date (raw dates)
    /// - Effective end date is after effective start date (after BDC adjustments)
    /// - Notional is positive
    /// - Quote rate (if set) is within reasonable bounds (logs warning if not)
    ///
    /// This is called automatically during cashflow generation and pricing.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        // Validate raw date ordering first (fast check)
        validation::validate_date_range_strict_with(
            self.start_date,
            self.maturity,
            |start, maturity| {
                format!(
                    "Deposit maturity date ({}) must be after start date ({})",
                    maturity, start
                )
            },
        )?;

        validation::validate_money_gt_with(self.notional, 0.0, |amount| {
            format!("Deposit notional must be positive, got {}", amount)
        })?;

        // Validate effective date ordering (catches BDC-induced inversions)
        // This is important when spot lag + calendar adjustments could cause issues
        let effective_start = self.effective_start_date()?;
        let effective_end = self.effective_end_date()?;
        validation::validate_date_range_strict_with(
            effective_start,
            effective_end,
            |start, end| {
                format!(
                    "Deposit effective end date ({}) must be after effective start date ({}) \
                 after business day adjustments",
                    end, start
                )
            },
        )?;

        // Warn about extreme rates (don't fail, as they may be intentional)
        if let Some(r) = self.fixed_rate {
            let r_f64 = decimal_to_f64(r, "Deposit fixed_rate")?;
            if validation::rate_outside_range(r_f64, MIN_REASONABLE_RATE, MAX_REASONABLE_RATE) {
                tracing::warn!(
                    deposit_id = %self.id,
                    fixed_rate = r_f64,
                    min_bound = MIN_REASONABLE_RATE,
                    max_bound = MAX_REASONABLE_RATE,
                    "Deposit quote rate {:.4} ({:.0} bp) is outside typical range [{:.0}%, {:.0}%]",
                    r_f64,
                    r_f64 * 10000.0,
                    MIN_REASONABLE_RATE * 100.0,
                    MAX_REASONABLE_RATE * 100.0
                );
            }
        }

        Ok(())
    }

    /// Compute the effective start date: `start_date` adjusted by the business
    /// day convention when `calendar_id` is set, otherwise `start_date` unchanged.
    ///
    /// # Returns
    /// The effective start date after all adjustments.
    pub fn effective_start_date(&self) -> finstack_quant_core::Result<Date> {
        let calendar: Option<&dyn HolidayCalendar> = match self.calendar_id.as_deref() {
            Some(id) => Some(calendar_by_id(id).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "Deposit '{}' references unknown calendar_id '{}'",
                    self.id, id
                ))
            })?),
            None => None,
        };

        let business_day_convention = self.business_day_convention;

        if let Some(cal) = calendar {
            adjust(self.start_date, business_day_convention, cal)
        } else {
            Ok(self.start_date)
        }
    }

    /// Compute the effective end date considering business day adjustments.
    ///
    /// The end date is adjusted using the business day convention and calendar if set.
    ///
    /// # Convention note
    ///
    /// Maturity is determined from the agreed term after spot settlement, then
    /// adjusted by BDC/calendar (common money-market convention).
    ///
    /// # Returns
    /// The effective end date after all adjustments.
    pub fn effective_end_date(&self) -> finstack_quant_core::Result<Date> {
        let calendar: Option<&dyn HolidayCalendar> = match self.calendar_id.as_deref() {
            Some(id) => Some(calendar_by_id(id).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "Deposit '{}' references unknown calendar_id '{}'",
                    self.id, id
                ))
            })?),
            None => None,
        };

        let business_day_convention = self.business_day_convention;

        // Apply business day adjustment if calendar is available
        if let Some(cal) = calendar {
            adjust(self.maturity, business_day_convention, cal)
        } else {
            Ok(self.maturity)
        }
    }
}

impl finstack_quant_cashflows::CashflowScheduleSource for Deposit {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(self.notional))
    }

    fn raw_cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        // Validate deposit parameters before building schedule
        // (includes effective date ordering check)
        self.validate()?;

        // Effective dates: start/end optionally BDC-adjusted.
        let effective_start = self.effective_start_date()?;
        let effective_end = self.effective_end_date()?;

        // Classify principal and simple interest separately at maturity.
        // Use effective dates for proper accrual calculation.
        let yf = self.day_count.year_fraction(
            effective_start,
            effective_end,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;

        let r = self.fixed_rate.ok_or_else(|| {
            finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                id: "deposit fixed_rate".to_string(),
            })
        })?;
        let r = decimal_to_f64(r, "Deposit fixed_rate")?;
        let redemption = self.notional * (1.0 + r * yf);
        let interest = redemption.checked_sub(self.notional)?;
        let flows = vec![
            crate::cashflow::primitives::CashFlow::new(
                effective_start,
                None,
                self.notional * -1.0,
                crate::cashflow::primitives::CFKind::Notional,
                0.0,
                None,
            ),
            crate::cashflow::primitives::CashFlow::new(
                effective_end,
                None,
                self.notional,
                crate::cashflow::primitives::CFKind::Notional,
                0.0,
                None,
            ),
            crate::cashflow::primitives::CashFlow::new(
                effective_end,
                None,
                interest,
                crate::cashflow::primitives::CFKind::Fixed,
                yf,
                Some(r),
            ),
        ];

        let schedule = crate::cashflow::traits::schedule_from_classified_flows(
            flows,
            self.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional()?,
                ..Default::default()
            },
        );
        Ok(schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Contractual))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::traits::CashflowProvider;
    use crate::instruments::common_impl::traits::Attributes;
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::currency::Currency;
    use rust_decimal::prelude::ToPrimitive;
    use time::macros::date;

    #[test]
    fn from_conventions_applies_rate_index_defaults() {
        let deposit = Deposit::from_conventions(ConventionDepositParams {
            id: InstrumentId::new("DEP-USD-SOFR-6M"),
            notional: Money::from((1_000_000_i64, Currency::USD)),
            trade_date: date!(2025 - 01 - 02),
            maturity: date!(2025 - 07 - 02),
            fixed_rate: 0.045,
            index_id: "USD-SOFR-OIS",
            discount_curve_id: "USD-OIS",
            attributes: Attributes::new(),
        })
        .expect("deposit conventions constructor should succeed");

        assert_eq!(deposit.id, InstrumentId::new("DEP-USD-SOFR-6M"));
        assert_eq!(
            deposit.notional,
            Money::from((1_000_000_i64, Currency::USD))
        );
        // Spot date: 2025-01-02 (Thu) + 2 USNY business days = 2025-01-06 (Mon).
        assert_eq!(deposit.start_date, date!(2025 - 01 - 06));
        assert_eq!(deposit.maturity, date!(2025 - 07 - 02));
        assert_eq!(deposit.day_count, DayCount::Act360);
        assert_eq!(
            deposit.fixed_rate.and_then(|rate| rate.to_f64()),
            Some(0.045)
        );
        assert_eq!(deposit.discount_curve_id, CurveId::new("USD-OIS"));
        assert_eq!(
            deposit.business_day_convention,
            BusinessDayConvention::ModifiedFollowing
        );
        assert_eq!(deposit.calendar_id.as_deref(), Some("usny"));
    }

    #[test]
    fn cashflow_schedule_marks_initial_exchange_as_notional() {
        let deposit = Deposit::builder()
            .id(InstrumentId::new("DEP-KIND"))
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .start_date(date!(2025 - 01 - 02))
            .maturity(date!(2025 - 07 - 02))
            .fixed_rate_opt(Decimal::try_from(0.045).ok())
            .day_count(DayCount::Act360)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Attributes::new())
            .build()
            .expect("deposit should build");

        let schedule = deposit
            .cashflow_schedule(&MarketContext::new(), date!(2025 - 01 - 01))
            .expect("deposit full schedule");

        assert_eq!(schedule.get_flows().len(), 3);
        assert_eq!(schedule.get_flows()[0].kind, CFKind::Notional);
        assert_eq!(schedule.get_flows()[1].kind, CFKind::Fixed);
        assert_eq!(schedule.get_flows()[2].kind, CFKind::Notional);
        assert_eq!(schedule.get_flows()[2].amount, deposit.notional);
        let yf = DayCount::Act360
            .year_fraction(
                deposit.start_date,
                deposit.maturity,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .expect("deposit accrual");
        let redemption = schedule.get_flows()[1]
            .amount
            .checked_add(schedule.get_flows()[2].amount)
            .expect("maturity cash");
        assert_eq!(redemption, deposit.notional * (1.0 + 0.045 * yf));
    }

    #[test]
    fn settled_deposit_prices_are_zero_across_money_and_raw_routes() {
        use crate::instruments::{Instrument, PricingOptions};
        use finstack_quant_core::market_data::term_structures::DiscountCurve;

        let maturity = date!(2025 - 01 - 15);
        let after_maturity = date!(2025 - 01 - 16);
        let market = MarketContext::new().insert(
            DiscountCurve::builder("DISC")
                .base_date(date!(2025 - 01 - 06))
                .knots([(0.0, 1.0), (1.0, 0.95)])
                .build()
                .expect("discount curve"),
        );
        for currency in [Currency::USD, Currency::EUR] {
            let deposit = Deposit::builder()
                .id("SETTLED-DEPOSIT".into())
                .notional(Money::from((1_000_000_i64, currency)))
                .start_date(date!(2025 - 01 - 06))
                .maturity(maturity)
                .fixed_rate_opt(Some(Decimal::try_from(0.05).expect("rate")))
                .day_count(DayCount::Act360)
                .discount_curve_id("DISC".into())
                .build()
                .expect("deposit");
            assert!(
                deposit
                    .value(&market, date!(2025 - 01 - 14))
                    .expect("live value")
                    .amount()
                    > 0.0
            );
            for as_of in [maturity, after_maturity] {
                assert_eq!(
                    deposit.value(&market, as_of).expect("settled value"),
                    Money::from((0_i64, currency))
                );
                assert_eq!(
                    deposit
                        .value_raw(&market, as_of)
                        .expect("settled raw value"),
                    0.0
                );
                assert_eq!(
                    deposit
                        .value_raw_with_currency(&market, as_of)
                        .expect("settled raw currency"),
                    (0.0, currency)
                );
                assert_eq!(
                    deposit
                        .base_value_raw_with_currency(&market, as_of)
                        .expect("settled raw kernel"),
                    (0.0, currency)
                );
                assert_eq!(
                    deposit
                        .price_with_metrics(&market, as_of, &[], PricingOptions::default())
                        .expect("settled metric valuation")
                        .value,
                    Money::from((0_i64, currency))
                );
            }

            let mut invalid = deposit.clone();
            invalid.maturity = invalid.start_date;
            assert!(invalid.value(&market, after_maturity).is_err());
            assert!(invalid.value_raw(&market, after_maturity).is_err());
            assert!(invalid
                .base_value_raw_with_currency(&market, after_maturity)
                .is_err());
            let mut missing_rate = deposit;
            missing_rate.fixed_rate = None;
            assert!(missing_rate.value(&market, after_maturity).is_err());
            assert!(missing_rate.value_raw(&market, after_maturity).is_err());
            assert!(missing_rate
                .base_value_raw_with_currency(&market, after_maturity)
                .is_err());
        }
    }
}

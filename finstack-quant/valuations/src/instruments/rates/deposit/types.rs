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
use finstack_quant_core::dates::CalendarRegistry;
use finstack_quant_core::dates::{
    adjust, BusinessDayConvention, Date, DateExt, DayCount, HolidayCalendar,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId, Rate};
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
/// - `spot_lag_days`: Number of business days from trade date to spot date (default: 2 for USD/EUR/JPY, 0 for GBP)
/// - `bdc`: Business day convention for date adjustment (default: ModifiedFollowing)
/// - `calendar_id`: Holiday calendar identifier for business day logic (e.g., "nyse", "target")
///
/// When these fields are set, the effective start date is computed as
/// `start + spot_lag` adjusted by the business day convention. In this case,
/// `start` is treated as the trade date; otherwise it is the accrual start date.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    finstack_quant_valuations_macros::FocusedPricingOverrides,
)]
#[serde(deny_unknown_fields)]
pub struct Deposit {
    /// Unique identifier for the deposit.
    pub id: InstrumentId,
    /// Principal amount of the deposit.
    pub notional: Money,
    /// Start date of the deposit period.
    #[schemars(with = "String")]
    pub start_date: Date,
    /// Maturity date of the deposit period.
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Day count convention for interest accrual.
    pub day_count: DayCount,

    /// Optional quoted simple rate r (annualised) for the deposit.
    ///
    /// Note: `cashflow_schedule()` requires `quote_rate` to be set. Leaving it as `None`
    /// is only appropriate if the caller never requests cashflow generation/PV from
    /// this instrument (e.g., constructing placeholders).
    #[builder(optional)]
    pub quote_rate: Option<Decimal>,
    /// Discount curve id used for valuation and par extraction.
    pub discount_curve_id: CurveId,
    /// Attributes for scenario selection and tagging.
    #[serde(default)]
    #[builder(default)]
    /// Instrument-owned pricing inputs.
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(default)]
    #[builder(default)]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(default)]
    #[builder(default)]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,

    /// Optional spot lag in business days from trade date to effective start.
    ///
    /// Market convention: T+2 for USD/EUR/JPY, T+0 for GBP.
    /// If not set, the raw `start` date is used without adjustment.
    #[builder(optional)]
    pub spot_lag_days: Option<i32>,

    /// Business day convention for date adjustments.
    ///
    /// Used to adjust the effective start/end dates to valid business days.
    /// Default: `ModifiedFollowing` (standard money market convention).
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub bdc: BusinessDayConvention,

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
    /// Quoted simple annualized rate in decimal form.
    pub quote_rate: f64,
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
    /// Returns a 6-month USD deposit with 4.5% quoted rate and standard
    /// T+2 spot settlement with ModifiedFollowing business day convention.
    pub fn example() -> finstack_quant_core::Result<Self> {
        // SAFETY: All inputs are compile-time validated constants
        Self::builder()
            .id(InstrumentId::new("DEP-USD-6M"))
            .notional(Money::new(100_000.0, Currency::USD))
            .start_date(date!(2024 - 01 - 01))
            .maturity(date!(2024 - 07 - 01))
            .day_count(DayCount::Act360)
            .quote_rate_opt(Decimal::try_from(0.045).ok())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Attributes::new())
            .spot_lag_days_opt(Some(2))
            .bdc(BusinessDayConvention::ModifiedFollowing)
            .build()
    }

    /// Create a deposit using market conventions resolved from `ConventionRegistry`.
    ///
    /// This constructor is the preferred shortcut for standard money-market
    /// deposits when the caller knows the trade date, maturity, and quoted rate.
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
            quote_rate,
            index_id,
            discount_curve_id,
            attributes,
        } = params;

        let registry = ConventionRegistry::try_global().map_err(|_| {
            finstack_quant_core::Error::Validation("ConventionRegistry not initialized.".into())
        })?;
        let conv = registry.require_rate_index(&IndexId::new(index_id))?;

        let deposit = Self::builder()
            .id(id)
            .notional(notional)
            .start_date(trade_date)
            .maturity(maturity)
            .day_count(conv.day_count)
            .quote_rate_opt(Some(finstack_quant_core::decimal::f64_to_decimal(
                quote_rate,
            )?))
            .discount_curve_id(CurveId::new(discount_curve_id))
            .attributes(attributes)
            .spot_lag_days_opt(Some(conv.market_settlement_days))
            .bdc(conv.market_business_day_convention)
            .calendar_id_opt(Some(conv.market_calendar_id.clone().into()))
            .build()?;

        deposit.validate()?;
        Ok(deposit)
    }

    /// Calculate the raw (unrounded) net present value of this deposit.
    pub fn npv_raw(
        &self,
        context: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::common_impl::helpers::schedule_pv_raw(
            self,
            context,
            as_of,
            &self.discount_curve_id,
        )
    }
}

impl DepositBuilder {
    /// Set the quoted rate using a typed rate.
    pub fn quote_rate_rate(mut self, rate: Rate) -> Self {
        self.quote_rate = Decimal::try_from(rate.as_decimal()).ok();
        self
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

    fn value_raw(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        self.npv_raw(curves, as_of)
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

/// Minimum reasonable deposit rate (-10% = -1000 bps).
/// Rates below this are likely data errors or misconfigured instruments.
const MIN_REASONABLE_RATE: f64 = -0.10;

/// Maximum reasonable deposit rate (100% = 10000 bps).
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

        // Validate positive notional
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

        // Validate non-negative spot lag (negative has no financial meaning)
        if let Some(lag) = self.spot_lag_days {
            if lag < 0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Deposit spot_lag_days must be non-negative, got {}",
                    lag
                )));
            }
        }

        // Warn about extreme rates (don't fail, as they may be intentional)
        if let Some(r) = self.quote_rate {
            let r_f64 = decimal_to_f64(r, "Deposit quote_rate")?;
            if validation::rate_outside_range(r_f64, MIN_REASONABLE_RATE, MAX_REASONABLE_RATE) {
                tracing::warn!(
                    deposit_id = %self.id,
                    quote_rate = r_f64,
                    min_bound = MIN_REASONABLE_RATE,
                    max_bound = MAX_REASONABLE_RATE,
                    "Deposit quote rate {:.4} ({:.0} bps) is outside typical range [{:.0}%, {:.0}%]",
                    r_f64,
                    r_f64 * 10000.0,
                    MIN_REASONABLE_RATE * 100.0,
                    MAX_REASONABLE_RATE * 100.0
                );
            }
        }

        Ok(())
    }

    /// Compute the effective start date considering spot lag and business day adjustments.
    ///
    /// If `spot_lag_days` is set, computes the start date as `start + spot_lag` business days
    /// (or calendar days if no calendar is set), then applies the business day convention.
    ///
    /// If `spot_lag_days` is not set, returns the raw `start` date optionally adjusted by BDC.
    ///
    /// # Returns
    /// The effective start date after all adjustments.
    pub fn effective_start_date(&self) -> finstack_quant_core::Result<Date> {
        let calendar: Option<&dyn HolidayCalendar> = match self.calendar_id.as_deref() {
            Some(id) => Some(CalendarRegistry::global().resolve_str(id).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "Deposit '{}' references unknown calendar_id '{}'",
                    self.id, id
                ))
            })?),
            None => None,
        };

        let bdc = self.bdc;

        let base_start = if let Some(lag_days) = self.spot_lag_days {
            // Compute spot date: start + spot_lag business days
            if let Some(cal) = calendar {
                self.start_date.add_business_days(lag_days, cal)?
            } else {
                self.start_date.add_weekdays(lag_days)
            }
        } else {
            // Use raw start date
            self.start_date
        };

        // Apply business day adjustment if calendar is available
        if let Some(cal) = calendar {
            adjust(base_start, bdc, cal)
        } else {
            Ok(base_start)
        }
    }

    /// Compute the effective end date considering business day adjustments.
    ///
    /// The end date is adjusted using the business day convention and calendar if set.
    ///
    /// # Convention note
    ///
    /// Spot lag is only applied to the start leg (`effective_start_date`), not to this
    /// end leg. This follows common money-market convention where maturity is determined
    /// from the agreed term after spot settlement, then adjusted by BDC/calendar.
    ///
    /// # Returns
    /// The effective end date after all adjustments.
    pub fn effective_end_date(&self) -> finstack_quant_core::Result<Date> {
        let calendar: Option<&dyn HolidayCalendar> = match self.calendar_id.as_deref() {
            Some(id) => Some(CalendarRegistry::global().resolve_str(id).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "Deposit '{}' references unknown calendar_id '{}'",
                    self.id, id
                ))
            })?),
            None => None,
        };

        let bdc = self.bdc;

        // Apply business day adjustment if calendar is available
        if let Some(cal) = calendar {
            adjust(self.maturity, bdc, cal)
        } else {
            Ok(self.maturity)
        }
    }
}

impl finstack_quant_cashflows::CashflowScheduleSource for Deposit {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn raw_cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        // Validate deposit parameters before building schedule
        // (includes effective date ordering check)
        self.validate()?;

        // Compute effective dates with spot lag and business day adjustments.
        // When spot_lag_days is set, compute effective start from trade date (start).
        // Otherwise, use the raw start/end dates (optionally BDC-adjusted).
        let effective_start = self.effective_start_date()?;
        let effective_end = self.effective_end_date()?;

        // True single-period deposit: two flows with simple interest
        // Use effective dates for proper accrual calculation
        let yf = self.day_count.year_fraction(
            effective_start,
            effective_end,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;

        let r = self.quote_rate.ok_or_else(|| {
            finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                id: "deposit quote_rate".to_string(),
            })
        })?;
        let r = decimal_to_f64(r, "Deposit quote_rate")?;
        let redemption = self.notional * (1.0 + r * yf);
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
                redemption,
                crate::cashflow::primitives::CFKind::Fixed,
                yf,
                Some(r),
            ),
        ];

        let schedule = crate::cashflow::traits::schedule_from_classified_flows(
            flows,
            self.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional(),
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
            notional: Money::new(1_000_000.0, Currency::USD),
            trade_date: date!(2025 - 01 - 02),
            maturity: date!(2025 - 07 - 02),
            quote_rate: 0.045,
            index_id: "USD-SOFR-OIS",
            discount_curve_id: "USD-OIS",
            attributes: Attributes::new(),
        })
        .expect("deposit conventions constructor should succeed");

        assert_eq!(deposit.id, InstrumentId::new("DEP-USD-SOFR-6M"));
        assert_eq!(deposit.notional, Money::new(1_000_000.0, Currency::USD));
        assert_eq!(deposit.start_date, date!(2025 - 01 - 02));
        assert_eq!(deposit.maturity, date!(2025 - 07 - 02));
        assert_eq!(deposit.day_count, DayCount::Act360);
        assert_eq!(
            deposit.quote_rate.and_then(|rate| rate.to_f64()),
            Some(0.045)
        );
        assert_eq!(deposit.discount_curve_id, CurveId::new("USD-OIS"));
        assert_eq!(deposit.spot_lag_days, Some(2));
        assert_eq!(deposit.bdc, BusinessDayConvention::ModifiedFollowing);
        assert_eq!(deposit.calendar_id.as_deref(), Some("usny"));
    }

    #[test]
    fn cashflow_schedule_marks_initial_exchange_as_notional() {
        let deposit = Deposit::builder()
            .id(InstrumentId::new("DEP-KIND"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(date!(2025 - 01 - 02))
            .maturity(date!(2025 - 07 - 02))
            .quote_rate_rate(finstack_quant_core::types::Rate::from_decimal(0.045))
            .day_count(DayCount::Act360)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Attributes::new())
            .build()
            .expect("deposit should build");

        let schedule = deposit
            .cashflow_schedule(&MarketContext::new(), date!(2025 - 01 - 01))
            .expect("deposit full schedule");

        assert_eq!(schedule.get_flows().len(), 2);
        assert_eq!(schedule.get_flows()[0].kind, CFKind::Notional);
    }
}

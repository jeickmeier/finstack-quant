//! Forward Rate Agreement (FRA) instrument types and trait implementations.
//!
//! Defines the `ForwardRateAgreement` instrument in the modern instrument style
//! used across valuations. Core PV logic is delegated to the pricing engine in
//! `pricing::engine`, and metrics are provided in the `metrics` submodule.

use crate::cashflow::traits::CashflowProvider;
use crate::impl_instrument_base;
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::common_impl::parameters::legs::PayReceive;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::market::conventions::ConventionRegistry;
use finstack_core::currency::Currency;
use finstack_core::dates::{adjust, BusinessDayConvention, CalendarRegistry, Date, DayCount};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::IndexId;
use finstack_core::types::{CalendarId, CurveId, InstrumentId, Rate};
use rust_decimal::Decimal;
use time::macros::date;

// =============================================================================
// Constants
// =============================================================================

/// Minimum denominator for settlement adjustment to avoid division issues.
/// When 1 + F × τ is below this threshold, the forward rate is considered invalid.
const MIN_SETTLEMENT_DENOM: f64 = 1e-12;

/// Minimum period length (in year fractions) for a valid FRA.
const MIN_PERIOD_LENGTH: f64 = 1e-12;

/// Minimum reasonable forward rate for validation warnings (-10%).
const MIN_REASONABLE_RATE: f64 = -0.10;

/// Maximum reasonable forward rate for validation warnings (50%).
const MAX_REASONABLE_RATE: f64 = 0.50;

/// Forward Rate Agreement instrument.
///
/// A FRA is a forward contract on an interest rate. The holder receives
/// the difference between the realized rate and the fixed rate, paid at
/// the start of the interest period (FRA convention).
///
/// # Direction Convention
///
/// - `side = PayReceive::Receive`: Receive fixed rate, pay floating rate.
///   When forward rate > fixed rate, PV is negative (you're paying more than receiving).
/// - `side = PayReceive::Pay`: Pay fixed rate, receive floating rate.
///   When forward rate > fixed rate, PV is positive (you're receiving more than paying).
///
/// # Side field
///
/// Use `side` to indicate the fixed leg direction. If omitted in JSON,
/// deserialization defaults to `Pay`.
#[derive(
    Debug,
    Clone,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ForwardRateAgreement {
    /// Unique identifier
    pub id: InstrumentId,
    /// Notional amount
    pub notional: Money,
    /// Rate fixing date. If `None`, inferred from `start_date - reset_lag` business days.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub fixing_date: Option<Date>,
    /// Interest period start date
    #[schemars(with = "String")]
    pub start_date: Date,
    /// Interest period end date
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Fixed rate (decimal, e.g., 0.05 for 5%)
    pub fixed_rate: Decimal,
    /// Day count convention for interest accrual
    pub day_count: DayCount,
    /// Reset lag in business days (fixing to value date)
    pub reset_lag: i32,
    /// Optional fixing calendar identifier for business day adjustment
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixing_calendar_id: Option<CalendarId>,
    /// Optional business day convention for fixing date adjustment (default: ModifiedFollowing)
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixing_bdc: Option<BusinessDayConvention>,
    /// Optional observed fixing (locked rate) when known
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_fixing: Option<f64>,
    /// Discount curve identifier
    pub discount_curve_id: CurveId,
    /// Forward curve identifier
    pub forward_curve_id: CurveId,
    /// Direction of the FRA: Pay means paying the fixed rate (receiving floating),
    /// Receive means receiving the fixed rate (paying floating).
    pub side: PayReceive,
    /// Attributes for scenario selection
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

/// Parameters for building an FRA from registered rate-index conventions.
#[derive(Debug, Clone)]
pub struct ConventionFraParams<'a> {
    /// Unique FRA identifier.
    pub id: InstrumentId,
    /// FRA notional.
    pub notional: Money,
    /// Start date of the accrual period.
    pub start_date: Date,
    /// End date of the accrual period.
    pub maturity: Date,
    /// Fixed FRA rate in decimal form.
    pub fixed_rate: f64,
    /// Rate index used to resolve FRA conventions.
    pub index_id: &'a str,
    /// Discount curve used for settlement discounting.
    pub discount_curve_id: &'a str,
    /// Forward curve used to project the realized floating rate.
    pub forward_curve_id: &'a str,
    /// FRA direction for the fixed leg.
    pub side: PayReceive,
    /// Scenario-selection and tagging attributes.
    pub attributes: Attributes,
}

/// Custom deserializer for ForwardRateAgreement that accepts `side`
/// (PayReceive enum), defaulting to `Pay` when omitted.
impl<'de> serde::Deserialize<'de> for ForwardRateAgreement {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        /// Helper struct that mirrors the JSON structure for deserialization.
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct FraHelper {
            id: InstrumentId,
            notional: Money,
            #[serde(default)]
            fixing_date: Option<Date>,
            start_date: Date,
            maturity: Date,
            fixed_rate: Decimal,
            day_count: DayCount,
            reset_lag: i32,
            #[serde(default)]
            fixing_calendar_id: Option<CalendarId>,
            #[serde(default)]
            fixing_bdc: Option<BusinessDayConvention>,
            #[serde(default)]
            observed_fixing: Option<f64>,
            discount_curve_id: CurveId,
            forward_curve_id: CurveId,
            #[serde(default)]
            side: Option<PayReceive>,
            #[serde(default)]
            pricing_overrides: crate::instruments::PricingOverrides,
            attributes: Attributes,
        }

        let helper = FraHelper::deserialize(deserializer)?;

        let side = helper.side.unwrap_or(PayReceive::Pay);

        Ok(ForwardRateAgreement {
            id: helper.id,
            notional: helper.notional,
            fixing_date: helper.fixing_date,
            start_date: helper.start_date,
            maturity: helper.maturity,
            fixed_rate: helper.fixed_rate,
            day_count: helper.day_count,
            reset_lag: helper.reset_lag,
            fixing_calendar_id: helper.fixing_calendar_id,
            fixing_bdc: helper.fixing_bdc,
            observed_fixing: helper.observed_fixing,
            discount_curve_id: helper.discount_curve_id,
            forward_curve_id: helper.forward_curve_id,
            side,
            pricing_overrides: helper.pricing_overrides,
            attributes: helper.attributes,
        })
    }
}

impl ForwardRateAgreement {
    /// Validate structural invariants of the FRA.
    ///
    /// This does not encode market conventions; it enforces finiteness and
    /// basic ordering constraints to prevent ambiguous pricing.
    pub fn validate(&self) -> finstack_core::Result<()> {
        validation::validate_date_range_non_strict(self.start_date, self.maturity, "FRA")?;

        validation::validate_money_finite(self.notional, "FRA notional")?;
        validation::validate_money_gt_with(self.notional, 0.0, |amount| {
            format!("FRA notional must be positive, got {}", amount)
        })?;

        let _ = decimal_to_f64(self.fixed_rate, "FRA fixed_rate")?;

        if let Some(observed) = self.observed_fixing {
            validation::validate_f64_finite(observed, "FRA observed_fixing")?;
        }

        // Guard against unit mistakes (e.g., passing years as days).
        validation::require_with(self.reset_lag.abs() <= 31, || {
            "FRA reset_lag has an unexpectedly large magnitude (expected business days)".to_string()
        })?;

        if let Some(fixing_date) = self.fixing_date {
            validation::require_with(fixing_date <= self.start_date, || {
                format!(
                    "FRA fixing_date ({}) must be on or before start_date ({})",
                    fixing_date, self.start_date
                )
            })?;
        }

        Ok(())
    }

    fn resolved_fixing_calendar_id(&self) -> Option<CalendarId> {
        if let Some(id) = &self.fixing_calendar_id {
            return Some(id.clone());
        }
        let Ok(registry) = ConventionRegistry::try_global() else {
            return None;
        };
        let index_id = IndexId::new(self.forward_curve_id.as_str());
        registry
            .require_rate_index(&index_id)
            .ok()
            .map(|conv| conv.market_calendar_id.clone().into())
    }

    /// Create a canonical example FRA for testing and documentation.
    ///
    /// Returns a 3x6 FRA (3 months forward, 3 month tenor).
    pub fn example() -> finstack_core::Result<Self> {
        // SAFETY: All inputs are compile-time validated constants
        Self::builder()
            .id(InstrumentId::new("FRA-3X6-USD"))
            .notional(Money::new(10_000_000.0, Currency::USD))
            .fixing_date(date!(2024 - 04 - 01))
            .start_date(date!(2024 - 04 - 03))
            .maturity(date!(2024 - 07 - 03))
            .fixed_rate(finstack_core::decimal::f64_to_decimal(0.045)?)
            .day_count(DayCount::Act360)
            .reset_lag(2)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-SOFR-3M"))
            .side(PayReceive::Receive)
            .attributes(Attributes::new())
            .build()
    }

    /// Create an FRA using market conventions resolved from `ConventionRegistry`.
    ///
    /// This is the concise path for standard forward-rate agreements when the
    /// caller knows the accrual dates, fixed rate, and curve identifiers.
    ///
    /// # Errors
    ///
    /// Returns an error if the global `ConventionRegistry` is unavailable or if
    /// the requested `index_id` is not present in the registry.
    pub fn from_conventions(params: ConventionFraParams<'_>) -> finstack_core::Result<Self> {
        let ConventionFraParams {
            id,
            notional,
            start_date,
            maturity,
            fixed_rate,
            index_id,
            discount_curve_id,
            forward_curve_id,
            side,
            attributes,
        } = params;

        let registry = ConventionRegistry::try_global().map_err(|_| {
            finstack_core::Error::Validation("ConventionRegistry not initialized.".into())
        })?;
        let conv = registry.require_rate_index(&IndexId::new(index_id))?;

        let fra = Self::builder()
            .id(id)
            .notional(notional)
            .start_date(start_date)
            .maturity(maturity)
            .fixed_rate(finstack_core::decimal::f64_to_decimal(fixed_rate)?)
            .day_count(conv.day_count)
            .reset_lag(conv.default_reset_lag_days)
            .discount_curve_id(CurveId::new(discount_curve_id))
            .forward_curve_id(CurveId::new(forward_curve_id))
            .side(side)
            .fixing_calendar_id_opt(Some(conv.market_calendar_id.clone().into()))
            .fixing_bdc_opt(Some(conv.market_business_day_convention))
            .attributes(attributes)
            .build()?;

        fra.validate()?;
        Ok(fra)
    }

    /// Settlement amount at period start (undiscounted).
    ///
    /// Returns the cashflow paid at `start_date` using standard FRA settlement
    /// convention: N × τ × (F - K) / (1 + F × τ), signed by direction.
    ///
    /// # Errors
    ///
    /// - Returns error if fixing date has passed but no `observed_fixing` is provided
    /// - Returns error if settlement denominator is pathological (forward rate ≈ -1/τ)
    fn settlement_amount_raw(
        &self,
        context: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<f64> {
        use finstack_core::dates::DateExt;

        self.validate()?;

        if as_of >= self.start_date {
            return Ok(0.0);
        }

        // Determine fixing date: use explicit fixing_date if provided,
        // otherwise infer from start_date - reset_lag business days.
        //
        // IMPORTANT: reset_lag is in BUSINESS DAYS, not calendar days.
        let fixing_date = match self.fixing_date {
            Some(explicit_date) => explicit_date,
            None => {
                let bdc = self
                    .fixing_bdc
                    .unwrap_or(BusinessDayConvention::ModifiedFollowing);
                let resolved_cal_id = self.resolved_fixing_calendar_id();

                // Compute base fixing date by subtracting reset_lag business days
                let base_fixing_date = if let Some(cal_id) = resolved_cal_id.as_deref() {
                    if let Some(cal) = CalendarRegistry::global().resolve_str(cal_id) {
                        // Use calendar-aware business day subtraction
                        self.start_date.add_business_days(-(self.reset_lag), cal)?
                    } else {
                        return Err(finstack_core::Error::Validation(format!(
                            "FRA '{}': fixing calendar '{}' could not be resolved",
                            self.id, cal_id
                        )));
                    }
                } else {
                    return Err(finstack_core::Error::Validation(format!(
                        "FRA '{}': fixing calendar is required to apply business-day reset lag",
                        self.id
                    )));
                };

                // Apply business day convention adjustment to the resulting date
                if let Some(cal_id) = resolved_cal_id.as_deref() {
                    if let Some(cal) = CalendarRegistry::global().resolve_str(cal_id) {
                        adjust(base_fixing_date, bdc, cal)?
                    } else {
                        base_fixing_date
                    }
                } else {
                    base_fixing_date
                }
            }
        };

        let fwd = context.get_forward(&self.forward_curve_id)?;

        // Time fractions for mapping into the forward curve domain must use the
        // forward curve's own day-count/time basis, not the instrument accrual basis.
        let fwd_base = fwd.base_date();
        let fwd_dc = fwd.day_count();
        let t_start = fwd_dc
            .year_fraction(
                fwd_base,
                self.start_date,
                finstack_core::dates::DayCountContext::default(),
            )?
            .max(0.0);
        let t_end = fwd_dc
            .year_fraction(
                fwd_base,
                self.maturity,
                finstack_core::dates::DayCountContext::default(),
            )?
            .max(t_start);

        // Accrual factor
        let tau = self
            .day_count
            .year_fraction(
                self.start_date,
                self.maturity,
                finstack_core::dates::DayCountContext::default(),
            )?
            .max(0.0);

        // Zero-length period produces zero settlement
        if tau < MIN_PERIOD_LENGTH {
            return Ok(0.0);
        }

        // Forward rate over the period
        // If fixing date has passed, require observed fixing to avoid ambiguity
        let forward_rate = if as_of >= fixing_date {
            self.observed_fixing.ok_or_else(|| {
                finstack_core::Error::Validation(format!(
                    "FRA '{}': fixing date {} has passed (as_of={}) but no observed_fixing provided",
                    self.id, fixing_date, as_of
                ))
            })?
        } else {
            fwd.rate_period(t_start, t_end)
        };

        // Warn if forward rate is outside reasonable bounds (likely data error)
        if validation::rate_outside_range(forward_rate, MIN_REASONABLE_RATE, MAX_REASONABLE_RATE) {
            tracing::warn!(
                instrument_id = %self.id.as_str(),
                forward_rate,
                min_bound = MIN_REASONABLE_RATE,
                max_bound = MAX_REASONABLE_RATE,
                "FRA forward rate outside typical bounds; possible market data error"
            );
        }

        // Market-standard FRA settlement at period start includes the
        // settlement discounting adjustment 1 / (1 + F × τ).
        let fixed_rate = decimal_to_f64(self.fixed_rate, "FRA fixed_rate")?;
        let rate_diff = forward_rate - fixed_rate;
        let denom = 1.0_f64 + forward_rate * tau;

        // Denominator near zero indicates pathological forward rate (F ≈ -1/τ)
        if denom.abs() <= MIN_SETTLEMENT_DENOM {
            return Err(finstack_core::Error::Validation(format!(
                "FRA '{}': settlement denominator near zero (forward_rate={}, tau={}); \
                 forward rate implies F ≈ {:.2}% which is pathological",
                self.id,
                forward_rate,
                tau,
                -100.0 / tau
            )));
        }

        let settlement = self.notional.amount() * rate_diff * tau / denom;

        // Apply direction: Receive means we receive K and pay F
        // When F > K: rate_diff > 0, settlement > 0 (we owe money)
        // So negate when Receive
        Ok(match self.side {
            PayReceive::Receive => -settlement,
            PayReceive::Pay => settlement,
        })
    }

    /// Calculate the raw net present value of this FRA (unrounded f64)
    ///
    /// # Reset Lag Handling
    ///
    /// The fixing date is inferred from `start_date - reset_lag` using **business days**
    /// when a calendar is available, or weekday-only subtraction otherwise. This aligns
    /// with market conventions where reset lag is specified in business days (e.g., T-2).
    ///
    /// The inferred date is then adjusted according to `fixing_bdc` (defaults to ModifiedFollowing).
    pub fn npv_raw(
        &self,
        context: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<f64> {
        self.validate()?;
        let disc = context.get_discount(&self.discount_curve_id)?;
        let flows = self.dated_cashflows(context, as_of)?;

        if flows.is_empty() {
            return Ok(0.0);
        }

        flows.into_iter().try_fold(0.0, |acc, (date, amount)| {
            let df = disc.df_between_dates(as_of, date)?;
            Ok(acc + amount.amount() * df)
        })
    }
}

impl ForwardRateAgreementBuilder {
    /// Set the fixed rate using a typed rate.
    pub fn fixed_rate_rate(mut self, rate: Rate) -> Self {
        self.fixed_rate = Decimal::try_from(rate.as_decimal()).ok();
        self
    }

    /// Set the observed fixing using a typed rate.
    pub fn observed_fixing_rate(mut self, rate: Rate) -> Self {
        self.observed_fixing = Some(rate.as_decimal());
        self
    }
}

// Explicit Instrument trait implementation (replaces macro for better IDE visibility)
impl crate::instruments::common_impl::traits::Instrument for ForwardRateAgreement {
    impl_instrument_base!(crate::pricer::InstrumentType::FRA);

    fn base_value(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        let pv = self.npv_raw(curves, as_of)?;
        Ok(finstack_core::money::Money::new(
            pv,
            self.notional.currency(),
        ))
    }

    fn value_raw(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<f64> {
        self.npv_raw(curves, as_of)
    }

    fn expiry(&self) -> Option<finstack_core::dates::Date> {
        Some(self.maturity)
    }

    fn effective_start_date(&self) -> Option<finstack_core::dates::Date> {
        Some(self.start_date)
    }

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for ForwardRateAgreement {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .forward(self.forward_curve_id.clone())
            .build()
    }
}

impl CashflowProvider for ForwardRateAgreement {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        // Settlement at start of accrual period; if already settled, no flows
        if self.start_date <= as_of {
            return Ok(crate::cashflow::traits::schedule_from_classified_flows(
                Vec::new(),
                self.day_count,
                crate::cashflow::traits::ScheduleBuildOpts {
                    notional_hint: self.notional(),
                    representation: crate::cashflow::builder::CashflowRepresentation::NoResidual,
                    ..Default::default()
                },
            ));
        }

        let flows = {
            let settlement = self.settlement_amount_raw(curves, as_of)?;
            vec![(
                self.start_date,
                Money::new(settlement, self.notional.currency()),
            )]
        };

        let schedule = crate::cashflow::traits::schedule_from_dated_flows(
            flows,
            self.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional(),
                kind: Some(crate::cashflow::primitives::CFKind::Fixed),
                ..Default::default()
            },
        );
        Ok(schedule.normalize_public(
            as_of,
            crate::cashflow::builder::CashflowRepresentation::Projected,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_core::currency::Currency;
    use finstack_core::dates::{Date, DateExt};
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::market_data::term_structures::ForwardCurve;
    use finstack_core::math::interp::InterpStyle;
    use time::Month;

    #[test]
    fn fra_par_pv_near_zero_with_settlement_adjustment() {
        // Build simple flat curves: 5% forward, discount with reasonable decay
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let disc = DiscountCurve::builder("DISC")
            .base_date(base)
            .knots([(0.0, 1.0), (5.0, 0.78)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("FRA builder should succeed in test");

        let fwd = ForwardCurve::builder("FWD-3M", 0.25)
            .base_date(base)
            .knots([(0.0, 0.05), (5.0, 0.05)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("FRA builder should succeed in test");

        let ctx = MarketContext::new().insert(disc).insert(fwd);

        // FRA 3M x 6M
        let start = base + time::Duration::days(90);
        let end = base + time::Duration::days(180);
        let fixing = start.add_weekdays(-2); // 2 business days before start for reset_lag
        let fra = ForwardRateAgreement::builder()
            .id("FRA-3x6".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .fixing_date(fixing)
            .start_date(start)
            .maturity(end)
            .fixed_rate(
                finstack_core::decimal::f64_to_decimal(0.05)
                    .expect("fixed rate should convert in test"),
            )
            .day_count(finstack_core::dates::DayCount::Act360)
            .reset_lag(2)
            .discount_curve_id("DISC".into())
            .forward_curve_id("FWD-3M".into())
            .side(PayReceive::Pay) // Pay fixed, receive floating
            .build()
            .expect("FRA builder should succeed in test");

        let pv = fra
            .value(&ctx, base)
            .expect("FRA valuation should succeed in test");
        // With settlement adjustment PV should be very close to zero at par
        assert!(
            pv.amount().abs() < 0.01,
            "FRA PV not near zero: {}",
            pv.amount()
        );
    }

    #[test]
    fn fra_par_rate_metric() {
        // Build simple flat curves
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let disc = DiscountCurve::builder("DISC")
            .base_date(base)
            .knots([(0.0, 1.0), (5.0, 0.78)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("FRA builder should succeed in test");

        let fwd_rate = 0.05;
        let fwd = ForwardCurve::builder("FWD-3M", 0.25)
            .base_date(base)
            .knots([(0.0, fwd_rate), (5.0, fwd_rate)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("FRA builder should succeed in test");

        let ctx = MarketContext::new().insert(disc).insert(fwd);

        let start = base + time::Duration::days(90);
        let end = base + time::Duration::days(180);
        let fixing = start.add_weekdays(-2);

        let fra = ForwardRateAgreement::builder()
            .id("FRA-TEST".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .fixing_date(fixing)
            .start_date(start)
            .maturity(end)
            .fixed_rate(
                finstack_core::decimal::f64_to_decimal(0.04)
                    .expect("fixed rate should convert in test"),
            ) // Different from market rate
            .day_count(finstack_core::dates::DayCount::Act360)
            .reset_lag(2)
            .discount_curve_id("DISC".into())
            .forward_curve_id("FWD-3M".into())
            .side(PayReceive::Receive)
            .build()
            .expect("Builder failed");

        use crate::instruments::common_impl::traits::Instrument;
        use crate::instruments::rates::fra::metrics::FraParRateCalculator;
        use crate::metrics::{MetricCalculator, MetricContext};
        use std::sync::Arc;

        // Wrap in Arc for metric context
        let fra_arc = Arc::new(fra);
        let ctx_arc = Arc::new(ctx);

        // Calculate base PV
        let base_pv = fra_arc
            .value(&ctx_arc, base)
            .expect("PV calculation failed");

        let calc = FraParRateCalculator;
        let mut m_ctx = MetricContext::new(
            fra_arc as Arc<dyn Instrument>,
            ctx_arc,
            base,
            base_pv,
            MetricContext::default_config(),
        );

        let par_rate = calc
            .calculate(&mut m_ctx)
            .expect("Par rate calculation failed");

        assert!(
            (par_rate - fwd_rate).abs() < 1e-10,
            "Par rate {} should equal forward rate {}",
            par_rate,
            fwd_rate
        );
    }
}

#[cfg(test)]
mod serde_tests {
    use super::*;
    use finstack_core::currency::Currency;
    use rust_decimal::prelude::ToPrimitive;

    #[test]
    fn fra_deserialize_defaults_side_to_pay_fixed() {
        let json = serde_json::json!({
            "id": "FRA-DEFAULT-SIDE",
            "notional": {"amount": 1_000_000.0, "currency": "USD"},
            "start_date": "2025-04-03",
            "maturity": "2025-07-03",
            "fixed_rate": "0.045",
            "day_count": "Act360",
            "reset_lag": 2,
            "discount_curve_id": "USD-OIS",
            "forward_curve_id": "USD-SOFR-3M",
            "pricing_overrides": {},
            "attributes": {"tags": [], "meta": {}}
        });
        let fra: ForwardRateAgreement = serde_json::from_value(json).expect("deserialize FRA");
        assert_eq!(fra.side, PayReceive::Pay);
        assert_eq!(fra.notional.currency(), Currency::USD);
    }

    #[test]
    fn from_conventions_applies_rate_index_defaults() {
        let fra = ForwardRateAgreement::from_conventions(ConventionFraParams {
            id: InstrumentId::new("FRA-USD-SOFR-3X6"),
            notional: Money::new(1_000_000.0, Currency::USD),
            start_date: Date::from_calendar_date(2025, time::Month::April, 3)
                .expect("valid start date"),
            maturity: Date::from_calendar_date(2025, time::Month::July, 3)
                .expect("valid maturity date"),
            fixed_rate: 0.045,
            index_id: "USD-SOFR-3M",
            discount_curve_id: "USD-OIS",
            forward_curve_id: "USD-SOFR-3M",
            side: PayReceive::Receive,
            attributes: Attributes::new(),
        })
        .expect("FRA conventions constructor should succeed");

        assert_eq!(fra.id, InstrumentId::new("FRA-USD-SOFR-3X6"));
        assert_eq!(fra.notional, Money::new(1_000_000.0, Currency::USD));
        assert_eq!(fra.day_count, DayCount::Act360);
        assert_eq!(fra.reset_lag, 2);
        assert_eq!(fra.discount_curve_id, CurveId::new("USD-OIS"));
        assert_eq!(fra.forward_curve_id, CurveId::new("USD-SOFR-3M"));
        assert_eq!(fra.fixed_rate.to_f64(), Some(0.045));
        assert_eq!(fra.side, PayReceive::Receive);
        assert_eq!(fra.fixing_calendar_id.as_deref(), Some("usny"));
        assert_eq!(
            fra.fixing_bdc,
            Some(BusinessDayConvention::ModifiedFollowing)
        );
    }
}

//! Fixed and floating swap leg specifications shared across rate derivatives.

use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor};
use finstack_quant_core::types::CurveId;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Add a business-day payment delay to `date`, using `calendar_id` or `weekends_only`.
///
/// Zero or negative delays leave `date` unchanged. Positive delays step forward
/// by business days on the resolved calendar.
///
/// # Arguments
///
/// * `date` - Base date before payment lag adjustment.
/// * `delay_days` - Number of business days to add (non-positive skips adjustment).
/// * `calendar_id` - Optional holiday calendar identifier (`"weekends_only"` when `None`).
pub fn add_payment_delay(
    date: Date,
    delay_days: i32,
    calendar_id: Option<&str>,
) -> finstack_quant_core::Result<Date> {
    if delay_days <= 0 {
        return Ok(date);
    }
    let cal_id = calendar_id.unwrap_or(finstack_quant_core::dates::WEEKENDS_ONLY_ID);
    let calendar = finstack_quant_core::dates::calendar_by_id_strict(cal_id)?;
    date.add_business_days(delay_days, calendar)
}

/// Error for a swap float leg set to `simple_average`, which the swap
/// projectors do not implement (they compound or take one term forward).
pub fn simple_average_unsupported() -> finstack_quant_core::Error {
    finstack_quant_core::Error::Validation(
        "float_leg.compounding = simple_average is not supported on swap legs; \
         use simple or a compounded_* variant"
            .to_string(),
    )
}

/// Specification for fixed rate legs in interest rate swaps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FixedLegSpec {
    /// Discount curve identifier for pricing.
    pub discount_curve_id: CurveId,
    /// Fixed rate (e.g., 0.05 for 5%).
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub rate: Decimal,
    /// Payment frequency.
    pub frequency: Tenor,
    /// Day count convention for accrual.
    pub day_count: DayCount,
    /// Business day convention for payment dates.
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Optional calendar for business day adjustments.
    pub calendar_id: Option<finstack_quant_core::types::CalendarId>,
    /// Stub period handling rule.
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Start date of the fixed leg.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start: Date,
    /// End date of the fixed leg.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end: Date,
    /// Payment lag in business days after period end (default: 0).
    #[serde(default)]
    pub payment_lag_days: i32,
    /// End-of-month roll convention (default: false).
    #[serde(default)]
    pub end_of_month: bool,
}

impl FixedLegSpec {
    /// Period-generation parameters for this leg's accrual schedule.
    ///
    /// Copies the leg's dates, frequency, stub, business-day convention,
    /// end-of-month flag, day count and payment lag. The calendar is the leg's
    /// own `calendar_id`, else the weekends-only calendar. Fixed legs have no
    /// resets (`reset_lag_days: None`) and no roll rule.
    ///
    /// # Arguments
    ///
    /// * `adjust_accrual_dates` - When `true`, accrual boundaries are moved
    ///   to business days with the leg's business-day convention; when
    ///   `false`, accrual uses the unadjusted roll dates and only payment
    ///   dates are adjusted.
    pub fn schedule_params(
        &self,
        adjust_accrual_dates: bool,
    ) -> finstack_quant_cashflows::builder::BuildPeriodsParams<'_> {
        finstack_quant_cashflows::builder::BuildPeriodsParams {
            start: self.start,
            end: self.end,
            frequency: self.frequency,
            stub: self.stub,
            business_day_convention: self.business_day_convention,
            calendar_id: self
                .calendar_id
                .as_deref()
                .unwrap_or(finstack_quant_core::dates::WEEKENDS_ONLY_ID),
            end_of_month: self.end_of_month,
            day_count: self.day_count,
            payment_lag_days: self.payment_lag_days,
            reset_lag_days: None,
            adjust_accrual_dates,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        }
    }

    /// Validate the structural invariants of this fixed-leg specification.
    ///
    /// # Errors
    /// Returns an error stating both dates when `start >= end`.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        finstack_quant_core::validation::validate_date_range_strict(
            self.start,
            self.end,
            "FixedLegSpec",
        )
    }
}

/// Specification for floating rate legs in interest rate swaps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FloatLegSpec {
    /// Contractual observed index identifying historical rate publications.
    pub index_id: finstack_quant_core::types::IndexId,
    /// Discount curve identifier for pricing.
    pub discount_curve_id: CurveId,
    /// Forward curve identifier for rate projections.
    pub forward_curve_id: CurveId,
    /// Spread in basis points added to the forward rate.
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub spread_bp: Decimal,
    /// Payment frequency.
    pub frequency: Tenor,
    /// Day count convention for accrual.
    pub day_count: DayCount,
    /// Business day convention for payment dates.
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Optional calendar for business day adjustments.
    pub calendar_id: Option<finstack_quant_core::types::CalendarId>,
    /// Stub period handling rule.
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Reset lag in business days for floating rate fixing (default: 0).
    #[serde(default)]
    pub reset_lag_days: i32,
    /// Optional calendar for rate fixing (reset lag).
    #[serde(default)]
    pub fixing_calendar_id: Option<finstack_quant_core::types::CalendarId>,
    /// Start date of the floating leg.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start: Date,
    /// End date of the floating leg.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end: Date,
    /// Compounding method for floating coupons.
    #[serde(default)]
    pub compounding: finstack_quant_cashflows::builder::FloatingLegCompounding,
    /// Payment lag in business days after period end (default: 0).
    #[serde(default)]
    pub payment_lag_days: i32,
    /// End-of-month roll convention (default: false).
    #[serde(default)]
    pub end_of_month: bool,
}

impl FloatLegSpec {
    /// Period-generation parameters for this leg's accrual schedule.
    ///
    /// Copies the leg's dates, frequency, stub, business-day convention,
    /// end-of-month flag, day count and payment lag, with no roll rule.
    ///
    /// # Arguments
    ///
    /// * `calendar_id` - Holiday calendar id used for payment-date and (when
    ///   `adjust_accrual_dates` is `true`) accrual-date adjustment. Pass
    ///   [`Self::calendar_or_weekends`] for the leg's own calendar with the
    ///   weekends-only fallback.
    /// * `reset_lag_days` - Business days between reset and accrual start
    ///   emitted on each period (`Some(n)`), or `None` when the caller
    ///   computes resets itself and the periods carry no reset date.
    /// * `adjust_accrual_dates` - When `true`, accrual boundaries are moved
    ///   to business days with the leg's business-day convention; when
    ///   `false`, accrual uses the unadjusted roll dates.
    pub fn schedule_params<'a>(
        &self,
        calendar_id: &'a str,
        reset_lag_days: Option<i32>,
        adjust_accrual_dates: bool,
    ) -> finstack_quant_cashflows::builder::BuildPeriodsParams<'a> {
        finstack_quant_cashflows::builder::BuildPeriodsParams {
            start: self.start,
            end: self.end,
            frequency: self.frequency,
            stub: self.stub,
            business_day_convention: self.business_day_convention,
            calendar_id,
            end_of_month: self.end_of_month,
            day_count: self.day_count,
            payment_lag_days: self.payment_lag_days,
            reset_lag_days,
            adjust_accrual_dates,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        }
    }

    /// The leg's own `calendar_id`, else the weekends-only calendar id.
    pub fn calendar_or_weekends(&self) -> &str {
        self.calendar_id
            .as_deref()
            .unwrap_or(finstack_quant_core::dates::WEEKENDS_ONLY_ID)
    }

    /// Validate the structural invariants of this floating-leg specification.
    ///
    /// # Errors
    /// Returns an error stating both dates when `start >= end`, and when
    /// `compounding` is `simple_average`, which swap legs do not implement.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        finstack_quant_core::market_data::fixings::validate_fixing_index_id(
            self.index_id.as_str(),
        )?;
        if self.compounding
            == finstack_quant_cashflows::builder::FloatingLegCompounding::SimpleAverage
        {
            return Err(simple_average_unsupported());
        }
        finstack_quant_core::validation::validate_date_range_strict(
            self.start,
            self.end,
            "FloatLegSpec",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    fn fixed_leg(start: Date, end: Date) -> FixedLegSpec {
        FixedLegSpec {
            discount_curve_id: CurveId::new("USD-OIS"),
            rate: Decimal::new(5, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            start,
            end,
            payment_lag_days: 0,
            end_of_month: false,
        }
    }

    fn float_leg(start: Date, end: Date) -> FloatLegSpec {
        FloatLegSpec {
            discount_curve_id: CurveId::new("USD-OIS"),
            index_id: "USD-SOFR".into(),
            forward_curve_id: CurveId::new("USD-SOFR"),
            spread_bp: Decimal::ZERO,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            start,
            end,
            compounding: finstack_quant_cashflows::builder::FloatingLegCompounding::default(),
            payment_lag_days: 0,
            end_of_month: false,
        }
    }

    #[test]
    fn fixed_leg_validate_rejects_non_increasing_dates() {
        let start = date!(2025 - 03 - 15);
        let end = date!(2030 - 03 - 15);

        let equal = fixed_leg(start, start)
            .validate()
            .expect_err("start == end must be rejected");
        assert!(
            equal.to_string().contains("FixedLegSpec"),
            "error should name the leg: {equal}"
        );

        assert!(fixed_leg(end, start).validate().is_err());
        assert!(fixed_leg(start, end).validate().is_ok());
    }

    #[test]
    fn float_leg_validate_rejects_non_increasing_dates() {
        let start = date!(2025 - 03 - 15);
        let end = date!(2030 - 03 - 15);

        let equal = float_leg(start, start)
            .validate()
            .expect_err("start == end must be rejected");
        assert!(
            equal.to_string().contains("FloatLegSpec"),
            "error should name the leg: {equal}"
        );

        assert!(float_leg(end, start).validate().is_err());
        assert!(float_leg(start, end).validate().is_ok());
    }

    #[test]
    fn float_leg_reset_lag_days_defaults_to_zero_on_the_wire() {
        let json = r#"{
            "discount_curve_id": "USD-OIS",
            "index_id": "USD-SOFR-3M",
            "forward_curve_id": "USD-SOFR-3M",
            "spread_bp": "0",
            "frequency": {"count": 3, "unit": "months"},
            "day_count": "act_360",
            "calendar_id": null,
            "start": "2025-01-15",
            "end": "2030-01-15"
        }"#;
        let leg: FloatLegSpec = serde_json::from_str(json).expect("float leg without reset lag");
        assert_eq!(leg.reset_lag_days, 0);
        assert_eq!(leg.payment_lag_days, 0);
        assert!(!leg.end_of_month);
    }

    #[test]
    fn float_leg_validate_rejects_simple_average() {
        let mut leg = float_leg(date!(2025 - 03 - 15), date!(2030 - 03 - 15));
        leg.compounding = finstack_quant_cashflows::builder::FloatingLegCompounding::SimpleAverage;
        assert!(
            leg.validate()
                .expect_err("simple_average must be rejected")
                .to_string()
                .contains("simple_average")
        );
    }

    #[test]
    fn test_add_payment_delay_zero_days() {
        let d = date!(2025 - 01 - 15);
        let result = add_payment_delay(d, 0, None).expect("should succeed");
        assert_eq!(result, d);
    }

    #[test]
    fn test_add_payment_delay_positive_days() {
        let d = date!(2025 - 01 - 17); // Friday
        let result = add_payment_delay(d, 2, None).expect("should succeed");
        // Friday + 2 business days = Tuesday (skipping Sat/Sun)
        assert_eq!(result, date!(2025 - 01 - 21));
    }

    #[test]
    fn test_add_payment_delay_invalid_calendar_errors() {
        let d = date!(2025 - 01 - 15);
        let result = add_payment_delay(d, 2, Some("nonexistent_calendar"));
        assert!(
            result.is_err(),
            "add_payment_delay with nonexistent calendar must return Err"
        );
    }
}

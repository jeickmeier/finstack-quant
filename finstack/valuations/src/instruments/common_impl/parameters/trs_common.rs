//! Common TRS types shared between equity and fixed income TRS.
//!
//! This module provides shared types used by both `EquityTotalReturnSwap`
//! and `FIIndexTotalReturnSwap` instruments.

use crate::cashflow::builder::ScheduleParams;
use finstack_core::dates::{Date, DateExt, Schedule, ScheduleBuilder};

/// Side of the TRS trade from the party's perspective.
///
/// Determines whether the party receives or pays the total return leg.
///
/// # Examples
///
/// ```
/// use finstack_valuations::instruments::TrsSide;
///
/// let side = TrsSide::ReceiveTotalReturn;
/// assert_eq!(side.sign(), 1.0);
///
/// let side = TrsSide::PayTotalReturn;
/// assert_eq!(side.sign(), -1.0);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TrsSide {
    /// Receive total return, pay financing.
    ReceiveTotalReturn,
    /// Pay total return, receive financing.
    PayTotalReturn,
}

impl std::fmt::Display for TrsSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrsSide::ReceiveTotalReturn => write!(f, "receive_total_return"),
            TrsSide::PayTotalReturn => write!(f, "pay_total_return"),
        }
    }
}

impl std::str::FromStr for TrsSide {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "receive_total_return" | "receive" => Ok(TrsSide::ReceiveTotalReturn),
            "pay_total_return" | "pay" => Ok(TrsSide::PayTotalReturn),
            other => Err(format!("Unknown TRS side: {}", other)),
        }
    }
}

impl TrsSide {
    /// Gets the sign multiplier for present value calculation.
    ///
    /// # Returns
    /// 1.0 for ReceiveTotalReturn, -1.0 for PayTotalReturn.
    pub fn sign(&self) -> f64 {
        match self {
            TrsSide::ReceiveTotalReturn => 1.0,
            TrsSide::PayTotalReturn => -1.0,
        }
    }
}

/// Schedule specification for TRS payment periods.
///
/// Defines the payment schedule and frequency for both legs of the TRS.
/// This is shared between equity and fixed income TRS instruments.
///
/// # Examples
///
/// ```
/// use finstack_valuations::instruments::TrsScheduleSpec;
/// use finstack_cashflows::builder::ScheduleParams;
/// use finstack_core::dates::{Date, Tenor, DayCount, BusinessDayConvention, StubKind};
///
/// let schedule = TrsScheduleSpec::from_params(
///     Date::from_calendar_date(2024, time::Month::January, 1).unwrap(),
///     Date::from_calendar_date(2025, time::Month::January, 1).unwrap(),
///     ScheduleParams {
///         freq: Tenor::quarterly(),
///         dc: DayCount::Act360,
///         bdc: BusinessDayConvention::Following,
///         calendar_id: "weekends_only".to_string(),
///         stub: StubKind::None,
///         end_of_month: false,
///         payment_lag_days: 0,
///     },
/// );
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrsScheduleSpec {
    /// Start date for the TRS leg.
    #[schemars(with = "String")]
    pub start: Date,
    /// End date for the TRS leg.
    #[schemars(with = "String")]
    pub end: Date,
    /// Schedule parameters (frequency, day count, bdc, calendar, stub).
    pub params: ScheduleParams,
}

impl TrsScheduleSpec {
    /// Creates a schedule specification from start/end dates and schedule parameters.
    pub fn from_params(start: Date, end: Date, schedule: ScheduleParams) -> Self {
        Self {
            start,
            end,
            params: schedule,
        }
    }

    /// Builds the period date schedule in a canonical way.
    pub fn period_schedule(&self) -> finstack_core::Result<Schedule> {
        let cal =
            crate::cashflow::builder::calendar::resolve_calendar_strict(&self.params.calendar_id)?;
        ScheduleBuilder::new(self.start, self.end)?
            .frequency(self.params.freq)
            .stub_rule(self.params.stub)
            .end_of_month(self.params.end_of_month)
            .adjust_with(self.params.bdc, cal)
            .build()
    }

    /// Computes the payment date for a given accrual end date.
    ///
    /// When `payment_lag_days == 0` this returns the accrual end date unchanged.
    /// Otherwise the date is shifted forward by the specified number of business
    /// days using the schedule's calendar.
    pub fn payment_date_for(&self, accrual_end: Date) -> finstack_core::Result<Date> {
        if self.params.payment_lag_days == 0 {
            return Ok(accrual_end);
        }
        let cal =
            crate::cashflow::builder::calendar::resolve_calendar_strict(&self.params.calendar_id)?;
        accrual_end.add_business_days(self.params.payment_lag_days, cal)
    }
}

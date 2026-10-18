//! Common TRS types shared between equity and fixed income TRS.
//!
//! This module provides shared types used by both `EquityTotalReturnSwap`
//! and `FiIndexTotalReturnSwap` instruments.

use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::dates::{Date, DateExt, Schedule};

/// Schedule specification for TRS payment periods.
///
/// Defines the payment schedule and frequency for both legs of the TRS.
/// This is shared between equity and fixed income TRS instruments.
///
/// # Examples
///
/// ```
/// use finstack_quant_valuations_core::instruments::TrsScheduleSpec;
/// use finstack_quant_cashflows::builder::ScheduleParams;
/// use finstack_quant_core::dates::{Date, Tenor, DayCount, BusinessDayConvention, StubKind};
///
/// let schedule = TrsScheduleSpec::from_params(
///     Date::from_calendar_date(2024, time::Month::January, 1).unwrap(),
///     Date::from_calendar_date(2025, time::Month::January, 1).unwrap(),
///     ScheduleParams {
///         frequency: Tenor::quarterly(),
///         day_count: DayCount::Act360,
///         business_day_convention: BusinessDayConvention::Following,
///         calendar_id: "weekends_only".into(),
///         stub: StubKind::None,
///         end_of_month: false,
///         payment_lag_days: 0,
///         adjust_accrual_dates: false,
///         roll_rule: finstack_quant_cashflows::builder::RollRule::None,
///     },
/// );
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TrsScheduleSpec {
    /// Start date for the TRS leg.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start: Date,
    /// End date for the TRS leg.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end: Date,
    /// Schedule parameters (frequency, day count, business_day_convention, calendar, stub).
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

    /// Validate the accrual interval, calendar, and generated period schedule.
    ///
    /// # Arguments
    ///
    /// * `context` - Instrument name included in validation diagnostics.
    pub fn validate(&self, context: &str) -> finstack_quant_core::Result<()> {
        if self.start >= self.end {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} schedule start must precede end"
            )));
        }
        finstack_quant_core::dates::calendar_by_id_strict(&self.params.calendar_id)?;
        let schedule = self.period_schedule()?;
        if schedule.dates.len() < 2 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} schedule must contain at least one accrual period"
            )));
        }
        Ok(())
    }

    /// Builds the period date schedule in a canonical way.
    pub fn period_schedule(&self) -> finstack_quant_core::Result<Schedule> {
        use finstack_quant_cashflows::builder::{build_periods, BuildPeriodsParams};

        let periods = build_periods(BuildPeriodsParams::from_schedule(
            &self.params,
            self.start,
            self.end,
            None,
        ))?;
        let mut dates = Vec::with_capacity(periods.len() + 1);
        if let Some(first) = periods.first() {
            dates.push(first.accrual_start);
            dates.extend(periods.iter().map(|period| period.accrual_end));
        }
        let payment_dates = periods.iter().map(|period| period.payment_date).collect();
        Ok(Schedule {
            dates,
            payment_dates,
            fixing_dates: Vec::new(),
            warnings: Vec::new(),
        })
    }

    /// Computes the payment date for a given accrual end date.
    ///
    /// Apply the schedule business-day convention, then its business-day lag.
    ///
    /// # Arguments
    ///
    /// * `accrual_end` - Contractual end date of the return period, before payment adjustment.
    pub fn payment_date_for(&self, accrual_end: Date) -> finstack_quant_core::Result<Date> {
        let cal = finstack_quant_core::dates::calendar_by_id_strict(&self.params.calendar_id)?;
        let adjusted = finstack_quant_core::dates::adjust(
            accrual_end,
            self.params.business_day_convention,
            cal,
        )?;
        adjusted.add_business_days(self.params.payment_lag_days, cal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::BusinessDayConvention;
    use time::macros::date;

    #[test]
    fn payment_dates_keep_month_end_adjustment_and_lag() {
        for lag in [0, 2] {
            let mut params = ScheduleParams::quarterly_act360();
            params.calendar_id = "weekends_only".into();
            params.business_day_convention = BusinessDayConvention::ModifiedFollowing;
            params.adjust_accrual_dates = false;
            params.payment_lag_days = lag;
            let spec =
                TrsScheduleSpec::from_params(date!(2024 - 03 - 30), date!(2024 - 06 - 30), params);
            let expected = if lag == 0 {
                date!(2024 - 06 - 28)
            } else {
                date!(2024 - 07 - 02)
            };
            assert_eq!(
                spec.period_schedule().expect("schedule").payment_dates,
                vec![expected]
            );
            assert_eq!(
                spec.payment_date_for(date!(2024 - 06 - 30))
                    .expect("payment"),
                expected
            );
        }
    }
}

//! Contractual term-index caplet schedules on the ACT/365F model clock.

use super::{pricing::cap_floor_periods, CapFloorQuote, SwapFrequency};
use finstack_quant_core::{Error, Result};

/// One live term-index caplet or floorlet, per unit notional.
///
/// Model times use ACT/365F years from the valuation date. Coupon accrual is
/// independent of that clock and retains the index's contractual day count.
/// Compounded overnight coupons require a different option model and are not
/// represented by this term-index schedule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CapletSchedule {
    /// Positive rate fixing time, at or before the accrual-period start.
    pub fixing_time: f64,
    /// Accrual-period start time, including contractual spot settlement.
    pub start_time: f64,
    /// Accrual-period end time, before any payment delay.
    pub end_time: f64,
    /// Coupon payment time, at or after accrual start. Calendar adjustment may
    /// move payment before the unadjusted accrual end.
    pub payment_time: f64,
    /// Positive coupon year fraction under the index day count, including stubs.
    pub accrual: f64,
}

/// Contractual live term-index coupons underlying one flat cap/floor quote.
///
/// The already-fixed first coupon is excluded, as in market cap quotes.
/// Periods must be ordered and non-overlapping; the final accrual-period end
/// must equal the associated quote's maturity on the model clock.
#[derive(Clone, Debug, PartialEq)]
pub struct CapFloorSchedule {
    /// Live caplet/floorlet periods in increasing fixing-time order.
    pub periods: Vec<CapletSchedule>,
}

impl CapFloorSchedule {
    pub(super) fn synthetic(maturity: f64, frequency: SwapFrequency) -> Self {
        Self {
            periods: cap_floor_periods(maturity, frequency)
                .map(|(start_time, end_time, accrual)| CapletSchedule {
                    fixing_time: start_time,
                    start_time,
                    end_time,
                    payment_time: end_time,
                    accrual,
                })
                .collect(),
        }
    }

    fn validate(&self, maturity: f64) -> Result<()> {
        if self.periods.is_empty() {
            return Err(Error::Validation(
                "cap/floor schedule has no live caplets after excluding the spot-start caplet"
                    .into(),
            ));
        }
        for (index, period) in self.periods.iter().enumerate() {
            if ![
                period.fixing_time,
                period.start_time,
                period.end_time,
                period.payment_time,
                period.accrual,
            ]
            .iter()
            .all(|value| value.is_finite())
                || period.fixing_time <= 0.0
                || period.start_time < period.fixing_time
                || period.end_time <= period.start_time
                || period.payment_time < period.start_time
                || period.accrual <= 0.0
            {
                return Err(Error::Validation(format!(
                    "invalid term-index caplet schedule at period {index}: require positive fixing/accrual, fixing <= start < end, and payment >= start"
                )));
            }
            if index > 0 {
                let previous = &self.periods[index - 1];
                if period.fixing_time <= previous.fixing_time
                    || period.start_time < previous.end_time - 1e-10
                {
                    return Err(Error::Validation(format!(
                        "cap/floor schedule periods must be ordered and non-overlapping at period {index}"
                    )));
                }
            }
        }
        if self
            .periods
            .last()
            .is_some_and(|period| (period.end_time - maturity).abs() > 1e-10)
        {
            return Err(Error::Validation(
                "cap/floor schedule final accrual end must match quote maturity".into(),
            ));
        }
        Ok(())
    }
}

pub(super) fn prepare_cap_floor_schedules(
    quotes: &[CapFloorQuote],
    schedules: Option<&[CapFloorSchedule]>,
    frequency: SwapFrequency,
) -> Result<Vec<CapFloorSchedule>> {
    if let Some(schedules) = schedules {
        if schedules.len() != quotes.len() {
            return Err(Error::Validation(format!(
                "cap/floor schedules.len() ({}) must match quotes.len() ({})",
                schedules.len(),
                quotes.len()
            )));
        }
    }
    quotes
        .iter()
        .enumerate()
        .map(|(index, quote)| {
            let schedule = schedules.map_or_else(
                || CapFloorSchedule::synthetic(quote.maturity, frequency),
                |schedules| schedules[index].clone(),
            );
            schedule.validate(quote.maturity).map_err(|error| {
                Error::Validation(format!(
                    "invalid cap/floor schedule at quote {index}: {error}"
                ))
            })?;
            Ok(schedule)
        })
        .collect()
}

#[cfg(test)]
#[path = "cap_schedule_tests.rs"]
mod tests;

//! Shared observation schedule generation for realized-variance instruments.

use finstack_quant_core::dates::calendar::Calendar;
use finstack_quant_core::dates::{Date, DateExt, HolidayCalendar, Tenor, TenorUnit};
use finstack_quant_core::types::Attributes;

static WEEKENDS_ONLY: Calendar = Calendar::new("weekends_only", "Weekends Only", true, &[]);

fn observation_calendar(attributes: &Attributes) -> &'static dyn HolidayCalendar {
    let requested = attributes
        .get_meta("observation_calendar_id")
        .or_else(|| attributes.get_meta("calendar_id"));
    if let Some(id) = requested {
        if let Ok(calendar) = crate::cashflow::builder::calendar::resolve_calendar_strict(id) {
            return calendar;
        }
        tracing::warn!(
            calendar_id = id,
            "Variance observation calendar is unavailable; using weekends_only"
        );
    }
    &WEEKENDS_ONLY
}

fn following(mut date: Date, calendar: &dyn HolidayCalendar) -> Date {
    while !calendar.is_business_day(date) {
        date += time::Duration::days(1);
    }
    date
}

fn preceding(mut date: Date, calendar: &dyn HolidayCalendar) -> Date {
    while !calendar.is_business_day(date) {
        date -= time::Duration::days(1);
    }
    date
}

/// Generate observation dates using business-day steps for `Days` tenors.
///
/// Day-tenor endpoints are adjusted following at inception and preceding at
/// maturity. Week/month/year tenors retain their contractual calendar spacing.
pub(crate) fn variance_observation_dates(
    start: Date,
    maturity: Date,
    frequency: Tenor,
    attributes: &Attributes,
) -> Vec<Date> {
    let mut dates = Vec::new();

    match frequency.unit {
        TenorUnit::Months | TenorUnit::Years => {
            let months_step = frequency.months().unwrap_or(12);
            let mut current = start;
            while current <= maturity {
                dates.push(current);
                current = current.add_months(months_step as i32);
            }
            if dates.last() != Some(&maturity) {
                dates.push(maturity);
            }
        }
        TenorUnit::Weeks => {
            let mut current = start;
            let days_step = i64::from(frequency.count) * 7;
            while current <= maturity {
                dates.push(current);
                current += time::Duration::days(days_step);
            }
            if dates.last() != Some(&maturity) {
                dates.push(maturity);
            }
        }
        TenorUnit::Days => {
            let calendar = observation_calendar(attributes);
            let mut current = following(start, calendar);
            let end = preceding(maturity, calendar);
            if current > end {
                return dates;
            }
            while current <= end {
                dates.push(current);
                let mut advanced = 0;
                while advanced < frequency.count {
                    current += time::Duration::days(1);
                    if calendar.is_business_day(current) {
                        advanced += 1;
                    }
                }
            }
            if dates.last() != Some(&end) {
                dates.push(end);
            }
        }
    }

    dates
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn day_tenor_uses_holiday_calendar_and_business_endpoints() {
        let attributes = Attributes::new().with_meta("observation_calendar_id", "USNY");
        let dates = variance_observation_dates(
            date!(2025 - 07 - 03),
            date!(2025 - 07 - 08),
            Tenor::daily(),
            &attributes,
        );
        assert_eq!(
            dates,
            vec![
                date!(2025 - 07 - 03),
                date!(2025 - 07 - 07),
                date!(2025 - 07 - 08),
            ]
        );

        let weekend_end = variance_observation_dates(
            date!(2025 - 07 - 03),
            date!(2025 - 07 - 06),
            Tenor::daily(),
            &attributes,
        );
        assert_eq!(weekend_end, vec![date!(2025 - 07 - 03)]);
    }
}

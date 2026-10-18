//! Date helpers for structured-credit floating-rate periods.

use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, Tenor};

/// Calculate a floating-rate period end from a tenor expressed in years.
///
/// Pricing callers propagate date-arithmetic failures rather than silently
/// replacing the period end with the start date.
#[inline]
pub(crate) fn try_tenor_to_period_end(
    start: Date,
    tenor_years: f64,
    day_count: DayCount,
) -> finstack_quant_core::Result<Date> {
    Tenor::from_years(tenor_years, day_count)?.add_to_date(
        start,
        None,
        BusinessDayConvention::Unadjusted,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn rolls_standard_periods() {
        assert_eq!(
            try_tenor_to_period_end(date!(2025 - 01 - 31), 0.25, DayCount::Act360),
            Ok(date!(2025 - 04 - 30))
        );
        assert_eq!(
            try_tenor_to_period_end(date!(2025 - 01 - 31), 1.0, DayCount::Act365F),
            Ok(date!(2026 - 01 - 31))
        );
    }
}

use finstack_quant_core::dates::Date;
use time::Month;

/// Convenience date helper for tests.
///
/// # Arguments
///
/// * `year` - Proleptic Gregorian calendar year.
/// * `month` - Calendar month, 1 (January) to 12 (December).
/// * `day` - Day of the month, 1-based; must exist in that month.
pub fn date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
        .expect("valid date")
}

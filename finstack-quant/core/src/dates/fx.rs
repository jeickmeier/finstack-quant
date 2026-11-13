//! FX forward value dates from an actual spot date (record
//! CORE-CONVENTIONS-011 #5–#10).
//!
//! There is no generic spot-date recipe here: a product either receives its
//! actual spot/settlement date or derives it under its own explicit
//! convention with [`DateExt::add_business_days`] and
//! [`crate::dates::resolve_joint_calendar`]. [`fx_forward_date`] is the one
//! forward value-date operation: it adds a tenor to an actual open spot date
//! on one explicit final settlement calendar.
//!
//! # References
//!
//! - Business-day conventions: `docs/REFERENCES.md#isda-2006-definitions`
//! - Business end-to-end FX forwards: ANZ, "Introduction to FX" (July 2002),
//!   PDF pages 20–21.

use crate::dates::{
    BusinessDayConvention, Date, DateExt, HolidayCalendar, Tenor, TenorUnit, adjust,
};
use crate::{Error, Result};

/// Forward value date `tenor` after an actual FX spot date.
///
/// Without business-EOM preservation (or with it and a spot that is not the
/// last business day of its month) the raw target is `spot_date + tenor`
/// (calendar days/weeks, or clamped months/years), rolled once with
/// `business_day_convention` on `calendar`. With preservation and a spot on
/// the last business day of its month, the result is the last business day
/// of the target month.
///
/// # Arguments
///
/// * `spot_date` - Actual spot date; must be open on `calendar` (it is never
///   pre-rolled).
/// * `tenor` - Tenor from spot, e.g. `3M` (a [`Tenor`] is always positive).
/// * `business_day_convention` - Roll applied once to the raw target on
///   `calendar`.
/// * `preserve_business_eom` - Keep a business month-end spot on the
///   business month-end of the target month. Only month and year tenors may
///   preserve it.
/// * `calendar` - The final settlement calendar, already the strict
///   composite of every contractually required member; nothing is inferred
///   from currencies.
///
/// # Errors
///
/// Returns an error when `preserve_business_eom` is set with
/// a day or week tenor, `spot_date` is closed on `calendar`, the result is
/// closed (for example a closed target under `Unadjusted`) or not strictly
/// after spot (for example `Preceding` collapsing onto spot), a preserved
/// target month has no open day, or calendar data or date arithmetic fails.
pub fn fx_forward_date(
    spot_date: Date,
    tenor: Tenor,
    business_day_convention: BusinessDayConvention,
    preserve_business_eom: bool,
    calendar: &dyn HolidayCalendar,
) -> Result<Date> {
    let month_tenor = matches!(tenor.unit(), TenorUnit::Months | TenorUnit::Years);
    if preserve_business_eom && !month_tenor {
        return Err(Error::Validation(format!(
            "business month-end preservation needs a month or year tenor, got {tenor}"
        )));
    }
    if !calendar.is_business_day(spot_date)? {
        return Err(Error::Validation(format!(
            "FX spot date {spot_date} is not a business day of the settlement calendar"
        )));
    }
    let raw = tenor.add_to_date(spot_date, None, BusinessDayConvention::Unadjusted)?;
    let value_date = if preserve_business_eom && is_business_month_end(spot_date, calendar)? {
        last_open_day_of_month(raw, calendar)?
    } else {
        adjust(raw, business_day_convention, calendar)?
    };
    if !calendar.is_business_day(value_date)? {
        return Err(Error::Validation(format!(
            "FX forward value date {value_date} ({tenor} after {spot_date}, \
             {business_day_convention}) is not a business day of the settlement calendar"
        )));
    }
    if value_date <= spot_date {
        return Err(Error::Validation(format!(
            "FX forward value date {value_date} ({tenor} after {spot_date}, \
             {business_day_convention}) is not after spot"
        )));
    }
    Ok(value_date)
}

/// Whether the open `date` is the last business day of its month: no later
/// day of that month is open.
fn is_business_month_end(date: Date, calendar: &dyn HolidayCalendar) -> Result<bool> {
    let month_end = date.end_of_month();
    let mut day = date;
    while day < month_end {
        day = day.add_days(1)?;
        if calendar.is_business_day(day)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Last open day of the month containing `date`; a wholly closed month is
/// an error rather than a neighbouring month.
fn last_open_day_of_month(date: Date, calendar: &dyn HolidayCalendar) -> Result<Date> {
    let mut day = date.end_of_month();
    loop {
        if calendar.is_business_day(day)? {
            return Ok(day);
        }
        if day.day() == 1 {
            return Err(Error::Validation(format!(
                "no business day in {} {} on the settlement calendar",
                date.month(),
                date.year()
            )));
        }
        day = day.add_days(-1)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::{
        WEEKENDS_ONLY, calendar_by_id_strict, create_date, resolve_calendar, resolve_joint_calendar,
    };
    use time::Month;

    fn d(year: i32, month: Month, day: u8) -> Date {
        create_date(year, month, day).expect("valid date")
    }

    fn tenor(text: &str) -> Tenor {
        text.parse().expect("valid tenor")
    }

    /// Weekends plus explicitly listed closed dates.
    struct Closures(Vec<Date>);

    impl HolidayCalendar for Closures {
        fn is_holiday(&self, date: Date) -> Result<bool> {
            Ok(self.0.contains(&date))
        }

        fn is_business_day(&self, date: Date) -> Result<bool> {
            Ok(WEEKENDS_ONLY.is_business_day(date)? && !self.0.contains(&date))
        }
    }

    #[test]
    fn natural_and_business_eom_from_the_last_business_day() {
        // August 29, 2025 is the last business day of August, not month-end.
        let spot = d(2025, Month::August, 29);
        let natural = fx_forward_date(
            spot,
            tenor("1M"),
            BusinessDayConvention::Following,
            false,
            &WEEKENDS_ONLY,
        )
        .expect("natural");
        assert_eq!(natural, d(2025, Month::September, 29));
        let preserved = fx_forward_date(
            spot,
            tenor("1M"),
            BusinessDayConvention::Following,
            true,
            &WEEKENDS_ONLY,
        )
        .expect("preserved");
        assert_eq!(preserved, d(2025, Month::September, 30));
    }

    #[test]
    fn a_non_eom_spot_uses_the_natural_branch_under_preservation() {
        let spot = d(2025, Month::August, 28);
        let date = fx_forward_date(
            spot,
            tenor("1M"),
            BusinessDayConvention::Following,
            true,
            &WEEKENDS_ONLY,
        )
        .expect("natural");
        assert_eq!(date, d(2025, Month::September, 29));
    }

    #[test]
    fn day_and_week_tenors_cannot_preserve_eom() {
        let spot = d(2025, Month::April, 30);
        for text in ["1D", "1W"] {
            assert!(
                fx_forward_date(
                    spot,
                    tenor(text),
                    BusinessDayConvention::Following,
                    true,
                    &WEEKENDS_ONLY
                )
                .is_err(),
                "{text}"
            );
        }
        let next = fx_forward_date(
            spot,
            tenor("1D"),
            BusinessDayConvention::Following,
            false,
            &WEEKENDS_ONLY,
        )
        .expect("1D");
        assert_eq!(next, d(2025, Month::May, 1));
    }

    #[test]
    fn preceding_onto_spot_and_closed_targets_are_errors() {
        // Friday spot, 1D lands on Saturday; Preceding collapses onto spot.
        let spot = d(2025, Month::January, 3);
        assert!(
            fx_forward_date(
                spot,
                tenor("1D"),
                BusinessDayConvention::Preceding,
                false,
                &WEEKENDS_ONLY
            )
            .is_err()
        );
        assert!(
            fx_forward_date(
                spot,
                tenor("1D"),
                BusinessDayConvention::Unadjusted,
                false,
                &WEEKENDS_ONLY
            )
            .is_err()
        );
    }

    #[test]
    fn a_closed_spot_is_rejected_not_pre_rolled() {
        let saturday = d(2025, Month::January, 4);
        assert!(
            fx_forward_date(
                saturday,
                tenor("1M"),
                BusinessDayConvention::Following,
                false,
                &WEEKENDS_ONLY
            )
            .is_err()
        );
    }

    #[test]
    fn preserved_eom_finds_the_last_open_target_day() {
        let calendar = Closures(vec![
            d(2025, Month::September, 29),
            d(2025, Month::September, 30),
        ]);
        let date = fx_forward_date(
            d(2025, Month::August, 29),
            tenor("1M"),
            BusinessDayConvention::Following,
            true,
            &calendar,
        )
        .expect("preserved");
        assert_eq!(date, d(2025, Month::September, 26));
    }

    #[test]
    fn a_wholly_closed_preserved_month_is_an_error() {
        let september: Vec<Date> = (1..=30).map(|day| d(2025, Month::September, day)).collect();
        let calendar = Closures(september);
        assert!(
            fx_forward_date(
                d(2025, Month::August, 29),
                tenor("1M"),
                BusinessDayConvention::Following,
                true,
                &calendar,
            )
            .is_err()
        );
    }

    #[test]
    fn date_range_boundaries_return_errors() {
        assert!(
            resolve_joint_calendar(None, None)
                .and_then(|cal| Date::MAX.add_business_days(1, cal))
                .is_err()
        );
        assert_eq!(
            resolve_joint_calendar(None, None)
                .and_then(|cal| Date::MAX.add_business_days(0, cal))
                .unwrap(),
            Date::MAX
        );
    }

    #[test]
    fn test_resolve_calendar_unknown_id() {
        let result = resolve_calendar(Some("unknown_calendar_id"));
        let err = result.err().expect("unknown calendar must error");
        assert!(
            matches!(err, Error::Input(ref e) if matches!(
                e,
                crate::InputError::CalendarNotFound { .. }
            )),
            "Should be CalendarNotFound error, got: {:?}",
            err
        );
    }

    #[test]
    fn symmetric_settlement_calendar_honours_usd_holiday_on_cross() {
        // EUR/JPY traded Wed 2025-07-02: two-currency T+2 lands on the US
        // holiday; a composite with USNY rolls to Mon 2025-07-07.
        let trade = d(2025, Month::July, 2);
        let spot_two_cal = resolve_joint_calendar(Some("target2"), Some("jpto"))
            .and_then(|cal| trade.add_business_days(2, cal))
            .expect("two-calendar roll");
        assert_eq!(spot_two_cal, d(2025, Month::July, 4));
        let spot_with_usd = calendar_by_id_strict("target2+jpto+usny")
            .and_then(|cal| trade.add_business_days(2, cal))
            .expect("three-calendar roll");
        assert_eq!(spot_with_usd, d(2025, Month::July, 7));
    }
}

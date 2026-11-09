//! Business day adjustment tests
//!
//! Tests for all business day conventions:
//! - Unadjusted
//! - Following
//! - Preceding
//! - ModifiedFollowing
//! - ModifiedPreceding
//! - Nearest
//!
//! Also includes edge cases for consecutive holidays, long closures and
//! search termination at the end of the supported date range.

use super::common::{TestCal, make_date};
use finstack_quant_core::InputError;
use finstack_quant_core::dates::{BusinessDayConvention, CalendarMetadata, HolidayCalendar};
use finstack_quant_core::dates::{adjust, available_calendars};
use time::Date;

// Basic Holiday Calendar Trait

#[test]
fn test_holiday_calendar_trait() {
    let cal = TestCal::new().with_holiday(make_date(2025, 1, 1)); // New Year's Day (Wednesday)

    // Wednesday New Year should be a holiday
    assert!(
        cal.is_holiday(make_date(2025, 1, 1))
            .expect("calendar lookup")
    );

    // Regular weekday should not be a holiday
    assert!(
        !cal.is_holiday(make_date(2025, 1, 2))
            .expect("calendar lookup")
    ); // Thursday

    // Weekend should not be a business day (handled by trait default)
    assert!(
        !cal.is_business_day(make_date(2025, 1, 4))
            .expect("calendar lookup")
    ); // Saturday
    assert!(
        !cal.is_business_day(make_date(2025, 1, 5))
            .expect("calendar lookup")
    ); // Sunday

    // Holiday should not be a business day
    assert!(
        !cal.is_business_day(make_date(2025, 1, 1))
            .expect("calendar lookup")
    ); // New Year

    // Regular weekday should be a business day
    assert!(
        cal.is_business_day(make_date(2025, 1, 2))
            .expect("calendar lookup")
    ); // Thursday
}

// Unadjusted Convention

#[test]
fn test_adjust_unadjusted() {
    let cal = TestCal::new();
    let saturday = make_date(2025, 1, 4); // Saturday

    // Unadjusted should return the same date even if it's a weekend
    let result = adjust(saturday, BusinessDayConvention::Unadjusted, &cal).unwrap();
    assert_eq!(result, saturday);
}

// Following Convention

#[test]
fn test_adjust_following() {
    let cal = TestCal::new().with_holiday(make_date(2025, 1, 2)); // Thursday holiday

    // Saturday should move to Monday
    let saturday = make_date(2025, 1, 4);
    let result = adjust(saturday, BusinessDayConvention::Following, &cal).unwrap();
    assert_eq!(result, make_date(2025, 1, 6)); // Monday

    // Holiday Thursday should move to Friday
    let thursday = make_date(2025, 1, 2);
    let result = adjust(thursday, BusinessDayConvention::Following, &cal).unwrap();
    assert_eq!(result, make_date(2025, 1, 3)); // Friday

    // Business day should remain unchanged
    let friday = make_date(2025, 1, 3);
    let result = adjust(friday, BusinessDayConvention::Following, &cal).unwrap();
    assert_eq!(result, friday);
}

#[test]
fn following_skips_multiple_consecutive_holidays() {
    // Set up calendar with back-to-back weekday holidays surrounding a weekend.
    let cal = TestCal::new()
        .with_holiday(make_date(2025, 7, 3)) // Thursday
        .with_holiday(make_date(2025, 7, 4)); // Friday (Independence Day)

    // Start from Thursday holiday: should roll forward to Monday July 7th.
    let thursday = make_date(2025, 7, 3);
    let adjusted = adjust(thursday, BusinessDayConvention::Following, &cal)
        .expect("Following adjustment should succeed");
    assert_eq!(adjusted, make_date(2025, 7, 7));
}

// Preceding Convention

#[test]
fn test_adjust_preceding() {
    let cal = TestCal::new().with_holiday(make_date(2025, 1, 3)); // Friday holiday

    // Sunday should move to Friday (but Friday is holiday, so Thursday)
    let sunday = make_date(2025, 1, 5);
    let result = adjust(sunday, BusinessDayConvention::Preceding, &cal).unwrap();
    assert_eq!(result, make_date(2025, 1, 2)); // Thursday

    // Holiday Friday should move to Thursday
    let friday = make_date(2025, 1, 3);
    let result = adjust(friday, BusinessDayConvention::Preceding, &cal).unwrap();
    assert_eq!(result, make_date(2025, 1, 2)); // Thursday
}

#[test]
fn preceding_skips_multiple_consecutive_holidays() {
    let cal = TestCal::new()
        .with_holiday(make_date(2025, 12, 29)) // Monday
        .with_holiday(make_date(2025, 12, 30)); // Tuesday

    let holiday = make_date(2025, 12, 30);
    let adjusted = adjust(holiday, BusinessDayConvention::Preceding, &cal)
        .expect("Preceding adjustment should succeed");
    assert_eq!(adjusted, make_date(2025, 12, 26)); // Friday, skipping weekend + holidays
}

// ModifiedFollowing Convention

#[test]
fn test_adjust_modified_following() {
    let cal = TestCal::new();

    // End of month Saturday that would roll to next month
    let jan25_sat = make_date(2025, 1, 25); // Saturday

    // This should go following (Monday) since it stays in same month
    let result = adjust(jan25_sat, BusinessDayConvention::ModifiedFollowing, &cal).unwrap();
    assert_eq!(result, make_date(2025, 1, 27)); // Monday

    // Test a case where following would cross month boundary
    // January 31, 2025 is a Friday, so let's use January 30 (Thursday) as a holiday
    let cal_with_holiday = TestCal::new()
        .with_holiday(make_date(2025, 1, 30)) // Thursday
        .with_holiday(make_date(2025, 1, 31)); // Friday

    let jan30 = make_date(2025, 1, 30);
    let result = adjust(
        jan30,
        BusinessDayConvention::ModifiedFollowing,
        &cal_with_holiday,
    )
    .unwrap();
    // Following would go to Feb 3 (Monday), but that crosses month, so go preceding to Jan 29
    assert_eq!(result, make_date(2025, 1, 29)); // Wednesday
}

// ModifiedPreceding Convention

#[test]
fn test_adjust_modified_preceding() {
    let cal = TestCal::new().with_holiday(make_date(2025, 2, 3)); // Monday holiday

    // February 1, 2025 is a Saturday
    let feb1_sat = make_date(2025, 2, 1);
    let result = adjust(feb1_sat, BusinessDayConvention::ModifiedPreceding, &cal).unwrap();
    // Preceding would go to Jan 31, but that crosses month, so go following to Feb 4
    assert_eq!(result, make_date(2025, 2, 4)); // Tuesday (since Feb 3 is holiday)
}

#[test]
fn test_adjust_nearest_tie_rolls_following() {
    let cal = TestCal::new().with_holiday(make_date(2025, 1, 1));
    let wednesday = make_date(2025, 1, 1);
    let result = adjust(wednesday, BusinessDayConvention::Nearest, &cal).unwrap();
    assert_eq!(result, make_date(2025, 1, 2));
}

// Available Calendars

#[test]
fn test_available_calendars() {
    let calendars = available_calendars();

    // Should have some calendars
    assert!(!calendars.is_empty());

    // Should contain some expected calendars
    assert!(calendars.contains(&"gblo"));
    assert!(calendars.contains(&"target2"));
}

#[test]
fn available_calendars_count_matches_module_docs() {
    // The module docs at src/dates/mod.rs ("26 built-in market
    // calendars") state the exact number of built-in calendars, generated at
    // build time from core/data/calendars/*.json. If you add or remove a
    // calendar JSON file, update BOTH the doc line and this constant in the
    // same commit — this test exists so the docs cannot drift again.
    const DOCUMENTED_CALENDAR_COUNT: usize = 26;
    assert_eq!(
        available_calendars().len(),
        DOCUMENTED_CALENDAR_COUNT,
        "calendar count changed: update src/dates/calendar/mod.rs module docs \
         and DOCUMENTED_CALENDAR_COUNT together"
    );
}

// Infinite Loop Guard

/// Test calendar that marks ALL days as holidays to trigger infinite loop scenarios.
struct AllHolidaysCal;

impl HolidayCalendar for AllHolidaysCal {
    fn is_holiday(&self, _date: Date) -> finstack_quant_core::Result<bool> {
        Ok({
            true // Every day is a holiday
        })
    }

    fn is_business_day(&self, _date: Date) -> finstack_quant_core::Result<bool> {
        Ok(false)
    }
}

/// With every day closed, each convention searches to the end of the supported
/// date range (there is no closed-day cap) and reports the original date and
/// convention.
#[test]
fn all_conventions_fail_only_at_the_date_range_end_with_original_date() {
    let cal = AllHolidaysCal;
    let original_date = make_date(2025, 1, 1);

    let conventions = [
        BusinessDayConvention::Following,
        BusinessDayConvention::Preceding,
        BusinessDayConvention::ModifiedFollowing,
        BusinessDayConvention::ModifiedPreceding,
        BusinessDayConvention::Nearest,
    ];

    for convention in conventions {
        let result = adjust(original_date, convention, &cal);
        assert!(
            result.is_err(),
            "{:?} should fail on AllHolidaysCal",
            convention
        );

        match result.unwrap_err() {
            finstack_quant_core::Error::Input(InputError::AdjustmentFailed {
                date: err_date,
                convention: err_conv,
            }) => {
                // Verify the error contains the ORIGINAL date (not the last attempted date)
                assert_eq!(
                    err_date, original_date,
                    "{:?}: error should contain original date",
                    convention
                );
                // Verify the error contains the correct convention enum
                assert_eq!(
                    err_conv, convention,
                    "{:?}: error should contain correct convention",
                    convention
                );
            }
            other => panic!(
                "Expected AdjustmentFailed for {:?}, got {:?}",
                convention, other
            ),
        }
    }
}

/// Business only outside `[closed_from, closed_until)`; dates in
/// `[error_from, error_until)` cannot be classified.
struct ClosedSpanCal {
    closed_from: Date,
    closed_until: Date,
    error_span: Option<(Date, Date)>,
}

impl ClosedSpanCal {
    fn new(closed_from: Date, closed_until: Date) -> Self {
        Self {
            closed_from,
            closed_until,
            error_span: None,
        }
    }

    fn with_errors(mut self, from: Date, until: Date) -> Self {
        self.error_span = Some((from, until));
        self
    }
}

impl HolidayCalendar for ClosedSpanCal {
    fn is_holiday(&self, date: Date) -> finstack_quant_core::Result<bool> {
        if let Some((from, until)) = self.error_span
            && (from..until).contains(&date)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "no holiday data for {date}"
            )));
        }
        Ok((self.closed_from..self.closed_until).contains(&date))
    }

    fn is_business_day(&self, date: Date) -> finstack_quant_core::Result<bool> {
        Ok(!self.is_holiday(date)?)
    }
}

#[test]
fn following_searches_through_a_closure_longer_than_100_days() {
    let cal = ClosedSpanCal::new(make_date(2025, 1, 1), make_date(2025, 5, 1));
    assert_eq!(
        adjust(
            make_date(2025, 1, 1),
            BusinessDayConvention::Following,
            &cal
        )
        .expect("closure is searched through"),
        make_date(2025, 5, 1)
    );
}

#[test]
fn modified_following_reverses_out_of_a_fully_closed_month() {
    // March 2025 is closed: Following lands in April, so Modified Following
    // reverses from the original date into February.
    let cal = ClosedSpanCal::new(make_date(2025, 3, 1), make_date(2025, 4, 1));
    assert_eq!(
        adjust(
            make_date(2025, 3, 10),
            BusinessDayConvention::ModifiedFollowing,
            &cal
        )
        .expect("reverse search succeeds"),
        make_date(2025, 2, 28)
    );
}

#[test]
fn modified_following_compares_year_and_month_across_a_year_end() {
    // Closed from 2025-01-15 to 2026-01-20: the first later business day is in
    // January 2026, the same month number a year later, which is still a
    // different month, so the roll reverses into January 2025.
    let cal = ClosedSpanCal::new(make_date(2025, 1, 15), make_date(2026, 1, 20));
    assert_eq!(
        adjust(
            make_date(2025, 1, 20),
            BusinessDayConvention::ModifiedFollowing,
            &cal
        )
        .expect("reverse search succeeds"),
        make_date(2025, 1, 14)
    );
}

#[test]
fn modified_following_does_not_classify_the_next_month() {
    // 2025-01-31 is closed and February has no data: Modified Following must
    // reverse without classifying any February date.
    let cal = ClosedSpanCal::new(make_date(2025, 1, 31), make_date(2025, 2, 1))
        .with_errors(make_date(2025, 2, 1), make_date(2025, 3, 1));
    assert_eq!(
        adjust(
            make_date(2025, 1, 31),
            BusinessDayConvention::ModifiedFollowing,
            &cal
        )
        .expect("no February date is needed"),
        make_date(2025, 1, 30)
    );
}

#[test]
fn nearest_stops_once_the_following_tie_is_proved() {
    // The following day is open at distance one; the preceding day has no
    // data but cannot beat the preferred tie, so it is never classified.
    let cal = ClosedSpanCal::new(make_date(2025, 1, 8), make_date(2025, 1, 9))
        .with_errors(make_date(2025, 1, 7), make_date(2025, 1, 8));
    assert_eq!(
        adjust(make_date(2025, 1, 8), BusinessDayConvention::Nearest, &cal)
            .expect("tie resolved without the preceding date"),
        make_date(2025, 1, 9)
    );
}

#[test]
fn nearest_propagates_missing_data_on_a_needed_date() {
    // Following is closed at distance one, so the preceding day is needed.
    let cal = ClosedSpanCal::new(make_date(2025, 1, 8), make_date(2025, 1, 10))
        .with_errors(make_date(2025, 1, 7), make_date(2025, 1, 8));
    let err = adjust(make_date(2025, 1, 8), BusinessDayConvention::Nearest, &cal)
        .expect_err("a needed date without data is an error");
    assert!(
        matches!(err, finstack_quant_core::Error::Validation(_)),
        "{err:?}"
    );
}

#[test]
fn unadjusted_does_not_classify_the_date() {
    let day = make_date(2025, 1, 8);
    let cal = ClosedSpanCal::new(day, day).with_errors(day, make_date(2025, 1, 9));
    assert_eq!(
        adjust(day, BusinessDayConvention::Unadjusted, &cal).expect("identity"),
        day
    );
}

// Business Day Convention Display

#[test]
fn business_day_convention_display_strings() {
    let values = [
        (BusinessDayConvention::Unadjusted, "unadjusted"),
        (BusinessDayConvention::Following, "following"),
        (
            BusinessDayConvention::ModifiedFollowing,
            "modified_following",
        ),
        (BusinessDayConvention::Preceding, "preceding"),
        (
            BusinessDayConvention::ModifiedPreceding,
            "modified_preceding",
        ),
        (BusinessDayConvention::Nearest, "nearest"),
    ];

    for (conv, expected) in values {
        assert_eq!(format!("{}", conv), expected);
        // Display is the serde name, so it round-trips through FromStr and
        // matches the JSON wire form.
        assert_eq!(conv.to_string().parse::<BusinessDayConvention>(), Ok(conv));
        assert_eq!(
            serde_json::to_string(&conv).expect("serialize"),
            format!("\"{expected}\"")
        );
    }
}

#[test]
fn business_day_convention_parser_accepts_only_normalized_canonical_names() {
    for convention in [
        BusinessDayConvention::Unadjusted,
        BusinessDayConvention::Following,
        BusinessDayConvention::ModifiedFollowing,
        BusinessDayConvention::Preceding,
        BusinessDayConvention::ModifiedPreceding,
        BusinessDayConvention::Nearest,
    ] {
        let name = format!(" {} ", convention.to_string().to_ascii_uppercase());
        assert_eq!(name.parse::<BusinessDayConvention>(), Ok(convention));
    }
    for alias in [
        "MF",
        "mf",
        "ModFollowing",
        "F",
        "P",
        "MP",
        "NONE",
        "modified-following",
    ] {
        assert!(alias.parse::<BusinessDayConvention>().is_err(), "{alias}");
    }
    let err = "bogus"
        .parse::<BusinessDayConvention>()
        .expect_err("unknown code")
        .to_string();
    assert!(err.contains("bogus") && err.contains("modified_following"));
}

// Calendar Metadata

/// Custom calendar that exposes metadata for testing.
struct MetadataCal;

impl HolidayCalendar for MetadataCal {
    fn is_holiday(
        &self,
        date: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<bool> {
        Ok({
            // Treat only January 1st as a holiday to keep behaviour simple.
            date.month() == time::Month::January && date.day() == 1
        })
    }

    fn is_business_day(
        &self,
        date: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<bool> {
        Ok({
            !finstack_quant_core::dates::DateExt::is_weekend(date)
                && !self.is_holiday(date).expect("calendar lookup")
        })
    }

    fn metadata(&self) -> Option<CalendarMetadata> {
        Some(CalendarMetadata {
            id: "meta-cal",
            name: "Metadata Calendar",
            ignore_weekends: false,
        })
    }
}

#[test]
fn calendar_metadata_override_is_respected() {
    let cal = MetadataCal;

    let metadata = cal.metadata().expect("metadata should be present");
    assert_eq!(metadata.id, "meta-cal");
    assert_eq!(metadata.name, "Metadata Calendar");
    assert!(!metadata.ignore_weekends);

    // Non-holiday weekday should be business day.
    let jan2 = make_date(2025, 1, 2);
    assert!(cal.is_business_day(jan2).expect("calendar lookup"));

    // Holiday should not be business day.
    let new_year = make_date(2025, 1, 1);
    assert!(!cal.is_business_day(new_year).expect("calendar lookup"));
}

// Joint business days T+0 behavior

#[test]
fn joint_business_days_t0_returns_start_unadjusted() {
    use finstack_quant_core::dates::{DateExt, resolve_joint_calendar};

    // Saturday Jan 4, 2025 is not a business day on either calendar, but
    // n_days = 0 pins the documented behavior: return start unadjusted.
    let saturday = make_date(2025, 1, 4);
    let result = resolve_joint_calendar(Some("nyse"), Some("gblo"))
        .and_then(|cal| saturday.add_business_days(0, cal))
        .expect("calendars resolve");
    assert_eq!(result, saturday, "T+0 returns start unadjusted");

    // Holiday (Jan 1, 2025, Wednesday) also returned unadjusted at T+0.
    let new_year = make_date(2025, 1, 1);
    let result = resolve_joint_calendar(Some("nyse"), Some("gblo"))
        .and_then(|cal| new_year.add_business_days(0, cal))
        .expect("calendars resolve");
    assert_eq!(result, new_year);
}

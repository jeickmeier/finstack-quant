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
//! Also includes edge cases for consecutive holidays and infinite loop guards.

use super::common::{make_date, TestCal};
use finstack_quant_core::dates::{adjust, available_calendars};
use finstack_quant_core::dates::{BusinessDayConvention, CalendarMetadata, HolidayCalendar};
use finstack_quant_core::InputError;
use time::Date;

// Basic Holiday Calendar Trait

#[test]
fn test_holiday_calendar_trait() {
    let cal = TestCal::new().with_holiday(make_date(2025, 1, 1)); // New Year's Day (Wednesday)

    // Wednesday New Year should be a holiday
    assert!(cal.is_holiday(make_date(2025, 1, 1)));

    // Regular weekday should not be a holiday
    assert!(!cal.is_holiday(make_date(2025, 1, 2))); // Thursday

    // Weekend should not be a business day (handled by trait default)
    assert!(!cal.is_business_day(make_date(2025, 1, 4))); // Saturday
    assert!(!cal.is_business_day(make_date(2025, 1, 5))); // Sunday

    // Holiday should not be a business day
    assert!(!cal.is_business_day(make_date(2025, 1, 1))); // New Year

    // Regular weekday should be a business day
    assert!(cal.is_business_day(make_date(2025, 1, 2))); // Thursday
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
    // The module docs at src/dates/calendar/mod.rs ("27 built-in market
    // calendars") state the exact number of built-in calendars, generated at
    // build time from core/data/calendars/*.json. If you add or remove a
    // calendar JSON file, update BOTH the doc line and this constant in the
    // same commit — this test exists so the docs cannot drift again.
    const DOCUMENTED_CALENDAR_COUNT: usize = 27;
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
    fn is_holiday(&self, _date: Date) -> bool {
        true // Every day is a holiday
    }
}

/// Test that all business day conventions properly guard against infinite loops
/// when a calendar marks all days as holidays, and that error messages contain
/// the original date and correct convention.
#[test]
fn test_all_conventions_infinite_loop_guard_and_error_messages() {
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
                max_days,
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
                // Verify the max_days limit
                assert_eq!(
                    max_days, 100,
                    "{:?}: error should report max_days=100",
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
fn business_day_convention_from_str_accepts_short_codes_case_insensitively() {
    let cases = [
        ("MF", BusinessDayConvention::ModifiedFollowing),
        ("mf", BusinessDayConvention::ModifiedFollowing),
        ("ModFollowing", BusinessDayConvention::ModifiedFollowing),
        ("F", BusinessDayConvention::Following),
        ("P", BusinessDayConvention::Preceding),
        ("MP", BusinessDayConvention::ModifiedPreceding),
        ("NONE", BusinessDayConvention::Unadjusted),
        (
            "Modified_Following",
            BusinessDayConvention::ModifiedFollowing,
        ),
        (" nearest ", BusinessDayConvention::Nearest),
    ];
    for (input, expected) in cases {
        assert_eq!(
            input.parse::<BusinessDayConvention>(),
            Ok(expected),
            "{input}"
        );
    }
    let err = "bogus"
        .parse::<BusinessDayConvention>()
        .expect_err("unknown code");
    assert!(err.contains("bogus") && err.contains("modified_following"));
}

// Calendar Metadata

/// Custom calendar that exposes metadata for testing.
struct MetadataCal;

impl HolidayCalendar for MetadataCal {
    fn is_holiday(&self, date: finstack_quant_core::dates::Date) -> bool {
        // Treat only January 1st as a holiday to keep behaviour simple.
        date.month() == time::Month::January && date.day() == 1
    }

    fn metadata(&self) -> Option<CalendarMetadata> {
        Some(CalendarMetadata {
            id: "meta-cal",
            name: "Metadata Calendar",
            ignore_weekends: false,
            weekend_rule: finstack_quant_core::dates::WeekendRule::default(),
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
    assert!(cal.is_business_day(jan2));

    // Holiday should not be business day.
    let new_year = make_date(2025, 1, 1);
    assert!(!cal.is_business_day(new_year));
}

// Joint business days T+0 behavior

#[test]
fn add_joint_business_days_t0_returns_start_unadjusted() {
    use finstack_quant_core::dates::fx::add_joint_business_days;

    // Saturday Jan 4, 2025 is not a business day on either calendar, but
    // n_days = 0 pins the documented behavior: return start unadjusted.
    let saturday = make_date(2025, 1, 4);
    let result = add_joint_business_days(saturday, 0, Some("nyse"), Some("gblo"), None)
        .expect("calendars resolve");
    assert_eq!(result, saturday, "T+0 returns start unadjusted");

    // Holiday (Jan 1, 2025, Wednesday) also returned unadjusted at T+0.
    let new_year = make_date(2025, 1, 1);
    let result = add_joint_business_days(new_year, 0, Some("nyse"), Some("gblo"), None)
        .expect("calendars resolve");
    assert_eq!(result, new_year);
}

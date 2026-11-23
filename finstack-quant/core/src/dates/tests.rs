use super::*;
use time::Month;

#[test]
fn test_create_date_valid_dates() {
    // Test normal valid dates
    assert!(create_date(2025, Month::January, 15).is_ok());
    assert!(create_date(2024, Month::February, 29).is_ok()); // Leap year
    assert!(create_date(2000, Month::February, 29).is_ok()); // Leap year (century divisible by 400)
    assert!(create_date(2023, Month::December, 31).is_ok());
    assert!(create_date(1970, Month::January, 1).is_ok()); // Unix epoch
}

#[test]
fn test_create_date_invalid_dates() {
    // Test invalid February dates
    let result = create_date(2023, Month::February, 29);
    assert!(result.is_err());
    match result.expect_err("Should fail for invalid date") {
        crate::Error::Input(crate::InputError::InvalidDate { year, month, day }) => {
            assert_eq!(year, 2023);
            assert_eq!(month, 2); // February
            assert_eq!(day, 29);
        }
        _ => panic!("Expected InvalidDate error"),
    }

    // Test non-leap year February 29
    let result = create_date(2023, Month::February, 29);
    assert!(result.is_err());

    // Test invalid day numbers
    assert!(create_date(2023, Month::January, 0).is_err());
    assert!(create_date(2023, Month::January, 32).is_err());
    assert!(create_date(2023, Month::April, 31).is_err()); // April has 30 days
    assert!(create_date(2023, Month::June, 31).is_err()); // June has 30 days

    // Test extreme invalid dates
    assert!(create_date(2023, Month::January, 255).is_err());
}

#[test]
fn test_create_date_error_message_format() {
    let result = create_date(2023, Month::February, 30);
    assert!(result.is_err());

    let error_msg = format!("{}", result.expect_err("Should fail for invalid date"));
    assert!(error_msg.contains("Invalid calendar date"));
    assert!(error_msg.contains("2023-02-30"));
}

#[test]
fn test_create_date_edge_cases() {
    // Test month boundaries
    assert!(create_date(2023, Month::January, 31).is_ok());
    assert!(create_date(2023, Month::March, 31).is_ok());
    assert!(create_date(2023, Month::April, 30).is_ok());
    assert!(create_date(2023, Month::May, 31).is_ok());

    // Test leap year edge cases
    assert!(create_date(1900, Month::February, 28).is_ok()); // Not leap year (century not divisible by 400)
    assert!(create_date(1900, Month::February, 29).is_err()); // Not leap year
    assert!(create_date(2000, Month::February, 29).is_ok()); // Leap year (century divisible by 400)
    assert!(create_date(2100, Month::February, 28).is_ok()); // Not leap year
    assert!(create_date(2100, Month::February, 29).is_err()); // Not leap year
}

#[test]
fn test_create_date_year_boundaries() {
    // Test reasonable year boundaries
    assert!(create_date(-9999, Month::January, 1).is_ok());
    assert!(create_date(9999, Month::December, 31).is_ok());

    // Test year 0 (should be valid in time crate)
    assert!(create_date(0, Month::January, 1).is_ok());
}

#[test]
fn test_create_date_comparison_with_direct_usage() {
    // Verify that create_date produces the same results as direct Date::from_calendar_date for valid dates
    let valid_cases = vec![
        (2023, Month::January, 15),
        (2024, Month::February, 29), // Leap year
        (2023, Month::December, 31),
        (1970, Month::January, 1),
    ];

    for (year, month, day) in valid_cases {
        let direct_result = Date::from_calendar_date(year, month, day);
        let helper_result = create_date(year, month, day);

        assert!(direct_result.is_ok());
        assert!(helper_result.is_ok());
        assert_eq!(
            direct_result.expect("Direct result should succeed in test"),
            helper_result.expect("Helper result should succeed in test")
        );
    }
}

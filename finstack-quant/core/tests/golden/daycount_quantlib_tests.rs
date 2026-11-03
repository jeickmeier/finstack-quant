//! QuantLib parity tests for day count conventions.
//!
//! These tests validate finstack-quant-core day count implementations against
//! QuantLib reference values. The golden test data covers ISDA 2006 Definitions
//! edge cases, particularly:
//!
//! - 30/360 US (Bond Basis / SIA-NASD PSA) Feb EOM rules (QuantLib `Thirty360::USA`;
//!   not ISDA 4.16(f), which leaves February D1 unadjusted)
//! - 30E/360 (Eurobond Basis) per ISDA 4.16(g)
//! - Act/Act ISDA year-boundary splitting per ISDA 4.16(b)
//! - Act/Act ICMA coupon-period basis per ICMA Rule 251
//! - Act/365L leap-year denominator per ICMA Rule 251 (not ACT/ACT AFB)
//!
//! # Reference
//!
//! - QuantLib 1.32
//! - ISDA (2006). "2006 ISDA Definitions." Sections 4.16(b), 4.16(d)-(g).
//! - ICMA (2010). "ICMA Rule Book." Rule 251.

use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Tenor};
use finstack_quant_test_utils::golden::{GoldenSuite, load_suite_from_path};
use finstack_quant_test_utils::golden_path;
use serde::Deserialize;
use time::Month;

/// Expected values for a single day count test case.
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used for documentation/reference in JSON
struct DayCountExpected {
    #[serde(default)]
    days_30_360: Option<i32>,
    year_fraction: Option<f64>,
    tolerance: f64,
    #[serde(default)]
    actual_days: Option<i32>,
    #[serde(default)]
    denominator: Option<i32>,
    // For comparison tests (US vs Euro)
    #[serde(default)]
    us_days: Option<i32>,
    #[serde(default)]
    us_year_fraction: Option<f64>,
    #[serde(default)]
    euro_days: Option<i32>,
    #[serde(default)]
    euro_year_fraction: Option<f64>,
    #[serde(default)]
    difference_days: Option<i32>,
}

/// A single day count test case from the golden fixture.
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // `notes` field used for documentation in JSON
struct DayCountCase {
    id: String,
    description: String,
    convention: String,
    start_date: String,
    end_date: String,
    expected: DayCountExpected,
    #[serde(default)]
    notes: Option<String>,
    /// Coupon frequency for ACT/ACT ICMA convention (e.g., "6M", "3M", "1Y", "1M")
    #[serde(default)]
    frequency: Option<String>,
    /// Regular coupon period `[start, end)` for irregular ACT/ACT ICMA stubs.
    #[serde(default)]
    coupon_period: Option<(String, String)>,
}

/// Parse a date string in "YYYY-MM-DD" format.
fn parse_date(s: &str) -> Date {
    let parts: Vec<&str> = s.split('-').collect();
    assert_eq!(
        parts.len(),
        3,
        "Invalid date format '{}': expected YYYY-MM-DD",
        s
    );
    let year: i32 = parts[0]
        .parse()
        .unwrap_or_else(|_| panic!("Invalid year in date '{}'", s));
    let month: u8 = parts[1]
        .parse()
        .unwrap_or_else(|_| panic!("Invalid month in date '{}'", s));
    let day: u8 = parts[2]
        .parse()
        .unwrap_or_else(|_| panic!("Invalid day in date '{}'", s));
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day)
        .unwrap_or_else(|_| panic!("Invalid calendar date '{}'", s))
}

/// Map convention string to DayCount enum.
fn parse_convention(s: &str) -> Option<DayCount> {
    match s {
        "30_360" => Some(DayCount::Thirty360),
        "30e_360" => Some(DayCount::ThirtyE360),
        "act_act" => Some(DayCount::ActAct),
        "act_act_isma" => Some(DayCount::ActActIsma),
        "act_365l" => Some(DayCount::Act365L),
        "act_360" => Some(DayCount::Act360),
        "act_365f" => Some(DayCount::Act365F),
        "act_act_afb" => Some(DayCount::ActActAfb),
        "30_360_it" => Some(DayCount::Thirty360It),
        "comparison" => None, // Special case for comparison tests
        _ => panic!("Unknown convention: {}", s),
    }
}

/// Parse frequency string to Tenor (e.g., "6M" -> semi-annual, "3M" -> quarterly).
fn parse_frequency(s: &str) -> Option<Tenor> {
    match s {
        "1M" => Some(Tenor::monthly()),
        "3M" => Some(Tenor::quarterly()),
        "6M" => Some(Tenor::semi_annual()),
        "1Y" => Some(Tenor::annual()),
        _ => None,
    }
}

#[test]
fn test_daycount_quantlib_parity() {
    let path = golden_path!("data/daycount_quantlib.json");
    let suite: GoldenSuite<DayCountCase> =
        load_suite_from_path(path).expect("Failed to load daycount golden suite");

    assert_eq!(suite.meta.suite_id, "daycount_quantlib_parity");
    assert!(!suite.cases.is_empty(), "Suite should have test cases");

    let mut failures: Vec<String> = Vec::new();

    for case in &suite.cases {
        let start = parse_date(&case.start_date);
        let end = parse_date(&case.end_date);

        let result = if case.convention == "comparison" {
            // Special comparison test: US vs Euro
            test_us_vs_euro_comparison(case, start, end)
        } else {
            let day_count = parse_convention(&case.convention).expect("Valid convention");
            test_single_convention(case, day_count, start, end)
        };

        if let Err(e) = result {
            failures.push(format!("[{}] {}: {}", case.id, case.description, e));
        }
    }

    assert!(
        failures.is_empty(),
        "QuantLib parity test failures ({}):\n{}",
        failures.len(),
        failures.join("\n")
    )
}

/// Test a single day count convention.
fn test_single_convention(
    case: &DayCountCase,
    day_count: DayCount,
    start: Date,
    end: Date,
) -> Result<(), String> {
    // Build context with frequency if ACT/ACT ICMA
    let frequency = case.frequency.as_deref().and_then(parse_frequency);
    let coupon_period = case
        .coupon_period
        .as_ref()
        .map(|(s, e)| (parse_date(s), parse_date(e)));
    let ctx = DayCountContext {
        calendar: None,
        frequency,
        bus_basis: None,
        coupon_period,
        end_is_termination_date: false,
    };

    let yf = day_count
        .year_fraction(start, end, ctx)
        .map_err(|e| format!("year_fraction failed: {}", e))?;

    let tol = case.expected.tolerance;

    if let Some(expected_yf) = case.expected.year_fraction
        && (yf - expected_yf).abs() > tol
    {
        return Err(format!(
            "year_fraction mismatch: expected {:.15}, got {:.15} (diff: {:.2e})",
            expected_yf,
            yf,
            (yf - expected_yf).abs()
        ));
    }

    Ok(())
}

/// Test comparison between 30/360 US and 30E/360.
fn test_us_vs_euro_comparison(case: &DayCountCase, start: Date, end: Date) -> Result<(), String> {
    let ctx = DayCountContext::default();
    let tol = case.expected.tolerance;

    let us_yf = DayCount::Thirty360
        .year_fraction(start, end, ctx)
        .map_err(|e| format!("US year_fraction failed: {}", e))?;

    let euro_yf = DayCount::ThirtyE360
        .year_fraction(start, end, ctx)
        .map_err(|e| format!("Euro year_fraction failed: {}", e))?;

    if let Some(expected_us) = case.expected.us_year_fraction
        && (us_yf - expected_us).abs() > tol
    {
        return Err(format!(
            "US year_fraction mismatch: expected {:.15}, got {:.15}",
            expected_us, us_yf
        ));
    }

    if let Some(expected_euro) = case.expected.euro_year_fraction
        && (euro_yf - expected_euro).abs() > tol
    {
        return Err(format!(
            "Euro year_fraction mismatch: expected {:.15}, got {:.15}",
            expected_euro, euro_yf
        ));
    }

    // Verify the difference
    if let Some(expected_diff) = case.expected.difference_days {
        let actual_diff_days = (us_yf - euro_yf) * 360.0;
        if (actual_diff_days - expected_diff as f64).abs() > 0.01 {
            return Err(format!(
                "US-Euro difference mismatch: expected {} days, got {:.4} days",
                expected_diff, actual_diff_days
            ));
        }
    }

    Ok(())
}

/// Test Act/Act ISDA year boundary splitting.
/// This is critical for swap valuation accuracy.
#[test]
fn test_actact_isda_year_boundary_splitting() {
    let ctx = DayCountContext::default();

    // Period spanning from leap year to non-leap year
    let start = Date::from_calendar_date(2024, Month::July, 1).unwrap();
    let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    let yf = DayCount::ActAct.year_fraction(start, end, ctx).unwrap();

    // Expected: 184/366 (H2 2024) + 365/365 (full 2025)
    // = 0.5027322404... + 1.0 = 1.5027322404...
    let expected = 184.0 / 366.0 + 1.0;

    assert!(
        (yf - expected).abs() < 1e-12,
        "Act/Act ISDA spanning years: expected {:.15}, got {:.15}",
        expected,
        yf
    );
}

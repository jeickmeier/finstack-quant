//! Calendar implementation using rule-based evaluation with bitset optimization.
//!
//! The `Calendar` struct provides a clean, efficient implementation supporting:
//! - Rule-based holiday definition (see [`super::rule::Rule`])
//! - Precomputed bitsets for fast lookup (1970-2150)
//! - Fallback to rule evaluation outside bitset range

use super::business_days::{CalendarMetadata, HolidayCalendar};
use super::generated::{bit_test, day_of_year_0_based, YearBits, BASE_YEAR, END_YEAR};
use super::rule::Rule;
use crate::dates::DateExt;
use time::{Date, Weekday};

/// Weekend convention for a calendar jurisdiction.
///
/// Most global markets observe Saturday/Sunday as the weekend, but Middle Eastern
/// markets (SAR, AED, QAR, BHD, OMR, KWD) observe Friday/Saturday. Crypto and
/// other 24/7 markets may have no weekends at all.
#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WeekendRule {
    /// Saturday and Sunday (default -- Western markets).
    #[default]
    SaturdaySunday,
    /// Friday and Saturday (Middle East -- SAR, AED, QAR, BHD, OMR, KWD).
    FridaySaturday,
    /// Friday only.
    FridayOnly,
    /// No weekends (24/7 markets, e.g., crypto).
    None,
}

impl WeekendRule {
    /// Returns `true` if the given weekday is a weekend day under this rule.
    #[inline]
    pub const fn is_weekend(self, weekday: Weekday) -> bool {
        match self {
            Self::SaturdaySunday => matches!(weekday, Weekday::Saturday | Weekday::Sunday),
            Self::FridaySaturday => matches!(weekday, Weekday::Friday | Weekday::Saturday),
            Self::FridayOnly => matches!(weekday, Weekday::Friday),
            Self::None => false,
        }
    }
}

/// A holiday calendar implementation that uses rule-based evaluation
/// with optional precomputed bitsets for performance.
#[derive(Debug, Clone, Copy)]
pub struct Calendar {
    /// Unique identifier (e.g., "target2", "gblo")
    pub id: &'static str,

    /// Display name for the calendar
    pub name: &'static str,

    /// Whether weekends should be ignored in is_holiday checks
    pub ignore_weekends: bool,

    /// The rules defining this calendar's holidays
    pub rules: &'static [Rule],

    /// Optional precomputed bitsets for fast lookup (1970-2150)
    pub bitsets: Option<&'static [YearBits]>,

    /// Weekend convention for this calendar's jurisdiction.
    ///
    /// Defaults to [`WeekendRule::SaturdaySunday`]. Middle Eastern calendars
    /// should use [`WeekendRule::FridaySaturday`].
    pub weekend_rule: WeekendRule,
}

impl Calendar {
    /// Create a new calendar with the given parameters.
    ///
    /// Defaults to [`WeekendRule::SaturdaySunday`]. Use
    /// [`with_weekend_rule`](Self::with_weekend_rule) to override.
    pub const fn new(
        id: &'static str,
        name: &'static str,
        ignore_weekends: bool,
        rules: &'static [Rule],
    ) -> Self {
        Self {
            id,
            name,
            ignore_weekends,
            rules,
            bitsets: None,
            weekend_rule: WeekendRule::SaturdaySunday,
        }
    }

    /// Add precomputed bitsets to this calendar for fast lookup.
    pub const fn with_bitsets(mut self, bitsets: &'static [YearBits]) -> Self {
        self.bitsets = Some(bitsets);
        self
    }

    /// Override the weekend convention for this calendar.
    pub const fn with_weekend_rule(mut self, rule: WeekendRule) -> Self {
        self.weekend_rule = rule;
        self
    }

    /// Get the calendar identifier.
    pub const fn id(&self) -> &'static str {
        self.id
    }

    /// Get the calendar display name.
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

impl HolidayCalendar for Calendar {
    fn is_holiday(&self, date: Date) -> bool {
        // Use fast bitset lookup if available and date is in range
        if let Some(bitsets) = self.bitsets {
            if (BASE_YEAR..=END_YEAR).contains(&date.year()) {
                let year_idx = (date.year() - BASE_YEAR) as usize;
                if year_idx < bitsets.len() {
                    let day_idx = day_of_year_0_based(date);
                    let mut is_holiday = bit_test(&bitsets[year_idx], day_idx);

                    // Apply weekend ignore logic
                    if self.ignore_weekends && date.is_weekend() {
                        is_holiday = false;
                    }

                    return is_holiday;
                }
            }
        }

        // Fall back to rule-based evaluation for dates outside bitset range
        // or when bitsets are not available
        if !(BASE_YEAR..=END_YEAR).contains(&date.year()) {
            // Emit a one-time warning per process when falling back
            static ONCE: core::sync::atomic::AtomicBool =
                core::sync::atomic::AtomicBool::new(false);
            if !ONCE.swap(true, core::sync::atomic::Ordering::Relaxed) {
                tracing::warn!(
                    calendar = self.id,
                    year = date.year(),
                    base_year = BASE_YEAR,
                    end_year = END_YEAR,
                    "calendar falling back to rule-based evaluation outside bitset range"
                );
            }
        }
        let mut is_holiday = self.rules.iter().any(|rule| rule.applies(date));

        // Apply weekend ignore logic using the calendar's own weekend rule
        // (not a hardcoded Sat/Sun) so Fri/Sat-weekend calendars agree with
        // `is_business_day`.
        if self.ignore_weekends && self.weekend_rule.is_weekend(date.weekday()) {
            is_holiday = false;
        }

        is_holiday
    }

    fn is_business_day(&self, date: Date) -> bool {
        !self.weekend_rule.is_weekend(date.weekday()) && !self.is_holiday(date)
    }

    fn metadata(&self) -> Option<CalendarMetadata> {
        Some(CalendarMetadata {
            id: self.id,
            name: self.name,
            ignore_weekends: self.ignore_weekends,
            weekend_rule: self.weekend_rule,
        })
    }
}

// Also implement HolidayCalendar for &Calendar for convenience
impl HolidayCalendar for &Calendar {
    fn is_holiday(&self, date: Date) -> bool {
        (*self).is_holiday(date)
    }

    fn metadata(&self) -> Option<CalendarMetadata> {
        (*self).metadata()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    // ------------------------------------------------------------------
    // WeekendRule::is_weekend unit tests
    // ------------------------------------------------------------------

    #[test]
    fn saturday_sunday_rule() {
        let rule = WeekendRule::SaturdaySunday;
        assert!(!rule.is_weekend(Weekday::Monday));
        assert!(!rule.is_weekend(Weekday::Tuesday));
        assert!(!rule.is_weekend(Weekday::Wednesday));
        assert!(!rule.is_weekend(Weekday::Thursday));
        assert!(!rule.is_weekend(Weekday::Friday));
        assert!(rule.is_weekend(Weekday::Saturday));
        assert!(rule.is_weekend(Weekday::Sunday));
    }

    #[test]
    fn friday_saturday_rule() {
        let rule = WeekendRule::FridaySaturday;
        assert!(!rule.is_weekend(Weekday::Monday));
        assert!(!rule.is_weekend(Weekday::Tuesday));
        assert!(!rule.is_weekend(Weekday::Wednesday));
        assert!(!rule.is_weekend(Weekday::Thursday));
        assert!(rule.is_weekend(Weekday::Friday));
        assert!(rule.is_weekend(Weekday::Saturday));
        assert!(!rule.is_weekend(Weekday::Sunday));
    }

    #[test]
    fn friday_only_rule() {
        let rule = WeekendRule::FridayOnly;
        assert!(!rule.is_weekend(Weekday::Thursday));
        assert!(rule.is_weekend(Weekday::Friday));
        assert!(!rule.is_weekend(Weekday::Saturday));
    }

    #[test]
    fn none_rule() {
        let rule = WeekendRule::None;
        for wd in [
            Weekday::Monday,
            Weekday::Tuesday,
            Weekday::Wednesday,
            Weekday::Thursday,
            Weekday::Friday,
            Weekday::Saturday,
            Weekday::Sunday,
        ] {
            assert!(!rule.is_weekend(wd));
        }
    }

    #[test]
    fn default_is_saturday_sunday() {
        assert_eq!(WeekendRule::default(), WeekendRule::SaturdaySunday);
    }

    // ------------------------------------------------------------------
    // Calendar with FridaySaturday weekend rule
    // ------------------------------------------------------------------

    #[test]
    fn friday_saturday_calendar_business_days() {
        let cal = Calendar::new("me_test", "Middle East Test", true, &[])
            .with_weekend_rule(WeekendRule::FridaySaturday);

        // 2025-01-03 is a Friday
        let friday = Date::from_calendar_date(2025, Month::January, 3).expect("Valid test date");
        // 2025-01-04 is a Saturday
        let saturday = Date::from_calendar_date(2025, Month::January, 4).expect("Valid test date");
        // 2025-01-05 is a Sunday
        let sunday = Date::from_calendar_date(2025, Month::January, 5).expect("Valid test date");
        // 2025-01-06 is a Monday
        let monday = Date::from_calendar_date(2025, Month::January, 6).expect("Valid test date");

        // Friday and Saturday are weekend under FridaySaturday rule
        assert!(
            !cal.is_business_day(friday),
            "Friday should NOT be a business day under FridaySaturday rule"
        );
        assert!(
            !cal.is_business_day(saturday),
            "Saturday should NOT be a business day under FridaySaturday rule"
        );

        // Sunday IS a business day under FridaySaturday rule
        assert!(
            cal.is_business_day(sunday),
            "Sunday SHOULD be a business day under FridaySaturday rule"
        );
        assert!(
            cal.is_business_day(monday),
            "Monday SHOULD be a business day under FridaySaturday rule"
        );
    }

    #[test]
    fn default_calendar_preserves_sat_sun_weekends() {
        // Existing calendars default to SaturdaySunday -- verify backward compat
        let cal = Calendar::new("compat_test", "Compat Test", true, &[]);
        assert_eq!(cal.weekend_rule, WeekendRule::SaturdaySunday);

        let saturday = Date::from_calendar_date(2025, Month::January, 4).expect("Valid test date");
        let sunday = Date::from_calendar_date(2025, Month::January, 5).expect("Valid test date");
        let friday = Date::from_calendar_date(2025, Month::January, 3).expect("Valid test date");

        assert!(!cal.is_business_day(saturday));
        assert!(!cal.is_business_day(sunday));
        assert!(cal.is_business_day(friday));
    }

    #[test]
    fn no_weekend_rule_makes_all_days_business_days() {
        let cal = Calendar::new("crypto_test", "Crypto Test", true, &[])
            .with_weekend_rule(WeekendRule::None);

        // Every day of the week is a business day
        for day in 3..=9u8 {
            let d = Date::from_calendar_date(2025, Month::January, day).expect("Valid test date");
            assert!(
                cal.is_business_day(d),
                "{d} should be a business day under None weekend rule"
            );
        }
    }

    #[test]
    fn weekend_rule_serde_roundtrip() {
        let rules = [
            WeekendRule::SaturdaySunday,
            WeekendRule::FridaySaturday,
            WeekendRule::FridayOnly,
            WeekendRule::None,
        ];
        for rule in rules {
            let json =
                serde_json::to_string(&rule).expect("JSON serialization should succeed in test");
            let deserialized: WeekendRule =
                serde_json::from_str(&json).expect("JSON deserialization should succeed in test");
            assert_eq!(rule, deserialized);
        }
    }

    #[test]
    fn weekend_rule_serde_snake_case() {
        assert_eq!(
            serde_json::to_string(&WeekendRule::SaturdaySunday)
                .expect("JSON serialization should succeed in test"),
            "\"saturday_sunday\""
        );
        assert_eq!(
            serde_json::to_string(&WeekendRule::FridaySaturday)
                .expect("JSON serialization should succeed in test"),
            "\"friday_saturday\""
        );
        assert_eq!(
            serde_json::to_string(&WeekendRule::FridayOnly)
                .expect("JSON serialization should succeed in test"),
            "\"friday_only\""
        );
        assert_eq!(
            serde_json::to_string(&WeekendRule::None)
                .expect("JSON serialization should succeed in test"),
            "\"none\""
        );
    }
}

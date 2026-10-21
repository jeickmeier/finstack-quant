//! Holiday calendar system for financial markets: generated market calendars,
//! rule evaluation, the year cache and `+`-joined composite calendars.
//!
//! Public callers reach calendars by id through [`super::calendar_by_id_strict`];
//! see the [`super`] module docs for the calendar model and coverage.

pub(crate) mod algo;
pub(crate) mod business_days;
pub(crate) mod composite;
mod jurisdictions;
pub(crate) mod rule;
pub(crate) mod types;
mod year_cache;

#[cfg(test)]
mod rule_coverage_tests;
#[cfg(test)]
mod rule_tests;

// Isolate generated `use` imports from this module's public re-exports.
pub(super) mod calendars_generated {
    include!(concat!(env!("OUT_DIR"), "/calendars.rs"));
}

pub use calendars_generated::*;

/// Identifier of the holiday-free [`WEEKENDS_ONLY`] calendar.
///
/// Use it wherever a calendar id is required but only Saturday/Sunday
/// adjustment applies; [`calendar_by_id_strict`](business_days::calendar_by_id_strict)
/// resolves it to [`WEEKENDS_ONLY`].
pub const WEEKENDS_ONLY_ID: &str = "weekends_only";

#[cfg(test)]
mod weekends_only_id_tests {
    #[test]
    fn id_matches_the_generated_calendar() {
        assert_eq!(super::WEEKENDS_ONLY.id, super::WEEKENDS_ONLY_ID);
    }
}

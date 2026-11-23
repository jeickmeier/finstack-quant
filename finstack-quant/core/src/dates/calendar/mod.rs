//! Holiday calendar system for financial markets: generated market calendars,
//! rule evaluation, the year cache and `+`-joined composite calendars.
//!
//! Public callers reach calendars by id through [`super::calendar_by_id_strict`];
//! see the [`super`] module docs for the calendar model and coverage.

pub(crate) mod algo;
pub(crate) mod business_days;
pub(crate) mod calendars_generated;
pub(crate) mod composite;
mod jurisdictions;
pub(crate) mod rule;
pub(crate) mod types;
mod year_cache;

#[cfg(test)]
#[path = "tests/rule_coverage.rs"]
mod rule_coverage_tests;
#[cfg(test)]
#[path = "tests/rule.rs"]
mod rule_tests;

// Crate-internal access to the generated market calendars for the tests.
#[cfg(test)]
pub(crate) use calendars_generated::*;

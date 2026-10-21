//! Dates module integration tests.
//!
//! This test suite verifies market-standard correctness for:
//! - Calendar holiday calculations (USNY, TARGET2, NYSE, GBLO, HKHK, etc.)
//! - Business day conventions (Following, ModifiedFollowing, etc.)
//! - Day count conventions (30/360, Act/360, Act/365, Act/Act, Bus/252)
//! - Schedule generation with stub rules and EOM conventions
//! - Date extensions (fiscal year, quarters, weekdays)
//!
//! # Test Organization
//!
//! - `common`: Shared test helpers (TestCal, make_date, DAYCOUNT_TOLERANCE)
//! - `calendars`: Built-in regional calendar holiday tests
//! - `adjustment`: Business day convention tests
//! - `registry`: free calendar lookup and resolution tests
//! - `generated`: Low-level bitset and helper tests
//! - `daycount`: Day count convention tests
//! - `schedule`: Schedule generation tests
//! - `extensions`: DateExt trait tests

#[path = "dates/common.rs"]
mod common;

// Calendar Infrastructure
#[path = "dates/adjustment.rs"]
mod adjustment;

#[path = "dates/calendars.rs"]
mod calendars;

#[path = "dates/generated.rs"]
mod generated;

#[path = "dates/registry.rs"]
mod registry;

// Day Count Conventions
#[path = "dates/daycount.rs"]
mod daycount;

// Schedules & Utilities
#[path = "dates/extensions.rs"]
mod extensions;

#[path = "dates/schedule.rs"]
mod schedule;

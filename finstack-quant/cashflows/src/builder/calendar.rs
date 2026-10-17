//! Calendar helpers for cashflow scheduling.
//!
//! Calendar ids resolve through `finstack_quant_core::dates::calendar_by_id_strict`,
//! which also serves the holiday-free `"weekends_only"` calendar.

/// Canonical ID for the weekends-only calendar.
pub const WEEKENDS_ONLY_ID: &str = "weekends_only";

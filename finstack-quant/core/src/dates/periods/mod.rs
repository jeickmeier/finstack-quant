//! Period system for financial statement and time-series modeling.
//!
//! Provides types and parsers for working with financial reporting periods
//! (quarters, months, years, etc.) commonly used in financial statement models
//! and forecast scenarios.
//!
//! # Features
//!
//! - Period identifiers: Q1-Q4, M01-M12, H1-H2, W01-W52, annual
//! - Range expressions: "2025Q1..Q4", "2024M06..2025M06"
//! - Fiscal year support with custom month offsets
//! - Actual vs forecast period tracking
//!
//! # Period Formats
//!
//! - **Quarterly**: 2025Q1, 2025Q2, 2025Q3, 2025Q4
//! - **Monthly**: 2025M01 through 2025M12
//! - **Semi-annual**: 2025H1, 2025H2
//! - **Weekly**: 2025W01 through 2025W52/53 (ISO 8601 week-year)
//! - **Annual**: 2025

pub(crate) mod calendar;
pub(crate) mod ltm;
mod parse;
pub(crate) mod period_id;
pub(crate) mod plan;
#[cfg(test)]
mod tests;

pub(crate) use calendar::PeriodCalendar;
#[cfg(test)]
pub(super) use ltm::ltm_window;
pub(crate) use period_id::{PeriodId, PeriodKind};
pub(crate) use plan::{FiscalConfig, Period, PeriodPlan};
#[cfg(test)]
pub(super) use plan::{build_fiscal_periods, build_periods};

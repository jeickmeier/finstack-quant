//! Market benchmark validation tests.
//!
//! Tests validating IRS implementation against:
//! - Hull, "Options, Futures, and Other Derivatives"
//! - ISDA documentation and market standards
//! - Known market formulas and conventions
//!
//! Includes:
//! - `market_standards`: Par rate, annuity, DV01 market standard tests
//! - `negative_rates`: Negative interest rate scenarios (EUR, CHF style)
//! - `numerical_stability`: Kahan summation, input validation, edge cases

mod market_standards;
mod negative_rates;
mod numerical_stability;

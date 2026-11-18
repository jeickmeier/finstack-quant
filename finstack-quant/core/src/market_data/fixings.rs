//! Shared utilities for historical rate fixing lookups.
//!
//! Fixings are stored as [`crate::market_data::scalars::ScalarTimeSeries`] in
//! [`crate::market_data::context::MarketContext`] using the convention
//! `FIXING:{index_id}` built from the observed index identifier (never from
//! a swapped projection curve or tenor approximation). This module centralizes
//! that convention and provides helpers with clear diagnostics for seasoned
//! instrument pricing.

use crate::Result;
use crate::dates::Date;
use crate::market_data::context::MarketContext;
use crate::market_data::scalars::ScalarTimeSeries;

/// Canonical prefix for fixing series stored in MarketContext.
pub const FIXING_PREFIX: &str = "FIXING:";

/// Check the identifier of a contractual observed rate before constructing a fixing key.
///
/// # Arguments
///
/// * `index_id` - Trimmed, nonempty benchmark name without the reserved FIXING prefix.
pub fn validate_fixing_index_id(index_id: &str) -> Result<&str> {
    if index_id.trim().is_empty() || index_id.trim() != index_id {
        return Err(crate::Error::Validation(
            "Fixing index identifier must be non-empty and trimmed".into(),
        ));
    }
    if index_id.starts_with(FIXING_PREFIX) {
        return Err(crate::Error::Validation(format!(
            "Fixing index identifier '{index_id}' must not already carry the '{FIXING_PREFIX}' prefix"
        )));
    }
    Ok(index_id)
}

/// Build the canonical series ID for a given observed rate/benchmark index.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::market_data::fixings::fixing_series_id;
/// assert_eq!(fixing_series_id("USD-SOFR"), "FIXING:USD-SOFR");
/// ```
///
/// # Arguments
///
/// * `index_id` - Identifier of the observed rate or benchmark index whose
///   historical fixing series is required (without the `FIXING:` prefix).
pub fn fixing_series_id(index_id: &str) -> String {
    format!("{}{}", FIXING_PREFIX, index_id)
}

/// Look up the fixing series for a rate index in MarketContext.
///
/// Returns a clear error when `index_id` is invalid or the series is missing,
/// directing the user to provide the expected `ScalarTimeSeries`. The lookup
/// uses only the canonical `FIXING:{index_id}` ID; it does not fall back to an
/// unqualified series or a forward curve projection.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] if `index_id` is empty/untrimmed or
/// already prefixed with `FIXING:`, or if `context` contains no scalar series
/// with the canonical ID (preserving the underlying lookup error cause).
///
/// # Arguments
///
/// * `context` - Market context containing scalar time series keyed by their
///   canonical fixing IDs.
/// * `index_id` - Observed rate-index identifier used to build the required
///   `FIXING:{index_id}` series key.
pub fn get_fixing_series<'a>(
    context: &'a MarketContext,
    index_id: &str,
) -> Result<&'a ScalarTimeSeries> {
    let index_id = validate_fixing_index_id(index_id)?;
    let id = fixing_series_id(index_id);
    context.get_series(&id).map_err(|err| {
        crate::Error::Validation(format!(
            "No fixing series found for index '{index_id}'. \
             Seasoned instruments require a ScalarTimeSeries with id '{id}' \
             containing historical observations for dates before the valuation date: {err}"
        ))
    })
}

/// Require a fixing value using exact-date matching and exact series identity.
///
/// Fails if `index_id` is invalid, no series is provided, `series.id()` does
/// not equal `FIXING:{index_id}`, or no observation exists on `date`.
/// Appropriate for term rate fixings (e.g., 3M LIBOR resets) where carrying a
/// previous publication forward or consuming an unrelated series would alter
/// the contractual reset.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when `index_id` is empty/untrimmed or
/// already prefixed with `FIXING:`, when `series` is `None`, when `series.id()`
/// does not match `FIXING:{index_id}`, or when the series lacks an exact
/// observation on `date` (preserving the lookup error cause).
///
/// # Arguments
///
/// * `series` - Optional resolved historical fixing series. `None` produces a
///   diagnostic that identifies the required canonical series ID.
/// * `index_id` - Observed rate/benchmark index identifier whose canonical
///   `FIXING:{index_id}` key must match `series.id()`.
/// * `date` - Contractual fixing date that must have an exact observation.
pub fn require_fixing_value_exact(
    series: Option<&ScalarTimeSeries>,
    index_id: &str,
    date: Date,
) -> Result<f64> {
    let index_id = validate_fixing_index_id(index_id)?;
    let expected_id = fixing_series_id(index_id);
    let s = series.ok_or_else(|| {
        crate::Error::Validation(format!(
            "Seasoned instrument requires fixings for index '{index_id}' on {date}. \
             Provide a ScalarTimeSeries with id '{expected_id}'."
        ))
    })?;
    if s.id().as_str() != expected_id {
        return Err(crate::Error::Validation(format!(
            "Fixing series identity mismatch for index '{index_id}': expected id '{expected_id}', got '{}'",
            s.id().as_str()
        )));
    }
    s.get_value_exact(date).map_err(|e| {
        crate::Error::Validation(format!(
            "Missing fixing for '{index_id}' on {date} in series '{expected_id}': no exact observation: {e}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_data::scalars::{ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation};
    use time::macros::date;

    fn sample_series() -> ScalarTimeSeries {
        ScalarTimeSeries::new(
            "FIXING:USD-SOFR",
            vec![
                (date!(2024 - 01 - 02), 0.053),
                (date!(2024 - 01 - 03), 0.054),
                (date!(2024 - 01 - 05), 0.052),
            ],
            None,
            SeriesInterpolation::Step,
            SeriesExtrapolation::Error,
        )
        .expect("valid series")
    }

    #[test]
    fn fixing_series_id_builds_correct_key() {
        assert_eq!(fixing_series_id("USD-SOFR"), "FIXING:USD-SOFR");
        assert_eq!(fixing_series_id("EUR-ESTR"), "FIXING:EUR-ESTR");
    }

    #[test]
    fn get_fixing_series_returns_series_when_present() {
        let series = sample_series();
        let ctx = MarketContext::new().insert_series(series);
        let result = get_fixing_series(&ctx, "USD-SOFR");
        assert!(result.is_ok());
    }

    #[test]
    fn get_fixing_series_errors_when_missing_or_invalid_id() {
        let ctx = MarketContext::new();
        let result = get_fixing_series(&ctx, "USD-SOFR");
        assert!(result.is_err());
        let msg = result.expect_err("should error").to_string();
        assert!(
            msg.contains("FIXING:USD-SOFR"),
            "error should mention series id: {msg}"
        );
        assert!(
            msg.contains("USD-SOFR"),
            "error should mention index: {msg}"
        );

        for bad_id in ["", "  ", " USD-SOFR", "FIXING:USD-SOFR"] {
            assert!(get_fixing_series(&ctx, bad_id).is_err());
        }
    }

    #[test]
    fn require_fixing_value_exact_returns_rate_on_observed_date() {
        let series = sample_series();
        let rate = require_fixing_value_exact(Some(&series), "USD-SOFR", date!(2024 - 01 - 03))
            .expect("exact date exists");
        assert!((rate - 0.054).abs() < 1e-10);
    }

    #[test]
    fn require_fixing_value_exact_rejects_mismatched_series_identity() {
        let wrong = ScalarTimeSeries::new(
            "FIXING:EUR-ESTR",
            vec![(date!(2024 - 01 - 03), 0.04)],
            None,
            SeriesInterpolation::Step,
            SeriesExtrapolation::Error,
        )
        .expect("valid series");
        assert!(
            require_fixing_value_exact(Some(&wrong), "USD-SOFR", date!(2024 - 01 - 03)).is_err()
        );
    }

    #[test]
    fn require_fixing_value_exact_errors_on_unobserved_date() {
        let series = sample_series();
        let result = require_fixing_value_exact(
            Some(&series),
            "USD-SOFR",
            date!(2024 - 01 - 04), // Not observed
        );
        assert!(result.is_err());
        let msg = result.expect_err("should error").to_string();
        assert!(msg.contains("2024-01-04"), "should mention date: {msg}");
    }

    #[test]
    fn require_fixing_value_exact_errors_when_series_is_none() {
        let result = require_fixing_value_exact(None, "USD-SOFR", date!(2024 - 01 - 02));
        assert!(result.is_err());
        let msg = result.expect_err("should error").to_string();
        assert!(
            msg.contains("FIXING:USD-SOFR"),
            "should mention series id: {msg}"
        );
    }
}

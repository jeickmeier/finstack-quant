//! Shared utilities for historical rate fixing lookups.
//!
//! Fixings are stored as [`crate::market_data::scalars::ScalarTimeSeries`] in
//! [`crate::market_data::context::MarketContext`] using the convention
//! `FIXING:{forward_curve_id}`. This module centralizes that convention and
//! provides helpers with clear error messages for seasoned instrument pricing.

use crate::dates::Date;
use crate::market_data::context::MarketContext;
use crate::market_data::scalars::ScalarTimeSeries;
use crate::Result;

/// Canonical prefix for fixing series stored in MarketContext.
pub const FIXING_PREFIX: &str = "FIXING:";

/// Build the canonical series ID for a given forward curve / rate index.
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
/// * `forward_curve_id` - Identifier of the rate index or forward curve whose
///   historical fixing series is required.
pub fn fixing_series_id(forward_curve_id: &str) -> String {
    format!("{}{}", FIXING_PREFIX, forward_curve_id)
}

/// Build the canonical series ID for CMS (par swap rate) fixings of a given
/// reference tenor projected off a given forward curve.
///
/// CMS fixings are par swap rates, not the forward curve's own IBOR/RFR
/// fixings, so the series is qualified by the swap tenor:
/// `FIXING:CMS-{tenor}:{forward_curve_id}`.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::market_data::fixings::cms_fixing_series_id;
/// assert_eq!(cms_fixing_series_id("USD-SOFR", 10.0), "FIXING:CMS-10Y:USD-SOFR");
/// assert_eq!(cms_fixing_series_id("USD-SOFR", 0.5), "FIXING:CMS-6M:USD-SOFR");
/// ```
///
/// # Arguments
///
/// * `forward_curve_id` - Identifier of the curve or rate index used to
///   project the CMS reference swap rate.
/// * `tenor_years` - Reference swap tenor in years. Exact whole-month values
///   are rendered in the canonical month or year form.
pub fn cms_fixing_series_id(forward_curve_id: &str, tenor_years: f64) -> String {
    format!(
        "{FIXING_PREFIX}CMS-{}:{forward_curve_id}",
        format_cms_tenor(tenor_years)
    )
}

/// Render a CMS reference tenor as `{n}Y` for whole years, `{n}M` for whole
/// months, falling back to raw fractional years otherwise.
fn format_cms_tenor(tenor_years: f64) -> String {
    let months = tenor_years * 12.0;
    let rounded = months.round();
    if rounded > 0.0 && (months - rounded).abs() < 1e-9 {
        let m = rounded as i64;
        if m % 12 == 0 {
            format!("{}Y", m / 12)
        } else {
            format!("{m}M")
        }
    } else {
        format!("{tenor_years}Y")
    }
}

/// Look up the fixing series for a rate index in MarketContext.
///
/// Returns a clear error when the series is missing, directing the user
/// to provide the expected `ScalarTimeSeries`. The lookup uses only the
/// canonical `FIXING:{forward_curve_id}` ID; it does not fall back to an
/// unqualified series, a forward curve projection, or a CMS fixing series.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] if `context` contains no scalar series
/// with the canonical ID. The diagnostic includes both the requested index and
/// expected series ID so a seasoned instrument can be supplied with the
/// correct historical data.
///
/// # Arguments
///
/// * `context` - Market context containing scalar time series keyed by their
///   canonical fixing IDs.
/// * `forward_curve_id` - Rate-index or forward-curve identifier used to build
///   the required `FIXING:{id}` series key.
pub fn get_fixing_series<'a>(
    context: &'a MarketContext,
    forward_curve_id: &str,
) -> Result<&'a ScalarTimeSeries> {
    let id = fixing_series_id(forward_curve_id);
    context.get_series(&id).map_err(|_| {
        crate::Error::Validation(format!(
            "No fixing series found for index '{forward_curve_id}'. \
             Seasoned instruments require a ScalarTimeSeries with id '{id}' \
             containing historical observations for dates before the valuation date."
        ))
    })
}

/// Require a fixing value from an already-resolved optional series.
///
/// Uses `value_on()` (step interpolation / LOCF), appropriate for overnight
/// RFR fixings in the compounded path. The carry-forward is **unbounded**: a
/// fixing arbitrarily far before `date` is silently used. When a staleness
/// limit is required (e.g. data-quality gates around long market closures),
/// use [`require_fixing_value_bounded`] instead.
///
/// `date` is the contractual fixing date and `as_of` is included only in the
/// diagnostic context; this helper does not itself prevent a caller from
/// requesting a fixing later than the valuation date.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when `series` is absent or cannot
/// resolve a value on `date` under its interpolation policy. For an existing
/// stepped series, this permits unbounded last-observation carry-forward;
/// callers needing data freshness must use the bounded variant.
///
/// # Arguments
///
/// * `series` - Optional resolved historical fixing series. `None` produces a
///   diagnostic that identifies the required canonical series ID.
/// * `forward_curve_id` - Rate-index identifier included in returned error
///   diagnostics and canonical-series guidance.
/// * `date` - Contractual fixing date to retrieve using stepped
///   last-observation-carried-forward interpolation.
/// * `as_of` - Valuation date included in diagnostics only; it does not limit
///   the lookup chronology.
pub fn require_fixing_value(
    series: Option<&ScalarTimeSeries>,
    forward_curve_id: &str,
    date: Date,
    as_of: Date,
) -> Result<f64> {
    let s = series.ok_or_else(|| {
        crate::Error::Validation(format!(
            "Seasoned instrument requires fixings for index '{forward_curve_id}' on {date} \
             (valuation date: {as_of}). Provide a ScalarTimeSeries with id '{}'.",
            fixing_series_id(forward_curve_id)
        ))
    })?;
    s.value_on(date).map_err(|e| {
        crate::Error::Validation(format!(
            "Missing fixing for '{forward_curve_id}' on {date} (valuation date: {as_of}). \
             The fixing series exists but lookup failed: {e}"
        ))
    })
}

/// Require a fixing value with a bounded last-observation-carried-forward
/// window.
///
/// Like [`require_fixing_value`] but uses
/// `value_on_or_before(date, max_staleness_days)`, so a prior observation is
/// only accepted when it is at most `max_staleness_days` calendar days before
/// `date`. Errors when the most recent observation is older than the bound
/// (or when the series is `None` / has no observation on or before `date`).
/// `as_of` is diagnostic context only; staleness is measured between the
/// requested fixing date and the last observation, not from valuation date.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when the series is absent, no
/// observation exists on or before `date`, or the newest prior observation is
/// more than `max_staleness_days` calendar days old. It also wraps any
/// underlying time-series lookup failure with the index, date, and as-of date.
///
/// # Arguments
///
/// * `series` - Optional resolved historical fixing series. `None` produces a
///   diagnostic that identifies the required canonical series ID.
/// * `forward_curve_id` - Rate-index identifier included in returned error
///   diagnostics and canonical-series guidance.
/// * `date` - Contractual fixing date to retrieve from this series.
/// * `as_of` - Valuation date included in diagnostics only; it does not affect
///   the staleness calculation.
/// * `max_staleness_days` - Maximum calendar-day age of a carried-forward
///   observation relative to `date`.
pub fn require_fixing_value_bounded(
    series: Option<&ScalarTimeSeries>,
    forward_curve_id: &str,
    date: Date,
    as_of: Date,
    max_staleness_days: u32,
) -> Result<f64> {
    let s = series.ok_or_else(|| {
        crate::Error::Validation(format!(
            "Seasoned instrument requires fixings for index '{forward_curve_id}' on {date} \
             (valuation date: {as_of}). Provide a ScalarTimeSeries with id '{}'.",
            fixing_series_id(forward_curve_id)
        ))
    })?;
    s.value_on_or_before(date, max_staleness_days).map_err(|e| {
        crate::Error::Validation(format!(
            "Missing fixing for '{forward_curve_id}' on {date} within {max_staleness_days} \
             calendar days (valuation date: {as_of}). The fixing series exists but lookup \
             failed: {e}"
        ))
    })
}

/// Require a fixing value using exact-date matching (no interpolation).
///
/// Fails if no observation exists for the exact requested date.
/// Appropriate for term rate fixings (e.g., 3M LIBOR resets), where carrying a
/// previous publication forward would change the contractual reset. `as_of`
/// is recorded only in failure diagnostics and does not impose a chronology
/// check.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when the series is absent or it lacks
/// an observation exactly on `date`. It intentionally does not interpolate or
/// fall back to a prior observation.
///
/// # Arguments
///
/// * `series` - Optional resolved historical fixing series. `None` produces a
///   diagnostic that identifies the required canonical series ID.
/// * `forward_curve_id` - Rate-index identifier included in returned error
///   diagnostics and canonical-series guidance.
/// * `date` - Contractual fixing date that must have an exact observation.
/// * `as_of` - Valuation date included in diagnostics only; it does not limit
///   the lookup chronology.
pub fn require_fixing_value_exact(
    series: Option<&ScalarTimeSeries>,
    forward_curve_id: &str,
    date: Date,
    as_of: Date,
) -> Result<f64> {
    let s = series.ok_or_else(|| {
        crate::Error::Validation(format!(
            "Seasoned instrument requires fixings for index '{forward_curve_id}' on {date} \
             (valuation date: {as_of}). Provide a ScalarTimeSeries with id '{}'.",
            fixing_series_id(forward_curve_id)
        ))
    })?;
    s.value_on_exact(date).map_err(|e| {
        crate::Error::Validation(format!(
            "Missing fixing for '{forward_curve_id}' on {date} (valuation date: {as_of}). \
             The fixing series exists but has no exact observation: {e}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_data::scalars::ScalarTimeSeries;
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
        )
        .expect("valid series")
    }

    #[test]
    fn fixing_series_id_builds_correct_key() {
        assert_eq!(fixing_series_id("USD-SOFR"), "FIXING:USD-SOFR");
        assert_eq!(fixing_series_id("EUR-ESTR"), "FIXING:EUR-ESTR");
    }

    #[test]
    fn cms_fixing_series_id_builds_tenor_qualified_key() {
        assert_eq!(
            cms_fixing_series_id("USD-SOFR", 10.0),
            "FIXING:CMS-10Y:USD-SOFR"
        );
        assert_eq!(
            cms_fixing_series_id("USD-SOFR", 2.0),
            "FIXING:CMS-2Y:USD-SOFR"
        );
        assert_eq!(
            cms_fixing_series_id("USD-SOFR", 0.5),
            "FIXING:CMS-6M:USD-SOFR"
        );
        assert_eq!(
            cms_fixing_series_id("USD-SOFR", 1.5),
            "FIXING:CMS-18M:USD-SOFR"
        );
    }

    #[test]
    fn get_fixing_series_returns_series_when_present() {
        let series = sample_series();
        let ctx = MarketContext::new().insert_series(series);
        let result = get_fixing_series(&ctx, "USD-SOFR");
        assert!(result.is_ok());
    }

    #[test]
    fn get_fixing_series_errors_when_missing() {
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
    }

    #[test]
    fn require_fixing_value_returns_rate_via_locf() {
        let series = sample_series();
        let as_of = date!(2024 - 01 - 10);
        // Jan 4 is not observed; LOCF from Jan 3 (0.054)
        let rate = require_fixing_value(Some(&series), "USD-SOFR", date!(2024 - 01 - 04), as_of)
            .expect("should resolve via LOCF");
        assert!((rate - 0.054).abs() < 1e-10);
    }

    #[test]
    fn require_fixing_value_bounded_returns_rate_within_window() {
        let series = sample_series();
        let rate = require_fixing_value_bounded(
            Some(&series),
            "USD-SOFR",
            date!(2024 - 01 - 04),
            date!(2024 - 01 - 10),
            1,
        )
        .expect("Jan 3 fixing is within one calendar day");

        assert!((rate - 0.054).abs() < 1e-10);
    }

    #[test]
    fn require_fixing_value_bounded_errors_when_stale_or_missing_series() {
        let series = sample_series();
        let stale = require_fixing_value_bounded(
            Some(&series),
            "USD-SOFR",
            date!(2024 - 01 - 08),
            date!(2024 - 01 - 10),
            2,
        )
        .expect_err("Jan 5 fixing is three days stale");
        assert!(
            stale.to_string().contains("within 2 calendar days"),
            "unexpected error: {stale}"
        );

        let missing = require_fixing_value_bounded(
            None,
            "USD-SOFR",
            date!(2024 - 01 - 04),
            date!(2024 - 01 - 10),
            1,
        )
        .expect_err("missing series should be rejected");
        assert!(
            missing.to_string().contains("FIXING:USD-SOFR"),
            "unexpected error: {missing}"
        );
    }

    #[test]
    fn require_fixing_value_errors_when_series_is_none() {
        let result = require_fixing_value(
            None,
            "USD-SOFR",
            date!(2024 - 01 - 02),
            date!(2024 - 01 - 10),
        );
        assert!(result.is_err());
        let msg = result.expect_err("should error").to_string();
        assert!(
            msg.contains("FIXING:USD-SOFR"),
            "should mention series id: {msg}"
        );
        assert!(msg.contains("2024-01-02"), "should mention date: {msg}");
    }

    #[test]
    fn require_fixing_value_exact_returns_rate_on_observed_date() {
        let series = sample_series();
        let rate = require_fixing_value_exact(
            Some(&series),
            "USD-SOFR",
            date!(2024 - 01 - 03),
            date!(2024 - 01 - 10),
        )
        .expect("exact date exists");
        assert!((rate - 0.054).abs() < 1e-10);
    }

    #[test]
    fn require_fixing_value_exact_errors_on_unobserved_date() {
        let series = sample_series();
        let result = require_fixing_value_exact(
            Some(&series),
            "USD-SOFR",
            date!(2024 - 01 - 04), // Not observed
            date!(2024 - 01 - 10),
        );
        assert!(result.is_err());
        let msg = result.expect_err("should error").to_string();
        assert!(msg.contains("2024-01-04"), "should mention date: {msg}");
    }

    #[test]
    fn require_fixing_value_exact_errors_when_series_is_none() {
        let result = require_fixing_value_exact(
            None,
            "USD-SOFR",
            date!(2024 - 01 - 02),
            date!(2024 - 01 - 10),
        );
        assert!(result.is_err());
        let msg = result.expect_err("should error").to_string();
        assert!(
            msg.contains("FIXING:USD-SOFR"),
            "should mention series id: {msg}"
        );
    }
}

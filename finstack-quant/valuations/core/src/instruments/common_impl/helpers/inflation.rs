//! Inflation observation lag and reference CPI resolution helpers.

use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::market_data::{
    context::MarketContext,
    scalars::{InflationLag, SeriesInterpolation},
};

/// Apply an inflation lag to a date.
///
/// - `Months(m)` subtracts `m` calendar months
/// - `Days(d)` subtracts `d` calendar days
/// - `None` returns the date unchanged
///
/// Returns a validation error when the lag underflows the supported calendar
/// or the lag convention is unsupported.
///
/// # Arguments
///
/// * `date` - Contractual date before applying the observation shift.
/// * `lag` - Explicit contractual calendar-month or calendar-day shift.
pub fn apply_inflation_lag(date: Date, lag: InflationLag) -> finstack_quant_core::Result<Date> {
    match lag {
        InflationLag::None => Ok(date),
        InflationLag::Months(m) => date.add_months(-i32::from(m.get())),
        InflationLag::Days(d) => date.add_days(-i64::from(d.get())),
        _ => Err(finstack_quant_core::Error::Validation(
            "unsupported inflation lag convention".into(),
        )),
    }
}

/// Resolved reference CPI together with the information needed to distinguish
/// observed fixings from projections for option valuation.
pub struct InflationReferenceValue {
    /// Reference CPI after observation lag and monthly interpolation.
    pub value: f64,
    /// Whether every required anchor has an available historical fixing.
    pub is_known: bool,
    /// Latest expected release among outstanding anchors, or `None` when all
    /// anchors are observed. This is a schedule assumption, not actual availability.
    pub publication_date: Option<Date>,
}

/// Resolve a contractual reference CPI, applying month lag and daily weight
/// once, together with the availability of every interpolation anchor.
///
/// Exact observations in the selected knowledge snapshot take priority over
/// release assumptions. A missing observation projects only when a matching
/// expected release is strictly after the cutoff. Missing or overdue release
/// facts error; valuation-date changes never publish a fixing.
///
/// # Arguments
///
/// * `market` - Market context holding the inflation index history and, for
///   unpublished anchors, the unlagged inflation curve named `index_id`.
/// * `index_id` - Identifier shared by the inflation index and inflation curve.
/// * `date` - Contractual reference date before the observation lag is applied.
/// * `knowledge_date` - Inclusive information cutoff; must match the supplied
///   index snapshot. This is independent of the product's valuation date.
/// * `lag` - Observation lag: month lags keep the contractual month's daily
///   weight, day lags shift the reference date itself.
/// * `interpolation` - `Step` uses the first-of-month anchor only; `Linear`
///   interpolates daily between consecutive monthly anchors.
///
/// # Errors
///
/// Returns an error for a mismatched snapshot cutoff, missing/overdue fixing
/// facts, missing projection coverage, invalid levels, or calendar overflow.
pub fn resolve_reference_inflation(
    market: &MarketContext,
    index_id: &str,
    date: Date,
    knowledge_date: Date,
    lag: InflationLag,
    interpolation: SeriesInterpolation,
) -> finstack_quant_core::Result<InflationReferenceValue> {
    let index = market.get_inflation_index(index_id)?;
    if index.knowledge_date() != knowledge_date {
        return Err(finstack_quant_core::Error::Validation(format!(
            "inflation index '{index_id}' snapshot cutoff {} does not match requested knowledge date {knowledge_date}",
            index.knowledge_date()
        )));
    }
    let (weight_date, months) = match lag {
        InflationLag::Months(months) => (date, months.get()),
        _ => (apply_inflation_lag(date, lag)?, 0),
    };
    let first = weight_date
        .replace_day(1)
        .map_err(|_| finstack_quant_core::InputError::InvalidDateRange)?
        .add_months(-i32::from(months))?;
    let weight = match interpolation {
        SeriesInterpolation::Step => 0.0,
        SeriesInterpolation::Linear => {
            f64::from(weight_date.day() - 1)
                / f64::from(weight_date.month().length(weight_date.year()))
        }
    };
    let resolve = |reference_month| -> finstack_quant_core::Result<(f64, Option<Date>)> {
        match index.get_observation(reference_month) {
            Ok(observation) => Ok((observation.value, None)),
            Err(finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                ..
            })) => {
                let release = index.get_expected_release(reference_month)?.ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "inflation index '{index_id}' has no observation or expected release for {reference_month}"
                    ))
                })?;
                if release.expected_release <= knowledge_date {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "inflation index '{index_id}' fixing for {reference_month} is overdue on {knowledge_date}"
                    )));
                }
                let projected = market
                    .get_inflation_curve(index_id)?
                    .cpi_on_date(reference_month)?;
                Ok((projected, Some(release.expected_release)))
            }
            Err(error) => Err(error),
        }
    };
    let (left, left_release) = resolve(first)?;
    let (value, right_release) = if weight == 0.0 {
        (left, None)
    } else {
        let (right, right_release) = resolve(first.add_months(1)?)?;
        let interpolated = if left <= right {
            left + weight * (right - left)
        } else {
            right + (1.0 - weight) * (left - right)
        };
        (interpolated, right_release)
    };
    if !value.is_finite() || value <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "inflation index '{index_id}' reference CPI is not finite and positive"
        )));
    }
    let publication_date = match (left_release, right_release) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };
    Ok(InflationReferenceValue {
        value,
        is_known: publication_date.is_none(),
        publication_date,
    })
}

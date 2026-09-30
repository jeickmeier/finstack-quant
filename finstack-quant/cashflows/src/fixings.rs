//! Materialize crossed index observations from canonical schedule projections.

use crate::builder::CashFlowSchedule;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{ScalarTimeSeries, SeriesInterpolation};
use finstack_quant_core::{Error, Result};
use std::collections::BTreeMap;

/// One raw market observation a schedule projected at its valuation date.
///
/// Floating coupons record their rate-index fixings; instruments that observe
/// a price history on a schedule (variance swaps) record those observations at
/// the valuation-date spot level.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectedFixing {
    /// Market-series identifier: `FIXING:{index}` for a rate or FX fixing
    /// series, or the price series an instrument observes (for example an
    /// underlying's close history).
    pub series_id: String,
    /// Contractual index observation date, after fixing-calendar adjustments.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Raw observed quantity in the index convention: annualized decimal rate
    /// before spread/gearing/caps/floors for rates, quote currency per base
    /// currency for an FX fixing series, or the price level for a price series.
    /// The series identifier fixes orientation.
    /// `None` records an observation for which the coupon's fallback policy
    /// masked a missing projection dependency; a time roll must supply an
    /// existing fixing or fail explicitly when crossing that date.
    pub value: Option<f64>,
}

/// Materialize reset observations crossed by a realized-forward market roll.
///
/// Every projected observation dated in `[old_date, new_date]` that `market`
/// lacks as an exact-date fixing is added at the value the schedule projected
/// on `market` at `old_date`. The window includes `old_date`: valuation is at
/// start of day, so a fixing dated on the pre-roll date is still projected
/// there but is a past observation once the valuation date moves past it.
///
/// # Arguments
///
/// * `market` - Immutable pre-roll market; existing exact-date observations
///   take precedence over every projection and are retained unchanged.
/// * `schedules` - Canonical schedules projected on `market` at `old_date`.
///   Their metadata carries the raw observations used by the coupon engines.
/// * `old_date` - Inclusive start of the fixing window (the pre-roll
///   valuation date); earlier observations are never invented by this
///   operation.
/// * `new_date` - Inclusive horizon; must be on or after `old_date`.
///
/// # Errors
///
/// Returns a validation error for a backward window, malformed fixing-series
/// identifiers, unavailable or non-finite crossed projections, or inconsistent
/// projections of the same index/date. The input market is never mutated.
pub fn materialize_fixings<'a>(
    market: &MarketContext,
    schedules: impl IntoIterator<Item = &'a CashFlowSchedule>,
    old_date: Date,
    new_date: Date,
) -> Result<MarketContext> {
    materialize_fixings_from_projections(
        market,
        schedules
            .into_iter()
            .flat_map(|schedule| &schedule.get_meta().projected_fixings),
        old_date,
        new_date,
    )
}

/// Materialize crossed observations from projected fixings directly.
///
/// Same window, precedence and failure rules as [`materialize_fixings`], for
/// callers that hold the projections rather than the schedules (the theta
/// metric caches only a schedule's flows and its projected fixings).
///
/// # Arguments
///
/// * `market` - Immutable pre-roll market; existing exact-date observations
///   take precedence over every projection and are retained unchanged.
/// * `fixings` - Raw observations projected on `market` at `old_date`, as
///   recorded in `CashFlowMeta::projected_fixings`.
/// * `old_date` - Inclusive start of the fixing window (the pre-roll
///   valuation date); earlier observations are never invented.
/// * `new_date` - Inclusive horizon; must be on or after `old_date`.
///
/// # Errors
///
/// Returns a validation error for a backward window, malformed fixing-series
/// identifiers, unavailable or non-finite crossed projections, or inconsistent
/// projections of the same index/date. The input market is never mutated.
pub fn materialize_fixings_from_projections<'a>(
    market: &MarketContext,
    fixings: impl IntoIterator<Item = &'a ProjectedFixing>,
    old_date: Date,
    new_date: Date,
) -> Result<MarketContext> {
    if new_date < old_date {
        return Err(Error::Validation(
            "fixing materialization requires a forward date window".into(),
        ));
    }
    let mut crossed: BTreeMap<(String, Date), Option<f64>> = BTreeMap::new();
    for fixing in fixings {
        if fixing.date < old_date || fixing.date > new_date {
            continue;
        }
        if fixing.series_id.is_empty() || fixing.series_id == "FIXING:" {
            return Err(Error::Validation(format!(
                "invalid projected fixing series '{}'",
                fixing.series_id
            )));
        }
        if market
            .get_series(&fixing.series_id)
            .ok()
            .is_some_and(|series| series.value_on_exact(fixing.date).is_ok())
        {
            continue;
        }
        let entry = crossed
            .entry((fixing.series_id.clone(), fixing.date))
            .or_default();
        if let Some(value) = fixing.value {
            if !value.is_finite() {
                return Err(Error::Validation(format!(
                    "non-finite pre-roll projection for '{}' on {}",
                    fixing.series_id, fixing.date
                )));
            }
            if let Some(previous) = *entry {
                let tolerance = 1e-12 * value.abs().max(previous.abs()).max(1.0);
                if (previous - value).abs() > tolerance {
                    return Err(Error::Validation(format!(
                        "conflicting pre-roll projections for '{}' on {}: {} and {}",
                        fixing.series_id, fixing.date, previous, value
                    )));
                }
            }
            *entry = Some(value);
        }
    }
    let mut by_series: BTreeMap<String, Vec<(Date, f64)>> = BTreeMap::new();
    for ((id, date), value) in crossed {
        let value = value.ok_or_else(|| Error::Validation(format!(
            "missing pre-roll projection for '{id}' on {date}; supply its projection curve or an exact-date fixing"
        )))?;
        by_series.entry(id).or_default().push((date, value));
    }
    let mut replacements = Vec::with_capacity(by_series.len());
    for (id, additions) in by_series {
        let existing = market.get_series(&id).ok();
        let mut observations: BTreeMap<Date, f64> = existing.map_or_else(BTreeMap::new, |series| {
            series.observations().into_iter().collect()
        });
        for (date, value) in additions {
            observations.entry(date).or_insert(value);
        }
        let currency = existing.and_then(ScalarTimeSeries::currency);
        let interpolation = existing.map_or(
            SeriesInterpolation::default(),
            ScalarTimeSeries::interpolation,
        );
        replacements.push(
            ScalarTimeSeries::new(id, observations.into_iter().collect(), currency)?
                .with_interpolation(interpolation),
        );
    }
    let mut projected = market.clone();
    for series in replacements {
        projected.insert_series_mut(series);
    }
    Ok(projected)
}

pub(crate) fn normalize_projected_fixings(fixings: &mut Vec<ProjectedFixing>) {
    fixings.sort_by(|left, right| {
        (&left.series_id, left.date)
            .cmp(&(&right.series_id, right.date))
            .then_with(|| match (left.value, right.value) {
                (Some(left), Some(right)) => left.total_cmp(&right),
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
    });
    fixings.dedup_by(|later, earlier| {
        if later.series_id != earlier.series_id || later.date != earlier.date {
            return false;
        }
        match (earlier.value, later.value) {
            (None, Some(_)) => {
                earlier.value = later.value;
                true
            }
            (_, None) => true,
            (Some(left), Some(right)) => left.total_cmp(&right).is_eq(),
        }
    });
}

//! Materialize crossed index observations from canonical coupon projections.

use crate::builder::CashFlowSchedule;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{ScalarTimeSeries, SeriesInterpolation};
use finstack_quant_core::{Error, Result};
use std::collections::BTreeMap;

/// One raw index observation needed by a projected floating coupon.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectedFixing {
    /// Canonical market-series identifier, including the `FIXING:` prefix.
    pub series_id: String,
    /// Contractual index observation date, after fixing-calendar adjustments.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Raw observed quantity in the index convention: annualized decimal rate
    /// before spread/gearing/caps/floors for rates, or quote currency per base
    /// currency for an FX fixing series. The series identifier fixes orientation.
    /// `None` records an observation for which the coupon's fallback policy
    /// masked a missing projection dependency; a time roll must supply an
    /// existing fixing or fail explicitly when crossing that date.
    pub value: Option<f64>,
}

/// Materialize reset observations crossed by a realized-forward market roll.
///
/// # Arguments
///
/// * `market` - Immutable pre-roll market; existing exact-date observations
///   take precedence over every projection and are retained unchanged.
/// * `schedules` - Canonical schedules projected on `market` at `old_date`.
///   Their metadata carries the raw observations used by the coupon engines.
/// * `old_date` - Exclusive start of the fixing window; earlier observations
///   are never invented by this operation.
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
    if new_date < old_date {
        return Err(Error::Validation(
            "fixing materialization requires a forward date window".into(),
        ));
    }
    let mut crossed: BTreeMap<(String, Date), Option<f64>> = BTreeMap::new();
    for schedule in schedules {
        for fixing in &schedule.get_meta().projected_fixings {
            if fixing.date <= old_date || fixing.date > new_date {
                continue;
            }
            if !fixing.series_id.starts_with("FIXING:") || fixing.series_id.len() == 7 {
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

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

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::scalars::{
        InflationExpectedRelease, InflationIndex, InflationObservation,
    };
    use finstack_quant_core::market_data::term_structures::InflationCurve;
    use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    use time::macros::date;

    fn inflation_market(
        cutoff: Date,
        observations: Vec<InflationObservation>,
        releases: Vec<InflationExpectedRelease>,
    ) -> MarketContext {
        let index = InflationIndex::new("CPI", observations, Currency::USD, cutoff)
            .unwrap()
            .with_expected_releases(releases)
            .unwrap();
        MarketContext::new()
            .insert_inflation_index(index)
            .try_insert(
                InflationCurve::builder("CPI")
                    .base_date(date!(2024 - 01 - 01))
                    .day_count(DayCount::Act365F)
                    .interp(InterpStyle::Linear)
                    .extrapolation(ExtrapolationPolicy::Flat)
                    .knots([(0.0, 306.0), (5.0, 306.0)])
                    .build()
                    .unwrap(),
            )
            .expect("insert market curve")
    }

    #[test]
    fn observations_win_and_outstanding_releases_admit_projection() {
        let cutoff = date!(2024 - 02 - 20);
        let january = date!(2024 - 01 - 01);
        let february = date!(2024 - 02 - 01);
        let market = inflation_market(
            cutoff,
            vec![InflationObservation {
                reference_month: january,
                value: 300.0,
                available_on: date!(2024 - 02 - 12),
            }],
            vec![
                InflationExpectedRelease {
                    reference_month: january,
                    expected_release: date!(2024 - 02 - 24),
                    known_on: january,
                },
                InflationExpectedRelease {
                    reference_month: february,
                    expected_release: date!(2024 - 03 - 12),
                    known_on: january,
                },
            ],
        );
        let result = resolve_reference_inflation(
            &market,
            "CPI",
            date!(2024 - 04 - 15),
            cutoff,
            InflationLag::months(3),
            SeriesInterpolation::Linear,
        )
        .unwrap();
        assert!((result.value - 302.8).abs() < 1e-12);
        assert!(!result.is_known);
        assert_eq!(result.publication_date, Some(date!(2024 - 03 - 12)));
        assert!(
            resolve_reference_inflation(
                &market,
                "CPI",
                date!(2024 - 04 - 15),
                date!(2024 - 03 - 12),
                InflationLag::months(3),
                SeriesInterpolation::Linear,
            )
            .is_err()
        );
    }

    #[test]
    fn missing_or_overdue_release_facts_never_project() {
        let cutoff = date!(2024 - 03 - 12);
        for releases in [
            vec![],
            vec![InflationExpectedRelease {
                reference_month: date!(2024 - 02 - 01),
                expected_release: cutoff,
                known_on: date!(2024 - 01 - 01),
            }],
        ] {
            let market = inflation_market(cutoff, vec![], releases);
            assert!(
                resolve_reference_inflation(
                    &market,
                    "CPI",
                    date!(2024 - 02 - 01),
                    cutoff,
                    InflationLag::None,
                    SeriesInterpolation::Step,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn monthly_geometry_and_unused_anchors_do_not_depend_on_metadata() {
        let cutoff = date!(2024 - 03 - 01);
        let january = date!(2024 - 01 - 01);
        let observations = vec![InflationObservation {
            reference_month: january,
            value: 300.0,
            available_on: date!(2024 - 02 - 12),
        }];
        let market = inflation_market(cutoff, observations, vec![]);
        for (lag, contract_date) in [
            (InflationLag::None, january),
            (InflationLag::months(0), january),
            (InflationLag::days(0), january),
            (InflationLag::days(10), date!(2024 - 01 - 11)),
            (InflationLag::months(3), date!(2024 - 04 - 01)),
        ] {
            let result = resolve_reference_inflation(
                &market,
                "CPI",
                contract_date,
                cutoff,
                lag,
                SeriesInterpolation::Linear,
            )
            .unwrap();
            assert_eq!(result.value, 300.0);
            assert!(result.is_known);
        }
        let step = resolve_reference_inflation(
            &market,
            "CPI",
            date!(2024 - 01 - 15),
            cutoff,
            InflationLag::None,
            SeriesInterpolation::Step,
        )
        .unwrap();
        assert_eq!(step.value, 300.0);
        assert!(
            resolve_reference_inflation(
                &market,
                "CPI",
                date!(2024 - 01 - 15),
                cutoff,
                InflationLag::None,
                SeriesInterpolation::Linear,
            )
            .is_err()
        );
        assert!(
            resolve_reference_inflation(
                &market,
                "CPI",
                date!(2023 - 12 - 15),
                cutoff,
                InflationLag::None,
                SeriesInterpolation::Step,
            )
            .is_err()
        );
    }

    #[test]
    fn month_lag_uses_contract_month_weight_and_preserves_extreme_levels() {
        let cutoff = date!(2024 - 03 - 01);
        for (left, right, expected) in [
            (285.0, 314.0, 313.0),
            (f64::from_bits(1), f64::from_bits(1), f64::from_bits(1)),
            (f64::MAX, f64::MAX, f64::MAX),
        ] {
            let market = inflation_market(
                cutoff,
                vec![
                    InflationObservation {
                        reference_month: date!(2023 - 11 - 01),
                        value: left,
                        available_on: date!(2023 - 12 - 12),
                    },
                    InflationObservation {
                        reference_month: date!(2023 - 12 - 01),
                        value: right,
                        available_on: date!(2024 - 01 - 12),
                    },
                ],
                vec![],
            );
            let result = resolve_reference_inflation(
                &market,
                "CPI",
                date!(2024 - 02 - 29),
                cutoff,
                InflationLag::months(3),
                SeriesInterpolation::Linear,
            )
            .unwrap();
            assert_eq!(result.value, expected);
            assert!(result.is_known);
        }
    }

    #[test]
    fn date_offset_rejects_inflation_lag_underflow() {
        for lag in [InflationLag::months(1), InflationLag::days(1)] {
            assert!(apply_inflation_lag(Date::MIN, lag).is_err());
        }
        assert_eq!(
            apply_inflation_lag(Date::MIN, InflationLag::None).expect("no date shift"),
            Date::MIN
        );
    }
}

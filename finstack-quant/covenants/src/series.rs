//! Dated metric observations for covenant series evaluation and forecasting.
//!
//! [`DatedMetricSeries`] is the crate-owned [`ModelTimeSeries`] for callers
//! that hold plain dated rows of metric values (a host table, a JSON array)
//! rather than a statement model. Each observation date becomes one daily
//! [`PeriodId`] whose period start and end are that date.

use std::collections::HashMap;

use finstack_quant_core::dates::{Date, PeriodId};
use finstack_quant_core::{Error, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::engine::{CovenantEngine, CovenantSpec};
use crate::forward::{
    forecast_breaches_generic, forecast_covenant_generic, CovenantForecast, CovenantForecastConfig,
    FutureBreach, ModelTimeSeries,
};
use crate::metric::HashMapMetricSource;
use crate::report::CovenantReport;

/// Covenant metric values observed (or projected) on one date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DatedMetrics {
    /// Covenant test date the metric values belong to.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Metric values keyed by covenant metric identifier (for example
    /// `debt_to_ebitda`), in the units the covenant tests expect: ratios in
    /// turns (`4.5` means 4.5x), amounts in the reporting currency.
    pub metrics: IndexMap<String, f64>,
}

/// Covenant reports produced for one test date of a series evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DatedCovenantReports {
    /// Test date the covenants were evaluated on.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
    /// Reports keyed by stable covenant instance key, in engine order.
    pub reports: IndexMap<String, CovenantReport>,
}

/// Dated metric observations exposed as a [`ModelTimeSeries`].
///
/// Observations are held in ascending date order. Each date maps to the daily
/// [`PeriodId`] of that date, so period start and end are both the
/// observation date.
#[derive(Debug, Clone, PartialEq)]
pub struct DatedMetricSeries {
    periods: Vec<PeriodId>,
    values: HashMap<PeriodId, IndexMap<String, f64>>,
}

impl DatedMetricSeries {
    /// Build a series from dated metric rows.
    ///
    /// # Arguments
    ///
    /// * `observations` - Rows of metric values, one per covenant test date,
    ///   in any order; they are sorted by date. Values are not checked here:
    ///   the forecast rejects non-finite values for the metrics it reads.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when `observations` is empty or two rows
    /// share a date.
    pub fn new(mut observations: Vec<DatedMetrics>) -> Result<Self> {
        if observations.is_empty() {
            return Err(Error::Validation(
                "metric series must contain at least one dated row".to_string(),
            ));
        }
        observations.sort_by_key(|row| row.date);
        let mut periods = Vec::with_capacity(observations.len());
        let mut values = HashMap::with_capacity(observations.len());
        for row in observations {
            let period = PeriodId::day(row.date.year(), row.date.ordinal())?;
            if values.insert(period, row.metrics).is_some() {
                return Err(Error::Validation(format!(
                    "metric series contains duplicate date {}",
                    row.date
                )));
            }
            periods.push(period);
        }
        Ok(Self { periods, values })
    }

    /// Daily periods of the series, in ascending date order.
    #[must_use]
    pub fn periods(&self) -> &[PeriodId] {
        &self.periods
    }
}

impl ModelTimeSeries for DatedMetricSeries {
    fn get_scalar(&self, node_id: &str, period: &PeriodId) -> Option<f64> {
        self.values.get(period)?.get(node_id).copied()
    }

    fn period_end_date(&self, period: &PeriodId) -> Result<Date> {
        Date::from_ordinal_date(period.year, period.index)
            .map_err(|error| Error::Validation(error.to_string()))
    }

    fn period_start_date(&self, period: &PeriodId) -> Result<Date> {
        self.period_end_date(period)
    }
}

/// Forecast one numeric covenant over every date of a dated metric series.
///
/// This is [`forecast_covenant_generic`] over all of the series' dates; see it
/// for the deterministic and stochastic conventions.
///
/// # Arguments
///
/// * `covenant` - Numeric covenant specification whose metric, threshold and
///   springing condition determine the forecast.
/// * `series` - Dated observations of the covenant metric (and of any
///   denominator or springing metric) on each test date.
/// * `config` - Forecast policy: deterministic or stochastic mode, annualized
///   metric log-volatility, sampling and the reference date.
///
/// # Errors
///
/// Returns the errors of [`forecast_covenant_generic`]: a validation error for
/// invalid configuration or terms, dates before the reference date or
/// non-finite observations, and `NotFound` for a missing required metric.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{create_date, Month, Tenor};
/// use finstack_quant_covenants::{
///     forecast_covenant, Covenant, CovenantForecastConfig, CovenantSpec, CovenantType,
///     DatedMetricSeries, DatedMetrics,
/// };
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let spec = CovenantSpec::with_metric(
///     Covenant::new(
///         CovenantType::MaxDebtToEbitda { threshold: 4.5 },
///         Tenor::quarterly(),
///         "max_leverage",
///     ),
///     "debt_to_ebitda",
/// );
/// let row = |month, day, leverage: f64| -> finstack_quant_core::Result<DatedMetrics> {
///     Ok(DatedMetrics {
///         date: create_date(2026, month, day)?,
///         metrics: [("debt_to_ebitda".to_string(), leverage)].into_iter().collect(),
///     })
/// };
/// let series = DatedMetricSeries::new(vec![
///     row(Month::March, 31, 4.0)?,
///     row(Month::June, 30, 4.8)?,
/// ])?;
///
/// let forecast = forecast_covenant(&spec, &series, CovenantForecastConfig::default())?;
/// assert_eq!(forecast.breach_probability, [0.0, 1.0]);
/// assert_eq!(forecast.first_breach_date, Some(create_date(2026, Month::June, 30)?));
/// # Ok(())
/// # }
/// ```
pub fn forecast_covenant(
    covenant: &CovenantSpec,
    series: &DatedMetricSeries,
    config: CovenantForecastConfig,
) -> Result<CovenantForecast> {
    forecast_covenant_generic(covenant, series, series.periods(), config)
}

/// Forecast every breach of an engine's covenants over a dated metric series.
///
/// This is [`forecast_breaches_generic`] over all of the series' dates; see it
/// for scope, waiver and inclusion rules.
///
/// # Arguments
///
/// * `engine` - Valid engine whose effective specifications are forecast.
/// * `series` - Dated observations of every metric the active numeric
///   covenants read on each test date.
/// * `config` - Scope (maintenance by default), volatility, sampling and the
///   breach-probability reporting threshold.
///
/// # Errors
///
/// Returns the errors of [`forecast_breaches_generic`]; no partial breach list
/// is returned.
pub fn forecast_breaches(
    engine: &CovenantEngine,
    series: &DatedMetricSeries,
    config: CovenantForecastConfig,
) -> Result<Vec<FutureBreach>> {
    forecast_breaches_generic(engine, series, series.periods(), config)
}

impl CovenantEngine {
    /// Evaluate the engine on each dated row of metric values.
    ///
    /// Each row is evaluated independently with
    /// [`evaluate`](CovenantEngine::evaluate); breach history is not updated.
    ///
    /// # Arguments
    ///
    /// * `observations` - Metric values per test date. Rows are evaluated in
    ///   the order given, and each row must carry every metric the covenants
    ///   applicable on its date read.
    ///
    /// # Returns
    ///
    /// One [`DatedCovenantReports`] per input row, in input order.
    ///
    /// # Errors
    ///
    /// Returns the first evaluation error: invalid engine configuration, a
    /// missing metric, or a covenant that cannot produce a test value.
    pub fn evaluate_series(
        &self,
        observations: &[DatedMetrics],
    ) -> Result<Vec<DatedCovenantReports>> {
        observations
            .iter()
            .map(|row| {
                let source = HashMapMetricSource::from_pairs(
                    row.metrics.iter().map(|(id, value)| (id.as_str(), *value)),
                );
                Ok(DatedCovenantReports {
                    as_of: row.date,
                    reports: self.evaluate(&source, row.date)?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Covenant, CovenantType};
    use finstack_quant_core::dates::Tenor;
    use time::Month;

    fn date(month: Month, day: u8) -> Date {
        Date::from_calendar_date(2026, month, day).unwrap()
    }

    fn row(month: Month, day: u8, leverage: f64) -> DatedMetrics {
        DatedMetrics {
            date: date(month, day),
            metrics: IndexMap::from([("debt_to_ebitda".to_string(), leverage)]),
        }
    }

    fn leverage_spec() -> CovenantSpec {
        CovenantSpec::with_metric(
            Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 4.5 },
                Tenor::quarterly(),
                "max_leverage",
            ),
            "debt_to_ebitda",
        )
    }

    #[test]
    fn series_sorts_rows_and_maps_each_date_to_a_daily_period() {
        let series =
            DatedMetricSeries::new(vec![row(Month::June, 30, 4.8), row(Month::March, 31, 4.0)])
                .unwrap();
        let ends: Vec<Date> = series
            .periods()
            .iter()
            .map(|period| series.period_end_date(period).unwrap())
            .collect();
        assert_eq!(ends, [date(Month::March, 31), date(Month::June, 30)]);
        assert_eq!(
            series.get_scalar("debt_to_ebitda", &series.periods()[1]),
            Some(4.8)
        );
        assert_eq!(series.get_scalar("dscr", &series.periods()[0]), None);
    }

    #[test]
    fn series_rejects_empty_and_duplicate_dates() {
        let empty = DatedMetricSeries::new(Vec::new()).unwrap_err();
        assert_eq!(
            empty.to_string(),
            "Validation error: metric series must contain at least one dated row"
        );
        let duplicate =
            DatedMetricSeries::new(vec![row(Month::March, 31, 4.0), row(Month::March, 31, 4.1)])
                .unwrap_err();
        assert_eq!(
            duplicate.to_string(),
            "Validation error: metric series contains duplicate date 2026-03-31"
        );
    }

    #[test]
    fn forecast_and_series_evaluation_agree_on_the_breach_date() {
        let rows = vec![row(Month::March, 31, 4.0), row(Month::June, 30, 4.8)];
        let mut engine = CovenantEngine::new();
        engine.add_spec(leverage_spec());

        let series = DatedMetricSeries::new(rows.clone()).unwrap();
        let forecast =
            forecast_covenant(&leverage_spec(), &series, CovenantForecastConfig::default())
                .unwrap();
        assert_eq!(forecast.first_breach_date, Some(date(Month::June, 30)));
        assert_eq!(forecast.breach_probability, [0.0, 1.0]);

        let breaches =
            forecast_breaches(&engine, &series, CovenantForecastConfig::default()).unwrap();
        assert_eq!(breaches.len(), 1);
        assert_eq!(breaches[0].breach_date, date(Month::June, 30));

        let evaluated = engine.evaluate_series(&rows).unwrap();
        assert_eq!(evaluated.len(), 2);
        assert!(evaluated[0].reports["max_leverage"].passed);
        assert!(!evaluated[1].reports["max_leverage"].passed);
        assert_eq!(evaluated[1].as_of, date(Month::June, 30));
    }

    #[test]
    fn series_evaluation_fails_on_a_missing_metric() {
        let mut engine = CovenantEngine::new();
        engine.add_spec(leverage_spec());
        let rows = [DatedMetrics {
            date: date(Month::March, 31),
            metrics: IndexMap::new(),
        }];
        assert!(engine.evaluate_series(&rows).is_err());
    }
}

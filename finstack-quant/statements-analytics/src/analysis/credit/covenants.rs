//! Covenant forecasting bridge for statements.
//!
//! This module provides the integration between financial statement forecasts
//! and the covenant engine, allowing for future compliance checking.

use finstack_quant_core::dates::{Date, PeriodId, PeriodKind};
use finstack_quant_core::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};
use finstack_quant_core::Result;
use finstack_quant_covenants::{
    forecast_breaches_generic, forecast_covenant_generic, CovenantEngine, CovenantForecast,
    CovenantForecastConfig, CovenantSpec, FutureBreach, ModelTimeSeries,
};
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::FinancialModelSpec;
use indexmap::IndexMap;
use serde_json::json;
use time::Month;

/// Adapter to use Statements StatementResult as a ModelTimeSeries.
///
/// This is primarily useful when integrating statement outputs with the
/// covenant engine without re-shaping data into a separate time-series object.
pub struct StatementsAdapter<'a> {
    model: Option<&'a FinancialModelSpec>,
    results: &'a StatementResult,
}

impl<'a> StatementsAdapter<'a> {
    /// Create a new adapter from results and optional model spec.
    pub fn new(results: &'a StatementResult, model: Option<&'a FinancialModelSpec>) -> Self {
        Self { model, results }
    }
}

impl<'a> ModelTimeSeries for StatementsAdapter<'a> {
    fn get_scalar(&self, node_id: &str, period: &PeriodId) -> Option<f64> {
        self.results.get(node_id, period)
    }

    fn period_end_date(&self, period: &PeriodId) -> Date {
        if let Some(model) = self.model {
            for p in &model.periods {
                if p.id == *period {
                    // Periods use half-open [start, end) semantics —
                    // return the last inclusive day, consistent with
                    // approximate_period_end which returns calendar
                    // month-end / year-end dates.
                    return p.end - time::Duration::days(1);
                }
            }
        }
        approximate_period_end(period)
    }
}

/// Approximate the end date of a period from its `PeriodId` when the model is
/// not available. Handles all `PeriodKind` variants.
fn approximate_period_end(period: &PeriodId) -> Date {
    let (month, day) = match period.kind() {
        PeriodKind::Monthly => {
            let m = Month::try_from(period.index as u8).unwrap_or(Month::December);
            let d = last_day_of_month(period.year, m);
            (m, d)
        }
        PeriodKind::Quarterly => {
            let m = match period.index {
                1 => Month::March,
                2 => Month::June,
                3 => Month::September,
                _ => Month::December,
            };
            let d = last_day_of_month(period.year, m);
            (m, d)
        }
        PeriodKind::SemiAnnual => {
            let m = if period.index == 1 {
                Month::June
            } else {
                Month::December
            };
            let d = last_day_of_month(period.year, m);
            (m, d)
        }
        PeriodKind::Annual => (Month::December, 31),
        PeriodKind::Daily => {
            // index is ordinal day 1..=366; convert back to (month, day)
            let jan1 =
                Date::from_calendar_date(period.year, Month::January, 1).unwrap_or(time::Date::MIN);
            let date = jan1.saturating_add(time::Duration::days(period.index as i64 - 1));
            (date.month(), date.day())
        }
        PeriodKind::Weekly => {
            // index is ISO week 1..=53; approximate end as Sunday of that week
            let jan4 =
                Date::from_calendar_date(period.year, Month::January, 4).unwrap_or(time::Date::MIN);
            let iso_week1_monday = jan4.saturating_sub(time::Duration::days(
                jan4.weekday().number_days_from_monday() as i64,
            ));
            let week_end =
                iso_week1_monday.saturating_add(time::Duration::days(period.index as i64 * 7 - 1));
            return week_end;
        }
    };
    Date::from_calendar_date(period.year, month, day).unwrap_or_else(|_| {
        Date::from_calendar_date(period.year, Month::December, 31).unwrap_or(time::Date::MIN)
    })
}

fn last_day_of_month(year: i32, month: Month) -> u8 {
    match month {
        Month::January
        | Month::March
        | Month::May
        | Month::July
        | Month::August
        | Month::October
        | Month::December => 31,
        Month::April | Month::June | Month::September | Month::November => 30,
        Month::February => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
    }
}

/// Forecast a single covenant's future compliance using statement results.
///
/// Stochastic forecasts require explicit annualized log-volatility of the
/// covenant metric in `config.volatility`. Earnings-level deviations are not
/// interchangeable with ratio volatility and are never inferred from drivers.
///
/// # Arguments
///
/// * `covenant` - Covenant specification to simulate
/// * `model` - Source statement model used to resolve inclusive period-end dates
/// * `base_case` - Evaluated base-case statement results
/// * `periods` - Future periods to test
/// * `config` - Forecasting configuration for simulation horizon and
///   distribution assumptions
///
/// # Returns
///
/// Returns a [`CovenantForecast`] containing projected values, thresholds,
/// headroom, and breach probabilities by test date.
///
/// # Errors
///
/// Returns an error if the covenant engine rejects the input series, if model
/// periods cannot be resolved consistently, or if explicit volatility
/// parameters are missing or malformed for a stochastic forecast.
///
/// # References
///
/// - Lognormal breach probability: `docs/REFERENCES.md#glasserman-2004-monte-carlo`
pub fn forecast_covenant(
    covenant: &CovenantSpec,
    model: &FinancialModelSpec,
    base_case: &StatementResult,
    periods: &[PeriodId],
    config: CovenantForecastConfig,
) -> Result<CovenantForecast> {
    let adapter = StatementsAdapter::new(base_case, Some(model));
    forecast_covenant_generic(covenant, &adapter, periods, config)
}

/// Forecast covenant breaches based on statement results.
///
/// # Arguments
///
/// * `results` - The forecast results (time-series of metrics)
/// * `covenants` - The covenant engine containing covenant specifications
/// * `model` - Optional financial model spec (for precise period dates)
/// * `config` - Forecasting configuration
///
/// # Returns
///
/// List of projected breaches.
///
/// # Errors
///
/// Returns an error if the covenant engine cannot project breaches from the
/// provided results set.
///
/// # References
///
/// - Lognormal breach probability: `docs/REFERENCES.md#glasserman-2004-monte-carlo`
pub fn forecast_breaches(
    results: &StatementResult,
    covenants: &CovenantEngine,
    model: Option<&FinancialModelSpec>,
    config: CovenantForecastConfig,
) -> Result<Vec<FutureBreach>> {
    let mut periods: Vec<PeriodId> = results
        .nodes
        .values()
        .flat_map(|map| map.keys().cloned())
        .collect();
    periods.sort();
    periods.dedup();

    let adapter = StatementsAdapter::new(results, model);
    forecast_breaches_generic(covenants, &adapter, &periods, config)
}

/// Convert a covenant forecast into a serializable table for downstream analysis.
///
/// The resulting schema is:
/// `(test_date, projected_value, threshold, headroom, breach_prob)`.
///
/// # Arguments
///
/// * `forecast` - Covenant forecast containing aligned test dates, projected
///   values, thresholds, headroom, and breach probabilities.
///
/// # Errors
///
/// Returns a validation error if the table envelope invariants are broken.
pub fn to_table(forecast: &CovenantForecast) -> Result<TableEnvelope> {
    let dates = forecast
        .test_dates
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>();

    let mut metadata = IndexMap::new();
    metadata.insert("layout".to_string(), json!("long"));
    metadata.insert("source".to_string(), json!("covenant_forecast"));

    TableEnvelope::new_with_metadata(
        vec![
            TableColumn::new("test_date", TableColumnData::String(dates))
                .with_role(TableColumnRole::Index),
            TableColumn::new(
                "projected_value",
                TableColumnData::NullableFloat64(forecast.projected_values.clone()),
            )
            .with_role(TableColumnRole::Measure),
            TableColumn::new(
                "threshold",
                TableColumnData::Float64(forecast.thresholds.clone()),
            )
            .with_role(TableColumnRole::Measure),
            TableColumn::new(
                "headroom",
                TableColumnData::NullableFloat64(forecast.headroom.clone()),
            )
            .with_role(TableColumnRole::Measure),
            TableColumn::new(
                "breach_prob",
                TableColumnData::Float64(forecast.breach_probability.clone()),
            )
            .with_role(TableColumnRole::Measure),
        ],
        metadata,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::{Date, Tenor};
    use finstack_quant_covenants::CovenantType;
    use finstack_quant_covenants::{Covenant, CovenantEngine, CovenantMetricId, CovenantSpec};
    use finstack_quant_statements::evaluator::StatementResult;
    use indexmap::IndexMap;
    use time::Month;

    #[test]
    fn iso_week_end_preserves_calendar_year() {
        let week = PeriodId::week(2020, 53).expect("valid ISO week");
        assert_eq!(
            approximate_period_end(&week),
            time::macros::date!(2021 - 01 - 03)
        );
    }

    #[test]
    fn approximate_period_end_quarterly() {
        let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        assert_eq!(
            approximate_period_end(&q1),
            Date::from_calendar_date(2025, Month::March, 31).expect("valid date")
        );
        let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        assert_eq!(
            approximate_period_end(&q2),
            Date::from_calendar_date(2025, Month::June, 30).expect("valid date")
        );
        let q3 = PeriodId::quarter(2025, 3).expect("valid period fixture");
        assert_eq!(
            approximate_period_end(&q3),
            Date::from_calendar_date(2025, Month::September, 30).expect("valid date")
        );
        let q4 = PeriodId::quarter(2025, 4).expect("valid period fixture");
        assert_eq!(
            approximate_period_end(&q4),
            Date::from_calendar_date(2025, Month::December, 31).expect("valid date")
        );
    }

    #[test]
    fn approximate_period_end_monthly() {
        let jan = PeriodId::month(2025, 1).expect("valid period fixture");
        assert_eq!(
            approximate_period_end(&jan),
            Date::from_calendar_date(2025, Month::January, 31).expect("valid date")
        );
        let feb = PeriodId::month(2024, 2).expect("valid period fixture"); // leap year
        assert_eq!(
            approximate_period_end(&feb),
            Date::from_calendar_date(2024, Month::February, 29).expect("valid date")
        );
        let feb_non_leap = PeriodId::month(2025, 2).expect("valid period fixture");
        assert_eq!(
            approximate_period_end(&feb_non_leap),
            Date::from_calendar_date(2025, Month::February, 28).expect("valid date")
        );
        let jun = PeriodId::month(2025, 6).expect("valid period fixture");
        assert_eq!(
            approximate_period_end(&jun),
            Date::from_calendar_date(2025, Month::June, 30).expect("valid date")
        );
    }

    #[test]
    fn approximate_period_end_semi_annual() {
        let h1 = PeriodId::half(2025, 1).expect("valid period fixture");
        assert_eq!(
            approximate_period_end(&h1),
            Date::from_calendar_date(2025, Month::June, 30).expect("valid date")
        );
        let h2 = PeriodId::half(2025, 2).expect("valid period fixture");
        assert_eq!(
            approximate_period_end(&h2),
            Date::from_calendar_date(2025, Month::December, 31).expect("valid date")
        );
    }

    #[test]
    fn approximate_period_end_annual() {
        let y = PeriodId::annual(2025);
        assert_eq!(
            approximate_period_end(&y),
            Date::from_calendar_date(2025, Month::December, 31).expect("valid date")
        );
    }

    #[test]
    fn covenant_volatility_is_not_inferred_from_earnings_drivers() {
        use finstack_quant_core::dates::Period;
        use finstack_quant_statements::types::{ForecastMethod, ForecastSpec, NodeSpec, NodeType};

        let period = Period {
            id: PeriodId::quarter(2025, 1).expect("valid period"),
            start: time::macros::date!(2025 - 01 - 01),
            end: time::macros::date!(2025 - 04 - 01),
            is_actual: false,
        };
        let periods = [period.id];
        let mut model = FinancialModelSpec::new("volatility", vec![period]);
        let results = StatementResult {
            nodes: IndexMap::from([("leverage".into(), IndexMap::from([(periods[0], 3.0)]))]),
            ..Default::default()
        };
        let covenant = CovenantSpec::with_metric(
            Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 4.0 },
                Tenor::quarterly(),
                "leverage",
            ),
            "leverage",
        );
        for method in [ForecastMethod::Normal, ForecastMethod::LogNormal] {
            let driver = NodeSpec::new("ebitda", NodeType::Mixed).with_forecast(ForecastSpec {
                method,
                params: IndexMap::from([
                    ("std_dev".into(), json!(10_000_000.0)),
                    ("seed".into(), json!(7)),
                ]),
            });
            model.nodes.insert("ebitda".into(), driver);
            let config = CovenantForecastConfig {
                stochastic: true,
                reference_date: Some(time::macros::date!(2024 - 03 - 31)),
                ..Default::default()
            };
            assert!(
                forecast_covenant(&covenant, &model, &results, &periods, config.clone()).is_err()
            );
            let forecast = forecast_covenant(
                &covenant,
                &model,
                &results,
                &periods,
                CovenantForecastConfig {
                    volatility: Some(0.2),
                    ..config
                },
            )
            .expect("explicit metric volatility");
            assert!(forecast.breach_probability[0] > 0.0);
            assert!(forecast.breach_probability[0] < 0.5);
        }
    }

    #[test]
    fn forecast_breaches_with_partial_metric_coverage() {
        let mut engine = CovenantEngine::new();
        engine.add_spec(CovenantSpec {
            covenant: Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 4.0 },
                Tenor::quarterly(),
                "max_debt_ebitda",
            ),
            metric_id: Some(CovenantMetricId::from("NetDebtEbitda")),
            denominator_metric_id: None,
            threshold_schedule: None,
        });

        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");

        let mut nodes = IndexMap::new();
        // The covenant metric only covers p1 (breaching).
        nodes.insert("NetDebtEbitda".to_string(), IndexMap::from([(p1, 4.5)]));
        // Another node extends the period union to p2.
        nodes.insert(
            "revenue".to_string(),
            IndexMap::from([(p1, 100.0), (p2, 110.0)]),
        );

        let results = StatementResult {
            nodes,
            ..StatementResult::default()
        };

        assert!(
            forecast_breaches(&results, &engine, None, CovenantForecastConfig::default()).is_err()
        );
    }

    #[test]
    fn test_forecast_breaches_concrete() {
        let mut engine = CovenantEngine::new();
        let covenant = Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 4.0 },
            Tenor::quarterly(),
            "max_debt_ebitda",
        );
        let spec = CovenantSpec {
            covenant,
            metric_id: Some(CovenantMetricId::from("NetDebtEbitda")),
            denominator_metric_id: None,
            threshold_schedule: None,
        };
        engine.add_spec(spec);

        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");

        let mut nodes = IndexMap::new();
        let mut net_debt_ebitda = IndexMap::new();
        net_debt_ebitda.insert(p1, 3.0); // Pass
        net_debt_ebitda.insert(p2, 4.5); // Fail
        nodes.insert("NetDebtEbitda".to_string(), net_debt_ebitda);

        let results = StatementResult {
            nodes,
            ..StatementResult::default()
        };

        let config = CovenantForecastConfig::default();
        let breaches =
            forecast_breaches(&results, &engine, None, config).expect("Forecast should succeed");

        assert_eq!(breaches.len(), 1);
        assert_eq!(breaches[0].covenant_id, "max_debt_ebitda");
        assert_eq!(breaches[0].covenant_description, "Debt/EBITDA <= 4.00x");
        assert_eq!(breaches[0].projected_value, Some(4.5));

        // Verify date approximation (Q2 2025 -> June 30)
        let expected_date = Date::from_calendar_date(2025, Month::June, 30)
            .expect("June 30, 2025 should be a valid date");
        assert_eq!(breaches[0].breach_date, expected_date);
    }
}

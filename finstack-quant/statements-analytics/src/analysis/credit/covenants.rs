//! Covenant forecasting bridge for statements.
//!
//! This module provides the integration between financial statement forecasts
//! and the covenant engine, allowing for future compliance checking.

use finstack_quant_core::dates::{Date, PeriodId};
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

/// Adapter to use Statements StatementResult as a ModelTimeSeries.
///
/// This is primarily useful when integrating statement outputs with the
/// covenant engine without re-shaping data into a separate time-series object.
pub struct StatementsAdapter<'a> {
    model: &'a FinancialModelSpec,
    results: &'a StatementResult,
}

impl<'a> StatementsAdapter<'a> {
    /// Create an adapter with the exact model calendar.
    ///
    /// # Arguments
    /// * `results` - Evaluated metric observations keyed by model period.
    /// * `model` - Source model providing actual calendar and fiscal boundaries.
    pub fn new(results: &'a StatementResult, model: &'a FinancialModelSpec) -> Self {
        Self { model, results }
    }
}

impl<'a> ModelTimeSeries for StatementsAdapter<'a> {
    fn get_scalar(&self, node_id: &str, period: &PeriodId) -> Option<f64> {
        self.results.get(node_id, period)
    }

    fn period_end_date(&self, period: &PeriodId) -> Result<Date> {
        self.model
            .periods
            .iter()
            .find(|p| p.id == *period)
            .and_then(|p| p.end.previous_day())
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "Missing or invalid model period {period}"
                ))
            })
    }
    fn period_start_date(&self, period: &PeriodId) -> Result<Date> {
        self.model
            .periods
            .iter()
            .find(|p| p.id == *period)
            .map(|p| p.start)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!("Missing model period {period}"))
            })
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
    let adapter = StatementsAdapter::new(base_case, model);
    forecast_covenant_generic(covenant, &adapter, periods, config)
}

/// Forecast covenant breaches based on statement results.
///
/// # Arguments
///
/// * `results` - The forecast results (time-series of metrics)
/// * `covenants` - The covenant engine containing covenant specifications
/// * `model` - Required source model containing exact calendar and fiscal period boundaries
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
    model: &FinancialModelSpec,
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
    use finstack_quant_core::dates::{Date, Month, Tenor};
    use finstack_quant_covenants::CovenantType;
    use finstack_quant_covenants::{Covenant, CovenantEngine, CovenantMetricId, CovenantSpec};
    use finstack_quant_statements::evaluator::StatementResult;
    use indexmap::IndexMap;

    #[test]
    fn fiscal_horizon_uses_model_boundaries() {
        use finstack_quant_core::dates::Period;
        use time::macros::date;
        let id: PeriodId = "FY2025Q1".parse().unwrap();
        let model = FinancialModelSpec::new(
            "fiscal",
            vec![Period {
                id,
                start: date!(2024 - 07 - 01),
                end: date!(2024 - 10 - 01),
                is_actual: false,
            }],
        );
        let results = StatementResult {
            nodes: IndexMap::from([("leverage".into(), IndexMap::from([(id, 3.)]))]),
            ..Default::default()
        };
        let spec = CovenantSpec::with_metric(
            Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 4. },
                Tenor::quarterly(),
                "lev",
            ),
            "leverage",
        );
        let config = CovenantForecastConfig {
            stochastic: true,
            volatility: Some(0.3),
            ..Default::default()
        };
        let automatic = forecast_covenant(&spec, &model, &results, &[id], config.clone()).unwrap();
        let explicit = forecast_covenant(
            &spec,
            &model,
            &results,
            &[id],
            CovenantForecastConfig {
                reference_date: Some(date!(2024 - 06 - 30)),
                ..config
            },
        )
        .unwrap();
        assert!(automatic.breach_probability[0] > 0.);
        assert_eq!(automatic.breach_probability, explicit.breach_probability);
        assert_eq!(automatic.test_dates, vec![date!(2024 - 09 - 30)]);
        let missing: PeriodId = "FY2025Q2".parse().unwrap();
        assert!(forecast_covenant(
            &spec,
            &model,
            &results,
            &[missing],
            CovenantForecastConfig::default()
        )
        .is_err());
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

        assert!(forecast_breaches(
            &results,
            &engine,
            &finstack_quant_statements::builder::ModelBuilder::new("calendar")
                .periods("2025Q1..Q2", None)
                .unwrap()
                .build()
                .unwrap(),
            CovenantForecastConfig::default()
        )
        .is_err());
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
        let breaches = forecast_breaches(
            &results,
            &engine,
            &finstack_quant_statements::builder::ModelBuilder::new("calendar")
                .periods("2025Q1..Q2", None)
                .unwrap()
                .build()
                .unwrap(),
            config,
        )
        .expect("Forecast should succeed");

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

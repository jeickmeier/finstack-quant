//! Covenant bridge for statements.
//!
//! This module connects evaluated statement results to the covenant engine:
//! [`evaluate_covenants`] tests compliance at every result period end and
//! [`forecast_breaches`] projects breaches over the same periods.

use finstack_quant_core::dates::{Date, PeriodId};
use finstack_quant_core::Result;
use finstack_quant_covenants::{
    evaluate_series_generic, forecast_breaches_generic, CovenantEngine, CovenantForecastConfig,
    DatedCovenantReports, FutureBreach, ModelTimeSeries,
};
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::FinancialModelSpec;

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

/// Every period that carries a value in `results`, in ascending order.
fn result_periods(results: &StatementResult) -> Vec<PeriodId> {
    let mut periods: Vec<PeriodId> = results
        .nodes
        .values()
        .flat_map(|map| map.keys().copied())
        .collect();
    periods.sort();
    periods.dedup();
    periods
}

/// Evaluate covenant compliance at the end of every statement result period.
///
/// Each period that carries a value in `results` is tested on its inclusive
/// end date from the model calendar, with the specifications applicable on
/// that date: a [`CovenantWindow`](finstack_quant_covenants::CovenantWindow)
/// covering the date replaces the engine's top-level set, and waivers and
/// threshold schedules apply as in
/// [`CovenantEngine::evaluate`]. A metric is read only when an applicable
/// covenant needs it, so a node that is missing or non-finite in a period
/// where no applicable covenant reads it (for example a trailing ratio before
/// four quarters of history, outside the window that tests it) does not fail
/// the evaluation. The covenant engine's breach history is not updated.
///
/// This is the point-in-time twin of [`forecast_breaches`]: both read the same
/// periods through [`StatementsAdapter`] and fail on the same missing or
/// non-finite required inputs.
///
/// # Arguments
///
/// * `results` - Evaluated statement results; node ids are the covenant
///   metric ids (`metric_id`, or each covenant type's conventional name), with
///   ratios in turns (`3.5` means 3.5x) and amounts in the model currency.
/// * `covenants` - Valid covenant engine whose specifications, windows,
///   waivers and threshold schedules are evaluated.
/// * `model` - Model whose period calendar gives each result period's test
///   date (the day before the period's exclusive end).
///
/// # Returns
///
/// One [`DatedCovenantReports`] per result period in ascending period order,
/// each with the reports of the covenants applicable on that date keyed by
/// covenant instance key in engine order. Empty when `results` holds no
/// values.
///
/// # Errors
///
/// Returns a validation error for an invalid engine, a result period missing
/// from the model calendar, or a non-finite value of a metric an applicable
/// covenant reads in that period, and `NotFound` when such a metric has no
/// value in the period. No partial series is returned.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{PeriodId, Tenor};
/// use finstack_quant_covenants::{Covenant, CovenantEngine, CovenantSpec, CovenantType};
/// use finstack_quant_statements::builder::ModelBuilder;
/// use finstack_quant_statements::evaluator::StatementResult;
/// use finstack_quant_statements_analytics::analysis::evaluate_covenants;
/// use indexmap::IndexMap;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut builder = ModelBuilder::new("borrower");
/// builder.periods("2025Q1..Q2", None)?;
/// let model = builder.build()?;
/// let results = StatementResult {
///     nodes: IndexMap::from([(
///         "leverage".to_string(),
///         IndexMap::from([
///             (PeriodId::quarter(2025, 1)?, 3.0),
///             (PeriodId::quarter(2025, 2)?, 4.2),
///         ]),
///     )]),
///     ..StatementResult::default()
/// };
/// let mut engine = CovenantEngine::new();
/// engine.add_spec(CovenantSpec::with_metric(
///     Covenant::new(
///         CovenantType::MaxDebtToEbitda { threshold: 4.0 },
///         Tenor::quarterly(),
///         "max_leverage",
///     ),
///     "leverage",
/// ));
///
/// let series = evaluate_covenants(&results, &engine, &model)?;
/// assert_eq!(series[0].as_of.to_string(), "2025-03-31");
/// assert!(series[0].reports["max_leverage"].passed);
/// assert!(!series[1].reports["max_leverage"].passed);
/// # Ok(())
/// # }
/// ```
pub fn evaluate_covenants(
    results: &StatementResult,
    covenants: &CovenantEngine,
    model: &FinancialModelSpec,
) -> Result<Vec<DatedCovenantReports>> {
    let adapter = StatementsAdapter::new(results, model);
    evaluate_series_generic(covenants, &adapter, &result_periods(results))
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
    let adapter = StatementsAdapter::new(results, model);
    forecast_breaches_generic(covenants, &adapter, &result_periods(results), config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::{Date, Tenor};
    use finstack_quant_covenants::forecast_covenant_generic;
    use finstack_quant_covenants::CovenantType;
    use finstack_quant_covenants::{Covenant, CovenantEngine, CovenantMetricId, CovenantSpec};
    use finstack_quant_statements::evaluator::StatementResult;
    use indexmap::IndexMap;
    use serde_json::json;
    use time::Month;

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
        let automatic = forecast_covenant_generic(
            &spec,
            &StatementsAdapter::new(&results, &model),
            &[id],
            config.clone(),
        )
        .unwrap();
        let explicit = forecast_covenant_generic(
            &spec,
            &StatementsAdapter::new(&results, &model),
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
        assert!(forecast_covenant_generic(
            &spec,
            &StatementsAdapter::new(&results, &model),
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
            assert!(forecast_covenant_generic(
                &covenant,
                &StatementsAdapter::new(&results, &model),
                &periods,
                config.clone()
            )
            .is_err());
            let forecast = forecast_covenant_generic(
                &covenant,
                &StatementsAdapter::new(&results, &model),
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
            &{
                let mut model_builder =
                    finstack_quant_statements::builder::ModelBuilder::new("calendar");
                model_builder.periods("2025Q1..Q2", None).unwrap();
                model_builder.build().unwrap()
            },
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
            &{
                let mut model_builder =
                    finstack_quant_statements::builder::ModelBuilder::new("calendar");
                model_builder.periods("2025Q1..Q2", None).unwrap();
                model_builder.build().unwrap()
            },
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

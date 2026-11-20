//! Point-in-time covenant testing over explicitly qualified statement observations.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Period, PeriodId};
use finstack_quant_core::{Error, InputError, Result};
use finstack_quant_covenants::{
    CovenantAssessment, CovenantAssessmentState, CovenantEngine, CovenantForecastConfig,
    CovenantMetricId, CovenantMetricSource, CovenantScope, FutureBreach, ModelTimeSeries,
    forecast_breaches_generic,
};
use finstack_quant_statements::evaluator::{ObservationCutoff, RunProvenance, StatementResult};
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec, NodeValueType};
use rust_decimal::{Decimal, prelude::FromPrimitive};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Mutex;

fn invalid(message: impl Into<String>) -> Error {
    Error::Validation(message.into())
}

/// Comparable units of the elected statement observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CovenantStatementUnit {
    /// Scalar ratio in turns, such as 4.2 for 4.2x.
    Ratio,
    /// Monetary amount whose denomination must match the covenant terms.
    Money {
        /// Expected consistent ISO reporting currency.
        currency: Currency,
    },
}

/// Bind one contractual metric, denominator or springing trigger to a named node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CovenantObservationBinding {
    /// Exact statement node identifier, with no name fallback.
    pub node: String,
    /// Expected monetary or ratio units.
    pub unit: CovenantStatementUnit,
}

/// Authoritative scope, cutoff and statement node bindings for covenant tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StatementCovenantSpec {
    /// Analysis scope applied to all selected contracts.
    pub scope: CovenantScope,
    /// Existing covenant instance IDs, unique and in presentation order.
    pub covenant_ids: Vec<String>,
    /// Existing period IDs, unique and in model timeline order.
    pub periods: Vec<PeriodId>,
    /// Inclusive publication cutoff agreed with the evaluated statement run.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub observed_through: Date,
    /// Named metric aliases and their admitted native units.
    pub bindings: BTreeMap<String, CovenantObservationBinding>,
}

/// Covenant results over one verified statement reporting interval.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StatementCovenantPeriod {
    /// Stored reporting period selected by the caller.
    pub period: PeriodId,
    /// Last included day of the stored fiscal or calendar interval.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Whether observations in this period are historical actuals.
    pub is_actual: bool,
    /// Individual effective terms, status and reports from the covenant owner.
    pub assessments: Vec<CovenantAssessment>,
    /// Available typed observations by contractual metric identity.
    pub observations: BTreeMap<String, AmountOrScalar>,
}

/// Scoped compliance and evidence retained for every requested date.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StatementCovenantAssessment {
    /// Run identity of the admitted statement.
    pub provenance: RunProvenance,
    /// Maintenance or incurrence selection.
    pub scope: CovenantScope,
    /// Historical and projected observations in request order.
    pub periods: Vec<StatementCovenantPeriod>,
}

/// Scoped breach forecast and coverage on the approved information cut.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StatementCovenantForecast {
    /// Elected stochastic reference date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub reference_date: Date,
    /// Evidence and availability for every elected projected period.
    pub assessment: StatementCovenantAssessment,
    /// Native breach probabilities or deterministic breaches from assessed dates.
    pub breaches: Vec<FutureBreach>,
}

struct BoundStatement<'a> {
    results: &'a StatementResult,
    spec: &'a StatementCovenantSpec,
}

impl BoundStatement<'_> {
    fn period(&self, id: &PeriodId) -> Result<&Period> {
        self.results
            .get_periods()
            .iter()
            .find(|period| period.id == *id)
            .ok_or_else(|| {
                invalid(format!(
                    "Covenant period {id} is absent from the admitted run"
                ))
            })
    }

    fn observation(&self, metric: &str, period: &PeriodId) -> Result<AmountOrScalar> {
        let binding = self.spec.bindings.get(metric).ok_or_else(|| {
            invalid(format!(
                "No explicit statement binding for covenant metric '{metric}'"
            ))
        })?;
        let expected = match binding.unit {
            CovenantStatementUnit::Ratio => NodeValueType::Scalar,
            CovenantStatementUnit::Money { currency } => NodeValueType::Monetary { currency },
        };
        if self.results.node_value_types.get(&binding.node) != Some(&expected) {
            return Err(invalid(format!(
                "Covenant metric '{metric}' has a missing node or different stated units"
            )));
        }
        let value = self
            .results
            .get_cell(&binding.node, period)
            .and_then(|cell| cell.value)
            .ok_or_else(|| InputError::NotFound {
                id: metric.to_owned(),
            })?;
        match (binding.unit, value) {
            (CovenantStatementUnit::Ratio, AmountOrScalar::Scalar(scalar))
                if scalar.is_finite() =>
            {
                Ok(value)
            }
            (CovenantStatementUnit::Money { currency }, AmountOrScalar::Amount(money))
                if money.currency() == currency
                    && Decimal::from_f64(money.amount()) == Some(money.amount_decimal()) =>
            {
                Ok(value)
            }
            _ => Err(invalid(format!(
                "Covenant metric '{metric}' has invalid, mismatched or unrepresentable observation units"
            ))),
        }
    }
}

struct PeriodSource<'a> {
    source: &'a BoundStatement<'a>,
    period: PeriodId,
    observations: Mutex<BTreeMap<String, AmountOrScalar>>,
}

impl CovenantMetricSource for PeriodSource<'_> {
    fn get_metric(&self, metric: &CovenantMetricId) -> Result<f64> {
        let value = self.source.observation(metric.as_str(), &self.period)?;
        self.observations
            .lock()
            .map_err(|error| Error::Internal(error.to_string()))?
            .insert(metric.as_str().to_owned(), value);
        Ok(value.value())
    }
}

impl ModelTimeSeries for BoundStatement<'_> {
    fn get_scalar(&self, node_id: &str, period: &PeriodId) -> Option<f64> {
        self.observation(node_id, period)
            .ok()
            .map(|value| value.value())
    }

    fn period_start_date(&self, period: &PeriodId) -> Result<Date> {
        Ok(self.period(period)?.start)
    }

    fn period_end_date(&self, period: &PeriodId) -> Result<Date> {
        self.period(period)?
            .end
            .previous_day()
            .ok_or_else(|| invalid("Covenant period has no included last day"))
    }
}

/// Assess selected covenants against authored observations from a verified statement model.
///
/// # Arguments
/// * `engine` - Covenant terms, schedules, waivers and windows, held read-only.
/// * `model` - Unchanged source model whose periods and source identity produced the results.
/// * `results` - Dated evaluations of the selected source model.
/// * `spec` - Explicit cutoff, scope, selected covenant IDs and ordered dates, plus metric bindings.
///
/// # Errors
/// Rejects mismatched point-in-time provenance, invalid bindings, unknown or
/// duplicate requests and invalid financial observations. Missing or masked
/// applicable observations are recorded as incomplete on their test row.
pub fn assess_statement_covenants(
    engine: &CovenantEngine,
    model: &FinancialModelSpec,
    results: &StatementResult,
    spec: &StatementCovenantSpec,
) -> Result<StatementCovenantAssessment> {
    let verified = results
        .provenance
        .as_ref()
        .filter(|run| {
            run.source_snapshot.is_some()
                && run
                    .valuation_date
                    .is_some_and(|date| date >= spec.observed_through)
                && run.observation_cutoff == ObservationCutoff::Through(spec.observed_through)
        })
        .ok_or_else(|| {
            invalid("Covenant observations require a sourced statement run at the elected cutoff")
        })?;
    let expected = RunProvenance::for_run(
        model,
        verified.observation_cutoff,
        verified.valuation_date,
        verified.market_content_hash.clone(),
    )?;
    if &expected != verified || results.periods.as_ref() != Some(&model.periods) {
        return Err(invalid(
            "Covenant statement run does not match its source model or period plan",
        ));
    }
    let source = BoundStatement { results, spec };
    let ordered = spec
        .periods
        .iter()
        .map(|id| source.period(id))
        .collect::<Result<Vec<_>>>()?;
    if ordered.is_empty()
        || ordered
            .windows(2)
            .any(|window| window[0].end >= window[1].end)
    {
        return Err(invalid(
            "Covenant assessment needs distinct periods in statement order",
        ));
    }
    let mut rows = Vec::with_capacity(ordered.len());
    for period in ordered {
        let date = source.period_end_date(&period.id)?;
        if period.is_actual && date > spec.observed_through {
            return Err(invalid(
                "Actual covenant period closes after the elected information cutoff",
            ));
        }
        let period_source = PeriodSource {
            source: &source,
            period: period.id,
            observations: Mutex::default(),
        };
        let assessments =
            engine.assess_selected(&period_source, date, spec.scope, &spec.covenant_ids)?;
        let observations = period_source
            .observations
            .into_inner()
            .map_err(|error| Error::Internal(error.to_string()))?;
        rows.push(StatementCovenantPeriod {
            period: period.id,
            date,
            is_actual: period.is_actual,
            assessments,
            observations,
        });
    }
    Ok(StatementCovenantAssessment {
        provenance: verified.clone(),
        scope: spec.scope,
        periods: rows,
    })
}

/// Forecast breaches on the explicitly selected, assessed projected intervals.
///
/// # Arguments
/// * `engine` - Immutable covenant engine selecting effective terms and lender waivers.
/// * `model` - Original statement model on the elected publication basis.
/// * `results` - Verified evaluated observations carrying the approved period plan.
/// * `spec` - The selected forecasts, covenant identities, bindings and information cutoff.
/// * `config` - Elected stochastic or deterministic policy with an explicit reference date.
///
/// # Errors
/// Rejects invalid or mismatched scope, actual periods, missing reference date,
/// and inadmissible volatility or forecast inputs. Coverage remains visible
/// for projected periods on which measurements were incomplete.
pub fn forecast_statement_covenants(
    engine: &CovenantEngine,
    model: &FinancialModelSpec,
    results: &StatementResult,
    spec: &StatementCovenantSpec,
    config: CovenantForecastConfig,
) -> Result<StatementCovenantForecast> {
    let reference_date = config
        .reference_date
        .filter(|date| *date >= spec.observed_through)
        .ok_or_else(|| {
            invalid("Covenant forecast requires an explicit reference date on or after the cutoff")
        })?;
    if config.scope != spec.scope || (config.stochastic && config.volatility.is_none()) {
        return Err(invalid(
            "Covenant forecast requires the elected scope and explicit stochastic volatility",
        ));
    }
    let assessment = assess_statement_covenants(engine, model, results, spec)?;
    if assessment
        .periods
        .iter()
        .any(|period| period.is_actual || period.date <= reference_date)
    {
        return Err(invalid(
            "Covenant forecasts require projected test dates after the reference date",
        ));
    }
    let time_series = BoundStatement { results, spec };
    let mut breaches = Vec::new();
    let mut forecasted = false;
    for id in &spec.covenant_ids {
        let periods = assessment
            .periods
            .iter()
            .filter(|period| {
                period.assessments.iter().any(|outcome| {
                    outcome.covenant_id == *id && outcome.state == CovenantAssessmentState::Assessed
                })
            })
            .map(|period| period.period)
            .collect::<Vec<_>>();
        if periods.is_empty() {
            continue;
        }
        forecasted = true;
        let mut scoped = engine.clone();
        scoped.specs.retain(|test| test.covenant.label == *id);
        for window in &mut scoped.windows {
            window.covenants.retain(|test| test.covenant.label == *id);
        }
        scoped.waivers.retain(|waiver| waiver.covenant_id == *id);
        breaches.extend(forecast_breaches_generic(
            &scoped,
            &time_series,
            &periods,
            config.clone(),
        )?);
    }
    if !forecasted {
        return Ok(StatementCovenantForecast {
            reference_date,
            assessment,
            breaches: forecast_breaches_generic(engine, &time_series, &[], config)?,
        });
    }
    breaches.sort_by(|a, b| {
        a.breach_date
            .cmp(&b.breach_date)
            .then_with(|| a.covenant_id.cmp(&b.covenant_id))
    });
    Ok(StatementCovenantForecast {
        reference_date,
        assessment,
        breaches,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::{dates::Tenor, market_data::context::MarketContext};
    use finstack_quant_covenants::{
        Covenant, CovenantSpec, CovenantType, CovenantWaiver, SpringingCondition, ThresholdTest,
    };
    use finstack_quant_statements::{builder::ModelBuilder, evaluator::Evaluator};
    use time::macros::date;

    #[test]
    fn qualified_covenants_keep_breach_missing_and_untriggered_evidence() {
        let q1 = PeriodId::quarter(2025, 1).unwrap();
        let q2 = PeriodId::quarter(2025, 2).unwrap();
        let day = date!(2024 - 12 - 31);
        let mut b = ModelBuilder::new("observations");
        b.periods("2025Q1..Q2", None).unwrap();
        b.source_snapshot("approved-forecast");
        b.value_scalar("net_leverage", &[(q1, 4.2)]);
        b.value_scalar("utilization", &[(q1, 0.3), (q2, 0.3)]);
        let model = b.build().unwrap();
        let results = Evaluator::new()
            .evaluate_with_market(
                &model,
                &MarketContext::new(),
                day,
                ObservationCutoff::Through(day),
            )
            .unwrap();

        let mut engine = CovenantEngine::new();
        let ordinary = |label| {
            Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 4.0 },
                Tenor::quarterly(),
                label,
            )
        };
        engine.add_spec(CovenantSpec::with_metric(ordinary("leverage"), "leverage"));
        let mut inactive = ordinary("inactive");
        inactive.is_active = false;
        engine.add_spec(CovenantSpec::with_metric(inactive, "never-used"));
        let mut spring = ordinary("spring");
        spring.springing_condition = Some(SpringingCondition {
            metric_id: CovenantMetricId::from("utilization"),
            test: ThresholdTest::Minimum(0.5),
        });
        engine.add_spec(CovenantSpec::with_metric(spring, "never-used"));
        engine.add_spec(CovenantSpec::with_metric(ordinary("waived"), "never-used"));
        engine.waivers.push(CovenantWaiver {
            covenant_id: "waived".to_owned(),
            effective_date: day,
            expiry_date: None,
            amended_threshold: None,
            description: "training waiver".to_owned(),
        });
        let spec = StatementCovenantSpec {
            scope: CovenantScope::Maintenance,
            covenant_ids: ["leverage", "inactive", "spring", "waived"]
                .map(str::to_owned)
                .to_vec(),
            periods: vec![q1, q2],
            observed_through: day,
            bindings: BTreeMap::from([
                (
                    "leverage".into(),
                    CovenantObservationBinding {
                        node: "net_leverage".into(),
                        unit: CovenantStatementUnit::Ratio,
                    },
                ),
                (
                    "utilization".into(),
                    CovenantObservationBinding {
                        node: "utilization".into(),
                        unit: CovenantStatementUnit::Ratio,
                    },
                ),
            ]),
        };
        let reported = assess_statement_covenants(&engine, &model, &results, &spec).unwrap();
        let first = &reported.periods[0];
        let second = &reported.periods[1];
        assert_eq!(first.date, date!(2025 - 03 - 31));
        assert!(!first.is_actual);
        assert_eq!(
            first
                .assessments
                .iter()
                .map(|row| row.state)
                .collect::<Vec<_>>(),
            [
                CovenantAssessmentState::Assessed,
                CovenantAssessmentState::Inactive,
                CovenantAssessmentState::Untriggered,
                CovenantAssessmentState::Waived
            ],
        );
        let measured = first.assessments[0].report.as_ref().unwrap();
        assert!(!measured.passed);
        assert!((measured.headroom_raw.unwrap() + 0.2).abs() < 1e-12);
        assert_eq!(
            second.assessments[0].state,
            CovenantAssessmentState::Incomplete
        );
        assert_eq!(
            second.assessments[0].missing_metric.as_deref(),
            Some("leverage")
        );
        assert!(engine.breach_history.is_empty());
        let forecast = forecast_statement_covenants(
            &engine,
            &model,
            &results,
            &spec,
            CovenantForecastConfig {
                reference_date: Some(day),
                ..CovenantForecastConfig::default()
            },
        )
        .unwrap();
        assert_eq!(forecast.breaches.len(), 1);
        assert_eq!(
            forecast.assessment.periods[1].assessments[0].state,
            CovenantAssessmentState::Incomplete
        );
        assert!(engine.breach_history.is_empty());
    }
}

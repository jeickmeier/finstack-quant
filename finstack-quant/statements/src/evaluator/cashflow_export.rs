//! Export statement node series as dated cashflow schedules.
//!
//! This is useful for bridging evaluated statement models (period-based) into
//! valuation instruments that operate on dated cashflows (e.g. real estate NOI DCFs).

use crate::error::{Error, Result};
use crate::types::FinancialModelSpec;
use finstack_quant_core::dates::Date;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use super::StatementResult;

/// Convention for mapping a statement period into a point-in-time cashflow date.
///
/// Serialized and parsed as `"start"` / `"end"`; the default is
/// [`PeriodDateConvention::End`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PeriodDateConvention {
    /// Use the period start date.
    Start,
    /// Use the last **inclusive** day of the period (`end - 1 day`).
    ///
    /// Statement periods use half-open semantics `[start, end)`, so `end` itself is the next
    /// period boundary. Using `end - 1 day` better matches typical "period end" cashflow timing.
    #[default]
    End,
}

impl FromStr for PeriodDateConvention {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "start" => Ok(Self::Start),
            "end" => Ok(Self::End),
            other => Err(Error::invalid_input(format!(
                "period date convention must be 'start' or 'end', got '{other}'"
            ))),
        }
    }
}

/// Export a statement node as a dated cashflow schedule.
///
/// Iterates periods in `model.periods` order and extracts values from `results` for `node_id`.
/// Each period is mapped to a date using `date_convention`.
///
/// # Arguments
///
/// * `model` - Financial model whose ordered periods define the exported
///   schedule dates.
/// * `results` - Evaluated statement results containing the requested node's
///   period values.
/// * `node_id` - Statement node identifier to export as dated numeric amounts.
/// * `date_convention` - Rule selecting each half-open period's start date or
///   last inclusive day as the cashflow date.
///
/// # Errors
///
/// Returns [`Error::NodeNotFound`] (not-found kind) if `node_id` has no
/// values in `results`.
pub fn node_to_dated_schedule(
    model: &FinancialModelSpec,
    results: &StatementResult,
    node_id: &str,
    date_convention: PeriodDateConvention,
) -> Result<Vec<(Date, f64)>> {
    let node_map = results
        .get_node(node_id)
        .ok_or_else(|| Error::node_not_found(node_id))?;

    let mut out = Vec::new();
    for period in &model.periods {
        if let Some(v) = node_map.get(&period.id).copied() {
            let d = match date_convention {
                PeriodDateConvention::Start => period.start,
                PeriodDateConvention::End => {
                    if period.end <= period.start {
                        period.start
                    } else {
                        period.end - time::Duration::days(1)
                    }
                }
            };
            out.push((d, v));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::ModelBuilder;
    use crate::evaluator::Evaluator;
    use crate::types::AmountOrScalar;
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_core::error::ErrorKind;

    fn model() -> FinancialModelSpec {
        let q1 = PeriodId::quarter(2025, 1).expect("valid period");
        let q2 = PeriodId::quarter(2025, 2).expect("valid period");
        ModelBuilder::new("dated")
            .periods("2025Q1..Q2", None)
            .expect("valid periods")
            .value(
                "revenue",
                &[
                    (q1, AmountOrScalar::scalar(100.0)),
                    (q2, AmountOrScalar::scalar(110.0)),
                ],
            )
            .build()
            .expect("valid model")
    }

    #[test]
    fn convention_parses_serializes_and_defaults_to_end() {
        assert_eq!(
            "start".parse::<PeriodDateConvention>().ok(),
            Some(PeriodDateConvention::Start)
        );
        assert_eq!(
            "end".parse::<PeriodDateConvention>().ok(),
            Some(PeriodDateConvention::End)
        );
        assert!("End".parse::<PeriodDateConvention>().is_err());
        assert_eq!(PeriodDateConvention::default(), PeriodDateConvention::End);
        assert_eq!(
            serde_json::to_string(&PeriodDateConvention::Start).expect("serialize"),
            "\"start\""
        );
    }

    #[test]
    fn end_convention_uses_last_inclusive_day_and_unknown_node_is_not_found() {
        let model = model();
        let results = Evaluator::new().evaluate(&model).expect("evaluate");
        let end = node_to_dated_schedule(&model, &results, "revenue", PeriodDateConvention::End)
            .expect("schedule");
        let start =
            node_to_dated_schedule(&model, &results, "revenue", PeriodDateConvention::Start)
                .expect("schedule");
        assert_eq!(end[0].0.to_string(), "2025-03-31");
        assert_eq!(end[1], (end[1].0, 110.0));
        assert_eq!(start[0].0.to_string(), "2025-01-01");

        let err = node_to_dated_schedule(&model, &results, "ebitda", PeriodDateConvention::End)
            .expect_err("unknown node");
        assert_eq!(err.kind(), ErrorKind::NotFound);
    }
}

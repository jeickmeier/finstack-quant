//! Export statement node series as dated schedules.
//!
//! A dated schedule anchors each result period at a date the caller chooses.
//! The dates are period anchors, not payment dates. Pricing reads a schedule
//! only through [`node_to_pricing_flows`], which names the monetary node, its
//! currency, its sign convention and the periods that must all be present.

use crate::error::{Error, Result};
use crate::types::AmountOrScalar;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Period, PeriodId, PeriodPlan};
use finstack_quant_core::money::Money;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use super::{ResultCell, StatementResult};

/// Which date of a half-open period `[start, end)` anchors its exported cell.
///
/// Serialized and parsed as `"start"` / `"end"`. There is no default: every
/// dated export states its anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PeriodDateConvention {
    /// The first day of the period.
    Start,
    /// The last day included in the period (`end - 1 day`).
    End,
}

impl PeriodDateConvention {
    fn anchor(self, period: &Period) -> Date {
        match self {
            Self::Start => period.start,
            Self::End => crate::capital_structure::period_flows::period_snapshot_date(period),
        }
    }
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

/// One period of a dated export: the anchor date, the period and its cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DatedCell {
    /// Anchor date selected by the export's [`PeriodDateConvention`].
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub date: Date,
    /// Period the cell belongs to.
    pub period_id: PeriodId,
    /// The typed cell: an exact amount or scalar, or its missing, masked or
    /// unavailable state.
    pub cell: ResultCell,
}

/// What a positive amount of a statement node means at the pricing boundary.
///
/// Serialized as `"inflow_positive"` / `"outflow_magnitude"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FlowSign {
    /// The node is a signed economic cash flow: positive is cash received.
    /// Amounts pass through unchanged.
    InflowPositive,
    /// The node is a non-negative outflow magnitude such as CapEx. Each amount
    /// is negated once; a negative amount is an error.
    OutflowMagnitude,
}

/// The monetary flow a pricing caller reads from a statement result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PricingFlowSelection {
    /// Monetary node to read.
    pub node_id: String,
    /// Currency every amount must carry. No FX is applied.
    pub currency: Currency,
    /// Sign convention of the node's amounts.
    pub sign: FlowSign,
    /// Periods the schedule requires, in any order. Each must hold an amount.
    pub periods: Vec<PeriodId>,
    /// Anchor date of each period.
    pub date_convention: PeriodDateConvention,
}

fn result_plan(results: &StatementResult) -> Result<&PeriodPlan> {
    results.periods.as_ref().ok_or_else(|| {
        Error::invalid_input("A dated export needs the result's period plan, which is absent")
    })
}

/// Export every period of one node as dated cells.
///
/// Periods come from the result's own period plan in timeline order. A period
/// whose cell has no value stays in the schedule with its status and reason,
/// so a gap is visible. Amounts keep their currency and sign.
///
/// # Arguments
///
/// * `results` - Evaluated statement result; its period plan supplies the
///   periods and their dates.
/// * `node_id` - Statement node identifier to export.
/// * `date_convention` - Whether each period is anchored at its first day or
///   its last included day.
///
/// # Errors
///
/// Returns a not-found error if `node_id` is not in `results`, and an
/// invalid-input error if the result carries no period plan.
pub fn node_to_dated_schedule(
    results: &StatementResult,
    node_id: &str,
    date_convention: PeriodDateConvention,
) -> Result<Vec<DatedCell>> {
    let cells = results
        .get_node(node_id)
        .ok_or_else(|| Error::node_not_found(node_id))?;
    Ok(result_plan(results)?
        .iter()
        .map(|period| DatedCell {
            date: date_convention.anchor(period),
            period_id: period.id,
            cell: cells
                .get(&period.id)
                .cloned()
                .unwrap_or_else(|| ResultCell::without_value(super::CellStatus::Missing)),
        })
        .collect())
}

/// Read one monetary node as signed economic cash flows for pricing.
///
/// Every requested period must hold an exact amount in the expected currency.
/// The result is in timeline order with positive meaning cash received: an
/// [`FlowSign::OutflowMagnitude`] node is negated once.
///
/// # Arguments
///
/// * `results` - Evaluated statement result; its period plan supplies the
///   periods and their dates.
/// * `selection` - The node, expected currency, sign convention, required
///   periods and anchor date. No FX conversion and no sign inference is
///   applied.
///
/// # Errors
///
/// Returns a not-found error if the node is not in `results`. Returns an
/// invalid-input error naming the node and period when the result has no
/// period plan, a requested period is repeated or not in the plan, a cell is
/// missing, masked, unavailable or a scalar, an amount is in another
/// currency, or an outflow magnitude is negative.
pub fn node_to_pricing_flows(
    results: &StatementResult,
    selection: &PricingFlowSelection,
) -> Result<Vec<(Date, Money)>> {
    let node_id = selection.node_id.as_str();
    let cells = results
        .get_node(node_id)
        .ok_or_else(|| Error::node_not_found(node_id))?;
    let plan = result_plan(results)?;
    for (index, requested) in selection.periods.iter().enumerate() {
        if selection.periods[..index].contains(requested) {
            return Err(Error::invalid_input(format!(
                "Pricing flow for node '{node_id}' requests period {requested} more than once"
            )));
        }
        if !plan.iter().any(|period| period.id == *requested) {
            return Err(Error::invalid_input(format!(
                "Pricing flow for node '{node_id}' requests period {requested}, which is not in \
                 the result"
            )));
        }
    }
    plan.iter()
        .filter(|period| selection.periods.contains(&period.id))
        .map(|period| {
            let cell = cells.get(&period.id);
            let amount = cell
                .and_then(|cell| cell.value.as_ref())
                .and_then(AmountOrScalar::as_money)
                .ok_or_else(|| {
                let found = match cell {
                    Some(cell) if cell.numeric().is_some() => "a scalar, not a monetary amount",
                    Some(cell) => cell.status.as_str(),
                    None => "missing",
                };
                Error::invalid_input(format!(
                    "Pricing flow for node '{node_id}' requires an amount in period {}, but the \
                     cell is {found}",
                    period.id
                ))
            })?;
            if amount.currency() != selection.currency {
                return Err(Error::invalid_input(format!(
                    "Pricing flow for node '{node_id}' expects {} but period {} is in {}; no \
                     implicit FX is applied",
                    selection.currency,
                    period.id,
                    amount.currency()
                )));
            }
            let signed = match selection.sign {
                FlowSign::InflowPositive => amount,
                FlowSign::OutflowMagnitude if amount.amount() < 0.0 => {
                    return Err(Error::invalid_input(format!(
                        "Pricing flow for node '{node_id}' is declared an outflow magnitude but \
                         period {} holds the negative amount {amount}",
                        period.id
                    )));
                }
                FlowSign::OutflowMagnitude => amount.checked_neg(),
            };
            Ok((selection.date_convention.anchor(period), signed))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::ModelBuilder;
    use crate::evaluator::{CellStatus, Evaluator};
    use crate::types::FinancialModelSpec;
    use finstack_quant_core::ErrorKind;

    fn month(month: u8) -> PeriodId {
        PeriodId::month(2025, month).expect("valid period")
    }

    /// Monthly CapEx with USD 125.01 in March and no April observation.
    fn model() -> FinancialModelSpec {
        let usd = |amount: f64| {
            AmountOrScalar::amount(amount, Currency::USD).expect("representable amount")
        };
        let mut builder = ModelBuilder::new("dated");
        builder
            .periods("2025M03..M05", None)
            .expect("valid periods");
        builder.value("capex", &[(month(3), usd(125.01)), (month(5), usd(80.0))]);
        builder.value(
            "units",
            &[
                (month(3), AmountOrScalar::scalar(4.0)),
                (month(4), AmountOrScalar::scalar(5.0)),
            ],
        );
        builder.build().expect("valid model")
    }

    fn results() -> StatementResult {
        Evaluator::new().evaluate(&model()).expect("evaluate")
    }

    fn selection(sign: FlowSign, periods: &[PeriodId]) -> PricingFlowSelection {
        PricingFlowSelection {
            node_id: "capex".to_string(),
            currency: Currency::USD,
            sign,
            periods: periods.to_vec(),
            date_convention: PeriodDateConvention::End,
        }
    }

    #[test]
    fn convention_parses_and_serializes_without_a_default() {
        assert_eq!(
            "start".parse::<PeriodDateConvention>().ok(),
            Some(PeriodDateConvention::Start)
        );
        assert_eq!(
            "end".parse::<PeriodDateConvention>().ok(),
            Some(PeriodDateConvention::End)
        );
        assert!("End".parse::<PeriodDateConvention>().is_err());
        assert_eq!(
            serde_json::to_string(&PeriodDateConvention::Start).expect("serialize"),
            "\"start\""
        );
    }

    #[test]
    fn dated_schedule_keeps_exact_amounts_and_shows_the_gap() {
        let results = results();
        let end =
            node_to_dated_schedule(&results, "capex", PeriodDateConvention::End).expect("schedule");
        let dates: Vec<String> = end.iter().map(|row| row.date.to_string()).collect();
        assert_eq!(dates, ["2025-03-31", "2025-04-30", "2025-05-31"]);
        let march = results.get_money("capex", &month(3)).expect("amount");
        assert_eq!(end[0].cell.value, Some(AmountOrScalar::Amount(march)));
        assert_eq!(march.amount_decimal().normalize().to_string(), "125.01");
        assert_eq!(march.currency(), Currency::USD);
        assert_eq!(end[1].period_id, month(4));
        assert_eq!(end[1].cell.status, CellStatus::Missing);

        let start = node_to_dated_schedule(&results, "capex", PeriodDateConvention::Start)
            .expect("schedule");
        assert_eq!(start[0].date.to_string(), "2025-03-01");

        let err = node_to_dated_schedule(&results, "ebitda", PeriodDateConvention::End)
            .expect_err("unknown node");
        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[test]
    fn dated_schedule_needs_the_result_period_plan() {
        let mut results = results();
        results.periods = None;
        let err = node_to_dated_schedule(&results, "capex", PeriodDateConvention::End)
            .expect_err("no plan");
        assert!(err.to_string().contains("period plan"), "{err}");
    }

    #[test]
    fn outflow_magnitude_is_negated_once_at_the_pricing_boundary() {
        let flows = node_to_pricing_flows(
            &results(),
            &selection(FlowSign::OutflowMagnitude, &[month(5), month(3)]),
        )
        .expect("flows");
        let rows: Vec<(String, String)> = flows
            .iter()
            .map(|(date, amount)| {
                (
                    date.to_string(),
                    amount.amount_decimal().normalize().to_string(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("2025-03-31".to_string(), "-125.01".to_string()),
                ("2025-05-31".to_string(), "-80".to_string())
            ]
        );

        let signed = node_to_pricing_flows(
            &results(),
            &selection(FlowSign::InflowPositive, &[month(3)]),
        )
        .expect("flows");
        assert_eq!(
            signed[0].1.amount_decimal().normalize().to_string(),
            "125.01"
        );
    }

    #[test]
    fn pricing_fails_when_the_schedule_requires_the_missing_april() {
        let err = node_to_pricing_flows(
            &results(),
            &selection(FlowSign::OutflowMagnitude, &[month(3), month(4), month(5)]),
        )
        .expect_err("April is missing");
        let message = err.to_string();
        assert!(
            message.contains("capex") && message.contains("2025M04"),
            "{message}"
        );
        assert!(message.contains("missing"), "{message}");
    }

    #[test]
    fn pricing_rejects_wrong_currency_scalars_negative_magnitudes_and_bad_periods() {
        let results = results();
        let mut eur = selection(FlowSign::InflowPositive, &[month(3)]);
        eur.currency = Currency::EUR;
        assert!(
            node_to_pricing_flows(&results, &eur)
                .expect_err("currency")
                .to_string()
                .contains("no implicit FX")
        );

        let mut scalar = selection(FlowSign::InflowPositive, &[month(3)]);
        scalar.node_id = "units".to_string();
        assert!(
            node_to_pricing_flows(&results, &scalar)
                .expect_err("scalar")
                .to_string()
                .contains("scalar")
        );

        let mut negative = results.clone();
        negative.set_money_series(
            "capex",
            [(
                month(3),
                Money::new(-1.0, Currency::USD).expect("representable amount"),
            )]
            .into_iter()
            .collect(),
        );
        assert!(
            node_to_pricing_flows(
                &negative,
                &selection(FlowSign::OutflowMagnitude, &[month(3)])
            )
            .expect_err("negative magnitude")
            .to_string()
            .contains("negative")
        );

        let outside = PeriodId::month(2026, 1).expect("valid period");
        for periods in [vec![outside], vec![month(3), month(3)]] {
            assert!(
                node_to_pricing_flows(&results, &selection(FlowSign::InflowPositive, &periods))
                    .is_err()
            );
        }
        let mut unknown = selection(FlowSign::InflowPositive, &[month(3)]);
        unknown.node_id = "ebitda".to_string();
        assert_eq!(
            node_to_pricing_flows(&results, &unknown)
                .expect_err("unknown")
                .kind(),
            ErrorKind::NotFound
        );
    }
}

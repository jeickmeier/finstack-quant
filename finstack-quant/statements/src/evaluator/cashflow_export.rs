//! Export statement node series as dated cashflow schedules.
//!
//! This is useful for bridging evaluated statement models (period-based) into
//! valuation instruments that operate on dated cashflows (e.g. real estate NOI DCFs).

use crate::error::{Error, Result};
use crate::types::FinancialModelSpec;
use finstack_quant_core::dates::Date;

use super::StatementResult;

/// Convention for mapping a statement period into a point-in-time cashflow date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodDateConvention {
    /// Use the period start date.
    Start,
    /// Use the last **inclusive** day of the period (`end - 1 day`).
    ///
    /// Statement periods use half-open semantics `[start, end)`, so `end` itself is the next
    /// period boundary. Using `end - 1 day` better matches typical "period end" cashflow timing.
    End,
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
/// Returns an error if the node is missing entirely.
pub fn node_to_dated_schedule(
    model: &FinancialModelSpec,
    results: &StatementResult,
    node_id: &str,
    date_convention: PeriodDateConvention,
) -> Result<Vec<(Date, f64)>> {
    let node_map = results
        .get_node(node_id)
        .ok_or_else(|| Error::invalid_input(format!("Unknown node '{node_id}'")))?;

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

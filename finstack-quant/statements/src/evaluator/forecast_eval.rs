//! Forecast evaluation logic.

use crate::error::{Error, Result};
use crate::evaluator::context::EvaluationContext;
use crate::forecast;
use crate::forecast::statistical::{
    monte_carlo_correlated_series, parse_correlation_params, record_independent_z_scores_for_mc,
    CorrelatedMonteCarloSeries,
};
use crate::types::{FinancialModelSpec, ForecastMethod, NodeId, NodeSpec};
use finstack_quant_core::dates::{Date, PeriodId};
use indexmap::IndexMap;

/// Evaluate a forecast for a specific period.
///
/// Determines the base value, applies the configured forecast method, and
/// caches the generated values for future lookups.
///
/// # Arguments
/// * `node_spec` - Node metadata containing forecast configuration
/// * `model` - Financial model definition providing periods
/// * `period_id` - Forecast period being requested
/// * `context` - Evaluation context with historical data
/// * `forecast_cache` - Cache reused across nodes/periods
/// * `visibility_cutoff` - As-of date hiding explicit values of actual
///   observations released after it; hidden actuals are forecast from the
///   latest preceding visible observation
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_forecast(
    node_spec: &NodeSpec,
    model: &FinancialModelSpec,
    period_id: &PeriodId,
    context: &EvaluationContext,
    forecast_cache: &mut IndexMap<NodeId, IndexMap<PeriodId, f64>>,
    seed_offset: Option<u64>,
    mc_z_cache: &mut Option<&mut IndexMap<NodeId, IndexMap<PeriodId, f64>>>,
    visibility_cutoff: Option<Date>,
) -> Result<f64> {
    // Check cache first
    if let Some(cached) = forecast_cache.get(node_spec.node_id.as_str()) {
        if let Some(value) = cached.get(period_id) {
            return Ok(*value);
        }
    }

    // Get forecast spec
    let forecast_spec = node_spec
        .forecast
        .as_ref()
        .ok_or_else(|| Error::eval(format!(
            "No forecast spec for node '{}'. Ensure the node has a forecast defined with .forecast()",
            node_spec.node_id
        )))?;

    // Cache only this uninterrupted forecast segment. A later visible actual
    // starts a new segment and must never seed forecasts for earlier periods.
    let forecast_periods: Vec<PeriodId> = model
        .periods
        .iter()
        .skip_while(|period| period.id < *period_id)
        .take_while(|period| {
            !period.is_actual || !node_spec.explicit_value_is_visible(period, visibility_cutoff)
        })
        .map(|period| period.id)
        .collect();

    if forecast_periods.is_empty() {
        return Err(Error::eval(
            "No forecast periods in model. All periods are marked as actuals. \
             Use .periods(range, Some(actuals_cutoff)) to define forecast periods."
                .to_string(),
        ));
    }

    // Anchor strictly before the segment's first forecast period.
    let base_value = determine_base_value(node_spec, period_id, model, context, visibility_cutoff)?;

    let forecast_results = if let Some(offset) = seed_offset {
        let mut series = match mc_z_cache.as_mut() {
            Some(cache) => match forecast_spec.method {
                ForecastMethod::Normal
                | ForecastMethod::LogNormal
                | ForecastMethod::MeanReverting => {
                    if let Some((peer, rho)) = parse_correlation_params(&forecast_spec.params)? {
                        if model.get_node(peer.as_str()).is_none() {
                            return Err(Error::forecast(format!(
                                "correlation_with references unknown node '{peer}'"
                            )));
                        }
                        let (series, z_map) =
                            monte_carlo_correlated_series(CorrelatedMonteCarloSeries {
                                method: forecast_spec.method,
                                params: &forecast_spec.params,
                                base_value,
                                forecast_periods: &forecast_periods,
                                seed_offset: offset,
                                node_id: node_spec.node_id.as_str(),
                                peer_id: peer.as_str(),
                                rho,
                                mc_z_cache: cache,
                            })?;
                        cache
                            .entry(node_spec.node_id.clone())
                            .or_default()
                            .extend(z_map);
                        series
                    } else {
                        let series = forecast::apply_forecast_seeded(
                            forecast_spec,
                            base_value,
                            &forecast_periods,
                            offset,
                            node_spec.node_id.as_str(),
                        )?;
                        record_independent_z_scores_for_mc(
                            forecast_spec.method,
                            &forecast_spec.params,
                            &forecast_periods,
                            &series,
                            base_value,
                            &node_spec.node_id,
                            cache,
                        )?;
                        series
                    }
                }
                _ => forecast::apply_forecast_seeded(
                    forecast_spec,
                    base_value,
                    &forecast_periods,
                    offset,
                    node_spec.node_id.as_str(),
                )?,
            },
            None => forecast::apply_forecast_seeded(
                forecast_spec,
                base_value,
                &forecast_periods,
                offset,
                node_spec.node_id.as_str(),
            )?,
        };
        // Bounds are applied after Z-score recording so correlation shocks
        // always invert the unclamped recurrence; the single-run path below
        // clamps inside `apply_forecast_for_node`.
        forecast::apply_bounds(&forecast_spec.params, &mut series)?;
        series
    } else {
        forecast::apply_forecast_for_node(
            forecast_spec,
            base_value,
            &forecast_periods,
            node_spec.node_id.as_str(),
        )?
    };

    // Cache results
    forecast_cache.insert(node_spec.node_id.clone(), forecast_results.clone());

    // Return value for requested period
    forecast_results.get(period_id).copied().ok_or_else(|| {
        Error::eval(format!(
            "Forecast did not produce value for period {:?}",
            period_id
        ))
    })
}

/// Determine the base value for forecasting.
///
/// The base value anchors every forecast method (growth, trend, seasonal, etc.)
/// to the most recent observed data point. The resolution order is:
///
/// 1. **Last visible actual period** — if the node has an explicit value or a
///    previously-evaluated result in the most recent `is_actual` period whose
///    observation availability date is on or before the cutoff, that value is
///    used. Values without an explicit availability date default to the
///    period's exclusive end date.
/// 2. **Most recent historical value** — falls back to the chronologically
///    latest value found anywhere in `EvaluationContext::history`.
///    Covers cases where the last actual period has no value for this
///    particular node (e.g., the node was added after earlier periods).
/// 3. **Error** — if no historical data exists at all, evaluation fails with a
///    descriptive message so the analyst can provide at least one actual.
///
/// # Assumption
///
/// The resolved base value is assumed to be correct and representative.
/// Callers should ensure that the actuals feeding the model are clean and
/// reviewed before running forecasts—this function does **not** attempt to
/// detect outliers, stale data, or unit mismatches.
fn determine_base_value(
    node_spec: &NodeSpec,
    current_period_id: &PeriodId,
    model: &FinancialModelSpec,
    context: &EvaluationContext,
    visibility_cutoff: Option<Date>,
) -> Result<f64> {
    // Try to get the last actual period whose observation is visible under
    // the as-of policy.
    let last_actual_period = model.periods.iter().rfind(|period| {
        period.id < *current_period_id
            && period.is_actual
            && node_spec.explicit_value_is_visible(period, visibility_cutoff)
    });
    if let Some(last_actual) = last_actual_period {
        // Check node's explicit values
        if let Some(values) = &node_spec.values {
            if let Some(val) = values.get(&last_actual.id) {
                return Ok(val.value());
            }
        }

        // Check historical context
        if let Some(val) = context.get_historical_value(node_spec.node_id.as_str(), &last_actual.id)
        {
            return Ok(val);
        }
    }

    // Try to find the most recent historical value by chronological ordering
    if let Some(&column) = context.node_to_column.get(node_spec.node_id.as_str()) {
        if let Some((_, value)) = context
            .history
            .iter()
            .filter(|(period, _)| *period < *current_period_id)
            .filter_map(|(period, row)| {
                row.get(column)
                    .copied()
                    .flatten()
                    .map(|value| (period, value))
            })
            .max_by_key(|(period, _)| *period)
        {
            return Ok(value);
        }
    }

    // No base value found
    Err(Error::forecast(format!(
        "Cannot determine base value for forecast of node '{}'. \
         No actual period value or historical value found. \
         Ensure the node has at least one actual period value or historical data.",
        node_spec.node_id
    )))
}

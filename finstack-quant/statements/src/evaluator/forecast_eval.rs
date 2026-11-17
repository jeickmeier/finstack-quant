//! Forecast evaluation logic.

use crate::error::{Error, Result};
use crate::evaluator::cell::UnavailableReason;
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::engine::PeriodEval;
use crate::evaluator::value::Value;
use crate::forecast;
use crate::forecast::statistical::{CorrelatedMonteCarloSeries, monte_carlo_correlated_series};
use crate::types::{FinancialModelSpec, ForecastSpec, NodeId, NodeSpec};
use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;

/// One node's forecast over its suffix in one run: the raw generated series,
/// which carries the recurrence, phase and shocks, and the published series
/// after output bounds. `Unavailable` when the forecast has no base.
#[derive(Debug, Clone)]
pub(crate) enum ForecastRun {
    /// The generated and published levels of every suffix slot.
    Series {
        /// Levels as generated, before output bounds.
        raw: IndexMap<PeriodId, f64>,
        /// Levels after output bounds.
        published: IndexMap<PeriodId, f64>,
    },
    /// The forecast produced no values, for the stated reason.
    Unavailable(UnavailableReason),
}

/// The forecast of one node in one period.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ForecastCell {
    /// The published level, and the raw level when an output bound clipped it.
    Value {
        /// Published level, after output bounds.
        value: f64,
        /// The raw level, when it differs from the published one.
        clipped_from: Option<f64>,
    },
    /// No forecast value, for the stated reason.
    Unavailable(UnavailableReason),
}

/// Evaluate a forecast for a specific period.
///
/// A node has one forecast suffix per run: the model's forecast periods. The
/// whole suffix is generated once and cached, so the
/// recurrence, phase and shocks advance one step per slot whether or not an
/// explicit value or a mask is published there. Output bounds apply to the
/// published levels only.
///
/// The base is the method's selected start level, or else the published
/// value of the slot immediately before the suffix: a hidden, masked, missing
/// or unavailable slot is not a base. Without either, the forecast is
/// unavailable.
///
/// # Arguments
/// * `node_spec` - Node metadata containing forecast configuration
/// * `eval` - Model (providing periods) and the forecast period being requested
/// * `context` - Evaluation context with historical data
/// * `forecast_cache` - Cache reused across nodes/periods
pub(crate) fn evaluate_forecast(
    node_spec: &NodeSpec,
    eval: PeriodEval<'_>,
    context: &EvaluationContext,
    forecast_cache: &mut IndexMap<NodeId, ForecastRun>,
    seed_offset: Option<u64>,
    mc_z_cache: &mut Option<&mut IndexMap<NodeId, IndexMap<PeriodId, f64>>>,
) -> Result<ForecastCell> {
    let model = eval.model;
    let period_id = &eval.period.id;
    if !forecast_cache.contains_key(node_spec.node_id.as_str()) {
        let run = generate_run(
            node_spec,
            model,
            period_id,
            context,
            seed_offset,
            mc_z_cache,
        )?;
        forecast_cache.insert(node_spec.node_id.clone(), run);
    }
    match forecast_cache.get(node_spec.node_id.as_str()) {
        Some(ForecastRun::Unavailable(reason)) => Ok(ForecastCell::Unavailable(*reason)),
        Some(ForecastRun::Series { raw, published }) => {
            // A keyed override without a base leaves the slots before its
            // first key out of the series.
            let Some(value) = published.get(period_id).copied() else {
                return Ok(ForecastCell::Unavailable(
                    UnavailableReason::MissingForecastBase,
                ));
            };
            let clipped_from = raw
                .get(period_id)
                .copied()
                .filter(|raw| raw.to_bits() != value.to_bits() && raw.is_finite());
            Ok(ForecastCell::Value {
                value,
                clipped_from,
            })
        }
        None => Err(Error::eval(format!(
            "Forecast of node '{}' was not generated",
            node_spec.node_id
        ))),
    }
}

/// Generate the node's whole forecast suffix for this run.
fn generate_run(
    node_spec: &NodeSpec,
    model: &FinancialModelSpec,
    period_id: &PeriodId,
    context: &EvaluationContext,
    seed_offset: Option<u64>,
    mc_z_cache: &mut Option<&mut IndexMap<NodeId, IndexMap<PeriodId, f64>>>,
) -> Result<ForecastRun> {
    let forecast_spec = node_spec
        .forecast
        .as_ref()
        .ok_or_else(|| Error::eval(format!(
            "No forecast spec for node '{}'. Ensure the node has a forecast defined with .forecast()",
            node_spec.node_id
        )))?;

    // The canonical suffix is the model's forecast periods, which a
    // canonical plan places after every actual period. Explicit values
    // inside the suffix replace outputs; they do not reset the elapsed
    // steps, phase or draw sequence.
    let Some(opening) = model.periods.iter().position(|p| !p.is_actual) else {
        return Err(Error::eval(format!(
            "Node '{}' was asked for a forecast in {period_id}, but the model has no forecast \
             periods. Use .periods(range, Some(actuals_cutoff)) to define them.",
            node_spec.node_id
        )));
    };
    let forecast_periods: Vec<PeriodId> = model.periods[opening..].iter().map(|p| p.id).collect();

    // The base is the selected start level, or else the published value of
    // the slot immediately before the suffix. A masked, missing or
    // unavailable slot is not a base.
    let preceding_value = opening.checked_sub(1).and_then(|index| {
        let slot = &model.periods[index].id;
        match context
            .history
            .get_operand(node_spec.node_id.as_str(), slot)?
        {
            Value::Unavailable(_) | Value::Bool(_) => None,
            value => Some(value.float()),
        }
    });
    let base_value = match (forecast_spec.start().or(preceding_value), forecast_spec) {
        (Some(base), _) => base,
        // These methods carry their own history or period-keyed values.
        (
            None,
            ForecastSpec::TimeSeries(_) | ForecastSpec::Seasonal(_) | ForecastSpec::Override(_),
        ) => f64::NAN,
        (None, _) => {
            return Ok(ForecastRun::Unavailable(
                UnavailableReason::MissingForecastBase,
            ));
        }
    };

    let parameters = forecast_spec;
    let raw = if let Some(offset) = seed_offset {
        let correlation = if mc_z_cache.is_some() {
            forecast::statistical::correlation(parameters)?
        } else {
            None
        };
        let (series, shocks) =
            if let (Some(cache), Some((peer, rho))) = (mc_z_cache.as_deref(), correlation) {
                if model.get_node(peer).is_none() {
                    return Err(Error::forecast(format!(
                        "correlation_with references unknown node '{peer}'"
                    )));
                }
                let (series, shocks) = monte_carlo_correlated_series(CorrelatedMonteCarloSeries {
                    parameters,
                    base_value,
                    forecast_periods: &forecast_periods,
                    seed_offset: offset,
                    node_id: node_spec.node_id.as_str(),
                    peer_id: peer,
                    rho,
                    mc_z_cache: cache,
                })?;
                (series, Some(shocks))
            } else {
                forecast::apply_forecast(
                    parameters,
                    base_value,
                    &forecast_periods,
                    node_spec.node_id.as_str(),
                    Some(offset),
                )?
            };
        // Sampled shocks are recorded before output bounds: clipping the
        // published levels does not change the latent correlation process.
        if let (Some(cache), Some(shocks)) = (mc_z_cache.as_mut(), shocks) {
            cache
                .entry(node_spec.node_id.clone())
                .or_default()
                .extend(shocks);
        }
        series
    } else {
        forecast::apply_forecast_unbounded(
            parameters,
            base_value,
            &forecast_periods,
            node_spec.node_id.as_str(),
        )?
    };
    let mut published = raw.clone();
    forecast::apply_bounds(parameters, &mut published)?;
    Ok(ForecastRun::Series { raw, published })
}

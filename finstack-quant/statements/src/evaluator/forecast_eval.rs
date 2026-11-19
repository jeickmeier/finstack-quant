//! Forecast evaluation logic.

use crate::error::{Error, Result};
use crate::evaluator::cell::UnavailableReason;
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::engine::PeriodEval;
use crate::evaluator::value::Value;
use crate::forecast;
use crate::forecast::statistical::{CorrelatedMonteCarloSeries, monte_carlo_correlated_series};
use crate::forecast::stream::PathId;
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
    path: Option<PathId>,
    mc_z_cache: &mut Option<&mut IndexMap<NodeId, IndexMap<PeriodId, f64>>>,
) -> Result<ForecastCell> {
    let model = eval.model;
    let period_id = &eval.period.id;
    ensure_run(
        node_spec,
        model,
        period_id,
        context,
        forecast_cache,
        path,
        mc_z_cache,
    )?;
    match forecast_cache.get(node_spec.node_id.as_str()) {
        Some(ForecastRun::Unavailable(reason)) => Ok(ForecastCell::Unavailable(*reason)),
        Some(ForecastRun::Series { raw, published }) => {
            let value = match published.get(period_id).copied() {
                Some(value) => value,
                // A keyed override without a base leaves the slots before
                // its first key out of the series.
                None if matches!(node_spec.forecast, Some(ForecastSpec::Override(_))) => {
                    return Ok(ForecastCell::Unavailable(
                        UnavailableReason::MissingForecastBase,
                    ));
                }
                None => {
                    return Err(Error::eval(format!(
                        "Forecast did not produce value for period {period_id:?}"
                    )));
                }
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

/// Generate and cache the node's forecast run, after the run of its active
/// correlation peer.
///
/// The peer's run is generated on demand, so the order in which nodes are
/// declared or evaluated does not decide whether its shocks exist, and it is
/// generated whether or not the peer publishes explicit values or is masked
/// in the suffix. The model rejects peer cycles when it is built.
fn ensure_run(
    node_spec: &NodeSpec,
    model: &FinancialModelSpec,
    period_id: &PeriodId,
    context: &EvaluationContext,
    forecast_cache: &mut IndexMap<NodeId, ForecastRun>,
    path: Option<PathId>,
    mc_z_cache: &mut Option<&mut IndexMap<NodeId, IndexMap<PeriodId, f64>>>,
) -> Result<()> {
    if forecast_cache.contains_key(node_spec.node_id.as_str()) {
        return Ok(());
    }
    let peer = match &node_spec.forecast {
        Some(forecast) => forecast::statistical::active_correlation(forecast),
        None => None,
    };
    let mut run = None;
    if let Some((peer, _)) = peer {
        if path.is_none() {
            return Err(Error::forecast(format!(
                "Node '{}' is correlated with '{peer}': a non-zero correlation needs Monte \
                 Carlo evaluation. Use evaluate_monte_carlo, or set the correlation to zero.",
                node_spec.node_id
            )));
        }
        let peer_spec = model.get_node(peer).ok_or_else(|| {
            Error::forecast(format!("correlation_with references unknown node '{peer}'"))
        })?;
        ensure_run(
            peer_spec,
            model,
            period_id,
            context,
            forecast_cache,
            path,
            mc_z_cache,
        )?;
        if matches!(forecast_cache.get(peer), Some(ForecastRun::Unavailable(_))) {
            run = Some(ForecastRun::Unavailable(
                UnavailableReason::MissingPeerShock,
            ));
        }
    }
    let run = match run {
        Some(run) => run,
        None => generate_run(node_spec, model, period_id, context, path, mc_z_cache)?,
    };
    forecast_cache.insert(node_spec.node_id.clone(), run);
    Ok(())
}

/// Generate the node's whole forecast suffix for this run.
fn generate_run(
    node_spec: &NodeSpec,
    model: &FinancialModelSpec,
    period_id: &PeriodId,
    context: &EvaluationContext,
    path: Option<PathId>,
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
    let raw = if let Some(path) = path {
        // At exactly zero correlation the node takes the uncorrelated path
        // and draws the same stream as a node without a peer.
        let correlation = if mc_z_cache.is_some() {
            forecast::statistical::active_correlation(parameters)
        } else {
            None
        };
        let (series, shocks) =
            if let (Some(cache), Some((peer, rho))) = (mc_z_cache.as_deref(), correlation) {
                let (series, shocks) = monte_carlo_correlated_series(CorrelatedMonteCarloSeries {
                    parameters,
                    base_value,
                    forecast_periods: &forecast_periods,
                    path,
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
                    Some(path),
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
    forecast::apply_bounds(parameters, &mut published);
    Ok(ForecastRun::Series { raw, published })
}

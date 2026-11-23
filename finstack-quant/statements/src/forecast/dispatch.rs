//! Forecast method dispatch and bound clamping.

use super::deterministic::{curve_pct, forward_fill, growth_pct};
use super::fade::fade_to_target;
use super::override_method::apply_override;
use super::timeseries::{seasonal_forecast, timeseries_forecast};

use crate::error::Result;
use crate::types::ForecastSpec;
use finstack_quant_core::dates::PeriodId;

/// Per-period forecast levels or standard-normal shocks.
pub(super) type ForecastSeries = indexmap::IndexMap<PeriodId, f64>;
/// Apply a forecast for a specific node in single-run (non-Monte-Carlo) mode.
///
/// Runs the method of `parameters` on the node's own single-run stream and
/// applies its `min`/`max` bounds.
#[cfg(test)]
pub(crate) fn apply_forecast_for_node(
    parameters: &ForecastSpec,
    base_value: f64,
    forecast_periods: &[PeriodId],
    node_id: &str,
) -> Result<ForecastSeries> {
    let mut results = apply_forecast_unbounded(parameters, base_value, forecast_periods, node_id)?;
    apply_bounds(parameters, &mut results);
    Ok(results)
}

/// The single-run forecast series as generated, before output bounds.
pub(crate) fn apply_forecast_unbounded(
    parameters: &ForecastSpec,
    base_value: f64,
    forecast_periods: &[PeriodId],
    node_id: &str,
) -> Result<ForecastSeries> {
    apply_forecast(parameters, base_value, forecast_periods, node_id, None)
        .map(|(series, _)| series)
}

/// Run a forecast method without bounds.
///
/// Stochastic methods (normal, log-normal, mean-reverting, bootstrap) draw
/// from the versioned stream of their seed, `node_id` and `path`
/// ([`super::stream::StreamKey`]); `None` is the single-run stream. Deterministic
/// methods ignore both. Monte Carlo callers apply [`apply_bounds`]
/// themselves, after recording correlation shocks.
///
/// Returns the generated levels and the sampled Gaussian shocks, or `None`
/// for methods that do not draw Gaussian shocks. Zero-volatility forecasts
/// record zero shocks so they cannot serve as stochastic correlation peers.
pub(crate) fn apply_forecast(
    parameters: &ForecastSpec,
    base_value: f64,
    forecast_periods: &[PeriodId],
    node_id: &str,
    path: Option<super::stream::PathId>,
) -> Result<(ForecastSeries, Option<ForecastSeries>)> {
    use super::statistical::{
        bootstrap_forecast_with_stream, lognormal_forecast_with_stream,
        mean_reverting_forecast_with_stream, normal_forecast_with_stream,
    };
    let rng = |seed: u64| {
        super::stream::StreamKey {
            seed,
            node_id,
            path,
        }
        .rng()
    };
    match parameters {
        ForecastSpec::Normal(p) => {
            normal_forecast_with_stream(base_value, forecast_periods, p, &mut rng(p.seed)?)
                .map(|(values, shocks)| (values, Some(shocks)))
        }
        ForecastSpec::LogNormal(p) => {
            lognormal_forecast_with_stream(base_value, forecast_periods, p, &mut rng(p.seed)?)
                .map(|(values, shocks)| (values, Some(shocks)))
        }
        ForecastSpec::MeanReverting(p) => {
            mean_reverting_forecast_with_stream(base_value, forecast_periods, p, &mut rng(p.seed)?)
                .map(|(values, shocks)| (values, Some(shocks)))
        }
        ForecastSpec::Bootstrap(p) => {
            bootstrap_forecast_with_stream(base_value, forecast_periods, p, &mut rng(p.seed)?)
                .map(|values| (values, None))
        }
        ForecastSpec::ForwardFill(_) => {
            forward_fill(base_value, forecast_periods).map(|values| (values, None))
        }
        ForecastSpec::GrowthPct(p) => {
            growth_pct(base_value, forecast_periods, p.rate).map(|values| (values, None))
        }
        ForecastSpec::CurvePct(p) => {
            curve_pct(base_value, forecast_periods, &p.curve).map(|values| (values, None))
        }
        ForecastSpec::Override(p) => {
            // A method without a base receives NaN; the keyed carry is the
            // one method that runs without one.
            apply_override(
                (!base_value.is_nan()).then_some(base_value),
                forecast_periods,
                &p.overrides,
            )
            .map(|values| (values, None))
        }
        ForecastSpec::TimeSeries(p) => {
            timeseries_forecast(forecast_periods, p).map(|values| (values, None))
        }
        ForecastSpec::Seasonal(p) => {
            seasonal_forecast(forecast_periods, p).map(|values| (values, None))
        }
        ForecastSpec::FadeToTarget(p) => {
            fade_to_target(base_value, forecast_periods, p).map(|values| (values, None))
        }
    }
}

/// Clamp generated forecast values to the optional `min` / `max` bounds of
/// `parameters`.
///
/// Bounds are a cross-cutting feature accepted by every forecast method:
/// ratios that must stay in a band (payout, capital ratios), levels that
/// cannot go negative (NIM, loss rates), or trend extrapolations that must
/// not explode over long horizons. The clamp is applied to **output values
/// only** — stochastic recurrences evolve unclamped, so a persistent walk can
/// pin at a bound while its underlying path drifts beyond it.
/// Non-finite observations are preserved so bounds cannot hide missing data
/// or suppress the evaluator's non-finite-value diagnostics.
///
/// In Monte Carlo mode the evaluator applies bounds after retaining the
/// sampled shocks. Clipping the reported levels leaves those shocks unchanged.
///
/// # Arguments
///
/// * `parameters` - Forecast parameters; reads the optional `min` and `max`
///   bounds, each a finite number in the node's own units
///   with `min <= max`, as validated with the model
/// * `results` - Generated per-period values, clamped in place
pub(crate) fn apply_bounds(parameters: &ForecastSpec, results: &mut ForecastSeries) {
    let (min, max) = parameters.bounds();
    if min.is_none() && max.is_none() {
        return;
    }

    for value in results.values_mut() {
        // Missing/non-finite observations retain their evaluator diagnostics;
        // f64::min/max would silently turn NaN into the finite bound.
        if !value.is_finite() {
            continue;
        }
        if let Some(lo) = min {
            *value = value.max(lo);
        }
        if let Some(hi) = max {
            *value = value.min(hi);
        }
    }
}

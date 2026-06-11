//! Forecast methods for time-series projection.
//!
//! This module provides various forecast methods for projecting values into
//! future periods, including:
//! - **Deterministic**: ForwardFill, GrowthPct, CurvePct, Override
//! - **Statistical**: Normal, LogNormal (with deterministic seeding)
//! - **TimeSeries**: Seasonal patterns
//!
//! All forecast methods operate on a base value (typically the last actual value)
//! and project forward for a specified number of periods.
//!
//! # Random Number Generation
//!
//! Statistical forecast methods (Normal, LogNormal) require a `seed` parameter
//! for deterministic random number generation. This ensures reproducibility:
//! - Same seed → identical forecast values across runs
//! - Different seeds → different (but still deterministic) values
//!
//! Both single-run evaluation and Monte Carlo mode mix a stable hash of the
//! node identifier into the configured seed so independent stochastic nodes do
//! not share identical shock draws (Monte Carlo additionally layers a per-path
//! seed offset).
//! Optional `correlation_with` / `correlation` parameters pair nodes for correlated
//! shocks (see `forecast::statistical::parse_correlation_params`). The peer node must
//! appear earlier in the evaluation order (for example via a formula dependency) so its
//! Z-scores are available when the dependent node is simulated.
//!
//! The RNG uses the Box-Muller transform for normal distribution sampling,
//! with guards against edge cases (e.g., ln(0)).
//!
//! # Parameter Validation
//!
//! - **std_dev**: Must be non-negative. Zero produces a degenerate distribution.
//! - **rate** (GrowthPct): Rates > 100% per period produce warnings.
//! - **seed**: Required for statistical methods (ensures reproducibility).
//!
//! # Overflow Protection
//!
//! Compound growth methods (GrowthPct, CurvePct) detect and error on overflow
//! conditions to prevent silent numerical failures.
//!
//! # Warnings
//!
//! The following conditions produce log warnings (but not errors):
//! - Growth rates exceeding 100% per period
//! - std_dev = 0.0 in LogNormal (degenerate distribution)
//!
//! For forecast analysis tools (backtesting, covenant breach detection), see
//! the `finstack-statements-analytics` crate.

mod deterministic;
mod override_method;
pub(crate) mod statistical;
mod timeseries;

use deterministic::{curve_pct, forward_fill, growth_pct};
use override_method::apply_override;
use statistical::{lognormal_forecast, normal_forecast};
use timeseries::{seasonal_forecast, timeseries_forecast};

use crate::error::Result;
use crate::types::ForecastSpec;
use finstack_core::dates::PeriodId;

/// Apply a forecast method to generate values for forecast periods.
///
/// Use this for the standalone deterministic forecast path. Statistical
/// methods use the seed recorded in `spec.params`. Monte Carlo evaluation
/// layers an additional per-path seed internally.
///
/// # Arguments
///
/// * `spec` - Forecast specification with method and parameters
/// * `base_value` - Starting value (typically last actual value)
/// * `forecast_periods` - List of periods to forecast
///
/// # Returns
///
/// Map of period_id → forecasted value
pub fn apply_forecast(
    spec: &ForecastSpec,
    base_value: f64,
    forecast_periods: &[PeriodId],
) -> Result<indexmap::IndexMap<PeriodId, f64>> {
    apply_forecast_internal(spec, base_value, forecast_periods, None)
}

/// Apply a forecast for a specific node in single-run (non-Monte-Carlo) mode.
///
/// Behaves like [`apply_forecast`] but mixes a stable hash of `node_id` into
/// the seed of statistical methods (Normal, LogNormal), matching Monte Carlo
/// mode. Without this mix, two stochastic nodes configured with the same
/// `seed` would receive identical shock paths within a single evaluation run.
/// This changes single-run stochastic sequences relative to earlier releases
/// that seeded purely from the configured `seed`. Deterministic methods are
/// unaffected.
pub(crate) fn apply_forecast_for_node(
    spec: &ForecastSpec,
    base_value: f64,
    forecast_periods: &[PeriodId],
    node_id: &str,
) -> Result<indexmap::IndexMap<PeriodId, f64>> {
    use crate::types::ForecastMethod;
    use statistical::{parse_seed_json, stable_hash_u64};

    match spec.method {
        ForecastMethod::Normal | ForecastMethod::LogNormal => {
            let params = mix_node_seed(&spec.params, node_id, parse_seed_json, stable_hash_u64);
            let spec = ForecastSpec {
                method: spec.method,
                params,
            };
            apply_forecast_internal(&spec, base_value, forecast_periods, None)
        }
        _ => apply_forecast_internal(spec, base_value, forecast_periods, None),
    }
}

/// Apply a forecast method with an additional seed offset for statistical
/// methods.
///
/// Used by Monte Carlo evaluation to derive independent, but still
/// deterministic, per-path seeds from the base seed configured in the
/// [`ForecastSpec`]. The `node_id` argument is mixed into the effective RNG
/// seed so different stochastic nodes on the same path do not reuse identical
/// draws. Deterministic methods ignore the seed and behave identically to
/// [`apply_forecast`].
pub(crate) fn apply_forecast_seeded(
    spec: &ForecastSpec,
    base_value: f64,
    forecast_periods: &[PeriodId],
    seed_offset: u64,
    node_id: &str,
) -> Result<indexmap::IndexMap<PeriodId, f64>> {
    apply_forecast_internal(
        spec,
        base_value,
        forecast_periods,
        Some((seed_offset, node_id)),
    )
}

fn apply_forecast_internal(
    spec: &ForecastSpec,
    base_value: f64,
    forecast_periods: &[PeriodId],
    seed_ctx: Option<(u64, &str)>,
) -> Result<indexmap::IndexMap<PeriodId, f64>> {
    use crate::types::ForecastMethod;
    use statistical::{
        lognormal_forecast_with_stream, normal_forecast_with_stream, parse_seed_json,
        stable_hash_u64,
    };

    match (spec.method, seed_ctx) {
        (ForecastMethod::Normal, Some((seed_offset, node_id))) => {
            let params = mix_node_seed(&spec.params, node_id, parse_seed_json, stable_hash_u64);
            normal_forecast_with_stream(base_value, forecast_periods, &params, Some(seed_offset))
        }
        (ForecastMethod::LogNormal, Some((seed_offset, node_id))) => {
            let params = mix_node_seed(&spec.params, node_id, parse_seed_json, stable_hash_u64);
            lognormal_forecast_with_stream(base_value, forecast_periods, &params, Some(seed_offset))
        }
        (ForecastMethod::ForwardFill, _) => forward_fill(base_value, forecast_periods),
        (ForecastMethod::GrowthPct, _) => growth_pct(base_value, forecast_periods, &spec.params),
        (ForecastMethod::CurvePct, _) => curve_pct(base_value, forecast_periods, &spec.params),
        (ForecastMethod::Override, _) => apply_override(base_value, forecast_periods, &spec.params),
        (ForecastMethod::Normal, None) => {
            normal_forecast(base_value, forecast_periods, &spec.params)
        }
        (ForecastMethod::LogNormal, None) => {
            lognormal_forecast(base_value, forecast_periods, &spec.params)
        }
        (ForecastMethod::TimeSeries, _) => {
            timeseries_forecast(base_value, forecast_periods, &spec.params)
        }
        (ForecastMethod::Seasonal, _) => {
            seasonal_forecast(base_value, forecast_periods, &spec.params)
        }
    }
}

fn mix_node_seed(
    params: &indexmap::IndexMap<String, serde_json::Value>,
    node_id: &str,
    parse_seed: fn(&serde_json::Value) -> Option<u64>,
    hash_node: fn(&str) -> u64,
) -> indexmap::IndexMap<String, serde_json::Value> {
    let mut params = params.clone();
    if let Some(seed_val) = params.get_mut("seed") {
        if let Some(seed) = parse_seed(seed_val) {
            let effective_seed = seed ^ hash_node(node_id);
            *seed_val = serde_json::json!(effective_seed);
        }
    }
    params
}

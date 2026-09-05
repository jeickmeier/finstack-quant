//! Forecast methods for time-series projection.
//!
//! This module provides various forecast methods for projecting values into
//! future periods, including:
//! - **Deterministic**: ForwardFill, GrowthPct, CurvePct, Override,
//!   FadeToTarget (linear / geometric / exponential glide to a target level)
//! - **Statistical**: Normal, LogNormal, MeanReverting (AR(1)), Bootstrap
//!   (historical resampling) — all with deterministic seeding
//! - **TimeSeries**: trend detection (linear, Holt / damped Holt via `phi`,
//!   moving average) and Seasonal patterns
//!
//! All forecast methods operate on a base value (typically the last actual value)
//! and project forward for a specified number of periods.
//!
//! # Bounds
//!
//! Every method accepts optional `min` / `max` parameters that clamp the
//! generated values to a band: ratios that must stay
//! in a range, levels that cannot go negative, or trend extrapolations that
//! must not explode. The clamp applies to output values only — stochastic
//! recurrences evolve unclamped, and Monte Carlo evaluation records
//! correlation Z-scores before clamping.
//!
//! # Random Number Generation
//!
//! Statistical forecast methods (Normal, LogNormal, MeanReverting, Bootstrap)
//! require a `seed` parameter
//! for deterministic random number generation. This ensures reproducibility:
//! - Same seed → identical forecast values across runs
//! - Different seeds → different (but still deterministic) values
//!
//! Both single-run evaluation and Monte Carlo mode mix a stable hash of the
//! node identifier into the configured seed so independent stochastic nodes do
//! not share identical shock draws (Monte Carlo additionally layers a per-path
//! seed offset).
//! Optional `correlation_with` / `correlation` parameters pair Normal,
//! LogNormal, and MeanReverting nodes for correlated
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
//! the `finstack-quant-statements-analytics` crate.

mod deterministic;
mod fade;
mod override_method;
pub(crate) mod statistical;
mod timeseries;

use deterministic::{curve_pct, forward_fill, growth_pct};
use fade::fade_to_target;
use override_method::apply_override;
use timeseries::{seasonal_forecast, timeseries_forecast};

use crate::error::Result;
use crate::types::ForecastSpec;
use finstack_quant_core::dates::PeriodId;
/// Apply a forecast for a specific node in single-run (non-Monte-Carlo) mode.
///
/// Runs the method selected by `spec` and applies its `min`/`max` bounds,
/// mixing a stable hash of `node_id` into
/// the seed of statistical methods (Normal, LogNormal, MeanReverting,
/// Bootstrap), matching Monte Carlo mode. Without this mix, two stochastic
/// nodes configured with the same `seed` would receive identical shock paths
/// within a single evaluation run.
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

    let mut results = match spec.method {
        // Stochastic (seeded) methods get the node-seed mix. Correlation
        // (`correlation_with`) is a Monte Carlo-only feature; the single-run
        // path produces independent draws. Warn loudly so a configured
        // correlation is not silently ignored (which would make a one-run
        // sanity check disagree with the MC output). Bootstrap does not
        // support correlation, so the parser returns `None` for it.
        ForecastMethod::Normal
        | ForecastMethod::LogNormal
        | ForecastMethod::MeanReverting
        | ForecastMethod::Bootstrap => {
            if let Some((peer, _rho)) = statistical::parse_correlation_params(&spec.params)? {
                tracing::warn!(
                    node = node_id,
                    peer = peer.as_str(),
                    "`correlation_with` is set but single-run evaluation ignores correlation \
                     (only Monte Carlo honors it); this node's draws are independent here"
                );
            }
            let params = mix_node_seed(&spec.params, node_id, parse_seed_json, stable_hash_u64);
            let spec = ForecastSpec {
                method: spec.method,
                params,
            };
            apply_forecast_internal(&spec, base_value, forecast_periods, None)?
        }
        _ => apply_forecast_internal(spec, base_value, forecast_periods, None)?,
    };
    apply_bounds(&spec.params, &mut results)?;
    Ok(results)
}

/// Apply a forecast method with an additional seed offset for statistical
/// methods.
///
/// Used by Monte Carlo evaluation to derive independent, but still
/// deterministic, per-path seeds from the base seed configured in the
/// [`ForecastSpec`]. The `node_id` argument is mixed into the effective RNG
/// seed so different stochastic nodes on the same path do not reuse identical
/// draws. Deterministic methods ignore the seed and behave identically to
/// [`apply_forecast_for_node`].
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
        bootstrap_forecast_with_stream, lognormal_forecast_with_stream,
        mean_reverting_forecast_with_stream, normal_forecast_with_stream, parse_seed_json,
        stable_hash_u64,
    };

    // Single dispatch point for every method, so an unknown key cannot slip
    // through on any path.
    validate_params(spec.method, &spec.params)?;

    match (spec.method, seed_ctx) {
        (ForecastMethod::Normal, Some((seed_offset, node_id))) => {
            let params = mix_node_seed(&spec.params, node_id, parse_seed_json, stable_hash_u64);
            normal_forecast_with_stream(base_value, forecast_periods, &params, Some(seed_offset))
        }
        (ForecastMethod::LogNormal, Some((seed_offset, node_id))) => {
            let params = mix_node_seed(&spec.params, node_id, parse_seed_json, stable_hash_u64);
            lognormal_forecast_with_stream(base_value, forecast_periods, &params, Some(seed_offset))
        }
        (ForecastMethod::MeanReverting, Some((seed_offset, node_id))) => {
            let params = mix_node_seed(&spec.params, node_id, parse_seed_json, stable_hash_u64);
            mean_reverting_forecast_with_stream(
                base_value,
                forecast_periods,
                &params,
                Some(seed_offset),
            )
        }
        (ForecastMethod::Bootstrap, Some((seed_offset, node_id))) => {
            let params = mix_node_seed(&spec.params, node_id, parse_seed_json, stable_hash_u64);
            bootstrap_forecast_with_stream(base_value, forecast_periods, &params, Some(seed_offset))
        }
        (ForecastMethod::ForwardFill, _) => forward_fill(base_value, forecast_periods),
        (ForecastMethod::GrowthPct, _) => growth_pct(base_value, forecast_periods, &spec.params),
        (ForecastMethod::CurvePct, _) => curve_pct(base_value, forecast_periods, &spec.params),
        (ForecastMethod::Override, _) => apply_override(base_value, forecast_periods, &spec.params),
        (ForecastMethod::Normal, None) => {
            normal_forecast_with_stream(base_value, forecast_periods, &spec.params, None)
        }
        (ForecastMethod::LogNormal, None) => {
            lognormal_forecast_with_stream(base_value, forecast_periods, &spec.params, None)
        }
        (ForecastMethod::TimeSeries, _) => {
            timeseries_forecast(base_value, forecast_periods, &spec.params)
        }
        (ForecastMethod::Seasonal, _) => {
            seasonal_forecast(base_value, forecast_periods, &spec.params)
        }
        (ForecastMethod::FadeToTarget, _) => {
            fade_to_target(base_value, forecast_periods, &spec.params)
        }
        (ForecastMethod::MeanReverting, None) => {
            mean_reverting_forecast_with_stream(base_value, forecast_periods, &spec.params, None)
        }
        (ForecastMethod::Bootstrap, None) => {
            bootstrap_forecast_with_stream(base_value, forecast_periods, &spec.params, None)
        }
    }
}

/// Parameter keys each forecast method understands.
///
/// The single vocabulary for every method, so a key a method silently ignores
/// cannot exist. Previously only TimeSeries and Seasonal rejected unknown keys,
/// which meant a typo elsewhere — `sigma` beside a stale `std_dev`, say — ran
/// clean at the wrong volatility with no diagnostic.
///
/// Every method additionally accepts the cross-cutting `min` / `max` bounds
/// applied by [`apply_bounds`] after generation.
pub(crate) fn allowed_params(method: crate::types::ForecastMethod) -> &'static [&'static str] {
    use crate::types::ForecastMethod;
    match method {
        ForecastMethod::ForwardFill => &["min", "max"],
        ForecastMethod::GrowthPct => &["rate", "min", "max"],
        ForecastMethod::CurvePct => &["curve", "min", "max"],
        ForecastMethod::Override => &["overrides", "min", "max"],
        // `correlation_with` / `correlation` are Monte Carlo-only but are
        // legal to configure on any Normal / LogNormal / MeanReverting node
        // (the single-run path warns that it ignores them).
        ForecastMethod::Normal | ForecastMethod::LogNormal => &[
            "mean",
            "std_dev",
            "seed",
            "correlation_with",
            "correlation",
            "min",
            "max",
        ],
        ForecastMethod::MeanReverting => &[
            "long_run_mean",
            "reversion_speed",
            "std_dev",
            "seed",
            "correlation_with",
            "correlation",
            "min",
            "max",
        ],
        ForecastMethod::Bootstrap => &["historical", "mode", "seed", "min", "max"],
        ForecastMethod::TimeSeries => &[
            "historical",
            "method",
            "alpha",
            "beta",
            "phi",
            "window",
            "min",
            "max",
        ],
        ForecastMethod::Seasonal => &[
            "historical",
            "season_length",
            "mode",
            "growth",
            "min",
            "max",
        ],
        ForecastMethod::FadeToTarget => &["target", "shape", "half_life", "min", "max"],
    }
}

/// Expected JSON shape of each forecast parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParamShape {
    Number,
    UnsignedInt,
    NumberArray,
    Text,
    Object,
}

/// JSON shape each parameter key must carry, independent of method.
///
/// Every key in [`allowed_params`] appears here exactly once, so type
/// validation cannot drift from key validation.
fn param_shape(key: &str) -> Option<ParamShape> {
    Some(match key {
        "rate" | "mean" | "std_dev" | "correlation" | "long_run_mean" | "reversion_speed"
        | "alpha" | "beta" | "phi" | "growth" | "target" | "half_life" | "min" | "max" => {
            ParamShape::Number
        }
        "seed" | "season_length" | "window" => ParamShape::UnsignedInt,
        "curve" | "historical" => ParamShape::NumberArray,
        "correlation_with" | "mode" | "method" | "shape" => ParamShape::Text,
        "overrides" => ParamShape::Object,
        _ => return None,
    })
}

/// Check that every parameter value has the JSON shape its key requires.
///
/// Keys not in the shared vocabulary are ignored here; [`validate_params`]
/// rejects them per method. Value *ranges* are checked by the method
/// implementations when the forecast runs.
///
/// # Arguments
///
/// * `params` - Forecast parameter map as stored on a `ForecastSpec`
///
/// # Errors
///
/// Returns a forecast error naming the first key whose value has the wrong
/// JSON type (for example a string where a number is expected).
pub(crate) fn validate_param_types(
    params: &indexmap::IndexMap<String, serde_json::Value>,
) -> Result<()> {
    for (key, value) in params {
        let Some(shape) = param_shape(key) else {
            continue;
        };
        let ok = match shape {
            ParamShape::Number => value.as_f64().is_some(),
            ParamShape::UnsignedInt => value.as_u64().is_some(),
            ParamShape::NumberArray => value
                .as_array()
                .is_some_and(|items| items.iter().all(|v| v.as_f64().is_some())),
            ParamShape::Text => value.is_string(),
            ParamShape::Object => value.is_object(),
        };
        if !ok {
            let expected = match shape {
                ParamShape::Number => "a number",
                ParamShape::UnsignedInt => "a non-negative integer",
                ParamShape::NumberArray => "an array of numbers",
                ParamShape::Text => "a string",
                ParamShape::Object => "an object keyed by period id",
            };
            return Err(crate::error::Error::forecast(format!(
                "Forecast parameter '{key}' must be {expected}, got {value}"
            )));
        }
    }
    Ok(())
}

/// Clamp generated forecast values to optional `min` / `max` bounds.
///
/// Bounds are a cross-cutting feature accepted by every forecast method:
/// ratios that must stay in a band (payout, capital ratios), levels that
/// cannot go negative (NIM, loss rates), or trend extrapolations that must
/// not explode over long horizons. The clamp is applied to **output values
/// only** — stochastic recurrences evolve unclamped, so a persistent walk can
/// pin at a bound while its underlying path drifts beyond it.
///
/// In Monte Carlo mode the evaluator applies this **after** Z-score
/// recording, so correlation shocks are always inverted from the unclamped
/// series.
///
/// # Arguments
///
/// * `params` - Forecast parameter map; reads optional `min` and `max`
///   entries, each a finite number in the node's own units
/// * `results` - Generated per-period values, clamped in place
///
/// # Errors
///
/// Returns an error when `min` or `max` is present but not a finite number,
/// or when both are present with `min > max`.
pub(crate) fn apply_bounds(
    params: &indexmap::IndexMap<String, serde_json::Value>,
    results: &mut indexmap::IndexMap<PeriodId, f64>,
) -> Result<()> {
    fn parse_bound(
        params: &indexmap::IndexMap<String, serde_json::Value>,
        key: &str,
    ) -> Result<Option<f64>> {
        match params.get(key) {
            None => Ok(None),
            Some(value) => {
                let bound = value.as_f64().filter(|b| b.is_finite()).ok_or_else(|| {
                    crate::error::Error::forecast(format!(
                        "Forecast bound '{key}' must be a finite number, got {value}"
                    ))
                })?;
                Ok(Some(bound))
            }
        }
    }

    let min = parse_bound(params, "min")?;
    let max = parse_bound(params, "max")?;

    if let (Some(lo), Some(hi)) = (min, max) {
        if lo > hi {
            return Err(crate::error::Error::forecast(format!(
                "Forecast bounds require min <= max, got min = {lo} and max = {hi}"
            )));
        }
    }
    if min.is_none() && max.is_none() {
        return Ok(());
    }

    for value in results.values_mut() {
        if let Some(lo) = min {
            *value = value.max(lo);
        }
        if let Some(hi) = max {
            *value = value.min(hi);
        }
    }
    Ok(())
}

/// Reject parameter keys the method does not understand.
///
/// # Errors
///
/// Returns an error naming the offending key and listing the allowed set.
pub(crate) fn validate_params(
    method: crate::types::ForecastMethod,
    params: &indexmap::IndexMap<String, serde_json::Value>,
) -> Result<()> {
    let allowed = allowed_params(method);
    for key in params.keys() {
        if !allowed.contains(&key.as_str()) {
            let allowed_list = if allowed.is_empty() {
                "(none)".to_string()
            } else {
                allowed.join(", ")
            };
            return Err(crate::error::Error::forecast(format!(
                "Unknown parameter '{key}' for {method:?} forecast. \
                 Allowed parameters: {allowed_list}"
            )));
        }
    }
    Ok(())
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
            // SplitMix64-style combine: a plain XOR can cancel bits when the
            // user seed and node hash overlap (worst case yielding seed 0);
            // multiply-add by the golden-ratio constant decorrelates them.
            let effective_seed = seed
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(hash_node(node_id));
            *seed_val = serde_json::json!(effective_seed);
        }
    }
    params
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::PeriodId;

    fn quarters(n: u8) -> Vec<PeriodId> {
        (0..n)
            .map(|i| {
                PeriodId::quarter(2025 + i32::from(i / 4), i % 4 + 1).expect("valid period fixture")
            })
            .collect()
    }

    #[test]
    fn bounds_clamp_deterministic_forecast_output() {
        let periods = quarters(4);
        let mut spec = ForecastSpec::growth(0.10);
        spec.params.insert("max".into(), serde_json::json!(120.0));
        let results =
            apply_forecast_for_node(&spec, 100.0, &periods, "node").expect("bounded growth");
        // Unbounded: 110, 121, 133.1, 146.41 — everything above 120 pins.
        assert!((results[&periods[0]] - 110.0).abs() < 1e-9);
        assert!((results[&periods[1]] - 120.0).abs() < 1e-9);
        assert!((results[&periods[2]] - 120.0).abs() < 1e-9);
        assert!((results[&periods[3]] - 120.0).abs() < 1e-9);
    }

    #[test]
    fn bounds_clamp_stochastic_forecast_output() {
        let periods = quarters(8);
        let mut spec = ForecastSpec::normal(0.0, 50.0, 42);
        spec.params.insert("min".into(), serde_json::json!(0.0));
        spec.params.insert("max".into(), serde_json::json!(150.0));
        let results =
            apply_forecast_for_node(&spec, 100.0, &periods, "node").expect("bounded normal");
        for (pid, value) in &results {
            assert!(
                (0.0..=150.0).contains(value),
                "value {value} at {pid:?} escaped the bounds"
            );
        }
    }

    #[test]
    fn bounds_reject_inverted_range() {
        let periods = quarters(1);
        let mut spec = ForecastSpec::forward_fill();
        spec.params.insert("min".into(), serde_json::json!(10.0));
        spec.params.insert("max".into(), serde_json::json!(-10.0));
        let err = apply_forecast_for_node(&spec, 100.0, &periods, "node").expect_err("min > max");
        assert!(err.to_string().contains("min <= max"), "{err}");
    }

    #[test]
    fn bounds_reject_non_finite_values() {
        let periods = quarters(1);
        for bad in [serde_json::json!("high"), serde_json::Value::Null] {
            let mut spec = ForecastSpec::forward_fill();
            spec.params.insert("max".into(), bad);
            let err =
                apply_forecast_for_node(&spec, 100.0, &periods, "node").expect_err("bad bound");
            assert!(err.to_string().contains("finite"), "{err}");
        }
    }

    #[test]
    fn bounds_are_accepted_by_every_method() {
        use crate::types::ForecastMethod;
        for method in [
            ForecastMethod::ForwardFill,
            ForecastMethod::GrowthPct,
            ForecastMethod::CurvePct,
            ForecastMethod::Normal,
            ForecastMethod::LogNormal,
            ForecastMethod::Override,
            ForecastMethod::TimeSeries,
            ForecastMethod::Seasonal,
            ForecastMethod::FadeToTarget,
            ForecastMethod::MeanReverting,
            ForecastMethod::Bootstrap,
        ] {
            let allowed = allowed_params(method);
            assert!(
                allowed.contains(&"min") && allowed.contains(&"max"),
                "{method:?} must accept the cross-cutting min/max bounds"
            );
        }
    }

    /// Monte Carlo path: `apply_forecast_seeded` must return **unclamped**
    /// values. Z-score recording inverts the generating recurrence from the
    /// series, so clamping before recording would corrupt every correlated
    /// peer's shocks. The evaluator applies bounds after recording.
    #[test]
    fn seeded_mc_forecast_is_not_clamped_before_z_recording() {
        let periods = quarters(8);
        let mut spec = ForecastSpec::normal(0.0, 50.0, 42);
        spec.params.insert("min".into(), serde_json::json!(99.0));
        spec.params.insert("max".into(), serde_json::json!(101.0));

        let seeded =
            apply_forecast_seeded(&spec, 100.0, &periods, 3, "node").expect("seeded forecast");
        // With sigma = 50 and tight bounds, an unclamped walk must escape
        // [99, 101] somewhere in 8 periods.
        assert!(
            seeded.values().any(|v| !(99.0..=101.0).contains(v)),
            "seeded MC series must be raw (unclamped): {seeded:?}"
        );

        // And the single-run node path with identical params must be clamped.
        let clamped =
            apply_forecast_for_node(&spec, 100.0, &periods, "node").expect("single-run forecast");
        for value in clamped.values() {
            assert!((99.0..=101.0).contains(value));
        }
    }

    #[test]
    fn every_allowed_param_has_a_declared_shape() {
        use crate::types::ForecastMethod;
        for method in [
            ForecastMethod::ForwardFill,
            ForecastMethod::GrowthPct,
            ForecastMethod::CurvePct,
            ForecastMethod::Normal,
            ForecastMethod::LogNormal,
            ForecastMethod::Override,
            ForecastMethod::TimeSeries,
            ForecastMethod::Seasonal,
            ForecastMethod::FadeToTarget,
            ForecastMethod::MeanReverting,
            ForecastMethod::Bootstrap,
        ] {
            for key in allowed_params(method) {
                assert!(
                    param_shape(key).is_some(),
                    "{method:?} parameter '{key}' has no declared JSON shape"
                );
            }
        }
    }

    #[test]
    fn spec_validate_rejects_wrong_value_types() {
        let mut spec = ForecastSpec::growth(0.05);
        spec.validate().expect("well-typed growth spec");
        spec.params.insert("rate".into(), serde_json::json!("x"));
        let err = spec.validate().expect_err("string rate must be rejected");
        assert!(err.to_string().contains("'rate' must be a number"), "{err}");

        let mut spec = ForecastSpec::forward_fill();
        spec.params.insert("sigma".into(), serde_json::json!(1.0));
        let err = spec.validate().expect_err("unknown key must be rejected");
        assert!(
            err.to_string().contains("Unknown parameter 'sigma'"),
            "{err}"
        );
    }

    #[test]
    fn convenience_ctors_produce_valid_specs() {
        let mut overrides = indexmap::IndexMap::new();
        overrides.insert(
            PeriodId::quarter(2025, 3).expect("valid period fixture"),
            120.0,
        );
        ForecastSpec::overrides(overrides)
            .validate()
            .expect("override spec");
        ForecastSpec::seasonal(vec![1.0; 8], 4, crate::types::SeasonalMode::Additive)
            .validate()
            .expect("seasonal spec");
        ForecastSpec::time_series(vec![1.0, 2.0, 3.0])
            .validate()
            .expect("time-series spec");
    }

    #[test]
    fn unknown_keys_still_rejected_for_new_methods() {
        let periods = quarters(1);
        let mut spec = ForecastSpec::fade_to_target(50.0);
        spec.params
            .insert("shpae".into(), serde_json::json!("linear"));
        let err = apply_forecast_for_node(&spec, 100.0, &periods, "node").expect_err("typo key");
        assert!(err.to_string().contains("shpae"), "{err}");
    }
}

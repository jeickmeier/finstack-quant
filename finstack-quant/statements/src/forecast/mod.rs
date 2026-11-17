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
//! shocks (see `forecast::statistical::correlation`). The peer node must
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

/// Per-period forecast levels or standard-normal shocks.
type ForecastSeries = indexmap::IndexMap<PeriodId, f64>;
/// Apply a forecast for a specific node in single-run (non-Monte-Carlo) mode.
///
/// Runs the method of `parameters` and applies its `min`/`max` bounds,
/// mixing a stable hash of `node_id` into the seed of statistical methods
/// (Normal, LogNormal, MeanReverting, Bootstrap), matching Monte Carlo mode.
/// Without this mix, two stochastic nodes configured with the same `seed`
/// would receive identical shock paths within a single evaluation run.
/// Deterministic methods are unaffected.
pub(crate) fn apply_forecast_for_node(
    parameters: &ForecastSpec,
    base_value: f64,
    forecast_periods: &[PeriodId],
    node_id: &str,
) -> Result<ForecastSeries> {
    // Correlation (`correlation_with`) is a Monte Carlo-only feature; the
    // single-run path produces independent draws. Warn loudly so a configured
    // correlation is not silently ignored (which would make a one-run sanity
    // check disagree with the MC output).
    if let Some((peer, _rho)) = statistical::correlation(parameters)? {
        tracing::warn!(
            node = node_id,
            peer,
            "`correlation_with` is set but single-run evaluation ignores correlation \
             (only Monte Carlo honors it); this node's draws are independent here"
        );
    }
    let (mut results, _) = apply_forecast(parameters, base_value, forecast_periods, node_id, None)?;
    apply_bounds(parameters, &mut results)?;
    Ok(results)
}

/// Run a forecast method without bounds.
///
/// Stochastic methods (normal, log-normal, mean-reverting, bootstrap) mix
/// `node_id` into their configured seed so different nodes on the same run do
/// not reuse identical draws. `stream` selects an independent RNG stream
/// for Monte Carlo path `stream`; `None` is the single-run stream.
/// Deterministic methods ignore both. Monte Carlo callers apply
/// [`apply_bounds`] themselves, after recording correlation shocks.
///
/// Returns the generated levels and the sampled Gaussian shocks, or `None`
/// for methods that do not draw Gaussian shocks. Zero-volatility forecasts
/// retain zero shocks so they cannot serve as stochastic correlation peers.
pub(crate) fn apply_forecast(
    parameters: &ForecastSpec,
    base_value: f64,
    forecast_periods: &[PeriodId],
    node_id: &str,
    stream: Option<u64>,
) -> Result<(ForecastSeries, Option<ForecastSeries>)> {
    use statistical::{
        bootstrap_forecast_with_stream, lognormal_forecast_with_stream,
        mean_reverting_forecast_with_stream, normal_forecast_with_stream,
    };
    let mut seeded = parameters.clone();
    if let Some(seed) = seeded.seed_mut() {
        *seed = mix_node_seed(*seed, node_id);
    }
    match &seeded {
        ForecastSpec::Normal(p) => {
            normal_forecast_with_stream(base_value, forecast_periods, p, stream)
                .map(|(values, shocks)| (values, Some(shocks)))
        }
        ForecastSpec::LogNormal(p) => {
            lognormal_forecast_with_stream(base_value, forecast_periods, p, stream)
                .map(|(values, shocks)| (values, Some(shocks)))
        }
        ForecastSpec::MeanReverting(p) => {
            mean_reverting_forecast_with_stream(base_value, forecast_periods, p, stream)
                .map(|(values, shocks)| (values, Some(shocks)))
        }
        ForecastSpec::Bootstrap(p) => {
            bootstrap_forecast_with_stream(base_value, forecast_periods, p, stream)
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
            apply_override(base_value, forecast_periods, &p.overrides).map(|values| (values, None))
        }
        ForecastSpec::TimeSeries(p) => {
            timeseries_forecast(forecast_periods, p).map(|values| (values, None))
        }
        ForecastSpec::Seasonal(p) => {
            seasonal_forecast(base_value, forecast_periods, p).map(|values| (values, None))
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
/// * `results` - Generated per-period values, clamped in place
///
/// # Errors
///
/// Returns an error when `min` or `max` is present but not finite, or when
/// both are present with `min > max`.
pub(crate) fn apply_bounds(parameters: &ForecastSpec, results: &mut ForecastSeries) -> Result<()> {
    let (min, max) = parameters.bounds();
    for (key, bound) in [("min", min), ("max", max)] {
        if let Some(bound) = bound
            && !bound.is_finite()
        {
            return Err(crate::error::Error::forecast(format!(
                "Forecast bound '{key}' must be a finite number, got {bound}"
            )));
        }
    }

    if let (Some(lo), Some(hi)) = (min, max)
        && lo > hi
    {
        return Err(crate::error::Error::forecast(format!(
            "Forecast bounds require min <= max, got min = {lo} and max = {hi}"
        )));
    }
    if min.is_none() && max.is_none() {
        return Ok(());
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
    Ok(())
}

fn mix_node_seed(seed: u64, node_id: &str) -> u64 {
    // SplitMix64-style combine: a plain XOR can cancel bits when the
    // user seed and node hash overlap (worst case yielding seed 0);
    // multiply-add by the golden-ratio constant decorrelates them.
    seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(statistical::stable_hash_u64(node_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ForecastSpec;
    use crate::types::{ForecastBootstrapMode, ForecastFadeShape, ForecastTrendMethod};
    use finstack_quant_core::dates::PeriodId;
    use serde_json::json;

    fn quarters(n: u8) -> Vec<PeriodId> {
        (0..n)
            .map(|i| {
                PeriodId::quarter(2025 + i32::from(i / 4), i % 4 + 1).expect("valid period fixture")
            })
            .collect()
    }

    fn run(spec: &ForecastSpec, base_value: f64, periods: &[PeriodId]) -> Result<ForecastSeries> {
        apply_forecast_for_node(&spec.clone(), base_value, periods, "node")
    }

    #[test]
    fn typed_parameters_round_trip_the_stored_wire_form() {
        let documents = [
            json!({"method": "forward_fill", "params": {"min": 0.0, "max": 1000.0}}),
            json!({"method": "growth_pct", "params": {"rate": 0.03}}),
            json!({"method": "curve_pct", "params": {"curve": [0.01, 0.02]}}),
            json!({"method": "override", "params": {"overrides": {"2025Q1": 1.0}}}),
            json!({"method": "normal", "params": {"mean": 1.0, "std_dev": 0.1, "seed": "7", "correlation_with": "peer", "correlation": 0.5}}),
            json!({"method": "log_normal", "params": {"mean": 0.0, "std_dev": 0.1, "seed": "7"}}),
            json!({"method": "mean_reverting", "params": {"long_run_mean": 1.0, "reversion_speed": 0.5, "std_dev": 0.1, "seed": "7"}}),
            json!({"method": "bootstrap", "params": {"historical": [1.0, 2.0], "mode": "diff", "seed": "7"}}),
            json!({"method": "time_series", "params": {"historical": [1.0, 2.0], "method": "exponential", "alpha": 0.5, "beta": 0.5, "phi": 0.9}}),
            json!({"method": "seasonal", "params": {"historical": [1.0, 2.0], "season_length": 2, "mode": "additive", "growth": 0.01}}),
            json!({"method": "fade_to_target", "params": {"target": 1.0, "shape": "exponential", "half_life": 2.0}}),
        ];
        for document in documents {
            let spec: ForecastSpec = serde_json::from_value(document.clone()).expect("spec");
            let typed = spec.clone();
            assert_eq!(serde_json::to_value(&typed).expect("serialize"), document);
            let direct: ForecastSpec = serde_json::from_value(document).expect("tagged parameters");
            assert_eq!(direct, typed);
        }
    }

    #[test]
    fn typed_parameters_reject_a_key_outside_the_method() {
        let document = json!({"method": "growth_pct", "params": {"rate": 0.03, "sigma": 1.0}});
        assert!(serde_json::from_value::<ForecastSpec>(document).is_err());
    }

    #[test]
    fn bounds_clamp_deterministic_forecast_output() {
        let periods = quarters(4);
        let mut spec = ForecastSpec::growth(0.10);
        spec = spec.clone().with_param("max", json!(120.0)).unwrap();
        let results = run(&spec, 100.0, &periods).expect("bounded growth");
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
        spec = spec.clone().with_param("min", json!(0.0)).unwrap();
        spec = spec.clone().with_param("max", json!(150.0)).unwrap();
        let results = run(&spec, 100.0, &periods).expect("bounded normal");
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
        spec = spec.clone().with_param("min", json!(10.0)).unwrap();
        spec = spec.clone().with_param("max", json!(-10.0)).unwrap();
        let err = run(&spec, 100.0, &periods).expect_err("min > max");
        assert!(err.to_string().contains("min <= max"), "{err}");
    }

    #[test]
    fn bounds_reject_non_numeric_values() {
        for bad in [json!("high"), serde_json::Value::Null] {
            let spec = ForecastSpec::forward_fill();
            let err = spec.clone().with_param("max", bad).expect_err("bad bound");
            assert!(err.to_string().contains("'max'"), "{err}");
        }
    }

    #[test]
    fn bounds_are_accepted_by_every_method() {
        let specs = [
            ForecastSpec::forward_fill(),
            ForecastSpec::growth(0.01),
            ForecastSpec::curve(vec![0.01]),
            ForecastSpec::normal(0.0, 1.0, 1),
            ForecastSpec::log_normal(0.0, 1.0, 1),
            ForecastSpec::overrides(indexmap::IndexMap::new()),
            ForecastSpec::time_series(vec![1.0, 2.0], ForecastTrendMethod::Linear),
            ForecastSpec::seasonal(vec![1.0; 8], 4, crate::types::SeasonalMode::Additive),
            ForecastSpec::fade_to_target(1.0, ForecastFadeShape::Linear),
            ForecastSpec::mean_reverting(1.0, 0.5, 0.1, 1),
            ForecastSpec::bootstrap(vec![1.0, 2.0], ForecastBootstrapMode::Growth, 1),
        ];
        for mut spec in specs {
            spec = spec.clone().with_param("min", json!(-5.0)).unwrap();
            spec = spec.clone().with_param("max", json!(5.0)).unwrap();
            let parameters = spec.clone();
            assert_eq!(
                parameters.bounds(),
                (Some(-5.0), Some(5.0)),
                "{:?}",
                spec.method()
            );
        }
    }

    /// Monte Carlo path: `apply_forecast` must return **unclamped**
    /// values. The recurrence evolves without bounds; the evaluator clamps
    /// only the reported levels after retaining the sampled shocks.
    #[test]
    fn seeded_mc_forecast_is_not_clamped_before_z_recording() {
        let periods = quarters(8);
        let mut spec = ForecastSpec::normal(0.0, 50.0, 42);
        spec = spec.clone().with_param("min", json!(99.0)).unwrap();
        spec = spec.clone().with_param("max", json!(101.0)).unwrap();
        let parameters = spec.clone();

        let (seeded, _) =
            apply_forecast(&parameters, 100.0, &periods, "node", Some(3)).expect("seeded forecast");
        // With sigma = 50 and tight bounds, an unclamped walk must escape
        // [99, 101] somewhere in 8 periods.
        assert!(
            seeded.values().any(|v| !(99.0..=101.0).contains(v)),
            "seeded MC series must be raw (unclamped): {seeded:?}"
        );

        // And the single-run node path with identical params must be clamped.
        let clamped = run(&spec, 100.0, &periods).expect("single-run forecast");
        for value in clamped.values() {
            assert!((99.0..=101.0).contains(value));
        }
    }

    /// Two nodes configured with the same seed draw different paths.
    #[test]
    fn node_id_is_mixed_into_the_seed() {
        let periods = quarters(4);
        let parameters = ForecastSpec::normal(0.0, 1.0, 42);
        let a = apply_forecast_for_node(&parameters, 0.0, &periods, "a").expect("a");
        let b = apply_forecast_for_node(&parameters, 0.0, &periods, "b").expect("b");
        assert_ne!(a, b);
    }

    #[test]
    fn a_typed_spec_cannot_hold_a_wrong_type_or_an_unknown_key() {
        let spec = ForecastSpec::growth(0.05);
        spec.validate().expect("well-typed growth spec");
        let err = spec
            .with_param("rate", json!("x"))
            .expect_err("string rate must be rejected");
        assert!(err.to_string().contains("'rate'"), "{err}");

        let err = ForecastSpec::forward_fill()
            .with_param("sigma", json!(1.0))
            .expect_err("unknown key must be rejected");
        assert!(err.to_string().contains("unknown field `sigma`"), "{err}");
    }

    /// STATEMENTS-CONTRACTS-003 #1: a duplicate, unknown, `null` or
    /// method-inapplicable field is rejected when the document is read, and
    /// the method choices have no default.
    #[test]
    fn the_wire_form_rejects_duplicate_unknown_null_and_inapplicable_fields() {
        for (document, cause) in [
            (
                r#"{"method":"growth_pct","params":{"rate":0.1,"rate":0.2}}"#,
                "appears more than once",
            ),
            (
                r#"{"method":"growth_pct","params":{"rate":0.1,"min":null}}"#,
                "'min' is null",
            ),
            (
                r#"{"method":"growth_pct","params":{"rate":0.1},"extra":1}"#,
                "unknown field `extra`",
            ),
            (
                r#"{"method":"growth_pct","params":{"rate":0.1,"seed":"1"}}"#,
                "unknown field `seed`",
            ),
            (
                r#"{"method":"fade_to_target","params":{"target":1.0}}"#,
                "missing field `shape`",
            ),
            (
                r#"{"method":"time_series","params":{"historical":[1.0,2.0]}}"#,
                "missing field `method`",
            ),
            (
                r#"{"method":"bootstrap","params":{"historical":[1.0,2.0],"seed":"1"}}"#,
                "missing field `mode`",
            ),
        ] {
            let error = serde_json::from_str::<ForecastSpec>(document).expect_err(document);
            assert!(error.to_string().contains(cause), "{document}: {error}");
        }
        // A method without required parameters may omit the object.
        let bare: ForecastSpec =
            serde_json::from_str(r#"{"method":"forward_fill"}"#).expect("bare");
        assert_eq!(bare, ForecastSpec::forward_fill());
        assert_eq!(
            serde_json::to_string(&bare).expect("serialize"),
            r#"{"method":"forward_fill"}"#
        );
        // A linear fade with a half-life reads but fails static validation.
        let fade: ForecastSpec = serde_json::from_str(
            r#"{"method":"fade_to_target","params":{"target":1.0,"shape":"linear","half_life":2.0}}"#,
        )
        .expect("well-typed");
        let error = fade.validate().expect_err("half_life on a linear fade");
        assert!(error.to_string().contains("half_life"), "{error}");
    }

    /// STATEMENTS-CONTRACTS-003 #4: a seed is an exact `u64`, written as a
    /// canonical decimal string so every host carries all 64 bits.
    #[test]
    fn seeds_are_exact_canonical_decimal_strings() {
        let spec = ForecastSpec::normal(0.0, 1.0, 9_007_199_254_740_993);
        let wire = serde_json::to_value(&spec).expect("serialize");
        assert_eq!(wire["params"]["seed"], json!("9007199254740993"));
        let back: ForecastSpec = serde_json::from_value(wire).expect("deserialize");
        assert_eq!(back, spec);

        for bad in [
            json!(42),
            json!(42.0),
            json!("042"),
            json!("+42"),
            json!("-1"),
            json!("1.5"),
            json!(""),
            json!("18446744073709551616"),
        ] {
            assert!(
                spec.clone().with_param("seed", bad.clone()).is_err(),
                "{bad}"
            );
        }
        assert!(
            spec.with_param("seed", json!("18446744073709551615"))
                .is_ok()
        );
        // Only a stochastic method takes a seed.
        assert!(
            ForecastSpec::growth(0.05)
                .with_param("seed", json!("1"))
                .is_err()
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
        ForecastSpec::time_series(vec![1.0, 2.0, 3.0], ForecastTrendMethod::Linear)
            .validate()
            .expect("time-series spec");
    }

    #[test]
    fn unknown_keys_and_variants_rejected_for_fade_to_target() {
        let spec = ForecastSpec::fade_to_target(50.0, ForecastFadeShape::Linear);
        let err = spec
            .with_param("shpae", json!("linear"))
            .expect_err("typo key");
        assert!(err.to_string().contains("shpae"), "{err}");

        let spec = ForecastSpec::fade_to_target(50.0, ForecastFadeShape::Linear);
        let err = spec
            .with_param("shape", json!("sigmoid"))
            .expect_err("unknown shape");
        assert!(err.to_string().contains("sigmoid"), "{err}");

        assert!(
            ForecastSpec::from_params(
                crate::types::ForecastMethod::FadeToTarget,
                indexmap::IndexMap::new(),
            )
            .is_err(),
            "target is required"
        );
    }
}

//! Statistical forecast helpers that produce deterministic sequences.
//!
//! Each algorithm consumes a pre-seeded pseudo-random number generator so that
//! repeated calls with identical parameters return the same series. This makes
//! them suitable for scenario analysis where reproducibility matters.
//!
//! Uses [`Pcg64Rng`] for production-quality random number generation.

use crate::error::{Error, Result};
use crate::types::{ForecastMethod, NodeId};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::math::random::{Pcg64Rng, RandomNumberGenerator};
use indexmap::IndexMap;

/// Common parameters for statistical distribution forecasts.
struct DistributionParams {
    mean: f64,
    std_dev: f64,
    seed: u64,
}

fn build_rng(seed: u64, stream_id: Option<u64>) -> Pcg64Rng {
    match stream_id {
        Some(stream_id) => Pcg64Rng::new_with_stream(seed, stream_id),
        None => Pcg64Rng::new(seed),
    }
}

/// Deterministic 64-bit mix of a node identifier for Monte Carlo seeding.
///
/// Used to decorrelate independent stochastic forecasts across nodes while
/// keeping results reproducible for a given `(seed, path_offset, node_id)` tuple.
///
/// Implementation: 64-bit FNV-1a absorption followed by a splitmix64 finalizer
/// to improve avalanche. FNV-1a alone has poor bit diffusion and can cluster
/// similar identifiers (e.g. `revenue_2024`, `revenue_2025`), which in
/// correlated Monte Carlo can translate into correlated seed streams across
/// otherwise-independent nodes. The splitmix64 finalizer is the standard
/// finalizer from Vigna's SplitMix generator; it is bijective, so no
/// collisions are introduced and reproducibility is preserved.
#[must_use]
pub(crate) fn stable_hash_u64(node_id: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in node_id.as_bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    splitmix64_finalize(hash)
}

#[inline]
fn splitmix64_finalize(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Parse a JSON seed as `u64`, accepting integer JSON numbers stored as floats
/// (e.g. `42.0`) when they represent exact integers.
pub(crate) fn parse_seed_json(value: &serde_json::Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        let f = value.as_f64()?;
        if !f.is_finite() || f.fract() != 0.0 || f < 0.0 || f > u64::MAX as f64 {
            return None;
        }
        Some(f as u64)
    })
}

/// Optional Monte Carlo correlation pair: `(peer_node_id, rho)` in `[-1, 1]`.
///
/// When both `correlation_with` and `correlation` are present in forecast params,
/// Monte Carlo evaluation samples shocks correlated with the peer node's standard
/// normal shocks (same forecast period). The peer node must be evaluated earlier in
/// the dependency order so its Z-scores are available.
pub(crate) fn parse_correlation_params(
    params: &IndexMap<String, serde_json::Value>,
) -> Result<Option<(String, f64)>> {
    let with = params
        .get("correlation_with")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let rho = params.get("correlation").and_then(|v| v.as_f64());

    match (with, rho) {
        (None, None) => Ok(None),
        (Some(peer), Some(rho)) => {
            if !rho.is_finite() || !(-1.0..=1.0).contains(&rho) {
                return Err(Error::forecast(format!(
                    "Monte Carlo 'correlation' must be finite and in [-1, 1], got {rho}"
                )));
            }
            Ok(Some((peer, rho)))
        }
        (None, Some(_)) | (Some(_), None) => Err(Error::forecast(
            "Monte Carlo correlation requires both 'correlation_with' (string) and \
             'correlation' (number in [-1, 1])"
                .to_string(),
        )),
    }
}

/// Extract distribution parameters from the params map.
///
/// Validates that mean, std_dev, and seed are present and valid.
fn extract_distribution_params(
    params: &IndexMap<String, serde_json::Value>,
    method_name: &str,
) -> Result<DistributionParams> {
    let mean = params.get("mean").and_then(|v| v.as_f64()).ok_or_else(|| {
        Error::forecast(format!(
            "Missing or invalid 'mean' parameter for {} forecast. \
             Expected a number.",
            method_name
        ))
    })?;

    let std_dev = params
        .get("std_dev")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| {
            Error::forecast(format!(
                "Missing or invalid 'std_dev' parameter for {} forecast. \
                 Expected a positive number.",
                method_name
            ))
        })?;

    let seed = params
        .get("seed")
        .and_then(parse_seed_json)
        .ok_or_else(|| {
            Error::forecast(format!(
                "Missing or invalid 'seed' parameter for {} forecast. \
                 A non-negative integer seed is required for deterministic sampling (e.g., 42).",
                method_name
            ))
        })?;

    if std_dev < 0.0 {
        return Err(Error::forecast(format!(
            "Standard deviation must be non-negative, got {}",
            std_dev
        )));
    }
    if !mean.is_finite() || !std_dev.is_finite() {
        return Err(Error::forecast(format!(
            "{} forecast requires finite mean and std_dev",
            method_name
        )));
    }

    Ok(DistributionParams {
        mean,
        std_dev,
        seed,
    })
}

/// Normal distribution forecast (deterministic with seed).
///
/// Produces a random-walk path starting from `base_value`:
/// `value[t] = value[t-1] + N(mean, std_dev²)`.
///
/// When `base_value` is zero the series reduces to a cumulative sum of
/// i.i.d. normal increments (a discrete Wiener process with drift).
///
/// # Arguments
///
/// * `base_value` - Starting level for the random walk
/// * `forecast_periods` - Periods to simulate
/// * `params` - JSON parameter map containing `mean`, `std_dev`, and `seed`
///
/// `mean` is the per-period drift and `std_dev` is the per-period
/// volatility. `seed` must be integer-like and is required for
/// deterministic sampling.
///
/// # Returns
///
/// Returns one simulated scalar per forecast period forming a path.
///
/// # Errors
///
/// Returns an error if the parameter map is incomplete, if `std_dev` is
/// negative, or if simulation produces a non-finite value.
///
/// # References
///
/// - Monte Carlo simulation practice: `docs/REFERENCES.md#glasserman-2004-monte-carlo`
/// - Numerical sampling techniques: `docs/REFERENCES.md#press-numerical-recipes`
pub(crate) fn normal_forecast_with_stream(
    base_value: f64,
    forecast_periods: &[PeriodId],
    params: &IndexMap<String, serde_json::Value>,
    stream_id: Option<u64>,
) -> Result<IndexMap<PeriodId, f64>> {
    let p = extract_distribution_params(params, "Normal")?;

    let mut rng = build_rng(p.seed, stream_id);
    let mut results = IndexMap::new();
    let mut prev = base_value;

    for period_id in forecast_periods {
        let z = rng.normal(0.0, 1.0);
        let value = prev + p.mean + p.std_dev * z;
        if !value.is_finite() {
            return Err(Error::forecast(format!(
                "Normal forecast produced a non-finite value at period {:?}",
                period_id
            )));
        }
        results.insert(*period_id, value);
        prev = value;
    }

    Ok(results)
}

/// Validate the strictly positive anchor required by a LogNormal path.
///
/// Zero is not silently converted into an unrelated i.i.d. level process:
/// under GBM it is absorbing, and no path-return shocks can be recovered for
/// correlated Monte Carlo peers. Negative and non-finite levels are likewise
/// invalid.
fn validate_lognormal_base(base_value: f64, context: &str) -> Result<()> {
    if !base_value.is_finite() || base_value <= 0.0 {
        return Err(crate::error::Error::forecast(format!(
            "{context} requires a finite, strictly positive base value; got {base_value}. \
             Use a Normal forecast or an explicitly level-based model for series that can \
             be zero or negative."
        )));
    }
    Ok(())
}

/// Log-normal distribution forecast (deterministic with seed).
///
/// Produces a geometric Brownian motion path starting from `base_value`:
/// `value[t] = value[t-1] * exp(N(mean - 0.5*std_dev², std_dev))`.
///
/// The Itô correction (`-0.5 * σ²`) ensures the expected value of the
/// multiplicative increment is `exp(mean)`, matching the drift convention
/// in Black–Scholes and standard GBM literature.
///
/// `base_value` must be strictly positive. Zero is an absorbing state under
/// GBM and cannot be inverted to recover Monte Carlo correlation shocks, so
/// callers must use a Normal or explicitly level-based forecast for zero-
/// anchored series.
///
/// # Arguments
///
/// * `base_value` - Strictly positive starting level for the geometric walk
/// * `forecast_periods` - Periods to simulate
/// * `params` - JSON parameter map containing `mean`, `std_dev`, and `seed`
///
/// `mean` and `std_dev` describe the underlying log-return distribution.
/// `seed` must be integer-like and is required for deterministic sampling.
///
/// # Returns
///
/// Returns one positive simulated scalar per forecast period.
///
/// # Errors
///
/// Returns an error if the parameter map is incomplete, `base_value` is not
/// finite and strictly positive, `std_dev` is negative, or exponentiation
/// produces a non-finite value.
///
/// # References
///
/// - Monte Carlo simulation practice: `docs/REFERENCES.md#glasserman-2004-monte-carlo`
/// - Numerical sampling techniques: `docs/REFERENCES.md#press-numerical-recipes`
pub(crate) fn lognormal_forecast_with_stream(
    base_value: f64,
    forecast_periods: &[PeriodId],
    params: &IndexMap<String, serde_json::Value>,
    stream_id: Option<u64>,
) -> Result<IndexMap<PeriodId, f64>> {
    let p = extract_distribution_params(params, "LogNormal")?;

    if p.std_dev == 0.0 {
        tracing::warn!(
            "LogNormal forecast with std_dev=0.0 produces degenerate distribution (all values identical)"
        );
    }

    validate_lognormal_base(base_value, "LogNormal forecast")?;

    let mut rng = build_rng(p.seed, stream_id);
    let mut results = IndexMap::new();
    let mut prev = base_value;

    const EXP_CLAMP: f64 = 709.0;

    for period_id in forecast_periods {
        let z = rng.normal(0.0, 1.0);
        let log_return = (p.mean - 0.5 * p.std_dev * p.std_dev) + p.std_dev * z;
        if log_return.abs() > EXP_CLAMP {
            tracing::warn!(
                mean = p.mean,
                std_dev = p.std_dev,
                "LogNormal exponent clamped to avoid overflow"
            );
        }
        let value = prev * log_return.clamp(-EXP_CLAMP, EXP_CLAMP).exp();
        if !value.is_finite() {
            return Err(Error::forecast(format!(
                "LogNormal forecast produced a non-finite value at period {:?}",
                period_id
            )));
        }
        results.insert(*period_id, value);
        prev = value;
    }

    Ok(results)
}

/// Parameters for the mean-reverting AR(1) forecast.
struct MeanRevertingParams {
    long_run_mean: f64,
    reversion_speed: f64,
    std_dev: f64,
    seed: u64,
}

/// Extract and validate mean-reverting AR(1) parameters from the params map.
fn extract_mean_reverting_params(
    params: &IndexMap<String, serde_json::Value>,
) -> Result<MeanRevertingParams> {
    let long_run_mean = params
        .get("long_run_mean")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| {
            Error::forecast(
                "Missing or invalid 'long_run_mean' parameter for MeanReverting forecast. \
                 Expected a finite number (the level the series reverts toward).",
            )
        })?;
    let reversion_speed = params
        .get("reversion_speed")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| {
            Error::forecast(
                "Missing or invalid 'reversion_speed' parameter for MeanReverting forecast. \
                 Expected a number in (0, 1] (fraction of the gap closed each period).",
            )
        })?;
    let std_dev = params
        .get("std_dev")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| {
            Error::forecast(
                "Missing or invalid 'std_dev' parameter for MeanReverting forecast. \
                 Expected a non-negative number (per-period shock volatility).",
            )
        })?;
    let seed = params
        .get("seed")
        .and_then(parse_seed_json)
        .ok_or_else(|| {
            Error::forecast(
                "Missing or invalid 'seed' parameter for MeanReverting forecast. \
                 A non-negative integer seed is required for deterministic sampling (e.g., 42).",
            )
        })?;

    if !long_run_mean.is_finite() {
        return Err(Error::forecast(format!(
            "MeanReverting 'long_run_mean' must be finite, got {long_run_mean}"
        )));
    }
    if !reversion_speed.is_finite() || reversion_speed <= 0.0 || reversion_speed > 1.0 {
        return Err(Error::forecast(format!(
            "MeanReverting 'reversion_speed' must be in (0, 1], got {reversion_speed}. \
             1.0 reverts fully to the long-run mean each period; values near 0 revert slowly."
        )));
    }
    if !std_dev.is_finite() || std_dev < 0.0 {
        return Err(Error::forecast(format!(
            "MeanReverting 'std_dev' must be a non-negative finite number, got {std_dev}"
        )));
    }

    Ok(MeanRevertingParams {
        long_run_mean,
        reversion_speed,
        std_dev,
        seed,
    })
}

/// Mean-reverting AR(1) forecast (deterministic with seed).
///
/// Produces an Ornstein–Uhlenbeck-style discrete path starting from
/// `base_value`:
/// `value[t] = value[t-1] + reversion_speed * (long_run_mean - value[t-1]) + std_dev * z[t]`.
///
/// Use this for autocorrelated series that revert toward a through-the-cycle
/// level — credit spreads, charge-off rates, net interest margins.
///
/// # Arguments
///
/// * `base_value` - Starting level for the mean-reverting walk; must be finite
/// * `forecast_periods` - Periods to simulate
/// * `params` - JSON parameter map containing `long_run_mean` (level the
///   series reverts toward, node units), `reversion_speed` (fraction of the
///   gap closed per period, in `(0, 1]`), `std_dev` (per-period additive
///   shock volatility, non-negative), and `seed` (integer-like, required for
///   deterministic sampling)
///
/// # Returns
///
/// Returns one simulated scalar per forecast period forming a path.
///
/// # Errors
///
/// Returns an error if the parameter map is incomplete or out of range, if
/// `base_value` is non-finite, or if simulation produces a non-finite value.
///
/// # References
///
/// - Monte Carlo simulation practice: `docs/REFERENCES.md#glasserman-2004-monte-carlo`
pub(crate) fn mean_reverting_forecast_with_stream(
    base_value: f64,
    forecast_periods: &[PeriodId],
    params: &IndexMap<String, serde_json::Value>,
    stream_id: Option<u64>,
) -> Result<IndexMap<PeriodId, f64>> {
    let p = extract_mean_reverting_params(params)?;

    if !base_value.is_finite() {
        return Err(Error::forecast(format!(
            "MeanReverting forecast requires a finite base_value, got {base_value}"
        )));
    }

    let mut rng = build_rng(p.seed, stream_id);
    let mut results = IndexMap::new();
    let mut prev = base_value;

    for period_id in forecast_periods {
        let z = rng.normal(0.0, 1.0);
        let value = prev + p.reversion_speed * (p.long_run_mean - prev) + p.std_dev * z;
        if !value.is_finite() {
            return Err(Error::forecast(format!(
                "MeanReverting forecast produced a non-finite value at period {:?}",
                period_id
            )));
        }
        results.insert(*period_id, value);
        prev = value;
    }

    Ok(results)
}

/// Bootstrap resampling mode: what the historical series is resampled as.
enum BootstrapMode {
    /// Resample period-over-period growth rates and compound multiplicatively.
    Growth,
    /// Resample additive level changes.
    Diff,
}

/// Parameters for the historical-bootstrap forecast.
struct BootstrapParams {
    /// Resampled per-step increments: growth rates or level diffs.
    increments: Vec<f64>,
    mode: BootstrapMode,
    seed: u64,
}

/// Extract and validate bootstrap parameters from the params map.
fn extract_bootstrap_params(
    params: &IndexMap<String, serde_json::Value>,
) -> Result<BootstrapParams> {
    let historical = params
        .get("historical")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            Error::forecast(
                "Missing or invalid 'historical' parameter for Bootstrap forecast. \
                 Expected an array of at least 2 numbers (oldest first).",
            )
        })?;
    if historical.len() < 2 {
        return Err(Error::forecast(format!(
            "Bootstrap forecast needs at least 2 historical values to derive increments, \
             got {}. Provide more history in the 'historical' parameter.",
            historical.len()
        )));
    }
    let mut hist = Vec::with_capacity(historical.len());
    for (idx, value) in historical.iter().enumerate() {
        let number = value.as_f64().filter(|n| n.is_finite()).ok_or_else(|| {
            Error::forecast(format!(
                "Bootstrap historical value at index {idx} must be a finite number, got {value}"
            ))
        })?;
        hist.push(number);
    }

    let mode = match params.get("mode") {
        None => BootstrapMode::Growth,
        Some(value) => match value.as_str() {
            Some("growth") => BootstrapMode::Growth,
            Some("diff") => BootstrapMode::Diff,
            _ => {
                return Err(Error::forecast(format!(
                    "Bootstrap 'mode' must be \"growth\" or \"diff\", got {value}"
                )));
            }
        },
    };

    let seed = params
        .get("seed")
        .and_then(parse_seed_json)
        .ok_or_else(|| {
            Error::forecast(
                "Missing or invalid 'seed' parameter for Bootstrap forecast. \
                 A non-negative integer seed is required for deterministic resampling (e.g., 42).",
            )
        })?;

    let increments = match mode {
        BootstrapMode::Growth => {
            if hist.iter().any(|&h| h <= 0.0) {
                return Err(Error::forecast(
                    "Bootstrap growth mode requires strictly positive historical values \
                     (growth rates are derived from consecutive ratios). Use mode = \"diff\" \
                     for series that touch zero or change sign."
                        .to_string(),
                ));
            }
            hist.windows(2).map(|w| w[1] / w[0] - 1.0).collect()
        }
        BootstrapMode::Diff => hist.windows(2).map(|w| w[1] - w[0]).collect(),
    };

    Ok(BootstrapParams {
        increments,
        mode,
        seed,
    })
}

/// Historical bootstrap forecast (deterministic with seed).
///
/// Resamples per-period increments observed in a historical series (i.i.d.,
/// with replacement) and applies them sequentially from `base_value`:
///
/// - `mode = "growth"` (default): increments are period-over-period growth
///   rates `h[i]/h[i-1] - 1`; the path compounds
///   `value[t] = value[t-1] * (1 + g*)`. Requires strictly positive history.
/// - `mode = "diff"`: increments are additive level changes `h[i] - h[i-1]`;
///   the path accumulates `value[t] = value[t-1] + d*`. Works for series
///   that cross zero.
///
/// Unlike the parametric Normal/LogNormal methods, the bootstrap reproduces
/// the empirical distribution of observed changes — including fat tails —
/// without a normality assumption.
///
/// # Arguments
///
/// * `base_value` - Starting level for the resampled path; must be finite
/// * `forecast_periods` - Periods to simulate
/// * `params` - JSON parameter map containing `historical` (array of at
///   least 2 finite numbers in the node's own units, oldest first), optional
///   `mode` (`"growth"` default or `"diff"`), and `seed` (integer-like,
///   required for deterministic resampling)
///
/// # Returns
///
/// Returns one simulated scalar per forecast period forming a path.
///
/// # Errors
///
/// Returns an error if the parameter map is incomplete or malformed, if the
/// history is too short or non-finite, if growth mode is requested with
/// non-positive history, or if the path produces a non-finite value.
pub(crate) fn bootstrap_forecast_with_stream(
    base_value: f64,
    forecast_periods: &[PeriodId],
    params: &IndexMap<String, serde_json::Value>,
    stream_id: Option<u64>,
) -> Result<IndexMap<PeriodId, f64>> {
    let p = extract_bootstrap_params(params)?;

    if !base_value.is_finite() {
        return Err(Error::forecast(format!(
            "Bootstrap forecast requires a finite base_value, got {base_value}"
        )));
    }

    let mut rng = build_rng(p.seed, stream_id);
    let mut results = IndexMap::new();
    let mut prev = base_value;
    let n = p.increments.len();

    for period_id in forecast_periods {
        // Uniform in [0, 1) scaled to an index; the min() guards the
        // (unreachable in exact arithmetic) u == 1.0 edge.
        let idx = ((rng.uniform() * n as f64) as usize).min(n - 1);
        let value = match p.mode {
            BootstrapMode::Growth => prev * (1.0 + p.increments[idx]),
            BootstrapMode::Diff => prev + p.increments[idx],
        };
        if !value.is_finite() {
            return Err(Error::forecast(format!(
                "Bootstrap forecast produced a non-finite value at period {:?}. \
                 Consider fewer periods or less extreme historical increments.",
                period_id
            )));
        }
        results.insert(*period_id, value);
        prev = value;
    }

    Ok(results)
}

/// Store standard-normal Z scores for independent Monte Carlo forecasts so peers can
/// correlate in a later [`crate::evaluator::forecast_eval::evaluate_forecast`] pass.
///
/// Recorded Z values are the **shock** Z that was applied at each period, not a
/// level normalization. They must match the recurrences in
/// [`normal_forecast_with_stream`] and [`lognormal_forecast_with_stream`]:
///
/// - Normal (random walk): `v_t = v_{t-1} + mean + std_dev * z_t`
///   ⇒ `z_t = (v_t - v_{t-1} - mean) / std_dev`.
/// - LogNormal (strictly positive GBM):
///   `v_t = v_{t-1} * exp((mean - 0.5*std_dev²) + std_dev * z_t)`
///   ⇒ `z_t = (ln(v_t / v_{t-1}) - (mean - 0.5*std_dev²)) / std_dev`.
///
/// These per-period shocks are what [`monte_carlo_correlated_series`] mixes
/// via `ρ·Z_peer + sqrt(1-ρ²)·Z_indep`, so the correlation is applied in the
/// same shock space that generated the peer path.
pub(crate) fn record_independent_z_scores_for_mc(
    method: ForecastMethod,
    params: &IndexMap<String, serde_json::Value>,
    forecast_periods: &[PeriodId],
    values: &IndexMap<PeriodId, f64>,
    base_value: f64,
    node_id: &NodeId,
    mc_z_cache: &mut IndexMap<NodeId, IndexMap<PeriodId, f64>>,
) -> Result<()> {
    match method {
        ForecastMethod::Normal => {
            let p = extract_distribution_params(params, "Normal")?;
            let entry = mc_z_cache.entry(node_id.clone()).or_default();
            let mut prev = base_value;
            for pid in forecast_periods {
                let v = *values.get(pid).ok_or_else(|| {
                    Error::forecast(format!(
                        "Monte Carlo forecast missing value for period {:?}",
                        pid
                    ))
                })?;
                let z = if p.std_dev == 0.0 {
                    0.0
                } else {
                    (v - prev - p.mean) / p.std_dev
                };
                entry.insert(*pid, z);
                prev = v;
            }
        }
        ForecastMethod::LogNormal => {
            let p = extract_distribution_params(params, "LogNormal")?;
            // These Z-scores invert the generating recurrence, so they are only
            // meaningful for a base the generator itself would accept. Apply the
            // same guard rather than recording shocks that would silently
            // propagate into every correlated peer.
            validate_lognormal_base(
                base_value,
                &format!("Monte Carlo Z-score recording for '{node_id}'"),
            )?;
            let entry = mc_z_cache.entry(node_id.clone()).or_default();
            // Must match the strictly positive GBM recurrence used by the
            // generator.
            let mut prev = base_value;
            for pid in forecast_periods {
                let v = *values.get(pid).ok_or_else(|| {
                    Error::forecast(format!(
                        "Monte Carlo forecast missing value for period {:?}",
                        pid
                    ))
                })?;
                let z = if p.std_dev == 0.0 {
                    0.0
                } else {
                    let ln_ratio = (v / prev).ln();
                    (ln_ratio - (p.mean - 0.5 * p.std_dev * p.std_dev)) / p.std_dev
                };
                entry.insert(*pid, z);
                prev = v;
            }
        }
        ForecastMethod::MeanReverting => {
            let p = extract_mean_reverting_params(params)?;
            if !base_value.is_finite() {
                return Err(Error::forecast(format!(
                    "Monte Carlo Z-score recording for '{node_id}' requires a finite \
                     base_value, got {base_value}"
                )));
            }
            let entry = mc_z_cache.entry(node_id.clone()).or_default();
            let mut prev = base_value;
            for pid in forecast_periods {
                let v = *values.get(pid).ok_or_else(|| {
                    Error::forecast(format!(
                        "Monte Carlo forecast missing value for period {:?}",
                        pid
                    ))
                })?;
                // Inverts v_t = v_{t-1} + κ(θ − v_{t-1}) + σ·z_t.
                let z = if p.std_dev == 0.0 {
                    0.0
                } else {
                    (v - prev - p.reversion_speed * (p.long_run_mean - prev)) / p.std_dev
                };
                entry.insert(*pid, z);
                prev = v;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Inputs for [`monte_carlo_correlated_series`].
pub(crate) struct CorrelatedMonteCarloSeries<'a> {
    /// Forecast method (Normal, LogNormal, or MeanReverting only).
    pub method: ForecastMethod,
    /// Method parameters.
    pub params: &'a IndexMap<String, serde_json::Value>,
    /// Starting level anchoring the path; must match the peer-path convention.
    pub base_value: f64,
    pub forecast_periods: &'a [PeriodId],
    pub seed_offset: u64,
    pub node_id: &'a str,
    pub peer_id: &'a str,
    pub rho: f64,
    pub mc_z_cache: &'a IndexMap<NodeId, IndexMap<PeriodId, f64>>,
}

/// Correlated Normal / LogNormal / MeanReverting series for Monte Carlo.
///
/// The shock `z_t = ρ·z_peer + sqrt(1-ρ²)·z_indep` is applied in the **same
/// recurrence** as the independent forecast paths
/// ([`normal_forecast_with_stream`], [`lognormal_forecast_with_stream`],
/// [`mean_reverting_forecast_with_stream`]) so correlated and uncorrelated
/// outputs live on the same process:
///
/// - Normal: additive random walk `v_t = v_{t-1} + mean + std_dev * z_t`
///   anchored at `base_value`.
/// - LogNormal (strictly positive GBM):
///   `v_t = v_{t-1} * exp((mean - 0.5*std_dev²) + std_dev * z_t)`.
///   Zero, negative, and non-finite bases are rejected.
/// - MeanReverting: AR(1)
///   `v_t = v_{t-1} + reversion_speed·(long_run_mean - v_{t-1}) + std_dev * z_t`.
///
/// Matches the shock convention recorded by
/// [`record_independent_z_scores_for_mc`] so linear correlation of the
/// peer path is preserved.
pub(crate) fn monte_carlo_correlated_series(
    input: CorrelatedMonteCarloSeries<'_>,
) -> Result<(IndexMap<PeriodId, f64>, IndexMap<PeriodId, f64>)> {
    let CorrelatedMonteCarloSeries {
        method,
        params,
        base_value,
        forecast_periods,
        seed_offset,
        node_id,
        peer_id,
        rho,
        mc_z_cache,
    } = input;

    // This path is called directly from forecast evaluation, bypassing
    // `apply_forecast_internal`'s dispatch, so it validates its own keys.
    crate::forecast::validate_params(method, params)?;

    if !base_value.is_finite() {
        return Err(Error::forecast(format!(
            "Monte Carlo correlated forecast for '{node_id}' requires a finite base_value, \
             got {base_value}"
        )));
    }
    if matches!(method, ForecastMethod::LogNormal) {
        // Apply the identical guard the independent path uses, so toggling
        // `correlation_with` on a node cannot change whether an invalid base is
        // caught or silently simulated in a different regime.
        validate_lognormal_base(
            base_value,
            &format!("Monte Carlo correlated LogNormal forecast for '{node_id}'"),
        )?;
    }

    let peer_key = NodeId::new(peer_id);
    let peer_map = mc_z_cache.get(&peer_key).ok_or_else(|| {
        Error::forecast(format!(
            "Monte Carlo correlation peer '{peer_id}' must be evaluated before node '{node_id}' \
             (no Z-scores in cache for peer)"
        ))
    })?;

    // A zero-variance peer (e.g. std_dev = 0) records all-zero Z-scores. Mixing
    // that into `z = rho·z_peer + sqrt(1-rho^2)·z_indep` would silently collapse
    // the dependent node's shock variance from sigma^2 to (1-rho^2)·sigma^2 with
    // no diagnostic. Reject it rather than produce badly understated tails.
    if rho.abs() > 0.0
        && !forecast_periods.is_empty()
        && forecast_periods
            .iter()
            .all(|pid| peer_map.get(pid).copied() == Some(0.0))
    {
        return Err(Error::forecast(format!(
            "Monte Carlo correlation peer '{peer_id}' has zero variance (e.g. std_dev = 0), so it \
             cannot anchor the correlation for '{node_id}': the dependent node's shock variance \
             would silently collapse to (1 - rho^2)·sigma^2. Give the peer a positive std_dev or \
             remove the correlation."
        )));
    }

    /// Per-method recurrence parameters for the correlated path.
    enum Kernel {
        Normal(DistributionParams),
        LogNormal(DistributionParams),
        MeanReverting(MeanRevertingParams),
    }

    let kernel = match method {
        ForecastMethod::Normal => Kernel::Normal(extract_distribution_params(params, "Normal")?),
        ForecastMethod::LogNormal => {
            Kernel::LogNormal(extract_distribution_params(params, "LogNormal")?)
        }
        ForecastMethod::MeanReverting => {
            Kernel::MeanReverting(extract_mean_reverting_params(params)?)
        }
        _ => {
            return Err(Error::forecast(
                "Monte Carlo correlation is only supported for Normal, LogNormal, and \
                 MeanReverting forecasts"
                    .to_string(),
            ));
        }
    };
    let seed = match &kernel {
        Kernel::Normal(p) | Kernel::LogNormal(p) => p.seed,
        Kernel::MeanReverting(p) => p.seed,
    };

    let mut rng = Pcg64Rng::new_with_stream(seed ^ stable_hash_u64(node_id), seed_offset);
    let mut values = IndexMap::new();
    let mut z_out = IndexMap::new();
    let mut prev = base_value;

    // Clamp kept in sync with `lognormal_forecast_with_stream`.
    const EXP_CLAMP: f64 = 709.0;
    // sqrt(1 - ρ²) with floor at zero in case of tiny numerical overshoot.
    let indep_weight = (1.0 - rho * rho).max(0.0).sqrt();

    for period_id in forecast_periods {
        let z_peer = peer_map.get(period_id).copied().ok_or_else(|| {
            Error::forecast(format!(
                "Monte Carlo correlation: peer '{peer_id}' has no Z-score for period {:?}. \
                 Ensure the peer forecast covers the same forecast periods.",
                period_id
            ))
        })?;

        let z_indep = rng.normal(0.0, 1.0);
        let z = rho * z_peer + indep_weight * z_indep;
        z_out.insert(*period_id, z);

        let value = match &kernel {
            Kernel::Normal(p) => prev + p.mean + p.std_dev * z,
            Kernel::LogNormal(p) => {
                let log_return = (p.mean - 0.5 * p.std_dev * p.std_dev) + p.std_dev * z;
                if log_return.abs() > EXP_CLAMP {
                    tracing::warn!(
                        mean = p.mean,
                        std_dev = p.std_dev,
                        "LogNormal correlated exponent clamped to avoid overflow"
                    );
                }
                prev * log_return.clamp(-EXP_CLAMP, EXP_CLAMP).exp()
            }
            Kernel::MeanReverting(p) => {
                prev + p.reversion_speed * (p.long_run_mean - prev) + p.std_dev * z
            }
        };

        if !value.is_finite() {
            return Err(Error::forecast(format!(
                "{:?} correlated forecast produced a non-finite value at period {:?}",
                method, period_id
            )));
        }
        values.insert(*period_id, value);
        prev = value;
    }

    Ok((values, z_out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::PeriodId;

    fn normal_forecast(
        base_value: f64,
        periods: &[PeriodId],
        params: &IndexMap<String, serde_json::Value>,
    ) -> Result<IndexMap<PeriodId, f64>> {
        normal_forecast_with_stream(base_value, periods, params, None)
    }

    fn lognormal_forecast(
        base_value: f64,
        periods: &[PeriodId],
        params: &IndexMap<String, serde_json::Value>,
    ) -> Result<IndexMap<PeriodId, f64>> {
        lognormal_forecast_with_stream(base_value, periods, params, None)
    }

    fn mean_reverting_forecast(
        base_value: f64,
        periods: &[PeriodId],
        params: &IndexMap<String, serde_json::Value>,
    ) -> Result<IndexMap<PeriodId, f64>> {
        mean_reverting_forecast_with_stream(base_value, periods, params, None)
    }

    fn bootstrap_forecast(
        base_value: f64,
        periods: &[PeriodId],
        params: &IndexMap<String, serde_json::Value>,
    ) -> Result<IndexMap<PeriodId, f64>> {
        bootstrap_forecast_with_stream(base_value, periods, params, None)
    }

    #[test]
    fn test_parse_seed_accepts_integer_like_json_float() {
        let v = serde_json::json!(42.0);
        assert_eq!(parse_seed_json(&v), Some(42));
    }

    fn lognormal_params() -> IndexMap<String, serde_json::Value> {
        let mut params = IndexMap::new();
        params.insert("mean".to_string(), serde_json::json!(0.02));
        params.insert("std_dev".to_string(), serde_json::json!(0.1));
        params.insert("seed".to_string(), serde_json::json!(7));
        params
    }

    #[test]
    fn lognormal_zero_base_is_rejected() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
        ];
        let error = lognormal_forecast_with_stream(0.0, &periods, &lognormal_params(), Some(11))
            .expect_err("zero cannot anchor a GBM path");
        assert!(error.to_string().contains("strictly positive"));
    }

    #[test]
    fn lognormal_zero_base_z_recording_is_rejected() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
        ];
        let params = lognormal_params();
        let node = NodeId::new("node");
        let values = IndexMap::from([(periods[0], 0.0), (periods[1], 0.0)]);
        let mut cache: IndexMap<NodeId, IndexMap<PeriodId, f64>> = IndexMap::new();

        let error = record_independent_z_scores_for_mc(
            ForecastMethod::LogNormal,
            &params,
            &periods,
            &values,
            0.0,
            &node,
            &mut cache,
        )
        .expect_err("zero cannot produce GBM return shocks");
        assert!(error.to_string().contains("strictly positive"));
    }

    /// A misspelled parameter must fail loudly rather than be ignored.
    ///
    /// Only TimeSeries and Seasonal rejected unknown keys, so a statistical
    /// node ran clean while silently ignoring the typo. The dangerous shape is
    /// a rename left half-done: `sigma: 0.4` added beside a stale
    /// `std_dev: 0.1` simulated at a quarter of the intended volatility, with
    /// every downstream tail and breach probability wrong and no diagnostic.
    #[test]
    fn statistical_forecast_rejects_unknown_parameter_keys() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        let mut params = lognormal_params();
        params.insert("sigma".to_string(), serde_json::json!(0.4));

        let spec = crate::types::ForecastSpec {
            method: ForecastMethod::LogNormal,
            params,
        };
        let err = crate::forecast::apply_forecast_for_node(&spec, 100.0, &periods, "node")
            .expect_err("an unknown parameter must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("sigma") && msg.contains("std_dev"),
            "error should name the bad key and list the allowed set: {msg}"
        );
    }

    /// The correlated Monte Carlo path bypasses the normal dispatch, so it
    /// must validate its own keys too.
    #[test]
    fn correlated_forecast_rejects_unknown_parameter_keys() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        let mut params = lognormal_params();
        params.insert("correlation_with".to_string(), serde_json::json!("peer"));
        params.insert("correlation".to_string(), serde_json::json!(0.5));
        params.insert("vol_floor".to_string(), serde_json::json!(0.05));

        let mut cache: IndexMap<NodeId, IndexMap<PeriodId, f64>> = IndexMap::new();
        cache
            .entry(NodeId::new("peer"))
            .or_default()
            .insert(periods[0], 0.5);

        let err = monte_carlo_correlated_series(CorrelatedMonteCarloSeries {
            method: ForecastMethod::LogNormal,
            params: &params,
            base_value: 100.0,
            forecast_periods: &periods,
            seed_offset: 1,
            node_id: "node",
            peer_id: "peer",
            rho: 0.5,
            mc_z_cache: &cache,
        })
        .expect_err("an unknown parameter must be rejected on the correlated path too");
        assert!(
            err.to_string().contains("vol_floor"),
            "error should name the bad key: {err}"
        );
    }

    /// Correlation keys stay legal on statistical methods.
    #[test]
    fn correlation_keys_are_allowed_on_statistical_methods() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        let mut params = lognormal_params();
        params.insert("correlation_with".to_string(), serde_json::json!("peer"));
        params.insert("correlation".to_string(), serde_json::json!(0.5));

        let spec = crate::types::ForecastSpec {
            method: ForecastMethod::LogNormal,
            params,
        };
        crate::forecast::apply_forecast_for_node(&spec, 100.0, &periods, "node")
            .expect("correlation parameters are legal to configure");
    }

    /// A non-finite base must fail the same strict-positive validation as zero
    /// and negative anchors. Non-finite formula outputs can reach
    /// `determine_base_value`, so the forecast boundary must fail closed.
    #[test]
    fn lognormal_rejects_non_finite_base_value() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        let err = lognormal_forecast(f64::NAN, &periods, &lognormal_params())
            .expect_err("NaN base must be rejected");
        assert!(
            err.to_string().contains("finite"),
            "error should flag the non-finite base: {err}"
        );
    }

    /// Correlated and independent paths apply the same strictly-positive base
    /// requirement, so enabling correlation cannot change validation.
    #[test]
    fn correlated_lognormal_rejects_negative_base_like_independent_path() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        let params = lognormal_params();

        // The independent path is the reference behaviour.
        let independent_err = lognormal_forecast(-5.0, &periods, &params)
            .expect_err("independent path must reject a negative base");

        let mut cache: IndexMap<NodeId, IndexMap<PeriodId, f64>> = IndexMap::new();
        cache
            .entry(NodeId::new("peer"))
            .or_default()
            .insert(periods[0], 0.5);

        let correlated_err = monte_carlo_correlated_series(CorrelatedMonteCarloSeries {
            method: ForecastMethod::LogNormal,
            params: &params,
            base_value: -5.0,
            forecast_periods: &periods,
            seed_offset: 1,
            node_id: "node",
            peer_id: "peer",
            rho: 0.5,
            mc_z_cache: &cache,
        })
        .expect_err("correlated path must reject a negative base like the independent path");

        for err in [&independent_err, &correlated_err] {
            assert!(
                err.to_string().contains("negative") || err.to_string().contains("non-negative"),
                "both paths should reject a negative base for the same reason: {err}"
            );
        }
    }

    /// The correlated path must also reject a non-finite base.
    #[test]
    fn correlated_lognormal_rejects_non_finite_base() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        let params = lognormal_params();

        let mut cache: IndexMap<NodeId, IndexMap<PeriodId, f64>> = IndexMap::new();
        cache
            .entry(NodeId::new("peer"))
            .or_default()
            .insert(periods[0], 0.5);

        let err = monte_carlo_correlated_series(CorrelatedMonteCarloSeries {
            method: ForecastMethod::LogNormal,
            params: &params,
            base_value: f64::NAN,
            forecast_periods: &periods,
            seed_offset: 1,
            node_id: "node",
            peer_id: "peer",
            rho: 0.5,
            mc_z_cache: &cache,
        })
        .expect_err("correlated path must reject a non-finite base");
        assert!(
            err.to_string().contains("finite"),
            "error should flag the non-finite base: {err}"
        );
    }

    /// Z-score recording must apply the same guard: it inverts the generating
    /// recurrence, so an invalid base means the recorded shocks are garbage
    /// that would silently propagate into every correlated peer.
    #[test]
    fn z_score_recording_rejects_invalid_lognormal_base() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        let params = lognormal_params();
        let mut values = IndexMap::new();
        values.insert(periods[0], 100.0);
        let mut cache: IndexMap<NodeId, IndexMap<PeriodId, f64>> = IndexMap::new();

        let err = record_independent_z_scores_for_mc(
            ForecastMethod::LogNormal,
            &params,
            &periods,
            &values,
            -5.0,
            &NodeId::new("node"),
            &mut cache,
        )
        .expect_err("z-score recording must reject a negative LogNormal base");
        assert!(
            err.to_string().contains("negative") || err.to_string().contains("non-negative"),
            "error should flag the negative base: {err}"
        );
    }

    fn mean_reverting_params() -> IndexMap<String, serde_json::Value> {
        let mut params = IndexMap::new();
        params.insert("long_run_mean".to_string(), serde_json::json!(0.05));
        params.insert("reversion_speed".to_string(), serde_json::json!(0.25));
        params.insert("std_dev".to_string(), serde_json::json!(0.01));
        params.insert("seed".to_string(), serde_json::json!(42));
        params
    }

    #[test]
    fn mean_reverting_is_deterministic_per_seed() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
        ];
        let params = mean_reverting_params();
        let a = mean_reverting_forecast(0.10, &periods, &params).expect("forecast");
        let b = mean_reverting_forecast(0.10, &periods, &params).expect("forecast");
        assert_eq!(a, b);

        let mut other_seed = params;
        other_seed.insert("seed".to_string(), serde_json::json!(43));
        let c = mean_reverting_forecast(0.10, &periods, &other_seed).expect("forecast");
        assert_ne!(a[&periods[0]], c[&periods[0]]);
    }

    /// With `std_dev = 0` the AR(1) recurrence is deterministic geometric
    /// decay of the gap: `gap_t = (1 - κ)^t · gap_0`.
    #[test]
    fn mean_reverting_zero_vol_decays_gap_geometrically() {
        let periods: Vec<PeriodId> = (1..=4)
            .map(|q| PeriodId::quarter(2025, q).expect("valid period fixture"))
            .collect();
        let mut params = mean_reverting_params();
        params.insert("std_dev".to_string(), serde_json::json!(0.0));

        let results = mean_reverting_forecast(0.10, &periods, &params).expect("forecast");
        let theta = 0.05;
        let kappa: f64 = 0.25;
        for (i, pid) in periods.iter().enumerate() {
            let expected = theta + (0.10 - theta) * (1.0 - kappa).powi(i as i32 + 1);
            assert!(
                (results[pid] - expected).abs() < 1e-12,
                "period {i}: expected {expected}, got {}",
                results[pid]
            );
        }
    }

    #[test]
    fn mean_reverting_rejects_out_of_range_reversion_speed() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        for speed in [0.0, -0.5, 1.5, f64::NAN] {
            let mut params = mean_reverting_params();
            params.insert("reversion_speed".to_string(), serde_json::json!(speed));
            let err = mean_reverting_forecast(0.10, &periods, &params)
                .expect_err("out-of-range reversion_speed");
            assert!(err.to_string().contains("reversion_speed"), "{err}");
        }
    }

    #[test]
    fn mean_reverting_rejects_missing_parameters_and_bad_base() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        for missing in ["long_run_mean", "reversion_speed", "std_dev", "seed"] {
            let mut params = mean_reverting_params();
            params.shift_remove(missing);
            assert!(
                mean_reverting_forecast(0.10, &periods, &params).is_err(),
                "missing '{missing}' must be rejected"
            );
        }
        assert!(mean_reverting_forecast(f64::NAN, &periods, &mean_reverting_params()).is_err());
    }

    /// The Z-score recorder must invert the AR(1) recurrence exactly: the
    /// recorded shocks must reproduce the raw normals the generator drew.
    #[test]
    fn mean_reverting_z_recording_inverts_the_generator() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
        ];
        let params = mean_reverting_params();
        let node = NodeId::new("nim");

        let values = mean_reverting_forecast_with_stream(0.10, &periods, &params, Some(5))
            .expect("forecast");
        let mut cache: IndexMap<NodeId, IndexMap<PeriodId, f64>> = IndexMap::new();
        record_independent_z_scores_for_mc(
            ForecastMethod::MeanReverting,
            &params,
            &periods,
            &values,
            0.10,
            &node,
            &mut cache,
        )
        .expect("record z-scores");

        let mut rng = build_rng(42, Some(5));
        for pid in &periods {
            let expected_z = rng.normal(0.0, 1.0);
            let recorded_z = cache[&node][pid];
            assert!(
                (recorded_z - expected_z).abs() < 1e-12,
                "recorded z {recorded_z} must invert the generator's draw {expected_z}"
            );
        }
    }

    /// With ρ = ±1 the correlated series' shocks must equal (mirror) the
    /// peer's Z-scores exactly, applied through the AR(1) recurrence.
    #[test]
    fn mean_reverting_correlated_series_mixes_peer_shocks() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
        ];
        let peer_z = [0.7, -1.2];
        let mut cache: IndexMap<NodeId, IndexMap<PeriodId, f64>> = IndexMap::new();
        let entry = cache.entry(NodeId::new("peer")).or_default();
        for (pid, z) in periods.iter().zip(peer_z) {
            entry.insert(*pid, z);
        }

        for rho in [1.0, -1.0] {
            let mut params = mean_reverting_params();
            params.insert("correlation_with".to_string(), serde_json::json!("peer"));
            params.insert("correlation".to_string(), serde_json::json!(rho));

            let (values, z_out) = monte_carlo_correlated_series(CorrelatedMonteCarloSeries {
                method: ForecastMethod::MeanReverting,
                params: &params,
                base_value: 0.10,
                forecast_periods: &periods,
                seed_offset: 1,
                node_id: "node",
                peer_id: "peer",
                rho,
                mc_z_cache: &cache,
            })
            .expect("correlated series");

            let theta = 0.05;
            let kappa = 0.25;
            let sigma = 0.01;
            let mut prev = 0.10;
            for (pid, z_peer) in periods.iter().zip(peer_z) {
                let z = rho * z_peer;
                assert!((z_out[pid] - z).abs() < 1e-12);
                let expected = prev + kappa * (theta - prev) + sigma * z;
                assert!(
                    (values[pid] - expected).abs() < 1e-12,
                    "rho = {rho}: expected {expected}, got {}",
                    values[pid]
                );
                prev = expected;
            }
        }
    }

    fn bootstrap_params() -> IndexMap<String, serde_json::Value> {
        let mut params = IndexMap::new();
        params.insert(
            "historical".to_string(),
            serde_json::json!([100.0, 105.0, 99.75, 109.725]),
        );
        params.insert("seed".to_string(), serde_json::json!(42));
        params
    }

    #[test]
    fn bootstrap_is_deterministic_per_seed() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
        ];
        let params = bootstrap_params();
        let a = bootstrap_forecast(100.0, &periods, &params).expect("forecast");
        let b = bootstrap_forecast(100.0, &periods, &params).expect("forecast");
        assert_eq!(a, b);

        let mut other_seed = params;
        other_seed.insert("seed".to_string(), serde_json::json!(43));
        let c = bootstrap_forecast(100.0, &periods, &other_seed).expect("forecast");
        assert_ne!(a, c);
    }

    /// Growth mode resamples the observed rates {+5%, -5%, +10%}; every step
    /// must compound one of exactly those rates and stay positive.
    #[test]
    fn bootstrap_growth_mode_resamples_observed_rates() {
        let periods: Vec<PeriodId> = (1..=4)
            .map(|q| PeriodId::quarter(2025, q).expect("valid period fixture"))
            .collect();
        let results = bootstrap_forecast(100.0, &periods, &bootstrap_params()).expect("forecast");

        let observed_rates = [0.05, -0.05, 0.10];
        let mut prev = 100.0;
        for pid in &periods {
            let value = results[pid];
            assert!(value > 0.0);
            let rate = value / prev - 1.0;
            assert!(
                observed_rates.iter().any(|r| (rate - r).abs() < 1e-9),
                "step rate {rate} is not one of the observed rates"
            );
            prev = value;
        }
    }

    /// Diff mode resamples level changes {+5, -5.25, +9.975} and works for
    /// sign-crossing series that growth mode must reject.
    #[test]
    fn bootstrap_diff_mode_handles_sign_crossing_history() {
        let periods: Vec<PeriodId> = (1..=4)
            .map(|q| PeriodId::quarter(2025, q).expect("valid period fixture"))
            .collect();
        let mut params = IndexMap::new();
        params.insert(
            "historical".to_string(),
            serde_json::json!([-10.0, 5.0, -2.0, 8.0]),
        );
        params.insert("mode".to_string(), serde_json::json!("diff"));
        params.insert("seed".to_string(), serde_json::json!(7));

        let results = bootstrap_forecast(0.0, &periods, &params).expect("forecast");
        let observed_diffs = [15.0, -7.0, 10.0];
        let mut prev = 0.0;
        for pid in &periods {
            let diff = results[pid] - prev;
            assert!(
                observed_diffs.iter().any(|d| (diff - d).abs() < 1e-9),
                "step diff {diff} is not one of the observed diffs"
            );
            prev = results[pid];
        }
    }

    #[test]
    fn bootstrap_growth_mode_rejects_non_positive_history() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        let mut params = bootstrap_params();
        params.insert(
            "historical".to_string(),
            serde_json::json!([100.0, 0.0, 50.0]),
        );
        let err = bootstrap_forecast(100.0, &periods, &params)
            .expect_err("non-positive history in growth mode");
        assert!(err.to_string().contains("diff"), "{err}");
    }

    #[test]
    fn bootstrap_rejects_short_history_bad_mode_and_missing_seed() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];

        let mut params = bootstrap_params();
        params.insert("historical".to_string(), serde_json::json!([100.0]));
        assert!(bootstrap_forecast(100.0, &periods, &params).is_err());

        let mut params = bootstrap_params();
        params.insert("mode".to_string(), serde_json::json!("block"));
        let err = bootstrap_forecast(100.0, &periods, &params).expect_err("bad mode");
        assert!(err.to_string().contains("mode"), "{err}");

        let mut params = bootstrap_params();
        params.shift_remove("seed");
        assert!(bootstrap_forecast(100.0, &periods, &params).is_err());
    }

    #[test]
    fn test_normal_forecast_deterministic() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
        ];

        let mut params = IndexMap::new();
        params.insert("mean".to_string(), serde_json::json!(100_000.0));
        params.insert("std_dev".to_string(), serde_json::json!(15_000.0));
        params.insert("seed".to_string(), serde_json::json!(42));

        let results1 =
            normal_forecast(0.0, &periods, &params).expect("normal_forecast should succeed");
        let results2 =
            normal_forecast(0.0, &periods, &params).expect("normal_forecast should succeed");

        // Same seed should produce identical results
        assert_eq!(
            results1[&PeriodId::quarter(2025, 1).expect("valid period fixture")],
            results2[&PeriodId::quarter(2025, 1).expect("valid period fixture")]
        );
        assert_eq!(
            results1[&PeriodId::quarter(2025, 2).expect("valid period fixture")],
            results2[&PeriodId::quarter(2025, 2).expect("valid period fixture")]
        );
    }

    #[test]
    fn test_normal_forecast_different_seeds() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];

        let mut params1 = IndexMap::new();
        params1.insert("mean".to_string(), serde_json::json!(100_000.0));
        params1.insert("std_dev".to_string(), serde_json::json!(15_000.0));
        params1.insert("seed".to_string(), serde_json::json!(42));

        let mut params2 = IndexMap::new();
        params2.insert("mean".to_string(), serde_json::json!(100_000.0));
        params2.insert("std_dev".to_string(), serde_json::json!(15_000.0));
        params2.insert("seed".to_string(), serde_json::json!(43));

        let results1 =
            normal_forecast(0.0, &periods, &params1).expect("normal_forecast should succeed");
        let results2 =
            normal_forecast(0.0, &periods, &params2).expect("normal_forecast should succeed");

        // Different seeds should produce different results
        assert_ne!(
            results1[&PeriodId::quarter(2025, 1).expect("valid period fixture")],
            results2[&PeriodId::quarter(2025, 1).expect("valid period fixture")]
        );
    }

    #[test]
    fn test_normal_forecast_missing_parameters() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];

        // Missing mean
        let mut params = IndexMap::new();
        params.insert("std_dev".to_string(), serde_json::json!(15_000.0));
        params.insert("seed".to_string(), serde_json::json!(42));
        assert!(normal_forecast(0.0, &periods, &params).is_err());

        // Missing std_dev
        let mut params = IndexMap::new();
        params.insert("mean".to_string(), serde_json::json!(100_000.0));
        params.insert("seed".to_string(), serde_json::json!(42));
        assert!(normal_forecast(0.0, &periods, &params).is_err());

        // Missing seed
        let mut params = IndexMap::new();
        params.insert("mean".to_string(), serde_json::json!(100_000.0));
        params.insert("std_dev".to_string(), serde_json::json!(15_000.0));
        assert!(normal_forecast(0.0, &periods, &params).is_err());
    }

    #[test]
    fn test_lognormal_forecast_always_positive() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
            PeriodId::quarter(2025, 3).expect("valid period fixture"),
            PeriodId::quarter(2025, 4).expect("valid period fixture"),
        ];

        let mut params = IndexMap::new();
        params.insert("mean".to_string(), serde_json::json!(11.5));
        params.insert("std_dev".to_string(), serde_json::json!(0.15));
        params.insert("seed".to_string(), serde_json::json!(42));

        let results =
            lognormal_forecast(1.0, &periods, &params).expect("lognormal_forecast should succeed");

        // All values should be positive
        for value in results.values() {
            assert!(*value > 0.0);
        }
    }

    #[test]
    fn test_lognormal_forecast_deterministic() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];

        let mut params = IndexMap::new();
        params.insert("mean".to_string(), serde_json::json!(11.5));
        params.insert("std_dev".to_string(), serde_json::json!(0.15));
        params.insert("seed".to_string(), serde_json::json!(42));

        let results1 =
            lognormal_forecast(1.0, &periods, &params).expect("lognormal_forecast should succeed");
        let results2 =
            lognormal_forecast(1.0, &periods, &params).expect("lognormal_forecast should succeed");

        // Same seed should produce identical results
        assert_eq!(
            results1[&PeriodId::quarter(2025, 1).expect("valid period fixture")],
            results2[&PeriodId::quarter(2025, 1).expect("valid period fixture")]
        );
    }

    #[test]
    fn test_lognormal_forecast_clamps_overflow() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];

        let mut params = IndexMap::new();
        params.insert("mean".to_string(), serde_json::json!(1000.0));
        params.insert("std_dev".to_string(), serde_json::json!(0.0));
        params.insert("seed".to_string(), serde_json::json!(42));

        let result = lognormal_forecast(1.0, &periods, &params);
        assert!(
            result.is_ok(),
            "lognormal with large mean should clamp, not fail"
        );
        let values = result.expect("test already asserted Ok");
        for v in values.values() {
            assert!(v.is_finite(), "clamped output must be finite");
        }
    }

    /// Normal forecast must never produce NaN or non-finite values — exercises
    /// the Box-Muller guard against ln(0) across many seeds.
    #[test]
    fn test_normal_forecast_no_nan() {
        let periods: Vec<_> = (0..100)
            .map(|i| {
                PeriodId::quarter(2025 + i / 4, ((i % 4) as u8) + 1).expect("valid period fixture")
            })
            .collect();

        for seed in 0..1000 {
            let mut params = IndexMap::new();
            params.insert("mean".to_string(), serde_json::json!(100.0));
            params.insert("std_dev".to_string(), serde_json::json!(15.0));
            params.insert("seed".to_string(), serde_json::json!(seed));

            let result =
                normal_forecast(0.0, &periods, &params).expect("normal_forecast should succeed");
            for value in result.values() {
                assert!(!value.is_nan(), "NaN produced with seed {}", seed);
                assert!(
                    value.is_finite(),
                    "Non-finite value produced with seed {}",
                    seed
                );
            }
        }
    }

    /// Lognormal with std_dev=0.0 is a deterministic geometric path whose
    /// per-period multiplier is `exp(mean)`.
    #[test]
    fn test_lognormal_zero_stddev_degenerate() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
            PeriodId::quarter(2025, 3).expect("valid period fixture"),
        ];

        let mut params = IndexMap::new();
        params.insert("mean".to_string(), serde_json::json!(11.5));
        params.insert("std_dev".to_string(), serde_json::json!(0.0));
        params.insert("seed".to_string(), serde_json::json!(42));

        let values =
            lognormal_forecast(1.0, &periods, &params).expect("lognormal std_dev=0 should succeed");
        for (index, value) in values.values().enumerate() {
            let expected = (11.5_f64 * (index + 1) as f64).exp();
            assert!(
                (*value - expected).abs() < 1e-10 * expected,
                "Expected {}, got {}",
                expected,
                value
            );
        }
    }
}

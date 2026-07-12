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
pub(super) fn normal_forecast(
    base_value: f64,
    forecast_periods: &[PeriodId],
    params: &IndexMap<String, serde_json::Value>,
) -> Result<IndexMap<PeriodId, f64>> {
    normal_forecast_with_stream(base_value, forecast_periods, params, None)
}

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

/// Log-normal distribution forecast (deterministic with seed).
///
/// Produces a geometric Brownian motion path starting from `base_value`:
/// `value[t] = value[t-1] * exp(N(mean - 0.5*std_dev², std_dev))`.
///
/// The Itô correction (`-0.5 * σ²`) ensures the expected value of the
/// multiplicative increment is `exp(mean)`, matching the drift convention
/// in Black–Scholes and standard GBM literature.
///
/// When `base_value` is zero, falls back to i.i.d. `exp(N(mean, std_dev))`
/// draws (no path dependence since multiplication by zero would collapse
/// the path).
///
/// # Arguments
///
/// * `base_value` - Starting level for the geometric random walk
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
/// Returns an error if the parameter map is incomplete, if `std_dev` is
/// negative, or if exponentiation produces a non-finite value.
///
/// # References
///
/// - Monte Carlo simulation practice: `docs/REFERENCES.md#glasserman-2004-monte-carlo`
/// - Numerical sampling techniques: `docs/REFERENCES.md#press-numerical-recipes`
pub(super) fn lognormal_forecast(
    base_value: f64,
    forecast_periods: &[PeriodId],
    params: &IndexMap<String, serde_json::Value>,
) -> Result<IndexMap<PeriodId, f64>> {
    lognormal_forecast_with_stream(base_value, forecast_periods, params, None)
}

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

    // A LogNormal level is non-negative by construction; a negative base would
    // produce strictly negative "LogNormal" paths (mirror-lognormal), which
    // contradicts the documented positivity and silently flips percentile
    // meaning. Reject it rather than emit meaningless paths.
    if base_value < 0.0 {
        return Err(crate::error::Error::forecast(format!(
            "LogNormal forecast requires a non-negative base value; got {base_value}. \
             Use a Normal forecast for series that can go negative."
        )));
    }

    let mut rng = build_rng(p.seed, stream_id);
    let mut results = IndexMap::new();
    let mut prev = base_value;
    // Anchor the geometric walk at any strictly-positive base; a base of exactly
    // zero uses the documented i.i.d. `exp(N(mean, std_dev))` fallback. Keying on
    // `> 0.0` (rather than `abs() > f64::EPSILON`) removes the discontinuity where
    // a rounding residue like 1e-17 vs 1e-15 produced forecasts orders of
    // magnitude apart.
    let use_path = base_value > 0.0;

    const EXP_CLAMP: f64 = 709.0;

    for period_id in forecast_periods {
        let z = rng.normal(0.0, 1.0);
        let value = if use_path {
            let log_return = (p.mean - 0.5 * p.std_dev * p.std_dev) + p.std_dev * z;
            if log_return.abs() > EXP_CLAMP {
                tracing::warn!(
                    mean = p.mean,
                    std_dev = p.std_dev,
                    "LogNormal exponent clamped to avoid overflow"
                );
            }
            prev * log_return.clamp(-EXP_CLAMP, EXP_CLAMP).exp()
        } else {
            let normal_value = p.mean + p.std_dev * z;
            if normal_value.abs() > EXP_CLAMP {
                tracing::warn!(
                    mean = p.mean,
                    std_dev = p.std_dev,
                    "LogNormal exponent clamped to avoid overflow"
                );
            }
            normal_value.clamp(-EXP_CLAMP, EXP_CLAMP).exp()
        };
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

/// Store standard-normal Z scores for independent Monte Carlo forecasts so peers can
/// correlate in a later [`crate::evaluator::forecast_eval::evaluate_forecast`] pass.
///
/// Recorded Z values are the **shock** Z that was applied at each period, not a
/// level normalization. They must match the recurrences in
/// [`normal_forecast_with_stream`] and [`lognormal_forecast_with_stream`]:
///
/// - Normal (random walk): `v_t = v_{t-1} + mean + std_dev * z_t`
///   ⇒ `z_t = (v_t - v_{t-1} - mean) / std_dev`.
/// - LogNormal with path (`|base_value| > EPSILON`, GBM):
///   `v_t = v_{t-1} * exp((mean - 0.5*std_dev²) + std_dev * z_t)`
///   ⇒ `z_t = (ln(v_t / v_{t-1}) - (mean - 0.5*std_dev²)) / std_dev`.
/// - LogNormal zero-base fallback (i.i.d. `exp(N(mean, std_dev))`):
///   ⇒ `z_t = (ln(v_t) - mean) / std_dev`.
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
            let entry = mc_z_cache.entry(node_id.clone()).or_default();
            // Must match `lognormal_forecast_with_stream`'s regime switch so the
            // recorded Z-scores invert the same recurrence.
            let use_path = base_value > 0.0;
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
                } else if use_path {
                    // GBM: log-return space with Itô correction
                    let ln_ratio = (v / prev).ln();
                    (ln_ratio - (p.mean - 0.5 * p.std_dev * p.std_dev)) / p.std_dev
                } else {
                    // Zero-base fallback: i.i.d. exp(N(mean, std_dev))
                    ((v).ln() - p.mean) / p.std_dev
                };
                entry.insert(*pid, z);
                if use_path {
                    prev = v;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Inputs for [`monte_carlo_correlated_series`].
pub(crate) struct CorrelatedMonteCarloSeries<'a> {
    /// Forecast method (Normal or LogNormal only).
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

/// Correlated Normal / LogNormal series for Monte Carlo.
///
/// The shock `z_t = ρ·z_peer + sqrt(1-ρ²)·z_indep` is applied in the **same
/// recurrence** as the independent forecast paths
/// ([`normal_forecast_with_stream`], [`lognormal_forecast_with_stream`]) so
/// correlated and uncorrelated outputs live on the same process:
///
/// - Normal: additive random walk `v_t = v_{t-1} + mean + std_dev * z_t`
///   anchored at `base_value`.
/// - LogNormal (GBM) when `|base_value| > EPSILON`:
///   `v_t = v_{t-1} * exp((mean - 0.5*std_dev²) + std_dev * z_t)`.
/// - LogNormal zero-base fallback: i.i.d. `exp(mean + std_dev * z_t)`.
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

    if !base_value.is_finite() {
        return Err(Error::forecast(format!(
            "Monte Carlo correlated forecast for '{node_id}' requires a finite base_value, \
             got {base_value}"
        )));
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

    let p = match method {
        ForecastMethod::Normal => extract_distribution_params(params, "Normal")?,
        ForecastMethod::LogNormal => extract_distribution_params(params, "LogNormal")?,
        _ => {
            return Err(Error::forecast(
                "Monte Carlo correlation is only supported for Normal and LogNormal forecasts"
                    .to_string(),
            ));
        }
    };

    let mut rng = Pcg64Rng::new_with_stream(p.seed ^ stable_hash_u64(node_id), seed_offset);
    let mut values = IndexMap::new();
    let mut z_out = IndexMap::new();
    let mut prev = base_value;
    // Match the non-correlated LogNormal regime switch (`base > 0.0`, not an
    // absolute-epsilon threshold) so both paths anchor the geometric walk
    // identically.
    let use_path = matches!(method, ForecastMethod::LogNormal) && base_value > 0.0;

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

        let value = match method {
            ForecastMethod::Normal => prev + p.mean + p.std_dev * z,
            ForecastMethod::LogNormal if use_path => {
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
            ForecastMethod::LogNormal => {
                // base_value ≈ 0 fallback: i.i.d. exp(N(mean, std_dev))
                let normal_value = p.mean + p.std_dev * z;
                if normal_value.abs() > EXP_CLAMP {
                    tracing::warn!(
                        mean = p.mean,
                        std_dev = p.std_dev,
                        "LogNormal correlated exponent clamped to avoid overflow"
                    );
                }
                normal_value.clamp(-EXP_CLAMP, EXP_CLAMP).exp()
            }
            _ => {
                return Err(Error::forecast(
                    "Monte Carlo correlation is only supported for Normal and LogNormal forecasts"
                        .to_string(),
                ));
            }
        };

        if !value.is_finite() {
            return Err(Error::forecast(format!(
                "{:?} correlated forecast produced a non-finite value at period {:?}",
                method, period_id
            )));
        }
        values.insert(*period_id, value);
        if use_path || matches!(method, ForecastMethod::Normal) {
            prev = value;
        }
    }

    Ok((values, z_out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::PeriodId;

    #[test]
    fn test_parse_seed_accepts_integer_like_json_float() {
        let v = serde_json::json!(42.0);
        assert_eq!(parse_seed_json(&v), Some(42));
    }

    #[test]
    fn test_normal_forecast_deterministic() {
        let periods = vec![PeriodId::quarter(2025, 1), PeriodId::quarter(2025, 2)];

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
            results1[&PeriodId::quarter(2025, 1)],
            results2[&PeriodId::quarter(2025, 1)]
        );
        assert_eq!(
            results1[&PeriodId::quarter(2025, 2)],
            results2[&PeriodId::quarter(2025, 2)]
        );
    }

    #[test]
    fn test_normal_forecast_different_seeds() {
        let periods = vec![PeriodId::quarter(2025, 1)];

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
            results1[&PeriodId::quarter(2025, 1)],
            results2[&PeriodId::quarter(2025, 1)]
        );
    }

    #[test]
    fn test_normal_forecast_missing_parameters() {
        let periods = vec![PeriodId::quarter(2025, 1)];

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
            PeriodId::quarter(2025, 1),
            PeriodId::quarter(2025, 2),
            PeriodId::quarter(2025, 3),
            PeriodId::quarter(2025, 4),
        ];

        let mut params = IndexMap::new();
        params.insert("mean".to_string(), serde_json::json!(11.5));
        params.insert("std_dev".to_string(), serde_json::json!(0.15));
        params.insert("seed".to_string(), serde_json::json!(42));

        let results =
            lognormal_forecast(0.0, &periods, &params).expect("lognormal_forecast should succeed");

        // All values should be positive
        for value in results.values() {
            assert!(*value > 0.0);
        }
    }

    #[test]
    fn test_lognormal_forecast_deterministic() {
        let periods = vec![PeriodId::quarter(2025, 1)];

        let mut params = IndexMap::new();
        params.insert("mean".to_string(), serde_json::json!(11.5));
        params.insert("std_dev".to_string(), serde_json::json!(0.15));
        params.insert("seed".to_string(), serde_json::json!(42));

        let results1 =
            lognormal_forecast(0.0, &periods, &params).expect("lognormal_forecast should succeed");
        let results2 =
            lognormal_forecast(0.0, &periods, &params).expect("lognormal_forecast should succeed");

        // Same seed should produce identical results
        assert_eq!(
            results1[&PeriodId::quarter(2025, 1)],
            results2[&PeriodId::quarter(2025, 1)]
        );
    }

    #[test]
    fn test_lognormal_forecast_clamps_overflow() {
        let periods = vec![PeriodId::quarter(2025, 1)];

        let mut params = IndexMap::new();
        params.insert("mean".to_string(), serde_json::json!(1000.0));
        params.insert("std_dev".to_string(), serde_json::json!(0.0));
        params.insert("seed".to_string(), serde_json::json!(42));

        let result = lognormal_forecast(0.0, &periods, &params);
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
            .map(|i| PeriodId::quarter(2025 + i / 4, ((i % 4) as u8) + 1))
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

    /// Lognormal with std_dev=0.0 is a degenerate distribution — every draw
    /// should return exp(mean) exactly.
    #[test]
    fn test_lognormal_zero_stddev_degenerate() {
        let periods = vec![
            PeriodId::quarter(2025, 1),
            PeriodId::quarter(2025, 2),
            PeriodId::quarter(2025, 3),
        ];

        let mut params = IndexMap::new();
        params.insert("mean".to_string(), serde_json::json!(11.5));
        params.insert("std_dev".to_string(), serde_json::json!(0.0));
        params.insert("seed".to_string(), serde_json::json!(42));

        let values =
            lognormal_forecast(0.0, &periods, &params).expect("lognormal std_dev=0 should succeed");
        let expected = (11.5_f64).exp();
        for value in values.values() {
            assert!(
                (*value - expected).abs() < 1e-10,
                "Expected {}, got {}",
                expected,
                value
            );
        }
    }
}

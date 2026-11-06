//! Statistical forecast helpers that produce deterministic sequences.
//!
//! Each algorithm consumes a pre-seeded pseudo-random number generator so that
//! repeated calls with identical parameters return the same series. This makes
//! them suitable for scenario analysis where reproducibility matters.
//!
//! Uses [`Pcg64Rng`] for production-quality random number generation.

use crate::error::{Error, Result};
use crate::types::{
    BootstrapParams, DistributionParams, ForecastBootstrapMode, ForecastParameters,
    MeanRevertingParams, NodeId,
};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::math::random::{Pcg64Rng, RandomNumberGenerator};
use indexmap::IndexMap;

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

/// Optional Monte Carlo correlation pair: `(peer_node_id, rho)` in `[-1, 1]`.
///
/// When both `correlation_with` and `correlation` are set on a Normal,
/// LogNormal or MeanReverting forecast, Monte Carlo evaluation samples shocks
/// correlated with the peer node's standard normal shocks (same forecast
/// period). The peer node must be evaluated earlier in the dependency order so
/// its Z-scores are available. Other methods carry no correlation.
pub(crate) fn correlation(parameters: &ForecastParameters) -> Result<Option<(&str, f64)>> {
    let (with, rho) = match parameters {
        ForecastParameters::Normal(p) | ForecastParameters::LogNormal(p) => {
            (p.correlation_with.as_deref(), p.correlation)
        }
        ForecastParameters::MeanReverting(p) => (p.correlation_with.as_deref(), p.correlation),
        _ => return Ok(None),
    };

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

/// Check that a distribution's mean and standard deviation are usable.
fn validate_distribution(p: &DistributionParams, method_name: &str) -> Result<()> {
    if p.std_dev < 0.0 {
        return Err(Error::forecast(format!(
            "Standard deviation must be non-negative, got {}",
            p.std_dev
        )));
    }
    if !p.mean.is_finite() || !p.std_dev.is_finite() {
        return Err(Error::forecast(format!(
            "{method_name} forecast requires finite mean and std_dev"
        )));
    }
    Ok(())
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
/// * `params` - Per-period drift `mean`, non-negative per-period volatility
///   `std_dev`, and the `seed` of the deterministic sampler
///
/// * `stream_id` - Optional independent RNG stream for a Monte Carlo path;
///   `None` uses the configured seed without selecting a path stream.
///
/// # Returns
///
/// Returns the per-period levels and the exact sampled standard-normal shocks.
/// A zero `std_dev` retains zero shocks while still consuming the same draws.
///
/// # Errors
///
/// Returns an error if `mean` or `std_dev` is non-finite, if `std_dev` is
/// negative, or if simulation produces a non-finite value.
///
/// # References
///
/// - Monte Carlo simulation practice: `docs/REFERENCES.md#glasserman-2004-monte-carlo`
/// - Numerical sampling techniques: `docs/REFERENCES.md#press-numerical-recipes`
pub(crate) fn normal_forecast_with_stream(
    base_value: f64,
    forecast_periods: &[PeriodId],
    p: &DistributionParams,
    stream_id: Option<u64>,
) -> Result<(IndexMap<PeriodId, f64>, IndexMap<PeriodId, f64>)> {
    validate_distribution(p, "Normal")?;

    let mut rng = build_rng(p.seed, stream_id);
    let mut results = IndexMap::new();
    let mut shocks = IndexMap::new();
    let mut prev = base_value;

    for period_id in forecast_periods {
        let z = rng.normal(0.0, 1.0);
        shocks.insert(*period_id, if p.std_dev == 0.0 { 0.0 } else { z });
        let value = normal_step(prev, p, z);
        if !value.is_finite() {
            return Err(Error::forecast(format!(
                "Normal forecast produced a non-finite value at period {:?}",
                period_id
            )));
        }
        results.insert(*period_id, value);
        prev = value;
    }

    Ok((results, shocks))
}

fn normal_step(prev: f64, params: &DistributionParams, z: f64) -> f64 {
    prev + params.mean + params.std_dev * z
}

fn lognormal_step(prev: f64, params: &DistributionParams, z: f64) -> f64 {
    const EXP_CLAMP: f64 = 709.0;
    let log_return = (params.mean - 0.5 * params.std_dev * params.std_dev) + params.std_dev * z;
    if log_return.abs() > EXP_CLAMP {
        tracing::warn!(
            mean = params.mean,
            std_dev = params.std_dev,
            "LogNormal exponent clamped to avoid overflow"
        );
    }
    prev * log_return.clamp(-EXP_CLAMP, EXP_CLAMP).exp()
}

fn mean_reverting_step(prev: f64, params: &MeanRevertingParams, z: f64) -> f64 {
    prev + params.reversion_speed * (params.long_run_mean - prev) + params.std_dev * z
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
/// * `params` - Continuously compounded expected-growth drift `mean` (the
///   expected log return is `mean - 0.5 * std_dev²`), non-negative log-return
///   volatility `std_dev`, and the `seed` of the deterministic sampler
///
/// * `stream_id` - Optional independent RNG stream for a Monte Carlo path;
///   `None` uses the configured seed without selecting a path stream.
///
/// # Returns
///
/// Returns the per-period positive levels and exact sampled standard-normal shocks.
/// A zero `std_dev` retains zero shocks while still consuming the same draws.
///
/// # Errors
///
/// Returns an error if `mean` or `std_dev` is non-finite, `base_value` is not
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
    p: &DistributionParams,
    stream_id: Option<u64>,
) -> Result<(IndexMap<PeriodId, f64>, IndexMap<PeriodId, f64>)> {
    validate_distribution(p, "LogNormal")?;

    if p.std_dev == 0.0 {
        tracing::warn!(
            "LogNormal forecast with std_dev=0.0 produces degenerate distribution (all values identical)"
        );
    }

    validate_lognormal_base(base_value, "LogNormal forecast")?;

    let mut rng = build_rng(p.seed, stream_id);
    let mut results = IndexMap::new();
    let mut shocks = IndexMap::new();
    let mut prev = base_value;

    for period_id in forecast_periods {
        let z = rng.normal(0.0, 1.0);
        shocks.insert(*period_id, if p.std_dev == 0.0 { 0.0 } else { z });
        let value = lognormal_step(prev, p, z);
        if !value.is_finite() {
            return Err(Error::forecast(format!(
                "LogNormal forecast produced a non-finite value at period {:?}",
                period_id
            )));
        }
        results.insert(*period_id, value);
        prev = value;
    }

    Ok((results, shocks))
}

/// Check that mean-reverting AR(1) parameters are in range.
fn validate_mean_reverting(p: &MeanRevertingParams) -> Result<()> {
    let MeanRevertingParams {
        long_run_mean,
        reversion_speed,
        std_dev,
        ..
    } = *p;
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
    Ok(())
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
/// * `params` - `long_run_mean` (level the series reverts toward, node
///   units), `reversion_speed` (fraction of the gap closed per period, in
///   `(0, 1]`), `std_dev` (per-period additive shock volatility,
///   non-negative), and the `seed` of the deterministic sampler
///
/// * `stream_id` - Optional independent RNG stream for a Monte Carlo path;
///   `None` uses the configured seed without selecting a path stream.
///
/// # Returns
///
/// Returns the per-period levels and the exact sampled standard-normal shocks.
/// A zero `std_dev` retains zero shocks while still consuming the same draws.
///
/// # Errors
///
/// Returns an error if a parameter is out of range, if
/// `base_value` is non-finite, or if simulation produces a non-finite value.
///
/// # References
///
/// - Monte Carlo simulation practice: `docs/REFERENCES.md#glasserman-2004-monte-carlo`
pub(crate) fn mean_reverting_forecast_with_stream(
    base_value: f64,
    forecast_periods: &[PeriodId],
    p: &MeanRevertingParams,
    stream_id: Option<u64>,
) -> Result<(IndexMap<PeriodId, f64>, IndexMap<PeriodId, f64>)> {
    validate_mean_reverting(p)?;

    if !base_value.is_finite() {
        return Err(Error::forecast(format!(
            "MeanReverting forecast requires a finite base_value, got {base_value}"
        )));
    }

    let mut rng = build_rng(p.seed, stream_id);
    let mut results = IndexMap::new();
    let mut shocks = IndexMap::new();
    let mut prev = base_value;

    for period_id in forecast_periods {
        let z = rng.normal(0.0, 1.0);
        shocks.insert(*period_id, if p.std_dev == 0.0 { 0.0 } else { z });
        let value = mean_reverting_step(prev, p, z);
        if !value.is_finite() {
            return Err(Error::forecast(format!(
                "MeanReverting forecast produced a non-finite value at period {:?}",
                period_id
            )));
        }
        results.insert(*period_id, value);
        prev = value;
    }

    Ok((results, shocks))
}

/// Per-step increments of a bootstrap's history: growth rates or level diffs.
fn bootstrap_increments(p: &BootstrapParams) -> Result<Vec<f64>> {
    let hist = &p.historical;
    if hist.len() < 2 {
        return Err(Error::forecast(format!(
            "Bootstrap forecast needs at least 2 historical values to derive increments, \
             got {}. Provide more history in the 'historical' parameter.",
            hist.len()
        )));
    }
    if let Some(idx) = hist.iter().position(|value| !value.is_finite()) {
        return Err(Error::forecast(format!(
            "Bootstrap historical value at index {idx} must be a finite number, got {}",
            hist[idx]
        )));
    }

    Ok(match p.mode {
        ForecastBootstrapMode::Growth => {
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
        ForecastBootstrapMode::Diff => hist.windows(2).map(|w| w[1] - w[0]).collect(),
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
/// * `params` - `historical` (at least 2 finite numbers in the node's own
///   units, oldest first), the resampling `mode`, and the `seed` of the
///   deterministic resampler
///
/// # Returns
///
/// Returns one simulated scalar per forecast period forming a path.
///
/// # Errors
///
/// Returns an error if the history is too short or non-finite, if growth mode is requested with
/// non-positive history, or if the path produces a non-finite value.
pub(crate) fn bootstrap_forecast_with_stream(
    base_value: f64,
    forecast_periods: &[PeriodId],
    p: &BootstrapParams,
    stream_id: Option<u64>,
) -> Result<IndexMap<PeriodId, f64>> {
    let increments = bootstrap_increments(p)?;

    if !base_value.is_finite() {
        return Err(Error::forecast(format!(
            "Bootstrap forecast requires a finite base_value, got {base_value}"
        )));
    }

    let mut rng = build_rng(p.seed, stream_id);
    let mut results = IndexMap::new();
    let mut prev = base_value;
    let n = increments.len();

    for period_id in forecast_periods {
        // Uniform in [0, 1) scaled to an index; the min() guards the
        // (unreachable in exact arithmetic) u == 1.0 edge.
        let idx = ((rng.uniform() * n as f64) as usize).min(n - 1);
        let value = match p.mode {
            ForecastBootstrapMode::Growth => prev * (1.0 + increments[idx]),
            ForecastBootstrapMode::Diff => prev + increments[idx],
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

/// Inputs for [`monte_carlo_correlated_series`].
pub(crate) struct CorrelatedMonteCarloSeries<'a> {
    /// Forecast parameters (Normal, LogNormal, or MeanReverting only).
    pub parameters: &'a ForecastParameters,
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
/// Uses the sampled shocks returned by the independent Gaussian kernels,
/// preserving correlation even when path values round to a constant.
pub(crate) fn monte_carlo_correlated_series(
    input: CorrelatedMonteCarloSeries<'_>,
) -> Result<(IndexMap<PeriodId, f64>, IndexMap<PeriodId, f64>)> {
    let CorrelatedMonteCarloSeries {
        parameters,
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
    if matches!(parameters, ForecastParameters::LogNormal(_)) {
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
    enum Kernel<'p> {
        Normal(&'p DistributionParams),
        LogNormal(&'p DistributionParams),
        MeanReverting(&'p MeanRevertingParams),
    }

    let (kernel, seed, sigma, method_name) = match parameters {
        ForecastParameters::Normal(p) => {
            validate_distribution(p, "Normal")?;
            (Kernel::Normal(p), p.seed, p.std_dev, "Normal")
        }
        ForecastParameters::LogNormal(p) => {
            validate_distribution(p, "LogNormal")?;
            (Kernel::LogNormal(p), p.seed, p.std_dev, "LogNormal")
        }
        ForecastParameters::MeanReverting(p) => {
            validate_mean_reverting(p)?;
            (Kernel::MeanReverting(p), p.seed, p.std_dev, "MeanReverting")
        }
        _ => {
            return Err(Error::forecast(
                "Monte Carlo correlation is only supported for Normal, LogNormal, and \
                 MeanReverting forecasts"
                    .to_string(),
            ));
        }
    };

    let mut rng = Pcg64Rng::new_with_stream(seed ^ stable_hash_u64(node_id), seed_offset);
    let mut values = IndexMap::new();
    let mut z_out = IndexMap::new();
    let mut prev = base_value;

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
        z_out.insert(*period_id, if sigma == 0.0 { 0.0 } else { z });

        let value = match kernel {
            Kernel::Normal(p) => normal_step(prev, p, z),
            Kernel::LogNormal(p) => lognormal_step(prev, p, z),
            Kernel::MeanReverting(p) => mean_reverting_step(prev, p, z),
        };

        if !value.is_finite() {
            return Err(Error::forecast(format!(
                "{method_name} correlated forecast produced a non-finite value at period {period_id:?}"
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
        params: &DistributionParams,
    ) -> Result<IndexMap<PeriodId, f64>> {
        normal_forecast_with_stream(base_value, periods, params, None).map(|(values, _)| values)
    }

    fn lognormal_forecast(
        base_value: f64,
        periods: &[PeriodId],
        params: &DistributionParams,
    ) -> Result<IndexMap<PeriodId, f64>> {
        lognormal_forecast_with_stream(base_value, periods, params, None).map(|(values, _)| values)
    }

    fn mean_reverting_forecast(
        base_value: f64,
        periods: &[PeriodId],
        params: &MeanRevertingParams,
    ) -> Result<IndexMap<PeriodId, f64>> {
        mean_reverting_forecast_with_stream(base_value, periods, params, None)
            .map(|(values, _)| values)
    }

    fn bootstrap_forecast(
        base_value: f64,
        periods: &[PeriodId],
        params: &BootstrapParams,
    ) -> Result<IndexMap<PeriodId, f64>> {
        bootstrap_forecast_with_stream(base_value, periods, params, None)
    }

    fn distribution(mean: f64, std_dev: f64, seed: u64) -> DistributionParams {
        DistributionParams {
            mean,
            std_dev,
            seed,
            correlation_with: None,
            correlation: None,
            min: None,
            max: None,
        }
    }

    fn lognormal_params() -> DistributionParams {
        distribution(0.02, 0.1, 7)
    }

    fn correlated(
        parameters: &ForecastParameters,
        base_value: f64,
        periods: &[PeriodId],
        node_id: &str,
        peer_id: &str,
        cache: &IndexMap<NodeId, IndexMap<PeriodId, f64>>,
    ) -> Result<(IndexMap<PeriodId, f64>, IndexMap<PeriodId, f64>)> {
        monte_carlo_correlated_series(CorrelatedMonteCarloSeries {
            parameters,
            base_value,
            forecast_periods: periods,
            seed_offset: 1,
            node_id,
            peer_id,
            rho: 0.5,
            mc_z_cache: cache,
        })
    }

    fn peer_cache(periods: &[PeriodId], z: f64) -> IndexMap<NodeId, IndexMap<PeriodId, f64>> {
        let mut cache: IndexMap<NodeId, IndexMap<PeriodId, f64>> = IndexMap::new();
        let entry = cache.entry(NodeId::new("peer")).or_default();
        for period in periods {
            entry.insert(*period, z);
        }
        cache
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

    /// A misspelled parameter must fail loudly rather than be ignored: a
    /// rename left half-done, `sigma: 0.4` beside a stale `std_dev: 0.1`,
    /// would otherwise simulate at a quarter of the intended volatility.
    #[test]
    fn statistical_forecast_rejects_unknown_parameter_keys() {
        let mut spec = crate::types::ForecastSpec::log_normal(0.02, 0.1, 7);
        spec.params
            .insert("sigma".to_string(), serde_json::json!(0.4));
        let msg = spec
            .parameters()
            .expect_err("an unknown parameter must be rejected")
            .to_string();
        assert!(
            msg.contains("sigma") && msg.contains("std_dev"),
            "error should name the bad key and list the allowed set: {msg}"
        );
    }

    /// Correlation keys stay legal on statistical methods.
    #[test]
    fn correlation_keys_are_allowed_on_statistical_methods() {
        let mut spec = crate::types::ForecastSpec::log_normal(0.02, 0.1, 7);
        spec.params
            .insert("correlation_with".to_string(), serde_json::json!("peer"));
        spec.params
            .insert("correlation".to_string(), serde_json::json!(0.5));
        let parameters = spec
            .parameters()
            .expect("correlation parameters are legal to configure");
        assert_eq!(
            correlation(&parameters).expect("valid pair"),
            Some(("peer", 0.5))
        );
    }

    #[test]
    fn correlation_requires_both_keys_and_a_valid_rho() {
        let mut half = lognormal_params();
        half.correlation = Some(0.5);
        assert!(correlation(&ForecastParameters::Normal(half)).is_err());

        let mut out_of_range = lognormal_params();
        out_of_range.correlation_with = Some("peer".into());
        out_of_range.correlation = Some(1.5);
        assert!(correlation(&ForecastParameters::Normal(out_of_range)).is_err());
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

        let independent_err = lognormal_forecast(-5.0, &periods, &params)
            .expect_err("independent path must reject a negative base");

        let cache = peer_cache(&periods, 0.5);
        let correlated_err = correlated(
            &ForecastParameters::LogNormal(params),
            -5.0,
            &periods,
            "node",
            "peer",
            &cache,
        )
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
        let cache = peer_cache(&periods, 0.5);
        let err = correlated(
            &ForecastParameters::LogNormal(lognormal_params()),
            f64::NAN,
            &periods,
            "node",
            "peer",
            &cache,
        )
        .expect_err("correlated path must reject a non-finite base");
        assert!(
            err.to_string().contains("finite"),
            "error should flag the non-finite base: {err}"
        );
    }

    #[test]
    fn direct_lognormal_shocks_reject_negative_base() {
        let periods = vec![PeriodId::quarter(2025, 1).unwrap()];
        assert!(lognormal_forecast_with_stream(-5.0, &periods, &lognormal_params(), None).is_err());
    }

    fn mean_reverting_params() -> MeanRevertingParams {
        MeanRevertingParams {
            long_run_mean: 0.05,
            reversion_speed: 0.25,
            std_dev: 0.01,
            seed: 42,
            correlation_with: None,
            correlation: None,
            min: None,
            max: None,
        }
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

        let other_seed = MeanRevertingParams { seed: 43, ..params };
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
        let params = MeanRevertingParams {
            std_dev: 0.0,
            ..mean_reverting_params()
        };

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
            let params = MeanRevertingParams {
                reversion_speed: speed,
                ..mean_reverting_params()
            };
            let err = mean_reverting_forecast(0.10, &periods, &params)
                .expect_err("out-of-range reversion_speed");
            assert!(err.to_string().contains("reversion_speed"), "{err}");
        }
    }

    #[test]
    fn mean_reverting_rejects_missing_parameters_and_bad_base() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        for missing in ["long_run_mean", "reversion_speed", "std_dev", "seed"] {
            let mut spec = crate::types::ForecastSpec::mean_reverting(0.05, 0.25, 0.01, 42);
            spec.params.shift_remove(missing);
            assert!(
                spec.parameters().is_err(),
                "missing '{missing}' must be rejected"
            );
        }
        assert!(mean_reverting_forecast(f64::NAN, &periods, &mean_reverting_params()).is_err());
    }

    /// The returned shocks are exactly the generator draws, without inversion.
    #[test]
    fn mean_reverting_records_the_sampled_shocks() {
        let periods = vec![
            PeriodId::quarter(2025, 1).unwrap(),
            PeriodId::quarter(2025, 2).unwrap(),
        ];
        let (_, shocks) =
            mean_reverting_forecast_with_stream(0.10, &periods, &mean_reverting_params(), Some(5))
                .expect("forecast");
        let mut rng = build_rng(42, Some(5));
        for pid in &periods {
            assert_eq!(shocks[pid].to_bits(), rng.normal(0.0, 1.0).to_bits());
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
            let parameters = ForecastParameters::MeanReverting(MeanRevertingParams {
                correlation_with: Some("peer".into()),
                correlation: Some(rho),
                ..mean_reverting_params()
            });

            let (values, z_out) = monte_carlo_correlated_series(CorrelatedMonteCarloSeries {
                parameters: &parameters,
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

    fn bootstrap_params() -> BootstrapParams {
        BootstrapParams {
            historical: vec![100.0, 105.0, 99.75, 109.725],
            mode: ForecastBootstrapMode::Growth,
            seed: 42,
            min: None,
            max: None,
        }
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

        let other_seed = BootstrapParams { seed: 43, ..params };
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

    /// Diff mode resamples level changes and works for sign-crossing series
    /// that growth mode must reject.
    #[test]
    fn bootstrap_diff_mode_handles_sign_crossing_history() {
        let periods: Vec<PeriodId> = (1..=4)
            .map(|q| PeriodId::quarter(2025, q).expect("valid period fixture"))
            .collect();
        let params = BootstrapParams {
            historical: vec![-10.0, 5.0, -2.0, 8.0],
            mode: ForecastBootstrapMode::Diff,
            seed: 7,
            min: None,
            max: None,
        };

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
        let params = BootstrapParams {
            historical: vec![100.0, 0.0, 50.0],
            ..bootstrap_params()
        };
        let err = bootstrap_forecast(100.0, &periods, &params)
            .expect_err("non-positive history in growth mode");
        assert!(err.to_string().contains("diff"), "{err}");
    }

    #[test]
    fn bootstrap_rejects_short_history_bad_mode_and_missing_seed() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];

        let params = BootstrapParams {
            historical: vec![100.0],
            ..bootstrap_params()
        };
        assert!(bootstrap_forecast(100.0, &periods, &params).is_err());

        let mut spec = crate::types::ForecastSpec::bootstrap(vec![100.0, 105.0], 42);
        spec.params
            .insert("mode".to_string(), serde_json::json!("block"));
        let err = spec.parameters().expect_err("bad mode");
        assert!(err.to_string().contains("mode"), "{err}");

        let mut spec = crate::types::ForecastSpec::bootstrap(vec![100.0, 105.0], 42);
        spec.params.shift_remove("seed");
        assert!(spec.parameters().is_err());
    }

    #[test]
    fn test_normal_forecast_deterministic() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
        ];
        let params = distribution(100_000.0, 15_000.0, 42);

        let results1 =
            normal_forecast(0.0, &periods, &params).expect("normal_forecast should succeed");
        let results2 =
            normal_forecast(0.0, &periods, &params).expect("normal_forecast should succeed");

        assert_eq!(results1, results2);
    }

    #[test]
    fn test_normal_forecast_different_seeds() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];

        let results1 = normal_forecast(0.0, &periods, &distribution(100_000.0, 15_000.0, 42))
            .expect("normal_forecast should succeed");
        let results2 = normal_forecast(0.0, &periods, &distribution(100_000.0, 15_000.0, 43))
            .expect("normal_forecast should succeed");

        assert_ne!(results1[&periods[0]], results2[&periods[0]]);
    }

    #[test]
    fn test_normal_forecast_rejects_invalid_parameters() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        assert!(normal_forecast(0.0, &periods, &distribution(0.0, -1.0, 42)).is_err());
        assert!(normal_forecast(0.0, &periods, &distribution(f64::NAN, 1.0, 42)).is_err());

        for missing in ["mean", "std_dev", "seed"] {
            let mut spec = crate::types::ForecastSpec::normal(100_000.0, 15_000.0, 42);
            spec.params.shift_remove(missing);
            assert!(
                spec.parameters().is_err(),
                "missing '{missing}' must be rejected"
            );
        }
    }

    #[test]
    fn test_lognormal_forecast_always_positive() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
            PeriodId::quarter(2025, 3).expect("valid period fixture"),
            PeriodId::quarter(2025, 4).expect("valid period fixture"),
        ];

        let results = lognormal_forecast(1.0, &periods, &distribution(11.5, 0.15, 42))
            .expect("lognormal_forecast should succeed");

        for value in results.values() {
            assert!(*value > 0.0);
        }
    }

    #[test]
    fn test_lognormal_forecast_deterministic() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        let params = distribution(11.5, 0.15, 42);

        let results1 =
            lognormal_forecast(1.0, &periods, &params).expect("lognormal_forecast should succeed");
        let results2 =
            lognormal_forecast(1.0, &periods, &params).expect("lognormal_forecast should succeed");

        assert_eq!(results1, results2);
    }

    #[test]
    fn test_lognormal_forecast_clamps_overflow() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];

        let values = lognormal_forecast(1.0, &periods, &distribution(1000.0, 0.0, 42))
            .expect("lognormal with large mean should clamp, not fail");
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
            let result = normal_forecast(0.0, &periods, &distribution(100.0, 15.0, seed))
                .expect("normal_forecast should succeed");
            for value in result.values() {
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

        let values = lognormal_forecast(1.0, &periods, &distribution(11.5, 0.0, 42))
            .expect("lognormal std_dev=0 should succeed");
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

    #[test]
    fn gaussian_kernels_retain_exact_draws_when_levels_round_flat() {
        let periods = [
            PeriodId::quarter(2025, 1).unwrap(),
            PeriodId::quarter(2025, 2).unwrap(),
        ];
        let flat = distribution(0.0, 1e-20, 42);
        let mean_reverting = MeanRevertingParams {
            long_run_mean: 1e16,
            reversion_speed: 0.1,
            std_dev: 1e-20,
            ..mean_reverting_params()
        };
        let runs = [
            (
                "Normal",
                normal_forecast_with_stream(1e16, &periods, &flat, Some(5)),
            ),
            (
                "LogNormal",
                lognormal_forecast_with_stream(1e16, &periods, &flat, Some(5)),
            ),
            (
                "MeanReverting",
                mean_reverting_forecast_with_stream(1e16, &periods, &mean_reverting, Some(5)),
            ),
        ];
        for (method, run) in runs {
            let (values, shocks) = run.expect("forecast");
            let mut rng = build_rng(42, Some(5));
            for period in &periods {
                assert_eq!(values[period], 1e16, "{method}");
                assert_eq!(
                    shocks[period].to_bits(),
                    rng.normal(0.0, 1.0).to_bits(),
                    "{method}"
                );
            }
        }
    }

    #[test]
    fn deterministic_correlated_node_cannot_anchor_a_later_node() {
        let periods = [
            PeriodId::quarter(2025, 1).unwrap(),
            PeriodId::quarter(2025, 2).unwrap(),
        ];
        let cache = peer_cache(&periods, 0.5);
        let flat = ForecastParameters::Normal(DistributionParams {
            std_dev: 0.0,
            ..lognormal_params()
        });
        let (_, shocks) = monte_carlo_correlated_series(CorrelatedMonteCarloSeries {
            parameters: &flat,
            base_value: 10.0,
            forecast_periods: &periods,
            seed_offset: 7,
            node_id: "flat",
            peer_id: "peer",
            rho: 0.5,
            mc_z_cache: &cache,
        })
        .expect("deterministic child");
        assert!(shocks.values().all(|z| *z == 0.0));
        let cache = [(NodeId::new("flat"), shocks)].into_iter().collect();
        let child = ForecastParameters::Normal(DistributionParams {
            std_dev: 1.0,
            ..lognormal_params()
        });
        let err = monte_carlo_correlated_series(CorrelatedMonteCarloSeries {
            parameters: &child,
            base_value: 10.0,
            forecast_periods: &periods,
            seed_offset: 7,
            node_id: "child",
            peer_id: "flat",
            rho: 0.5,
            mc_z_cache: &cache,
        })
        .expect_err("zero-variance peer");
        assert!(err.to_string().contains("zero variance"));
    }
}

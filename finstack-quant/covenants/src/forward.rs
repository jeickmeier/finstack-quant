//! Generic covenant forecasting driven by [`ModelTimeSeries`].
//!
//! Statement-model adapters live in `finstack-quant-statements-analytics` so
//! this crate has no statements dependency.

use crate::engine::{
    headroom_for, is_covenant_breached, spec_metric_names, springing_condition_met, BoundKind,
    CovenantSpec, CovenantType, SpringingCondition,
};
use finstack_quant_core::dates::{Date, PeriodId};
use finstack_quant_core::math::norm_cdf;
use finstack_quant_core::math::random::{Pcg64Rng, RandomNumberGenerator};
use finstack_quant_core::Error;
use finstack_quant_core::InputError;
use finstack_quant_core::Result;
use serde::{Deserialize, Serialize};

/// Default RNG seed used when `random_seed` is `None` in Monte Carlo mode.
pub(crate) const DEFAULT_MC_SEED: u64 = 0;

/// Covenant forecast configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CovenantForecastConfig {
    /// Whether to use stochastic (as opposed to deterministic) breach probabilities.
    pub stochastic: bool,
    /// Selects the stochastic sub-mode: closed-form analytic vs. Monte Carlo.
    ///
    /// `0` (the default) uses the closed-form analytic lognormal
    /// probabilities via [`norm_cdf`]. A value greater than `0` switches to
    /// path-consistent Monte Carlo simulation with that many paths; with
    /// `antithetic` enabled, paths are simulated in `(Z, -Z)` pairs and this
    /// count is rounded up to a whole number of pairs. Ignored when
    /// `stochastic` is `false`.
    pub num_paths: usize,
    /// Volatility for stochastic scenarios (annualized).
    pub volatility: Option<f64>,
    /// RNG seed for Monte Carlo mode (`num_paths > 0`).
    ///
    /// `None` uses the crate-internal `DEFAULT_MC_SEED` (`0`). Ignored in analytic mode
    /// (`num_paths == 0`) and when `stochastic` is `false`.
    pub random_seed: Option<u64>,
    /// Enables antithetic variate pairing in Monte Carlo mode.
    ///
    /// Each simulated unit draws `Z` and reuses `-Z` as a paired path,
    /// halving the number of independent RNG draws and reducing estimator
    /// variance for payoffs that are monotone in `Z` (Glasserman 2003,
    /// *Monte Carlo Methods in Financial Engineering*, §4.1). Only
    /// meaningful when `num_paths > 0`; `antithetic == true` with
    /// `num_paths == 0` is rejected during validation as an inert flag.
    #[serde(default)]
    pub antithetic: bool,
    /// Reference date for time-scaling lognormal shocks. When set, shocks scale with
    /// `sqrt(T)` where T is the year-fraction from this date to the test date.
    /// When `None`, the engine uses the end date of the period immediately
    /// preceding the first forecast period, so the first simulated point still
    /// has a non-zero forecast horizon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_date: Option<Date>,
    /// Minimum stochastic breach probability to include in batch breach output.
    #[serde(default = "default_breach_probability_threshold")]
    pub breach_probability_threshold: f64,
}

impl Default for CovenantForecastConfig {
    fn default() -> Self {
        Self {
            stochastic: false,
            num_paths: 0,
            volatility: None,
            random_seed: None,
            antithetic: false,
            reference_date: None,
            breach_probability_threshold: default_breach_probability_threshold(),
        }
    }
}

fn default_breach_probability_threshold() -> f64 {
    0.05
}

/// Forecast output with headroom analytics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CovenantForecast {
    /// Stable covenant instance identifier.
    pub covenant_id: String,
    /// Human-readable covenant description.
    pub covenant_description: String,
    /// Comparison direction for the covenant threshold test.
    pub comparator: BoundKind,
    /// Future test dates for covenant evaluation
    pub test_dates: Vec<Date>,
    /// Projected metric values at each test date.
    ///
    /// `None` means the projected value was not representable as finite JSON
    /// (for example NaN or ±∞).
    pub projected_values: Vec<Option<f64>>,
    /// Covenant thresholds at each test date
    pub thresholds: Vec<f64>,
    /// Headroom (distance from breach) at each test date.
    ///
    /// `None` means the covenant is inactive for the period or the headroom is
    /// not meaningful under the applicable covenant convention.
    pub headroom: Vec<Option<f64>>,
    /// Probability of breach at each test date (stochastic mode).
    pub breach_probability: Vec<f64>,
    /// Standard error of the breach probability estimate.
    ///
    /// Analytic stochastic probabilities have zero estimator error.
    #[serde(default)]
    pub breach_probability_stderr: Vec<f64>,
    /// Date of first projected breach (if any)
    pub first_breach_date: Option<Date>,
    /// Date with minimum finite headroom.
    pub min_headroom_date: Option<Date>,
    /// Minimum finite headroom value across all active test dates.
    pub min_headroom_value: Option<f64>,
}

/// A projected covenant breach.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FutureBreach {
    /// Stable covenant instance identifier.
    pub covenant_id: String,
    /// Human-readable covenant description.
    pub covenant_description: String,
    /// Date of the breach
    pub breach_date: Date,
    /// Projected value, if finite.
    pub projected_value: Option<f64>,
    /// Threshold value
    pub threshold: f64,
    /// Headroom (negative means breach), if meaningful and finite.
    pub headroom: Option<f64>,
    /// Probability of breach (if stochastic)
    pub breach_probability: f64,
}

/// Minimal read-only adapter to query model time-series values and map periods to dates.
pub trait ModelTimeSeries: Send + Sync {
    /// Get scalar value for a metric node and period
    fn get_scalar(&self, node_id: &str, period: &PeriodId) -> Option<f64>;
    /// Get end date for a given period
    fn period_end_date(&self, period: &PeriodId) -> Date;
}

/// Forecast one numeric covenant across supplied model periods.
///
/// The adapter supplies an end date and scalar metric for each [`PeriodId`].
/// The forecast uses an explicit covenant `metric_id` first, then the
/// conventional metric name for the covenant type, and finally a custom metric
/// name where applicable. A threshold schedule overrides the covenant's static
/// threshold from its effective date onward. A covenant with an unmet springing
/// condition is inactive for that period and receives no headroom or breach
/// probability.
///
/// With `config.stochastic == false`, probabilities are deterministic: `0.0`
/// for pass and `1.0` for breach. With stochastic mode enabled, the function
/// applies a lognormal GBM overlay using `volatility` and calendar time from
/// the reference date, in one of two sub-modes selected by `config.num_paths`:
///
/// - `num_paths == 0`: closed-form analytic probabilities via [`norm_cdf`].
///   `breach_probability_stderr` is `0.0` (no estimator error).
/// - `num_paths > 0`: path-consistent Monte Carlo simulation. Each simulated
///   path draws from an independent [`Pcg64Rng`] stream keyed by
///   `(random_seed.unwrap_or(DEFAULT_MC_SEED), path_index)`, so results are
///   deterministic and independent of evaluation order. With
///   `config.antithetic` enabled, paths are simulated in `(Z, -Z)` pairs,
///   which cannot increase variance for a payoff monotone in `Z` (Glasserman
///   2003, *Monte Carlo Methods in Financial Engineering*, §4.1).
///   `breach_probability_stderr` carries the Monte Carlo standard error.
///
/// Both sub-modes fall back to the deterministic convention for a
/// non-positive or non-finite metric because a multiplicative lognormal
/// shock is not meaningful in that regime. A `NaN` metric is treated as an
/// indeterminate breach, matching point-in-time engine evaluation.
///
/// # Arguments
///
/// * `covenant` - Numeric covenant specification whose metric, threshold, and
///   springing condition determine the forecast.
/// * `model` - Time-series model that supplies the covenant metric and any
///   inputs used by a springing condition for each requested period.
/// * `periods` - Ordered reporting periods to forecast; the slice must not be
///   empty and each period must be covered by the covenant metric.
/// * `config` - Forecast policy including deterministic or stochastic mode,
///   volatility assumptions, and the reference date.
///
/// # Errors
///
/// Returns a validation error for an empty period set, invalid forecast
/// configuration, missing stochastic volatility, or a non-numeric covenant
/// without a bound and threshold. Returns `NotFound` when a required metric is
/// absent for any requested period, and propagates errors raised while
/// evaluating a springing condition. The caller should use
/// [`forecast_breaches_generic`] when a batch should skip uncovered periods
/// rather than fail as a whole.
pub fn forecast_covenant_generic<MTS: ModelTimeSeries>(
    covenant: &CovenantSpec,
    model: &MTS,
    periods: &[PeriodId],
    config: CovenantForecastConfig,
) -> Result<CovenantForecast> {
    if periods.is_empty() {
        return Err(Error::Validation("no periods provided".to_string()));
    }

    validate_config(&config)?;
    covenant.validate()?;

    let id = covenant.covenant.instance_key();
    let description = covenant.covenant.description();
    tracing::debug!(
        covenant = %id,
        periods = periods.len(),
        stochastic = config.stochastic,
        "forecasting covenant",
    );
    let bound_kind = covenant
        .covenant
        .covenant_type
        .bound_kind()
        .ok_or_else(|| {
            Error::Validation(format!(
                "covenant '{description}' has no bound kind (non-numeric covenants cannot be forecasted)"
            ))
        })?;
    let base_threshold = covenant
        .covenant
        .covenant_type
        .threshold_value()
        .ok_or_else(|| {
            Error::Validation(format!(
                "covenant '{description}' has no threshold (non-numeric covenants cannot be forecasted)"
            ))
        })?;

    let mut test_dates: Vec<Date> = Vec::with_capacity(periods.len());
    let mut thresholds: Vec<f64> = Vec::with_capacity(periods.len());
    let mut values: Vec<f64> = Vec::with_capacity(periods.len());
    let mut activation_flags: Vec<bool> = Vec::with_capacity(periods.len());

    for pid in periods {
        let date = model.period_end_date(pid);
        test_dates.push(date);

        let thr = covenant
            .threshold_schedule
            .as_ref()
            .and_then(|s| s.threshold_for(date))
            .unwrap_or(base_threshold);
        thresholds.push(thr);

        let is_active =
            springing_condition_active(covenant.covenant.springing_condition.as_ref(), model, pid)?;
        activation_flags.push(is_active);

        let v = metric_value_for_spec(covenant, model, pid).ok_or_else(|| {
            let looked_up = spec_metric_names(covenant).join("', '");
            Error::from(finstack_quant_core::InputError::NotFound {
                id: format!(
                    "metric for covenant '{description}' in period {pid}; \
                     looked for '{looked_up}'"
                ),
            })
        })?;
        values.push(v);
    }

    let mut headroom: Vec<Option<f64>> = values
        .iter()
        .zip(thresholds.iter())
        .map(|(&v, &t)| {
            let raw = headroom_for(covenant.covenant.covenant_type.bound_kind(), v, t);
            raw.is_finite()
                .then_some(raw)
                .filter(|_| !(covenant.covenant.covenant_type.is_ratio_max() && v < 0.0))
        })
        .collect();

    let mut deterministic_breach_prob: Vec<f64> = values
        .iter()
        .zip(thresholds.iter())
        .map(|(&v, &t)| {
            let breached = is_covenant_breached(&covenant.covenant.covenant_type, v, t);
            breached as u8 as f64
        })
        .collect();

    for (i, active) in activation_flags.iter().enumerate() {
        if !active {
            headroom[i] = None;
            deterministic_breach_prob[i] = 0.0;
        }
    }

    let mut breach_probability = deterministic_breach_prob.clone();
    let mut breach_probability_stderr = vec![0.0_f64; values.len()];

    if config.stochastic {
        let sigma = config.volatility.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "stochastic forecast requires volatility but none was set".to_owned(),
            )
        })?;
        tracing::debug!(
            sigma,
            num_paths = config.num_paths,
            "starting stochastic breach probability calculation"
        );

        let ref_date = config.reference_date.unwrap_or_else(|| {
            periods[0]
                .prev()
                .ok()
                .map(|prev| model.period_end_date(&prev))
                .unwrap_or(test_dates[0])
        });

        let mut eligible: Vec<usize> = Vec::new();
        let mut horizons: Vec<f64> = Vec::new();
        for i in 0..values.len() {
            if !activation_flags[i] {
                breach_probability[i] = 0.0;
                continue;
            }
            let base = values[i];
            let thr = thresholds[i];

            // Multiplicative lognormal shock is only valid for base > 0.
            if !base.is_finite() || base <= 0.0 {
                let breached = is_covenant_breached(&covenant.covenant.covenant_type, base, thr);
                breach_probability[i] = if breached { 1.0 } else { 0.0 };
                continue;
            }

            let t_years = (test_dates[i] - ref_date).whole_days().max(0) as f64 / 365.25;
            if sigma <= 0.0 || t_years <= 0.0 {
                breach_probability[i] =
                    if is_covenant_breached(&covenant.covenant.covenant_type, base, thr) {
                        1.0
                    } else {
                        0.0
                    };
                continue;
            }

            if config.num_paths == 0 {
                breach_probability[i] =
                    lognormal_breach_probability(bound_kind, base, thr, sigma, t_years);
            } else {
                eligible.push(i);
                horizons.push(t_years);
            }
        }

        if !eligible.is_empty() {
            let bases: Vec<f64> = eligible.iter().map(|&i| values[i]).collect();
            let thrs: Vec<f64> = eligible.iter().map(|&i| thresholds[i]).collect();
            let (probs, stderrs) = mc_breach_probabilities(
                &covenant.covenant.covenant_type,
                &bases,
                &thrs,
                &horizons,
                sigma,
                &config,
            );
            for (k, &i) in eligible.iter().enumerate() {
                breach_probability[i] = probs[k];
                breach_probability_stderr[i] = stderrs[k];
            }
        }
    }

    let min_idx = headroom
        .iter()
        .enumerate()
        .filter_map(|(i, h)| h.map(|h| (i, h)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i);
    let min_headroom_date = min_idx.map(|i| test_dates[i]);
    let min_headroom_value = min_idx.and_then(|i| headroom[i]);

    let first_breach_date = (0..values.len()).find_map(|i| {
        let v = values[i];
        let t = thresholds[i];
        if !activation_flags[i] {
            return None;
        }
        let breached = is_covenant_breached(&covenant.covenant.covenant_type, v, t);
        breached.then_some(test_dates[i])
    });

    let comparator = bound_kind;

    let projected_values = values.iter().map(|v| v.is_finite().then_some(*v)).collect();

    Ok(CovenantForecast {
        covenant_id: id,
        covenant_description: description,
        comparator,
        test_dates,
        projected_values,
        thresholds,
        headroom,
        breach_probability,
        breach_probability_stderr,
        first_breach_date,
        min_headroom_date,
        min_headroom_value,
    })
}

/// Forecast breaches for all active numeric covenants in an engine.
///
/// For each covenant the period set is restricted to the periods where its
/// metric actually resolves in the model; periods where the metric is missing
/// are skipped (with a `tracing::warn!`) instead of failing the whole batch.
/// A covenant whose projected metric is NaN in a period is reported as a
/// breach in that period, mirroring the point-in-time engine convention.
/// Non-numeric covenants are skipped because they lack a comparable threshold.
/// The result is ordered first by breach date and then by stable covenant
/// instance identifier. In stochastic mode, a period is included when the
/// analytic probability reaches `breach_probability_threshold`; deterministic
/// breaches are always included.
///
/// # Arguments
///
/// * `engine` - Covenant engine whose active numeric specifications are
///   considered for breach forecasting.
/// * `model` - Time-series model providing metric values and condition inputs
///   across the requested periods.
/// * `periods` - Reporting periods to inspect; uncovered metric periods are
///   skipped per covenant rather than failing the entire batch.
/// * `config` - Forecast policy including stochastic settings and the breach
///   probability threshold.
///
/// # Errors
///
/// Returns configuration, stochastic-volatility, and springing-condition
/// errors from [`forecast_covenant_generic`]. Missing metrics do not fail the
/// batch: the affected period is omitted for that covenant and logged at warn
/// level. The function returns an empty vector when no active numeric covenant
/// has a covered period that meets the breach criterion.
pub fn forecast_breaches_generic<MTS: ModelTimeSeries>(
    engine: &crate::engine::CovenantEngine,
    model: &MTS,
    periods: &[PeriodId],
    config: CovenantForecastConfig,
) -> Result<Vec<FutureBreach>> {
    validate_config(&config)?;
    engine.validate()?;

    let mut breaches = Vec::new();

    for spec in &engine.specs {
        if !spec.covenant.is_active {
            continue;
        }
        if spec.covenant.covenant_type.bound_kind().is_none()
            || spec.covenant.covenant_type.threshold_value().is_none()
        {
            tracing::warn!(
                covenant = %spec.covenant.description(),
                "non-numeric covenant skipped in breach forecast batch",
            );
            continue;
        }

        // Restrict to periods where this covenant's metric resolves. The
        // caller-supplied set is typically the union over all model nodes, so
        // a metric covering fewer periods must not hard-fail the batch.
        let covered: Vec<PeriodId> = periods
            .iter()
            .filter(|pid| metric_value_for_spec(spec, model, pid).is_some())
            .copied()
            .collect();
        if covered.len() < periods.len() {
            tracing::warn!(
                covenant = %spec.covenant.description(),
                skipped = periods.len() - covered.len(),
                total = periods.len(),
                "covenant metric missing for some periods — skipping them in breach forecast",
            );
        }
        if covered.is_empty() {
            continue;
        }

        let forecast = forecast_covenant_generic(spec, model, &covered, config.clone())?;

        for (i, &headroom) in forecast.headroom.iter().enumerate() {
            let is_breach = forecast.breach_probability[i] >= 1.0;
            let prob = forecast.breach_probability[i];

            if is_breach || (config.stochastic && prob >= config.breach_probability_threshold) {
                breaches.push(FutureBreach {
                    covenant_id: forecast.covenant_id.clone(),
                    covenant_description: forecast.covenant_description.clone(),
                    breach_date: forecast.test_dates[i],
                    projected_value: forecast.projected_values[i],
                    threshold: forecast.thresholds[i],
                    headroom,
                    breach_probability: prob,
                });
            }
        }
    }

    breaches.sort_by(|a, b| {
        a.breach_date
            .cmp(&b.breach_date)
            .then_with(|| a.covenant_id.cmp(&b.covenant_id))
    });

    Ok(breaches)
}

fn metric_value_for_spec<MTS: ModelTimeSeries>(
    spec: &CovenantSpec,
    model: &MTS,
    period: &PeriodId,
) -> Option<f64> {
    for name in spec_metric_names(spec) {
        if let Some(value) = model.get_scalar(name, period) {
            return Some(value);
        }
    }
    match &spec.covenant.covenant_type {
        CovenantType::Negative { .. } | CovenantType::Affirmative { .. } => Some(1.0),
        _ => None,
    }
}

fn validate_config(config: &CovenantForecastConfig) -> Result<()> {
    if !(0.0..=1.0).contains(&config.breach_probability_threshold)
        || !config.breach_probability_threshold.is_finite()
    {
        return Err(Error::Validation(
            "breach_probability_threshold must be finite and between 0 and 1".to_string(),
        ));
    }
    if config.stochastic {
        let sigma = config.volatility.ok_or_else(|| {
            Error::Validation(
                "stochastic covenant forecasts require volatility to be provided".to_string(),
            )
        })?;
        if !sigma.is_finite() || sigma < 0.0 {
            return Err(Error::Validation(
                "stochastic covenant forecast volatility must be finite and non-negative"
                    .to_string(),
            ));
        }
        if config.antithetic && config.num_paths == 0 {
            return Err(Error::Validation(
                "antithetic pairing requires num_paths > 0 (num_paths == 0 selects \
                 the closed-form analytic mode)"
                    .to_string(),
            ));
        }
    }
    Ok(())
}

fn lognormal_breach_probability(
    bound_kind: BoundKind,
    base: f64,
    threshold: f64,
    sigma: f64,
    t_years: f64,
) -> f64 {
    match bound_kind {
        BoundKind::AtMost if threshold <= 0.0 => 1.0,
        BoundKind::AtLeast if threshold <= 0.0 => 0.0,
        _ => {
            let drift = -0.5 * sigma * sigma * t_years;
            let z = ((threshold / base).ln() - drift) / (sigma * t_years.sqrt());
            match bound_kind {
                BoundKind::AtMost => 1.0 - norm_cdf(z),
                BoundKind::AtLeast => norm_cdf(z),
            }
        }
    }
}

/// Path-consistent Monte Carlo breach probabilities for the eligible dates.
///
/// Simulates GBM terminal values of the covenant metric under the same
/// drift/vol assumptions as [`lognormal_breach_probability`]:
///
/// ```text
/// W(T_k) = Σ_{j<=k} sqrt(T_j - T_{j-1}) · Z_j        (T_0 = 0)
/// V_k    = base_k · exp(-0.5·σ²·T_k + σ·W(T_k))
/// ```
///
/// The marginal law of `W(T_k)` is `N(0, T_k)`, so each date's estimator
/// converges to the analytic closed form while paths stay jointly consistent
/// across test dates. Deterministic: path `p` draws from
/// `Pcg64Rng::new_with_stream(seed, p)`, so results are reproducible and
/// independent of evaluation order. With `antithetic`, `(Z, -Z)` pairs are
/// simulated and `num_paths` is rounded up to a whole number of pairs
/// (Glasserman 2003, *Monte Carlo Methods in Financial Engineering*, §4.1).
///
/// Returns `(probabilities, standard_errors)` aligned with the input slices.
fn mc_breach_probabilities(
    covenant_type: &CovenantType,
    bases: &[f64],
    thresholds: &[f64],
    horizons: &[f64],
    sigma: f64,
    config: &CovenantForecastConfig,
) -> (Vec<f64>, Vec<f64>) {
    let n_dates = bases.len();
    let seed = config.random_seed.unwrap_or(DEFAULT_MC_SEED);
    let n_units = if config.antithetic {
        config.num_paths.div_ceil(2)
    } else {
        config.num_paths
    };

    let mut sum = vec![0.0_f64; n_dates];
    let mut sum_sq = vec![0.0_f64; n_dates];

    for unit in 0..n_units {
        let mut rng = Pcg64Rng::new_with_stream(seed, unit as u64);
        let mut w_pos = 0.0_f64;
        let mut w_neg = 0.0_f64;
        let mut t_prev = 0.0_f64;

        for k in 0..n_dates {
            let dt = (horizons[k] - t_prev).max(0.0);
            let step = dt.sqrt() * rng.normal(0.0, 1.0);
            w_pos += step;
            w_neg -= step;
            t_prev = horizons[k];

            let drift = -0.5 * sigma * sigma * horizons[k];
            let value_pos = bases[k] * (drift + sigma * w_pos).exp();
            let hit_pos = f64::from(is_covenant_breached(
                covenant_type,
                value_pos,
                thresholds[k],
            ));
            let unit_value = if config.antithetic {
                let value_neg = bases[k] * (drift + sigma * w_neg).exp();
                let hit_neg = f64::from(is_covenant_breached(
                    covenant_type,
                    value_neg,
                    thresholds[k],
                ));
                0.5 * (hit_pos + hit_neg)
            } else {
                hit_pos
            };
            sum[k] += unit_value;
            sum_sq[k] += unit_value * unit_value;
        }
    }

    let n = n_units as f64;
    let mut probs = Vec::with_capacity(n_dates);
    let mut stderrs = Vec::with_capacity(n_dates);
    for k in 0..n_dates {
        let mean = sum[k] / n;
        let variance = if n_units > 1 {
            ((sum_sq[k] - n * mean * mean) / (n - 1.0)).max(0.0)
        } else {
            0.0
        };
        probs.push(mean);
        stderrs.push((variance / n).sqrt());
    }
    (probs, stderrs)
}

fn springing_condition_active<MTS: ModelTimeSeries>(
    condition: Option<&SpringingCondition>,
    model: &MTS,
    period: &PeriodId,
) -> Result<bool> {
    if let Some(cond) = condition {
        let metric_name = cond.metric_id.as_str();
        let value = model.get_scalar(metric_name, period).ok_or_else(|| {
            Error::from(InputError::NotFound {
                id: format!("springing_metric:{metric_name}"),
            })
        })?;
        Ok(springing_condition_met(metric_name, value, cond.test))
    } else {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::{Date, PeriodId};
    use time::Month;

    struct MockTs {
        map: finstack_quant_core::HashMap<(String, String), f64>,
    }

    impl MockTs {
        fn new() -> Self {
            Self {
                map: finstack_quant_core::HashMap::default(),
            }
        }
        fn with(mut self, node: &str, period: PeriodId, v: f64) -> Self {
            self.map.insert((node.to_string(), period.to_string()), v);
            self
        }
    }

    impl ModelTimeSeries for MockTs {
        fn get_scalar(&self, node_id: &str, period: &PeriodId) -> Option<f64> {
            self.map
                .get(&(node_id.to_string(), period.to_string()))
                .copied()
        }
        fn period_end_date(&self, period: &PeriodId) -> Date {
            let m = [3u8, 6, 9, 12][(period.index as usize - 1).min(3)];
            Date::from_calendar_date(
                period.year,
                Month::try_from(m).expect("Valid month (1-12)"),
                30,
            )
            .expect("Valid test date")
        }
    }

    fn q(year: i32, q: u8) -> PeriodId {
        PeriodId::quarter(year, q).expect("valid period fixture")
    }

    #[test]
    fn deterministic_headroom_positive_zero_breach_prob() {
        // Debt/EBITDA <= 5, actual ratio at 4 → positive headroom
        let spec = CovenantSpec::with_metric(
            crate::engine::Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 5.0 },
                finstack_quant_core::dates::Tenor::quarterly(),
                "max_debt_ebitda",
            ),
            "debt_to_ebitda",
        );

        let periods = vec![q(2025, 1), q(2025, 2)];
        let mts = MockTs::new().with("debt_to_ebitda", periods[0], 4.0).with(
            "debt_to_ebitda",
            periods[1],
            4.2,
        );

        let cfg = CovenantForecastConfig::default();
        let fc = forecast_covenant_generic(&spec, &mts, &periods, cfg)
            .expect("Forecast covenant should succeed in test");

        assert!(fc.headroom.iter().all(|h| h.is_some_and(|h| h > 0.0)));
        assert!(fc
            .breach_probability
            .iter()
            .all(|&p| (p - 0.0).abs() < 1e-12));
        assert!(fc.first_breach_date.is_none());
    }

    #[test]
    fn stochastic_breach_probability_moves_with_vol() {
        // Debt/EBITDA <= 1.0, base ~ 1.0; with high vol, breach prob should be material
        let spec = CovenantSpec::with_metric(
            crate::engine::Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 1.0 },
                finstack_quant_core::dates::Tenor::quarterly(),
                "max_debt_ebitda",
            ),
            "debt_to_ebitda",
        );

        let periods = vec![q(2025, 1)];
        let mts = MockTs::new().with("debt_to_ebitda", periods[0], 1.0);

        let cfg = CovenantForecastConfig {
            stochastic: true,
            num_paths: 10_000,
            volatility: Some(0.25),
            random_seed: Some(42),
            antithetic: true,
            reference_date: None,
            breach_probability_threshold: default_breach_probability_threshold(),
        };
        let fc = forecast_covenant_generic(&spec, &mts, &periods, cfg)
            .expect("Forecast covenant should succeed in test");
        let p = fc.breach_probability[0];
        assert!(p > 0.2 && p < 0.8, "unexpected breach probability: {p}");
    }
    #[test]
    fn nan_metric_is_breached_deterministic() {
        // EBITDA through zero → ratio NaN. Must mirror the engine convention:
        // NaN ⇒ breached (probability 1), not a clean 0% path.
        let spec = CovenantSpec::with_metric(
            crate::engine::Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 4.0 },
                finstack_quant_core::dates::Tenor::quarterly(),
                "max_debt_ebitda",
            ),
            "debt_to_ebitda",
        );

        let periods = vec![q(2025, 1), q(2025, 2)];
        let mts = MockTs::new().with("debt_to_ebitda", periods[0], 3.0).with(
            "debt_to_ebitda",
            periods[1],
            f64::NAN,
        );

        let fc =
            forecast_covenant_generic(&spec, &mts, &periods, CovenantForecastConfig::default())
                .expect("forecast should succeed");

        assert_eq!(fc.breach_probability[0], 0.0);
        assert_eq!(fc.breach_probability[1], 1.0, "NaN metric must be breached");
        assert_eq!(
            fc.first_breach_date,
            Some(mts.period_end_date(&periods[1])),
            "NaN period must register as the first breach"
        );
    }

    #[test]
    fn nan_metric_is_breached_stochastic() {
        let spec = CovenantSpec::with_metric(
            crate::engine::Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 4.0 },
                finstack_quant_core::dates::Tenor::quarterly(),
                "max_debt_ebitda",
            ),
            "debt_to_ebitda",
        );

        let periods = vec![q(2025, 1)];
        let mts = MockTs::new().with("debt_to_ebitda", periods[0], f64::NAN);

        let cfg = CovenantForecastConfig {
            stochastic: true,
            num_paths: 1_000,
            volatility: Some(0.25),
            random_seed: Some(42),
            antithetic: false,
            reference_date: None,
            breach_probability_threshold: default_breach_probability_threshold(),
        };
        let fc =
            forecast_covenant_generic(&spec, &mts, &periods, cfg).expect("forecast should succeed");
        assert_eq!(
            fc.breach_probability[0], 1.0,
            "NaN base must produce stochastic breach probability 1.0"
        );
    }

    #[test]
    fn negative_ebitda_leverage_breaches_in_forecast_paths() {
        let spec = CovenantSpec::with_metric(
            crate::engine::Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 4.0 },
                finstack_quant_core::dates::Tenor::quarterly(),
                "max_debt_ebitda",
            ),
            "debt_to_ebitda",
        );

        let periods = vec![q(2025, 1)];
        let mts = MockTs::new().with("debt_to_ebitda", periods[0], -10.0);

        let deterministic =
            forecast_covenant_generic(&spec, &mts, &periods, CovenantForecastConfig::default())
                .expect("forecast should succeed");
        assert_eq!(deterministic.projected_values[0], Some(-10.0));
        assert_eq!(
            deterministic.headroom[0], None,
            "NM leverage headroom must not report positive cushion"
        );
        assert_eq!(deterministic.breach_probability[0], 1.0);
        assert_eq!(
            deterministic.first_breach_date,
            Some(mts.period_end_date(&periods[0]))
        );

        let stochastic = forecast_covenant_generic(
            &spec,
            &mts,
            &periods,
            CovenantForecastConfig {
                stochastic: true,
                num_paths: 1_000,
                volatility: Some(0.25),
                ..CovenantForecastConfig::default()
            },
        )
        .expect("stochastic forecast should succeed");
        assert_eq!(stochastic.breach_probability[0], 1.0);
    }

    #[test]
    fn forecast_breaches_generic_reports_nan_periods_as_breaches() {
        use crate::engine::CovenantEngine;

        let mut engine = CovenantEngine::new();
        engine.add_spec(CovenantSpec::with_metric(
            crate::engine::Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 4.0 },
                finstack_quant_core::dates::Tenor::quarterly(),
                "max_debt_ebitda",
            ),
            "debt_to_ebitda",
        ));

        let p1 = q(2025, 1);
        let p2 = q(2025, 2);
        let mts =
            MockTs::new()
                .with("debt_to_ebitda", p1, 3.0)
                .with("debt_to_ebitda", p2, f64::NAN);

        let breaches =
            forecast_breaches_generic(&engine, &mts, &[p1, p2], CovenantForecastConfig::default())
                .expect("forecast should succeed");

        assert_eq!(breaches.len(), 1, "NaN period must be reported as breach");
        assert!(breaches[0].projected_value.is_none());
        assert_eq!(breaches[0].breach_probability, 1.0);
    }

    #[test]
    fn forecast_breaches_generic_skips_uncovered_periods() {
        use crate::engine::CovenantEngine;

        // Two covenants on different metrics with different period coverage:
        // the union period set must not hard-fail the narrower covenant.
        let mut engine = CovenantEngine::new();
        engine.add_spec(CovenantSpec::with_metric(
            crate::engine::Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 4.0 },
                finstack_quant_core::dates::Tenor::quarterly(),
                "max_debt_ebitda",
            ),
            "debt_to_ebitda",
        ));
        engine.add_spec(CovenantSpec::with_metric(
            crate::engine::Covenant::new(
                CovenantType::MinInterestCoverage { threshold: 2.0 },
                finstack_quant_core::dates::Tenor::quarterly(),
                "min_interest_coverage",
            ),
            "interest_coverage",
        ));

        let p1 = q(2025, 1);
        let p2 = q(2025, 2);
        // interest_coverage only covers p1 (and breaches there);
        // debt_to_ebitda covers both and breaches in p2.
        let mts = MockTs::new()
            .with("debt_to_ebitda", p1, 3.0)
            .with("debt_to_ebitda", p2, 5.0)
            .with("interest_coverage", p1, 1.5);

        let breaches =
            forecast_breaches_generic(&engine, &mts, &[p1, p2], CovenantForecastConfig::default())
                .expect("partial metric coverage must not hard-fail");

        assert_eq!(breaches.len(), 2);
        assert!(breaches
            .iter()
            .any(|b| b.covenant_id == "min_interest_coverage"));
        assert!(breaches.iter().any(|b| b.covenant_id == "max_debt_ebitda"));
    }

    #[test]
    fn forecast_breaches_generic_skips_non_numeric_covenants() {
        use crate::engine::CovenantEngine;
        use crate::templates;

        let mut engine = CovenantEngine::new();
        for spec in templates::cov_lite(6.0, 4.0).unwrap() {
            engine.add_spec(spec);
        }

        let p1 = q(2025, 1);
        let mts = MockTs::new()
            .with("total_leverage", p1, 6.5)
            .with("senior_leverage", p1, 3.0);

        let breaches =
            forecast_breaches_generic(&engine, &mts, &[p1], CovenantForecastConfig::default())
                .expect("non-numeric covenants must be skipped, not batch-fail");

        assert_eq!(breaches.len(), 1);
        assert_eq!(breaches[0].covenant_id, "max_total_leverage");
    }

    #[test]
    fn test_forecast_breaches_generic() {
        use crate::engine::CovenantEngine;

        let mut engine = CovenantEngine::new();
        let covenant = crate::engine::Covenant::new(
            crate::engine::CovenantType::MaxDebtToEbitda { threshold: 3.0 },
            finstack_quant_core::dates::Tenor::quarterly(),
            "max_debt_ebitda",
        );
        let spec = CovenantSpec {
            covenant,
            metric_id: Some(crate::CovenantMetricId::from("NetDebtEbitda")),
            threshold_schedule: None,
        };
        engine.add_spec(spec);

        let p1 = q(2025, 1);
        let p2 = q(2025, 2);

        let mut adapter = MockTs::new();
        adapter = adapter.with("NetDebtEbitda", p1, 2.5); // Pass
        adapter = adapter.with("NetDebtEbitda", p2, 3.5); // Fail

        let periods = vec![p1, p2];
        let config = CovenantForecastConfig::default();

        let breaches = forecast_breaches_generic(&engine, &adapter, &periods, config)
            .expect("Forecast should succeed");

        assert_eq!(breaches.len(), 1);
        assert_eq!(breaches[0].covenant_id, "max_debt_ebitda");
        assert_eq!(breaches[0].covenant_description, "Debt/EBITDA <= 3.00x");
        assert_eq!(breaches[0].projected_value, Some(3.5));
    }

    fn atm_spec_and_model() -> (CovenantSpec, MockTs, Vec<PeriodId>) {
        // MaxDebtToEbitda <= 1.0 (AtMost bound), base exactly 1.0.
        let spec = CovenantSpec::with_metric(
            crate::engine::Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 1.0 },
                finstack_quant_core::dates::Tenor::quarterly(),
                "max_debt_ebitda",
            ),
            "debt_to_ebitda",
        );
        let periods = vec![q(2025, 1)];
        let mts = MockTs::new().with("debt_to_ebitda", periods[0], 1.0);
        (spec, mts, periods)
    }

    fn mc_config(num_paths: usize, antithetic: bool, seed: u64) -> CovenantForecastConfig {
        CovenantForecastConfig {
            stochastic: true,
            num_paths,
            volatility: Some(0.25),
            random_seed: Some(seed),
            antithetic,
            // Pin the horizon: 2024-12-30 -> 2025-03-30 = 90 days,
            // T = 90 / 365.25 = 0.2464065708...
            reference_date: Some(
                Date::from_calendar_date(2024, Month::December, 30).expect("valid date"),
            ),
            breach_probability_threshold: default_breach_probability_threshold(),
        }
    }

    #[test]
    fn num_paths_zero_selects_analytic_mode() {
        // Hand calculation of the analytic answer:
        //   T = 90/365.25 = 0.2464066, sigma = 0.25, base = thr = 1.0
        //   z = (ln(thr/base) + 0.5*sigma^2*T) / (sigma*sqrt(T))
        //     = 0.5 * 0.25 * sqrt(0.2464066) = 0.0620492
        //   p = 1 - Phi(0.0620492) ≈ 0.47526
        let (spec, mts, periods) = atm_spec_and_model();
        let fc = forecast_covenant_generic(&spec, &mts, &periods, mc_config(0, false, 42))
            .expect("num_paths = 0 must select analytic mode, not error");
        let p = fc.breach_probability[0];
        assert!((p - 0.47526).abs() < 1e-3, "analytic p = {p}");
        assert_eq!(fc.breach_probability_stderr[0], 0.0);
    }

    #[test]
    fn mc_converges_to_analytic_lognormal_probability() {
        let (spec, mts, periods) = atm_spec_and_model();

        let analytic = forecast_covenant_generic(&spec, &mts, &periods, mc_config(0, false, 42))
            .expect("analytic");
        let mc = forecast_covenant_generic(&spec, &mts, &periods, mc_config(200_000, true, 42))
            .expect("mc");

        let p_analytic = analytic.breach_probability[0];
        let p_mc = mc.breach_probability[0];
        let se = mc.breach_probability_stderr[0];

        assert!(se > 0.0, "MC mode must report a standard error");
        assert!(
            (p_mc - p_analytic).abs() < 5e-3,
            "MC {p_mc} vs analytic {p_analytic} (se {se})"
        );
    }

    #[test]
    fn mc_is_deterministic_for_a_fixed_seed() {
        let (spec, mts, periods) = atm_spec_and_model();
        let a = forecast_covenant_generic(&spec, &mts, &periods, mc_config(10_000, true, 42))
            .expect("run a");
        let b = forecast_covenant_generic(&spec, &mts, &periods, mc_config(10_000, true, 42))
            .expect("run b");
        assert_eq!(a.breach_probability, b.breach_probability);
        assert_eq!(a.breach_probability_stderr, b.breach_probability_stderr);

        let c = forecast_covenant_generic(&spec, &mts, &periods, mc_config(10_000, true, 43))
            .expect("run c");
        assert_ne!(
            a.breach_probability, c.breach_probability,
            "different seeds must produce different estimates"
        );
    }

    #[test]
    fn antithetic_reduces_standard_error_here() {
        // The breach indicator is monotone in Z for a lognormal metric, so
        // antithetic pairing cannot increase variance; assert it helps on
        // this fixture (fixed seed => deterministic assertion).
        let (spec, mts, periods) = atm_spec_and_model();
        let plain = forecast_covenant_generic(&spec, &mts, &periods, mc_config(20_000, false, 42))
            .expect("plain");
        let anti = forecast_covenant_generic(&spec, &mts, &periods, mc_config(20_000, true, 42))
            .expect("antithetic");
        assert!(
            anti.breach_probability_stderr[0] <= plain.breach_probability_stderr[0],
            "antithetic se {} > plain se {}",
            anti.breach_probability_stderr[0],
            plain.breach_probability_stderr[0]
        );
    }

    #[test]
    fn antithetic_without_paths_is_rejected() {
        let (spec, mts, periods) = atm_spec_and_model();
        let mut cfg = mc_config(0, true, 42);
        cfg.antithetic = true;
        assert!(
            forecast_covenant_generic(&spec, &mts, &periods, cfg).is_err(),
            "antithetic with num_paths = 0 is an inert flag and must be rejected"
        );
    }

    #[test]
    fn mc_keeps_deterministic_conventions_for_degenerate_bases() {
        // NaN base => breached with probability 1 in MC mode too.
        let spec = CovenantSpec::with_metric(
            crate::engine::Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 4.0 },
                finstack_quant_core::dates::Tenor::quarterly(),
                "max_debt_ebitda",
            ),
            "debt_to_ebitda",
        );
        let periods = vec![q(2025, 1)];
        let mts = MockTs::new().with("debt_to_ebitda", periods[0], f64::NAN);
        let fc = forecast_covenant_generic(&spec, &mts, &periods, mc_config(1_000, false, 42))
            .expect("forecast");
        assert_eq!(fc.breach_probability[0], 1.0);
        assert_eq!(fc.breach_probability_stderr[0], 0.0);
    }
}

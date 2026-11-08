//! Compact right-labelled observation-count rolling return and risk metrics.
//!
//! Centered/scaled incremental states rebuild after 1,024 slides or earlier
//! cancellation. Scalar same-estimator recovery handles inadequate intermediate
//! scales without converting positive risk to measured zero.

use finstack_quant_core::dates::Date;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::{Error, Result};

const ROLLING_KERNEL_RECOMPUTE_INTERVAL: usize = 1024;

/// The metric a rolling [`DatedSeries`] carries.
///
/// Serialized as its lowercase name (`"volatility"`, `"sortino"`,
/// `"sharpe"`, `"return"`), which hosts also use as the value-column label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RollingMetric {
    /// Annualized sample volatility (`Performance::rolling_volatility`).
    Volatility,
    /// Annualized Sortino ratio (`Performance::rolling_sortino`).
    Sortino,
    /// Annualized Sharpe ratio (`Performance::rolling_sharpe`).
    Sharpe,
    /// Compounded simple return over the window (`Performance::rolling_returns`).
    Return,
}

impl RollingMetric {
    /// The metric's wire name, identical to its serde form.
    ///
    /// # Returns
    ///
    /// `"volatility"`, `"sortino"`, `"sharpe"` or `"return"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Volatility => "volatility",
            Self::Sortino => "sortino",
            Self::Sharpe => "sharpe",
            Self::Return => "return",
        }
    }
}

impl std::fmt::Display for RollingMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A dated time-series column: scalar values aligned with window-end dates.
///
/// Shared carrier type for rolling analytics outputs. Concrete metrics
/// (rolling Sharpe, Sortino, volatility, returns) reuse this struct so they
/// share field names, serde shape, and helper methods; `value_column` names
/// the metric that produced the values.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DatedSeries {
    /// Computed metric values, one per completed rolling window. Undefined
    /// estimates (singleton sample risk or a zero-over-zero ratio) are `NaN`;
    /// genuine zero-risk ratios can be signed infinity. JSON preserves each
    /// nonfinite value with the corresponding string sentinel.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::NonFiniteF64Wire>")
    )]
    pub values: Vec<f64>,
    /// Window-end dates aligned 1:1 with `values`.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub dates: Vec<Date>,
    /// Metric carried by `values`; hosts use its name as the value-column label.
    pub value_column: RollingMetric,
}

impl DatedSeries {
    /// Allocate an empty series of `metric` with capacity for `cap` points.
    #[inline]
    #[must_use]
    fn with_capacity(metric: RollingMetric, cap: usize) -> Self {
        Self {
            values: Vec::with_capacity(cap),
            dates: Vec::with_capacity(cap),
            value_column: metric,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DatedSeriesWire {
    #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
    values: Vec<f64>,
    dates: Vec<Date>,
    value_column: RollingMetric,
}

impl<'de> serde::Deserialize<'de> for DatedSeries {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let wire = DatedSeriesWire::deserialize(deserializer)?;
        validate_alignment(&wire.values, &wire.dates).map_err(serde::de::Error::custom)?;
        Ok(Self {
            values: wire.values,
            dates: wire.dates,
            value_column: wire.value_column,
        })
    }
}

fn validate_alignment(values: &[f64], dates: &[Date]) -> Result<()> {
    if values.len() != dates.len() {
        return Err(Error::Validation(
            "Rolling values and dates must have equal lengths".into(),
        ));
    }
    if dates.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(Error::Validation(
            "Rolling dates must be strictly increasing".into(),
        ));
    }
    Ok(())
}

fn validate_request(returns: &[f64], dates: &[Date], window: usize) -> Result<()> {
    validate_alignment(returns, dates)?;
    if window == 0 {
        return Err(Error::Validation("Rolling window must be positive".into()));
    }
    crate::returns::validate_history(returns)
}

fn unavailable(metric: RollingMetric, dates: &[Date]) -> DatedSeries {
    DatedSeries {
        values: vec![f64::NAN; dates.len()],
        dates: dates.to_vec(),
        value_column: metric,
    }
}

/// Welford state in coordinates anchored on an actual return. Scaling the
/// dispersion, rather than the return level, retains high-offset small risk.
struct RollingMoments {
    anchor: f64,
    scale: f64,
    mean: f64,
    m2: f64,
    peak: f64,
}

impl RollingMoments {
    fn rebuild(window: &[f64]) -> Self {
        let anchor = window[0];
        let scale = window
            .iter()
            .map(|&value| (value - anchor).abs())
            .fold(0.0_f64, f64::max);
        let mut mean = 0.0;
        let mut m2 = 0.0;
        if scale > 0.0 {
            for (idx, &value) in window.iter().enumerate() {
                let value = (value - anchor) / scale;
                let delta = value - mean;
                mean += delta / (idx + 1) as f64;
                m2 += delta * (value - mean);
            }
        }
        Self {
            anchor,
            scale,
            mean,
            m2,
            peak: m2,
        }
    }

    fn slide(&mut self, outgoing: f64, incoming: f64, window: usize) -> bool {
        let new_delta = incoming - self.anchor;
        if self.scale == 0.0 && new_delta == 0.0 {
            return true;
        }
        if new_delta.abs() > self.scale {
            // Rescale the existing Welford state in constant work when a new
            // level expands its coordinate range. A trending series must not
            // rebuild its entire window for every new maximum.
            let ratio = self.scale / new_delta.abs();
            self.mean *= ratio;
            self.m2 *= ratio * ratio;
            self.peak *= ratio * ratio;
            self.scale = new_delta.abs();
        }
        let outgoing = (outgoing - self.anchor) / self.scale;
        let incoming = new_delta / self.scale;
        let count = window as f64;
        let keep_mean = self.mean - (outgoing - self.mean) / (count - 1.0);
        let keep_m2 = self.m2 - (outgoing - self.mean) * (outgoing - keep_mean);
        let delta = incoming - keep_mean;
        self.mean = keep_mean + delta / count;
        self.m2 = keep_m2 + delta * (incoming - self.mean);
        self.peak = self.peak.max(self.m2);
        self.valid() && !(self.peak > 0.0 && self.m2 <= f64::EPSILON.sqrt() * self.peak)
    }

    fn valid(&self) -> bool {
        self.mean.is_finite() && self.m2.is_finite() && self.m2 >= 0.0
    }

    fn mean(&self) -> f64 {
        self.anchor + self.scale * self.mean
    }
    fn sd(&self, window: usize) -> f64 {
        self.scale * (self.m2 / (window - 1) as f64).sqrt()
    }
}

fn moment_series<F>(
    returns: &[f64],
    dates: &[Date],
    window: usize,
    metric: RollingMetric,
    mut evaluate: F,
) -> Result<DatedSeries>
where
    F: FnMut(&[f64], &RollingMoments) -> Result<f64>,
{
    let n = returns.len();
    if n < window {
        return Ok(DatedSeries::with_capacity(metric, 0));
    }
    if window == 1 {
        return Ok(unavailable(metric, dates));
    }
    let mut output = DatedSeries::with_capacity(metric, n - window + 1);
    let mut state = RollingMoments::rebuild(&returns[..window]);
    let mut slides = 0;
    for end in window..=n {
        let current = &returns[end - window..end];
        if end > window {
            slides += 1;
            if !state.slide(returns[end - window - 1], returns[end - 1], window)
                || slides >= ROLLING_KERNEL_RECOMPUTE_INTERVAL
            {
                state = RollingMoments::rebuild(current);
                slides = 0;
            }
        }
        output.values.push(evaluate(current, &state)?);
        output.dates.push(dates[end - 1]);
    }
    Ok(output)
}

/// Frequency-annualized sample SD of each complete trailing window.
pub(crate) fn rolling_volatility(
    returns: &[f64],
    dates: &[Date],
    window: usize,
    ann_factor: f64,
) -> Result<DatedSeries> {
    validate_request(returns, dates, window)?;
    crate::returns::validate_frequency(ann_factor)?;
    moment_series(
        returns,
        dates,
        window,
        RollingMetric::Volatility,
        |values, state| {
            let value = state.sd(window) * ann_factor.sqrt();
            if !state.valid() || !value.is_finite() || (value == 0.0 && state.scale > 0.0) {
                super::return_based::volatility(values, ann_factor)
            } else {
                Ok(value)
            }
        },
    )
}

/// Arithmetic excess-return Sharpe with explicit annual-effective cash.
pub(crate) fn rolling_sharpe(
    returns: &[f64],
    dates: &[Date],
    window: usize,
    ann_factor: f64,
    risk_free_rate: f64,
) -> Result<DatedSeries> {
    validate_request(returns, dates, window)?;
    let cash = crate::returns::periodic_risk_free_rate(risk_free_rate, ann_factor)?;
    moment_series(
        returns,
        dates,
        window,
        RollingMetric::Sharpe,
        |values, state| {
            if state.scale == 0.0 {
                return Ok(super::ratio_or_sign_infinity(state.anchor - cash, 0.0));
            }
            let sd = state.sd(window);
            let value = ((state.mean() - cash) / sd) * ann_factor.sqrt();
            if !state.valid() || sd == 0.0 || !value.is_finite() || value == 0.0 {
                super::return_based::sharpe(values, risk_free_rate, ann_factor)
            } else {
                Ok(value)
            }
        },
    )
}

struct RollingDownside {
    scale: f64,
    sum: NeumaierAccumulator,
    downside: NeumaierAccumulator,
    peak: f64,
    below: usize,
    above: usize,
}

impl RollingDownside {
    fn rebuild(window: &[f64], mar: f64) -> Self {
        let scale = window
            .iter()
            .fold(mar.abs(), |scale, value| scale.max(value.abs()));
        // Reserve scale headroom so a gradual trend does not require a full
        // rebuild at every observation. The finite cap still admits f64::MAX.
        let scale = if scale <= f64::MAX / 2.0 {
            scale * 2.0
        } else {
            f64::MAX
        };
        let mut state = Self {
            scale,
            sum: NeumaierAccumulator::new(),
            downside: NeumaierAccumulator::new(),
            peak: 0.0,
            below: 0,
            above: 0,
        };
        if scale > 0.0 {
            for &value in window {
                state.update(value, mar, 1.0);
            }
        }
        state.peak = state.downside.total();
        state
    }

    fn update(&mut self, value: f64, mar: f64, sign: f64) {
        // Sum observation-level differences directly. Subtracting MAR from
        // an already averaged return loses a small but representable reward
        // when every observation lies only a few ulps below a large hurdle.
        let difference = value - mar;
        let excess = if difference.is_finite() {
            difference / self.scale
        } else {
            value / self.scale - mar / self.scale
        };
        self.sum.add(sign * excess);
        if value > mar {
            if sign > 0.0 {
                self.above += 1;
            } else {
                self.above -= 1;
            }
        }
        if value < mar {
            if sign > 0.0 {
                self.below += 1;
            } else {
                self.below -= 1;
            }
            let shortfall = -excess;
            self.downside.add(sign * shortfall * shortfall);
        }
    }

    fn valid(&self) -> bool {
        let downside = self.downside.total();
        self.sum.total().is_finite() && downside.is_finite() && downside >= 0.0
    }

    fn slide(&mut self, outgoing: f64, incoming: f64, mar: f64) -> bool {
        if self.scale == 0.0 && incoming == 0.0 {
            return true;
        }
        if incoming.abs() > self.scale {
            return false;
        }
        self.update(incoming, mar, 1.0);
        self.peak = self.peak.max(self.downside.total());
        self.update(outgoing, mar, -1.0);
        self.valid()
            && !(self.peak > 0.0 && self.downside.total() <= f64::EPSILON.sqrt() * self.peak)
    }
}

/// Full-observation downside Sortino with explicit per-observation MAR.
pub(crate) fn rolling_sortino(
    returns: &[f64],
    dates: &[Date],
    window: usize,
    ann_factor: f64,
    mar: f64,
) -> Result<DatedSeries> {
    validate_request(returns, dates, window)?;
    crate::returns::validate_frequency(ann_factor)?;
    if !mar.is_finite() {
        return Err(Error::Validation(
            "Rolling Sortino MAR must be finite".into(),
        ));
    }
    if returns.len() < window {
        return Ok(DatedSeries::with_capacity(RollingMetric::Sortino, 0));
    }
    if window == 1 {
        return Ok(unavailable(RollingMetric::Sortino, dates));
    }
    let mut output = DatedSeries::with_capacity(RollingMetric::Sortino, returns.len() - window + 1);
    let mut state = RollingDownside::rebuild(&returns[..window], mar);
    let mut slides = 0;
    for end in window..=returns.len() {
        let current = &returns[end - window..end];
        if end > window {
            slides += 1;
            if !state.slide(returns[end - window - 1], returns[end - 1], mar)
                || slides >= ROLLING_KERNEL_RECOMPUTE_INTERVAL
            {
                state = RollingDownside::rebuild(current, mar);
                slides = 0;
            }
        }
        if state.below == 0 {
            output.values.push(if state.above == 0 {
                f64::NAN
            } else {
                f64::INFINITY
            });
            output.dates.push(dates[end - 1]);
            continue;
        }
        let downside = (state.downside.total() / window as f64).sqrt();
        let reward = state.sum.total() / window as f64;
        let value = (reward / downside) * ann_factor.sqrt();
        let value = if !state.valid() || downside == 0.0 || !value.is_finite() || value == 0.0 {
            super::return_based::sortino(current, mar, ann_factor)?
        } else {
            value
        };
        output.values.push(value);
        output.dates.push(dates[end - 1]);
    }
    Ok(output)
}

fn log_sum(window: &[f64]) -> (NeumaierAccumulator, usize) {
    let mut sum = NeumaierAccumulator::new();
    let mut terminals = 0;
    for &value in window {
        if value <= -1.0 {
            terminals += 1;
        } else {
            sum.add(value.ln_1p());
        }
    }
    (sum, terminals)
}

/// Compounded return from each trailing window's own unit baseline.
pub(crate) fn rolling_returns(
    returns: &[f64],
    dates: &[Date],
    window: usize,
) -> Result<DatedSeries> {
    validate_request(returns, dates, window)?;
    if returns.len() < window {
        return Ok(DatedSeries::with_capacity(RollingMetric::Return, 0));
    }
    let mut output = DatedSeries::with_capacity(RollingMetric::Return, returns.len() - window + 1);
    let (mut state, mut terminals) = log_sum(&returns[..window]);
    let mut slides = 0;
    for end in window..=returns.len() {
        if end > window {
            let incoming = returns[end - 1];
            let outgoing = returns[end - window - 1];
            if incoming <= -1.0 {
                terminals += 1;
            } else {
                state.add(incoming.ln_1p());
            }
            if outgoing <= -1.0 {
                terminals -= 1;
            } else {
                state.add(-outgoing.ln_1p());
            }
            slides += 1;
            if !state.total().is_finite() || slides >= ROLLING_KERNEL_RECOMPUTE_INTERVAL {
                (state, terminals) = log_sum(&returns[end - window..end]);
                slides = 0;
            }
        }
        let value = if terminals > 0 {
            -1.0
        } else {
            crate::returns::finite_result(state.total().exp_m1(), "rolling compounded return")?
        };
        output.values.push(value);
        output.dates.push(dates[end - 1]);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::{Duration, Month};
    fn dates(count: usize) -> Vec<Date> {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        (0..count)
            .map(|i| start + Duration::days(i as i64))
            .collect()
    }

    #[test]
    fn terminal_return_and_compact_labels() {
        let dates = dates(3);
        let output = rolling_returns(&[0.01, 0.02, -1.0], &dates, 2).expect("valid");
        assert!((output.values[0] - 0.0302).abs() < 1e-15);
        assert_eq!(output.values[1], -1.0);
        assert_eq!(output.dates, dates[1..]);
        assert_eq!(output.value_column, RollingMetric::Return);
        assert!(rolling_returns(&[-1.0, 0.0], &dates[..2], 3).is_err());
        assert!(rolling_returns(&[0.01], &dates[..1], 0).is_err());
        assert!(rolling_returns(&[1.0e308; 2], &dates[..2], 2).is_err());
    }

    #[test]
    fn validation_precedes_empty_and_singleton_outputs() {
        let dates = dates(2);
        assert!(rolling_volatility(&[0.1], &dates, 5, 252.0).is_err());
        assert!(rolling_volatility(&[0.1, 0.2], &[dates[0]; 2], 5, 252.0).is_err());
        assert!(rolling_volatility(&[], &[], 5, 0.0).is_err());
        assert!(rolling_sharpe(&[], &[], 5, 252.0, -1.0).is_err());
        assert!(rolling_sortino(&[], &[], 5, 252.0, f64::NAN).is_err());
        let series = rolling_sharpe(&[0.01, 0.02], &dates, 1, 252.0, 0.0).expect("valid");
        assert_eq!(series.values.len(), 2);
        assert!(series.values.iter().all(|value| value.is_nan()));
        let empty = rolling_sortino(&[], &[], 5, 252.0, 0.0).expect("empty");
        assert!(empty.values.is_empty());
        assert_eq!(empty.value_column, RollingMetric::Sortino);
    }

    #[test]
    fn scaled_risk_survives_underflow_and_outlier_departure() {
        let tiny = 1.0e-200;
        let returns = [0.1, tiny, 0.0, tiny, 0.0];
        let output = rolling_volatility(&returns, &dates(5), 2, 1.0).expect("scaled risk");
        let expected = tiny / 2.0_f64.sqrt();
        for &value in &output.values[1..] {
            assert!((value / expected - 1.0).abs() < 1e-12, "{value}");
        }
        let output =
            rolling_sortino(&[-tiny, tiny], &dates(2), 2, 1.0, 0.0).expect("scaled downside");
        assert_eq!(output.values[0], 0.0);
        let output =
            rolling_sortino(&[-tiny, 3.0 * tiny], &dates(2), 2, 1.0, 0.0).expect("scaled ratio");
        assert!((output.values[0] - 2.0_f64.sqrt()).abs() < 1e-12);

        let mar = 0.005;
        let returns = [mar - 0.03, mar - 0.01, mar - 1e-18, mar - 1e-18];
        let output = rolling_sortino(&returns, &dates(4), 2, 252.0, mar)
            .expect("small reward relative to MAR");
        // A constant observation below MAR has equal-magnitude reward and
        // downside, however small its difference from the hurdle.
        assert!((output.values[2] + 252.0_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn requested_ratios_survive_unrequested_annual_mean_overflow() {
        let output =
            rolling_sharpe(&[0.0, 1.0e308], &dates(2), 2, 12.0, 0.0).expect("representable Sharpe");
        assert!((output.values[0] - 6.0_f64.sqrt()).abs() < 1e-12);
        let constant =
            rolling_sharpe(&[1.0e308; 2], &dates(2), 2, 12.0, 0.0).expect("genuine zero risk");
        assert_eq!(constant.values[0], f64::INFINITY);
        let flat = rolling_sortino(&[0.0; 2], &dates(2), 2, 12.0, 0.0).expect("zero-over-zero");
        assert!(flat.values[0].is_nan());
        assert!(rolling_volatility(&[0.0, 1.0e308], &dates(2), 2, 12.0).is_err());
    }

    #[test]
    fn high_offset_and_rebuild_boundary_have_independent_sample_sd() {
        let center = 1.0e8;
        let step = 1.0e-3;
        let returns: Vec<f64> = (0..2100).map(|i| center + (i % 3) as f64 * step).collect();
        let output = rolling_volatility(&returns, &dates(returns.len()), 3, 1.0).expect("rolling");
        let a = center;
        let b = center + step;
        let c = center + 2.0 * step;
        let mean_offset = ((b - a) + (c - a)) / 3.0;
        let expected =
            ((mean_offset.powi(2) + (b - a - mean_offset).powi(2) + (c - a - mean_offset).powi(2))
                / 2.0)
                .sqrt();
        for value in output.values {
            assert!((value / expected - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn restoration_checks_shape_dates_and_preserves_sentinels() {
        let output = rolling_sharpe(&[0.0, 0.0], &dates(2), 1, 252.0, 0.0).expect("singleton");
        let json = serde_json::to_value(&output).expect("serialize");
        assert_eq!(json["values"][0], "nan");
        let restored: DatedSeries = serde_json::from_value(json.clone()).expect("restore");
        assert!(restored.values.iter().all(|value| value.is_nan()));
        let mut invalid = json.clone();
        invalid["dates"] = serde_json::json!([]);
        assert!(serde_json::from_value::<DatedSeries>(invalid).is_err());
        let mut invalid = json;
        invalid["dates"][1] = invalid["dates"][0].clone();
        assert!(serde_json::from_value::<DatedSeries>(invalid).is_err());
        let json = serde_json::json!({"values":["inf","-inf"],"dates":["2025-01-01","2025-01-02"],"value_column":"sortino"});
        let restored: DatedSeries = serde_json::from_value(json).expect("infinite ratios");
        assert_eq!(restored.values, [f64::INFINITY, f64::NEG_INFINITY]);
    }
}

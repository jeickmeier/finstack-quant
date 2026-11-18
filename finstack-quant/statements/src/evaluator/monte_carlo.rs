//! Monte Carlo evaluation types and aggregation utilities.

use crate::error::{Error, Result};
use crate::evaluator::results::{CapitalStructureWarning, EvalWarning};
use crate::types::FinancialModelSpec;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::math::stats::quantile_sorted;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};
use indexmap::IndexMap;
use serde_json::json;

/// One path's value of one requested metric in one period.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PathSample {
    /// A finite value.
    Value(f64),
    /// The cell had no value on this path, for the stated reason.
    Absent(&'static str),
}

/// Results for a single Monte Carlo path: one sample per requested metric
/// and period, and any warnings emitted.
pub(crate) type PathResult = (
    IndexMap<String, IndexMap<PeriodId, PathSample>>,
    Vec<EvalWarning>,
);

/// Most samples a run may hold: paths x metrics x forecast periods.
pub const MAX_MONTE_CARLO_SAMPLES: usize = 100_000_000;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

fn default_percentiles() -> Vec<f64> {
    vec![0.05, 0.5, 0.95]
}

/// Configuration for Monte Carlo evaluation of a statement model.
///
/// # Choosing the number of paths
///
/// The required path count depends on the quantities of interest:
///
/// | Use case | Recommended `n_paths` |
/// |---|---|
/// | Mean / median estimates | 1 000 – 2 000 |
/// | 5th / 95th percentiles | 5 000 – 10 000 |
/// | 1st / 99th percentiles or CVaR | 10 000 – 50 000 |
/// | Breach-probability estimates | 10 000+ |
///
/// Standard-error of a percentile estimate scales as
/// $O\bigl(1/\sqrt{n}\bigr)$, so tails require proportionally more
/// paths to converge. When in doubt, run two simulations with
/// different seeds and compare results; if the metric of interest
/// moves by more than the desired precision, increase `n_paths`.
///
/// The default constructor ([`MonteCarloConfig::new`]) does **not** impose a
/// minimum—callers choose the path count explicitly so the trade-off between
/// accuracy and runtime is visible.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MonteCarloConfig {
    /// Number of Monte Carlo paths to simulate: at least one, and at most
    /// `u32::MAX`, the path-identifier domain.
    pub n_paths: usize,
    /// Base random seed used to derive per-path streams: an exact unsigned
    /// 64-bit integer, a canonical decimal string in JSON (`"42"`, never the
    /// number `42`) so every host carries it exactly.
    #[serde(with = "crate::types::forecast_seed")]
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub seed: u64,
    /// Node identifiers to report. Required: at least one, each a node of
    /// the model, none repeated. Only these are sampled and stored.
    pub metrics: Vec<String>,
    /// Probability levels to compute, each finite in `[0, 1]` and distinct.
    /// Omitted or empty selects `0.05`, `0.5` and `0.95`.
    #[serde(default = "default_percentiles")]
    pub percentiles: Vec<f64>,
    /// Whether to include the long-format path table in serialized results.
    #[serde(default)]
    pub include_path_data: bool,
}

impl MonteCarloConfig {
    /// Create a Monte Carlo configuration with the default percentiles.
    ///
    /// # Arguments
    ///
    /// * `n_paths` - Number of paths to simulate, from 1 to `u32::MAX`.
    /// * `seed` - Exact base seed; the same seed replays the same paths.
    /// * `metrics` - Node identifiers to report: at least one, each a node of
    ///   the model, none repeated.
    pub fn new(
        n_paths: usize,
        seed: u64,
        metrics: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            n_paths,
            seed,
            metrics: metrics.into_iter().map(Into::into).collect(),
            percentiles: default_percentiles(),
            include_path_data: false,
        }
    }

    /// Override the percentiles to compute.
    #[must_use]
    pub fn with_percentiles(mut self, percentiles: Vec<f64>) -> Self {
        self.percentiles = percentiles;
        self
    }

    /// Include the long-format path table in results.
    #[must_use]
    pub fn with_path_data(mut self, include_path_data: bool) -> Self {
        self.include_path_data = include_path_data;
        self
    }
}

/// The market and observation context a Monte Carlo run evaluates in.
///
/// It is the same context an ordinary market-aware evaluation takes
/// ([`Evaluator::evaluate_with_market`](crate::evaluator::Evaluator::evaluate_with_market)).
#[derive(Clone, Copy)]
pub struct MonteCarloContext<'a> {
    /// Market the capital structure's instruments are projected on.
    pub market: &'a finstack_quant_core::market_data::context::MarketContext,
    /// Valuation date of the capital structure's contractual flows.
    pub as_of: finstack_quant_core::dates::Date,
    /// Explicit observations the paths may read.
    pub cutoff: crate::evaluator::ObservationCutoff,
}

/// Why a Monte Carlo run published no statistics.
///
/// A run is complete only when every path has a value for every requested
/// metric in every forecast period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MonteCarloIncomplete {
    /// First metric, in the requested order, with a sample without a value.
    pub metric: String,
    /// First forecast period of that metric with such a sample.
    pub period: PeriodId,
    /// Lowest path with such a sample for that metric and period.
    pub path: u32,
    /// Why that cell had no value: `missing`, `masked`, or the reason an
    /// unavailable cell gives (for example `missing_peer_shock`).
    pub reason: String,
    /// Number of samples without a value across the run.
    pub missing_samples: usize,
}

/// Per-metric percentile time series.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct PercentileSeries {
    /// Metric / node identifier.
    pub metric: String,
    /// Units of every value: scalar, or money in one currency.
    pub value_type: crate::types::NodeValueType,
    /// Number of samples behind each quantile: the path count.
    pub sample_count: usize,
    /// Period → `(probability, value)` pairs in ascending probability. The
    /// value is the R-7 sample quantile, in `value_type` units.
    pub values: IndexMap<PeriodId, Vec<(f64, f64)>>,
}

/// Monte Carlo results for a statement model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct MonteCarloResults {
    /// Aggregated percentile results: `metric → PercentileSeries`, in the
    /// requested metric order. Empty when the run is incomplete.
    pub percentile_results: IndexMap<String, PercentileSeries>,
    /// Why no statistics were published, when a requested sample had no
    /// value on some path. `None` for a complete run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incomplete: Option<MonteCarloIncomplete>,
    /// Number of Monte Carlo paths simulated.
    pub n_paths: usize,
    /// The run's exact base seed, a decimal string in JSON: an unsigned
    /// 64-bit value does not fit a JavaScript number.
    #[serde(with = "crate::types::forecast_seed")]
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub seed: u64,
    /// Version of the mapping from the seed, path and node to a random
    /// stream. With the seed it replays any path from the result alone.
    pub stream_version: u32,
    /// The metrics requested, in order.
    pub metrics: Vec<String>,
    /// Percentiles computed for each metric/period.
    pub percentiles: Vec<f64>,
    /// Forecast (non-actual) periods included in the simulation.
    pub forecast_periods: Vec<PeriodId>,
    /// Distinct warnings raised in the simulated forecast periods, each
    /// reported once however many paths raised it, in a canonical sorted
    /// order. Warnings in actual periods are omitted: those periods are not
    /// simulated and every path repeats the deterministic evaluation's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<EvalWarning>,
    /// Internal storage of path-level values keyed by `(path_id, value)` so
    /// per-path aggregations (e.g. breach probability) remain correct
    /// regardless of parallel merge order.
    #[serde(skip)]
    pub(crate) path_values: IndexMap<String, IndexMap<PeriodId, Vec<(u32, f64)>>>,
    /// Optional full path data in long-format table form.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_data: Option<TableEnvelope>,
}

/// Column label for a percentile given as a fraction: `0.05` → `p5`,
/// `0.975` → `p97.5`.
fn percentile_label(percentile: f64) -> String {
    let pct = percentile * 100.0;
    let rounded = pct.round();
    if (pct - rounded).abs() < 1e-9 {
        format!("p{rounded}")
    } else {
        format!("p{pct}")
    }
}

impl MonteCarloResults {
    /// Get a time series of one probability level for a metric.
    ///
    /// The level is matched exactly: pass a probability the run was
    /// configured with. `None` when the metric or the level was not computed,
    /// including for an incomplete run.
    pub fn percentile_by_period(
        &self,
        metric: &str,
        percentile: f64,
    ) -> Option<IndexMap<PeriodId, f64>> {
        let series = self.percentile_results.get(metric)?;
        let mut out = IndexMap::new();

        for (period, pairs) in &series.values {
            if let Some((_, value)) = pairs.iter().find(|(q, _)| q.total_cmp(&percentile).is_eq()) {
                out.insert(*period, *value);
            }
        }

        if out.is_empty() { None } else { Some(out) }
    }

    /// Export one metric's percentile bands as a wide table.
    ///
    /// Columns: `period` (period identifier such as `2025Q1`) followed by one
    /// column per configured percentile, named `p<pct>` from the percentile in
    /// percent (`0.05` → `p5`, `0.5` → `p50`, `0.975` → `p97.5`). A percentile
    /// with no stored value for a period is `NaN`.
    ///
    /// # Arguments
    ///
    /// * `metric` - Node id of a simulated metric, exactly as it appears in
    ///   `percentile_results`.
    ///
    /// # Returns
    ///
    /// `None` when `metric` was not simulated.
    ///
    /// # Errors
    ///
    /// Returns an error if two configured percentiles produce the same column
    /// name.
    pub fn percentile_table(&self, metric: &str) -> Result<Option<TableEnvelope>> {
        let Some(series) = self.percentile_results.get(metric) else {
            return Ok(None);
        };
        let periods = series.values.keys().map(ToString::to_string).collect();
        let mut columns = vec![
            TableColumn::new("period", TableColumnData::String(periods))
                .with_role(TableColumnRole::Index),
        ];
        for &percentile in &self.percentiles {
            let values = series
                .values
                .values()
                .map(|pairs| {
                    pairs
                        .iter()
                        .find(|(q, _)| q.total_cmp(&percentile).is_eq())
                        .map_or(f64::NAN, |(_, value)| *value)
                })
                .collect();
            columns.push(
                TableColumn::new(
                    percentile_label(percentile),
                    TableColumnData::Float64(values),
                )
                .with_role(TableColumnRole::Measure),
            );
        }
        TableEnvelope::new(columns, Default::default())
            .map(Some)
            .map_err(Into::into)
    }

    /// Estimate the probability that a metric exceeds a threshold in any
    /// forecast period.
    ///
    /// A path counts once when any forecast-period value is strictly greater
    /// than the threshold; the count is divided by the path count. This is an
    /// **upper** breach. For a floor (for example coverage falling below a
    /// minimum), report a metric that flips the sign.
    ///
    /// Path values come from the in-memory simulation store or, for results
    /// restored from JSON (where that store is not serialized), from the
    /// long-format `path_data` table (`MonteCarloConfig::with_path_data`).
    ///
    /// # Arguments
    ///
    /// * `metric` - A requested metric.
    /// * `threshold` - A finite upper threshold in the metric's own units: a
    ///   scalar for a scalar metric, money in the metric's currency for a
    ///   monetary one.
    ///
    /// # Returns
    ///
    /// `None` when no path samples are available: the run is incomplete, the
    /// metric was not requested, or the result was restored without
    /// `path_data`.
    ///
    /// # Errors
    ///
    /// Returns an evaluation error when the threshold is not finite or its
    /// units differ from the metric's, or when a restored path table does not
    /// hold exactly one finite sample for every path and forecast period.
    pub fn breach_probability(
        &self,
        metric: &str,
        threshold: crate::types::AmountOrScalar,
    ) -> Result<Option<f64>> {
        use crate::types::{AmountOrScalar, NodeValueType};
        let Some(series) = self.percentile_results.get(metric) else {
            return Ok(None);
        };
        let level = match (threshold, series.value_type) {
            (AmountOrScalar::Scalar(value), NodeValueType::Scalar) => value,
            (AmountOrScalar::Amount(money), NodeValueType::Monetary { currency })
                if money.currency() == currency =>
            {
                money.amount()
            }
            _ => {
                return Err(Error::eval(format!(
                    "breach threshold {threshold:?} is not in the units of metric '{metric}' \
                     ({:?})",
                    series.value_type
                )));
            }
        };
        if !level.is_finite() {
            return Err(Error::eval(format!(
                "breach threshold for metric '{metric}' must be finite, got {level}"
            )));
        }
        let restored;
        let metric_map = if self.path_values.is_empty() {
            let Some(table) = self.path_data.as_ref() else {
                return Ok(None);
            };
            restored = self.restore_metric_paths(table, metric)?;
            &restored
        } else {
            match self.path_values.get(metric) {
                Some(map) => map,
                None => return Ok(None),
            }
        };
        let mut breached = vec![false; self.n_paths];
        for values in metric_map.values() {
            for (path, value) in values {
                if *value > level {
                    breached[*path as usize] = true;
                }
            }
        }
        let count = breached.iter().filter(|hit| **hit).count();
        Ok(Some(count as f64 / self.n_paths as f64))
    }

    /// Rebuild one metric's `period -> [(path_id, value)]` store from the
    /// long-format path table, requiring exactly one finite sample for every
    /// path and forecast period.
    fn restore_metric_paths(
        &self,
        table: &TableEnvelope,
        metric: &str,
    ) -> Result<IndexMap<PeriodId, Vec<(u32, f64)>>> {
        let malformed = |what: &str| Error::eval(format!("Monte Carlo path table: {what}"));
        let column = |name: &str| {
            table
                .columns
                .iter()
                .find(|c| c.name == name)
                .map(|c| &c.data)
        };
        let (
            Some(TableColumnData::UInt32(path_ids)),
            Some(TableColumnData::String(periods)),
            Some(TableColumnData::String(metrics)),
            Some(TableColumnData::Float64(values)),
        ) = (
            column("path_id"),
            column("period"),
            column("metric"),
            column("value"),
        )
        else {
            return Err(malformed(
                "expected the columns path_id, period, metric and value",
            ));
        };
        let forecast: HashSet<PeriodId> = self.forecast_periods.iter().copied().collect();
        let mut seen: HashSet<(u32, PeriodId)> = HashSet::new();
        let mut out: IndexMap<PeriodId, Vec<(u32, f64)>> = IndexMap::new();
        for (((path_id, period), row_metric), value) in
            path_ids.iter().zip(periods).zip(metrics).zip(values)
        {
            if row_metric != metric {
                continue;
            }
            let period: PeriodId = period
                .parse()
                .map_err(|_| malformed(&format!("'{period}' is not a period")))?;
            if !forecast.contains(&period) || (*path_id as usize) >= self.n_paths {
                return Err(malformed(&format!(
                    "the sample of path {path_id} in {period} is outside the run"
                )));
            }
            if !value.is_finite() {
                return Err(malformed(&format!(
                    "the sample of path {path_id} in {period} for '{metric}' is not finite"
                )));
            }
            if !seen.insert((*path_id, period)) {
                return Err(malformed(&format!(
                    "path {path_id} has more than one sample of '{metric}' in {period}"
                )));
            }
            out.entry(period).or_default().push((*path_id, *value));
        }
        if seen.len() != self.n_paths * self.forecast_periods.len() {
            return Err(malformed(&format!(
                "'{metric}' has {} samples; a complete run has {} paths x {} forecast periods",
                seen.len(),
                self.n_paths,
                self.forecast_periods.len()
            )));
        }
        Ok(out)
    }
}

fn normalize_percentiles(raw: &[f64]) -> Result<Vec<f64>> {
    if raw.is_empty() {
        return Ok(default_percentiles());
    }

    // Reject out-of-range values instead of silently clamping: a user writing
    // `5` for the 5th percentile would otherwise get the maximum.
    for &q in raw {
        if !q.is_finite() || !(0.0..=1.0).contains(&q) {
            return Err(Error::eval(format!(
                "Monte Carlo percentiles must be in [0, 1] (e.g. 0.05 for the \
                 5th percentile), got {q}"
            )));
        }
    }

    let mut v: Vec<f64> = raw.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    if let Some(pair) = v
        .windows(2)
        .find(|pair| pair[0].total_cmp(&pair[1]).is_eq())
    {
        return Err(Error::eval(format!(
            "Monte Carlo percentiles must be distinct; {} is requested more than once",
            pair[0]
        )));
    }
    Ok(v)
}

/// Totally ordered form of an [`EvalWarning`]; amounts compare by bit
/// pattern so identical NaN warnings from different paths are equal.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum WarningKey<'a> {
    DivisionByZero(&'a PeriodId, &'a str),
    NonFiniteValue(&'a PeriodId, &'a str, u64),
    CapitalStructure(&'a PeriodId, CapitalStructureKey<'a>),
    GappedSample(&'a PeriodId, &'a str, &'a str, usize),
    ForecastClipped(&'a PeriodId, &'a str, u64),
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum CapitalStructureKey<'a> {
    NegativeAvailableCashFloored(&'a str, u64),
    InterestShortfall(&'a str, u64),
    FeeShortfall(&'a str, u64),
    PrincipalShortfall(&'a str, u64),
}

impl<'a> WarningKey<'a> {
    fn of(warning: &'a EvalWarning) -> Self {
        match warning {
            EvalWarning::DivisionByZero { node_id, period } => {
                Self::DivisionByZero(period, node_id)
            }
            EvalWarning::NonFiniteValue {
                node_id,
                period,
                value,
            } => Self::NonFiniteValue(period, node_id, value.to_bits()),
            EvalWarning::CapitalStructure { period, warning } => {
                Self::CapitalStructure(period, CapitalStructureKey::of(warning))
            }
            EvalWarning::GappedSample {
                node_id,
                period,
                function,
                observations,
                ..
            } => Self::GappedSample(period, node_id, function, *observations),
            EvalWarning::ForecastClipped {
                node_id,
                period,
                published,
                ..
            } => Self::ForecastClipped(period, node_id, published.to_bits()),
        }
    }
}

impl<'a> CapitalStructureKey<'a> {
    fn of(warning: &'a CapitalStructureWarning) -> Self {
        match warning {
            CapitalStructureWarning::NegativeAvailableCashFloored { node_id, amount } => {
                Self::NegativeAvailableCashFloored(node_id, amount.to_bits())
            }
            CapitalStructureWarning::InterestShortfall {
                instrument_id,
                amount,
            } => Self::InterestShortfall(instrument_id, amount.to_bits()),
            CapitalStructureWarning::FeeShortfall {
                instrument_id,
                amount,
            } => Self::FeeShortfall(instrument_id, amount.to_bits()),
            CapitalStructureWarning::PrincipalShortfall {
                instrument_id,
                amount,
            } => Self::PrincipalShortfall(instrument_id, amount.to_bits()),
        }
    }
}

/// Period a warning was raised in.
fn warning_period(warning: &EvalWarning) -> &PeriodId {
    match warning {
        EvalWarning::DivisionByZero { period, .. }
        | EvalWarning::NonFiniteValue { period, .. }
        | EvalWarning::CapitalStructure { period, .. }
        | EvalWarning::GappedSample { period, .. }
        | EvalWarning::ForecastClipped { period, .. } => period,
    }
}

/// A sample without a value: `(metric index, period, path, reason)`. The
/// least one in this order is the one reported.
type Gap = (usize, PeriodId, u32, &'static str);

pub(crate) struct MonteCarloAccumulator {
    expected_paths: usize,
    observed_paths: usize,
    seed: u64,
    metrics: Vec<String>,
    value_types: Vec<crate::types::NodeValueType>,
    percentiles: Vec<f64>,
    forecast_periods: Vec<PeriodId>,
    forecast_set: HashSet<PeriodId>,
    path_values: IndexMap<String, IndexMap<PeriodId, Vec<(u32, f64)>>>,
    first_gap: Option<Gap>,
    gaps: usize,
    include_path_data: bool,
    warnings: Vec<EvalWarning>,
}

impl MonteCarloAccumulator {
    /// Validate the run's size and requests before anything is allocated.
    ///
    /// `value_types` are the resolved units of the model's nodes.
    pub(crate) fn new(
        model: &FinancialModelSpec,
        config: &MonteCarloConfig,
        value_types: &IndexMap<crate::types::NodeId, crate::types::NodeValueType>,
    ) -> Result<Self> {
        if config.n_paths == 0 {
            return Err(Error::eval(
                "Monte Carlo configuration requires n_paths > 0",
            ));
        }
        if u32::try_from(config.n_paths).is_err() {
            return Err(Error::eval(format!(
                "Monte Carlo configuration requires n_paths <= {}, the path-identifier domain; \
                 got {}",
                u32::MAX,
                config.n_paths
            )));
        }
        let forecast_periods: Vec<PeriodId> = model
            .periods
            .iter()
            .filter(|p| !p.is_actual)
            .map(|p| p.id)
            .collect();
        if forecast_periods.is_empty() {
            return Err(Error::eval(
                "Monte Carlo evaluation requires at least one forecast period. \
                 Use .periods(range, Some(actuals_cutoff)) to define forecast periods.",
            ));
        }
        if config.metrics.is_empty() {
            return Err(Error::eval(
                "Monte Carlo configuration requires `metrics`: list the nodes to report",
            ));
        }
        let mut seen = HashSet::new();
        let mut metric_types = Vec::with_capacity(config.metrics.len());
        for metric in &config.metrics {
            if !seen.insert(metric.as_str()) {
                return Err(Error::eval(format!(
                    "Monte Carlo metric '{metric}' is requested more than once"
                )));
            }
            let value_type = value_types.get(metric.as_str()).ok_or_else(|| {
                Error::eval(format!(
                    "Monte Carlo metric '{metric}' is not a node of the model"
                ))
            })?;
            metric_types.push(*value_type);
        }
        // Work and storage: one sample per path, metric and forecast period.
        let samples = config
            .n_paths
            .checked_mul(config.metrics.len())
            .and_then(|count| count.checked_mul(forecast_periods.len()))
            .filter(|count| *count <= MAX_MONTE_CARLO_SAMPLES)
            .ok_or_else(|| {
                Error::eval(format!(
                    "Monte Carlo run of {} paths x {} metrics x {} forecast periods exceeds \
                     the limit of {MAX_MONTE_CARLO_SAMPLES} samples; request fewer paths or \
                     metrics",
                    config.n_paths,
                    config.metrics.len(),
                    forecast_periods.len()
                ))
            })?;
        debug_assert!(samples > 0);

        Ok(Self {
            expected_paths: config.n_paths,
            observed_paths: 0,
            seed: config.seed,
            metrics: config.metrics.clone(),
            value_types: metric_types,
            percentiles: normalize_percentiles(&config.percentiles)?,
            forecast_set: forecast_periods.iter().copied().collect(),
            forecast_periods,
            path_values: IndexMap::new(),
            first_gap: None,
            gaps: 0,
            include_path_data: config.include_path_data,
            warnings: Vec::new(),
        })
    }

    /// The requested metrics, in order.
    pub(crate) fn metrics(&self) -> &[String] {
        &self.metrics
    }

    pub(crate) fn push_path(
        &mut self,
        path_idx: usize,
        path_results: IndexMap<String, IndexMap<PeriodId, PathSample>>,
        warnings: Vec<EvalWarning>,
    ) -> Result<()> {
        self.observed_paths += 1;
        let path = path_idx as u32;
        // Actual periods are not simulated: their warnings repeat the
        // deterministic evaluation's on every path.
        self.warnings.extend(
            warnings
                .into_iter()
                .filter(|warning| self.forecast_set.contains(warning_period(warning))),
        );

        // Exactly one sample per requested metric and forecast period.
        for index in 0..self.metrics.len() {
            let metric = self.metrics[index].clone();
            let samples = path_results.get(&metric);
            for period_index in 0..self.forecast_periods.len() {
                let period = &self.forecast_periods[period_index].clone();
                match samples.and_then(|samples| samples.get(period)) {
                    Some(PathSample::Value(value)) if value.is_finite() => {
                        self.path_values
                            .entry(metric.clone())
                            .or_default()
                            .entry(*period)
                            .or_default()
                            .push((path, *value));
                    }
                    Some(PathSample::Value(value)) => {
                        return Err(Error::eval(format!(
                            "Monte Carlo path {path} produced the invalid sample {value} for \
                             metric '{metric}' in period {period}"
                        )));
                    }
                    Some(PathSample::Absent(reason)) => {
                        self.note_gap((index, *period, path, reason));
                    }
                    None => self.note_gap((index, *period, path, "missing")),
                }
            }
        }

        Ok(())
    }

    fn note_gap(&mut self, gap: Gap) {
        self.gaps += 1;
        let earlier = self
            .first_gap
            .is_none_or(|held| (gap.0, gap.1, gap.2) < (held.0, held.1, held.2));
        if earlier {
            self.first_gap = Some(gap);
        }
    }

    pub(crate) fn finish(mut self) -> Result<MonteCarloResults> {
        if self.observed_paths != self.expected_paths {
            return Err(Error::eval(format!(
                "Monte Carlo aggregation mismatch: expected {} paths, got {}",
                self.expected_paths, self.observed_paths
            )));
        }

        // Rayon's reduce tree makes the merge order of partitions
        // nondeterministic, so sort serialized outputs (path-table rows and
        // warnings) into a canonical order: serial ≡ parallel must hold for
        // everything that reaches the wire.
        // Report each distinct warning once rather than once per path.
        self.warnings
            .sort_by(|lhs, rhs| WarningKey::of(lhs).cmp(&WarningKey::of(rhs)));
        self.warnings
            .dedup_by(|lhs, rhs| WarningKey::of(lhs) == WarningKey::of(rhs));

        // Statistics are published only for a complete run.
        if let Some((index, period, path, reason)) = self.first_gap {
            return Ok(MonteCarloResults {
                percentile_results: IndexMap::new(),
                incomplete: Some(MonteCarloIncomplete {
                    metric: self.metrics[index].clone(),
                    period,
                    path,
                    reason: reason.to_string(),
                    missing_samples: self.gaps,
                }),
                n_paths: self.observed_paths,
                seed: self.seed,
                stream_version: crate::forecast::stream::STREAM_VERSION,
                metrics: self.metrics,
                percentiles: self.percentiles,
                forecast_periods: self.forecast_periods,
                warnings: self.warnings,
                path_values: IndexMap::new(),
                path_data: None,
            });
        }

        let mut percentile_results: IndexMap<String, PercentileSeries> = IndexMap::new();
        for (metric, value_type) in self.metrics.iter().zip(&self.value_types) {
            let mut series = PercentileSeries {
                metric: metric.clone(),
                value_type: *value_type,
                sample_count: self.observed_paths,
                values: IndexMap::new(),
            };
            for period_id in &self.forecast_periods {
                let values = self
                    .path_values
                    .get(metric)
                    .and_then(|periods| periods.get(period_id))
                    .ok_or_else(|| {
                        Error::eval(format!(
                            "Monte Carlo aggregation has no samples of '{metric}' in {period_id}"
                        ))
                    })?;
                let mut sorted: Vec<f64> = values.iter().map(|(_, v)| *v).collect();
                sorted.sort_by(|a, b| a.total_cmp(b));
                let mut pairs: Vec<(f64, f64)> = Vec::with_capacity(self.percentiles.len());
                for &q in &self.percentiles {
                    let value = quantile_sorted(&sorted, q)?.ok_or_else(|| {
                        Error::eval("Monte Carlo quantile requested for empty sorted sample")
                    })?;
                    pairs.push((q, value));
                }
                series.values.insert(*period_id, pairs);
            }
            percentile_results.insert(metric.clone(), series);
        }

        let path_data = if !self.include_path_data {
            None
        } else {
            let mut rows = Vec::new();
            for (metric, period_values) in &self.path_values {
                for (period, values) in period_values {
                    for &(path, value) in values {
                        rows.push((path, metric.as_str(), period.to_string(), value));
                    }
                }
            }
            rows.sort_by(|a, b| (a.0, a.1, &a.2).cmp(&(b.0, b.1, &b.2)));
            let mut path_ids = Vec::with_capacity(rows.len());
            let mut periods = Vec::with_capacity(rows.len());
            let mut metrics = Vec::with_capacity(rows.len());
            let mut values = Vec::with_capacity(rows.len());
            for (path, metric, period, value) in rows {
                path_ids.push(path);
                periods.push(period);
                metrics.push(metric.to_string());
                values.push(value);
            }
            let mut metadata = IndexMap::new();
            metadata.insert("layout".to_string(), json!("long"));
            metadata.insert("source".to_string(), json!("statement_monte_carlo_paths"));
            Some(TableEnvelope::new(
                vec![
                    TableColumn::new("path_id", TableColumnData::UInt32(path_ids))
                        .with_role(TableColumnRole::Dimension),
                    TableColumn::new("period", TableColumnData::String(periods))
                        .with_role(TableColumnRole::Index),
                    TableColumn::new("metric", TableColumnData::String(metrics))
                        .with_role(TableColumnRole::Dimension),
                    TableColumn::new("value", TableColumnData::Float64(values))
                        .with_role(TableColumnRole::Measure),
                ],
                metadata,
            )?)
        };

        Ok(MonteCarloResults {
            percentile_results,
            incomplete: None,
            n_paths: self.observed_paths,
            seed: self.seed,
            stream_version: crate::forecast::stream::STREAM_VERSION,
            metrics: self.metrics,
            percentiles: self.percentiles,
            forecast_periods: self.forecast_periods,
            warnings: self.warnings,
            path_values: self.path_values,
            path_data,
        })
    }

    // Only the rayon `reduce` path calls `merge`; the wasm32 serial
    // accumulator folds in place, so this is dead code there.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) fn merge(mut self, other: Self) -> Result<Self> {
        if self.expected_paths != other.expected_paths
            || self.percentiles != other.percentiles
            || self.forecast_periods != other.forecast_periods
            || self.metrics != other.metrics
            || self.include_path_data != other.include_path_data
        {
            return Err(Error::eval(
                "Monte Carlo accumulator merge mismatch across parallel partitions",
            ));
        }

        self.observed_paths += other.observed_paths;
        self.warnings.extend(other.warnings);
        self.gaps += other.gaps;
        if let Some(gap) = other.first_gap {
            self.gaps -= 1;
            self.note_gap(gap);
        }

        for (metric, period_map) in other.path_values {
            let target_metric = self.path_values.entry(metric).or_default();
            for (period_id, values) in period_map {
                target_metric.entry(period_id).or_default().extend(values);
            }
        }

        Ok(self)
    }

    pub(crate) fn empty_like(&self) -> Self {
        Self {
            expected_paths: self.expected_paths,
            observed_paths: 0,
            seed: self.seed,
            metrics: self.metrics.clone(),
            value_types: self.value_types.clone(),
            percentiles: self.percentiles.clone(),
            forecast_periods: self.forecast_periods.clone(),
            forecast_set: self.forecast_set.clone(),
            path_values: IndexMap::new(),
            first_gap: None,
            gaps: 0,
            include_path_data: self.include_path_data,
            warnings: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_MONTE_CARLO_SAMPLES, MonteCarloAccumulator, MonteCarloConfig, MonteCarloResults,
        PathSample, normalize_percentiles, percentile_label,
    };
    use crate::builder::ModelBuilder;
    use crate::error::Result;
    use crate::types::{AmountOrScalar, FinancialModelSpec, NodeId, NodeValueType};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_core::money::Money;
    use indexmap::IndexMap;

    fn q(quarter: u8) -> PeriodId {
        PeriodId::quarter(2025, quarter).expect("quarter")
    }

    /// An actual first quarter and forecast second and third quarters, with a
    /// scalar `ratio` and a dollar `cash`.
    fn model() -> FinancialModelSpec {
        let mut builder = ModelBuilder::new("mc");
        builder
            .periods("2025Q1..Q3", Some("2025Q1"))
            .expect("periods");
        let scalar: Vec<_> = (1..=3)
            .map(|n| (q(n), AmountOrScalar::scalar(1.0)))
            .collect();
        builder.value("ratio", &scalar);
        let money: Vec<_> = (1..=3)
            .map(|n| (q(n), Money::from((100_i64, Currency::USD))))
            .collect();
        builder.value_money("cash", &money);
        builder.build().expect("model")
    }

    fn value_types() -> IndexMap<NodeId, NodeValueType> {
        IndexMap::from([
            (NodeId::new("ratio"), NodeValueType::Scalar),
            (
                NodeId::new("cash"),
                NodeValueType::Monetary {
                    currency: Currency::USD,
                },
            ),
        ])
    }

    fn accumulator(config: &MonteCarloConfig) -> Result<MonteCarloAccumulator> {
        MonteCarloAccumulator::new(&model(), config, &value_types())
    }

    /// One path's samples of `ratio`: its second- and third-quarter values.
    fn ratio_path(second: f64, third: f64) -> IndexMap<String, IndexMap<PeriodId, PathSample>> {
        IndexMap::from([(
            "ratio".to_string(),
            IndexMap::from([
                (q(2), PathSample::Value(second)),
                (q(3), PathSample::Value(third)),
            ]),
        )])
    }

    fn aggregate(config: &MonteCarloConfig, paths: &[(f64, f64)]) -> MonteCarloResults {
        let mut accumulator = accumulator(config).expect("valid run");
        for (path, (second, third)) in paths.iter().enumerate() {
            accumulator
                .push_path(path, ratio_path(*second, *third), Vec::new())
                .expect("valid sample");
        }
        accumulator.finish().expect("aggregation")
    }

    #[test]
    fn percentile_labels_are_percent_labels() {
        assert_eq!(percentile_label(0.05), "p5");
        assert_eq!(percentile_label(0.5), "p50");
        assert_eq!(percentile_label(0.975), "p97.5");
    }

    #[test]
    fn percentiles_are_sorted_distinct_and_in_range() {
        assert_eq!(
            normalize_percentiles(&[0.95, 0.05, 0.5]).expect("valid"),
            vec![0.05, 0.5, 0.95]
        );
        assert_eq!(
            normalize_percentiles(&[]).expect("default"),
            vec![0.05, 0.5, 0.95]
        );
        let error = normalize_percentiles(&[0.5, 0.05, 0.5])
            .expect_err("duplicate level")
            .to_string();
        assert!(error.contains("more than once"), "{error}");
        // `5` for the 5th percentile is an error, not a clamp to the maximum.
        for level in [5.0, -0.1, f64::NAN, f64::INFINITY] {
            assert!(normalize_percentiles(&[level]).is_err(), "{level}");
        }
    }

    /// Path count, metrics and the run's size are checked before a path runs.
    #[test]
    fn the_run_is_validated_before_any_path() {
        let rejected = |config: MonteCarloConfig| {
            accumulator(&config)
                .err()
                .expect("rejected configuration")
                .to_string()
        };
        assert!(rejected(MonteCarloConfig::new(0, 7, ["ratio"])).contains("n_paths > 0"));
        if let Ok(beyond) = usize::try_from(u64::from(u32::MAX) + 1) {
            assert!(
                rejected(MonteCarloConfig::new(beyond, 7, ["ratio"])).contains("path-identifier")
            );
        }
        assert!(rejected(MonteCarloConfig::new(4, 7, [""; 0])).contains("metrics"));
        assert!(rejected(MonteCarloConfig::new(4, 7, ["ghost"])).contains("'ghost'"));
        assert!(
            rejected(MonteCarloConfig::new(4, 7, ["ratio", "ratio"])).contains("more than once")
        );
        // Two forecast periods and two metrics: four samples a path.
        let within = MAX_MONTE_CARLO_SAMPLES / 4;
        accumulator(&MonteCarloConfig::new(within, 7, ["ratio", "cash"])).expect("at the limit");
        assert!(
            rejected(MonteCarloConfig::new(within + 1, 7, ["ratio", "cash"]))
                .contains("exceeds the limit")
        );

        let mut actuals_only = ModelBuilder::new("no-forecast");
        actuals_only
            .periods("2025Q1..Q2", Some("2025Q2"))
            .expect("periods");
        actuals_only.value(
            "ratio",
            &[
                (q(1), AmountOrScalar::scalar(1.0)),
                (q(2), AmountOrScalar::scalar(1.0)),
            ],
        );
        let error = MonteCarloAccumulator::new(
            &actuals_only.build().expect("model"),
            &MonteCarloConfig::new(4, 7, ["ratio"]),
            &value_types(),
        )
        .err()
        .expect("no forecast period")
        .to_string();
        assert!(error.contains("forecast period"), "{error}");
    }

    /// R-7 against the definition worked by hand: `h = (n - 1) q`, linear
    /// between the order statistics around `h`.
    #[test]
    fn quantiles_are_r7_with_units_and_sample_count() {
        let config = MonteCarloConfig::new(5, 7, ["ratio"])
            .with_percentiles(vec![0.0, 0.1, 0.25, 0.5, 0.9, 1.0]);
        // Second-quarter samples 3, 1, 4, 1, 5 sort to 1, 1, 3, 4, 5.
        let results = aggregate(
            &config,
            &[(3.0, 0.0), (1.0, 0.0), (4.0, 0.0), (1.0, 0.0), (5.0, 0.0)],
        );
        let series = &results.percentile_results["ratio"];
        assert_eq!(series.value_type, NodeValueType::Scalar);
        assert_eq!(series.sample_count, 5);
        let by_hand = [
            (0.0, 1.0),
            (0.1, 1.0),       // h = 0.4: between 1 and 1
            (0.25, 1.0),      // h = 1
            (0.5, 3.0),       // h = 2
            (0.9, 4.0 + 0.6), // h = 3.6: 60% of the way from 4 to 5
            (1.0, 5.0),
        ];
        for ((level, expected), (stored_level, value)) in by_hand.iter().zip(&series.values[&q(2)])
        {
            assert_eq!(level, stored_level);
            assert!((value - expected).abs() < 1e-12, "q={level}: {value}");
        }
        // Retrieval is by the exact probability key.
        assert_eq!(
            results.percentile_by_period("ratio", 0.9).expect("level")[&q(2)],
            4.6
        );
        assert!(results.percentile_by_period("ratio", 0.9000001).is_none());
        // The run replays from the result alone.
        assert_eq!(results.seed, 7);
        assert_eq!(
            results.stream_version,
            crate::forecast::stream::STREAM_VERSION
        );
        assert_eq!(results.metrics, vec!["ratio".to_string()]);
    }

    /// A sample without a value makes the run incomplete: no statistics, and
    /// the first gap in metric, period and path order is named.
    #[test]
    fn a_missing_sample_makes_the_run_incomplete() {
        let config = MonteCarloConfig::new(3, 7, ["ratio"]).with_path_data(true);
        let mut accumulator = accumulator(&config).expect("valid run");
        let gap = |reason: &'static str| {
            IndexMap::from([(
                "ratio".to_string(),
                IndexMap::from([
                    (q(2), PathSample::Value(1.0)),
                    (q(3), PathSample::Absent(reason)),
                ]),
            )])
        };
        // The later path arrives first.
        accumulator
            .push_path(2, gap("masked"), Vec::new())
            .expect("a gap is not an error");
        accumulator
            .push_path(1, gap("missing_peer_shock"), Vec::new())
            .expect("a gap is not an error");
        accumulator
            .push_path(0, ratio_path(1.0, 1.0), Vec::new())
            .expect("valid sample");
        let results = accumulator
            .finish()
            .expect("an incomplete run still returns");
        assert!(results.percentile_results.is_empty());
        assert!(results.path_data.is_none());
        let incomplete = results.incomplete.as_ref().expect("incomplete");
        assert_eq!(
            (
                incomplete.metric.as_str(),
                incomplete.period,
                incomplete.path
            ),
            ("ratio", q(3), 1)
        );
        assert_eq!(incomplete.reason, "missing_peer_shock");
        assert_eq!(incomplete.missing_samples, 2);
        assert_eq!(
            results
                .breach_probability("ratio", AmountOrScalar::scalar(0.0))
                .expect("typed threshold"),
            None
        );

        // A value that is not finite is an execution error.
        let mut accumulator = super::MonteCarloAccumulator::new(&model(), &config, &value_types())
            .expect("valid run");
        let error = accumulator
            .push_path(0, ratio_path(f64::NAN, 1.0), Vec::new())
            .expect_err("invalid sample")
            .to_string();
        assert!(error.contains("invalid sample"), "{error}");
    }

    /// The same paths give the same result whatever order they arrive in or
    /// however they are partitioned and merged.
    #[test]
    fn results_do_not_depend_on_path_order_or_partitioning() {
        let config = MonteCarloConfig::new(4, 7, ["ratio"]).with_path_data(true);
        let paths = [(1.0, 9.0), (2.0, 8.0), (3.0, 7.0), (4.0, 6.0)];
        let serial = aggregate(&config, &paths);

        let seed = accumulator(&config).expect("valid run");
        let mut left = seed.empty_like();
        let mut right = seed.empty_like();
        for path in [3, 1] {
            let (second, third) = paths[path];
            left.push_path(path, ratio_path(second, third), Vec::new())
                .expect("sample");
        }
        for path in [2, 0] {
            let (second, third) = paths[path];
            right
                .push_path(path, ratio_path(second, third), Vec::new())
                .expect("sample");
        }
        let merged = right.merge(left).expect("merge").finish().expect("finish");
        assert_eq!(
            serde_json::to_value(&serial).expect("serialize"),
            serde_json::to_value(&merged).expect("serialize")
        );
    }

    /// Breach probability counts a path once when any forecast value is
    /// strictly above a typed, finite threshold, and needs path samples.
    #[test]
    fn breach_probability_is_strict_typed_and_needs_path_samples() {
        let config = MonteCarloConfig::new(4, 7, ["ratio"]).with_path_data(true);
        // Path 0 breaches in both quarters and counts once; path 1 equals
        // the threshold, which is not a breach.
        let results = aggregate(&config, &[(6.0, 7.0), (5.0, 5.0), (1.0, 5.5), (1.0, 1.0)]);
        let breach =
            |results: &MonteCarloResults, threshold| results.breach_probability("ratio", threshold);
        assert_eq!(
            breach(&results, AmountOrScalar::scalar(5.0)).expect("typed"),
            Some(0.5)
        );
        // A threshold in other units, or not finite, is an error.
        assert!(
            breach(
                &results,
                AmountOrScalar::Amount(Money::from((5_i64, Currency::USD)))
            )
            .is_err()
        );
        assert!(breach(&results, AmountOrScalar::scalar(f64::NAN)).is_err());
        assert_eq!(
            results
                .breach_probability("cash", AmountOrScalar::scalar(5.0))
                .expect("unrequested metric"),
            None
        );

        // Restored from JSON: the path table gives the same answer.
        let wire = serde_json::to_value(&results).expect("serialize");
        let restored: MonteCarloResults = serde_json::from_value(wire.clone()).expect("restore");
        assert_eq!(
            breach(&restored, AmountOrScalar::scalar(5.0)).expect("typed"),
            Some(0.5)
        );
        // Without path samples it is unavailable.
        let mut without = wire;
        without.as_object_mut().expect("object").remove("path_data");
        let restored: MonteCarloResults = serde_json::from_value(without).expect("restore");
        assert_eq!(
            breach(&restored, AmountOrScalar::scalar(5.0)).expect("typed"),
            None
        );
        // An incomplete or repeated key set cannot be restored.
        let truncated = aggregate(
            &MonteCarloConfig::new(3, 7, ["ratio"]).with_path_data(true),
            &[(6.0, 7.0), (5.0, 5.0), (1.0, 5.5)],
        );
        let mut short: MonteCarloResults =
            serde_json::from_value(serde_json::to_value(&truncated).expect("serialize"))
                .expect("restore");
        short.n_paths = 4;
        let error = breach(&short, AmountOrScalar::scalar(5.0))
            .expect_err("incomplete key set")
            .to_string();
        assert!(error.contains("complete run"), "{error}");
    }

    #[test]
    fn config_wire_form_is_exact_and_closed() {
        let config: MonteCarloConfig = serde_json::from_value(serde_json::json!({
            "n_paths": 16,
            "seed": "18446744073709551615",
            "metrics": ["ratio"]
        }))
        .expect("config");
        assert_eq!(config.seed, u64::MAX);
        assert_eq!(config.percentiles, vec![0.05, 0.5, 0.95]);
        assert!(!config.include_path_data);
        assert_eq!(
            serde_json::to_value(&config).expect("serialize")["seed"],
            serde_json::json!("18446744073709551615")
        );
        for invalid in [
            serde_json::json!({"n_paths": 16, "seed": "7"}),
            serde_json::json!({"n_paths": 16, "seed": 7, "metrics": ["ratio"]}),
            serde_json::json!({"n_paths": 16, "seed": "-7", "metrics": ["ratio"]}),
            serde_json::json!({"n_paths": 16, "seed": "7", "metrics": ["ratio"], "extra": 1}),
        ] {
            assert!(serde_json::from_value::<MonteCarloConfig>(invalid).is_err());
        }
    }

    #[test]
    fn parallel_monte_carlo_reports_the_lowest_failing_path() {
        // y = sqrt(x) with x ~ N(0.5, 1): many paths fail. The error must
        // name the lowest failing path's on every run, the same error the
        // serial (wasm32) loop returns.
        use crate::evaluator::Evaluator;
        use crate::types::ForecastSpec;

        let model = {
            let mut model_builder = ModelBuilder::new("mc-first-error");
            model_builder
                .periods("2025Q1..2026Q4", Some("2025Q1"))
                .expect("valid periods");
            let mut model_builder = model_builder.mixed("x");
            model_builder.values(&[(q(1), AmountOrScalar::scalar(1.0))]);
            model_builder.forecast(ForecastSpec::normal(0.5, 1.0, 7));
            let mut model_builder = model_builder.build().expect("valid mixed node");
            model_builder
                .compute("y", "sqrt(x)")
                .expect("valid formula");
            model_builder.build().expect("valid model")
        };

        // Serial reference: path results do not depend on `n_paths`, so the
        // smallest `n` that fails isolates the lowest failing path, whose error
        // is then the only one.
        let serial_error = (1..=64)
            .find_map(|n| {
                Evaluator::new()
                    .evaluate_monte_carlo(&model, &MonteCarloConfig::new(n, 42, ["y"]), None)
                    .err()
            })
            .expect("some early path must fail")
            .to_string();

        let config = MonteCarloConfig::new(2_000, 42, ["y"]);
        for _ in 0..20 {
            let error = Evaluator::new()
                .evaluate_monte_carlo(&model, &config, None)
                .expect_err("failing paths must fail the run");
            assert_eq!(error.to_string(), serial_error);
        }
    }
}

//! Python wrappers for statement-model Monte Carlo configuration and results.
//!
//! The simulation itself is started from ``Evaluator.evaluate_monte_carlo``
//! (see `evaluator.rs`), mirroring Rust's `Evaluator::evaluate_monte_carlo`.

use crate::bindings::pandas_utils::{dict_to_dataframe, table_to_dataframe};
use crate::errors::serde_json_to_py;
use finstack_quant_statements::evaluator::{
    MonteCarloConfig as RustMonteCarloConfig, MonteCarloResults as RustMonteCarloResults,
};
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

/// Configuration for statement-model Monte Carlo evaluation.
#[pyclass(
    name = "MonteCarloConfig",
    module = "finstack_quant.statements",
    from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyMonteCarloConfig {
    pub(crate) inner: RustMonteCarloConfig,
}

#[pymethods]
impl PyMonteCarloConfig {
    /// Configure a Monte Carlo run over a statement model's forecast periods.
    ///
    /// Parameters
    /// ----------
    /// n_paths : int
    ///     Number of simulated paths. Percentile standard error scales as
    ///     ``1 / sqrt(n_paths)``, so tails need proportionally more paths:
    ///     roughly 1 000–2 000 for means/medians, 5 000–10 000 for the 5th and
    ///     95th percentiles, and 10 000+ for 1st/99th percentiles, CVaR, or
    ///     breach probabilities. No minimum is imposed.
    /// seed : int
    ///     Base seed from which per-path and per-node seeds are derived. The
    ///     same seed reproduces the run exactly, in serial and in parallel.
    /// percentiles : list[float] | None
    ///     Percentiles to compute, each a **decimal fraction in [0, 1]** —
    ///     ``0.05`` is the 5th percentile, not ``5``. Stored as given; the
    ///     engine sorts, deduplicates and range-checks them when
    ///     ``Evaluator.evaluate_monte_carlo`` starts, which raises
    ///     ``ValueError`` for a value that is not finite or lies outside
    ///     [0, 1]. ``None`` or an empty list uses the engine default
    ///     ``[0.05, 0.5, 0.95]``.
    /// include_path_data : bool
    ///     Whether to retain the full per-path long table. Off by default
    ///     because it grows as ``n_paths * metrics * forecast periods``.
    #[new]
    #[pyo3(signature = (n_paths, seed, percentiles=None, include_path_data=false))]
    fn new(
        n_paths: usize,
        seed: u64,
        percentiles: Option<Vec<f64>>,
        include_path_data: bool,
    ) -> Self {
        let mut inner = RustMonteCarloConfig::new(n_paths, seed);
        if let Some(percentiles) = percentiles {
            inner = inner.with_percentiles(percentiles);
        }
        inner = inner.with_path_data(include_path_data);
        Self { inner }
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a configuration from its canonical JSON form.
    ///
    /// Unknown fields are rejected, so a mistyped key fails loudly instead of
    /// silently falling back to a default.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid MonteCarloConfig JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this configuration to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize MonteCarloConfig"))
    }

    /// Number of paths the simulation will draw.
    #[getter]
    fn n_paths(&self) -> usize {
        self.inner.n_paths
    }

    /// Base seed from which per-path and per-node seeds are derived.
    ///
    /// Reusing a seed reproduces the run bit-for-bit; serial and parallel
    /// execution agree.
    #[getter]
    fn seed(&self) -> u64 {
        self.inner.seed
    }

    /// Percentiles to compute, as decimal fractions in [0, 1].
    ///
    /// Returns
    /// -------
    /// list[float]
    ///     Quantile levels exactly as supplied (not yet normalized) —
    ///     ``0.05`` is the 5th percentile, not ``5``. See
    ///     ``MonteCarloResults.percentiles`` for the sorted, deduplicated
    ///     levels a run computed.
    #[getter]
    fn percentiles(&self) -> Vec<f64> {
        self.inner.percentiles.clone()
    }

    /// Whether the run retains the full per-path table alongside percentiles.
    #[getter]
    fn include_path_data(&self) -> bool {
        self.inner.include_path_data
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("MonteCarloConfig", &self.inner)
    }
}

/// Column label for one percentile level: ``0.05`` → ``p5``, ``0.5`` →
/// ``p50``, ``0.975`` → ``p97.5``.
fn percentile_column(quantile: f64) -> String {
    let pct = quantile * 100.0;
    let rounded = pct.round();
    if (pct - rounded).abs() < 1e-9 {
        format!("p{}", rounded as i64)
    } else {
        format!("p{pct}")
    }
}

/// Typed results for statement-model Monte Carlo evaluation.
#[pyclass(
    name = "MonteCarloResults",
    module = "finstack_quant.statements",
    from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyMonteCarloResults {
    pub(crate) inner: RustMonteCarloResults,
}

#[pymethods]
impl PyMonteCarloResults {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize results from their canonical JSON form.
    ///
    /// Per-path detail is only present when the producing run set
    /// ``include_path_data``; the internal path buffer used by
    /// ``breach_probability`` is never serialized, so a round-tripped result
    /// carries percentiles (and the optional path table) but not that buffer.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid MonteCarloResults JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize these results to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize MonteCarloResults"))
    }

    /// Number of paths actually simulated.
    ///
    /// The aggregator fails the run unless this equals the configured
    /// ``n_paths``, so a partial simulation never reaches a result.
    #[getter]
    fn n_paths(&self) -> usize {
        self.inner.n_paths
    }

    /// Percentiles computed for every metric and period.
    ///
    /// Returns
    /// -------
    /// list[float]
    ///     Sorted, deduplicated quantile levels as decimal fractions in
    ///     [0, 1] — ``0.5`` is the median.
    #[getter]
    fn percentiles(&self) -> Vec<f64> {
        self.inner.percentiles.clone()
    }

    /// Forecast periods covered by the simulation, in evaluation order.
    ///
    /// Only non-actual periods are simulated, so historical periods are
    /// absent from this list and from every percentile series.
    ///
    /// Returns
    /// -------
    /// list[str]
    ///     Period identifier strings (e.g. ``"2025Q1"``).
    #[getter]
    fn forecast_periods(&self) -> Vec<String> {
        self.inner
            .forecast_periods
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Metric (node) identifiers that were simulated, in model order.
    #[getter]
    fn metrics(&self) -> Vec<String> {
        self.inner.percentile_results.keys().cloned().collect()
    }

    /// Warnings raised while evaluating paths, in their serde form.
    ///
    /// One dict per ``EvalWarning`` with a single snake_case variant key
    /// (``"division_by_zero"``, ``"non_finite_value"``, ...) whose value holds
    /// the variant fields (periods as ``"2025Q1"``-style ids, non-finite
    /// numbers as ``"nan"`` / ``"inf"`` / ``"-inf"``), identical to the WASM
    /// ``Evaluator.evaluateMonteCarlo`` result. Empty when every path evaluated cleanly.
    #[getter]
    fn warnings<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.warnings)
    }

    /// Look up one percentile of one metric as a period-keyed dict.
    ///
    /// Parameters
    /// ----------
    /// metric : str
    ///     Node identifier whose simulated distribution to read.
    /// percentile : float
    ///     Quantile as a decimal fraction in [0, 1] (``0.95`` for the 95th
    ///     percentile). It must match a configured percentile to within
    ///     1e-12; nothing is interpolated between configured levels.
    ///
    /// Returns
    /// -------
    /// dict[str, float] | None
    ///     Period identifier → percentile value, in forecast-period order.
    ///     ``None`` when the metric is unknown or the percentile was not
    ///     configured for this run.
    fn percentile_by_period<'py>(
        &self,
        py: Python<'py>,
        metric: &str,
        percentile: f64,
    ) -> PyResult<Option<Bound<'py, PyDict>>> {
        let Some(values) = self.inner.percentile_by_period(metric, percentile) else {
            return Ok(None);
        };
        let series = PyDict::new(py);
        for (period, value) in values {
            series.set_item(period.to_string(), value)?;
        }
        Ok(Some(series))
    }

    /// Probability that a metric exceeds ``threshold`` in **any** forecast
    /// period, estimated across paths.
    ///
    /// Counts upside breaches only (``value > threshold``); for a downside
    /// test (DSCR below a floor) simulate a negated metric or a derived node
    /// that flips the sign.
    ///
    /// Parameters
    /// ----------
    /// metric : str
    ///     Simulated node identifier.
    /// threshold : float
    ///     Breach level in the metric's own units.
    ///
    /// Returns
    /// -------
    /// float | None
    ///     Fraction of paths in [0, 1] with at least one breach, or ``None``
    ///     when the metric was not simulated, there are no forecast periods,
    ///     the path buffer is incomplete, or the result was reconstructed
    ///     from JSON / pickle (the per-path buffer is not serialized).
    #[pyo3(text_signature = "($self, metric, threshold)")]
    fn breach_probability(&self, metric: &str, threshold: f64) -> Option<f64> {
        self.inner.breach_probability(metric, threshold)
    }

    /// Export one metric's percentile fan as a pandas ``DataFrame``.
    ///
    /// Rows are the metric's forecast periods (index name ``period``, in
    /// evaluation order); each column holds one configured percentile of the
    /// simulated distribution, in the metric's own units (currency amounts
    /// for monetary nodes, unitless otherwise).
    ///
    /// Columns: one per configured percentile, named ``p<percent>`` —
    /// ``p5``, ``p50``, ``p95`` for the default levels (``p97.5`` for
    /// ``0.975``). A cell is ``NaN`` when a period is missing that
    /// percentile.
    ///
    /// Parameters
    /// ----------
    /// metric : str
    ///     Node identifier whose fan chart to build.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``metric`` was not simulated.
    #[pyo3(text_signature = "($self, metric)")]
    fn to_dataframe<'py>(&self, py: Python<'py>, metric: &str) -> PyResult<Bound<'py, PyAny>> {
        let series = self.inner.percentile_results.get(metric).ok_or_else(|| {
            PyKeyError::new_err(format!("unknown Monte Carlo metric: {metric:?}"))
        })?;

        let periods: Vec<String> = series.values.keys().map(ToString::to_string).collect();
        let columns = PyDict::new(py);
        for &quantile in &self.inner.percentiles {
            let percentile_values = self.inner.percentile_by_period(metric, quantile);
            let column: Vec<f64> = series
                .values
                .keys()
                .map(|period| {
                    percentile_values
                        .as_ref()
                        .and_then(|values| values.get(period))
                        .copied()
                        .unwrap_or(f64::NAN)
                })
                .collect();
            columns.set_item(percentile_column(quantile), column)?;
        }

        let index = PyList::new(py, &periods)?;
        let df = dict_to_dataframe(py, &columns, Some(index.into_any()))?;
        df.getattr("index")?.setattr("name", "period")?;
        Ok(df)
    }

    /// Export the retained per-path values as a long-format pandas
    /// ``DataFrame``.
    ///
    /// Populated only when the run was configured with
    /// ``include_path_data=True``; otherwise the frame is empty but still
    /// carries the documented columns. Rows are ordered canonically by
    /// ``(path_id, metric, period)`` so serial and parallel runs match.
    ///
    /// Columns: ``path_id`` (0-based path index), ``period`` (period
    /// identifier string), ``metric`` (node identifier), ``value``
    /// (float64, in the metric's own units).
    #[pyo3(text_signature = "($self)")]
    fn to_paths_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.inner.path_data {
            Some(table) => table_to_dataframe(py, table),
            None => {
                let columns = PyDict::new(py);
                columns.set_item("path_id", Vec::<u32>::new())?;
                columns.set_item("period", Vec::<String>::new())?;
                columns.set_item("metric", Vec::<String>::new())?;
                columns.set_item("value", Vec::<f64>::new())?;
                dict_to_dataframe(py, &columns, None)
            }
        }
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("MonteCarloResults", &self.inner)
    }
}

/// Accept a typed ``MonteCarloConfig`` or its JSON serialization.
pub(crate) fn extract_config(value: &Bound<'_, PyAny>) -> PyResult<RustMonteCarloConfig> {
    if let Ok(config) = value.extract::<PyRef<'_, PyMonteCarloConfig>>() {
        return Ok(config.inner.clone());
    }
    serde_json::from_str(value.extract::<&str>()?)
        .map_err(|e| serde_json_to_py(e, "invalid MonteCarloConfig JSON"))
}

pub fn register(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyMonteCarloConfig>()?;
    module.add_class::<PyMonteCarloResults>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::percentile_column;

    #[test]
    fn percentile_columns_are_percent_labels() {
        assert_eq!(percentile_column(0.05), "p5");
        assert_eq!(percentile_column(0.5), "p50");
        assert_eq!(percentile_column(0.95), "p95");
        assert_eq!(percentile_column(0.975), "p97.5");
        assert_eq!(percentile_column(0.01), "p1");
    }
}

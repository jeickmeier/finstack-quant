//! Typed Python wrappers for statement-analysis configs and root results.

use crate::bindings::statements::evaluator::PyStatementResult;
use crate::errors::display_to_py;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::evaluator::{
    MonteCarloConfig as RustMonteCarloConfig, MonteCarloResults as RustMonteCarloResults,
    StatementResult,
};
use finstack_quant_statements_analytics::analysis::{
    ParameterSpec, ScenarioDefinition, ScenarioResults, ScenarioSet as RustScenarioSet,
    SensitivityConfig as RustSensitivityConfig, SensitivityMode,
    SensitivityResult as RustSensitivityResult, VarianceConfig as RustVarianceConfig,
    VarianceReport as RustVarianceReport, VarianceRow as RustVarianceRow,
};
use indexmap::IndexMap;
use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

type SensitivityParameterInput = (String, String, f64, Vec<f64>);

fn parse_period(period: &str) -> PyResult<PeriodId> {
    period.parse().map_err(display_to_py)
}

fn parse_sensitivity_mode(mode: &str) -> PyResult<SensitivityMode> {
    match mode {
        "Diagonal" | "diagonal" => Ok(SensitivityMode::Diagonal),
        "FullGrid" | "full_grid" => Ok(SensitivityMode::FullGrid),
        "Tornado" | "tornado" => Ok(SensitivityMode::Tornado),
        _ => Err(PyValueError::new_err(format!(
            "unknown sensitivity mode '{mode}'; expected Diagonal, FullGrid, or Tornado"
        ))),
    }
}

fn sensitivity_mode_name(mode: SensitivityMode) -> &'static str {
    match mode {
        SensitivityMode::Diagonal => "Diagonal",
        SensitivityMode::FullGrid => "FullGrid",
        SensitivityMode::Tornado => "Tornado",
    }
}

fn extract_overrides(value: &Bound<'_, PyAny>) -> PyResult<IndexMap<String, f64>> {
    let values = value.cast::<PyDict>()?;
    let mut overrides = IndexMap::with_capacity(values.len());
    for (node_id, value) in values.iter() {
        overrides.insert(node_id.extract()?, value.extract()?);
    }
    Ok(overrides)
}

/// Configuration for statement sensitivity analysis.
#[pyclass(
    name = "SensitivityConfig",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PySensitivityConfig {
    pub(crate) inner: RustSensitivityConfig,
}

#[pymethods]
impl PySensitivityConfig {
    #[new]
    #[pyo3(signature = (mode, parameters=Vec::new(), target_metrics=Vec::new()))]
    fn new(
        mode: &str,
        parameters: Vec<SensitivityParameterInput>,
        target_metrics: Vec<String>,
    ) -> PyResult<Self> {
        let parameters = parameters
            .into_iter()
            .map(|(node_id, period, base_value, perturbations)| {
                Ok(ParameterSpec::new(
                    node_id,
                    parse_period(&period)?,
                    base_value,
                    perturbations,
                ))
            })
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            inner: RustSensitivityConfig {
                mode: parse_sensitivity_mode(mode)?,
                parameters,
                target_metrics,
            },
        })
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn mode(&self) -> &'static str {
        sensitivity_mode_name(self.inner.mode)
    }

    #[getter]
    fn target_metrics(&self) -> Vec<String> {
        self.inner.target_metrics.clone()
    }

    #[getter]
    fn parameter_count(&self) -> usize {
        self.inner.parameters.len()
    }
}

/// Configuration for comparing two statement results.
#[pyclass(
    name = "VarianceConfig",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyVarianceConfig {
    pub(crate) inner: RustVarianceConfig,
}

#[pymethods]
impl PyVarianceConfig {
    #[new]
    fn new(
        baseline_label: &str,
        comparison_label: &str,
        metrics: Vec<String>,
        periods: Vec<String>,
    ) -> PyResult<Self> {
        let periods = periods
            .iter()
            .map(|period| parse_period(period))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            inner: RustVarianceConfig::new(baseline_label, comparison_label, metrics, periods),
        })
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn baseline_label(&self) -> &str {
        &self.inner.baseline_label
    }

    #[getter]
    fn comparison_label(&self) -> &str {
        &self.inner.comparison_label
    }

    #[getter]
    fn metrics(&self) -> Vec<String> {
        self.inner.metrics.clone()
    }

    #[getter]
    fn periods(&self) -> Vec<String> {
        self.inner.periods.iter().map(ToString::to_string).collect()
    }
}

/// Named scenario definitions for statement-model evaluation.
#[pyclass(
    name = "ScenarioSet",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyScenarioSet {
    pub(crate) inner: RustScenarioSet,
}

#[pymethods]
impl PyScenarioSet {
    #[new]
    #[pyo3(signature = (scenarios, parents=None, model_ids=None))]
    fn new(
        scenarios: &Bound<'_, PyDict>,
        parents: Option<&Bound<'_, PyDict>>,
        model_ids: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut definitions = IndexMap::with_capacity(scenarios.len());
        for (name, overrides) in scenarios.iter() {
            let name = name.extract::<String>()?;
            let overrides = extract_overrides(&overrides)?;
            let parent = parents
                .and_then(|items| items.get_item(&name).transpose())
                .transpose()?
                .map(|value| value.extract::<String>())
                .transpose()?;
            let model_id = model_ids
                .and_then(|items| items.get_item(&name).transpose())
                .transpose()?
                .map(|value| value.extract::<String>())
                .transpose()?;
            definitions.insert(
                name,
                ScenarioDefinition {
                    model_id,
                    parent,
                    overrides,
                },
            );
        }
        Ok(Self {
            inner: RustScenarioSet {
                scenarios: definitions,
            },
        })
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn names(&self) -> Vec<String> {
        self.inner.scenarios.keys().cloned().collect()
    }
}

/// Configuration for statement-model Monte Carlo evaluation.
#[pyclass(
    name = "MonteCarloConfig",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyMonteCarloConfig {
    pub(crate) inner: RustMonteCarloConfig,
}

#[pymethods]
impl PyMonteCarloConfig {
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

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn n_paths(&self) -> usize {
        self.inner.n_paths
    }

    #[getter]
    fn seed(&self) -> u64 {
        self.inner.seed
    }

    #[getter]
    fn percentiles(&self) -> Vec<f64> {
        self.inner.percentiles.clone()
    }

    #[getter]
    fn include_path_data(&self) -> bool {
        self.inner.include_path_data
    }
}

/// Typed root result for statement sensitivity analysis.
#[pyclass(
    name = "SensitivityResult",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PySensitivityResult {
    pub(crate) inner: RustSensitivityResult,
}

#[pymethods]
impl PySensitivityResult {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    #[getter]
    fn target_metrics(&self) -> Vec<String> {
        self.inner.config.target_metrics.clone()
    }

    fn get_parameter_value(&self, scenario_index: usize, parameter: &str) -> PyResult<Option<f64>> {
        let scenario = self
            .inner
            .scenarios
            .get(scenario_index)
            .ok_or_else(|| PyIndexError::new_err("scenario index out of range"))?;
        Ok(scenario.parameter_values.get(parameter).copied())
    }

    fn get_value(
        &self,
        scenario_index: usize,
        node_id: &str,
        period: &str,
    ) -> PyResult<Option<f64>> {
        let scenario = self
            .inner
            .scenarios
            .get(scenario_index)
            .ok_or_else(|| PyIndexError::new_err("scenario index out of range"))?;
        Ok(scenario.results.get(node_id, &parse_period(period)?))
    }
}

/// One typed variance-report row.
#[pyclass(
    name = "VarianceRow",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyVarianceRow {
    inner: RustVarianceRow,
}

#[pymethods]
impl PyVarianceRow {
    #[getter]
    fn period(&self) -> String {
        self.inner.period.to_string()
    }

    #[getter]
    fn metric(&self) -> &str {
        &self.inner.metric
    }

    #[getter]
    fn baseline(&self) -> f64 {
        self.inner.baseline
    }

    #[getter]
    fn comparison(&self) -> f64 {
        self.inner.comparison
    }

    #[getter]
    fn abs_var(&self) -> f64 {
        self.inner.abs_var
    }

    #[getter]
    fn pct_var(&self) -> Option<f64> {
        self.inner.pct_var
    }
}

/// Typed root variance report.
#[pyclass(
    name = "VarianceReport",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyVarianceReport {
    pub(crate) inner: RustVarianceReport,
}

#[pymethods]
impl PyVarianceReport {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn baseline_label(&self) -> &str {
        &self.inner.baseline_label
    }

    #[getter]
    fn comparison_label(&self) -> &str {
        &self.inner.comparison_label
    }

    #[getter]
    fn rows(&self) -> Vec<PyVarianceRow> {
        self.inner
            .rows
            .iter()
            .cloned()
            .map(|inner| PyVarianceRow { inner })
            .collect()
    }
}

/// Typed evaluated results for a set of named scenarios.
#[pyclass(
    name = "ScenarioResultSet",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyScenarioResultSet {
    pub(crate) inner: ScenarioResults,
}

#[pymethods]
impl PyScenarioResultSet {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let scenarios: IndexMap<String, StatementResult> =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self {
            inner: ScenarioResults { scenarios },
        })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.scenarios).map_err(display_to_py)
    }

    #[getter]
    fn names(&self) -> Vec<String> {
        self.inner.scenarios.keys().cloned().collect()
    }

    fn get(&self, name: &str) -> Option<PyStatementResult> {
        self.inner
            .scenarios
            .get(name)
            .cloned()
            .map(|inner| PyStatementResult { inner })
    }
}

/// Typed root results for statement-model Monte Carlo evaluation.
#[pyclass(
    name = "MonteCarloResults",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyMonteCarloResults {
    pub(crate) inner: RustMonteCarloResults,
}

#[pymethods]
impl PyMonteCarloResults {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn n_paths(&self) -> usize {
        self.inner.n_paths
    }

    #[getter]
    fn percentiles(&self) -> Vec<f64> {
        self.inner.percentiles.clone()
    }

    #[getter]
    fn forecast_periods(&self) -> Vec<String> {
        self.inner
            .forecast_periods
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn get_percentile_series<'py>(
        &self,
        py: Python<'py>,
        metric: &str,
        percentile: f64,
    ) -> PyResult<Option<Bound<'py, PyDict>>> {
        let Some(values) = self.inner.get_percentile_series(metric, percentile) else {
            return Ok(None);
        };
        let series = PyDict::new(py);
        for (period, value) in values {
            series.set_item(period.to_string(), value)?;
        }
        Ok(Some(series))
    }
}

pub fn register(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PySensitivityConfig>()?;
    module.add_class::<PyVarianceConfig>()?;
    module.add_class::<PyScenarioSet>()?;
    module.add_class::<PyMonteCarloConfig>()?;
    module.add_class::<PySensitivityResult>()?;
    module.add_class::<PyVarianceRow>()?;
    module.add_class::<PyVarianceReport>()?;
    module.add_class::<PyScenarioResultSet>()?;
    module.add_class::<PyMonteCarloResults>()?;
    Ok(())
}

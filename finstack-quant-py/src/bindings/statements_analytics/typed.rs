//! Typed Python wrappers for statement-analysis configs and root results.

use crate::bindings::core::money::PyMoney;
use crate::bindings::pandas_utils::{
    serde_rows_to_dataframe_with_schema, table_to_dataframe, ColumnSchema,
};
use crate::bindings::statements::evaluator::PyStatementResult;
use crate::errors::{core_to_py, display_to_py, serde_json_to_py};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::types::AmountOrScalar;
use finstack_quant_statements_analytics::analysis::{
    BridgeChart as RustBridgeChart, BridgeStep as RustBridgeStep,
    ParameterSpec as RustParameterSpec, ScenarioDefinition, ScenarioDiff as RustScenarioDiff,
    ScenarioResults, ScenarioSet as RustScenarioSet, SensitivityConfig as RustSensitivityConfig,
    SensitivityMode, SensitivityResult as RustSensitivityResult, TornadoEntry as RustTornadoEntry,
    VarianceConfig as RustVarianceConfig, VarianceReport as RustVarianceReport,
    VarianceRow as RustVarianceRow,
};
use indexmap::IndexMap;
use pyo3::exceptions::PyIndexError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

type SensitivityParameterInput = (String, String, f64, Vec<f64>);

/// Column schema for [`PyVarianceReport::to_dataframe`].
const VARIANCE_ROW_COLUMNS: [ColumnSchema<'static>; 7] = [
    ("period", "str"),
    ("metric", "str"),
    ("baseline", "float64"),
    ("comparison", "float64"),
    ("abs_var", "float64"),
    ("pct_var", "float64"),
    ("driver_contribution", "object"),
];

/// Column schema for the fixed part of [`PySensitivityResult::to_dataframe`];
/// one column per perturbed parameter is inserted after ``scenario``.
const SENSITIVITY_TAIL_COLUMNS: [ColumnSchema<'static>; 3] =
    [("node_id", "str"), ("period", "str"), ("value", "float64")];

/// Column schema for [`PyBridgeChart::to_dataframe`].
const BRIDGE_STEP_COLUMNS: [ColumnSchema<'static>; 2] =
    [("driver", "str"), ("contribution", "float64")];

fn parse_period(period: &str) -> PyResult<PeriodId> {
    period.parse().map_err(display_to_py)
}

/// Parse the serde name of a [`SensitivityMode`] (`"diagonal"`, `"full_grid"`, `"tornado"`).
fn parse_sensitivity_mode(mode: &str) -> PyResult<SensitivityMode> {
    finstack_quant_core::wire::serde_parse(mode).map_err(core_to_py)
}

/// Serde name of a [`SensitivityMode`]; identical to the `to_json` form.
fn sensitivity_mode_name(mode: SensitivityMode) -> PyResult<String> {
    finstack_quant_core::wire::serde_label(&mode).map_err(core_to_py)
}

fn extract_amount_or_scalar(value: &Bound<'_, PyAny>) -> PyResult<AmountOrScalar> {
    if let Ok(money) = value.extract::<PyRef<'_, PyMoney>>() {
        return Ok(AmountOrScalar::Amount(money.inner));
    }
    Ok(AmountOrScalar::scalar(value.extract()?))
}

/// Split a scenario's ``{node: value | {period: value}}`` dict into the
/// model-wide and per-period override maps of a `ScenarioDefinition`.
type OverrideMaps = (
    IndexMap<String, AmountOrScalar>,
    IndexMap<String, IndexMap<PeriodId, AmountOrScalar>>,
);

fn extract_overrides(value: &Bound<'_, PyAny>) -> PyResult<OverrideMaps> {
    let values = value.cast::<PyDict>()?;
    let mut overrides = IndexMap::with_capacity(values.len());
    let mut period_overrides: IndexMap<String, IndexMap<PeriodId, AmountOrScalar>> =
        IndexMap::new();
    for (node_id, value) in values.iter() {
        let node_id: String = node_id.extract()?;
        if let Ok(by_period) = value.cast::<PyDict>() {
            let entry = period_overrides.entry(node_id).or_default();
            for (period, value) in by_period.iter() {
                let period = parse_period(&period.extract::<String>()?)?;
                entry.insert(period, extract_amount_or_scalar(&value)?);
            }
        } else {
            overrides.insert(node_id, extract_amount_or_scalar(&value)?);
        }
    }
    Ok((overrides, period_overrides))
}

/// One parameter to vary in a sensitivity run.
///
/// Parameters
/// ----------
/// node_id : str
///     Node identifier to perturb.
/// period : str
///     Period-id string of the perturbed value (e.g. ``"2025Q2"``).
/// base_value : float
///     Unperturbed value, recorded for reference.
/// perturbations : list[float]
///     Absolute replacement values applied one at a time.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import ParameterSpec
/// >>> ParameterSpec.with_percentages("revenue", "2025Q2", 100.0, [-10.0, 10.0]).perturbations
/// [90.0, 110.0]
#[pyclass(
    name = "ParameterSpec",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyParameterSpec {
    pub(crate) inner: RustParameterSpec,
}

#[pymethods]
impl PyParameterSpec {
    #[new]
    #[pyo3(text_signature = "(node_id, period, base_value, perturbations)")]
    fn new(
        node_id: &str,
        period: &str,
        base_value: f64,
        perturbations: Vec<f64>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: RustParameterSpec::new(
                node_id,
                parse_period(period)?,
                base_value,
                perturbations,
            ),
        })
    }

    /// Build a spec whose perturbations are ``base_value * (1 + pct / 100)``.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier to perturb.
    /// period : str
    ///     Period-id string of the perturbed value.
    /// base_value : float
    ///     Unperturbed value the percentages are applied to.
    /// pct_range : list[float]
    ///     Percentage bumps (``[-10.0, 0.0, 10.0]`` = -10%, 0%, +10%).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``period`` does not parse.
    #[staticmethod]
    #[pyo3(text_signature = "(node_id, period, base_value, pct_range)")]
    fn with_percentages(
        node_id: &str,
        period: &str,
        base_value: f64,
        pct_range: Vec<f64>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: RustParameterSpec::with_percentages(
                node_id,
                parse_period(period)?,
                base_value,
                pct_range,
            ),
        })
    }

    /// Node identifier to perturb.
    #[getter]
    fn node_id(&self) -> &str {
        &self.inner.node_id
    }

    /// Period-id string of the perturbed value.
    #[getter]
    fn period(&self) -> String {
        self.inner.period_id.to_string()
    }

    /// Unperturbed value.
    #[getter]
    fn base_value(&self) -> f64 {
        self.inner.base_value
    }

    /// Absolute replacement values applied one at a time.
    #[getter]
    fn perturbations(&self) -> Vec<f64> {
        self.inner.perturbations.clone()
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "ParameterSpec"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``ParameterSpec`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid ParameterSpec JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ParameterSpec", &self.inner)
    }
}

fn extract_parameter_spec(value: &Bound<'_, PyAny>) -> PyResult<RustParameterSpec> {
    if let Ok(spec) = value.extract::<PyRef<'_, PyParameterSpec>>() {
        return Ok(spec.inner.clone());
    }
    let (node_id, period, base_value, perturbations): SensitivityParameterInput =
        value.extract().map_err(|_| {
            crate::errors::value_error(
                "each parameter must be a ParameterSpec or a (node_id, period, base_value, perturbations) tuple",
            )
        })?;
    Ok(RustParameterSpec::new(
        node_id,
        parse_period(&period)?,
        base_value,
        perturbations,
    ))
}

/// Configuration for statement sensitivity analysis.
///
/// Parameters
/// ----------
/// mode : str
///     ``"diagonal"`` (one-at-a-time), ``"full_grid"`` or ``"tornado"``.
/// parameters : list[ParameterSpec | tuple[str, str, float, list[float]]]
///     Parameters to vary; tuples are ``(node_id, period, base_value,
///     perturbations)`` with absolute replacement values.
/// target_metrics : list[str]
///     Node identifiers tracked across scenarios.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import ParameterSpec, SensitivityConfig
/// >>> cfg = SensitivityConfig("diagonal", target_metrics=["profit"])
/// >>> cfg.add_parameter("revenue", "2025Q2", 100.0, pct=[-10.0, 10.0])
/// >>> cfg.parameters[0].perturbations
/// [90.0, 110.0]
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
        parameters: Vec<Bound<'_, PyAny>>,
        target_metrics: Vec<String>,
    ) -> PyResult<Self> {
        let parameters = parameters
            .iter()
            .map(extract_parameter_spec)
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            inner: RustSensitivityConfig {
                mode: parse_sensitivity_mode(mode)?,
                parameters,
                target_metrics,
            },
        })
    }

    /// Append a parameter to vary.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier to perturb.
    /// period : str
    ///     Period-id string of the perturbed value.
    /// base_value : float
    ///     Unperturbed value.
    /// perturbations : list[float] | None
    ///     Absolute replacement values.
    /// pct : list[float] | None
    ///     Percentage bumps applied to ``base_value`` (``[-10.0, 10.0]`` =
    ///     -10% / +10%). Exactly one of ``perturbations`` and ``pct`` must be
    ///     given.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``period`` does not parse or neither/both of ``perturbations``
    ///     and ``pct`` are supplied.
    #[pyo3(signature = (node_id, period, base_value, perturbations=None, pct=None))]
    fn add_parameter(
        &mut self,
        node_id: &str,
        period: &str,
        base_value: f64,
        perturbations: Option<Vec<f64>>,
        pct: Option<Vec<f64>>,
    ) -> PyResult<()> {
        let spec = match (perturbations, pct) {
            (Some(values), None) => {
                RustParameterSpec::new(node_id, parse_period(period)?, base_value, values)
            }
            (None, Some(pct_range)) => RustParameterSpec::with_percentages(
                node_id,
                parse_period(period)?,
                base_value,
                pct_range,
            ),
            _ => {
                return Err(crate::errors::value_error(
                    "add_parameter takes exactly one of `perturbations` or `pct`",
                ))
            }
        };
        self.inner.add_parameter(spec);
        Ok(())
    }

    /// Configured parameters in insertion order.
    #[getter]
    fn parameters(&self) -> Vec<PyParameterSpec> {
        self.inner
            .parameters
            .iter()
            .cloned()
            .map(|inner| PyParameterSpec { inner })
            .collect()
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

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Analysis mode: ``"diagonal"``, ``"full_grid"``, or ``"tornado"`` (the
    /// serde name, identical to ``to_json``).
    #[getter]
    fn mode(&self) -> PyResult<String> {
        sensitivity_mode_name(self.inner.mode)
    }

    /// Node identifiers of the statement metrics tracked across scenarios.
    #[getter]
    fn target_metrics(&self) -> Vec<String> {
        self.inner.target_metrics.clone()
    }

    /// Number of configured parameters (one `ParameterSpec` per entry).
    #[getter]
    fn parameter_count(&self) -> usize {
        self.inner.parameters.len()
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("SensitivityConfig", &self.inner)
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

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Label for the baseline scenario (e.g. ``"management_case"``).
    #[getter]
    fn baseline_label(&self) -> &str {
        &self.inner.baseline_label
    }

    /// Label for the comparison scenario (e.g. ``"bank_case"``).
    #[getter]
    fn comparison_label(&self) -> &str {
        &self.inner.comparison_label
    }

    /// Node identifiers of the metrics compared between the two scenarios.
    #[getter]
    fn metrics(&self) -> Vec<String> {
        self.inner.metrics.clone()
    }

    /// Periods to compare, as period-id strings (e.g. ``"2025Q1"``).
    #[getter]
    fn periods(&self) -> Vec<String> {
        self.inner.periods.iter().map(ToString::to_string).collect()
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("VarianceConfig", &self.inner)
    }
}

/// Named scenario definitions for statement-model evaluation.
///
/// Parameters
/// ----------
/// scenarios : dict[str, dict[str, float | Money | dict[str, float | Money]]]
///     Scenario name to overrides. Each override value is either a model-wide
///     value applied to every forecast period (``{"revenue": 90.0}``) or a
///     ``{period: value}`` dict applied to the named forecast periods only
///     (``{"growth": {"2025Q3": 0.02}}``); per-period values win over a
///     model-wide value for that period.
/// parents : dict[str, str] | None
///     Optional scenario name to parent scenario name for inheritance.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import ScenarioSet
/// >>> ScenarioSet({"base": {}, "down": {"revenue": {"2025Q2": 90.0}}}, parents={"down": "base"}).names
/// ['base', 'down']
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
    #[pyo3(signature = (scenarios, parents=None))]
    fn new(scenarios: &Bound<'_, PyDict>, parents: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let mut definitions = IndexMap::with_capacity(scenarios.len());
        for (name, overrides) in scenarios.iter() {
            let name = name.extract::<String>()?;
            let (overrides, period_overrides) = extract_overrides(&overrides)?;
            let parent = parents
                .and_then(|items| items.get_item(&name).transpose())
                .transpose()?
                .map(|value| value.extract::<String>())
                .transpose()?;
            definitions.insert(
                name,
                ScenarioDefinition {
                    parent,
                    overrides,
                    period_overrides,
                },
            );
        }
        Ok(Self {
            inner: RustScenarioSet {
                scenarios: definitions,
            },
        })
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

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Scenario names in definition (insertion) order.
    #[getter]
    fn names(&self) -> Vec<String> {
        self.inner.scenarios.keys().cloned().collect()
    }

    /// Resolve a scenario's inheritance lineage, root-first.
    ///
    /// Parameters
    /// ----------
    /// scenario : str
    ///     Name of the scenario to trace.
    ///
    /// Returns
    /// -------
    /// list[str]
    ///     Scenario names from the root ancestor through to `scenario`.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the scenario is unknown or its parent chain contains a cycle.
    #[pyo3(text_signature = "(scenario)")]
    fn trace(&self, scenario: &str) -> PyResult<Vec<String>> {
        self.inner.trace(scenario).map_err(display_to_py)
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ScenarioSet", &self.inner)
    }
}

/// One parameter's downside and upside impact in a tornado chart.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import TornadoEntry
/// >>> entry = TornadoEntry.from_json('{"parameter_id":"revenue","downside":-5.0,"upside":7.0}')
/// >>> entry.swing
/// 12.0
#[pyclass(
    name = "TornadoEntry",
    module = "finstack_quant.statements_analytics",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyTornadoEntry {
    pub(crate) inner: RustTornadoEntry,
}

impl PyTornadoEntry {
    pub(crate) fn from_inner(inner: RustTornadoEntry) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyTornadoEntry {
    /// Deserialize one tornado entry from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize this entry to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Statement sensitivity parameter key in ``node@period`` form; DCF entries
    /// use assumption identifiers such as ``wacc`` or ``g``.
    #[getter]
    fn parameter_id(&self) -> &str {
        &self.inner.parameter_id
    }

    /// Metric change at the parameter's minimum perturbation.
    #[getter]
    fn downside(&self) -> f64 {
        self.inner.downside
    }

    /// Metric change at the parameter's maximum perturbation.
    #[getter]
    fn upside(&self) -> f64 {
        self.inner.upside
    }

    /// Total swing magnitude, calculated as `upside - downside`.
    #[getter]
    fn swing(&self) -> f64 {
        self.inner.swing()
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "TornadoEntry(parameter_id='{}', downside={}, upside={})",
            self.inner.parameter_id, self.inner.downside, self.inner.upside
        )
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
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

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

    /// Node identifiers of the metrics tracked by the originating config.
    #[getter]
    fn target_metrics(&self) -> Vec<String> {
        self.inner.config.target_metrics.clone()
    }

    /// Configuration the run was generated from.
    #[getter]
    fn config(&self) -> PySensitivityConfig {
        PySensitivityConfig {
            inner: self.inner.config.clone(),
        }
    }

    /// Unperturbed baseline evaluation retained by every sensitivity run;
    /// ``None`` only when absent from an imported result document.
    #[getter]
    fn baseline(&self) -> Option<PyStatementResult> {
        self.inner
            .baseline
            .clone()
            .map(|inner| PyStatementResult { inner })
    }

    /// Per-scenario ``(parameter_values, results)`` pairs in generation order,
    /// where ``parameter_values`` is a ``{"node_id@period": value}`` dict.
    #[getter]
    fn scenarios(&self) -> Vec<(std::collections::BTreeMap<String, f64>, PyStatementResult)> {
        self.inner
            .scenarios
            .iter()
            .map(|scenario| {
                (
                    scenario
                        .parameter_values
                        .iter()
                        .map(|(k, v)| (k.clone(), *v))
                        .collect(),
                    PyStatementResult {
                        inner: scenario.results.clone(),
                    },
                )
            })
            .collect()
    }

    /// Export the run as a long pandas ``DataFrame``.
    ///
    /// Columns: ``scenario`` (0-based index), one column per perturbed
    /// parameter named ``node_id@period`` (``NaN`` where a scenario does not
    /// perturb it), then ``node_id``, ``period`` and ``value`` holding each
    /// tracked metric's value in that scenario. One row per (scenario, metric,
    /// period); when no metric is tracked, one row per scenario with the
    /// metric columns null so the parameter grid stays visible.
    ///
    /// Parameters
    /// ----------
    /// metrics : list[str] | None
    ///     Node identifiers to emit; ``None`` uses the config's
    ///     ``target_metrics``.
    #[pyo3(signature = (metrics=None))]
    fn to_dataframe<'py>(
        &self,
        py: Python<'py>,
        metrics: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let metrics = metrics.unwrap_or_else(|| self.inner.config.target_metrics.clone());
        let mut parameter_names: Vec<String> = Vec::new();
        for scenario in &self.inner.scenarios {
            for name in scenario.parameter_values.keys() {
                if !parameter_names.contains(name) {
                    parameter_names.push(name.clone());
                }
            }
        }
        let mut rows: Vec<serde_json::Value> = Vec::new();
        for (index, scenario) in self.inner.scenarios.iter().enumerate() {
            let mut base = serde_json::Map::new();
            base.insert("scenario".to_string(), serde_json::json!(index));
            for (parameter, value) in &scenario.parameter_values {
                base.insert(parameter.clone(), serde_json::json!(value));
            }
            let mut emitted = false;
            for metric in &metrics {
                if let Some(series) = scenario.results.get_node(metric) {
                    for (period, value) in series.iter() {
                        let mut row = base.clone();
                        row.insert("node_id".to_string(), serde_json::json!(metric));
                        row.insert("period".to_string(), serde_json::json!(period.to_string()));
                        row.insert("value".to_string(), serde_json::json!(value));
                        rows.push(serde_json::Value::Object(row));
                        emitted = true;
                    }
                }
            }
            if !emitted {
                let mut row = base;
                row.insert("node_id".to_string(), serde_json::Value::Null);
                row.insert("period".to_string(), serde_json::Value::Null);
                row.insert("value".to_string(), serde_json::Value::Null);
                rows.push(serde_json::Value::Object(row));
            }
        }
        let mut columns: Vec<ColumnSchema<'_>> = vec![("scenario", "int64")];
        columns.extend(
            parameter_names
                .iter()
                .map(|name| (name.as_str(), "float64")),
        );
        columns.extend(SENSITIVITY_TAIL_COLUMNS);
        serde_rows_to_dataframe_with_schema(py, &rows, &columns)
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

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py, None).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("SensitivityResult", &self.inner)
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
    /// Period this row covers, as a period-id string (e.g. ``"2025Q1"``).
    #[getter]
    fn period(&self) -> String {
        self.inner.period.to_string()
    }

    /// Node identifier of the compared metric.
    #[getter]
    fn metric(&self) -> &str {
        &self.inner.metric
    }

    /// Metric value in the baseline scenario, in the metric's own units.
    #[getter]
    fn baseline(&self) -> f64 {
        self.inner.baseline
    }

    /// Metric value in the comparison scenario, in the metric's own units.
    #[getter]
    fn comparison(&self) -> f64 {
        self.inner.comparison
    }

    /// Absolute variance ``comparison - baseline``, in the metric's units.
    #[getter]
    fn abs_var(&self) -> f64 {
        self.inner.abs_var
    }

    /// Percentage variance ``abs_var / baseline`` as a decimal fraction
    /// (``0.1`` = +10%).
    ///
    /// ``None`` when the baseline is effectively zero, where a ratio would be
    /// undefined rather than zero; fall back to ``abs_var`` in that case.
    #[getter]
    fn pct_var(&self) -> Option<f64> {
        self.inner.pct_var
    }

    /// Driver attribution ``{driver_node: contribution}`` computed by the
    /// variance analyzer, in the driver's own units; empty when the config
    /// declared no drivers.
    #[getter]
    fn driver_contribution(&self) -> std::collections::BTreeMap<String, f64> {
        self.inner
            .driver_contribution
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("VarianceRow", &self.inner)
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
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Label for the baseline scenario (e.g. ``"management_case"``).
    #[getter]
    fn baseline_label(&self) -> &str {
        &self.inner.baseline_label
    }

    /// Label for the comparison scenario (e.g. ``"bank_case"``).
    #[getter]
    fn comparison_label(&self) -> &str {
        &self.inner.comparison_label
    }

    /// Per-metric, per-period variance rows, in report order.
    #[getter]
    fn rows(&self) -> Vec<PyVarianceRow> {
        self.inner
            .rows
            .iter()
            .cloned()
            .map(|inner| PyVarianceRow { inner })
            .collect()
    }

    /// Export the variance rows as a pandas ``DataFrame``.
    ///
    /// Columns: ``period``, ``metric``, ``baseline``, ``comparison``,
    /// ``abs_var``, ``pct_var``, ``driver_contribution``. One row per
    /// (metric, period) pair, in report order; an empty report still carries
    /// the full column schema.
    ///
    /// ``baseline``, ``comparison`` and ``abs_var`` are in the metric's own
    /// units; ``pct_var`` is a decimal fraction (``0.1`` = +10%) and is
    /// ``NaN`` where the baseline is effectively zero;
    /// ``driver_contribution`` is the ``{driver: contribution}`` dict of
    /// ``VarianceRow.driver_contribution`` (an object column, empty dict when
    /// no drivers were declared). The scenario labels are report metadata
    /// (``baseline_label`` / ``comparison_label``) and are not repeated per row.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "period": row.period.to_string(),
                    "metric": row.metric,
                    "baseline": row.baseline,
                    "comparison": row.comparison,
                    "abs_var": row.abs_var,
                    // Emitted explicitly (not via `VarianceRow`'s serde, which
                    // skips a `None`) so the column exists even when every
                    // baseline is zero.
                    "pct_var": row.pct_var,
                    "driver_contribution": row.driver_contribution,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, &VARIANCE_ROW_COLUMNS)
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("VarianceReport", &self.inner)
    }
}

/// Typed evaluated results for a set of named scenarios.
///
/// Named after the canonical Rust type
/// (`finstack_quant_statements_analytics::analysis::ScenarioResults`).
#[pyclass(
    name = "ScenarioResults",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyScenarioResults {
    pub(crate) inner: ScenarioResults,
}

#[pymethods]
impl PyScenarioResults {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: ScenarioResults = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Evaluated scenario names, in the order the scenario set defined them.
    #[getter]
    fn names(&self) -> Vec<String> {
        self.inner.scenarios.keys().cloned().collect()
    }

    /// Identify this result set in notebooks and logs.
    ///
    /// Rendered as the scenario count; use :meth:`names` for the full list.
    fn __repr__(&self) -> String {
        format!("ScenarioResults(scenarios={})", self.inner.scenarios.len())
    }

    fn get(&self, name: &str) -> Option<PyStatementResult> {
        self.inner
            .scenarios
            .get(name)
            .cloned()
            .map(|inner| PyStatementResult { inner })
    }

    /// Build a side-by-side comparison table across every evaluated scenario.
    ///
    /// Parameters
    /// ----------
    /// metrics : list[str]
    ///     Node identifiers to include as rows.
    ///
    /// Returns
    /// -------
    /// ArrowTable
    ///     One column per scenario, one row per (metric, period).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the result set or `metrics` is empty.
    #[pyo3(text_signature = "(metrics)")]
    fn to_comparison_table(
        &self,
        metrics: Vec<String>,
    ) -> PyResult<crate::bindings::core::table::PyArrowTable> {
        let table = self.comparison_table(&metrics)?;
        crate::bindings::core::table::PyArrowTable::from_envelope(&table)
    }

    /// Export the scenario comparison as a pandas ``DataFrame``.
    ///
    /// Columns: ``period``, ``metric``, one column per scenario name holding
    /// that scenario's metric value, and one ``{scenario}_vs_{baseline}_frac``
    /// column per non-baseline scenario holding the relative change as a
    /// decimal fraction (``0.1`` = +10%, ``NaN`` on a near-zero baseline). The
    /// ``_frac`` suffix states the unit: multiply by 100 for percent.
    /// One row per (metric, period) pair.
    ///
    /// This is the same table as ``to_comparison_table`` — both call one Rust
    /// implementation, so the two exports cannot drift apart. The baseline is
    /// the scenario named ``"base"`` when present, otherwise the first
    /// scenario.
    ///
    /// Parameters
    /// ----------
    /// metrics : list[str]
    ///     Node identifiers to include as rows.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the result set or ``metrics`` is empty.
    #[pyo3(text_signature = "(metrics)")]
    fn to_dataframe<'py>(
        &self,
        py: Python<'py>,
        metrics: Vec<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table = self.comparison_table(&metrics)?;
        table_to_dataframe(py, &table)
    }
}

impl PyScenarioResults {
    /// Build the canonical comparison table shared by `to_comparison_table`
    /// and `to_dataframe`.
    fn comparison_table(
        &self,
        metrics: &[String],
    ) -> PyResult<finstack_quant_core::table::TableEnvelope> {
        let refs: Vec<&str> = metrics.iter().map(String::as_str).collect();
        self.inner.to_comparison_table(&refs).map_err(display_to_py)
    }
}

/// Variance between two named scenarios in an evaluated scenario set.
#[pyclass(
    name = "ScenarioDiff",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyScenarioDiff {
    pub(crate) inner: RustScenarioDiff,
}

#[pymethods]
impl PyScenarioDiff {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a scenario diff from its canonical JSON form.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: RustScenarioDiff = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to canonical JSON (``baseline``, ``comparison``, ``variance``).
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Name of the scenario used as the baseline of the diff.
    #[getter]
    fn baseline(&self) -> &str {
        &self.inner.baseline
    }

    /// Identify this diff in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ScenarioDiff", &self.inner)
    }

    /// Name of the scenario compared against the baseline.
    #[getter]
    fn comparison(&self) -> &str {
        &self.inner.comparison
    }

    /// Underlying variance report between the two named scenarios.
    #[getter]
    fn variance(&self) -> PyVarianceReport {
        PyVarianceReport {
            inner: self.inner.variance.clone(),
        }
    }

    /// Export the underlying variance rows as a pandas ``DataFrame``.
    ///
    /// Columns: ``period``, ``metric``, ``baseline``, ``comparison``,
    /// ``abs_var``, ``pct_var``. One row per (metric, period) pair, in report
    /// order; an empty diff still carries the full column schema.
    ///
    /// This is the same table as ``variance.to_dataframe()`` — both call one
    /// implementation, so the two cannot drift apart. The two scenario *names*
    /// are diff metadata (the ``baseline`` / ``comparison`` getters) and are
    /// not repeated per row; the ``baseline`` and ``comparison`` columns hold
    /// the metric *values* in each scenario.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.variance().to_dataframe(py)
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// One driver step in a bridge decomposition.
#[pyclass(
    name = "BridgeStep",
    module = "finstack_quant.statements_analytics",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBridgeStep {
    pub(crate) inner: RustBridgeStep,
}

#[pymethods]
impl PyBridgeStep {
    /// Driver node identifier (e.g. ``"revenue"``).
    #[getter]
    fn driver(&self) -> &str {
        &self.inner.driver
    }

    /// This driver's raw delta between the two scenarios, in the *driver's*
    /// own units.
    ///
    /// Contributions are not sensitivities of the target metric, so they
    /// generally do not sum to the target variance — see
    /// ``BridgeChart.unexplained``.
    #[getter]
    fn contribution(&self) -> f64 {
        self.inner.contribution
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("BridgeStep", &self.inner)
    }
}

/// Bridge decomposition of a metric's variance across named drivers.
#[pyclass(
    name = "BridgeChart",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyBridgeChart {
    pub(crate) inner: RustBridgeChart,
}

#[pymethods]
impl PyBridgeChart {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Node identifier of the metric this bridge decomposes (e.g.
    /// ``"ebitda"``).
    #[getter]
    fn target_metric(&self) -> &str {
        &self.inner.target_metric
    }

    /// Period the bridge covers, as a period-id string (e.g. ``"2025Q1"``).
    #[getter]
    fn period(&self) -> String {
        self.inner.period.to_string()
    }

    /// Label for the baseline scenario (e.g. ``"management_case"``).
    #[getter]
    fn baseline_label(&self) -> &str {
        &self.inner.baseline_label
    }

    /// Label for the comparison scenario (e.g. ``"bank_case"``).
    #[getter]
    fn comparison_label(&self) -> &str {
        &self.inner.comparison_label
    }

    /// Target-metric value in the baseline scenario, in the metric's units.
    #[getter]
    fn baseline_value(&self) -> f64 {
        self.inner.baseline_value
    }

    /// Target-metric value in the comparison scenario, in the metric's units.
    #[getter]
    fn comparison_value(&self) -> f64 {
        self.inner.comparison_value
    }

    /// Ordered driver contributions making up the bridge.
    #[getter]
    fn steps(&self) -> Vec<PyBridgeStep> {
        self.inner
            .steps
            .iter()
            .map(|step| PyBridgeStep {
                inner: step.clone(),
            })
            .collect()
    }

    /// Export the driver steps as a pandas ``DataFrame``.
    ///
    /// Columns: ``driver``, ``contribution``. One row per bridge step, in
    /// decomposition order; an empty bridge still carries both columns.
    /// Contributions are raw deltas in each driver's own units.
    ///
    /// The scalar header fields (``target_metric``, ``period``,
    /// ``baseline_label``, ``comparison_label``, ``baseline_value``,
    /// ``comparison_value``, ``unexplained``) are chart metadata and are not
    /// repeated on every row.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .steps
            .iter()
            .map(|step| {
                serde_json::json!({
                    "driver": step.driver,
                    "contribution": step.contribution,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, &BRIDGE_STEP_COLUMNS)
    }

    /// Residual variance not explained by the driver deltas.
    ///
    /// Driver contributions are raw deltas in driver units rather than
    /// sensitivities of the target metric, so they generally do not sum to
    /// the target variance. This term makes that gap explicit.
    #[getter]
    fn unexplained(&self) -> f64 {
        self.inner.unexplained
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("BridgeChart", &self.inner)
    }
}

pub fn register(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyParameterSpec>()?;
    module.add_class::<PySensitivityConfig>()?;
    module.add_class::<PyVarianceConfig>()?;
    module.add_class::<PyScenarioSet>()?;
    module.add_class::<PySensitivityResult>()?;
    module.add_class::<PyTornadoEntry>()?;
    module.add_class::<PyVarianceRow>()?;
    module.add_class::<PyVarianceReport>()?;
    module.add_class::<PyScenarioResults>()?;
    module.add_class::<PyScenarioDiff>()?;
    module.add_class::<PyBridgeStep>()?;
    module.add_class::<PyBridgeChart>()?;
    Ok(())
}

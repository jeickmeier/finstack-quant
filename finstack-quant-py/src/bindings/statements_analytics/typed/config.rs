//! Sensitivity, variance, and scenario-set configuration Python bindings.

use crate::bindings::core::money::PyMoney;
use crate::bindings::extract::{label, parse_label};
use crate::bindings::macros::wire_methods;
use crate::bindings::statements::parse_period_id;
use crate::bindings::statements::types::PyForecastSpec;
use finstack_quant_statements::types::AmountOrScalar;
use finstack_quant_statements_analytics::analysis::{
    ParameterSpec as RustParameterSpec, ScenarioDefinition, ScenarioSet as RustScenarioSet,
    SensitivityConfig as RustSensitivityConfig, SensitivityMode,
    VarianceConfig as RustVarianceConfig,
};
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::PyDict;

type SensitivityParameterInput = (String, String, f64, Vec<f64>);

fn extract_amount_or_scalar(value: &Bound<'_, PyAny>) -> PyResult<AmountOrScalar> {
    if let Ok(money) = value.extract::<PyRef<'_, PyMoney>>() {
        return Ok(AmountOrScalar::Amount(money.inner));
    }
    Ok(AmountOrScalar::scalar(value.extract()?))
}

/// Split a scenario's ``{node: value | {period: value} | ForecastSpec}`` dict
/// into the overrides of a `ScenarioDefinition` (parent left unset).
fn extract_overrides(value: &Bound<'_, PyAny>) -> PyResult<ScenarioDefinition> {
    let values = value.cast::<PyDict>()?;
    let mut definition = ScenarioDefinition::default();
    for (node_id, value) in values.iter() {
        let node_id: String = node_id.extract()?;
        match value.extract::<PyRef<'_, PyForecastSpec>>() {
            Ok(forecast) => {
                definition
                    .forecast_overrides
                    .insert(node_id, forecast.inner.clone());
            }
            _ => match value.cast::<PyDict>() {
                Ok(by_period) => {
                    let entry = definition.period_overrides.entry(node_id).or_default();
                    for (period, value) in by_period.iter() {
                        let period = parse_period_id(&period.extract::<String>()?)?;
                        entry.insert(period, extract_amount_or_scalar(&value)?);
                    }
                }
                _ => {
                    definition
                        .overrides
                        .insert(node_id, extract_amount_or_scalar(&value)?);
                }
            },
        }
    }
    Ok(definition)
}

/// One parameter to vary in a sensitivity run.
///
/// Each perturbation replaces the node's value in ``period`` only; it does
/// not re-anchor the node's forecast in later periods (see
/// ``run_sensitivity``).
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
                parse_period_id(period)?,
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
    ///     If ``period`` does not parse, ``base_value`` or a percentage is not
    ///     finite (``NaN`` or infinite), or a converted shock overflows.
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
                parse_period_id(period)?,
                base_value,
                pct_range,
            )
            .map_err(crate::errors::statements_to_py)?,
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

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ParameterSpec", &self.inner)
    }
}

wire_methods!(PyParameterSpec, RustParameterSpec, "ParameterSpec");

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
        parse_period_id(&period)?,
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
                mode: parse_label::<SensitivityMode>(mode)?,
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
    ///     If ``period`` does not parse, neither/both of ``perturbations``
    ///     and ``pct`` are supplied, or ``pct`` is given with a non-finite
    ///     ``base_value`` or percentage or a shock that overflows.
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
                RustParameterSpec::new(node_id, parse_period_id(period)?, base_value, values)
            }
            (None, Some(pct_range)) => RustParameterSpec::with_percentages(
                node_id,
                parse_period_id(period)?,
                base_value,
                pct_range,
            )
            .map_err(crate::errors::statements_to_py)?,
            _ => {
                return Err(crate::errors::value_error(
                    "add_parameter takes exactly one of `perturbations` or `pct`",
                ));
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

    /// Analysis mode: ``"diagonal"``, ``"full_grid"``, or ``"tornado"`` (the
    /// serde name, identical to ``to_json``).
    #[getter]
    fn mode(&self) -> PyResult<String> {
        label(&self.inner.mode)
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

wire_methods!(
    PySensitivityConfig,
    RustSensitivityConfig,
    "SensitivityConfig"
);

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
            .map(|period| parse_period_id(period))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            inner: RustVarianceConfig::new(baseline_label, comparison_label, metrics, periods),
        })
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

wire_methods!(PyVarianceConfig, RustVarianceConfig, "VarianceConfig");

/// Named scenario definitions for statement-model evaluation.
///
/// Parameters
/// ----------
/// scenarios : dict[str, dict[str, float | Money | dict[str, float | Money] | ForecastSpec]]
///     Scenario name to overrides. Each override value is a model-wide value
///     applied to every forecast period (``{"revenue": 90.0}``), a
///     ``{period: value}`` dict applied to the named forecast periods only
///     (``{"growth": {"2025Q3": 0.02}}``), or a ``ForecastSpec`` that
///     replaces the node's forecast (``{"revenue": ForecastSpec.growth(0.02)}``):
///     its method and every parameter, such as curves, seeds and
///     volatilities. Per-period values win over a model-wide value for that
///     period, and explicit values win over the forecast.
/// parents : dict[str, str] | None
///     Optional scenario name to parent scenario name for inheritance.
///
/// Raises
/// ------
/// ValueError
///     If a period id is malformed or an override value is not a number,
///     ``Money``, period dict or ``ForecastSpec``.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ForecastSpec
/// >>> from finstack_quant.statements_analytics import ScenarioSet
/// >>> ScenarioSet({"base": {}, "down": {"revenue": {"2025Q2": 90.0}}}, parents={"down": "base"}).names
/// ['base', 'down']
/// >>> ScenarioSet({"slow": {"revenue": ForecastSpec.growth(0.01)}}).names
/// ['slow']
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
            let mut definition = extract_overrides(&overrides)?;
            definition.parent = parents
                .and_then(|items| items.get_item(&name).transpose())
                .transpose()?
                .map(|value| value.extract::<String>())
                .transpose()?;
            definitions.insert(name, definition);
        }
        Ok(Self {
            inner: RustScenarioSet {
                scenarios: definitions,
            },
        })
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
        self.inner
            .trace(scenario)
            .map_err(crate::errors::statements_to_py)
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

wire_methods!(PyScenarioSet, RustScenarioSet, "ScenarioSet");

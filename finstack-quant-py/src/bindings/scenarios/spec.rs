//! Typed Python wrappers for scenario specifications and template metadata.

use crate::bindings::date_utils::date_to_py;
use crate::errors::{display_to_py, scenarios_to_py};
use pyo3::prelude::*;
use pyo3::types::PyAny;

use super::operation_spec::PyOperationSpec;

fn enum_label<T: serde::Serialize>(value: &T) -> PyResult<String> {
    match serde_json::to_value(value).map_err(display_to_py)? {
        serde_json::Value::String(label) => Ok(label),
        _ => Err(crate::errors::value_error(
            "scenario enum did not serialize to a string",
        )),
    }
}

fn parse_resolution_mode(
    value: &str,
) -> PyResult<finstack_quant_core::market_data::hierarchy::ResolutionMode> {
    serde_json::from_value(serde_json::Value::String(value.to_string())).map_err(|_| {
        crate::errors::value_error(format!(
            "Unknown resolution_mode {value:?}; expected 'most_specific_wins' or 'cumulative'"
        ))
    })
}

fn parse_hazard_bump_mode(value: &str) -> PyResult<finstack_quant_scenarios::HazardBumpMode> {
    serde_json::from_value(serde_json::Value::String(value.to_string())).map_err(|_| {
        crate::errors::value_error(format!(
            "Unknown hazard_bump_mode {value:?}; expected 'solve_to_par' or 'first_order_shift'"
        ))
    })
}

/// Validated scenario specification executed by the scenario engine.
///
/// Parameters
/// ----------
/// id : str
///     Stable scenario identifier used for lookup and serialization.
/// operations : list[OperationSpec]
///     Ordered operations applied by the scenario engine.
/// name : str, optional
///     Human-readable scenario name.
/// description : str, optional
///     Human-readable explanation of the scenario.
/// priority : int, default 0
///     Composition priority; lower values execute first.
/// resolution_mode : str, default "most_specific_wins"
///     Hierarchy conflict policy: ``"most_specific_wins"`` or ``"cumulative"``.
/// hazard_bump_mode : str, default "solve_to_par"
///     ParCDS delivery: ``"solve_to_par"`` re-bootstraps hazard from shocked
///     par spreads; ``"first_order_shift"`` shifts hazard knots in place.
///
/// Raises
/// ------
/// ValueError
///     If the resolution mode, hazard bump mode, or resulting scenario is
///     invalid (empty id, more than one time roll, invalid operation).
///
/// Examples
/// --------
/// >>> from finstack_quant.scenarios import CurveKind, OperationSpec, ScenarioSpec
/// >>> operation = OperationSpec.curve_parallel_bp(CurveKind.discount(), "USD-OIS", 25.0)
/// >>> spec = ScenarioSpec("rates_up", [operation])
/// >>> spec.id
/// 'rates_up'
/// >>> spec == ScenarioSpec.from_json(spec.to_json())
/// True
#[pyclass(
    name = "ScenarioSpec",
    module = "finstack_quant.scenarios",
    eq,
    frozen,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyScenarioSpec {
    pub(crate) inner: finstack_quant_scenarios::ScenarioSpec,
}

impl PyScenarioSpec {
    pub(crate) fn from_inner(inner: finstack_quant_scenarios::ScenarioSpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyScenarioSpec {
    #[new]
    #[pyo3(signature = (
        id,
        operations,
        name=None,
        description=None,
        priority=0,
        resolution_mode="most_specific_wins",
        hazard_bump_mode="solve_to_par"
    ))]
    fn new(
        id: &str,
        operations: Vec<PyOperationSpec>,
        name: Option<&str>,
        description: Option<&str>,
        priority: i32,
        resolution_mode: &str,
        hazard_bump_mode: &str,
    ) -> PyResult<Self> {
        let inner = finstack_quant_scenarios::ScenarioSpec {
            id: id.to_string(),
            name: name.map(str::to_string),
            description: description.map(str::to_string),
            operations: operations
                .into_iter()
                .map(|operation| operation.inner)
                .collect(),
            priority,
            resolution_mode: parse_resolution_mode(resolution_mode)?,
            hazard_bump_mode: parse_hazard_bump_mode(hazard_bump_mode)?,
        };
        inner.validate().map_err(scenarios_to_py)?;
        Ok(Self { inner })
    }

    /// Deserialize and validate canonical scenario JSON.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON object matching the Rust ``ScenarioSpec`` serde contract.
    ///     Unknown operation fields are rejected.
    ///
    /// Returns
    /// -------
    /// ScenarioSpec
    ///     Validated typed scenario specification.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If JSON parsing or scenario validation fails.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner =
            finstack_quant_scenarios::ScenarioSpec::from_json(json).map_err(scenarios_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize this scenario to canonical JSON.
    ///
    /// Returns
    /// -------
    /// str
    ///     Compact JSON matching the Rust serde contract.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Stable scenario identifier.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Optional human-readable scenario name.
    #[getter]
    fn name(&self) -> Option<&str> {
        self.inner.name.as_deref()
    }

    /// Optional human-readable scenario description.
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }

    /// Ordered scenario operations as independent typed values.
    #[getter]
    fn operations(&self) -> Vec<PyOperationSpec> {
        self.inner
            .operations
            .iter()
            .cloned()
            .map(|inner| PyOperationSpec { inner })
            .collect()
    }

    /// Composition priority; lower values execute first.
    #[getter]
    fn priority(&self) -> i32 {
        self.inner.priority
    }

    /// Hierarchy conflict policy as its canonical snake-case label.
    #[getter]
    fn resolution_mode(&self) -> PyResult<String> {
        enum_label(&self.inner.resolution_mode)
    }

    /// ParCDS hazard delivery as its canonical snake-case label.
    #[getter]
    fn hazard_bump_mode(&self) -> PyResult<String> {
        enum_label(&self.inner.hazard_bump_mode)
    }

    /// Validate the scenario using the canonical Rust rules.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an identifier, operation, numeric field, or composition rule is
    ///     invalid.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(scenarios_to_py)
    }

    /// Whether applying this scenario needs instruments in the execution
    /// context (instrument-scoped shocks or a ``time_roll_forward``).
    fn requires_instruments(&self) -> bool {
        self.inner.requires_instruments()
    }

    /// Whether applying this scenario can replace or mutate instruments
    /// (instrument price / spread / structured-credit correlation shocks).
    /// A time roll reads instruments for carry but does not mutate them.
    fn mutates_instruments(&self) -> bool {
        self.inner.mutates_instruments()
    }

    /// Return a copy with a different ParCDS hazard delivery mode.
    ///
    /// Parameters
    /// ----------
    /// mode : str
    ///     ``"solve_to_par"`` or ``"first_order_shift"``.
    ///
    /// Returns
    /// -------
    /// ScenarioSpec
    ///     New specification with ``hazard_bump_mode`` replaced.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``mode`` is not one of the accepted labels.
    fn with_hazard_bump_mode(&self, mode: &str) -> PyResult<Self> {
        let mode = parse_hazard_bump_mode(mode)?;
        Ok(Self {
            inner: self.inner.clone().with_hazard_bump_mode(mode),
        })
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "ScenarioSpec(id={:?}, operations={}, priority={})",
            self.inner.id,
            self.inner.operations.len(),
            self.inner.priority
        )
    }
}

/// Discovery metadata for one built-in historical scenario template.
///
/// Examples
/// --------
/// >>> from finstack_quant.scenarios import list_builtin_template_metadata
/// >>> metadata = list_builtin_template_metadata()
/// >>> metadata[0].id in {"gfc_2008", "covid_2020", "rate_shock_2022", "svb_2023", "ltcm_1998"}
/// True
#[pyclass(
    name = "TemplateMetadata",
    module = "finstack_quant.scenarios",
    eq,
    frozen,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyTemplateMetadata {
    pub(crate) inner: finstack_quant_scenarios::TemplateMetadata,
}

impl PyTemplateMetadata {
    pub(crate) fn from_inner(inner: finstack_quant_scenarios::TemplateMetadata) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyTemplateMetadata {
    /// Stable template identifier.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Human-readable template name.
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// Historical event and modeled-effects description.
    #[getter]
    fn description(&self) -> &str {
        &self.inner.description
    }

    /// Primary historical event date.
    #[getter]
    fn event_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.event_date)
    }

    /// Canonical snake-case asset-class labels affected by the scenario.
    #[getter]
    fn asset_classes(&self) -> PyResult<Vec<String>> {
        self.inner.asset_classes.iter().map(enum_label).collect()
    }

    /// Freeform discovery tags.
    #[getter]
    fn tags(&self) -> Vec<String> {
        self.inner.tags.clone()
    }

    /// Canonical snake-case severity label.
    #[getter]
    fn severity(&self) -> PyResult<String> {
        enum_label(&self.inner.severity)
    }

    /// Component identifiers in deterministic build order.
    #[getter]
    fn components(&self) -> Vec<String> {
        self.inner.components.clone()
    }

    /// Deserialize template metadata from canonical JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize this metadata to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "TemplateMetadata(id={:?}, name={:?})",
            self.inner.id, self.inner.name
        )
    }
}

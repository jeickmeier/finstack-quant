//! Typed calibration-envelope authoring: steps, plans and envelopes.
//!
//! Every wrapper holds the canonical Rust value; constructors assemble the
//! serde wire shape and let the Rust deserializer validate it, so the binding
//! never re-implements field rules.

use super::config::extract_config;
use super::quotes::extract_market_datum;
use crate::bindings::macros::wire_methods;
use crate::bindings::module_utils::{py_to_json_value, spec_from_py};
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::pickle_support::reduce_via_json;
use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_calibration::api::market_datum::MarketDatum;
use finstack_quant_calibration::api::prior_market::PriorMarketObject;
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep,
};
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde_json::{Map, Value};

/// Deserialize a JSON object into `T` with a labelled error.
pub(crate) fn from_value<T: serde::de::DeserializeOwned>(value: Value, label: &str) -> PyResult<T> {
    serde_json::from_value(value).map_err(|e| serde_json_to_py(e, &format!("invalid {label}")))
}

fn extract_market_data(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<MarketDatum>> {
    let Some(obj) = obj else {
        return Ok(Vec::new());
    };
    if obj.is_none() {
        return Ok(Vec::new());
    }
    obj.try_iter()?
        .map(|item| extract_market_datum(py, &item?))
        .collect()
}

fn extract_prior_market(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<PriorMarketObject>> {
    let Some(obj) = obj else {
        return Ok(Vec::new());
    };
    if obj.is_none() {
        return Ok(Vec::new());
    }
    obj.try_iter()?
        .map(|item| {
            let value = py_to_json_value(py, &item?, "prior market object")?;
            from_value(value, "prior market object (expected {\"kind\": ..., ...})")
        })
        .collect()
}

/// One calibration step: the object to build, its quote set, and kind-specific parameters.
///
/// Every constructor takes the step ``id``, a ``params`` dict (or JSON text)
/// holding the kind's wire fields named exactly as in the Rust ``StepParams``
/// schema (see ``schema.get("calibration.schema.json")``), and an optional
/// ``quote_set``. Quote payloads live in ``CalibrationEnvelope.market_data``.
/// The plan names their IDs in ``quote_sets``; a step references the set by name.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationStep
/// >>> step = CalibrationStep.discount("USD-OIS", {"currency": "USD", "base_date": "2026-05-08"})
/// >>> step.kind, step.quote_set
/// ('discount', 'USD-OIS')
#[pyclass(
    name = "CalibrationStep",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCalibrationStep {
    pub(crate) inner: CalibrationStep,
}

impl PyCalibrationStep {
    pub(crate) fn from_inner(inner: CalibrationStep) -> Self {
        Self { inner }
    }

    /// Shared constructor: `kind` + `id` + the kind's wire-field object.
    ///
    /// Rust `CalibrationStep::from_wire_fields` owns the omitted-field
    /// defaults (quote set and produced-object identifiers fall back to `id`)
    /// and the strict deserialization.
    fn build(
        kind: &str,
        id: &str,
        params: &Bound<'_, PyAny>,
        quote_set: Option<String>,
    ) -> PyResult<Self> {
        let fields: Map<String, Value> =
            spec_from_py(params.py(), params, &format!("{kind} step params"))?;
        let step = CalibrationStep::from_wire_fields(kind, id, quote_set.as_deref(), fields)
            .map_err(core_to_py)?;
        Ok(Self::from_inner(step))
    }
}

#[pymethods]
impl PyCalibrationStep {
    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.discount")]
    #[staticmethod]
    #[pyo3(signature = (id, params, quote_set = None))]
    #[pyo3(text_signature = "(id, params, quote_set=None)")]
    fn discount(id: &str, params: &Bound<'_, PyAny>, quote_set: Option<String>) -> PyResult<Self> {
        Self::build("discount", id, params, quote_set)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.forward")]
    #[staticmethod]
    #[pyo3(signature = (id, params, quote_set = None))]
    #[pyo3(text_signature = "(id, params, quote_set=None)")]
    fn forward(id: &str, params: &Bound<'_, PyAny>, quote_set: Option<String>) -> PyResult<Self> {
        Self::build("forward", id, params, quote_set)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.hazard")]
    #[staticmethod]
    #[pyo3(signature = (id, params, quote_set = None))]
    #[pyo3(text_signature = "(id, params, quote_set=None)")]
    fn hazard(id: &str, params: &Bound<'_, PyAny>, quote_set: Option<String>) -> PyResult<Self> {
        Self::build("hazard", id, params, quote_set)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.inflation")]
    #[staticmethod]
    #[pyo3(signature = (id, params, quote_set = None))]
    #[pyo3(text_signature = "(id, params, quote_set=None)")]
    fn inflation(id: &str, params: &Bound<'_, PyAny>, quote_set: Option<String>) -> PyResult<Self> {
        Self::build("inflation", id, params, quote_set)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.vol_surface")]
    #[staticmethod]
    #[pyo3(signature = (id, params, quote_set = None))]
    #[pyo3(text_signature = "(id, params, quote_set=None)")]
    fn vol_surface(
        id: &str,
        params: &Bound<'_, PyAny>,
        quote_set: Option<String>,
    ) -> PyResult<Self> {
        Self::build("vol_surface", id, params, quote_set)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.swaption_vol")]
    #[staticmethod]
    #[pyo3(signature = (id, params, quote_set = None))]
    #[pyo3(text_signature = "(id, params, quote_set=None)")]
    fn swaption_vol(
        id: &str,
        params: &Bound<'_, PyAny>,
        quote_set: Option<String>,
    ) -> PyResult<Self> {
        Self::build("swaption_vol", id, params, quote_set)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.base_correlation")]
    #[staticmethod]
    #[pyo3(signature = (id, params, quote_set = None))]
    #[pyo3(text_signature = "(id, params, quote_set=None)")]
    fn base_correlation(
        id: &str,
        params: &Bound<'_, PyAny>,
        quote_set: Option<String>,
    ) -> PyResult<Self> {
        Self::build("base_correlation", id, params, quote_set)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.student_t")]
    #[staticmethod]
    #[pyo3(signature = (id, params, quote_set = None))]
    #[pyo3(text_signature = "(id, params, quote_set=None)")]
    fn student_t(id: &str, params: &Bound<'_, PyAny>, quote_set: Option<String>) -> PyResult<Self> {
        Self::build("student_t", id, params, quote_set)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.hull_white")]
    #[staticmethod]
    #[pyo3(signature = (id, params, quote_set = None))]
    #[pyo3(text_signature = "(id, params, quote_set=None)")]
    fn hull_white(
        id: &str,
        params: &Bound<'_, PyAny>,
        quote_set: Option<String>,
    ) -> PyResult<Self> {
        Self::build("hull_white", id, params, quote_set)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.cap_floor_hull_white")]
    #[staticmethod]
    #[pyo3(signature = (id, params, quote_set = None))]
    #[pyo3(text_signature = "(id, params, quote_set=None)")]
    fn cap_floor_hull_white(
        id: &str,
        params: &Bound<'_, PyAny>,
        quote_set: Option<String>,
    ) -> PyResult<Self> {
        Self::build("cap_floor_hull_white", id, params, quote_set)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.svi_surface")]
    #[staticmethod]
    #[pyo3(signature = (id, params, quote_set = None))]
    #[pyo3(text_signature = "(id, params, quote_set=None)")]
    fn svi_surface(
        id: &str,
        params: &Bound<'_, PyAny>,
        quote_set: Option<String>,
    ) -> PyResult<Self> {
        Self::build("svi_surface", id, params, quote_set)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.xccy_basis")]
    #[staticmethod]
    #[pyo3(signature = (id, params, quote_set = None))]
    #[pyo3(text_signature = "(id, params, quote_set=None)")]
    fn xccy_basis(
        id: &str,
        params: &Bound<'_, PyAny>,
        quote_set: Option<String>,
    ) -> PyResult<Self> {
        Self::build("xccy_basis", id, params, quote_set)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.parametric")]
    #[staticmethod]
    #[pyo3(signature = (id, params, quote_set = None))]
    #[pyo3(text_signature = "(id, params, quote_set=None)")]
    fn parametric(
        id: &str,
        params: &Bound<'_, PyAny>,
        quote_set: Option<String>,
    ) -> PyResult<Self> {
        Self::build("parametric", id, params, quote_set)
    }

    /// Step identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Name of the quote set this step reads from ``plan.quote_sets``.
    #[getter]
    fn quote_set(&self) -> String {
        self.inner.quote_set.clone()
    }

    /// Step kind (``"discount"``, ``"forward"``, ``"hazard"``, ...).
    #[getter]
    fn kind(&self) -> String {
        self.inner.params.kind_str().to_string()
    }

    /// Kind-specific parameters as a dict (including ``kind``).
    #[getter]
    fn params<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.params)
    }

    fn __repr__(&self) -> String {
        format!(
            "CalibrationStep(id={:?}, kind={:?}, quote_set={:?})",
            self.inner.id,
            self.inner.params.kind_str(),
            self.inner.quote_set,
        )
    }
}

wire_methods!(PyCalibrationStep, CalibrationStep, "CalibrationStep");

/// Ordered calibration plan: steps, named quote sets, and solver settings.
///
/// Quote IDs are supplied explicitly in ``quote_sets`` and resolve against
/// the enclosing ``CalibrationEnvelope.market_data``.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, CalibrationStep, RateQuote, calibrate
/// >>> quotes = [
/// ...     RateQuote.deposit("USD-DEP-3M", "USD-SOFR-OIS", "3M", 0.052),
/// ...     RateQuote.swap("USD-SWAP-2Y", "USD-SOFR-OIS", "2Y", 0.049),
/// ... ]
/// >>> plan = CalibrationPlan([CalibrationStep.discount("USD-OIS", {"currency": "USD", "base_date": "2026-05-08"})], quote_sets={"USD-OIS": [q.id for q in quotes]})
/// >>> plan.quote_sets
/// {'USD-OIS': ['USD-DEP-3M', 'USD-SWAP-2Y']}
/// >>> calibrate(CalibrationEnvelope(plan, market_data=quotes)).success
/// True
#[pyclass(
    name = "CalibrationPlan",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCalibrationPlan {
    pub(crate) inner: CalibrationPlan,
}

impl PyCalibrationPlan {
    pub(crate) fn from_inner(inner: CalibrationPlan) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCalibrationPlan {
    /// Build a plan from typed steps.
    ///
    /// Parameters
    /// ----------
    /// steps : list[CalibrationStep]
    ///     Plain steps in execution order.
    /// id : str, default "plan"
    ///     Plan identifier (Rust ``CalibrationPlan::DEFAULT_ID`` when omitted).
    /// description : str | None, default None
    ///     Free-text description.
    /// settings : CalibrationConfig | dict | None, default None
    ///     Solver settings; a dict is overlaid onto the Rust defaults.
    /// quote_sets : dict[str, list[str]] | None, default None
    ///     Explicit quote sets (ids must exist in the envelope
    ///     ``market_data``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``settings`` is invalid.
    #[new]
    #[pyo3(signature = (steps, id = CalibrationPlan::DEFAULT_ID, description = None, settings = None, quote_sets = None))]
    #[pyo3(text_signature = "(steps, id='plan', description=None, settings=None, quote_sets=None)")]
    fn new(
        py: Python<'_>,
        steps: Vec<PyRef<'_, PyCalibrationStep>>,
        id: &str,
        description: Option<String>,
        settings: Option<&Bound<'_, PyAny>>,
        quote_sets: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let settings = extract_config(py, settings)?;
        let mut sets: IndexMap<String, Vec<finstack_quant_calibration::quotes::QuoteId>> =
            IndexMap::new();
        if let Some(quote_sets) = quote_sets {
            for (name, ids) in quote_sets.iter() {
                let name: String = name.extract()?;
                let ids: Vec<String> = ids.extract()?;
                sets.insert(
                    name,
                    ids.into_iter()
                        .map(finstack_quant_calibration::quotes::QuoteId::new)
                        .collect(),
                );
            }
        }
        let steps = steps.iter().map(|step| step.inner.clone()).collect();
        Ok(Self::from_inner(CalibrationPlan {
            id: id.to_string(),
            description,
            settings,
            quote_sets: sets,
            steps,
        }))
    }

    /// Plan identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Plan description, when set.
    #[getter]
    fn description(&self) -> Option<String> {
        self.inner.description.clone()
    }

    /// Step identifiers in execution order.
    #[getter]
    fn step_ids(&self) -> Vec<String> {
        self.inner.steps.iter().map(|s| s.id.clone()).collect()
    }

    /// Plain typed steps in execution order.
    #[getter]
    fn steps(&self) -> Vec<PyCalibrationStep> {
        self.inner
            .steps
            .iter()
            .cloned()
            .map(PyCalibrationStep::from_inner)
            .collect()
    }

    /// Named quote sets: ``{name: [quote_id, ...]}``.
    #[getter]
    fn quote_sets<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (name, ids) in &self.inner.quote_sets {
            let list: Vec<&str> = ids.iter().map(|id| id.as_str()).collect();
            dict.set_item(name, PyList::new(py, list)?)?;
        }
        Ok(dict)
    }

    /// Solver settings.
    #[getter]
    fn settings(&self) -> super::config::PyCalibrationConfig {
        super::config::PyCalibrationConfig::from_inner(self.inner.settings.clone())
    }

    fn __repr__(&self) -> String {
        format!(
            "CalibrationPlan(id={:?}, steps={:?}, quote_sets={})",
            self.inner.id,
            self.step_ids(),
            self.inner.quote_sets.len(),
        )
    }
}

wire_methods!(PyCalibrationPlan, CalibrationPlan, "CalibrationPlan");

/// Complete calibration request: plan, flat market data, and prior calibrated objects.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan
/// >>> envelope = CalibrationEnvelope(CalibrationPlan([], id="smoke"))
/// >>> envelope.schema
/// 'finstack_quant.calibration/1'
/// >>> len(envelope.content_hash()) > 0
/// True
#[pyclass(
    name = "CalibrationEnvelope",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCalibrationEnvelope {
    pub(crate) inner: CalibrationEnvelope,
}

impl PyCalibrationEnvelope {
    pub(crate) fn from_inner(inner: CalibrationEnvelope) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCalibrationEnvelope {
    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationEnvelope.__init__")]
    #[new]
    #[pyo3(signature = (plan, market_data = None, prior_market = None))]
    #[pyo3(text_signature = "(plan, market_data=None, prior_market=None)")]
    fn new(
        py: Python<'_>,
        plan: PyRef<'_, PyCalibrationPlan>,
        market_data: Option<&Bound<'_, PyAny>>,
        prior_market: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let market_data = extract_market_data(py, market_data)?;
        let prior_market = extract_prior_market(py, prior_market)?;
        Ok(Self::from_inner(CalibrationEnvelope::new(
            plan.inner.clone(),
            market_data,
            prior_market,
        )))
    }

    /// Schema marker (``"finstack_quant.calibration/1"``).
    #[getter]
    fn schema(&self) -> String {
        finstack_quant_calibration::api::schema::CALIBRATION_SCHEMA.to_string()
    }

    /// The plain calibration plan.
    #[getter]
    fn plan(&self) -> PyCalibrationPlan {
        PyCalibrationPlan::from_inner(self.inner.plan.clone())
    }

    /// Flat market data as ``{"kind": ..., ...}`` dicts.
    #[getter]
    fn market_data<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.market_data)
    }

    /// Prior calibrated objects as ``{"kind": ..., ...}`` dicts.
    #[getter]
    fn prior_market<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.prior_market)
    }

    /// Versioned SHA-256 hash of the canonical request JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the request contains a non-finite number.
    fn content_hash(&self) -> PyResult<String> {
        self.inner.content_hash().map_err(core_to_py)
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CalibrationEnvelope"))
    }

    /// Strictly load an envelope from JSON.
    ///
    /// Raises
    /// ------
    /// CalibrationEnvelopeError
    ///     If the JSON is malformed, the schema marker is missing or
    ///     unsupported, or the structure is invalid; ``diagnostics`` lists the
    ///     pointer-level findings.
    #[staticmethod]
    fn from_json(py: Python<'_>, json: &str) -> PyResult<Self> {
        super::parse_envelope(py, json).map(Self::from_inner)
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "CalibrationEnvelope(plan={:?}, steps={}, market_data={}, prior_market={})",
            self.inner.plan.id,
            self.inner.plan.steps.len(),
            self.inner.market_data.len(),
            self.inner.prior_market.len()
        )
    }
}

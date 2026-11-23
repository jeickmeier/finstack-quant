//! Credit state and PIK toggle exercise model Python bindings.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::core_to_py;
use finstack_quant_core::wire::serde_parse;
use finstack_quant_models::credit::{
    CreditState, CreditStateVariable, ThresholdDirection, ToggleExerciseModel,
};
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.models.credit._toggle", "CreditState")]
#[pyclass(
    name = "CreditState",
    module = "finstack_quant.models.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyCreditState {
    pub(crate) inner: CreditState,
}

#[pymethods]
impl PyCreditState {
    #[doc = pydoc!("finstack_quant.models.credit._toggle", "CreditState.__init__")]
    #[new]
    #[pyo3(signature = (hazard_rate=0.0, distance_to_default=None, leverage=0.0, accreted_notional=0.0, coupon_due=0.0, asset_value=None))]
    #[pyo3(
        text_signature = "(hazard_rate=0.0, distance_to_default=None, leverage=0.0, accreted_notional=0.0, coupon_due=0.0, asset_value=None)"
    )]
    fn new(
        hazard_rate: f64,
        distance_to_default: Option<f64>,
        leverage: f64,
        accreted_notional: f64,
        coupon_due: f64,
        asset_value: Option<f64>,
    ) -> PyResult<Self> {
        CreditState::new(
            hazard_rate,
            distance_to_default,
            leverage,
            accreted_notional,
            coupon_due,
            asset_value,
        )
        .map(|inner| Self { inner })
        .map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CreditState", &self.inner)
    }
}

py_getters!(PyCreditState |s| {
    #[doc = pydoc!("finstack_quant.models.credit._toggle", "CreditState.hazard_rate")]
    hazard_rate -> f64 = s.inner.hazard_rate;
    #[doc = pydoc!("finstack_quant.models.credit._toggle", "CreditState.distance_to_default")]
    distance_to_default -> Option<f64> = s.inner.distance_to_default;
    #[doc = pydoc!("finstack_quant.models.credit._toggle", "CreditState.leverage")]
    leverage -> f64 = s.inner.leverage;
    #[doc = pydoc!("finstack_quant.models.credit._toggle", "CreditState.accreted_notional")]
    accreted_notional -> f64 = s.inner.accreted_notional;
    #[doc = pydoc!("finstack_quant.models.credit._toggle", "CreditState.coupon_due")]
    coupon_due -> f64 = s.inner.coupon_due;
    #[doc = pydoc!("finstack_quant.models.credit._toggle", "CreditState.asset_value")]
    asset_value -> Option<f64> = s.inner.asset_value;
});

impl PyCreditState {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64("hazard_rate", vec![self.inner.hazard_rate])?
            .nullable_f64("distance_to_default", vec![self.inner.distance_to_default])?
            .f64("leverage", vec![self.inner.leverage])?
            .f64("accreted_notional", vec![self.inner.accreted_notional])?
            .f64("coupon_due", vec![self.inner.coupon_due])?
            .nullable_f64("asset_value", vec![self.inner.asset_value])?
            .build()
    }
}

tabular_methods!(
    PyCreditState,
    "CreditState",
    module = "finstack_quant.models.credit._toggle"
);
wire_methods!(
    PyCreditState,
    CreditState,
    "CreditState",
    module = "finstack_quant.models.credit._toggle"
);

#[doc = pydoc!("finstack_quant.models.credit._toggle", "ToggleExerciseModel")]
#[pyclass(
    name = "ToggleExerciseModel",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyToggleExerciseModel {
    pub(crate) inner: ToggleExerciseModel,
}

#[pymethods]
impl PyToggleExerciseModel {
    #[doc = pydoc!("finstack_quant.models.credit._toggle", "ToggleExerciseModel.threshold")]
    #[staticmethod]
    #[pyo3(text_signature = "(variable, threshold, direction)")]
    fn threshold(variable: &str, threshold: f64, direction: &str) -> PyResult<Self> {
        let variable = serde_parse::<CreditStateVariable>(variable).map_err(core_to_py)?;
        let direction = serde_parse::<ThresholdDirection>(direction).map_err(core_to_py)?;
        ToggleExerciseModel::threshold(variable, threshold, direction)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._toggle", "ToggleExerciseModel.stochastic")]
    #[staticmethod]
    #[pyo3(text_signature = "(variable, intercept, sensitivity)")]
    fn stochastic(variable: &str, intercept: f64, sensitivity: f64) -> PyResult<Self> {
        let variable = serde_parse::<CreditStateVariable>(variable).map_err(core_to_py)?;
        ToggleExerciseModel::stochastic(variable, intercept, sensitivity)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._toggle", "ToggleExerciseModel.should_pik")]
    #[pyo3(text_signature = "($self, state, u)")]
    fn should_pik(&self, state: &PyCreditState, u: f64) -> PyResult<bool> {
        self.inner.should_pik(&state.inner, u).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._toggle", "ToggleExerciseModel.optimal")]
    #[staticmethod]
    #[pyo3(text_signature = "(nested_paths, asset_vol, risk_free_rate, horizon)")]
    fn optimal(
        nested_paths: usize,
        asset_vol: f64,
        risk_free_rate: f64,
        horizon: f64,
    ) -> PyResult<Self> {
        ToggleExerciseModel::optimal(nested_paths, asset_vol, risk_free_rate, horizon)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._toggle", "ToggleExerciseModel.kind")]
    #[getter]
    fn kind(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(&self.inner).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._toggle", "ToggleExerciseModel.params")]
    #[getter]
    fn params<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.inner {
            ToggleExerciseModel::Threshold(spec) => serde_to_py(py, spec),
            ToggleExerciseModel::Stochastic(spec) => serde_to_py(py, spec),
            ToggleExerciseModel::OptimalExercise(spec) => serde_to_py(py, spec),
        }
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ToggleExerciseModel", &self.inner)
    }
}

wire_methods!(
    PyToggleExerciseModel,
    ToggleExerciseModel,
    "ToggleExerciseModel",
    module = "finstack_quant.models.credit._toggle"
);

/// Render an externally-tagged enum as its Python constructor call:
/// `"terminal"` -> `Name.terminal()`, `{"first_passage": {"k": v}}` ->
/// `Name.first_passage(k=v)`. Never fails; falls back to `Name(...)`.
pub(super) fn variant_repr<T: serde::Serialize>(type_name: &str, value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(tag)) => format!("{type_name}.{tag}()"),
        Ok(serde_json::Value::Object(map)) if map.len() == 1 => {
            let Some((tag, fields)) = map.iter().next() else {
                return format!("{type_name}(...)");
            };
            let inner = crate::bindings::repr_support::repr_from_serde("", fields);
            format!("{type_name}.{tag}{inner}")
        }
        _ => format!("{type_name}(...)"),
    }
}

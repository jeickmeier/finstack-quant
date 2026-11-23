//! Dynamic recovery and endogenous hazard Python specifications.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::serde_object_to_single_row_arrow_table;
use crate::errors::core_to_py;
use finstack_quant_models::credit::{DynamicRecoverySpec, EndogenousHazardSpec};
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "DynamicRecoverySpec")]
#[pyclass(
    name = "DynamicRecoverySpec",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyDynamicRecoverySpec {
    pub(crate) inner: DynamicRecoverySpec,
}

#[pymethods]
impl PyDynamicRecoverySpec {
    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "DynamicRecoverySpec.constant")]
    #[staticmethod]
    #[pyo3(text_signature = "(recovery)")]
    fn constant(recovery: f64) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::constant(recovery).map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "DynamicRecoverySpec.inverse_linear")]
    #[staticmethod]
    #[pyo3(text_signature = "(base_recovery, base_notional)")]
    fn inverse_linear(base_recovery: f64, base_notional: f64) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::inverse_linear(base_recovery, base_notional)
                .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "DynamicRecoverySpec.inverse_power")]
    #[staticmethod]
    #[pyo3(text_signature = "(base_recovery, base_notional, exponent)")]
    fn inverse_power(base_recovery: f64, base_notional: f64, exponent: f64) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::inverse_power(base_recovery, base_notional, exponent)
                .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "DynamicRecoverySpec.floored_inverse")]
    #[staticmethod]
    #[pyo3(text_signature = "(base_recovery, base_notional, floor)")]
    fn floored_inverse(base_recovery: f64, base_notional: f64, floor: f64) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::floored_inverse(base_recovery, base_notional, floor)
                .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "DynamicRecoverySpec.linear_decline")]
    #[staticmethod]
    #[pyo3(text_signature = "(base_recovery, base_notional, sensitivity, floor)")]
    fn linear_decline(
        base_recovery: f64,
        base_notional: f64,
        sensitivity: f64,
        floor: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::linear_decline(
                base_recovery,
                base_notional,
                sensitivity,
                floor,
            )
            .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "DynamicRecoverySpec.recovery_at_notional")]
    #[pyo3(text_signature = "($self, notional)")]
    fn recovery_at_notional(&self, notional: f64) -> f64 {
        self.inner.recovery_at_notional(notional)
    }

    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "DynamicRecoverySpec.model")]
    #[getter]
    fn model<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, self.inner.model())
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("DynamicRecoverySpec", &self.inner)
    }
}

py_getters!(PyDynamicRecoverySpec |s| {
    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "DynamicRecoverySpec.kind")]
    kind -> PyResult<String> = finstack_quant_core::wire::serde_tag(s.inner.model()).map_err(core_to_py);
    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "DynamicRecoverySpec.base_recovery")]
    base_recovery -> f64 = s.inner.base_recovery();
    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "DynamicRecoverySpec.base_notional")]
    base_notional -> f64 = s.inner.base_notional();
});

impl PyDynamicRecoverySpec {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_object_to_single_row_arrow_table(
            &self.inner,
            &[
                ("base_recovery", "float64"),
                ("base_notional", "float64"),
                ("model", "str"),
            ],
        )
    }
}

tabular_methods!(
    PyDynamicRecoverySpec,
    "DynamicRecoverySpec",
    module = "finstack_quant.models.credit._recovery_hazard"
);
wire_methods!(
    PyDynamicRecoverySpec,
    DynamicRecoverySpec,
    "DynamicRecoverySpec",
    module = "finstack_quant.models.credit._recovery_hazard"
);

#[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "EndogenousHazardSpec")]
#[pyclass(
    name = "EndogenousHazardSpec",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyEndogenousHazardSpec {
    pub(crate) inner: EndogenousHazardSpec,
}

#[pymethods]
impl PyEndogenousHazardSpec {
    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "EndogenousHazardSpec.power_law")]
    #[staticmethod]
    #[pyo3(text_signature = "(base_hazard, base_leverage, exponent)")]
    fn power_law(base_hazard: f64, base_leverage: f64, exponent: f64) -> PyResult<Self> {
        Ok(Self {
            inner: EndogenousHazardSpec::power_law(base_hazard, base_leverage, exponent)
                .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "EndogenousHazardSpec.exponential")]
    #[staticmethod]
    #[pyo3(text_signature = "(base_hazard, base_leverage, sensitivity)")]
    fn exponential(base_hazard: f64, base_leverage: f64, sensitivity: f64) -> PyResult<Self> {
        Ok(Self {
            inner: EndogenousHazardSpec::exponential(base_hazard, base_leverage, sensitivity)
                .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "EndogenousHazardSpec.tabular")]
    #[staticmethod]
    #[pyo3(text_signature = "(leverage_points, hazard_points)")]
    fn tabular(leverage_points: Vec<f64>, hazard_points: Vec<f64>) -> PyResult<Self> {
        Ok(Self {
            inner: EndogenousHazardSpec::tabular(leverage_points, hazard_points)
                .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "EndogenousHazardSpec.hazard_at_leverage")]
    #[pyo3(text_signature = "($self, leverage)")]
    fn hazard_at_leverage(&self, leverage: f64) -> f64 {
        self.inner.hazard_at_leverage(leverage)
    }

    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "EndogenousHazardSpec.hazard_after_pik_accrual")]
    #[pyo3(text_signature = "($self, accreted_notional, asset_value)")]
    fn hazard_after_pik_accrual(&self, accreted_notional: f64, asset_value: f64) -> f64 {
        self.inner
            .hazard_after_pik_accrual(accreted_notional, asset_value)
    }

    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "EndogenousHazardSpec.leverage_hazard_map")]
    #[getter]
    fn leverage_hazard_map<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, self.inner.leverage_hazard_map())
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("EndogenousHazardSpec", &self.inner)
    }
}

py_getters!(PyEndogenousHazardSpec |s| {
    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "EndogenousHazardSpec.kind")]
    kind -> PyResult<String> = finstack_quant_core::wire::serde_tag(s.inner.leverage_hazard_map()).map_err(core_to_py);
    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "EndogenousHazardSpec.base_hazard_rate")]
    base_hazard_rate -> f64 = s.inner.base_hazard_rate();
    #[doc = pydoc!("finstack_quant.models.credit._recovery_hazard", "EndogenousHazardSpec.base_leverage")]
    base_leverage -> f64 = s.inner.base_leverage();
});

impl PyEndogenousHazardSpec {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_object_to_single_row_arrow_table(
            &self.inner,
            &[
                ("base_hazard_rate", "float64"),
                ("base_leverage", "float64"),
                ("leverage_hazard_map", "str"),
            ],
        )
    }
}

tabular_methods!(
    PyEndogenousHazardSpec,
    "EndogenousHazardSpec",
    module = "finstack_quant.models.credit._recovery_hazard"
);
wire_methods!(
    PyEndogenousHazardSpec,
    EndogenousHazardSpec,
    "EndogenousHazardSpec",
    module = "finstack_quant.models.credit._recovery_hazard"
);

//! Stage 3 credit-impaired ECL exposure, evidence, scenario, and calculation bindings.

use crate::bindings::json_bridge::serde_to_py;
use crate::errors::core_to_py;
use finstack_quant_statements_analytics::analysis as rust_ecl;
use pyo3::prelude::*;
use pyo3::types::PyDict;

#[pyclass(
    name = "ImpairedExposure",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "ImpairedExposure")]
#[derive(Clone)]
pub(super) struct PyImpairedExposure {
    inner: rust_ecl::ImpairedExposure,
}

#[pymethods]
impl PyImpairedExposure {
    #[new]
    #[pyo3(signature = (**fields), text_signature = "(*, exposure_id, carrying_amount_source, carrying_amount, reporting_date, effective_interest_rate, recovery_day_count)")]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "ImpairedExposure.__init__")]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        Ok(Self {
            inner: crate::bindings::module_utils::fields_from_py(py, fields, "ImpairedExposure")?,
        })
    }
}

#[pyclass(
    name = "RecoveryEvidence",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "RecoveryEvidence")]
#[derive(Clone)]
pub(super) struct PyRecoveryEvidence {
    inner: rust_ecl::RecoveryEvidence,
}

#[pymethods]
impl PyRecoveryEvidence {
    #[new]
    #[pyo3(signature = (**fields), text_signature = "(*, source_id, amount, date)")]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "RecoveryEvidence.__init__")]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        Ok(Self {
            inner: crate::bindings::module_utils::fields_from_py(py, fields, "RecoveryEvidence")?,
        })
    }
}

#[pyclass(
    name = "ImpairedEclResult",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "ImpairedEclResult")]
#[derive(Clone)]
pub(super) struct PyImpairedEclResult {
    inner: rust_ecl::ImpairedEclResult,
}

#[pymethods]
impl PyImpairedEclResult {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "ImpairedEclResult.present_value_recovery")]
    fn present_value_recovery(&self) -> crate::bindings::core::money::PyMoney {
        crate::bindings::core::money::PyMoney::from_inner(self.inner.present_value_recovery)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "ImpairedEclResult.signed_shortfall")]
    fn signed_shortfall(&self) -> crate::bindings::core::money::PyMoney {
        crate::bindings::core::money::PyMoney::from_inner(self.inner.signed_shortfall)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "ImpairedEclResult.allowance")]
    fn allowance(&self) -> crate::bindings::core::money::PyMoney {
        crate::bindings::core::money::PyMoney::from_inner(self.inner.allowance)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "ImpairedEclResult.time_to_recovery_years")]
    fn time_to_recovery_years(&self) -> f64 {
        self.inner.time_to_recovery_years
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "ImpairedEclResult.discount_factor")]
    fn discount_factor(&self) -> f64 {
        self.inner.discount_factor
    }
}

crate::bindings::macros::wire_methods!(
    PyImpairedEclResult,
    rust_ecl::ImpairedEclResult,
    "ImpairedEclResult",
    module = "finstack_quant.statements_analytics._ecl.impaired"
);

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "compute_impaired_ecl")]
#[pyfunction]
pub(super) fn compute_impaired_ecl(
    exposure: &PyImpairedExposure,
    recovery: &PyRecoveryEvidence,
) -> PyResult<PyImpairedEclResult> {
    rust_ecl::compute_impaired_ecl(&exposure.inner, &recovery.inner)
        .map(|inner| PyImpairedEclResult { inner })
        .map_err(core_to_py)
}

#[pyclass(
    name = "ImpairedEclScenario",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "ImpairedEclScenario")]
#[derive(Clone)]
pub(super) struct PyImpairedEclScenario {
    inner: rust_ecl::ImpairedEclScenario,
}

#[pymethods]
impl PyImpairedEclScenario {
    #[new]
    #[pyo3(signature = (*, id, weight, recovery=None))]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "ImpairedEclScenario.__init__")]
    fn new(
        id: String,
        weight: &Bound<'_, PyAny>,
        recovery: Option<&PyRecoveryEvidence>,
    ) -> PyResult<Self> {
        let weight = crate::bindings::core::money::extract_exact_decimal(
            weight,
            "weight must be a decimal probability",
        )?;
        Ok(Self {
            inner: rust_ecl::ImpairedEclScenario {
                id,
                weight: finstack_quant_core::wire::DecimalWire(weight),
                recovery: recovery.map(|source| source.inner.clone()),
            },
        })
    }
}

#[pyclass(
    name = "WeightedImpairedEclResult",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "WeightedImpairedEclResult")]
#[derive(Clone)]
pub(super) struct PyWeightedImpairedEclResult {
    inner: rust_ecl::WeightedImpairedEclResult,
}

#[pymethods]
impl PyWeightedImpairedEclResult {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "WeightedImpairedEclResult.allowance")]
    fn allowance(&self) -> Option<crate::bindings::core::money::PyMoney> {
        self.inner
            .allowance
            .map(crate::bindings::core::money::PyMoney::from_inner)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "WeightedImpairedEclResult.outcomes")]
    fn outcomes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.outcomes)
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._ecl.impaired", "compute_impaired_ecl_weighted")]
#[pyfunction]
pub(super) fn compute_impaired_ecl_weighted(
    exposure: &PyImpairedExposure,
    scenarios: Vec<PyImpairedEclScenario>,
) -> PyResult<PyWeightedImpairedEclResult> {
    let scenarios = scenarios
        .into_iter()
        .map(|scenario| scenario.inner)
        .collect::<Vec<_>>();
    rust_ecl::compute_impaired_ecl_weighted(&exposure.inner, &scenarios)
        .map(|inner| PyWeightedImpairedEclResult { inner })
        .map_err(core_to_py)
}

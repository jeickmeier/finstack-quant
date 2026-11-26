//! A range accrual with explicit issuer call rights.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::exotics::callable_range_accrual::{
    CallableRangeAccrual, CallableRangeAccrualBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_trait_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_callable_range_accrual", "CallableRangeAccrual")]
#[pyclass(
    name = "CallableRangeAccrual",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCallableRangeAccrual {
    pub(crate) inner: CallableRangeAccrual,
}

instrument_envelope_methods!(
    PyCallableRangeAccrual(
        "CallableRangeAccrual",
        module = "finstack_quant.valuations.instruments._typed_callable_range_accrual"
    ),
    CallableRangeAccrual,
    builder = PyCallableRangeAccrualBuilder("CallableRangeAccrualBuilder", no_fields),
    seed = CallableRangeAccrual::builder(),
);
instrument_trait_methods!(
    PyCallableRangeAccrual,
    "CallableRangeAccrual",
    module = "finstack_quant.valuations.instruments._typed_callable_range_accrual"
);

#[pymethods]
impl PyCallableRangeAccrual {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_callable_range_accrual", "CallableRangeAccrual.to_builder")]
    fn to_builder(&self) -> PyCallableRangeAccrualBuilder {
        PyCallableRangeAccrualBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_callable_range_accrual", "CallableRangeAccrual.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        CallableRangeAccrual::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_callable_range_accrual", "CallableRangeAccrual.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_callable_range_accrual", "CallableRangeAccrual.range_accrual")]
    #[getter]
    fn range_accrual<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.range_accrual)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_callable_range_accrual", "CallableRangeAccrual.call_provision")]
    #[getter]
    fn call_provision<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.call_provision)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_callable_range_accrual", "CallableRangeAccrualBuilder")]
#[pyclass(
    name = "CallableRangeAccrualBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyCallableRangeAccrualBuilder {
    inner: Option<CallableRangeAccrualBuilder>,
}

#[pymethods]
impl PyCallableRangeAccrualBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_callable_range_accrual", "CallableRangeAccrualBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: CallableRangeAccrualBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_callable_range_accrual", "CallableRangeAccrualBuilder.range_accrual")]
    fn range_accrual<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "range_accrual")?;
        builder_set!(slf, |b: CallableRangeAccrualBuilder| b.range_accrual(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_callable_range_accrual", "CallableRangeAccrualBuilder.call_provision")]
    fn call_provision<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "call_provision")?;
        builder_set!(slf, |b: CallableRangeAccrualBuilder| b
            .call_provision(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_callable_range_accrual", "CallableRangeAccrualBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: CallableRangeAccrualBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCallableRangeAccrual>()?;
    module.add_class::<PyCallableRangeAccrualBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["CallableRangeAccrual", "CallableRangeAccrualBuilder"];

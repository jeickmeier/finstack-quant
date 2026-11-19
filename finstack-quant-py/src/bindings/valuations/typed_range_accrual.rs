//! A range accrual paying coupons weighted by eligible observations.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::exotics::range_accrual::{
    RangeAccrual, RangeAccrualBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_range_accrual", "RangeAccrual")]
#[pyclass(
    name = "RangeAccrual",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRangeAccrual {
    pub(crate) inner: RangeAccrual,
}

instrument_envelope_methods!(
    PyRangeAccrual(
        "RangeAccrual",
        module = "finstack_quant.valuations.instruments._typed_range_accrual"
    ),
    RangeAccrual,
    builder = PyRangeAccrualBuilder("RangeAccrualBuilder", no_fields),
    seed = RangeAccrual::builder(),
);
instrument_pricing_methods!(
    PyRangeAccrual,
    "RangeAccrual",
    module = "finstack_quant.valuations.instruments._typed_range_accrual"
);

#[pymethods]
impl PyRangeAccrual {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_range_accrual", "RangeAccrual.to_builder")]
    fn to_builder(&self) -> PyRangeAccrualBuilder {
        PyRangeAccrualBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_range_accrual", "RangeAccrual.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        RangeAccrual::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_range_accrual", "RangeAccrual.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_range_accrual", "RangeAccrual.terms")]
    #[getter]
    fn terms<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.terms)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_range_accrual", "RangeAccrualBuilder")]
#[pyclass(
    name = "RangeAccrualBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyRangeAccrualBuilder {
    inner: Option<RangeAccrualBuilder>,
}

#[pymethods]
impl PyRangeAccrualBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_range_accrual", "RangeAccrualBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: RangeAccrualBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_range_accrual", "RangeAccrualBuilder.terms")]
    fn terms<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "terms")?;
        builder_set!(slf, |b: RangeAccrualBuilder| b.terms(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_range_accrual", "RangeAccrualBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: RangeAccrualBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyRangeAccrual>()?;
    module.add_class::<PyRangeAccrualBuilder>()?;
    Ok(())
}

//! A swap exchanging fixed-income index total return for contractual financing.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fixed_income::fi_trs::{
    FiIndexTotalReturnSwap, FiIndexTotalReturnSwapBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwap")]
#[pyclass(
    name = "FiIndexTotalReturnSwap",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFiIndexTotalReturnSwap {
    pub(crate) inner: FiIndexTotalReturnSwap,
}

instrument_envelope_methods!(
    PyFiIndexTotalReturnSwap(
        "FiIndexTotalReturnSwap",
        module = "finstack_quant.valuations.instruments._typed_fi_index_total_return_swap"
    ),
    FiIndexTotalReturnSwap,
    builder = PyFiIndexTotalReturnSwapBuilder("FiIndexTotalReturnSwapBuilder", no_fields),
    seed = FiIndexTotalReturnSwap::builder(),
);
instrument_pricing_methods!(
    PyFiIndexTotalReturnSwap,
    "FiIndexTotalReturnSwap",
    module = "finstack_quant.valuations.instruments._typed_fi_index_total_return_swap"
);

#[pymethods]
impl PyFiIndexTotalReturnSwap {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwap.to_builder")]
    fn to_builder(&self) -> PyFiIndexTotalReturnSwapBuilder {
        PyFiIndexTotalReturnSwapBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwap.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        FiIndexTotalReturnSwap::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwap.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwap.notional")]
    #[getter]
    fn notional(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.notional)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwap.underlying")]
    #[getter]
    fn underlying<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.underlying)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwap.financing_leg")]
    #[getter]
    fn financing_leg<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.financing_leg)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwap.schedule")]
    #[getter]
    fn schedule<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.schedule)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwap.side")]
    #[getter]
    fn side<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.side)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwap.initial_level")]
    #[getter]
    fn initial_level(&self) -> Option<f64> {
        self.inner.initial_level
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwap.fixing_series_id")]
    #[getter]
    fn fixing_series_id(&self) -> Option<String> {
        self.inner
            .fixing_series_id
            .as_ref()
            .map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwap.margin_spec")]
    #[getter]
    fn margin_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.margin_spec)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwapBuilder")]
#[pyclass(
    name = "FiIndexTotalReturnSwapBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyFiIndexTotalReturnSwapBuilder {
    inner: Option<FiIndexTotalReturnSwapBuilder>,
}

#[pymethods]
impl PyFiIndexTotalReturnSwapBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwapBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: FiIndexTotalReturnSwapBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwapBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: FiIndexTotalReturnSwapBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwapBuilder.underlying")]
    fn underlying<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "underlying")?;
        builder_set!(slf, |b: FiIndexTotalReturnSwapBuilder| b.underlying(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwapBuilder.financing_leg")]
    fn financing_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "financing_leg")?;
        builder_set!(slf, |b: FiIndexTotalReturnSwapBuilder| b
            .financing_leg(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwapBuilder.schedule")]
    fn schedule<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "schedule")?;
        builder_set!(slf, |b: FiIndexTotalReturnSwapBuilder| b.schedule(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwapBuilder.side")]
    fn side<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "side")?;
        builder_set!(slf, |b: FiIndexTotalReturnSwapBuilder| b.side(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwapBuilder.initial_level")]
    fn initial_level<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FiIndexTotalReturnSwapBuilder| b
            .initial_level_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwapBuilder.fixing_series_id")]
    fn fixing_series_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: FiIndexTotalReturnSwapBuilder| b
            .fixing_series_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwapBuilder.margin_spec")]
    fn margin_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "margin_spec")?;
        builder_set!(slf, |b: FiIndexTotalReturnSwapBuilder| b
            .margin_spec_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fi_index_total_return_swap", "FiIndexTotalReturnSwapBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: FiIndexTotalReturnSwapBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFiIndexTotalReturnSwap>()?;
    module.add_class::<PyFiIndexTotalReturnSwapBuilder>()?;
    Ok(())
}

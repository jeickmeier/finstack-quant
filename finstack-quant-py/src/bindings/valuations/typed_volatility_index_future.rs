//! A listed future on a forward volatility index level.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::equity::vol_index_future::{
    VolatilityIndexFuture, VolatilityIndexFutureBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_listed::PyListedFutureTerms;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future", "VolatilityIndexFuture")]
#[pyclass(
    name = "VolatilityIndexFuture",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyVolatilityIndexFuture {
    pub(crate) inner: VolatilityIndexFuture,
}

instrument_envelope_methods!(
    PyVolatilityIndexFuture(
        "VolatilityIndexFuture",
        module = "finstack_quant.valuations.instruments._typed_volatility_index_future"
    ),
    VolatilityIndexFuture,
    builder = PyVolatilityIndexFutureBuilder("VolatilityIndexFutureBuilder", no_fields),
    seed = VolatilityIndexFuture::builder(),
);
instrument_pricing_methods!(
    PyVolatilityIndexFuture,
    "VolatilityIndexFuture",
    module = "finstack_quant.valuations.instruments._typed_volatility_index_future"
);

#[pymethods]
impl PyVolatilityIndexFuture {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future", "VolatilityIndexFuture.to_builder")]
    fn to_builder(&self) -> PyVolatilityIndexFutureBuilder {
        PyVolatilityIndexFutureBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future", "VolatilityIndexFuture.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        VolatilityIndexFuture::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future", "VolatilityIndexFuture.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }
}

crate::bindings::macros::py_getters!(PyVolatilityIndexFuture |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future", "VolatilityIndexFuture.terms")]
    terms -> PyListedFutureTerms = PyListedFutureTerms {
        inner: s.inner.terms.clone(),
    };

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future", "VolatilityIndexFuture.vol_index_curve_id")]
    vol_index_curve_id -> String = s.inner.vol_index_curve_id.to_string();
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future", "VolatilityIndexFutureBuilder")]
#[pyclass(
    name = "VolatilityIndexFutureBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyVolatilityIndexFutureBuilder {
    inner: Option<VolatilityIndexFutureBuilder>,
}

#[pymethods]
impl PyVolatilityIndexFutureBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future", "VolatilityIndexFutureBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: VolatilityIndexFutureBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future", "VolatilityIndexFutureBuilder.terms")]
    fn terms<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyListedFutureTerms,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: VolatilityIndexFutureBuilder| b.terms(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future", "VolatilityIndexFutureBuilder.vol_index_curve_id")]
    fn vol_index_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "vol_index_curve_id")?;
        builder_set!(slf, |b: VolatilityIndexFutureBuilder| b
            .vol_index_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future", "VolatilityIndexFutureBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: VolatilityIndexFutureBuilder| b.attributes(value))
    }
}

/// Register the volatility-index future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyVolatilityIndexFuture>()?;
    module.add_class::<PyVolatilityIndexFutureBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["VolatilityIndexFuture", "VolatilityIndexFutureBuilder"];

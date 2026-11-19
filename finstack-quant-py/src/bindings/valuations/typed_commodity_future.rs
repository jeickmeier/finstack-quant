//! A listed future on a commodity and an exchange settlement price.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::commodity::commodity_future::{
    CommodityFuture, CommodityFutureBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFuture")]
#[pyclass(
    name = "CommodityFuture",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCommodityFuture {
    pub(crate) inner: CommodityFuture,
}

instrument_envelope_methods!(
    PyCommodityFuture(
        "CommodityFuture",
        module = "finstack_quant.valuations.instruments._typed_commodity_future"
    ),
    CommodityFuture,
    builder = PyCommodityFutureBuilder("CommodityFutureBuilder", no_fields),
    seed = CommodityFuture::builder(),
);
instrument_pricing_methods!(
    PyCommodityFuture,
    "CommodityFuture",
    module = "finstack_quant.valuations.instruments._typed_commodity_future"
);

#[pymethods]
impl PyCommodityFuture {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFuture.to_builder")]
    fn to_builder(&self) -> PyCommodityFutureBuilder {
        PyCommodityFutureBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFuture.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        CommodityFuture::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFuture.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFuture.underlying_ticker")]
    #[getter]
    fn underlying_ticker(&self) -> String {
        self.inner.underlying_ticker.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFuture.terms")]
    #[getter]
    fn terms(&self) -> super::typed_listed::PyListedFutureTerms {
        super::typed_listed::PyListedFutureTerms {
            inner: self.inner.terms.clone(),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFuture.forward_curve_id")]
    #[getter]
    fn forward_curve_id(&self) -> String {
        self.inner.forward_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFuture.fixing")]
    #[getter]
    fn fixing<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.fixing)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFutureBuilder")]
#[pyclass(
    name = "CommodityFutureBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyCommodityFutureBuilder {
    inner: Option<CommodityFutureBuilder>,
}

#[pymethods]
impl PyCommodityFutureBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFutureBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: CommodityFutureBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFutureBuilder.underlying_ticker")]
    fn underlying_ticker<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommodityFutureBuilder| b.underlying_ticker(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFutureBuilder.terms")]
    fn terms<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &super::typed_listed::PyListedFutureTerms,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: CommodityFutureBuilder| b.terms(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFutureBuilder.forward_curve_id")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommodityFutureBuilder| b.forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFutureBuilder.fixing")]
    fn fixing<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "fixing")?;
        builder_set!(slf, |b: CommodityFutureBuilder| b.fixing(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future", "CommodityFutureBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: CommodityFutureBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCommodityFuture>()?;
    module.add_class::<PyCommodityFutureBuilder>()?;
    Ok(())
}

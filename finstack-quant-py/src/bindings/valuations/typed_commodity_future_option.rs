//! Typed options on listed futures.

use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::commodity::commodity_future_option::{
    CommodityFutureOption, CommodityFutureOptionBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_listed::PyFutureOptionTerms;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future_option", "CommodityFutureOption")]
#[pyclass(
    name = "CommodityFutureOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCommodityFutureOption {
    pub(crate) inner: CommodityFutureOption,
}

instrument_envelope_methods!(
    PyCommodityFutureOption(
        "CommodityFutureOption",
        module = "finstack_quant.valuations.instruments._typed_commodity_future_option"
    ),
    CommodityFutureOption,
    builder = PyCommodityFutureOptionBuilder("CommodityFutureOptionBuilder", no_fields),
    seed = CommodityFutureOption::builder(),
);
instrument_pricing_methods!(
    PyCommodityFutureOption,
    "CommodityFutureOption",
    module = "finstack_quant.valuations.instruments._typed_commodity_future_option"
);

#[pymethods]
impl PyCommodityFutureOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future_option", "CommodityFutureOption.to_builder")]
    fn to_builder(&self) -> PyCommodityFutureOptionBuilder {
        PyCommodityFutureOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future_option", "CommodityFutureOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        CommodityFutureOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future_option", "CommodityFutureOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future_option", "CommodityFutureOption.terms")]
    #[getter]
    fn terms(&self) -> PyFutureOptionTerms {
        PyFutureOptionTerms {
            inner: self.inner.terms.clone(),
        }
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future_option", "CommodityFutureOptionBuilder")]
#[pyclass(
    name = "CommodityFutureOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyCommodityFutureOptionBuilder {
    inner: Option<CommodityFutureOptionBuilder>,
}

#[pymethods]
impl PyCommodityFutureOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future_option", "CommodityFutureOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: CommodityFutureOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future_option", "CommodityFutureOptionBuilder.terms")]
    fn terms<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyFutureOptionTerms,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: CommodityFutureOptionBuilder| b.terms(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_future_option", "CommodityFutureOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: CommodityFutureOptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCommodityFutureOption>()?;
    module.add_class::<PyCommodityFutureOptionBuilder>()?;
    Ok(())
}

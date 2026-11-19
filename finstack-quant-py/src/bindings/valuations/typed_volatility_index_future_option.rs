//! Typed options on listed futures.

use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::equity::vol_index_future_option::{
    VolatilityIndexFutureOption, VolatilityIndexFutureOptionBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_listed::PyFutureOptionTerms;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future_option", "VolatilityIndexFutureOption")]
#[pyclass(
    name = "VolatilityIndexFutureOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyVolatilityIndexFutureOption {
    pub(crate) inner: VolatilityIndexFutureOption,
}

instrument_envelope_methods!(
    PyVolatilityIndexFutureOption(
        "VolatilityIndexFutureOption",
        module = "finstack_quant.valuations.instruments._typed_volatility_index_future_option"
    ),
    VolatilityIndexFutureOption,
    builder = PyVolatilityIndexFutureOptionBuilder("VolatilityIndexFutureOptionBuilder", no_fields),
    seed = VolatilityIndexFutureOption::builder(),
);
instrument_pricing_methods!(
    PyVolatilityIndexFutureOption,
    "VolatilityIndexFutureOption",
    module = "finstack_quant.valuations.instruments._typed_volatility_index_future_option"
);

#[pymethods]
impl PyVolatilityIndexFutureOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future_option", "VolatilityIndexFutureOption.to_builder")]
    fn to_builder(&self) -> PyVolatilityIndexFutureOptionBuilder {
        PyVolatilityIndexFutureOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future_option", "VolatilityIndexFutureOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        VolatilityIndexFutureOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future_option", "VolatilityIndexFutureOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future_option", "VolatilityIndexFutureOption.terms")]
    #[getter]
    fn terms(&self) -> PyFutureOptionTerms {
        PyFutureOptionTerms {
            inner: self.inner.terms.clone(),
        }
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future_option", "VolatilityIndexFutureOptionBuilder")]
#[pyclass(
    name = "VolatilityIndexFutureOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyVolatilityIndexFutureOptionBuilder {
    inner: Option<VolatilityIndexFutureOptionBuilder>,
}

#[pymethods]
impl PyVolatilityIndexFutureOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future_option", "VolatilityIndexFutureOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: VolatilityIndexFutureOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future_option", "VolatilityIndexFutureOptionBuilder.terms")]
    fn terms<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyFutureOptionTerms,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: VolatilityIndexFutureOptionBuilder| b.terms(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_volatility_index_future_option", "VolatilityIndexFutureOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: VolatilityIndexFutureOptionBuilder| b
            .attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyVolatilityIndexFutureOption>()?;
    module.add_class::<PyVolatilityIndexFutureOptionBuilder>()?;
    Ok(())
}

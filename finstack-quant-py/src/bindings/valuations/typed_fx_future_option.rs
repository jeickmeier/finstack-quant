//! Typed options on listed futures.

use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fx::fx_future_option::{
    FxFutureOption, FxFutureOptionBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_listed::PyFutureOptionTerms;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future_option", "FxFutureOption")]
#[pyclass(
    name = "FxFutureOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFxFutureOption {
    pub(crate) inner: FxFutureOption,
}

instrument_envelope_methods!(
    PyFxFutureOption(
        "FxFutureOption",
        module = "finstack_quant.valuations.instruments._typed_fx_future_option"
    ),
    FxFutureOption,
    builder = PyFxFutureOptionBuilder("FxFutureOptionBuilder", no_fields),
    seed = FxFutureOption::builder(),
);
instrument_pricing_methods!(
    PyFxFutureOption,
    "FxFutureOption",
    module = "finstack_quant.valuations.instruments._typed_fx_future_option"
);

#[pymethods]
impl PyFxFutureOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future_option", "FxFutureOption.to_builder")]
    fn to_builder(&self) -> PyFxFutureOptionBuilder {
        PyFxFutureOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future_option", "FxFutureOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        FxFutureOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future_option", "FxFutureOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }
}

crate::bindings::macros::py_getters!(PyFxFutureOption |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future_option", "FxFutureOption.terms")]
    terms -> PyFutureOptionTerms = PyFutureOptionTerms {
        inner: s.inner.terms.clone(),
    };
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future_option", "FxFutureOptionBuilder")]
#[pyclass(
    name = "FxFutureOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyFxFutureOptionBuilder {
    inner: Option<FxFutureOptionBuilder>,
}

#[pymethods]
impl PyFxFutureOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future_option", "FxFutureOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: FxFutureOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future_option", "FxFutureOptionBuilder.terms")]
    fn terms<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyFutureOptionTerms,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: FxFutureOptionBuilder| b.terms(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future_option", "FxFutureOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: FxFutureOptionBuilder| b.attributes(value))
    }
}

/// Register the FX future option and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFxFutureOption>()?;
    module.add_class::<PyFxFutureOptionBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["FxFutureOption", "FxFutureOptionBuilder"];

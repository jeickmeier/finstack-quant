//! Typed options on listed futures.

use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::equity::equity_future_option::{
    EquityFutureOption, EquityFutureOptionBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_listed::PyFutureOptionTerms;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future_option", "EquityFutureOption")]
#[pyclass(
    name = "EquityFutureOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEquityFutureOption {
    pub(crate) inner: EquityFutureOption,
}

instrument_envelope_methods!(
    PyEquityFutureOption(
        "EquityFutureOption",
        module = "finstack_quant.valuations.instruments._typed_equity_future_option"
    ),
    EquityFutureOption,
    builder = PyEquityFutureOptionBuilder("EquityFutureOptionBuilder", no_fields),
    seed = EquityFutureOption::builder(),
);
instrument_pricing_methods!(
    PyEquityFutureOption,
    "EquityFutureOption",
    module = "finstack_quant.valuations.instruments._typed_equity_future_option"
);

#[pymethods]
impl PyEquityFutureOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future_option", "EquityFutureOption.to_builder")]
    fn to_builder(&self) -> PyEquityFutureOptionBuilder {
        PyEquityFutureOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future_option", "EquityFutureOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        EquityFutureOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future_option", "EquityFutureOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future_option", "EquityFutureOption.terms")]
    #[getter]
    fn terms(&self) -> PyFutureOptionTerms {
        PyFutureOptionTerms {
            inner: self.inner.terms.clone(),
        }
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future_option", "EquityFutureOptionBuilder")]
#[pyclass(
    name = "EquityFutureOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyEquityFutureOptionBuilder {
    inner: Option<EquityFutureOptionBuilder>,
}

#[pymethods]
impl PyEquityFutureOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future_option", "EquityFutureOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: EquityFutureOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future_option", "EquityFutureOptionBuilder.terms")]
    fn terms<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyFutureOptionTerms,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: EquityFutureOptionBuilder| b.terms(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future_option", "EquityFutureOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: EquityFutureOptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyEquityFutureOption>()?;
    module.add_class::<PyEquityFutureOptionBuilder>()?;
    Ok(())
}

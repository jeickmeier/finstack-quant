//! Typed options on listed futures.

use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::rates::ir_future_option::{
    InterestRateFutureOption, InterestRateFutureOptionBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_listed::PyFutureOptionTerms;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future_option", "InterestRateFutureOption")]
#[pyclass(
    name = "InterestRateFutureOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyInterestRateFutureOption {
    pub(crate) inner: InterestRateFutureOption,
}

instrument_envelope_methods!(
    PyInterestRateFutureOption(
        "InterestRateFutureOption",
        module = "finstack_quant.valuations.instruments._typed_interest_rate_future_option"
    ),
    InterestRateFutureOption,
    builder = PyInterestRateFutureOptionBuilder("InterestRateFutureOptionBuilder", no_fields),
    seed = InterestRateFutureOption::builder(),
);
instrument_pricing_methods!(
    PyInterestRateFutureOption,
    "InterestRateFutureOption",
    module = "finstack_quant.valuations.instruments._typed_interest_rate_future_option"
);

#[pymethods]
impl PyInterestRateFutureOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future_option", "InterestRateFutureOption.to_builder")]
    fn to_builder(&self) -> PyInterestRateFutureOptionBuilder {
        PyInterestRateFutureOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future_option", "InterestRateFutureOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        InterestRateFutureOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future_option", "InterestRateFutureOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future_option", "InterestRateFutureOption.terms")]
    #[getter]
    fn terms(&self) -> PyFutureOptionTerms {
        PyFutureOptionTerms {
            inner: self.inner.terms.clone(),
        }
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future_option", "InterestRateFutureOptionBuilder")]
#[pyclass(
    name = "InterestRateFutureOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyInterestRateFutureOptionBuilder {
    inner: Option<InterestRateFutureOptionBuilder>,
}

#[pymethods]
impl PyInterestRateFutureOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future_option", "InterestRateFutureOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: InterestRateFutureOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future_option", "InterestRateFutureOptionBuilder.terms")]
    fn terms<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyFutureOptionTerms,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: InterestRateFutureOptionBuilder| b.terms(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future_option", "InterestRateFutureOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: InterestRateFutureOptionBuilder| b
            .attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyInterestRateFutureOption>()?;
    module.add_class::<PyInterestRateFutureOptionBuilder>()?;
    Ok(())
}

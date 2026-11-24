//! A listed currency future with quote-currency variation margin.

use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fx::fx_future::{FxFuture, FxFutureBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_listed::PyListedFutureTerms;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFuture")]
#[pyclass(
    name = "FxFuture",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFxFuture {
    pub(crate) inner: FxFuture,
}

instrument_envelope_methods!(
    PyFxFuture(
        "FxFuture",
        module = "finstack_quant.valuations.instruments._typed_fx_future"
    ),
    FxFuture,
    builder = PyFxFutureBuilder("FxFutureBuilder", no_fields),
    seed = FxFuture::builder(),
);
instrument_pricing_methods!(
    PyFxFuture,
    "FxFuture",
    module = "finstack_quant.valuations.instruments._typed_fx_future"
);

#[pymethods]
impl PyFxFuture {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFuture.to_builder")]
    fn to_builder(&self) -> PyFxFutureBuilder {
        PyFxFutureBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFuture.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        FxFuture::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFuture.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }
}

crate::bindings::macros::py_getters!(PyFxFuture |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFuture.terms")]
    terms -> PyListedFutureTerms = PyListedFutureTerms {
        inner: s.inner.terms.clone(),
    };

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFuture.base_currency")]
    base_currency -> String = s.inner.base_currency.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFuture.domestic_discount_curve_id")]
    domestic_discount_curve_id -> String = s.inner.domestic_discount_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFuture.foreign_discount_curve_id")]
    foreign_discount_curve_id -> String = s.inner.foreign_discount_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFuture.quoted_spot")]
    quoted_spot -> Option<f64> = s.inner.quoted_spot;
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFutureBuilder")]
#[pyclass(
    name = "FxFutureBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyFxFutureBuilder {
    inner: Option<FxFutureBuilder>,
}

#[pymethods]
impl PyFxFutureBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFutureBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: FxFutureBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFutureBuilder.terms")]
    fn terms<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyListedFutureTerms,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: FxFutureBuilder| b.terms(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFutureBuilder.base_currency")]
    fn base_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: FxFutureBuilder| b.base_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFutureBuilder.domestic_discount_curve_id")]
    fn domestic_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = CurveId::new(value);
        builder_set!(slf, |b: FxFutureBuilder| b
            .domestic_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFutureBuilder.foreign_discount_curve_id")]
    fn foreign_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = CurveId::new(value);
        builder_set!(slf, |b: FxFutureBuilder| b.foreign_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFutureBuilder.quoted_spot")]
    fn quoted_spot<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FxFutureBuilder| b.quoted_spot_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_future", "FxFutureBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: FxFutureBuilder| b.attributes(value))
    }
}

/// Register the FX future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFxFuture>()?;
    module.add_class::<PyFxFutureBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["FxFuture", "FxFutureBuilder"];

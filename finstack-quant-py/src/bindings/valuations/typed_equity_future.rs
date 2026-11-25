//! A listed future on an equity or index.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::equity::equity_future::{
    EquityFuture, EquityFutureBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_listed::PyListedFutureTerms;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFuture")]
#[pyclass(
    name = "EquityFuture",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEquityFuture {
    pub(crate) inner: EquityFuture,
}

instrument_envelope_methods!(
    PyEquityFuture(
        "EquityFuture",
        module = "finstack_quant.valuations.instruments._typed_equity_future"
    ),
    EquityFuture,
    builder = PyEquityFutureBuilder("EquityFutureBuilder", no_fields),
    seed = EquityFuture::builder(),
);
instrument_pricing_methods!(
    PyEquityFuture,
    "EquityFuture",
    module = "finstack_quant.valuations.instruments._typed_equity_future"
);

#[pymethods]
impl PyEquityFuture {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFuture.to_builder")]
    fn to_builder(&self) -> PyEquityFutureBuilder {
        PyEquityFutureBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFuture.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        EquityFuture::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFuture.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFuture.discrete_dividends")]
    #[getter]
    fn discrete_dividends<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .discrete_dividends
            .iter()
            .map(|(date, amount)| {
                crate::bindings::date_utils::date_to_py(py, *date)
                    .map(|day| (day.into_any(), *amount))
            })
            .collect()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFuture.quanto")]
    #[getter]
    fn quanto<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.quanto)
    }
}

crate::bindings::macros::py_getters!(PyEquityFuture |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFuture.terms")]
    terms -> PyListedFutureTerms = PyListedFutureTerms {
        inner: s.inner.terms.clone(),
    };

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFuture.underlying_ticker")]
    underlying_ticker -> String = s.inner.underlying_ticker.clone();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFuture.underlying_currency")]
    underlying_currency -> String = s.inner.underlying_currency.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFuture.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFuture.spot_id")]
    spot_id -> String = s.inner.spot_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFuture.div_yield_id")]
    div_yield_id -> Option<String> = s.inner.div_yield_id.as_ref().map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFuture.vol_surface_id")]
    vol_surface_id -> Option<String> = s.inner.vol_surface_id.as_ref().map(ToString::to_string);
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFutureBuilder")]
#[pyclass(
    name = "EquityFutureBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyEquityFutureBuilder {
    inner: Option<EquityFutureBuilder>,
}

#[pymethods]
impl PyEquityFutureBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFutureBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: EquityFutureBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFutureBuilder.terms")]
    fn terms<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyListedFutureTerms,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: EquityFutureBuilder| b.terms(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFutureBuilder.underlying_ticker")]
    fn underlying_ticker<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "underlying_ticker")?;
        builder_set!(slf, |b: EquityFutureBuilder| b.underlying_ticker(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFutureBuilder.underlying_currency")]
    fn underlying_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: EquityFutureBuilder| b.underlying_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFutureBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "discount_curve_id")?;
        builder_set!(slf, |b: EquityFutureBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFutureBuilder.spot_id")]
    fn spot_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "spot_id")?;
        builder_set!(slf, |b: EquityFutureBuilder| b.spot_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFutureBuilder.div_yield_id")]
    fn div_yield_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "div_yield_id")?;
        builder_set!(slf, |b: EquityFutureBuilder| b.div_yield_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFutureBuilder.discrete_dividends")]
    fn discrete_dividends<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "discrete_dividends")?;
        builder_set!(slf, |b: EquityFutureBuilder| b.discrete_dividends(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFutureBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "vol_surface_id")?;
        builder_set!(slf, |b: EquityFutureBuilder| b.vol_surface_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFutureBuilder.quanto")]
    fn quanto<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "quanto")?;
        builder_set!(slf, |b: EquityFutureBuilder| b.quanto_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_future", "EquityFutureBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: EquityFutureBuilder| b.attributes(value))
    }
}

/// Register the equity future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyEquityFuture>()?;
    module.add_class::<PyEquityFutureBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["EquityFuture", "EquityFutureBuilder"];

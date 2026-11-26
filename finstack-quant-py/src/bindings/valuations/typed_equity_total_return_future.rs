//! A listed futures contract referencing an equity total-return index.

use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::equity::equity_total_return_future::{
    EquityTotalReturnFuture, EquityTotalReturnFutureBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_trait_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFuture")]
#[pyclass(
    name = "EquityTotalReturnFuture",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEquityTotalReturnFuture {
    pub(crate) inner: EquityTotalReturnFuture,
}

instrument_envelope_methods!(
    PyEquityTotalReturnFuture(
        "EquityTotalReturnFuture",
        module = "finstack_quant.valuations.instruments._typed_equity_total_return_future"
    ),
    EquityTotalReturnFuture,
    builder = PyEquityTotalReturnFutureBuilder("EquityTotalReturnFutureBuilder", no_fields),
    seed = EquityTotalReturnFuture::builder(),
);
instrument_trait_methods!(
    PyEquityTotalReturnFuture,
    "EquityTotalReturnFuture",
    module = "finstack_quant.valuations.instruments._typed_equity_total_return_future"
);

#[pymethods]
impl PyEquityTotalReturnFuture {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFuture.to_builder")]
    fn to_builder(&self) -> PyEquityTotalReturnFutureBuilder {
        PyEquityTotalReturnFutureBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFuture.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        EquityTotalReturnFuture::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFuture.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFuture.underlying_ticker")]
    #[getter]
    fn underlying_ticker(&self) -> String {
        self.inner.underlying_ticker.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFuture.terms")]
    #[getter]
    fn terms(&self) -> super::typed_listed::PyListedFutureTerms {
        super::typed_listed::PyListedFutureTerms {
            inner: self.inner.terms.clone(),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFuture.spot_id")]
    #[getter]
    fn spot_id(&self) -> String {
        self.inner.spot_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFuture.accrued_distributions_id")]
    #[getter]
    fn accrued_distributions_id(&self) -> String {
        self.inner.accrued_distributions_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFuture.accrued_funding_id")]
    #[getter]
    fn accrued_funding_id(&self) -> String {
        self.inner.accrued_funding_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFuture.spread_bp_id")]
    #[getter]
    fn spread_bp_id(&self) -> String {
        self.inner.spread_bp_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFuture.spread_day_count")]
    #[getter]
    fn spread_day_count(&self) -> crate::bindings::core::dates::daycount::PyDayCount {
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(self.inner.spread_day_count)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFutureBuilder")]
#[pyclass(
    name = "EquityTotalReturnFutureBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyEquityTotalReturnFutureBuilder {
    inner: Option<EquityTotalReturnFutureBuilder>,
}

#[pymethods]
impl PyEquityTotalReturnFutureBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFutureBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: EquityTotalReturnFutureBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFutureBuilder.underlying_ticker")]
    fn underlying_ticker<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: EquityTotalReturnFutureBuilder| b
            .underlying_ticker(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFutureBuilder.terms")]
    fn terms<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &super::typed_listed::PyListedFutureTerms,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: EquityTotalReturnFutureBuilder| b.terms(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFutureBuilder.spot_id")]
    fn spot_id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: EquityTotalReturnFutureBuilder| b.spot_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFutureBuilder.accrued_distributions_id")]
    fn accrued_distributions_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: EquityTotalReturnFutureBuilder| b
            .accrued_distributions_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFutureBuilder.accrued_funding_id")]
    fn accrued_funding_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: EquityTotalReturnFutureBuilder| b
            .accrued_funding_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFutureBuilder.spread_bp_id")]
    fn spread_bp_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: EquityTotalReturnFutureBuilder| b
            .spread_bp_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFutureBuilder.spread_day_count")]
    fn spread_day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "spread_day_count")?;
        builder_set!(slf, |b: EquityTotalReturnFutureBuilder| b
            .spread_day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_future", "EquityTotalReturnFutureBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: EquityTotalReturnFutureBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyEquityTotalReturnFuture>()?;
    module.add_class::<PyEquityTotalReturnFutureBuilder>()?;
    Ok(())
}

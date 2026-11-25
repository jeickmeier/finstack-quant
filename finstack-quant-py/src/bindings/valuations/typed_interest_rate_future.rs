//! A listed future on a short-term rate.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::rates::ir_future::{
    InterestRateFuture, InterestRateFutureBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_listed::PyListedFutureTerms;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture")]
#[pyclass(
    name = "InterestRateFuture",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyInterestRateFuture {
    pub(crate) inner: InterestRateFuture,
}

instrument_envelope_methods!(
    PyInterestRateFuture(
        "InterestRateFuture",
        module = "finstack_quant.valuations.instruments._typed_interest_rate_future"
    ),
    InterestRateFuture,
    builder = PyInterestRateFutureBuilder("InterestRateFutureBuilder", no_fields),
    seed = InterestRateFuture::builder(),
);
instrument_pricing_methods!(
    PyInterestRateFuture,
    "InterestRateFuture",
    module = "finstack_quant.valuations.instruments._typed_interest_rate_future"
);

#[pymethods]
impl PyInterestRateFuture {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.to_builder")]
    fn to_builder(&self) -> PyInterestRateFutureBuilder {
        PyInterestRateFutureBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        InterestRateFuture::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.fixing_date")]
    #[getter]
    fn fixing_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .fixing_date
            .map(|d| crate::bindings::date_utils::date_to_py(py, d))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.period_start")]
    #[getter]
    fn period_start<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .period_start
            .map(|d| crate::bindings::date_utils::date_to_py(py, d))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.period_end")]
    #[getter]
    fn period_end<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .period_end
            .map(|d| crate::bindings::date_utils::date_to_py(py, d))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.contract_specs")]
    #[getter]
    fn contract_specs<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.contract_specs)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.compounding")]
    #[getter]
    fn compounding<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.compounding)
    }
}

crate::bindings::macros::py_getters!(PyInterestRateFuture |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.terms")]
    terms -> PyListedFutureTerms = PyListedFutureTerms {
        inner: s.inner.terms.clone(),
    };

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.day_count")]
    day_count -> crate::bindings::core::dates::daycount::PyDayCount =
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(s.inner.day_count);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.forward_curve_id")]
    forward_curve_id -> String = s.inner.forward_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.index_id")]
    index_id -> Option<String> = s.inner.index_id.as_ref().map(|id| id.to_string());

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.fixing_calendar_id")]
    fixing_calendar_id -> Option<String> =
        s.inner.fixing_calendar_id.as_ref().map(|id| id.to_string());

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFuture.vol_surface_id")]
    vol_surface_id -> Option<String> = s.inner.vol_surface_id.as_ref().map(|id| id.to_string());
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder")]
#[pyclass(
    name = "InterestRateFutureBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyInterestRateFutureBuilder {
    inner: Option<InterestRateFutureBuilder>,
}

#[pymethods]
impl PyInterestRateFutureBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: InterestRateFutureBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder.terms")]
    fn terms<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyListedFutureTerms,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: InterestRateFutureBuilder| b.terms(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder.fixing_date")]
    fn fixing_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "fixing_date")?;
        builder_set!(slf, |b: InterestRateFutureBuilder| b.fixing_date_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder.period_start")]
    fn period_start<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "period_start")?;
        builder_set!(slf, |b: InterestRateFutureBuilder| b
            .period_start_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder.period_end")]
    fn period_end<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "period_end")?;
        builder_set!(slf, |b: InterestRateFutureBuilder| b.period_end_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "day_count")?;
        builder_set!(slf, |b: InterestRateFutureBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder.contract_specs")]
    fn contract_specs<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "contract_specs")?;
        builder_set!(slf, |b: InterestRateFutureBuilder| b.contract_specs(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "discount_curve_id")?;
        builder_set!(slf, |b: InterestRateFutureBuilder| b
            .discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder.forward_curve_id")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "forward_curve_id")?;
        builder_set!(slf, |b: InterestRateFutureBuilder| b
            .forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder.compounding")]
    fn compounding<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "compounding")?;
        builder_set!(slf, |b: InterestRateFutureBuilder| b.compounding(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder.index_id")]
    fn index_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "index_id")?;
        builder_set!(slf, |b: InterestRateFutureBuilder| b.index_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder.fixing_calendar_id")]
    fn fixing_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "fixing_calendar_id")?;
        builder_set!(slf, |b: InterestRateFutureBuilder| b
            .fixing_calendar_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "vol_surface_id")?;
        builder_set!(slf, |b: InterestRateFutureBuilder| b
            .vol_surface_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_interest_rate_future", "InterestRateFutureBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: InterestRateFutureBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyInterestRateFuture>()?;
    module.add_class::<PyInterestRateFutureBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["InterestRateFuture", "InterestRateFutureBuilder"];

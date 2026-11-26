//! A forward purchase or sale of agency mortgage collateral under TBA terms.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fixed_income::tba::{AgencyTba, AgencyTbaBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_trait_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba")]
#[pyclass(
    name = "AgencyTba",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyAgencyTba {
    pub(crate) inner: AgencyTba,
}

instrument_envelope_methods!(
    PyAgencyTba(
        "AgencyTba",
        module = "finstack_quant.valuations.instruments._typed_agency_tba"
    ),
    AgencyTba,
    builder = PyAgencyTbaBuilder("AgencyTbaBuilder", no_fields),
    seed = AgencyTba::builder(),
);
instrument_trait_methods!(
    PyAgencyTba,
    "AgencyTba",
    module = "finstack_quant.valuations.instruments._typed_agency_tba"
);

#[pymethods]
impl PyAgencyTba {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.to_builder")]
    fn to_builder(&self) -> PyAgencyTbaBuilder {
        PyAgencyTbaBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        AgencyTba::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.agency")]
    #[getter]
    fn agency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.agency)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.term")]
    #[getter]
    fn term<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.term)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.settlement_class")]
    #[getter]
    fn settlement_class<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.settlement_class)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.settlement_date")]
    #[getter]
    fn settlement_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .settlement_date
            .map(|value| crate::bindings::date_utils::date_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.trade_date")]
    #[getter]
    fn trade_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .trade_date
            .map(|value| crate::bindings::date_utils::date_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.assumed_pool")]
    #[getter]
    fn assumed_pool<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.assumed_pool)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.prepayment_spec")]
    #[getter]
    fn prepayment_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.prepayment_spec)
    }
}

crate::bindings::macros::py_getters!(PyAgencyTba |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.coupon")]
    coupon -> f64 = s.inner.coupon;
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.settlement_year")]
    settlement_year -> i32 = s.inner.settlement_year;
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.settlement_month")]
    settlement_month -> u8 = s.inner.settlement_month;
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.notional")]
    notional -> PyMoney = PyMoney::from_inner(s.inner.notional);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.trade_price")]
    trade_price -> f64 = s.inner.trade_price;
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.pool_factor")]
    pool_factor -> Option<f64> = s.inner.pool_factor;
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTba.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder")]
#[pyclass(
    name = "AgencyTbaBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyAgencyTbaBuilder {
    inner: Option<AgencyTbaBuilder>,
}

#[pymethods]
impl PyAgencyTbaBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: AgencyTbaBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.agency")]
    fn agency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "agency")?;
        builder_set!(slf, |b: AgencyTbaBuilder| b.agency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.coupon")]
    fn coupon<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AgencyTbaBuilder| b.coupon(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.term")]
    fn term<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "term")?;
        builder_set!(slf, |b: AgencyTbaBuilder| b.term(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.settlement_year")]
    fn settlement_year<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: i32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AgencyTbaBuilder| b.settlement_year(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.settlement_month")]
    fn settlement_month<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: u8,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AgencyTbaBuilder| b.settlement_month(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.settlement_class")]
    fn settlement_class<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "settlement_class")?;
        builder_set!(slf, |b: AgencyTbaBuilder| b.settlement_class(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.settlement_date")]
    fn settlement_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(crate::bindings::date_utils::py_to_date)
            .transpose()?;
        builder_set!(slf, |b: AgencyTbaBuilder| b.settlement_date_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: AgencyTbaBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.trade_price")]
    fn trade_price<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AgencyTbaBuilder| b.trade_price(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.trade_date")]
    fn trade_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(crate::bindings::date_utils::py_to_date)
            .transpose()?;
        builder_set!(slf, |b: AgencyTbaBuilder| b.trade_date_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.pool_factor")]
    fn pool_factor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AgencyTbaBuilder| b.pool_factor_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.assumed_pool")]
    fn assumed_pool<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "assumed_pool")?;
        builder_set!(slf, |b: AgencyTbaBuilder| b.assumed_pool_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.prepayment_spec")]
    fn prepayment_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "prepayment_spec")?;
        builder_set!(slf, |b: AgencyTbaBuilder| b.prepayment_spec_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: AgencyTbaBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_tba", "AgencyTbaBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: AgencyTbaBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyAgencyTba>()?;
    module.add_class::<PyAgencyTbaBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["AgencyTba", "AgencyTbaBuilder"];

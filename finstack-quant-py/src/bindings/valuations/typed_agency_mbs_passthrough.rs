//! An agency mortgage pass-through with projected scheduled and prepaid principal.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::{InstrumentId, PoolId};
use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::{
    AgencyMbsPassthrough, AgencyMbsPassthroughBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_trait_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough")]
#[pyclass(
    name = "AgencyMbsPassthrough",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyAgencyMbsPassthrough {
    pub(crate) inner: AgencyMbsPassthrough,
}

instrument_envelope_methods!(
    PyAgencyMbsPassthrough(
        "AgencyMbsPassthrough",
        module = "finstack_quant.valuations.instruments._typed_agency_mbs_passthrough"
    ),
    AgencyMbsPassthrough,
    builder = PyAgencyMbsPassthroughBuilder("AgencyMbsPassthroughBuilder", no_fields),
    seed = AgencyMbsPassthrough::builder(),
);
instrument_trait_methods!(
    PyAgencyMbsPassthrough,
    "AgencyMbsPassthrough",
    module = "finstack_quant.valuations.instruments._typed_agency_mbs_passthrough"
);

#[pymethods]
impl PyAgencyMbsPassthrough {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.to_builder")]
    fn to_builder(&self) -> PyAgencyMbsPassthroughBuilder {
        PyAgencyMbsPassthroughBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        AgencyMbsPassthrough::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.agency")]
    #[getter]
    fn agency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.agency)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.pool_type")]
    #[getter]
    fn pool_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.pool_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.issue_date")]
    #[getter]
    fn issue_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.issue_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.last_paid_accrual_end")]
    #[getter]
    fn last_paid_accrual_end<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .last_paid_accrual_end
            .map(|value| crate::bindings::date_utils::date_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.maturity)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.prepayment_spec")]
    #[getter]
    fn prepayment_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.prepayment_spec)
    }
}

crate::bindings::macros::py_getters!(PyAgencyMbsPassthrough |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.pool_id")]
    pool_id -> String = s.inner.pool_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.original_face")]
    original_face -> PyMoney = PyMoney::from_inner(s.inner.original_face);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.current_face")]
    current_face -> PyMoney = PyMoney::from_inner(s.inner.current_face);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.wac")]
    wac -> f64 = s.inner.wac;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.coupon")]
    coupon -> f64 = s.inner.coupon;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.servicing_fee_bp")]
    servicing_fee_bp -> f64 = s.inner.servicing_fee_bp;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.guarantee_fee_bp")]
    guarantee_fee_bp -> f64 = s.inner.guarantee_fee_bp;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.wam_months")]
    wam_months -> u32 = s.inner.wam_months;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.stated_delay_days")]
    stated_delay_days -> Option<u32> = s.inner.stated_delay_days;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthrough.day_count")]
    day_count -> crate::bindings::core::dates::daycount::PyDayCount =
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(s.inner.day_count);
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder")]
#[pyclass(
    name = "AgencyMbsPassthroughBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyAgencyMbsPassthroughBuilder {
    inner: Option<AgencyMbsPassthroughBuilder>,
}

#[pymethods]
impl PyAgencyMbsPassthroughBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.pool_id")]
    fn pool_id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = PoolId::new(value);
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b.pool_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.agency")]
    fn agency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "agency")?;
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b.agency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.pool_type")]
    fn pool_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "pool_type")?;
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b.pool_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.original_face")]
    fn original_face<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b.original_face(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.current_face")]
    fn current_face<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b.current_face(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.wac")]
    fn wac<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b.wac(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.coupon")]
    fn coupon<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b.coupon(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.servicing_fee_bp")]
    fn servicing_fee_bp<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b
            .servicing_fee_bp(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.guarantee_fee_bp")]
    fn guarantee_fee_bp<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b
            .guarantee_fee_bp(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.wam_months")]
    fn wam_months<'py>(mut slf: PyRefMut<'py, Self>, value: u32) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b.wam_months(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.issue_date")]
    fn issue_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b.issue_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.last_paid_accrual_end")]
    fn last_paid_accrual_end<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(crate::bindings::date_utils::py_to_date)
            .transpose()?;
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b
            .last_paid_accrual_end_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.maturity")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b.maturity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.stated_delay_days")]
    fn stated_delay_days<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<u32>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b
            .stated_delay_days_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.prepayment_spec")]
    fn prepayment_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "prepayment_spec")?;
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b
            .prepayment_spec(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b
            .discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_mbs_passthrough", "AgencyMbsPassthroughBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: AgencyMbsPassthroughBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyAgencyMbsPassthrough>()?;
    module.add_class::<PyAgencyMbsPassthroughBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["AgencyMbsPassthrough", "AgencyMbsPassthroughBuilder"];

use pyo3::prelude::*;

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_rate_decimal;
use crate::bindings::valuations::convert::{
    attributes_from_py, attributes_to_py, bool_repr, enum_to_py_string, money_to_py, opt_repr,
};
use crate::errors::core_to_py;
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_core::types::CreditRating;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    CoverageTrigger, Tranche,
};
use finstack_quant_valuations::results::structured_credit::TrancheSeniority;

use super::super::convert::enum_from_str;
use super::super::typed_macros::builder_set;

use finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "Tranche",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyTranche {
    /// Inner canonical Rust tranche.
    pub(crate) inner: Tranche,
}

#[pymethods]
impl PyTranche {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.builder")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builder() -> PyTrancheBuilder {
        PyTrancheBuilder {
            inner: Some(Tranche::builder()),
            attach_pct: None,
            detach_pct: None,
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| crate::errors::serde_json_to_py(err, "invalid Tranche JSON"))?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(crate::errors::display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.rating")]
    #[getter]
    fn rating(&self) -> PyResult<Option<String>> {
        self.inner
            .rating
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.coupon")]
    #[getter]
    fn coupon<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.coupon)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.oc_trigger")]
    #[getter]
    fn oc_trigger<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .oc_trigger
            .as_ref()
            .map(|trigger| crate::bindings::pandas_utils::serde_to_py(py, trigger))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.ic_trigger")]
    #[getter]
    fn ic_trigger<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .ic_trigger
            .as_ref()
            .map(|trigger| crate::bindings::pandas_utils::serde_to_py(py, trigger))
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "Tranche(id='{}', seniority='{}', attach_pct={}, detach_pct={}, original_balance={}, maturity='{}', pik_enabled={})",
            self.inner.id.as_str(),
            enum_to_py_string(&self.inner.seniority).unwrap_or_default(),
            opt_repr(self.inner.attach_pct),
            opt_repr(self.inner.detach_pct),
            self.inner.original_balance.amount(),
            self.inner.maturity,
            bool_repr(self.inner.pik_enabled),
        )
    }
}

crate::bindings::macros::py_getters!(PyTranche |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.id")]
    id -> String = s.inner.id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.attach_pct")]
    attach_pct -> Option<f64> = s.inner.attach_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.detach_pct")]
    detach_pct -> Option<f64> = s.inner.detach_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.seniority")]
    seniority -> PyResult<String> = enum_to_py_string(&s.inner.seniority);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.original_balance")]
    original_balance -> PyMoney = money_to_py(s.inner.original_balance);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.current_balance")]
    current_balance -> PyMoney = money_to_py(s.inner.current_balance);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.frequency")]
    frequency -> PyTenor = PyTenor::from_inner(s.inner.frequency);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.deferred_interest")]
    deferred_interest -> PyMoney = money_to_py(s.inner.deferred_interest);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.pik_enabled")]
    pik_enabled -> bool = s.inner.pik_enabled;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.non_deferrable")]
    non_deferrable -> Option<bool> = s.inner.non_deferrable;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.is_non_deferrable")]
    is_non_deferrable -> bool = s.inner.is_non_deferrable();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.payment_priority")]
    payment_priority -> u32 = s.inner.payment_priority;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "Tranche.attributes")]
    attributes -> crate::bindings::core::types::PyAttributes =
        attributes_to_py(&s.inner.attributes);
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "TrancheBuilder",
    skip_from_py_object
)]
pub struct PyTrancheBuilder {
    inner: Option<TrancheBuilder>,
    attach_pct: Option<f64>,
    detach_pct: Option<f64>,
}

#[pymethods]
impl PyTrancheBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: TrancheBuilder| b.id(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.attach_pct")]
    #[pyo3(text_signature = "($self, value)")]
    fn attach_pct<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        let b = crate::bindings::valuations::convert::take_builder(&mut slf.inner)?;
        slf.inner = Some(b.attach_pct(value));
        slf.attach_pct = Some(value);
        Ok(slf)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.detach_pct")]
    #[pyo3(text_signature = "($self, value)")]
    fn detach_pct<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        let b = crate::bindings::valuations::convert::take_builder(&mut slf.inner)?;
        slf.inner = Some(b.detach_pct(value));
        slf.detach_pct = Some(value);
        Ok(slf)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.seniority")]
    #[pyo3(text_signature = "($self, value)")]
    fn seniority<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let seniority: TrancheSeniority = enum_from_str(value, "seniority")?;
        builder_set!(slf, |b: TrancheBuilder| b.seniority(seniority))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.original_balance")]
    #[pyo3(text_signature = "($self, value)")]
    fn original_balance<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: TrancheBuilder| b.balance(value.inner))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.coupon_fixed")]
    #[pyo3(text_signature = "($self, rate)")]
    fn coupon_fixed<'py>(
        mut slf: PyRefMut<'py, Self>,
        rate: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let rate = extract_rate_decimal(rate, "rate")?;
        builder_set!(slf, |b: TrancheBuilder| b.coupon(RateSpec::Fixed { rate }))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.coupon_floating")]
    #[pyo3(text_signature = "($self, value)")]
    fn coupon_floating<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let coupon: RateSpec = crate::bindings::module_utils::py_to_serde(py, value, "coupon")?;
        builder_set!(slf, |b: TrancheBuilder| b.coupon(coupon))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.maturity")]
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let maturity = py_to_date(value)?;
        builder_set!(slf, |b: TrancheBuilder| b.maturity(maturity))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.frequency")]
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let frequency = crate::bindings::core::dates::tenor::extract_tenor(value)?;
        builder_set!(slf, |b: TrancheBuilder| b.frequency(frequency))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.day_count")]
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count =
            crate::bindings::valuations::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: TrancheBuilder| b.day_count(day_count))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.current_balance")]
    #[pyo3(text_signature = "($self, value)")]
    fn current_balance<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: TrancheBuilder| b.current_balance(value.inner))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.deferred_interest")]
    #[pyo3(text_signature = "($self, value)")]
    fn deferred_interest<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: TrancheBuilder| b.deferred_interest(value.inner))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.pik_enabled")]
    #[pyo3(text_signature = "($self, value)")]
    fn pik_enabled<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: TrancheBuilder| b.pik_enabled(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.non_deferrable")]
    #[pyo3(text_signature = "($self, value)")]
    fn non_deferrable<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: TrancheBuilder| b.non_deferrable(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.rating")]
    #[pyo3(text_signature = "($self, value)")]
    fn rating<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let rating: CreditRating = value.parse().map_err(core_to_py)?;
        builder_set!(slf, |b: TrancheBuilder| b.rating(rating))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.oc_trigger")]
    #[pyo3(text_signature = "($self, value)")]
    fn oc_trigger<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let trigger: CoverageTrigger =
            crate::bindings::module_utils::py_to_serde(py, value, "oc_trigger")?;
        builder_set!(slf, |b: TrancheBuilder| b.oc_trigger(trigger))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.ic_trigger")]
    #[pyo3(text_signature = "($self, value)")]
    fn ic_trigger<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let trigger: CoverageTrigger =
            crate::bindings::module_utils::py_to_serde(py, value, "ic_trigger")?;
        builder_set!(slf, |b: TrancheBuilder| b.ic_trigger(trigger))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attributes = attributes_from_py(value)?;
        builder_set!(slf, |b: TrancheBuilder| b.attributes(attributes))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche", "TrancheBuilder.build")]
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyTranche> {
        let b = crate::bindings::valuations::convert::take_builder(&mut slf.inner)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyTranche { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "TrancheBuilder(attach_pct={}, detach_pct={}, consumed={})",
            opt_repr(self.attach_pct),
            opt_repr(self.detach_pct),
            bool_repr(self.inner.is_none()),
        )
    }
}

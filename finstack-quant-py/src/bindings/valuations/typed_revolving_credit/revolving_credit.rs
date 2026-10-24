//! Typed `RevolvingCredit` instrument class and its fluent builder.

use pyo3::prelude::*;

use crate::bindings::cashflows::builder::schedule::PyCashFlowSchedule;
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_market;
use crate::bindings::module_utils::{opt_spec_from_py, spec_from_py};
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::convert::{
    attributes_from_py, bdc_from_py, day_count_from_py, enum_to_py_string, money_from_py,
    money_repr, money_to_py,
};
use crate::bindings::valuations::convert::{instrument_expiry, stub_kind_from_py};
use crate::bindings::valuations::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_pricing_methods,
};
use crate::errors::core_to_py;
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_cashflows::CashflowScheduleSource;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCreditPricer;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};

use super::PyEnhancedMonteCarloResult;

use finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCreditBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "RevolvingCredit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRevolvingCredit {
    /// Inner canonical Rust facility.
    pub(crate) inner: RevolvingCredit,
}

instrument_envelope_methods!(
    PyRevolvingCredit("RevolvingCredit"),
    RevolvingCredit,
    builder = PyRevolvingCreditBuilder("RevolvingCreditBuilder", fields),
    seed = RevolvingCredit::builder(),
);
instrument_pricing_methods!(PyRevolvingCredit, "RevolvingCredit");

#[pymethods]
impl PyRevolvingCredit {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        RevolvingCredit::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.price_with_paths")]
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn price_with_paths(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyEnhancedMonteCarloResult> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let facility = self.inner.clone();
        let inner = py
            .detach(move || RevolvingCreditPricer::price_with_paths(&facility, &market, as_of))
            .map_err(core_to_py)?;
        Ok(PyEnhancedMonteCarloResult { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.expected_cashflows")]
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn expected_cashflows(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyCashFlowSchedule> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let facility = self.inner.clone();
        let schedule = py
            .detach(move || facility.raw_cashflow_schedule(&market, as_of))
            .map_err(core_to_py)?;
        Ok(PyCashFlowSchedule::from_inner(schedule))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.issue_date")]
    #[getter]
    fn issue_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.issue_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.rate")]
    #[getter]
    fn rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.rate)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.fees")]
    #[getter]
    fn fees<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.fees)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.commitment_steps")]
    #[getter]
    fn commitment_steps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.commitment_steps)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.margin_steps")]
    #[getter]
    fn margin_steps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.margin_steps)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.scheduled_fees")]
    #[getter]
    fn scheduled_fees<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.scheduled_fees)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.oid_eir")]
    #[getter]
    fn oid_eir<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .oid_eir
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.lc")]
    #[getter]
    fn lc<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .lc
            .as_ref()
            .map(|lc| serde_to_py(py, lc))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.draw_repay_spec")]
    #[getter]
    fn draw_repay_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.draw_repay_spec)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RevolvingCredit(id={:?}, commitment={}, drawn={}, maturity={}, stochastic={})",
            self.inner.id.as_str(),
            money_repr(self.inner.commitment),
            money_repr(self.inner.drawn),
            self.inner.maturity,
            self.inner.is_stochastic(),
        )
    }
}

crate::bindings::macros::py_getters!(PyRevolvingCredit |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.commitment")]
    commitment -> PyMoney = money_to_py(s.inner.commitment);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.drawn")]
    drawn -> PyMoney = money_to_py(s.inner.drawn);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.frequency")]
    frequency -> PyTenor = PyTenor::from_inner(s.inner.frequency);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.is_stochastic")]
    is_stochastic -> bool = s.inner.is_stochastic();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.credit_curve_id")]
    credit_curve_id -> Option<String> = s.inner.credit_curve_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.recovery_rate")]
    recovery_rate -> f64 = s.inner.recovery_rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.leq")]
    leq -> f64 = s.inner.leq;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.stub")]
    stub -> PyStubKind = PyStubKind::from_inner(s.inner.stub);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.business_day_convention")]
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.payment_lag_days")]
    payment_lag_days -> u32 = s.inner.payment_lag_days;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCredit.settlement_days")]
    settlement_days -> u32 = s.inner.settlement_days;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "RevolvingCreditBuilder",
    skip_from_py_object
)]
pub struct PyRevolvingCreditBuilder {
    inner: Option<RevolvingCreditBuilder>,
    fields: Vec<(&'static str, String)>,
}

#[pymethods]
impl PyRevolvingCreditBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            id,
            format!("{value:?}"),
            |b: RevolvingCreditBuilder| b.id(InstrumentId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.commitment")]
    #[pyo3(signature = (value, currency=None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn commitment<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "commitment")?;
        builder_set!(
            slf,
            commitment,
            money_repr(money),
            |b: RevolvingCreditBuilder| b.commitment(money)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.drawn")]
    #[pyo3(signature = (value, currency=None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn drawn<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "drawn")?;
        builder_set!(
            slf,
            drawn,
            money_repr(money),
            |b: RevolvingCreditBuilder| b.drawn(money)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.issue_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn issue_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(
            slf,
            issue_date,
            date.to_string(),
            |b: RevolvingCreditBuilder| b.issue_date(date)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.maturity")]
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(
            slf,
            maturity,
            date.to_string(),
            |b: RevolvingCreditBuilder| b.maturity(date)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.rate")]
    #[pyo3(text_signature = "($self, value)")]
    fn rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: RateSpec =
            if value.extract::<&str>().is_ok() || value.cast::<pyo3::types::PyDict>().is_ok() {
                spec_from_py(py, value, "rate")?
            } else {
                let rate = super::super::convert::rate_from_py(value, "rate")?;
                RateSpec::fixed_rate(rate)
            };
        let repr = serde_json::to_string(&spec).unwrap_or_default();
        builder_set!(slf, rate, repr, |b: RevolvingCreditBuilder| b.rate(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.day_count")]
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count = day_count_from_py(value, "day_count")?;
        builder_set!(
            slf,
            day_count,
            format!("{day_count:?}"),
            |b: RevolvingCreditBuilder| b.day_count(day_count)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.frequency")]
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let tenor = crate::bindings::core::dates::tenor::extract_tenor(value)?;
        builder_set!(
            slf,
            frequency,
            format!("{tenor:?}"),
            |b: RevolvingCreditBuilder| b.frequency(tenor)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.fees")]
    #[pyo3(text_signature = "($self, value)")]
    fn fees<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let fees: RevolvingCreditFees = spec_from_py(py, value, "fees")?;
        builder_set!(
            slf,
            fees,
            "{..}".to_string(),
            |b: RevolvingCreditBuilder| b.fees(fees)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.commitment_steps")]
    #[pyo3(text_signature = "($self, value)")]
    fn commitment_steps<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let steps: Vec<
            finstack_quant_valuations::instruments::fixed_income::loan_terms::CommitmentStep,
        > = spec_from_py(py, value, "commitment_steps")?;
        builder_set!(
            slf,
            commitment_steps,
            "[..]".to_string(),
            |b: RevolvingCreditBuilder| b.commitment_steps(steps)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.scheduled_fees")]
    #[pyo3(text_signature = "($self, value)")]
    fn scheduled_fees<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let fees: Vec<
            finstack_quant_valuations::instruments::fixed_income::loan_terms::ScheduledFee,
        > = spec_from_py(py, value, "scheduled_fees")?;
        builder_set!(
            slf,
            scheduled_fees,
            "[..]".to_string(),
            |b: RevolvingCreditBuilder| b.scheduled_fees(fees)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.oid_eir")]
    #[pyo3(signature = (value))]
    #[pyo3(text_signature = "($self, value)")]
    fn oid_eir<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: Option<
            finstack_quant_valuations::instruments::fixed_income::loan_terms::OidEirSpec,
        > = opt_spec_from_py(py, value, "oid_eir")?;
        let repr = if spec.is_some() { "{..}" } else { "None" };
        builder_set!(
            slf,
            oid_eir,
            repr.to_string(),
            |b: RevolvingCreditBuilder| b.oid_eir_opt(spec)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.lc")]
    #[pyo3(signature = (value))]
    #[pyo3(text_signature = "($self, value)")]
    fn lc<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let lc: Option<
            finstack_quant_valuations::instruments::fixed_income::loan_terms::LetterOfCreditSpec,
        > = opt_spec_from_py(py, value, "lc")?;
        let repr = if lc.is_some() { "{..}" } else { "None" };
        builder_set!(slf, lc, repr.to_string(), |b: RevolvingCreditBuilder| b
            .lc_opt(lc))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.margin_steps")]
    #[pyo3(text_signature = "($self, value)")]
    fn margin_steps<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let steps: Vec<
            finstack_quant_valuations::instruments::fixed_income::loan_terms::MarginStep,
        > = spec_from_py(py, value, "margin_steps")?;
        builder_set!(
            slf,
            margin_steps,
            "[..]".to_string(),
            |b: RevolvingCreditBuilder| b.margin_steps(steps)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.draw_repay_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn draw_repay_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: DrawRepaySpec = spec_from_py(py, value, "draw_repay_spec")?;
        let kind = match &spec {
            DrawRepaySpec::Deterministic(_) => "deterministic",
            DrawRepaySpec::Stochastic(_) => "stochastic",
        };
        builder_set!(
            slf,
            draw_repay_spec,
            kind.to_string(),
            |b: RevolvingCreditBuilder| b.draw_repay_spec(spec)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            discount_curve_id,
            format!("{value:?}"),
            |b: RevolvingCreditBuilder| b.discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.credit_curve_id")]
    #[pyo3(signature = (value))]
    #[pyo3(text_signature = "($self, value)")]
    fn credit_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            credit_curve_id,
            format!("{value:?}"),
            |b: RevolvingCreditBuilder| b
                .credit_curve_id_opt(value.map(|v| CurveId::new(v.to_string())))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.recovery_rate")]
    #[pyo3(text_signature = "($self, value)")]
    fn recovery_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            recovery_rate,
            value.to_string(),
            |b: RevolvingCreditBuilder| b.recovery_rate(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.leq")]
    #[pyo3(text_signature = "($self, value)")]
    fn leq<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, leq, value.to_string(), |b: RevolvingCreditBuilder| b
            .leq(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.stub")]
    #[pyo3(text_signature = "($self, value)")]
    fn stub<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let stub = stub_kind_from_py(Some(value), "stub")?;
        builder_set!(
            slf,
            stub,
            format!("{stub:?}"),
            |b: RevolvingCreditBuilder| b.stub(stub)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.business_day_convention")]
    #[pyo3(text_signature = "($self, value)")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention = bdc_from_py(value, "business_day_convention")?;
        builder_set!(
            slf,
            business_day_convention,
            format!("{convention:?}"),
            |b: RevolvingCreditBuilder| b.business_day_convention(convention)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.calendar_id")]
    #[pyo3(signature = (value))]
    #[pyo3(text_signature = "($self, value)")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<String>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            calendar_id,
            format!("{value:?}"),
            |b: RevolvingCreditBuilder| b.calendar_id_opt(value.clone().map(Into::into))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.payment_lag_days")]
    #[pyo3(text_signature = "($self, value)")]
    fn payment_lag_days<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: u32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            payment_lag_days,
            value.to_string(),
            |b: RevolvingCreditBuilder| b.payment_lag_days(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.settlement_days")]
    #[pyo3(text_signature = "($self, value)")]
    fn settlement_days<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: u32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            settlement_days,
            value.to_string(),
            |b: RevolvingCreditBuilder| b.settlement_days(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RevolvingCreditBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attributes = attributes_from_py(value)?;
        builder_set!(
            slf,
            attributes,
            "{..}".to_string(),
            |b: RevolvingCreditBuilder| b.attributes(attributes)
        )
    }
}

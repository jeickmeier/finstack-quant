//! Corporate TermLoan Python bindings.

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::module_utils::py_to_serde;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId};

use super::super::convert::{
    attributes_from_py, enum_from_str, enum_to_py_string, instrument_expiry, money_from_py,
    money_to_py, opt_serde_to_py, rate_from_py, stub_kind_from_py,
};
use super::super::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_trait_methods,
};

use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoanBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "TermLoan",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyTermLoan {
    /// Inner canonical Rust term loan.
    pub(crate) inner: finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan,
}

instrument_envelope_methods!(
    PyTermLoan(
        "TermLoan",
        module = "finstack_quant.valuations.instruments._instruments.term_loan"
    ),
    TermLoan,
    builder = PyTermLoanBuilder("TermLoanBuilder", fields),
    seed = finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan::builder(),
);
instrument_trait_methods!(
    PyTermLoan,
    "TermLoan",
    module = "finstack_quant.valuations.instruments._instruments.term_loan"
);

#[pymethods]
impl PyTermLoan {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.example_floating_with_ddtl")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_floating_with_ddtl() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan::example_floating_with_ddtl()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.example_callable")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_callable() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan::example_callable(
        )
        .map(|inner| Self { inner })
        .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.issue_date")]
    #[getter]
    fn issue_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.issue_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.rate")]
    #[getter]
    fn rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.rate)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.amortization")]
    #[getter]
    fn amortization<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.amortization)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.upfront_fee")]
    #[getter]
    fn upfront_fee<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.upfront_fee.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.ddtl")]
    #[getter]
    fn ddtl<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.ddtl.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.covenants")]
    #[getter]
    fn covenants<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.covenants.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.oid_eir")]
    #[getter]
    fn oid_eir<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.oid_eir.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.call_schedule")]
    #[getter]
    fn call_schedule<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.call_schedule.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.to_builder")]
    fn to_builder(&self) -> PyTermLoanBuilder {
        PyTermLoanBuilder {
            inner: Some(self.inner.to_builder()),
            fields: Vec::new(),
        }
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "TermLoan(id={:?}, notional_limit={}, issue_date={}, maturity={}, discount_curve_id={:?})",
            self.inner.id.as_str(),
            crate::bindings::valuations::convert::money_repr(self.inner.notional_limit),
            self.inner.issue_date,
            self.inner.maturity,
            self.inner.discount_curve_id.as_str(),
        )
    }
}

crate::bindings::macros::py_getters!(PyTermLoan |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.currency")]
    currency -> String = s.inner.currency.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.notional_limit")]
    notional_limit -> PyMoney = money_to_py(s.inner.notional_limit);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.frequency")]
    frequency -> PyTenor = PyTenor::from_inner(s.inner.frequency);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.quote_compounding")]
    quote_compounding -> String = s.inner.quote_compounding.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.yield_basis")]
    yield_basis -> PyResult<String> = enum_to_py_string(&s.inner.yield_basis);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.business_day_convention")]
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.stub")]
    stub -> PyStubKind = PyStubKind::from_inner(s.inner.stub);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.credit_curve_id")]
    credit_curve_id -> Option<String> = s.inner.credit_curve_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.coupon_type")]
    coupon_type -> PyResult<String> = enum_to_py_string(&s.inner.coupon_type);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoan.settlement_days")]
    settlement_days -> u32 = s.inner.settlement_days;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "TermLoanBuilder",
    skip_from_py_object
)]
pub struct PyTermLoanBuilder {
    inner: Option<TermLoanBuilder>,
    fields: Vec<(&'static str, String)>,
}

#[pymethods]
impl PyTermLoanBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: TermLoanBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.currency")]
    #[pyo3(text_signature = "($self, value)")]
    fn currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let ccy = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(
            slf,
            currency,
            format!("{:?}", ccy.to_string()),
            |b: TermLoanBuilder| b.currency(ccy)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.notional_limit")]
    #[pyo3(text_signature = "($self, value)")]
    fn notional_limit<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, "notional_limit")?;
        builder_set!(
            slf,
            notional_limit,
            crate::bindings::valuations::convert::money_repr(money),
            |b: TermLoanBuilder| b.notional_limit(money)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.issue_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn issue_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, issue_date, date.to_string(), |b: TermLoanBuilder| b
            .issue_date(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.maturity")]
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, maturity, date.to_string(), |b: TermLoanBuilder| b
            .maturity(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.rate")]
    #[pyo3(text_signature = "($self, value)")]
    fn rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let (spec, shown) = if value.extract::<&str>().is_ok() || value.cast::<PyDict>().is_ok() {
            let spec: finstack_quant_cashflows::builder::RateSpec = py_to_serde(py, value, "rate")?;
            (spec, "{...}".to_string())
        } else {
            let rate = rate_from_py(value, "rate")?;
            (
                finstack_quant_cashflows::builder::RateSpec::fixed_rate(rate),
                rate.as_decimal().to_string(),
            )
        };
        builder_set!(slf, rate, shown, |b: TermLoanBuilder| b.rate(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.frequency")]
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let tenor = crate::bindings::core::dates::tenor::extract_tenor(value)?;
        builder_set!(slf, frequency, tenor.to_string(), |b: TermLoanBuilder| b
            .frequency(tenor))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.quote_compounding")]
    #[pyo3(text_signature = "($self, value)")]
    fn quote_compounding<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let compounding: finstack_quant_core::math::Compounding =
            value.parse().map_err(crate::errors::core_to_py)?;
        builder_set!(
            slf,
            quote_compounding,
            compounding.to_string(),
            |b: TermLoanBuilder| b.quote_compounding(compounding)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.day_count")]
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count =
            crate::bindings::valuations::convert::day_count_from_py(value, "day_count")?;
        builder_set!(
            slf,
            day_count,
            day_count.to_string(),
            |b: TermLoanBuilder| b.day_count(day_count)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.yield_basis")]
    #[pyo3(text_signature = "($self, value)")]
    fn yield_basis<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let basis = super::super::convert::yield_basis_from_str(value, "yield_basis")?;
        builder_set!(
            slf,
            yield_basis,
            format!("{value:?}"),
            |b: TermLoanBuilder| b.yield_basis(basis)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.business_day_convention")]
    #[pyo3(text_signature = "($self, value)")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention = super::super::convert::bdc_from_str(value, "business_day_convention")?;
        builder_set!(
            slf,
            business_day_convention,
            format!("{value:?}"),
            |b: TermLoanBuilder| b.business_day_convention(convention)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.calendar_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            calendar_id,
            format!("{value:?}"),
            |b: TermLoanBuilder| b.calendar_id(value.into())
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.stub")]
    #[pyo3(text_signature = "($self, value)")]
    fn stub<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let stub = stub_kind_from_py(Some(value), "stub")?;
        builder_set!(
            slf,
            stub,
            format!("{:?}", enum_to_py_string(&stub).unwrap_or_default()),
            |b: TermLoanBuilder| b.stub(stub)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            discount_curve_id,
            format!("{value:?}"),
            |b: TermLoanBuilder| b.discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.credit_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn credit_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            credit_curve_id,
            format!("{value:?}"),
            |b: TermLoanBuilder| b.credit_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.amortization")]
    #[pyo3(text_signature = "($self, value)")]
    fn amortization<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        // A string is the label of a unit variant (`"none"`); dicts and typed
        // specs go through the shared serde coercer.
        let spec: finstack_quant_cashflows::builder::AmortizationSpec =
            match value.extract::<&str>() {
                Ok(name) => enum_from_str(name, "amortization")?,
                _ => py_to_serde(py, value, "amortization")?,
            };
        builder_set!(
            slf,
            amortization,
            "{...}".to_string(),
            |b: TermLoanBuilder| b.amortization(spec)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.coupon_type")]
    #[pyo3(text_signature = "($self, value)")]
    fn coupon_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let coupon_type = enum_from_str(value, "coupon_type")?;
        builder_set!(
            slf,
            coupon_type,
            format!("{value:?}"),
            |b: TermLoanBuilder| b.coupon_type(coupon_type)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.upfront_fee")]
    #[pyo3(text_signature = "($self, value)")]
    fn upfront_fee<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        use finstack_quant_valuations::instruments::fixed_income::loan_terms::UpfrontFee;
        let fee: UpfrontFee = py_to_serde(value.py(), value, "upfront_fee")?;
        builder_set!(
            slf,
            upfront_fee,
            "{...}".to_string(),
            |b: TermLoanBuilder| b.upfront_fee(fee)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.ddtl")]
    #[pyo3(text_signature = "($self, value)")]
    fn ddtl<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::term_loan::DdtlSpec =
            py_to_serde(py, value, "ddtl")?;
        builder_set!(slf, ddtl, "{...}".to_string(), |b: TermLoanBuilder| b
            .ddtl(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.covenants")]
    #[pyo3(text_signature = "($self, value)")]
    fn covenants<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoanCovenantEvents =
            py_to_serde(py, value, "covenants")?;
        builder_set!(slf, covenants, "{...}".to_string(), |b: TermLoanBuilder| b
            .covenants(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.oid_eir")]
    #[pyo3(text_signature = "($self, value)")]
    fn oid_eir<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::loan_terms::OidEirSpec =
            py_to_serde(py, value, "oid_eir")?;
        builder_set!(slf, oid_eir, "{...}".to_string(), |b: TermLoanBuilder| b
            .oid_eir(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.call_schedule")]
    #[pyo3(text_signature = "($self, value)")]
    fn call_schedule<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::term_loan::LoanCallSchedule =
            py_to_serde(py, value, "call_schedule")?;
        builder_set!(
            slf,
            call_schedule,
            "{...}".to_string(),
            |b: TermLoanBuilder| b.call_schedule(spec)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.settlement_days")]
    #[pyo3(text_signature = "($self, value)")]
    fn settlement_days<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: u32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            settlement_days,
            value.to_string(),
            |b: TermLoanBuilder| b.settlement_days(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.term_loan", "TermLoanBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        builder_set!(
            slf,
            attributes,
            "Attributes(...)".to_string(),
            |b: TermLoanBuilder| b.attributes(attrs)
        )
    }
}

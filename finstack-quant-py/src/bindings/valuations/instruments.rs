//! Typed instrument classes for the `finstack_quant.valuations.instruments`
//! namespace.
//!
//! Thin wrappers over the canonical Rust structs
//! `finstack_quant_valuations::instruments::fixed_income::bond::Bond` and
//! `finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan`. Construction and
//! validation stay in Rust; the wrappers only convert to and from the canonical
//! `finstack_quant.instrument/1` envelope accepted by the JSON loader, expose
//! one getter per public Rust field, and route `price` / `metric` through the
//! same pricer entry points as `price_instrument`.

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::module_utils::spec_from_py;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::merton_mc::{PyMertonMcConfig, PyMertonMcResult};
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId};

use super::convert::{
    attributes_from_py, bps_value_from_py, enum_from_str, enum_to_py_string, instrument_expiry,
    money_from_py, money_to_py, opt_repr, opt_serde_to_py, rate_from_py, stub_kind_from_py,
};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

use finstack_quant_valuations::instruments::fixed_income::bond::BondBuilder;
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoanBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "Bond",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBond {
    /// Inner canonical Rust bond.
    pub(crate) inner: finstack_quant_valuations::instruments::fixed_income::bond::Bond,
}

instrument_envelope_methods!(
    PyBond("Bond"),
    Bond,
    builder = PyBondBuilder("BondBuilder", fields),
    seed = finstack_quant_valuations::instruments::fixed_income::bond::Bond::builder(),
);
instrument_pricing_methods!(PyBond, "Bond");

#[pymethods]
impl PyBond {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.fixed")]
    #[staticmethod]
    #[pyo3(signature = (id, notional, coupon_rate, issue_date, maturity, stub, discount_curve_id, *, currency = None))]
    #[pyo3(
        text_signature = "(id, notional, coupon_rate, issue_date, maturity, stub, discount_curve_id, *, currency=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn fixed(
        id: &str,
        notional: &Bound<'_, PyAny>,
        coupon_rate: &Bound<'_, PyAny>,
        issue_date: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        stub: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        currency: Option<&str>,
    ) -> PyResult<Self> {
        let inner = finstack_quant_valuations::instruments::fixed_income::bond::Bond::fixed(
            id,
            money_from_py(notional, currency, "notional")?,
            rate_from_py(coupon_rate, "coupon_rate")?,
            py_to_date(issue_date)?,
            py_to_date(maturity)?,
            stub_kind_from_py(Some(stub), "stub")?,
            discount_curve_id,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.with_stub")]
    #[pyo3(text_signature = "($self, stub)")]
    fn with_stub(&self, stub: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_stub(stub_kind_from_py(Some(stub), "stub")?),
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.with_convention")]
    #[staticmethod]
    #[pyo3(signature = (id, notional, coupon_rate, issue_date, maturity, convention, discount_curve_id, *, currency = None))]
    #[pyo3(
        text_signature = "(id, notional, coupon_rate, issue_date, maturity, convention, discount_curve_id, *, currency=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn with_convention(
        id: &str,
        notional: &Bound<'_, PyAny>,
        coupon_rate: &Bound<'_, PyAny>,
        issue_date: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        convention: &str,
        discount_curve_id: &str,
        currency: Option<&str>,
    ) -> PyResult<Self> {
        let inner =
            finstack_quant_valuations::instruments::fixed_income::bond::Bond::with_convention(
                id,
                money_from_py(notional, currency, "notional")?,
                rate_from_py(coupon_rate, "coupon_rate")?,
                py_to_date(issue_date)?,
                py_to_date(maturity)?,
                enum_from_str::<finstack_quant_valuations::instruments::BondConvention>(
                    convention,
                    "convention",
                )?,
                discount_curve_id,
            )
            .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.floating")]
    #[staticmethod]
    #[pyo3(signature = (id, notional, forward_curve_id, spread_bp, issue_date, maturity, frequency, day_count, discount_curve_id, *, currency = None))]
    #[pyo3(
        text_signature = "(id, notional, forward_curve_id, spread_bp, issue_date, maturity, frequency, day_count, discount_curve_id, *, currency=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn floating(
        id: &str,
        notional: &Bound<'_, PyAny>,
        forward_curve_id: &str,
        spread_bp: &Bound<'_, PyAny>,
        issue_date: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        frequency: PyRef<'_, PyTenor>,
        day_count: PyRef<'_, PyDayCount>,
        discount_curve_id: &str,
        currency: Option<&str>,
    ) -> PyResult<Self> {
        let inner = finstack_quant_valuations::instruments::fixed_income::bond::Bond::floating(
            id,
            money_from_py(notional, currency, "notional")?,
            forward_curve_id,
            bps_value_from_py(spread_bp, "spread_bp")?,
            py_to_date(issue_date)?,
            py_to_date(maturity)?,
            frequency.inner,
            day_count.inner,
            discount_curve_id,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.floating_with_convention")]
    #[staticmethod]
    #[pyo3(signature = (id, notional, forward_curve_id, spread_bp, issue_date, maturity, frequency, day_count, convention, discount_curve_id, *, currency = None))]
    #[pyo3(
        text_signature = "(id, notional, forward_curve_id, spread_bp, issue_date, maturity, frequency, day_count, convention, discount_curve_id, *, currency=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn floating_with_convention(
        id: &str,
        notional: &Bound<'_, PyAny>,
        forward_curve_id: &str,
        spread_bp: &Bound<'_, PyAny>,
        issue_date: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        frequency: PyRef<'_, PyTenor>,
        day_count: PyRef<'_, PyDayCount>,
        convention: &str,
        discount_curve_id: &str,
        currency: Option<&str>,
    ) -> PyResult<Self> {
        let inner = finstack_quant_valuations::instruments::fixed_income::bond::Bond::floating_with_convention(
            id,
            money_from_py(notional, currency, "notional")?,
            forward_curve_id,
            bps_value_from_py(spread_bp, "spread_bp")?,
            py_to_date(issue_date)?,
            py_to_date(maturity)?,
            frequency.inner,
            day_count.inner,
            enum_from_str::<finstack_quant_valuations::instruments::BondConvention>(
                convention,
                "convention",
            )?,
            discount_curve_id,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.zero_coupon")]
    #[staticmethod]
    #[pyo3(signature = (id, notional, issue_date, maturity, discount_curve_id, *, currency = None))]
    #[pyo3(
        text_signature = "(id, notional, issue_date, maturity, discount_curve_id, *, currency=None)"
    )]
    fn zero_coupon(
        id: &str,
        notional: &Bound<'_, PyAny>,
        issue_date: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        currency: Option<&str>,
    ) -> PyResult<Self> {
        let inner = finstack_quant_valuations::instruments::fixed_income::bond::Bond::zero_coupon(
            id,
            money_from_py(notional, currency, "notional")?,
            py_to_date(issue_date)?,
            py_to_date(maturity)?,
            discount_curve_id,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.example_floating")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_floating() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::example_floating()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.example_callable")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_callable() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::example_callable()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.example_amortizing")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_amortizing() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::example_amortizing()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.min_moic")]
    #[pyo3(text_signature = "($self, multiple)")]
    fn min_moic(&self, multiple: f64) -> Self {
        Self {
            inner: self.inner.clone().min_moic(multiple),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.min_xirr")]
    #[pyo3(text_signature = "($self, rate)")]
    fn min_xirr(&self, rate: &Bound<'_, PyAny>) -> PyResult<Self> {
        let rate = rate_from_py(rate, "rate")?;
        Ok(Self {
            inner: self.inner.clone().min_xirr(rate),
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.issue_date")]
    #[getter]
    fn issue_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.issue_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.cashflow_spec")]
    #[getter]
    fn cashflow_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.cashflow_spec)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.call_put")]
    #[getter]
    fn call_put<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.call_put.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.return_floor")]
    #[getter]
    fn return_floor<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.return_floor.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.custom_cashflows")]
    #[getter]
    fn custom_cashflows<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.custom_cashflows.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.settlement_convention")]
    #[getter]
    fn settlement_convention<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.settlement_convention.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "Bond(id={:?}, notional={}, issue_date={}, maturity={}, discount_curve_id={:?}, credit_curve_id={})",
            self.inner.id.as_str(),
            crate::bindings::valuations::convert::money_repr(self.inner.notional),
            self.inner.issue_date,
            self.inner.maturity,
            self.inner.discount_curve_id.as_str(),
            opt_repr(self.inner.credit_curve_id.as_ref().map(|c| format!("{:?}", c.as_str()))),
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.price_merton_mc")]
    #[pyo3(text_signature = "($self, config, discount_rate, as_of)")]
    fn price_merton_mc(
        &self,
        py: Python<'_>,
        config: PyRef<'_, PyMertonMcConfig>,
        discount_rate: f64,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyMertonMcResult> {
        let as_of = py_to_date(as_of)?;
        let bond = self.inner.clone();
        let config = config.inner.clone();
        let result = py
            .detach(move || bond.price_merton_mc(&config, discount_rate, as_of))
            .map_err(core_to_py)?;
        Ok(PyMertonMcResult::from_inner(result))
    }
}

crate::bindings::macros::py_getters!(PyBond |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.credit_curve_id")]
    credit_curve_id -> Option<String> = s.inner.credit_curve_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.repo_curve_id")]
    repo_curve_id -> Option<String> = s.inner.repo_curve_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.accrual_method")]
    accrual_method -> PyResult<String> = enum_to_py_string(&s.inner.accrual_method);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.settlement_days")]
    settlement_days -> Option<u32> = s.inner.settlement_days();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.ex_coupon_days")]
    ex_coupon_days -> Option<u32> = s.inner.ex_coupon_days();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Bond.has_floating_coupons")]
    has_floating_coupons -> bool = s.inner.has_floating_coupons();
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "BondBuilder",
    skip_from_py_object
)]
pub struct PyBondBuilder {
    inner: Option<BondBuilder>,
    fields: Vec<(&'static str, String)>,
}

#[pymethods]
impl PyBondBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: BondBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder.notional")]
    #[pyo3(signature = (value, currency = None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "notional")?;
        builder_set!(
            slf,
            notional,
            crate::bindings::valuations::convert::money_repr(money),
            |b: BondBuilder| b.notional(money)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder.issue_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn issue_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, issue_date, date.to_string(), |b: BondBuilder| b
            .issue_date(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder.maturity")]
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, maturity, date.to_string(), |b: BondBuilder| b
            .maturity(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder.cashflow_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn cashflow_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec =
            spec_from_py(py, value, "cashflow_spec")?;
        builder_set!(slf, cashflow_spec, "{...}".to_string(), |b: BondBuilder| b
            .cashflow_spec(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder.discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            discount_curve_id,
            format!("{value:?}"),
            |b: BondBuilder| b.discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder.credit_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn credit_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            credit_curve_id,
            format!("{value:?}"),
            |b: BondBuilder| b.credit_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder.repo_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn repo_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            repo_curve_id,
            format!("{value:?}"),
            |b: BondBuilder| b.repo_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder.call_put")]
    #[pyo3(text_signature = "($self, value)")]
    fn call_put<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::bond::CallPutSchedule =
            spec_from_py(py, value, "call_put")?;
        builder_set!(slf, call_put, "{...}".to_string(), |b: BondBuilder| b
            .call_put(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder.return_floor")]
    #[pyo3(text_signature = "($self, value)")]
    fn return_floor<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::bond::ReturnFloorSpec =
            spec_from_py(py, value, "return_floor")?;
        builder_set!(slf, return_floor, "{...}".to_string(), |b: BondBuilder| b
            .return_floor(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder.custom_cashflows")]
    #[pyo3(text_signature = "($self, value)")]
    fn custom_cashflows<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let schedule: finstack_quant_cashflows::builder::CashFlowSchedule =
            spec_from_py(py, value, "custom_cashflows")?;
        builder_set!(
            slf,
            custom_cashflows,
            "{...}".to_string(),
            |b: BondBuilder| b.custom_cashflows(schedule)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder.accrual_method")]
    #[pyo3(text_signature = "($self, value)")]
    fn accrual_method<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let method: finstack_quant_cashflows::accrual::AccrualMethod =
            enum_from_str(value, "accrual_method")?;
        builder_set!(
            slf,
            accrual_method,
            format!("{value:?}"),
            |b: BondBuilder| b.accrual_method(method)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder.attributes")]
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
            |b: BondBuilder| b.attributes(attrs)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BondBuilder.settlement_convention")]
    #[pyo3(text_signature = "($self, value)")]
    fn settlement_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention: finstack_quant_valuations::instruments::fixed_income::bond::BondSettlementConvention =
            spec_from_py(py, value, "settlement_convention")?;
        builder_set!(
            slf,
            settlement_convention,
            "{...}".to_string(),
            |b: BondBuilder| b.settlement_convention(convention)
        )
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan")]
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
    PyTermLoan("TermLoan"),
    TermLoan,
    builder = PyTermLoanBuilder("TermLoanBuilder", fields),
    seed = finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan::builder(),
);
instrument_pricing_methods!(PyTermLoan, "TermLoan");

#[pymethods]
impl PyTermLoan {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.example_floating_with_ddtl")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_floating_with_ddtl() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan::example_floating_with_ddtl()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.example_callable")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_callable() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan::example_callable(
        )
        .map(|inner| Self { inner })
        .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.issue_date")]
    #[getter]
    fn issue_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.issue_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.rate")]
    #[getter]
    fn rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.rate)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.amortization")]
    #[getter]
    fn amortization<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.amortization)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.upfront_fee")]
    #[getter]
    fn upfront_fee<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.upfront_fee.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.ddtl")]
    #[getter]
    fn ddtl<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.ddtl.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.covenants")]
    #[getter]
    fn covenants<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.covenants.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.oid_eir")]
    #[getter]
    fn oid_eir<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.oid_eir.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.call_schedule")]
    #[getter]
    fn call_schedule<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.call_schedule.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.currency")]
    currency -> String = s.inner.currency.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.notional_limit")]
    notional_limit -> PyMoney = money_to_py(s.inner.notional_limit);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.frequency")]
    frequency -> PyTenor = PyTenor::from_inner(s.inner.frequency);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.business_day_convention")]
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.stub")]
    stub -> PyStubKind = PyStubKind::from_inner(s.inner.stub);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.credit_curve_id")]
    credit_curve_id -> Option<String> = s.inner.credit_curve_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.coupon_type")]
    coupon_type -> PyResult<String> = enum_to_py_string(&s.inner.coupon_type);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoan.settlement_days")]
    settlement_days -> u32 = s.inner.settlement_days;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: TermLoanBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.currency")]
    #[pyo3(text_signature = "($self, value)")]
    fn currency<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let ccy = crate::bindings::module_utils::parse_currency(value)?;
        builder_set!(slf, currency, format!("{value:?}"), |b: TermLoanBuilder| b
            .currency(ccy))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.notional_limit")]
    #[pyo3(signature = (value, currency = None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn notional_limit<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "notional_limit")?;
        builder_set!(
            slf,
            notional_limit,
            crate::bindings::valuations::convert::money_repr(money),
            |b: TermLoanBuilder| b.notional_limit(money)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.issue_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn issue_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, issue_date, date.to_string(), |b: TermLoanBuilder| b
            .issue_date(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.maturity")]
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, maturity, date.to_string(), |b: TermLoanBuilder| b
            .maturity(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.rate")]
    #[pyo3(text_signature = "($self, value)")]
    fn rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let (spec, shown) = if value.extract::<&str>().is_ok() || value.cast::<PyDict>().is_ok() {
            let spec: finstack_quant_cashflows::builder::RateSpec =
                spec_from_py(py, value, "rate")?;
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.frequency")]
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let tenor = crate::bindings::core::dates::tenor::extract_tenor(value)?;
        builder_set!(slf, frequency, tenor.to_string(), |b: TermLoanBuilder| b
            .frequency(tenor))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.day_count")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.business_day_convention")]
    #[pyo3(text_signature = "($self, value)")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention = super::convert::bdc_from_str(value, "business_day_convention")?;
        builder_set!(
            slf,
            business_day_convention,
            format!("{value:?}"),
            |b: TermLoanBuilder| b.business_day_convention(convention)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.calendar_id")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.stub")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.discount_curve_id")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.credit_curve_id")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.amortization")]
    #[pyo3(text_signature = "($self, value)")]
    fn amortization<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        // A bare name (`"bullet"`) is a unit variant; JSON text, dicts and
        // typed specs go through the shared spec coercer.
        let spec: finstack_quant_cashflows::builder::AmortizationSpec =
            match value.extract::<&str>() {
                Ok(name) if !name.trim_start().starts_with(['{', '"']) => {
                    enum_from_str(name, "amortization")?
                }
                _ => spec_from_py(py, value, "amortization")?,
            };
        builder_set!(
            slf,
            amortization,
            "{...}".to_string(),
            |b: TermLoanBuilder| b.amortization(spec)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.coupon_type")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.upfront_fee")]
    #[pyo3(signature = (value, currency = None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn upfront_fee<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        use finstack_quant_valuations::instruments::fixed_income::loan_terms::UpfrontFee;
        let (fee, shown) = if value.extract::<&str>().is_ok() || value.cast::<PyDict>().is_ok() {
            let fee: UpfrontFee = spec_from_py(py, value, "upfront_fee")?;
            (fee, "{...}".to_string())
        } else {
            let money = money_from_py(value, currency, "upfront_fee")?;
            (
                UpfrontFee::Amount(money),
                crate::bindings::valuations::convert::money_repr(money),
            )
        };
        builder_set!(slf, upfront_fee, shown, |b: TermLoanBuilder| b
            .upfront_fee(fee))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.ddtl")]
    #[pyo3(text_signature = "($self, value)")]
    fn ddtl<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::term_loan::DdtlSpec =
            spec_from_py(py, value, "ddtl")?;
        builder_set!(slf, ddtl, "{...}".to_string(), |b: TermLoanBuilder| b
            .ddtl(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.covenants")]
    #[pyo3(text_signature = "($self, value)")]
    fn covenants<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoanCovenantEvents =
            spec_from_py(py, value, "covenants")?;
        builder_set!(slf, covenants, "{...}".to_string(), |b: TermLoanBuilder| b
            .covenants(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.oid_eir")]
    #[pyo3(text_signature = "($self, value)")]
    fn oid_eir<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::loan_terms::OidEirSpec =
            spec_from_py(py, value, "oid_eir")?;
        builder_set!(slf, oid_eir, "{...}".to_string(), |b: TermLoanBuilder| b
            .oid_eir(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.call_schedule")]
    #[pyo3(text_signature = "($self, value)")]
    fn call_schedule<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::term_loan::LoanCallSchedule =
            spec_from_py(py, value, "call_schedule")?;
        builder_set!(
            slf,
            call_schedule,
            "{...}".to_string(),
            |b: TermLoanBuilder| b.call_schedule(spec)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.settlement_days")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TermLoanBuilder.attributes")]
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

/// Register the typed instrument classes on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBond>()?;
    m.add_class::<PyBondBuilder>()?;
    m.add_class::<PyTermLoan>()?;
    m.add_class::<PyTermLoanBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
///
/// Extend this list (sorted) when adding a class or function here; `mod.rs`
/// merges every submodule list so registration stays in one place per file.
pub(crate) const EXPORTS: &[&str] = &["Bond", "BondBuilder", "TermLoan", "TermLoanBuilder"];

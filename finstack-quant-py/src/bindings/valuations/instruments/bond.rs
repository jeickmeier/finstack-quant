//! Fixed, floating, and amortizing Bond Python bindings.

use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::module_utils::spec_from_py;
use crate::bindings::valuations::merton_mc::{PyMertonMcConfig, PyMertonMcResult};
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId};

use super::super::convert::{
    attributes_from_py, enum_from_str, enum_to_py_string, instrument_expiry, money_from_py,
    money_to_py, opt_repr, opt_serde_to_py, rate_from_py, stub_kind_from_py,
};
use super::super::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_pricing_methods,
};

use finstack_quant_valuations::instruments::fixed_income::bond::BondBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond")]
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
    PyBond(
        "Bond",
        module = "finstack_quant.valuations.instruments._instruments.bond"
    ),
    Bond,
    builder = PyBondBuilder("BondBuilder", fields),
    seed = finstack_quant_valuations::instruments::fixed_income::bond::Bond::builder(),
);
instrument_pricing_methods!(
    PyBond,
    "Bond",
    module = "finstack_quant.valuations.instruments._instruments.bond"
);

fn floating_bond_params(
    params: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_valuations::instruments::fixed_income::bond::FloatingBondParams> {
    use crate::bindings::cashflows::builder::specs::{
        decimal_bps_from_any, fields_with_decimal_from_py,
    };

    match params.cast::<pyo3::types::PyDict>() {
        Ok(fields) => fields_with_decimal_from_py(
            params.py(),
            Some(fields),
            "FloatingBondParams",
            "spread_bp",
            decimal_bps_from_any,
        ),
        Err(_) => spec_from_py(params.py(), params, "FloatingBondParams"),
    }
}

#[pymethods]
impl PyBond {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.from_cashflows")]
    #[staticmethod]
    #[pyo3(signature = (id, schedule, quote_compounding, discount_curve_id, quoted_clean_price_pct=None))]
    fn from_cashflows(
        id: &str,
        schedule: &crate::bindings::cashflows::builder::schedule::PyCashFlowSchedule,
        quote_compounding: &str,
        discount_curve_id: &str,
        quoted_clean_price_pct: Option<f64>,
    ) -> PyResult<Self> {
        let quote_compounding = quote_compounding.parse().map_err(core_to_py)?;
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::from_cashflows(
            id,
            schedule.inner.clone(),
            quote_compounding,
            discount_curve_id,
            quoted_clean_price_pct,
        )
        .map(|inner| Self { inner })
        .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.fixed")]
    #[staticmethod]
    #[pyo3(
        text_signature = "(id, notional, coupon_rate, issue_date, maturity, stub, discount_curve_id)"
    )]
    fn fixed(
        id: &str,
        notional: PyRef<'_, PyMoney>,
        coupon_rate: &Bound<'_, PyAny>,
        issue_date: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        stub: &Bound<'_, PyAny>,
        discount_curve_id: &str,
    ) -> PyResult<Self> {
        let inner = finstack_quant_valuations::instruments::fixed_income::bond::Bond::fixed(
            id,
            notional.inner,
            rate_from_py(coupon_rate, "coupon_rate")?,
            py_to_date(issue_date)?,
            py_to_date(maturity)?,
            stub_kind_from_py(Some(stub), "stub")?,
            discount_curve_id,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.with_stub")]
    #[pyo3(text_signature = "($self, stub)")]
    fn with_stub(&self, stub: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_stub(stub_kind_from_py(Some(stub), "stub")?),
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.with_convention")]
    #[staticmethod]
    #[pyo3(
        text_signature = "(id, notional, coupon_rate, issue_date, maturity, convention, discount_curve_id)"
    )]
    fn with_convention(
        id: &str,
        notional: PyRef<'_, PyMoney>,
        coupon_rate: &Bound<'_, PyAny>,
        issue_date: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        convention: &str,
        discount_curve_id: &str,
    ) -> PyResult<Self> {
        let inner =
            finstack_quant_valuations::instruments::fixed_income::bond::Bond::with_convention(
                id,
                notional.inner,
                rate_from_py(coupon_rate, "coupon_rate")?,
                py_to_date(issue_date)?,
                py_to_date(maturity)?,
                enum_from_str::<
                    finstack_quant_valuations::instruments::fixed_income::BondConvention,
                >(convention, "convention")?,
                discount_curve_id,
            )
            .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.floating")]
    #[staticmethod]
    #[pyo3(text_signature = "(params)")]
    fn floating(params: &Bound<'_, PyAny>) -> PyResult<Self> {
        let params = floating_bond_params(params)?;
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::floating(params)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.floating_with_convention")]
    #[staticmethod]
    #[pyo3(text_signature = "(params, convention)")]
    fn floating_with_convention(params: &Bound<'_, PyAny>, convention: &str) -> PyResult<Self> {
        let params = floating_bond_params(params)?;
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::floating_with_convention(
            params,
            enum_from_str::<finstack_quant_valuations::instruments::fixed_income::BondConvention>(
                convention,
                "convention",
            )?,
        )
        .map(|inner| Self { inner })
        .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.zero_coupon")]
    #[staticmethod]
    #[pyo3(
        text_signature = "(id, notional, issue_date, maturity, quote_compounding, discount_curve_id)"
    )]
    fn zero_coupon(
        id: &str,
        notional: &Bound<'_, PyAny>,
        issue_date: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        quote_compounding: &str,
        discount_curve_id: &str,
    ) -> PyResult<Self> {
        let inner = finstack_quant_valuations::instruments::fixed_income::bond::Bond::zero_coupon(
            id,
            money_from_py(notional, "notional")?,
            py_to_date(issue_date)?,
            py_to_date(maturity)?,
            quote_compounding
                .parse::<finstack_quant_core::math::Compounding>()
                .map_err(core_to_py)?,
            discount_curve_id,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.example_floating")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_floating() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::example_floating()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.example_callable")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_callable() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::example_callable()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.example_amortizing")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_amortizing() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::example_amortizing()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.min_moic")]
    #[pyo3(text_signature = "($self, multiple)")]
    fn min_moic(&self, multiple: f64) -> Self {
        Self {
            inner: self.inner.clone().min_moic(multiple),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.min_xirr")]
    #[pyo3(text_signature = "($self, rate)")]
    fn min_xirr(&self, rate: &Bound<'_, PyAny>) -> PyResult<Self> {
        let rate = rate_from_py(rate, "rate")?;
        Ok(Self {
            inner: self.inner.clone().min_xirr(rate),
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.issue_date")]
    #[getter]
    fn issue_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.issue_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.cashflow_spec")]
    #[getter]
    fn cashflow_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.cashflow_spec)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.call_put")]
    #[getter]
    fn call_put<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.call_put.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.return_floor")]
    #[getter]
    fn return_floor<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.return_floor.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.custom_cashflows")]
    #[getter]
    fn custom_cashflows<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.custom_cashflows.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.settlement_convention")]
    #[getter]
    fn settlement_convention<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.settlement_convention.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.to_builder")]
    fn to_builder(&self) -> PyBondBuilder {
        PyBondBuilder {
            inner: Some(self.inner.to_builder()),
            fields: Vec::new(),
        }
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
            opt_repr(
                self.inner
                    .credit_curve_id
                    .as_ref()
                    .map(|c| format!("{:?}", c.as_str()))
            ),
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.price_merton_mc")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.quote_compounding")]
    quote_compounding -> String = s.inner.quote_compounding.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.credit_curve_id")]
    credit_curve_id -> Option<String> = s.inner.credit_curve_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.repo_curve_id")]
    repo_curve_id -> Option<String> = s.inner.repo_curve_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.accrual_method")]
    accrual_method -> PyResult<String> = enum_to_py_string(&s.inner.accrual_method);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.settlement_days")]
    settlement_days -> Option<u32> = s.inner.settlement_days();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.ex_coupon_days")]
    ex_coupon_days -> Option<u32> = s.inner.ex_coupon_days();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "Bond.has_floating_coupons")]
    has_floating_coupons -> bool = s.inner.has_floating_coupons();
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: BondBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.notional")]
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, "notional")?;
        builder_set!(
            slf,
            notional,
            crate::bindings::valuations::convert::money_repr(money),
            |b: BondBuilder| b.notional(money)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.issue_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn issue_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, issue_date, date.to_string(), |b: BondBuilder| b
            .issue_date(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.maturity")]
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, maturity, date.to_string(), |b: BondBuilder| b
            .maturity(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.cashflow_spec")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.quote_compounding")]
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
            |b: BondBuilder| b.quote_compounding(compounding)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.discount_curve_id")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.credit_curve_id")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.repo_curve_id")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.call_put")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.return_floor")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.custom_cashflows")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.accrual_method")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.attributes")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._instruments.bond", "BondBuilder.settlement_convention")]
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

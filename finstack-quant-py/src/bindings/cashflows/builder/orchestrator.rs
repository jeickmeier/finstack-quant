//! Python bindings for `CashFlowBuilder`.

use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::builder::CashFlowBuilder;
use pyo3::prelude::*;

use crate::bindings::core::cashflow::extract_cf_kind;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::py_to_date;
use crate::errors::core_to_py;

use super::schedule::PyCashFlowSchedule;
use super::specs::{
    date_decimal_pairs, PyAmortizationSpec, PyCouponType, PyFeeSpec, PyFixedCouponSpec,
    PyFloatingCouponSpec, PyPrincipalExchange, PyStepUpCouponSpec,
};

#[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder")]
#[pyclass(
    name = "CashFlowBuilder",
    module = "finstack_quant.cashflows.builder",
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyCashFlowBuilder {
    /// Inner fluent builder.
    pub(crate) inner: CashFlowBuilder,
}

#[pymethods]
impl PyCashFlowBuilder {
    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.principal")]
    #[pyo3(text_signature = "(self, initial, issue_date, maturity)")]
    fn principal<'py>(
        mut slf: PyRefMut<'py, Self>,
        initial: PyMoney,
        issue_date: &Bound<'py, PyAny>,
        maturity: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let issue = py_to_date(issue_date)?;
        let maturity = py_to_date(maturity)?;
        let _ = slf.inner.principal(initial.inner, issue, maturity);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.principal_exchange")]
    #[pyo3(text_signature = "(self, exchange)")]
    fn principal_exchange<'py>(
        mut slf: PyRefMut<'py, Self>,
        exchange: PyRef<'py, PyPrincipalExchange>,
    ) -> PyRefMut<'py, Self> {
        let _ = slf.inner.principal_exchange(exchange.inner);
        slf
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.amortization")]
    #[pyo3(text_signature = "(self, spec)")]
    fn amortization<'py>(
        mut slf: PyRefMut<'py, Self>,
        spec: PyRef<'py, PyAmortizationSpec>,
    ) -> PyRefMut<'py, Self> {
        let inner_spec = spec.inner.clone();
        let _ = slf.inner.amortization(inner_spec);
        slf
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.add_principal_event")]
    #[pyo3(
        signature = (date, payment_date, delta, kind, cash=None),
        text_signature = "(self, date, payment_date, delta, kind, cash=None)"
    )]
    fn add_principal_event<'py>(
        mut slf: PyRefMut<'py, Self>,
        date: &Bound<'py, PyAny>,
        payment_date: &Bound<'py, PyAny>,
        delta: PyMoney,
        kind: &Bound<'py, PyAny>,
        cash: Option<PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(date)?;
        let payment_date = py_to_date(payment_date)?;
        let kind = extract_cf_kind(kind)?;
        let _ = slf.inner.add_principal_event(
            date,
            payment_date,
            delta.inner,
            cash.map(|c| c.inner),
            kind,
        );
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.fixed_cf")]
    #[pyo3(text_signature = "(self, spec)")]
    fn fixed_cf<'py>(
        mut slf: PyRefMut<'py, Self>,
        spec: PyRef<'py, PyFixedCouponSpec>,
    ) -> PyRefMut<'py, Self> {
        let inner_spec = spec.inner.clone();
        let _ = slf.inner.fixed_cf(inner_spec);
        slf
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.floating_cf")]
    #[pyo3(text_signature = "(self, spec)")]
    fn floating_cf<'py>(
        mut slf: PyRefMut<'py, Self>,
        spec: PyRef<'py, PyFloatingCouponSpec>,
    ) -> PyRefMut<'py, Self> {
        let inner_spec = spec.inner.clone();
        let _ = slf.inner.floating_cf(inner_spec);
        slf
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.step_up_cf")]
    #[pyo3(text_signature = "(self, spec)")]
    fn step_up_cf<'py>(
        mut slf: PyRefMut<'py, Self>,
        spec: PyRef<'py, PyStepUpCouponSpec>,
    ) -> PyRefMut<'py, Self> {
        let inner_spec = spec.inner.clone();
        let _ = slf.inner.step_up_cf(inner_spec);
        slf
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.fee")]
    #[pyo3(text_signature = "(self, spec)")]
    fn fee<'py>(mut slf: PyRefMut<'py, Self>, spec: PyRef<'py, PyFeeSpec>) -> PyRefMut<'py, Self> {
        let inner_spec = spec.inner.clone();
        let _ = slf.inner.fee(inner_spec);
        slf
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.add_fixed_window")]
    #[pyo3(text_signature = "(self, start, end, spec)")]
    fn add_fixed_window<'py>(
        mut slf: PyRefMut<'py, Self>,
        start: &Bound<'py, PyAny>,
        end: &Bound<'py, PyAny>,
        spec: PyRef<'py, PyFixedCouponSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let (start, end) = (py_to_date(start)?, py_to_date(end)?);
        let inner_spec = spec.inner.clone();
        let _ = slf.inner.add_fixed_window(start, end, inner_spec);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.add_floating_window")]
    #[pyo3(text_signature = "(self, start, end, spec)")]
    fn add_floating_window<'py>(
        mut slf: PyRefMut<'py, Self>,
        start: &Bound<'py, PyAny>,
        end: &Bound<'py, PyAny>,
        spec: PyRef<'py, PyFloatingCouponSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let (start, end) = (py_to_date(start)?, py_to_date(end)?);
        let inner_spec = spec.inner.clone();
        let _ = slf.inner.add_floating_window(start, end, inner_spec);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.add_payment_window")]
    #[pyo3(text_signature = "(self, start, end, split)")]
    fn add_payment_window<'py>(
        mut slf: PyRefMut<'py, Self>,
        start: &Bound<'py, PyAny>,
        end: &Bound<'py, PyAny>,
        split: PyRef<'py, PyCouponType>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let (start, end) = (py_to_date(start)?, py_to_date(end)?);
        let split = split.inner;
        let _ = slf.inner.add_payment_window(start, end, split);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.payment_split_program")]
    #[pyo3(text_signature = "(self, steps)")]
    fn payment_split_program<'py>(
        mut slf: PyRefMut<'py, Self>,
        steps: Vec<(Bound<'py, PyAny>, PyRef<'py, PyCouponType>)>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let steps: Vec<(finstack_quant_core::dates::Date, _)> = steps
            .iter()
            .map(|(d, ct)| Ok((py_to_date(d)?, ct.inner)))
            .collect::<PyResult<_>>()?;
        let _ = slf.inner.payment_split_program(&steps);
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.fixed_to_float")]
    #[pyo3(text_signature = "(self, switch, fixed, floating)")]
    fn fixed_to_float<'py>(
        mut slf: PyRefMut<'py, Self>,
        switch: &Bound<'py, PyAny>,
        fixed: PyRef<'py, PyFixedCouponSpec>,
        floating: PyRef<'py, PyFloatingCouponSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let switch = py_to_date(switch)?;
        let _ = slf
            .inner
            .fixed_to_float(switch, fixed.inner.clone(), floating.inner.clone());
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.float_margin_steps")]
    #[pyo3(text_signature = "(self, steps, base_spec)")]
    fn float_margin_steps<'py>(
        mut slf: PyRefMut<'py, Self>,
        steps: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)>,
        base_spec: PyRef<'py, PyFloatingCouponSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let steps = date_decimal_pairs(steps)?;
        let _ = slf
            .inner
            .float_margin_steps(&steps, base_spec.inner.clone());
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.build")]
    #[pyo3(signature = (market=None), text_signature = "(self, market=None)")]
    fn build(
        &self,
        py: Python<'_>,
        market: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyCashFlowSchedule> {
        let market = crate::bindings::extract::extract_market_opt(py, market)?;
        let builder = self.inner.clone();
        py.detach(move || builder.build(market.as_ref()))
            .map(PyCashFlowSchedule::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._orchestrator", "CashFlowBuilder.build_as_of")]
    fn build_as_of(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyCashFlowSchedule> {
        let market = crate::bindings::extract::extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let builder = self.inner.clone();
        py.detach(move || builder.build_as_of(&market, as_of))
            .map(PyCashFlowSchedule::from_inner)
            .map_err(core_to_py)
    }

    /// Python-style summary of what has been configured so far.
    fn __repr__(&self) -> String {
        self.inner.to_string()
    }
}

//! Python bindings for `CashFlowBuilder` and `PrincipalEvent`.

use finstack_quant_cashflows::builder::{CashFlowBuilder, PrincipalEvent};
use pyo3::prelude::*;

use crate::bindings::cashflows::primitives::{extract_cf_kind, PyCFKind};
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

use super::schedule::PyCashFlowSchedule;
use super::specs::{
    date_decimal_pairs, PyAmortizationSpec, PyCouponType, PyFeeSpec, PyFixedCouponSpec,
    PyFloatingCouponSpec, PyPrincipalExchange, PyStepUpCouponSpec,
};

/// Wrapper for [`PrincipalEvent`]
/// (`finstack_quant.cashflows.builder.PrincipalEvent`).
#[pyclass(
    name = "PrincipalEvent",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyPrincipalEvent {
    /// Inner principal event.
    pub(crate) inner: PrincipalEvent,
}

#[pymethods]
impl PyPrincipalEvent {
    /// Principal event applied during schedule build (draws/repays).
    ///
    /// Parameters
    /// ----------
    /// date : datetime.date
    ///     Economic date when outstanding principal changes.
    /// payment_date : datetime.date
    ///     Cash settlement date, which may differ after business-day adjustment.
    /// delta : Money
    ///     Outstanding delta (positive increases balance, negative repays).
    /// cash : Money
    ///     Cash leg paid/received (may differ from delta for OID/fees).
    /// kind : CFKind or str
    ///     Classification for the emitted cashflow.
    #[new]
    #[pyo3(text_signature = "(date, payment_date, delta, cash, kind)")]
    fn new(
        date: &Bound<'_, PyAny>,
        payment_date: &Bound<'_, PyAny>,
        delta: PyMoney,
        cash: PyMoney,
        kind: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: PrincipalEvent {
                date: py_to_date(date)?,
                payment_date: py_to_date(payment_date)?,
                delta: delta.inner,
                cash: cash.inner,
                kind: extract_cf_kind(kind)?,
            },
        })
    }

    /// Event date.
    #[getter]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.date)
    }

    /// Cash settlement date, independent of the economic event date.
    #[getter]
    fn payment_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.payment_date)
    }

    /// Outstanding delta.
    #[getter]
    fn delta(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.delta)
    }

    /// Cash leg.
    #[getter]
    fn cash(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.cash)
    }

    /// Emitted cashflow classification.
    #[getter]
    fn kind(&self) -> PyCFKind {
        PyCFKind::from_inner(self.inner.kind)
    }

    /// Debug-style representation.
    fn __repr__(&self) -> String {
        format!(
            "PrincipalEvent(date='{}', payment_date='{}', delta={} {}, cash={} {}, kind='{}')",
            self.inner.date,
            self.inner.payment_date,
            self.inner.delta.amount(),
            self.inner.delta.currency(),
            self.inner.cash.amount(),
            self.inner.cash.currency(),
            self.inner.kind
        )
    }
}

/// Wrapper for [`CashFlowBuilder`]
/// (`finstack_quant.cashflows.builder.CashFlowBuilder`).
///
/// Created via ``CashFlowSchedule.builder()`` only; fluent methods mutate in
/// place and return ``self`` for chaining, matching the Rust `&mut self` API.
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

impl PyCashFlowBuilder {
    /// Fresh default builder (used by `CashFlowSchedule.builder()`).
    pub(crate) fn new_default() -> Self {
        Self {
            inner: CashFlowBuilder::default(),
        }
    }
}

#[pymethods]
impl PyCashFlowBuilder {
    /// Set principal details and instrument horizon.
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

    /// Select whether issue funding and maturity redemption notionals are emitted.
    ///
    /// Outstanding still starts at the :meth:`principal` initial amount for
    /// coupon math. Scheduled amortization and explicit principal events still
    /// emit. The default is :attr:`PrincipalExchange.INITIAL_AND_FINAL`.
    ///
    /// Parameters
    /// ----------
    /// exchange : PrincipalExchange
    ///     ``INITIAL_AND_FINAL`` emits the issue draw and the redemption
    ///     balloon on the lagged payment date. ``NONE`` tracks outstanding
    ///     only (vanilla IRS / basis-swap convention).
    #[pyo3(text_signature = "(self, exchange)")]
    fn principal_exchange<'py>(
        mut slf: PyRefMut<'py, Self>,
        exchange: PyRef<'py, PyPrincipalExchange>,
    ) -> PyRefMut<'py, Self> {
        let _ = slf.inner.principal_exchange(exchange.inner);
        slf
    }

    /// Configure amortization for the instrument notional.
    #[pyo3(text_signature = "(self, spec)")]
    fn amortization<'py>(
        mut slf: PyRefMut<'py, Self>,
        spec: PyRef<'py, PyAmortizationSpec>,
    ) -> PyRefMut<'py, Self> {
        let inner_spec = spec.inner.clone();
        let _ = slf.inner.amortization(inner_spec);
        slf
    }

    /// Add a single principal event (draw/repay).
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

    /// Add a full-horizon fixed coupon leg.
    #[pyo3(text_signature = "(self, spec)")]
    fn fixed_cf<'py>(
        mut slf: PyRefMut<'py, Self>,
        spec: PyRef<'py, PyFixedCouponSpec>,
    ) -> PyRefMut<'py, Self> {
        let inner_spec = spec.inner.clone();
        let _ = slf.inner.fixed_cf(inner_spec);
        slf
    }

    /// Add a full-horizon floating coupon leg.
    #[pyo3(text_signature = "(self, spec)")]
    fn floating_cf<'py>(
        mut slf: PyRefMut<'py, Self>,
        spec: PyRef<'py, PyFloatingCouponSpec>,
    ) -> PyRefMut<'py, Self> {
        let inner_spec = spec.inner.clone();
        let _ = slf.inner.floating_cf(inner_spec);
        slf
    }

    /// Add a full-horizon step-up coupon leg.
    #[pyo3(text_signature = "(self, spec)")]
    fn step_up_cf<'py>(
        mut slf: PyRefMut<'py, Self>,
        spec: PyRef<'py, PyStepUpCouponSpec>,
    ) -> PyRefMut<'py, Self> {
        let inner_spec = spec.inner.clone();
        let _ = slf.inner.step_up_cf(inner_spec);
        slf
    }

    /// Add a fee specification (fixed or periodic bp).
    #[pyo3(text_signature = "(self, spec)")]
    fn fee<'py>(mut slf: PyRefMut<'py, Self>, spec: PyRef<'py, PyFeeSpec>) -> PyRefMut<'py, Self> {
        let inner_spec = spec.inner.clone();
        let _ = slf.inner.fee(inner_spec);
        slf
    }

    /// Add a fixed coupon over the half-open window ``[start, end)``.
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

    /// Add a floating coupon over the half-open window ``[start, end)``.
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

    /// Set the payment split (Cash/PIK/Split) over ``[start, end)``.
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

    /// Payment split program from boundary dates (PIK toggle windows).
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

    /// Switch from a fixed coupon to a floating coupon at ``switch``.
    ///
    /// Parameters
    /// ----------
    /// switch : datetime.date
    ///     Date on which the floating leg begins (exclusive end of the
    ///     fixed window).
    /// fixed : FixedCouponSpec
    ///     Fixed rate, settlement type, and schedule for the pre-switch window.
    /// floating : FloatingCouponSpec
    ///     Floating coupon spec for the post-switch window.
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

    /// Consecutive floating windows whose margin changes at ``steps``.
    ///
    /// Parameters
    /// ----------
    /// steps : list[tuple[datetime.date, decimal.Decimal]]
    ///     Ordered ``(window_end, spread_bp)`` pairs. Each date is the
    ///     exclusive end of a window whose margin is that spread.
    /// base_spec : FloatingCouponSpec
    ///     Base floating spec; each window replaces ``spread_bp``.
    #[pyo3(text_signature = "(self, steps, base_spec)")]
    fn float_margin_stepup<'py>(
        mut slf: PyRefMut<'py, Self>,
        steps: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)>,
        base_spec: PyRef<'py, PyFloatingCouponSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let steps = date_decimal_pairs(steps)?;
        let _ = slf
            .inner
            .float_margin_stepup(&steps, base_spec.inner.clone());
        Ok(slf)
    }

    /// Build the cashflow schedule.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext, optional
    ///     Market context for floating-rate projection. Fixed coupons and
    ///     deterministic fees do not require one.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If required inputs (principal) or market data are missing.
    /// ValueError
    ///     If a spec or date validation fails (including deferred fluent errors).
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

    /// Python-style summary of what has been configured so far.
    fn __repr__(&self) -> String {
        let principal = self.inner.principal_notional().map_or_else(
            || "None".to_string(),
            |n| format!("{} {}", n.initial.amount(), n.initial.currency()),
        );
        let horizon = match (self.inner.issue_date(), self.inner.maturity_date()) {
            (Some(issue), Some(maturity)) => format!("'{issue}'..'{maturity}'"),
            _ => "None".to_string(),
        };
        format!(
            "CashFlowBuilder(principal={principal}, horizon={horizon}, coupon_legs={}, fees={}, principal_events={})",
            self.inner.coupon_leg_count(),
            self.inner.fee_count(),
            self.inner.principal_event_count(),
        )
    }
}

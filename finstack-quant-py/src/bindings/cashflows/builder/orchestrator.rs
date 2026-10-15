//! Python bindings for `CashFlowBuilder`.

use finstack_quant_cashflows::builder::CashFlowBuilder;
use pyo3::prelude::*;

use crate::bindings::cashflows::primitives::extract_cf_kind;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::py_to_date;
use crate::errors::core_to_py;

use super::schedule::PyCashFlowSchedule;
use super::specs::{
    date_decimal_pairs, PyAmortizationSpec, PyCouponType, PyFeeSpec, PyFixedCouponSpec,
    PyFloatingCouponSpec, PyPrincipalExchange, PyStepUpCouponSpec,
};

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

    /// Floating coupons whose margin changes on effective-from ``steps``.
    ///
    /// Parameters
    /// ----------
    /// steps : list[tuple[datetime.date, decimal.Decimal]]
    ///     ``(effective_from, spread_bp)`` pairs in strictly increasing date
    ///     order. Each spread (bp over the index, ``250`` = 2.50%) replaces
    ///     the base spread from its date until the next step; the last runs
    ///     to maturity.
    /// base_spec : FloatingCouponSpec
    ///     Base floating spec; its ``spread_bp`` applies from issue until the
    ///     first step.
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

    /// Build the cashflow schedule.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext, optional
    ///     Market context for floating-rate projection. Fixed coupons and
    ///     deterministic fees do not require one. Complete supplied fixing
    ///     observations can build floating coupons without a forward curve.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If required inputs (principal) or market data are missing.
    /// ValueError
    ///     If a spec or date validation fails (including deferred fluent errors).
    ///     This includes negative payment/reset lags, IMM roll rules combined
    ///     with end-of-month rolling, third-Wednesday IMM with ACT/ACT ICMA,
    ///     and an ACT/ACT ICMA end-of-month grid whose regular anchor is not
    ///     month-end. For front stubs that anchor is maturity; otherwise it
    ///     is the window start. Supplied historical fixing gaps before the
    ///     resolved curve's base date fail under every fallback policy. Explicit
    ///     fallback covers missing curves or absent fixing series, never curve,
    ///     date, day-count or arithmetic errors.
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

    /// Build a schedule with an explicit historical-observation cutoff.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext
    ///     Projection curves and exact historical index fixings.
    /// as_of : datetime.date
    ///     Resets before this date require fixings independently of curve base dates;
    ///     an available same-day fixing takes precedence over projection.
    ///
    /// Returns
    /// -------
    /// CashFlowSchedule
    ///     Canonical cashflows with realized and projected coupon amounts.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If required market data or principal inputs are missing.
    /// ValueError
    ///     If dates, schedule specifications, or required historical fixings are invalid.
    /// RuntimeError
    ///     If a numerical computation fails during cashflow construction.
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
        let principal = self.inner.get_initial_principal().map_or_else(
            || "None".to_string(),
            |n| format!("{} {}", n.amount(), n.currency()),
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

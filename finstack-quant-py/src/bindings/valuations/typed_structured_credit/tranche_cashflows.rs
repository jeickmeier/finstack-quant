//! Typed projected cashflows of one tranche (`TrancheCashflows`).

use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::convert::money_to_py;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheCashflows;

/// Projected cashflows of one tranche from a deterministic simulation
/// (:meth:`StructuredCredit.tranche_cashflows`'s return value): the total
/// flows and their interest, principal, PIK, deferred-interest and
/// write-down components, plus the accrual periods behind them.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.currency import Currency
/// >>> from finstack_quant.core.dates import DayCount
/// >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
/// >>> from finstack_quant.core.money import Money
/// >>> from finstack_quant.valuations.instruments import (
/// ...     AssetPool, PoolAsset, StructuredCredit, Tranche, TrancheStructure,
/// ... )
/// >>> as_of, maturity = datetime.date(2024, 1, 15), datetime.date(2031, 1, 15)
/// >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_assets([
/// ...     PoolAsset.fixed_rate_bond("LOAN-1", Money(80_000_000.0, Currency("USD")), 0.07, maturity, DayCount.ACT_360)
/// ... ])
/// >>> note = (
/// ...     Tranche.builder().id("A").attach_pct(0.0).detach_pct(100.0)
/// ...     .seniority("senior").original_balance(Money(80_000_000.0, Currency("USD")))
/// ...     .coupon_fixed(0.05).maturity(maturity).build()
/// ... )
/// >>> deal = StructuredCredit.new_clo(
/// ...     "CLO-1", pool, TrancheStructure([note]), as_of, maturity, "USD-SOFR-DISC", calendar_id="nyse"
/// ... )
/// >>> market = MarketContext().insert(DiscountCurve.flat("USD-SOFR-DISC", as_of, 0.03))
/// >>> flows = deal.tranche_cashflows("A", market, as_of)
/// >>> flows.tranche_id, list(flows.to_dataframe().columns)
/// ('A', ['date', 'cashflow', 'interest', 'principal', 'pik', 'deferred', 'writedown'])
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "TrancheCashflows",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyTrancheCashflows {
    /// Inner canonical Rust cashflows.
    pub(crate) inner: TrancheCashflows,
}

crate::bindings::macros::wire_methods!(
    PyTrancheCashflows,
    TrancheCashflows,
    "TrancheCashflows",
    to_dict
);

/// Convert dated flows to ``(datetime.date, Money)`` pairs.
fn flows_to_py<'py>(
    py: Python<'py>,
    flows: &[(Date, Money)],
) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
    flows
        .iter()
        .map(|(date, amount)| Ok((date_to_py(py, *date)?, money_to_py(*amount))))
        .collect()
}

#[pymethods]
impl PyTrancheCashflows {
    /// Total cash paid to the tranche per payment date, as
    /// ``(datetime.date, Money)`` pairs.
    #[getter]
    fn cashflows<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        flows_to_py(py, &self.inner.cashflows)
    }

    /// Interest paid per payment date.
    #[getter]
    fn interest_flows<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        flows_to_py(py, &self.inner.interest_flows)
    }

    /// Principal paid per payment date.
    #[getter]
    fn principal_flows<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        flows_to_py(py, &self.inner.principal_flows)
    }

    /// Interest capitalized (PIK) per payment date.
    #[getter]
    fn pik_flows<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        flows_to_py(py, &self.inner.pik_flows)
    }

    /// Interest deferred (shortfall carried forward) per payment date.
    #[getter]
    fn deferred_flows<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        flows_to_py(py, &self.inner.deferred_flows)
    }

    /// Principal written down per payment date.
    #[getter]
    fn writedown_flows<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        flows_to_py(py, &self.inner.writedown_flows)
    }

    /// Accrual periods as ``TrancheAccrualPeriod`` serde dicts.
    #[getter]
    fn accrual_periods<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.accrual_periods)
    }

    /// Detailed classified flows as ``CashFlow`` serde dicts.
    #[getter]
    fn detailed_flows<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.detailed_flows)
    }

    /// One row per payment date as a pandas ``DataFrame``.
    ///
    /// Columns: ``date`` (ISO 8601 string), ``cashflow`` (total paid),
    /// ``interest``, ``principal``, ``pik``, ``deferred`` and ``writedown``,
    /// all in currency units; a component absent on a date is ``0.0``.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     The projected flows in date order.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If component currencies differ or the canonical table cannot be built.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::table::envelope_to_pandas(
            py,
            &self.inner.to_table().map_err(crate::errors::core_to_py)?,
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "TrancheCashflows(tranche_id={:?}, payments={}, total_interest={}, total_principal={})",
            self.inner.tranche_id,
            self.inner.cashflows.len(),
            self.inner.total_interest.amount(),
            self.inner.total_principal.amount()
        )
    }
}

crate::bindings::macros::py_getters!(PyTrancheCashflows |s| {
    /// Tranche identifier.
    tranche_id -> String = s.inner.tranche_id.clone();

    /// Balance outstanding after the last projected payment.
    final_balance -> PyMoney = money_to_py(s.inner.final_balance);

    /// Total interest paid over the projection.
    total_interest -> PyMoney = money_to_py(s.inner.total_interest);

    /// Total principal paid over the projection.
    total_principal -> PyMoney = money_to_py(s.inner.total_principal);

    /// Total interest capitalized over the projection.
    total_pik -> PyMoney = money_to_py(s.inner.total_pik);

    /// Total interest deferred as a claim over the projection.
    total_deferred -> PyMoney = money_to_py(s.inner.total_deferred);

    /// Total principal written down over the projection.
    total_writedown -> PyMoney = money_to_py(s.inner.total_writedown);
});

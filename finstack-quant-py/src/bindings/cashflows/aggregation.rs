//! Python bindings for `finstack_quant_cashflows::aggregation`.

use finstack_quant_cashflows::aggregation::{
    aggregate_by_period, aggregate_cashflows_checked, PeriodAggregation,
};
use finstack_quant_cashflows::DatedFlow;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};

use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::dates::periods::PyPeriod;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::py_to_date;
use crate::errors::core_to_py;

/// Convert Python ``[(date, Money), ...]`` into Rust dated flows.
pub(crate) fn extract_dated_flows(
    flows: Vec<(Bound<'_, PyAny>, PyMoney)>,
) -> PyResult<Vec<DatedFlow>> {
    flows
        .iter()
        .map(|(d, m)| Ok((py_to_date(d)?, m.inner)))
        .collect()
}

/// Per-period, per-currency totals from ``aggregate_by_period`` or
/// ``CashFlowSchedule.pv_by_period``.
///
/// Behaves like a read-only mapping ``{period_id_label: {currency_code: Money}}``
/// (``agg["2025Q1"]["USD"]``, ``"2025Q1" in agg``, ``len(agg)``) and exports
/// a tidy ``pandas.DataFrame`` with one ``(period, currency, amount)`` row per
/// entry. Periods with no flows are omitted; amounts are never FX-converted.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.cashflows.aggregation import aggregate_by_period
/// >>> from finstack_quant.core.dates import build_periods
/// >>> from finstack_quant.core.money import Money
/// >>> flows = [(datetime.date(2025, 3, 15), Money(100.0, "USD"))]
/// >>> agg = aggregate_by_period(flows, build_periods("2025Q1..Q4").periods)
/// >>> agg.periods
/// ['2025Q1']
/// >>> agg.to_dataframe().columns.tolist()
/// ['period', 'currency', 'amount']
#[pyclass(
    name = "PeriodAggregation",
    module = "finstack_quant.cashflows.aggregation",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyPeriodAggregation {
    /// Inner nested totals.
    pub(crate) inner: PeriodAggregation,
}

impl PyPeriodAggregation {
    /// Build from an existing Rust [`PeriodAggregation`].
    pub(crate) fn from_inner(inner: PeriodAggregation) -> Self {
        Self { inner }
    }

    fn period_dict<'py>(
        py: Python<'py>,
        per_currency: &indexmap::IndexMap<
            finstack_quant_core::currency::Currency,
            finstack_quant_core::money::Money,
        >,
    ) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (currency, amount) in per_currency {
            out.set_item(currency.to_string(), PyMoney::from_inner(*amount))?;
        }
        Ok(out)
    }
}

#[pymethods]
impl PyPeriodAggregation {
    /// Period id labels with at least one flow, in reporting-period order.
    #[getter]
    fn periods(&self) -> Vec<String> {
        self.inner.keys().map(ToString::to_string).collect()
    }

    /// Total for one ``(period, currency)`` cell, or ``None`` when absent.
    ///
    /// Parameters
    /// ----------
    /// period : str
    ///     Period id label (e.g. ``"2025Q1"``).
    /// currency : Currency or str
    ///     ISO 4217 currency of the requested total.
    ///
    /// Returns
    /// -------
    /// Money or None
    ///     Nominal (or PV) total in ``currency`` for ``period``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``currency`` is not a valid ISO 4217 code.
    #[pyo3(text_signature = "(self, period, currency)")]
    fn get_amount(&self, period: &str, currency: &Bound<'_, PyAny>) -> PyResult<Option<PyMoney>> {
        let currency = extract_currency(currency)?;
        Ok(self
            .inner
            .get_amount(period, currency)
            .map(PyMoney::from_inner))
    }

    /// Nested ``{period_id_label: {currency_code: Money}}`` dictionary.
    #[pyo3(text_signature = "(self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (period_id, per_currency) in self.inner.iter() {
            out.set_item(period_id.to_string(), Self::period_dict(py, per_currency)?)?;
        }
        Ok(out)
    }

    /// ``{currency_code: Money}`` for one period label.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``period`` has no flows.
    fn __getitem__<'py>(&self, py: Python<'py>, period: &str) -> PyResult<Bound<'py, PyDict>> {
        self.inner
            .iter()
            .find(|(id, _)| id.to_string() == period)
            .map(|(_, per_currency)| Self::period_dict(py, per_currency))
            .unwrap_or_else(|| {
                Err(pyo3::exceptions::PyKeyError::new_err(format!(
                    "no flows in period '{period}'"
                )))
            })
    }

    /// Whether ``period`` has at least one flow.
    fn __contains__(&self, period: &str) -> bool {
        self.inner.keys().any(|id| id.to_string() == period)
    }

    /// Number of non-empty periods.
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// Serialize to JSON (``{period: {currency: money}}``).
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| {
            crate::errors::serde_json_to_py(e, "failed to serialize PeriodAggregation")
        })
    }

    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str::<PeriodAggregation>(json)
            .map(Self::from_inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid PeriodAggregation JSON"))
    }

    /// Support ``pickle`` through the JSON wire form.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Tidy ``pandas.DataFrame`` with columns ``period, currency, amount``.
    ///
    /// ``amount`` is a float in ``currency`` units; one row per non-empty
    /// ``(period, currency)`` cell, in reporting-period order.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = self.inner.rows();
        let columns = PyDict::new(py);
        columns.set_item(
            "period",
            rows.iter()
                .map(|(p, _, _)| p.to_string())
                .collect::<Vec<_>>(),
        )?;
        columns.set_item(
            "currency",
            rows.iter()
                .map(|(_, c, _)| c.to_string())
                .collect::<Vec<_>>(),
        )?;
        columns.set_item(
            "amount",
            rows.iter().map(|(_, _, m)| m.amount()).collect::<Vec<_>>(),
        )?;
        crate::bindings::pandas_utils::dict_to_dataframe(py, &columns, None)
    }

    /// Python-style summary.
    fn __repr__(&self) -> String {
        format!(
            "PeriodAggregation(periods={:?}, cells={})",
            self.periods(),
            self.inner.rows().len()
        )
    }
}

/// Aggregate dated flows into reporting periods (half-open ``[start, end)``).
///
/// Parameters
/// ----------
/// flows : list[tuple[datetime.date, Money]]
///     Dated cashflows; unsorted input is sorted internally.
/// periods : list[Period]
///     Sorted, disjoint reporting periods.
///
/// Returns
/// -------
/// PeriodAggregation
///     Mapping-like ``{period_id_label: {currency_code: nominal_sum}}`` with
///     ``to_dataframe()``; empty periods are omitted.
///
/// Raises
/// ------
/// ValueError
///     If periods are unsorted, overlapping, or contain duplicate ids, or
///     if a per-currency total is non-finite or exceeds the Decimal range.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.cashflows.aggregation import aggregate_by_period
/// >>> from finstack_quant.core.dates import build_periods
/// >>> from finstack_quant.core.money import Money
/// >>> flows = [(datetime.date(2025, 3, 15), Money(100.0, "USD"))]
/// >>> periods = build_periods("2025Q1..Q4").periods
/// >>> aggregate_by_period(flows, periods)["2025Q1"]["USD"].amount
/// 100.0
#[pyfunction(name = "aggregate_by_period")]
#[pyo3(text_signature = "(flows, periods)")]
fn py_aggregate_by_period<'py>(
    py: Python<'py>,
    flows: Vec<(Bound<'py, PyAny>, PyMoney)>,
    periods: Vec<PyRef<'py, PyPeriod>>,
) -> PyResult<PyPeriodAggregation> {
    let flows = extract_dated_flows(flows)?;
    let periods: Vec<finstack_quant_core::dates::Period> =
        periods.iter().map(|p| p.inner.clone()).collect();
    py.detach(move || aggregate_by_period(&flows, &periods))
        .map(PyPeriodAggregation::from_inner)
        .map_err(core_to_py)
}

/// Currency-checked single-currency aggregation with an explicit target currency.
///
/// Parameters
/// ----------
/// flows : list[tuple[datetime.date, Money]]
///     Dated cashflows; every flow must be in ``target`` currency.
/// target : Currency or str
///     Required currency for every flow and the returned total.
///
/// Returns
/// -------
/// Money
///     Single total in ``target`` currency. Empty ``flows`` returns a zero
///     total.
///
/// Raises
/// ------
/// ValueError
///     If any flow currency differs from ``target`` (currency mismatch is
///     rejected, never silently converted).
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.cashflows.aggregation import aggregate_cashflows_checked
/// >>> from finstack_quant.core.money import Money
/// >>> flows = [(datetime.date(2025, 1, 15), Money(50_000.0, "USD"))]
/// >>> aggregate_cashflows_checked(flows, "USD").amount
/// 50000.0
#[pyfunction(name = "aggregate_cashflows_checked")]
#[pyo3(text_signature = "(flows, target)")]
fn py_aggregate_cashflows_checked(
    py: Python<'_>,
    flows: Vec<(Bound<'_, PyAny>, PyMoney)>,
    target: &Bound<'_, PyAny>,
) -> PyResult<PyMoney> {
    let flows = extract_dated_flows(flows)?;
    let target = extract_currency(target)?;
    py.detach(move || aggregate_cashflows_checked(&flows, target))
        .map(PyMoney::from_inner)
        .map_err(core_to_py)
}

/// Group dated cashflows into a calendar-year non-principal / principal / PV ladder.
///
/// Parameters
/// ----------
/// dates : list[datetime.date]
///     Payment dates; the Gregorian year of each date is the bucket.
/// kinds : list[str]
///     Cashflow kind labels (``"fixed"``, ``"notional"``, ``"coupon"``,
///     ``"principal"``, …). ASCII case is ignored. Unknown labels raise
///     ``ValueError``.
/// amounts : list[float]
///     Signed finite cashflow amounts, one per date, in native currency units.
/// pvs : list[float]
///     Finite present values, one per date, in the same units as ``amounts``.
///
/// Returns
/// -------
/// list[tuple[int, float, float, float]]
///     One ``(year, non_principal, principal, pv)`` row per calendar year,
///     sorted by year.
///
/// Raises
/// ------
/// ValueError
///     If the four lists have different lengths, a kind label is unknown, or
///     an amount or PV is non-finite.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.cashflows.aggregation import calendar_year_ladder
/// >>> calendar_year_ladder(
/// ...     [datetime.date(2027, 3, 15), datetime.date(2034, 3, 15)],
/// ...     ["coupon", "principal"],
/// ...     [100.0, 1000.0],
/// ...     [90.0, 700.0],
/// ... )
/// [(2027, 100.0, 0.0, 90.0), (2034, 0.0, 1000.0, 700.0)]
#[pyfunction(name = "calendar_year_ladder")]
#[pyo3(text_signature = "(dates, kinds, amounts, pvs)")]
fn py_calendar_year_ladder(
    py: Python<'_>,
    dates: Vec<Bound<'_, PyAny>>,
    kinds: Vec<String>,
    amounts: Vec<f64>,
    pvs: Vec<f64>,
) -> PyResult<Vec<(i32, f64, f64, f64)>> {
    let dates: Vec<finstack_quant_core::dates::Date> =
        dates.iter().map(py_to_date).collect::<PyResult<_>>()?;
    py.detach(move || {
        let kind_refs: Vec<&str> = kinds.iter().map(String::as_str).collect();
        finstack_quant_cashflows::aggregation::calendar_year_ladder(
            &dates, &kind_refs, &amounts, &pvs,
        )
        .map(|rows| {
            rows.into_iter()
                .map(|row| (row.year, row.non_principal, row.principal, row.pv))
                .collect()
        })
    })
    .map_err(core_to_py)
}

/// Register the `finstack_quant.cashflows.aggregation` submodule.
pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "aggregation")?;
    module.setattr(
        "__doc__",
        "Currency-preserving aggregation of dated cashflows into periods and totals.",
    )?;
    module.add_class::<PyPeriodAggregation>()?;
    module.add_function(wrap_pyfunction!(py_aggregate_by_period, &module)?)?;
    module.add_function(wrap_pyfunction!(py_aggregate_cashflows_checked, &module)?)?;
    module.add_function(wrap_pyfunction!(py_calendar_year_ladder, &module)?)?;

    let all = PyList::new(
        py,
        [
            "PeriodAggregation",
            "aggregate_by_period",
            "aggregate_cashflows_checked",
            "calendar_year_ladder",
        ],
    )?;
    module.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &module,
        "aggregation",
        "finstack_quant.cashflows",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;
    Ok(())
}

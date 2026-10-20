//! Python bindings for the portfolio cashflow ladder: `aggregate_full_cashflows`,
//! `net_in_currency_by_date` and `PortfolioCashflows`.

use crate::bindings::core::currency::extract_currency;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::extract::extract_cashflows_ref;
use crate::bindings::extract::extract_market_ref;
use crate::bindings::extract::extract_portfolio_ref;
use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::core_to_py;
use crate::errors::display_to_py;
use crate::errors::portfolio_to_py;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::types::CurveId;
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::PyAny;
use pyo3::types::PyDict;
use pyo3::types::PyModule;
use std::collections::HashMap;

/// Classified portfolio cashflow ladder.
///
/// Holds the flat position-scaled ``events`` (sorted by payment date), the
/// per-position drill-down ``by_position``, the ``(date -> currency -> kind)``
/// totals ``by_date``, and the extraction ``issues`` recorded for positions
/// that could not produce a schedule. Returned by
/// :func:`aggregate_full_cashflows`; typed getters return JSON-shaped Python
/// objects and the ``*_json`` twins return the same payload as compact JSON.
#[pyclass(
    name = "PortfolioCashflows",
    module = "finstack_quant.portfolio",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPortfolioCashflows {
    pub(crate) inner: finstack_quant_portfolio::cashflows::PortfolioCashflows,
}

impl PyPortfolioCashflows {
    pub(crate) fn from_inner(
        inner: finstack_quant_portfolio::cashflows::PortfolioCashflows,
    ) -> Self {
        Self { inner }
    }

    fn collapse(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        base_currency: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        discount_curves: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<
        IndexMap<
            finstack_quant_core::dates::Date,
            IndexMap<finstack_quant_core::cashflow::CFKind, finstack_quant_core::money::Money>,
        >,
    > {
        let market = crate::bindings::extract::extract_market_ref(py, market)?;
        let ccy = extract_currency(base_currency)?;
        let as_of_date = py_to_date(as_of)?;
        let curves = extract_discount_curve_map(discount_curves)?;
        let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
        let cashflows = &self.inner;
        py.detach(|| {
            cashflows.collapse_to_base_by_date_kind(market_ref, ccy, as_of_date, curves.as_ref())
        })
        .map_err(portfolio_to_py)
    }
}

#[pymethods]
impl PyPortfolioCashflows {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let payload = self.to_json(py)?;
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, payload)
    }

    /// Parse a cashflow ladder from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(cashflows_json)")]
    fn from_json(py: Python<'_>, cashflows_json: &str) -> PyResult<Self> {
        let cashflows_json = cashflows_json.to_owned();
        let inner: finstack_quant_portfolio::cashflows::PortfolioCashflows = py
            .detach(move || serde_json::from_str(&cashflows_json))
            .map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize the full ladder back to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        let cashflows = &self.inner;
        py.detach(|| serde_json::to_string(cashflows))
            .map_err(display_to_py)
    }

    /// Number of dated cashflow events.
    fn __len__(&self) -> usize {
        self.inner.events.len()
    }

    /// Number of positions represented in the ladder (contributing events or
    /// recorded as issues).
    #[getter]
    fn num_positions(&self) -> usize {
        self.inner.by_position.len()
    }

    /// Number of extraction issues recorded during aggregation.
    #[getter]
    fn num_issues(&self) -> usize {
        self.inner.issues.len()
    }

    /// Flat position-scaled events as a list of JSON-shaped dicts (payment
    /// date order). Each carries ``position_id``, ``instrument_id``,
    /// ``instrument_type``, ``date``, ``amount`` (``{"amount", "currency"}``),
    /// ``kind``, ``reset_date``, ``accrual_factor`` and ``rate``.
    #[getter]
    fn events<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.events)
    }

    /// Per-position event drill-down: ``dict[position_id, list[event]]``.
    #[getter]
    fn by_position<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.by_position)
    }

    /// Totals by payment date, then ISO currency, then cashflow kind:
    /// ``dict[date_iso, dict[currency, dict[kind, {"amount", "currency"}]]]``.
    #[getter]
    fn by_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.by_date)
    }

    /// Extraction issues as a list of JSON-shaped dicts (``position_id``,
    /// ``instrument_id``, ``instrument_type``, ``kind``, ``message``).
    #[getter]
    fn issues<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.issues)
    }

    /// FX policy stamped for :meth:`collapse_to_base_by_date_kind` (serde
    /// name).
    #[getter]
    fn fx_collapse_policy(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner.fx_collapse_policy).map_err(core_to_py)
    }

    /// JSON for the flat ``events`` vector only.
    #[pyo3(text_signature = "(self)")]
    fn events_json(&self, py: Python<'_>) -> PyResult<String> {
        let events = &self.inner.events;
        py.detach(|| serde_json::to_string(events))
            .map_err(display_to_py)
    }

    /// JSON for the ``by_date`` currency/kind totals only.
    #[pyo3(text_signature = "(self)")]
    fn by_date_json(&self, py: Python<'_>) -> PyResult<String> {
        let by_date = &self.inner.by_date;
        py.detach(|| serde_json::to_string(by_date))
            .map_err(display_to_py)
    }

    /// Export the flat ``events`` ladder as a pandas ``DataFrame``.
    ///
    /// One row per dated cashflow event, in the ladder's canonical
    /// payment-date order. ``amount`` is flattened to a float column plus a
    /// ``currency`` column rather than a nested dict, so a multi-currency
    /// ladder stays currency-safe: group by ``currency`` before summing.
    ///
    /// Columns: ``position_id``, ``instrument_id``, ``instrument_type``,
    /// ``date`` (ISO 8601 string), ``amount`` (position-scaled), ``currency``,
    /// ``kind`` (``"fixed"``, ``"float_reset"``, ``"notional"``, ...),
    /// ``reset_date`` (ISO 8601 string, ``None`` outside floating coupons),
    /// ``accrual_factor``, ``rate`` (``None`` when the event carries no rate).
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let events = &self.inner.events;
        let position_ids: Vec<&str> = events.iter().map(|e| e.position_id.as_str()).collect();
        let instrument_ids: Vec<&str> = events.iter().map(|e| e.instrument_id.as_str()).collect();
        let instrument_types: Vec<String> = events
            .iter()
            .map(|e| e.instrument_type.to_string())
            .collect();
        let dates: Vec<String> = events.iter().map(|e| e.date.to_string()).collect();
        let amounts: Vec<f64> = events.iter().map(|e| e.amount.amount()).collect();
        let currencies: Vec<String> = events
            .iter()
            .map(|e| e.amount.currency().to_string())
            .collect();
        let kinds: Vec<String> = events.iter().map(|e| e.kind.to_string()).collect();
        let reset_dates: Vec<Option<String>> = events
            .iter()
            .map(|e| e.reset_date.map(|d| d.to_string()))
            .collect();
        let accrual_factors: Vec<f64> = events.iter().map(|e| e.accrual_factor).collect();
        let rates: Vec<Option<f64>> = events.iter().map(|e| e.rate).collect();

        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("instrument_id", instrument_ids)?;
        data.set_item("instrument_type", instrument_types)?;
        data.set_item("date", dates)?;
        data.set_item("amount", amounts)?;
        data.set_item("currency", currencies)?;
        data.set_item("kind", kinds)?;
        data.set_item("reset_date", reset_dates)?;
        data.set_item("accrual_factor", accrual_factors)?;
        data.set_item("rate", rates)?;
        dict_to_dataframe(py, &data, None)
    }

    /// Export the extraction ``issues`` as a pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``instrument_id``, ``instrument_type``,
    /// ``kind`` (issue category, e.g. ``"build_failed"``), ``message``.
    #[pyo3(text_signature = "(self)")]
    fn to_issues_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let issues = &self.inner.issues;
        let position_ids: Vec<&str> = issues.iter().map(|i| i.position_id.as_str()).collect();
        let instrument_ids: Vec<&str> = issues.iter().map(|i| i.instrument_id.as_str()).collect();
        let instrument_types: Vec<String> = issues
            .iter()
            .map(|i| i.instrument_type.to_string())
            .collect();
        let kinds: Vec<String> = issues
            .iter()
            .map(|i| {
                serde_json::to_value(i.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default()
            })
            .collect();
        let messages: Vec<&str> = issues.iter().map(|i| i.message.as_str()).collect();
        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("instrument_id", instrument_ids)?;
        data.set_item("instrument_type", instrument_types)?;
        data.set_item("kind", kinds)?;
        data.set_item("message", messages)?;
        dict_to_dataframe(py, &data, None)
    }

    /// JSON for the ``issues`` vector only.
    #[pyo3(text_signature = "(self)")]
    fn issues_json(&self, py: Python<'_>) -> PyResult<String> {
        let issues = &self.inner.issues;
        py.detach(|| serde_json::to_string(issues))
            .map_err(display_to_py)
    }

    /// Net same-currency cashflow amounts across kinds for each payment date.
    ///
    /// Parameters
    /// ----------
    /// currency : Currency | str
    ///     ISO-4217 currency whose per-date kind buckets are summed.
    ///
    /// Returns
    /// -------
    /// list[tuple[str, float]]
    ///     ``(ISO date, net amount)`` pairs in ladder date order; dates with
    ///     no flows in ``currency`` are omitted. Non-finite amounts are
    ///     skipped and totals use compensated summation.
    #[pyo3(text_signature = "(self, currency)")]
    fn net_in_currency_by_date(&self, currency: &Bound<'_, PyAny>) -> PyResult<Vec<(String, f64)>> {
        let ccy = extract_currency(currency)?;
        Ok(self
            .inner
            .net_in_currency_by_date(ccy)
            .into_iter()
            .map(|(date, amount)| (date.to_string(), amount))
            .collect())
    }

    /// Collapse multi-currency flows into a single base-currency ladder
    /// bucketed by ``(date, kind)`` and return it as a pandas ``DataFrame``.
    ///
    /// Payments on or before ``as_of`` use spot FX at ``as_of``. Later
    /// payments use the CIP forward ``F(T) = S × DF_from(T) / DF_base(T)``.
    /// ``discount_curves`` maps ISO currency codes to discount-curve ids;
    /// omitted currencies fall back to ``market.get_discount(currency)``.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market supplying FX spot and discount curves.
    /// base_currency : Currency | str
    ///     Target currency of the collapsed ladder.
    /// as_of : datetime.date | str
    ///     Spot/forward split date (date-like or ISO 8601 string).
    /// discount_curves : dict[str, str] | None
    ///     Optional ``{currency_code: curve_id}`` overrides.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     Columns ``date`` (ISO 8601 string), ``kind``, ``amount`` (float in
    ///     ``base_currency``), ``currency``; one row per ``(date, kind)``.
    ///     Use :meth:`collapse_to_base_by_date_kind_json` for the nested wire
    ///     form.
    ///
    /// Raises
    /// ------
    /// PortfolioError
    ///     If an FX rate or discount factor needed for a conversion is missing
    ///     or invalid.
    #[pyo3(
        signature = (market, base_currency, as_of, discount_curves=None),
        text_signature = "(self, market, base_currency, as_of, discount_curves=None)"
    )]
    fn collapse_to_base_by_date_kind<'py>(
        &self,
        py: Python<'py>,
        market: &Bound<'py, PyAny>,
        base_currency: &Bound<'py, PyAny>,
        as_of: &Bound<'py, PyAny>,
        discount_curves: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let collapsed = self.collapse(py, market, base_currency, as_of, discount_curves)?;
        let mut dates = Vec::new();
        let mut kinds = Vec::new();
        let mut amounts = Vec::new();
        let mut currencies = Vec::new();
        for (date, per_kind) in &collapsed {
            for (kind, money) in per_kind {
                dates.push(date.to_string());
                kinds.push(kind.to_string());
                amounts.push(money.amount());
                currencies.push(money.currency().to_string());
            }
        }
        let data = PyDict::new(py);
        data.set_item("date", dates)?;
        data.set_item("kind", kinds)?;
        data.set_item("amount", amounts)?;
        data.set_item("currency", currencies)?;
        dict_to_dataframe(py, &data, None)
    }

    /// Wire twin of :meth:`collapse_to_base_by_date_kind`: same inputs, the
    /// nested ``{date: {kind: {"amount", "currency"}}}`` ladder as JSON.
    #[pyo3(
        signature = (market, base_currency, as_of, discount_curves=None),
        text_signature = "(self, market, base_currency, as_of, discount_curves=None)"
    )]
    fn collapse_to_base_by_date_kind_json(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        base_currency: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        discount_curves: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<String> {
        let collapsed = self.collapse(py, market, base_currency, as_of, discount_curves)?;
        py.detach(move || serde_json::to_string(&collapsed))
            .map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "PortfolioCashflows(events={}, positions={}, issues={})",
            self.inner.events.len(),
            self.inner.by_position.len(),
            self.inner.issues.len(),
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

fn extract_discount_curve_map(
    discount_curves: Option<&Bound<'_, PyDict>>,
) -> PyResult<Option<HashMap<Currency, CurveId>>> {
    let Some(dict) = discount_curves else {
        return Ok(None);
    };
    let mut map = HashMap::with_capacity(dict.len());
    for (key, value) in dict.iter() {
        let currency = extract_currency(&key)?;
        let curve_id: String = value.extract()?;
        map.insert(currency, CurveId::new(curve_id));
    }
    Ok(Some(map))
}

/// Aggregate the full classified cashflow ladder.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     A :class:`Portfolio` object (fast path, no rebuild) or a
///     JSON-serialized ``PortfolioSpec`` string.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string used to build
///     instrument schedules.
/// allow_partial : bool | None
///     ``None`` uses the Rust ``CashflowAggregationOptions`` default,
///     ``False``: any schedule-construction issue aborts the call. If
///     ``True``, remaining positions still contribute to the ladder and
///     issues are returned on the result.
///
/// Returns
/// -------
/// PortfolioCashflows
///     Typed wrapper around the full cashflow ladder. Use
///     ``to_json()``/``from_json()`` for round-tripping and typed accessors
///     (``events_json``, ``by_date_json``, ``collapse_to_base_by_date_kind``)
///     to drill in without re-parsing.
///
/// Raises
/// ------
/// PortfolioError
///     If any position fails schedule construction while
///     ``allow_partial`` is ``False``, or same-date same-currency
///     same-kind amounts cannot be added.
#[pyfunction]
#[pyo3(signature = (portfolio, market, allow_partial=None))]
fn aggregate_full_cashflows(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    allow_partial: Option<bool>,
) -> PyResult<PyPortfolioCashflows> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let market = extract_market_ref(py, market)?;
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    let mut options = finstack_quant_portfolio::cashflows::CashflowAggregationOptions::default();
    if let Some(allow_partial) = allow_partial {
        options.allow_partial = allow_partial;
    }
    let cashflows = py
        .detach(|| {
            finstack_quant_portfolio::cashflows::aggregate_full_cashflows(
                portfolio_ref,
                market_ref,
                &options,
            )
        })
        .map_err(portfolio_to_py)?;
    Ok(PyPortfolioCashflows::from_inner(cashflows))
}

/// Net same-currency cashflow amounts across kinds for each payment date.
///
/// Parameters
/// ----------
/// cashflows : PortfolioCashflows | str
///     A :class:`PortfolioCashflows` ladder (fast path, no parse), full
///     cashflow-ladder JSON, or a ``{date: {ccy: {kind: money}}}`` object
///     (optionally wrapped as ``{"by_date": ...}``). Kind keys are opaque
///     strings; money objects require a finite decimal-string ``amount`` and
///     a ``currency`` matching their enclosing bucket. Numeric JSON amounts
///     are rejected by the canonical Money wire contract.
///     Every ISO payment date and currency bucket is validated, including
///     currencies other than the requested output currency.
/// currency : Currency | str
///     ISO-4217 code selecting which per-date currency bucket to net.
///
/// Returns
/// -------
/// list[tuple[str, float]]
///     ``(ISO date, net amount)`` pairs sorted by date. Dates with no flows
///     in ``currency`` are omitted.
///
/// Raises
/// ------
/// TypeError
///     If ``cashflows`` is neither a ``PortfolioCashflows`` nor a string.
/// ValueError
///     If the requested ``currency`` is unknown.
/// PortfolioError
///     If cashflow JSON is malformed, a date or currency bucket is invalid,
///     a container is not an object, a money amount is malformed/non-finite,
///     a money currency differs from its bucket, or a bucket total overflows.
///
/// Examples
/// --------
/// >>> from finstack_quant.portfolio import net_in_currency_by_date
/// >>> net_in_currency_by_date(
/// ...     '{"by_date":{"2025-01-15":{"USD":{"notional":{"amount":"-100","currency":"USD"}}}}}',
/// ...     "USD",
/// ... )
/// [('2025-01-15', -100.0)]
#[pyfunction]
#[pyo3(text_signature = "(cashflows, currency)")]
fn net_in_currency_by_date(
    py: Python<'_>,
    cashflows: &Bound<'_, PyAny>,
    currency: &Bound<'_, PyAny>,
) -> PyResult<Vec<(String, f64)>> {
    let ccy = extract_currency(currency)?;
    if let Ok(cashflows_json) = cashflows.extract::<String>() {
        let code = ccy.to_string();
        return py
            .detach(move || {
                finstack_quant_portfolio::cashflows::net_in_currency_by_date_json(
                    &cashflows_json,
                    &code,
                )
            })
            .map_err(portfolio_to_py);
    }
    let ladder = extract_cashflows_ref(py, cashflows)?;
    Ok(ladder
        .net_in_currency_by_date(ccy)
        .into_iter()
        .map(|(date, amount)| (date.to_string(), amount))
        .collect())
}

/// Register the cashflows bindings on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPortfolioCashflows>()?;
    m.add_function(pyo3::wrap_pyfunction!(aggregate_full_cashflows, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(net_in_currency_by_date, m)?)?;
    Ok(())
}

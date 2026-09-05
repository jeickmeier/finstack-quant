//! Python bindings for `finstack_quant_cashflows::builder::schedule`.

use finstack_quant_cashflows::builder::schedule::merge_cashflow_schedules;
use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, CashflowRepresentation};
use finstack_quant_cashflows::primitives::CashFlow;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::bindings::cashflows::aggregation::PyPeriodAggregation;
use crate::bindings::cashflows::primitives::{extract_cf_kind, PyCashFlow};
use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::periods::PyPeriod;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::errors::core_to_py;

use super::orchestrator::PyCashFlowBuilder;
use super::specs::PyNotional;

/// Parse a schedule representation label (``"contractual"``, ``"projected"``,
/// ``"placeholder"``, ``"no_residual"``).
pub(crate) fn parse_representation(label: &str) -> PyResult<CashflowRepresentation> {
    serde_json::from_value(serde_json::Value::String(label.to_string())).map_err(|_| {
        crate::errors::value_error(format!(
            "unknown cashflow representation '{label}'; expected one of: contractual, projected, placeholder, no_residual"
        ))
    })
}

/// Schedule-level metadata: representation, calendars, facility limit, horizon.
///
/// Parameters
/// ----------
/// representation : str, default "contractual"
///     One of ``"contractual"``, ``"projected"``, ``"placeholder"``,
///     ``"no_residual"``.
/// calendar_ids : list[str], optional
///     Holiday calendar identifiers used by the schedule (default none).
/// facility_limit : Money, optional
///     Facility limit / commitment for revolving structures.
/// issue_date : datetime.date or str, optional
///     Instrument issue date.
/// maturity_date : datetime.date or str, optional
///     Contractual maturity date.
///
/// Raises
/// ------
/// ValueError
///     If ``representation`` is not one of the accepted labels or a date
///     cannot be parsed.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import CashFlowMeta
/// >>> CashFlowMeta(calendar_ids=["usny"]).calendar_ids
/// ['usny']
#[pyclass(
    name = "CashFlowMeta",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyCashFlowMeta {
    /// Inner schedule metadata.
    pub(crate) inner: CashFlowMeta,
}

#[pymethods]
impl PyCashFlowMeta {
    /// Construct schedule metadata; see the class docstring for parameters.
    #[new]
    #[pyo3(
        signature = (representation="contractual", calendar_ids=None, facility_limit=None, issue_date=None, maturity_date=None),
        text_signature = "(representation=\"contractual\", calendar_ids=None, facility_limit=None, issue_date=None, maturity_date=None)"
    )]
    fn new(
        representation: &str,
        calendar_ids: Option<Vec<String>>,
        facility_limit: Option<PyMoney>,
        issue_date: Option<&Bound<'_, PyAny>>,
        maturity_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: CashFlowMeta {
                representation: parse_representation(representation)?,
                calendar_ids: calendar_ids.unwrap_or_default(),
                facility_limit: facility_limit.map(|m| m.inner),
                issue_date: issue_date.map(extract_date).transpose()?,
                maturity_date: maturity_date.map(extract_date).transpose()?,
            },
        })
    }

    /// Schedule representation label (``"contractual"``, ``"projected"``, …).
    #[getter]
    fn representation(&self) -> &'static str {
        self.inner.representation.as_str()
    }

    /// Holiday calendar identifiers used by the schedule.
    #[getter]
    fn calendar_ids(&self) -> Vec<String> {
        self.inner.calendar_ids.clone()
    }

    /// Optional facility limit / commitment.
    #[getter]
    fn facility_limit(&self) -> Option<PyMoney> {
        self.inner.facility_limit.map(PyMoney::from_inner)
    }

    /// Instrument issue date, when known.
    #[getter]
    fn issue_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner.issue_date.map(|d| date_to_py(py, d)).transpose()
    }

    /// Contractual maturity date, when known.
    #[getter]
    fn maturity_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .maturity_date
            .map(|d| date_to_py(py, d))
            .transpose()
    }

    /// Serialize to canonical JSON.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "failed to serialize CashFlowMeta"))
    }

    /// Deserialize from canonical JSON (strict field names).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed or carries unknown fields.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str::<CashFlowMeta>(json)
            .map(|inner| Self { inner })
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid CashFlowMeta JSON"))
    }

    /// Support ``pickle`` through the JSON wire form.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CashFlowMeta", &self.inner)
    }
}

/// Canonical, sorted cashflow schedule with notional, day count and metadata.
///
/// Build one with ``CashFlowSchedule.builder()`` (fluent), ``from_flows`` /
/// ``from_parts`` (explicit rows), or ``from_json``. Accessors return typed
/// ``CashFlow`` rows; ``to_dataframe()`` exports a pandas frame; ``pv_by_period``
/// discounts against a ``MarketContext``.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import CashFlowMeta, CashFlowSchedule, Notional
/// >>> from finstack_quant.core.dates import DayCount
/// >>> CashFlowSchedule.from_parts([], Notional.par(1.0, "USD"), DayCount.ACT_360, CashFlowMeta()).get_notional().initial.amount
/// 1.0
#[pyclass(
    name = "CashFlowSchedule",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyCashFlowSchedule {
    /// Inner canonical schedule.
    pub(crate) inner: CashFlowSchedule,
}

impl PyCashFlowSchedule {
    /// Build from an existing Rust [`CashFlowSchedule`].
    pub(crate) fn from_inner(inner: CashFlowSchedule) -> Self {
        Self { inner }
    }
}

/// Extract a `CashFlowSchedule` from a wrapper or its JSON string.
pub(crate) fn extract_schedule(obj: &Bound<'_, PyAny>) -> PyResult<CashFlowSchedule> {
    if let Ok(schedule) = obj.extract::<PyRef<'_, PyCashFlowSchedule>>() {
        return Ok(schedule.inner.clone());
    }
    let json: String = obj.extract().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("expected CashFlowSchedule or JSON str")
    })?;
    serde_json::from_str::<CashFlowSchedule>(&json)
        .map_err(|e| crate::errors::serde_json_to_py(e, "invalid CashFlowSchedule JSON"))
}

/// Extract one string column of a DataFrame-like object as Python objects.
fn frame_column<'py>(
    frame: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
    let columns = frame.getattr("columns")?;
    if !columns.contains(name)? {
        return Ok(None);
    }
    let values = frame.get_item(name)?.call_method0("tolist")?;
    Ok(Some(values.extract()?))
}

fn is_missing(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    if obj.is_none() {
        return Ok(true);
    }
    if let Ok(value) = obj.extract::<f64>() {
        return Ok(value.is_nan());
    }
    // pandas missing-datetime sentinel (``pd.NaT``) is neither ``None`` nor a float.
    if obj.get_type().name()? == "NaTType" {
        return Ok(true);
    }
    Ok(false)
}

/// Extract cashflow rows from ``list[CashFlow]`` or a DataFrame with columns
/// ``date, amount, currency, kind`` and optional ``reset_date, accrual_factor, rate``.
pub(crate) fn extract_flows(obj: &Bound<'_, PyAny>) -> PyResult<Vec<CashFlow>> {
    if let Ok(flows) = obj.extract::<Vec<PyRef<'_, PyCashFlow>>>() {
        return Ok(flows.iter().map(|f| f.inner.clone()).collect());
    }
    if obj.hasattr("columns")? {
        let missing = |name: &str| {
            crate::errors::value_error(format!(
                "cashflow DataFrame is missing required column '{name}' \
                 (required: date, amount, currency, kind; optional: reset_date, accrual_factor, rate)"
            ))
        };
        let dates = frame_column(obj, "date")?.ok_or_else(|| missing("date"))?;
        let amounts = frame_column(obj, "amount")?.ok_or_else(|| missing("amount"))?;
        let currencies = frame_column(obj, "currency")?.ok_or_else(|| missing("currency"))?;
        let kinds = frame_column(obj, "kind")?.ok_or_else(|| missing("kind"))?;
        let resets = frame_column(obj, "reset_date")?;
        let factors = frame_column(obj, "accrual_factor")?;
        let rates = frame_column(obj, "rate")?;
        let accruals = frame_column(obj, "accrual")?;
        let deltas = frame_column(obj, "principal_delta")?;
        let mut flows = Vec::with_capacity(dates.len());
        for (i, date) in dates.iter().enumerate() {
            let amount: f64 = amounts[i].extract()?;
            let currency = extract_currency(&currencies[i])?;
            let reset = match resets.as_ref().map(|r| &r[i]) {
                Some(r) if !is_missing(r)? => Some(extract_date(r)?),
                _ => None,
            };
            let accrual_factor = match factors.as_ref().map(|f| &f[i]) {
                Some(f) if !is_missing(f)? => f.extract()?,
                _ => 0.0,
            };
            let rate = match rates.as_ref().map(|r| &r[i]) {
                Some(r) if !is_missing(r)? => Some(r.extract()?),
                _ => None,
            };
            let mut flow = CashFlow::new(
                extract_date(date)?,
                reset,
                Money::new(amount, currency).map_err(crate::errors::core_to_py)?,
                extract_cf_kind(&kinds[i])?,
                accrual_factor,
                rate,
            );
            if let Some(value) = accruals.as_ref().map(|rows| &rows[i]) {
                if !is_missing(value)? {
                    let json = crate::bindings::module_utils::py_to_json_string(
                        obj.py(),
                        value,
                        "CashFlowAccrual",
                    )?;
                    flow.accrual = Some(serde_json::from_str(&json).map_err(|e| {
                        crate::errors::serde_json_to_py(e, "invalid CashFlowAccrual")
                    })?);
                }
            }
            if let Some(value) = deltas.as_ref().map(|rows| &rows[i]) {
                if !is_missing(value)? {
                    let json = crate::bindings::module_utils::py_to_json_string(
                        obj.py(),
                        value,
                        "principal_delta",
                    )?;
                    flow.principal_delta = Some(serde_json::from_str(&json).map_err(|e| {
                        crate::errors::serde_json_to_py(e, "invalid principal_delta")
                    })?);
                }
            }
            flow.validate().map_err(core_to_py)?;
            flows.push(flow);
        }
        return Ok(flows);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected list[CashFlow] or a pandas DataFrame with columns date, amount, currency, kind",
    ))
}

#[pymethods]
impl PyCashFlowSchedule {
    /// Create a new fluent cashflow builder (the only builder entry point).
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builder() -> PyCashFlowBuilder {
        PyCashFlowBuilder::new_default()
    }

    /// Build a schedule from explicit rows, a notional, a day count and metadata.
    ///
    /// Rows are sorted into canonical order. Mirrors Rust
    /// ``CashFlowSchedule::from_parts``.
    ///
    /// Parameters
    /// ----------
    /// flows : list[CashFlow]
    ///     Classified cashflow rows in any order.
    /// notional : Notional
    ///     Representative notional stamped on the schedule.
    /// day_count : DayCount
    ///     Representative day-count convention.
    /// meta : CashFlowMeta
    ///     Schedule-level metadata.
    ///
    /// Returns
    /// -------
    /// CashFlowSchedule
    ///     Canonical schedule holding the sorted rows.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``flows`` is not a list of ``CashFlow``.
    #[staticmethod]
    #[pyo3(text_signature = "(flows, notional, day_count, meta)")]
    fn from_parts(
        flows: Vec<PyRef<'_, PyCashFlow>>,
        notional: PyRef<'_, PyNotional>,
        day_count: PyRef<'_, PyDayCount>,
        meta: PyRef<'_, PyCashFlowMeta>,
    ) -> Self {
        Self::from_inner(CashFlowSchedule::from_parts(
            flows.iter().map(|f| f.inner.clone()).collect(),
            notional.inner.clone(),
            day_count.inner,
            meta.inner.clone(),
        ))
    }

    /// Build a schedule from ``CashFlow`` rows or a pandas DataFrame.
    ///
    /// Parameters
    /// ----------
    /// flows : list[CashFlow] or pandas.DataFrame
    ///     Either typed rows, or a frame with columns ``date``, ``amount``
    ///     (float, native currency units), ``currency`` (ISO code), ``kind``
    ///     (``CFKind`` label such as ``"fixed"``), and optional
    ///     ``reset_date``, ``accrual_factor``, ``rate``. Dates accept
    ///     ``datetime.date``, ``pandas.Timestamp`` or ISO strings; ``NaN`` /
    ///     ``None`` in the optional columns means absent.
    /// notional : Notional
    ///     Representative notional stamped on the schedule.
    /// day_count : DayCount
    ///     Representative day-count convention.
    /// meta : CashFlowMeta, optional
    ///     Schedule metadata (default: contractual, no calendars).
    ///
    /// Returns
    /// -------
    /// CashFlowSchedule
    ///     Canonical schedule holding the sorted rows.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a required frame column is missing, a currency code or kind
    ///     label is unknown, or a date cannot be parsed.
    /// TypeError
    ///     If ``flows`` is neither a list of ``CashFlow`` nor a DataFrame.
    ///
    /// Examples
    /// --------
    /// >>> import datetime
    /// >>> from finstack_quant.cashflows.builder import CashFlowSchedule, Notional
    /// >>> from finstack_quant.cashflows.primitives import CashFlow, CFKind
    /// >>> from finstack_quant.core.dates import DayCount
    /// >>> from finstack_quant.core.money import Money
    /// >>> flow = CashFlow(datetime.date(2025, 6, 15), Money(100.0, "USD"), CFKind.FIXED)
    /// >>> CashFlowSchedule.from_flows([flow], Notional.par(1_000.0, "USD"), DayCount.ACT_360).get_flows()[0].kind.name
    /// 'fixed'
    #[staticmethod]
    #[pyo3(
        signature = (flows, notional, day_count, meta=None),
        text_signature = "(flows, notional, day_count, meta=None)"
    )]
    fn from_flows(
        flows: &Bound<'_, PyAny>,
        notional: PyRef<'_, PyNotional>,
        day_count: PyRef<'_, PyDayCount>,
        meta: Option<PyRef<'_, PyCashFlowMeta>>,
    ) -> PyResult<Self> {
        let flows = extract_flows(flows)?;
        Ok(Self::from_inner(CashFlowSchedule::from_parts(
            flows,
            notional.inner.clone(),
            day_count.inner,
            meta.map_or_else(CashFlowMeta::default, |m| m.inner.clone()),
        )))
    }

    /// Return a copy with the metadata representation label replaced.
    ///
    /// Parameters
    /// ----------
    /// representation : str
    ///     One of ``"contractual"``, ``"projected"``, ``"placeholder"``,
    ///     ``"no_residual"``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the label is not one of the accepted representations.
    #[pyo3(text_signature = "(self, representation)")]
    fn with_representation(&self, representation: &str) -> PyResult<Self> {
        Ok(Self::from_inner(self.inner.clone().with_representation(
            parse_representation(representation)?,
        )))
    }

    /// Return a copy with the representative notional replaced (flows unchanged).
    ///
    /// Parameters
    /// ----------
    /// notional : Notional
    ///     New representative notional.
    #[pyo3(text_signature = "(self, notional)")]
    fn with_notional(&self, notional: PyRef<'_, PyNotional>) -> Self {
        Self::from_inner(self.inner.clone().with_notional(notional.inner.clone()))
    }

    /// Canonical ordered cashflows.
    #[pyo3(text_signature = "(self)")]
    fn get_flows(&self) -> Vec<PyCashFlow> {
        self.inner
            .get_flows()
            .iter()
            .map(|f| PyCashFlow::from_inner(f.clone()))
            .collect()
    }

    /// Interest-like coupon cashflows (Fixed/FloatReset/InflationCoupon/Stub).
    #[pyo3(text_signature = "(self)")]
    fn coupons(&self) -> Vec<PyCashFlow> {
        self.inner
            .coupons()
            .map(|f| PyCashFlow::from_inner(f.clone()))
            .collect()
    }

    /// Flow dates in schedule order.
    #[pyo3(text_signature = "(self)")]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.inner
            .dates()
            .into_iter()
            .map(|d| date_to_py(py, d))
            .collect()
    }

    /// Representative schedule notional.
    #[pyo3(text_signature = "(self)")]
    fn get_notional(&self) -> PyNotional {
        PyNotional::from_inner(self.inner.get_notional().clone())
    }

    /// Representative day-count convention.
    #[pyo3(text_signature = "(self)")]
    fn get_day_count(&self) -> PyDayCount {
        PyDayCount::from_inner(self.inner.get_day_count())
    }

    /// Schedule-level metadata.
    #[pyo3(text_signature = "(self)")]
    fn get_meta(&self) -> PyCashFlowMeta {
        PyCashFlowMeta {
            inner: self.inner.get_meta().clone(),
        }
    }

    /// Validate all schedule-level and per-flow invariants.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a flow is non-finite, mis-ordered, or the notional is invalid.
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Return a new schedule with every amount scaled by ``scale``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``scale`` is NaN or infinite.
    #[pyo3(text_signature = "(self, scale)")]
    fn scale_amounts(&self, scale: f64) -> PyResult<Self> {
        self.inner
            .clone()
            .scale_amounts(scale)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Weighted average life in years from ``as_of`` (Act/365F basis).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the schedule carries no principal flows after ``as_of``.
    #[pyo3(text_signature = "(self, as_of)")]
    fn weighted_average_life(&self, as_of: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner
            .weighted_average_life(extract_date(as_of)?)
            .map_err(core_to_py)
    }

    /// Outstanding balance path as ``[(date, Money), ...]``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If principal flows mix currencies.
    #[pyo3(text_signature = "(self)")]
    fn outstanding_by_date<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .outstanding_by_date()
            .map_err(core_to_py)?
            .into_iter()
            .map(|(d, m)| Ok((date_to_py(py, d)?, PyMoney::from_inner(m))))
            .collect()
    }

    /// Periodized present values resolved from a market context.
    ///
    /// Parameters
    /// ----------
    /// periods : list[Period]
    ///     Reporting periods (half-open buckets).
    /// market : MarketContext or str
    ///     Market context (or its JSON) containing the required curves.
    /// disc_curve_id : str
    ///     Discount curve identifier.
    /// base : datetime.date or str
    ///     Valuation date for discount times; flows on or before it get zero PV.
    /// day_count : DayCount, optional
    ///     Day-count for discount times (default Act/365F).
    /// hazard_curve_id : str, optional
    ///     Hazard curve identifier for credit-adjusted PV.
    ///
    /// Returns
    /// -------
    /// PeriodAggregation
    ///     ``{period_id_label: {currency_code: pv}}`` with ``to_dataframe()``;
    ///     empty periods omitted.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a curve id is not present in ``market``.
    /// ValueError
    ///     If periods overlap or discount inputs are inconsistent.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(
        signature = (periods, market, disc_curve_id, base, day_count=None, hazard_curve_id=None),
        text_signature = "(self, periods, market, disc_curve_id, base, day_count=None, hazard_curve_id=None)"
    )]
    fn pv_by_period<'py>(
        &self,
        py: Python<'py>,
        periods: Vec<PyRef<'_, PyPeriod>>,
        market: &Bound<'_, PyAny>,
        disc_curve_id: &str,
        base: &Bound<'_, PyAny>,
        day_count: Option<PyRef<'_, PyDayCount>>,
        hazard_curve_id: Option<&str>,
    ) -> PyResult<PyPeriodAggregation> {
        let periods: Vec<finstack_quant_core::dates::Period> =
            periods.iter().map(|p| p.inner.clone()).collect();
        let market = crate::bindings::extract::extract_market(py, market)?;
        let base = extract_date(base)?;
        let day_count = day_count.map(|d| d.inner);
        let disc_id = CurveId::from(disc_curve_id);
        let hazard_id = hazard_curve_id.map(CurveId::from);
        let schedule = self.inner.clone();
        py.detach(move || {
            schedule.pv_by_period(
                &periods,
                &market,
                &disc_id,
                hazard_id.as_ref(),
                base,
                day_count,
            )
        })
        .map(PyPeriodAggregation::from_inner)
        .map_err(core_to_py)
    }

    /// Calendar-year non-principal / principal / PV ladder as a DataFrame.
    ///
    /// Parameters
    /// ----------
    /// pvs : list[float]
    ///     Present value of each flow in ``get_flows()`` order, in the flow's
    ///     currency units; must be finite.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     Columns ``year, non_principal, principal, pv`` sorted by year.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``pvs`` does not have one entry per flow or contains a
    ///     non-finite value.
    #[pyo3(text_signature = "(self, pvs)")]
    fn calendar_year_ladder<'py>(
        &self,
        py: Python<'py>,
        pvs: Vec<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let rows = self.inner.calendar_year_ladder(&pvs).map_err(core_to_py)?;
        let columns = PyDict::new(py);
        columns.set_item("year", rows.iter().map(|r| r.year).collect::<Vec<_>>())?;
        columns.set_item(
            "non_principal",
            rows.iter().map(|r| r.non_principal).collect::<Vec<_>>(),
        )?;
        columns.set_item(
            "principal",
            rows.iter().map(|r| r.principal).collect::<Vec<_>>(),
        )?;
        columns.set_item("pv", rows.iter().map(|r| r.pv).collect::<Vec<_>>())?;
        crate::bindings::pandas_utils::dict_to_dataframe(py, &columns, None)
    }

    /// Serialize the canonical schedule to JSON.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "failed to serialize CashFlowSchedule"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a schedule from canonical JSON (strict field names).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed or carries unknown fields.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str::<CashFlowSchedule>(json)
            .map(Self::from_inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid CashFlowSchedule JSON"))
    }

    /// Flows as a pandas DataFrame.
    ///
    /// Parameters
    /// ----------
    /// outstanding : bool, default False
    ///     Append an ``outstanding`` column with the principal balance
    ///     (float, in the flow currency) after the last principal event on or
    ///     before each flow date, from ``outstanding_by_date``.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     Columns ``date`` (``datetime64``), ``reset_date`` (``datetime64``,
    ///     ``NaT`` when absent), ``kind``, ``amount`` (float), ``currency``,
    ///     ``accrual_factor``, ``rate`` (``NaN`` when absent) and optionally
    ///     ``outstanding``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``outstanding=True`` and principal flows mix currencies.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(signature = (outstanding=false), text_signature = "(self, outstanding=False)")]
    fn to_dataframe<'py>(&self, py: Python<'py>, outstanding: bool) -> PyResult<Bound<'py, PyAny>> {
        let flows = self.inner.get_flows();
        let pd = py.import("pandas")?;
        let dates: Vec<time::Date> = flows.iter().map(|f| f.date).collect();
        let reset_dates: Vec<Option<Bound<'py, PyAny>>> = flows
            .iter()
            .map(|f| f.reset_date.map(|d| date_to_py(py, d)).transpose())
            .collect::<PyResult<_>>()?;
        let kinds: Vec<String> = flows.iter().map(|f| f.kind.to_string()).collect();
        let amounts: Vec<f64> = flows.iter().map(|f| f.amount.amount()).collect();
        let currencies: Vec<String> = flows
            .iter()
            .map(|f| f.amount.currency().to_string())
            .collect();
        let factors: Vec<f64> = flows.iter().map(|f| f.accrual_factor).collect();
        let rates: Vec<Option<f64>> = flows.iter().map(|f| f.rate).collect();
        let columns = PyDict::new(py);
        columns.set_item(
            "date",
            pd.call_method1(
                "to_datetime",
                (crate::bindings::pandas_utils::dates_to_pylist(py, &dates)?,),
            )?,
        )?;
        columns.set_item(
            "reset_date",
            pd.call_method1("to_datetime", (reset_dates,))?,
        )?;
        columns.set_item("kind", kinds)?;
        columns.set_item("amount", amounts)?;
        columns.set_item("currency", currencies)?;
        columns.set_item("accrual_factor", factors)?;
        columns.set_item("rate", rates)?;
        columns.set_item(
            "accrual",
            crate::bindings::pandas_utils::serde_to_py(
                py,
                &flows.iter().map(|f| &f.accrual).collect::<Vec<_>>(),
            )?,
        )?;
        columns.set_item(
            "principal_delta",
            crate::bindings::pandas_utils::serde_to_py(
                py,
                &flows.iter().map(|f| f.principal_delta).collect::<Vec<_>>(),
            )?,
        )?;
        if outstanding {
            let path = self.inner.outstanding_by_date().map_err(core_to_py)?;
            let balances: Vec<Option<f64>> = flows
                .iter()
                .map(|f| {
                    path.iter()
                        .take_while(|(d, _)| *d <= f.date)
                        .last()
                        .map(|(_, m)| m.amount())
                })
                .collect();
            columns.set_item("outstanding", balances)?;
        }
        crate::bindings::pandas_utils::dict_to_dataframe(py, &columns, None)
    }

    /// Python-style summary.
    fn __repr__(&self) -> String {
        let notional = self.inner.get_notional();
        format!(
            "CashFlowSchedule(flows={}, notional={} {}, day_count='{}', representation='{}')",
            self.inner.get_flows().len(),
            notional.initial.amount(),
            notional.initial.currency(),
            self.inner.get_day_count(),
            self.inner.get_meta().representation.as_str(),
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py, false).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Merge multiple schedules into one deterministic composite schedule.
///
/// Parameters
/// ----------
/// schedules : list[CashFlowSchedule]
///     Schedules to combine.
/// notional : Notional
///     Representative notional stamped on the merged schedule.
/// day_count : DayCount
///     Day-count convention attached to the merged schedule.
///
/// Returns
/// -------
/// CashFlowSchedule
///     Combined schedule with all rows in canonical order.
#[pyfunction(name = "merge_cashflow_schedules")]
#[pyo3(text_signature = "(schedules, notional, day_count)")]
pub(crate) fn py_merge_cashflow_schedules(
    schedules: Vec<PyRef<'_, PyCashFlowSchedule>>,
    notional: PyRef<'_, PyNotional>,
    day_count: PyRef<'_, PyDayCount>,
) -> PyCashFlowSchedule {
    let inner = merge_cashflow_schedules(
        schedules.iter().map(|s| s.inner.clone()),
        notional.inner.clone(),
        day_count.inner,
    );
    PyCashFlowSchedule::from_inner(inner)
}

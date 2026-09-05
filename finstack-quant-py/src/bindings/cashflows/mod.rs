//! Python bindings for the `finstack-quant-cashflows` crate.

pub(crate) mod accrual;
pub(crate) mod aggregation;
pub(crate) mod builder;
pub(crate) mod primitives;
mod schema;

use finstack_quant_cashflows::builder::CashFlowMeta;
use finstack_quant_cashflows::{CashflowScheduleBuildSpec, ScheduleBuildOpts};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::date_to_py;
use builder::schedule::{extract_schedule, PyCashFlowMeta, PyCashFlowSchedule};
use primitives::{extract_cf_kind, PyCashFlow};

/// Build a cashflow schedule from a JSON spec and return canonical schedule JSON.
///
/// Parameters
/// ----------
/// spec_json : str
///     JSON-encoded `CashflowScheduleBuildSpec`.
/// market_json : str, optional
///     JSON-encoded market context for floating-rate lookups.
///
/// Returns
/// -------
/// str
///     JSON-encoded `CashFlowSchedule`.
///
/// Raises
/// ------
/// ValueError
///     If either JSON document is malformed or the schedule fails to build.
/// KeyError
///     If a floating leg references a curve missing from ``market_json``.
#[pyfunction]
#[pyo3(
    signature = (spec_json, market_json = None),
    text_signature = "(spec_json, market_json=None)"
)]
fn build_cashflow_schedule_json(
    py: Python<'_>,
    spec_json: &str,
    market_json: Option<&str>,
) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_cashflows::build_cashflow_schedule_json(spec_json, market_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Build a typed ``CashFlowSchedule`` from a build spec (typed twin of
/// ``build_cashflow_schedule_json``).
///
/// Parameters
/// ----------
/// spec : dict or str
///     ``CashflowScheduleBuildSpec`` as a JSON string or an equivalent
///     ``dict`` (keys ``notional``, ``issue``, ``maturity``,
///     ``coupon_program``, ``payment_program``, ``fees``,
///     ``principal_events``, ``principal_exchange``).
/// market : MarketContext or str, optional
///     Market context (or its JSON) for floating-rate projection; fixed
///     coupons and deterministic fees do not need one.
///
/// Returns
/// -------
/// CashFlowSchedule
///     Canonical typed schedule with ``to_dataframe()``.
///
/// Raises
/// ------
/// ValueError
///     If the spec is malformed or the schedule fails validation.
/// KeyError
///     If a floating leg references a curve missing from ``market``.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows import build_cashflow_schedule
/// >>> spec = {
/// ...     "notional": {"initial": {"amount": 1000000.0, "currency": "USD"}, "amort": "none"},
/// ...     "issue": "2025-01-15", "maturity": "2026-01-15",
/// ...     "coupon_program": [{"kind": "fixed", "rate": "0.05", "coupon_type": "cash",
/// ...         "frequency": "6M", "day_count": "30/360", "calendar_id": "weekends_only"}],
/// ... }
/// >>> build_cashflow_schedule(spec).get_flows()[0].kind.name
/// 'notional'
#[pyfunction]
#[pyo3(signature = (spec, market=None), text_signature = "(spec, market=None)")]
fn build_cashflow_schedule(
    py: Python<'_>,
    spec: &Bound<'_, PyAny>,
    market: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyCashFlowSchedule> {
    let spec_json =
        crate::bindings::module_utils::py_to_json_string(py, spec, "CashflowScheduleBuildSpec")?;
    let spec: CashflowScheduleBuildSpec = serde_json::from_str(&spec_json)
        .map_err(|e| crate::errors::serde_json_to_py(e, "invalid CashflowScheduleBuildSpec"))?;
    let market = crate::bindings::extract::extract_market_opt(py, market)?;
    py.detach(move || spec.build(market.as_ref()))
        .map(PyCashFlowSchedule::from_inner)
        .map_err(crate::errors::core_to_py)
}

/// Validate a cashflow schedule JSON string and return it canonicalized.
///
/// Parameters
/// ----------
/// schedule_json : str
///     JSON-encoded `CashFlowSchedule`.
///
/// Returns
/// -------
/// str
///     Canonicalized JSON-encoded `CashFlowSchedule`.
///
/// Raises
/// ------
/// ValueError
///     If the JSON is malformed or the schedule fails validation.
#[pyfunction]
#[pyo3(text_signature = "(schedule_json)")]
fn validate_cashflow_schedule_json(py: Python<'_>, schedule_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_cashflows::validate_cashflow_schedule_json(schedule_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Extract dated flows from a cashflow schedule.
///
/// Parameters
/// ----------
/// schedule_json : str
///     JSON-encoded `CashFlowSchedule`.
///
/// Returns
/// -------
/// str
///     JSON array of settlement cash entries. Non-cash PIK and default-write-down
///     rows are omitted; parse the full schedule JSON when classifications are needed.
///
/// Raises
/// ------
/// ValueError
///     If the schedule JSON is malformed.
#[pyfunction]
#[pyo3(text_signature = "(schedule_json)")]
fn dated_flows_json(py: Python<'_>, schedule_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_cashflows::dated_flows_json(schedule_json).map_err(crate::errors::core_to_py)
    })
}

/// Settlement cash entries of a schedule as ``[(date, Money), ...]`` (typed
/// twin of ``dated_flows_json``).
///
/// Parameters
/// ----------
/// schedule : CashFlowSchedule or str
///     Typed schedule or its canonical JSON.
///
/// Returns
/// -------
/// list[tuple[datetime.date, Money]]
///     Cash-settling rows in schedule order; PIK capitalizations and
///     default write-downs are omitted.
///
/// Raises
/// ------
/// ValueError
///     If ``schedule`` is a malformed JSON string.
/// TypeError
///     If ``schedule`` is neither a ``CashFlowSchedule`` nor a string.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.cashflows import dated_flows, schedule_from_dated_flows
/// >>> from finstack_quant.core.dates import DayCount
/// >>> from finstack_quant.core.money import Money
/// >>> schedule = schedule_from_dated_flows([(datetime.date(2025, 6, 15), Money(100.0, "USD"))], "fixed", DayCount.ACT_360)
/// >>> dated_flows(schedule)[0][1].amount
/// 100.0
#[pyfunction]
#[pyo3(text_signature = "(schedule)")]
fn dated_flows<'py>(
    py: Python<'py>,
    schedule: &Bound<'py, PyAny>,
) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
    let schedule = extract_schedule(schedule)?;
    schedule
        .get_flows()
        .iter()
        .filter(|flow| finstack_quant_cashflows::primitives::is_cash_settlement_kind(flow.kind))
        .map(|flow| Ok((date_to_py(py, flow.date)?, PyMoney::from_inner(flow.amount))))
        .collect()
}

/// Compute accrued interest for a schedule as of a given date.
///
/// Parameters
/// ----------
/// schedule_json : str
///     JSON-encoded `CashFlowSchedule`.
/// as_of : datetime.date | str
///     Accrual snapshot date, either a date-like object (``datetime.date``,
///     ``pandas.Timestamp``) or an ISO 8601 string.
/// config_json : str, optional
///     JSON-encoded `AccrualConfig` overriding defaults.
///
/// Returns
/// -------
/// float
///     Accrued interest in the schedule's settlement currency, returned as a
///     host-language double. The Rust engine computes from the canonical
///     schedule and then crosses the binding boundary as `f64`; for large
///     notionals, compare results with an absolute tolerance scaled to the
///     schedule notional rather than expecting decimal-string equality.
///
/// Raises
/// ------
/// ValueError
///     If either JSON document is malformed or the schedule fails validation.
#[pyfunction]
#[pyo3(
    signature = (schedule_json, as_of, config_json = None),
    text_signature = "(schedule_json, as_of, config_json=None)"
)]
fn accrued_interest(
    py: Python<'_>,
    schedule_json: &str,
    as_of: &Bound<'_, PyAny>,
    config_json: Option<&str>,
) -> PyResult<f64> {
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    py.detach(|| {
        finstack_quant_cashflows::accrued_interest(schedule_json, &as_of, config_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Schedule-level inputs shared by ``schedule_from_dated_flows`` and
/// ``schedule_from_classified_flows``.
///
/// Parameters
/// ----------
/// notional_hint : Money, optional
///     Notional stamped on the resulting schedule. When omitted, a zero
///     notional in the currency of the first flow (USD if there are none)
///     is used.
/// meta : CashFlowMeta, optional
///     Schedule-level metadata (default: contractual, no calendars).
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows import ScheduleBuildOpts
/// >>> from finstack_quant.core.money import Money
/// >>> ScheduleBuildOpts(notional_hint=Money(100.0, "USD")).notional_hint.amount
/// 100.0
#[pyclass(
    name = "ScheduleBuildOpts",
    module = "finstack_quant.cashflows",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyScheduleBuildOpts {
    /// Inner build options.
    pub(crate) inner: ScheduleBuildOpts,
}

#[pymethods]
impl PyScheduleBuildOpts {
    /// Construct build options; see the class docstring for parameters.
    #[new]
    #[pyo3(
        signature = (notional_hint=None, meta=None),
        text_signature = "(notional_hint=None, meta=None)"
    )]
    fn new(notional_hint: Option<PyMoney>, meta: Option<PyRef<'_, PyCashFlowMeta>>) -> Self {
        Self {
            inner: ScheduleBuildOpts {
                notional_hint: notional_hint.map(|m| m.inner),
                meta: meta.map_or_else(CashFlowMeta::default, |m| m.inner.clone()),
            },
        }
    }

    /// Notional stamped on the resulting schedule, if provided.
    #[getter]
    fn notional_hint(&self) -> Option<PyMoney> {
        self.inner.notional_hint.map(PyMoney::from_inner)
    }

    /// Schedule-level metadata.
    #[getter]
    fn meta(&self) -> PyCashFlowMeta {
        PyCashFlowMeta {
            inner: self.inner.meta.clone(),
        }
    }

    /// Python-style summary.
    fn __repr__(&self) -> String {
        let hint = self.inner.notional_hint.map_or_else(
            || "None".to_string(),
            |m| format!("{} {}", m.amount(), m.currency()),
        );
        format!(
            "ScheduleBuildOpts(notional_hint={hint}, meta={})",
            crate::bindings::repr_support::repr_from_serde("CashFlowMeta", &self.inner.meta)
        )
    }
}

/// Build a ``CashFlowSchedule`` from dated ``(date, Money)`` flows that all
/// share one classification.
///
/// Parameters
/// ----------
/// flows : list[tuple[datetime.date, Money]]
///     Dated amounts; any order.
/// kind : CFKind or str
///     Classification stamped on every row (e.g. ``"fixed"``).
/// day_count : DayCount
///     Representative day-count convention of the schedule.
/// opts : ScheduleBuildOpts, optional
///     Notional hint and metadata (default: zero notional in the first flow's
///     currency, contractual metadata).
///
/// Returns
/// -------
/// CashFlowSchedule
///     Canonical schedule with ``accrual_factor`` 0 and no rates on each row.
///
/// Raises
/// ------
/// ValueError
///     If ``kind`` is not a known cashflow kind label or a date cannot be
///     parsed.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.cashflows import schedule_from_dated_flows
/// >>> from finstack_quant.core.dates import DayCount
/// >>> from finstack_quant.core.money import Money
/// >>> flows = [(datetime.date(2025, 6, 15), Money(100.0, "USD"))]
/// >>> schedule_from_dated_flows(flows, "fixed", DayCount.THIRTY_360).get_flows()[0].amount.amount
/// 100.0
#[pyfunction]
#[pyo3(
    signature = (flows, kind, day_count, opts=None),
    text_signature = "(flows, kind, day_count, opts=None)"
)]
fn schedule_from_dated_flows(
    flows: Vec<(Bound<'_, PyAny>, PyMoney)>,
    kind: &Bound<'_, PyAny>,
    day_count: PyRef<'_, PyDayCount>,
    opts: Option<PyRef<'_, PyScheduleBuildOpts>>,
) -> PyResult<PyCashFlowSchedule> {
    let flows = aggregation::extract_dated_flows(flows)?;
    let kind = extract_cf_kind(kind)?;
    let opts = opts.map_or_else(ScheduleBuildOpts::default, |o| o.inner.clone());
    Ok(PyCashFlowSchedule::from_inner(
        finstack_quant_cashflows::schedule_from_dated_flows(flows, kind, day_count.inner, opts),
    ))
}

/// Build a ``CashFlowSchedule`` from pre-classified ``CashFlow`` rows.
///
/// Parameters
/// ----------
/// flows : list[CashFlow]
///     Classified rows; any order, kinds preserved.
/// day_count : DayCount
///     Representative day-count convention of the schedule.
/// opts : ScheduleBuildOpts, optional
///     Notional hint and metadata (default: zero notional in the first flow's
///     currency, contractual metadata).
///
/// Returns
/// -------
/// CashFlowSchedule
///     Canonical schedule holding the sorted rows.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.cashflows import schedule_from_classified_flows
/// >>> from finstack_quant.cashflows.primitives import CashFlow, CFKind
/// >>> from finstack_quant.core.dates import DayCount
/// >>> from finstack_quant.core.money import Money
/// >>> flow = CashFlow(datetime.date(2025, 6, 15), Money(100.0, "USD"), CFKind.PIK)
/// >>> schedule_from_classified_flows([flow], DayCount.ACT_360).get_flows()[0].kind.name
/// 'pik'
#[pyfunction]
#[pyo3(
    signature = (flows, day_count, opts=None),
    text_signature = "(flows, day_count, opts=None)"
)]
fn schedule_from_classified_flows(
    flows: Vec<PyRef<'_, PyCashFlow>>,
    day_count: PyRef<'_, PyDayCount>,
    opts: Option<PyRef<'_, PyScheduleBuildOpts>>,
) -> PyCashFlowSchedule {
    let flows = flows.iter().map(|f| f.inner.clone()).collect();
    let opts = opts.map_or_else(ScheduleBuildOpts::default, |o| o.inner.clone());
    PyCashFlowSchedule::from_inner(finstack_quant_cashflows::schedule_from_classified_flows(
        flows,
        day_count.inner,
        opts,
    ))
}

/// Register the `finstack_quant.cashflows` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "cashflows")?;
    m.setattr(
        "__doc__",
        "Cashflow schedule construction (typed and JSON), validation, and dated-flow extraction.",
    )?;
    m.setattr("__package__", "finstack_quant.cashflows")?;

    primitives::register(py, &m)?;
    builder::register(py, &m)?;
    accrual::register(py, &m)?;
    aggregation::register(py, &m)?;
    schema::register(py, &m)?;

    m.add_class::<PyScheduleBuildOpts>()?;
    m.add_function(wrap_pyfunction!(accrued_interest, &m)?)?;
    m.add_function(wrap_pyfunction!(build_cashflow_schedule, &m)?)?;
    m.add_function(wrap_pyfunction!(build_cashflow_schedule_json, &m)?)?;
    m.add_function(wrap_pyfunction!(dated_flows, &m)?)?;
    m.add_function(wrap_pyfunction!(dated_flows_json, &m)?)?;
    m.add_function(wrap_pyfunction!(schedule_from_classified_flows, &m)?)?;
    m.add_function(wrap_pyfunction!(schedule_from_dated_flows, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_cashflow_schedule_json, &m)?)?;
    // Prepayment/default rate-convention conversions. The Rust crate root
    // re-exports these from `builder::credit_rates`, so they are exposed both
    // flat here (mirroring the crate root, and the WASM facade) and on the
    // typed `builder` submodule (mirroring `builder::`).
    m.add_function(wrap_pyfunction!(builder::cdr_to_mdr, &m)?)?;
    m.add_function(wrap_pyfunction!(builder::cpr_to_smm, &m)?)?;
    m.add_function(wrap_pyfunction!(builder::mdr_to_cdr, &m)?)?;
    m.add_function(wrap_pyfunction!(builder::smm_to_cpr, &m)?)?;

    for name in [
        "accrued_interest",
        "build_cashflow_schedule",
        "build_cashflow_schedule_json",
        "cdr_to_mdr",
        "cpr_to_smm",
        "dated_flows",
        "dated_flows_json",
        "mdr_to_cdr",
        "schedule_from_classified_flows",
        "schedule_from_dated_flows",
        "smm_to_cpr",
        "validate_cashflow_schedule_json",
    ] {
        m.getattr(name)?
            .setattr("__module__", "finstack_quant.cashflows")?;
    }

    let all = PyList::new(
        py,
        [
            "ScheduleBuildOpts",
            "accrual",
            "accrued_interest",
            "aggregation",
            "build_cashflow_schedule",
            "build_cashflow_schedule_json",
            "builder",
            "cdr_to_mdr",
            "cpr_to_smm",
            "dated_flows",
            "dated_flows_json",
            "mdr_to_cdr",
            "primitives",
            "schedule_from_classified_flows",
            "schedule_from_dated_flows",
            "schema",
            "smm_to_cpr",
            "validate_cashflow_schedule_json",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "cashflows",
        crate::bindings::module_utils::ROOT_PACKAGE,
        crate::bindings::module_utils::ParentNameSource::Name,
    )?;

    Ok(())
}

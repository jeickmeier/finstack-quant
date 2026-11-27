//! Cashflow schedule JSON, dated-flow extraction, and construction helpers.

use super::aggregation;
use super::builder::schedule::{PyCashFlowMeta, PyCashFlowSchedule};
use crate::bindings::macros::py_getters;
use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::builder::CashFlowMeta;
use finstack_quant_cashflows::{CashflowScheduleBuildSpec, ScheduleBuildOpts};
use pyo3::prelude::*;

use crate::bindings::core::cashflow::{PyCashFlow, extract_cf_kind};
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::core::money::PyMoney;

#[doc = pydoc!("finstack_quant.cashflows._schedule_json", "build_cashflow_schedule_json")]
#[pyfunction]
#[pyo3(
    signature = (spec_json, market_json = None, as_of = None),
    text_signature = "(spec_json, market_json=None, as_of=None)"
)]
pub(super) fn build_cashflow_schedule_json(
    py: Python<'_>,
    spec_json: &str,
    market_json: Option<&str>,
    as_of: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let as_of = as_of
        .map(crate::bindings::date_utils::py_to_date)
        .transpose()?;
    py.detach(|| {
        finstack_quant_cashflows::build_cashflow_schedule_json(spec_json, market_json, as_of)
            .map_err(crate::errors::core_to_py)
    })
}

#[doc = pydoc!("finstack_quant.cashflows._schedule_json", "build_cashflow_schedule")]
#[pyfunction]
#[pyo3(
    signature = (spec, market=None, as_of=None),
    text_signature = "(spec, market=None, as_of=None)"
)]
pub(super) fn build_cashflow_schedule(
    py: Python<'_>,
    spec: &Bound<'_, PyAny>,
    market: Option<PyRef<'_, PyMarketContext>>,
    as_of: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyCashFlowSchedule> {
    let spec: CashflowScheduleBuildSpec =
        crate::bindings::module_utils::py_to_serde(py, spec, "CashflowScheduleBuildSpec")?;
    let market = market.map(|market| market.inner.clone());
    let as_of = as_of
        .map(crate::bindings::date_utils::py_to_date)
        .transpose()?;
    py.detach(move || spec.build_as_of(market.as_ref(), as_of))
        .map(PyCashFlowSchedule::from_inner)
        .map_err(crate::errors::core_to_py)
}

#[doc = pydoc!("finstack_quant.cashflows._schedule_json", "validate_cashflow_schedule_json")]
#[pyfunction]
#[pyo3(text_signature = "(schedule_json)")]
pub(super) fn validate_cashflow_schedule_json(
    py: Python<'_>,
    schedule_json: &str,
) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_cashflows::validate_cashflow_schedule_json(schedule_json)
            .map_err(crate::errors::core_to_py)
    })
}

#[doc = pydoc!("finstack_quant.cashflows._schedule_json", "dated_flows_json")]
#[pyfunction]
#[pyo3(text_signature = "(schedule_json)")]
pub(super) fn dated_flows_json(py: Python<'_>, schedule_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_cashflows::dated_flows_json(schedule_json).map_err(crate::errors::core_to_py)
    })
}

#[doc = pydoc!("finstack_quant.cashflows._schedule_json", "dated_flows")]
#[pyfunction]
#[pyo3(text_signature = "(schedule)")]
pub(super) fn dated_flows<'py>(
    py: Python<'py>,
    schedule: PyRef<'py, PyCashFlowSchedule>,
) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
    super::builder::specs::date_money_to_py(
        py,
        &schedule
            .inner
            .dated_flows()
            .map_err(crate::errors::core_to_py)?,
    )
}

#[doc = pydoc!("finstack_quant.cashflows._schedule_json", "ScheduleBuildOpts")]
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
    #[doc = pydoc!("finstack_quant.cashflows._schedule_json", "ScheduleBuildOpts.__init__")]
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

py_getters! { PyScheduleBuildOpts |s| {
    #[doc = pydoc!("finstack_quant.cashflows._schedule_json", "ScheduleBuildOpts.notional_hint")]
    notional_hint -> Option<PyMoney> = s.inner.notional_hint.map(PyMoney::from_inner);

    #[doc = pydoc!("finstack_quant.cashflows._schedule_json", "ScheduleBuildOpts.meta")]
    meta -> PyCashFlowMeta = PyCashFlowMeta {
        inner: s.inner.meta.clone(),
    };
}}

#[doc = pydoc!("finstack_quant.cashflows._schedule_json", "schedule_from_dated_flows")]
#[pyfunction]
#[pyo3(
    signature = (flows, kind, day_count, opts=None),
    text_signature = "(flows, kind, day_count, opts=None)"
)]
pub(super) fn schedule_from_dated_flows(
    flows: Vec<(Bound<'_, PyAny>, PyMoney)>,
    kind: &Bound<'_, PyAny>,
    day_count: PyRef<'_, PyDayCount>,
    opts: Option<PyRef<'_, PyScheduleBuildOpts>>,
) -> PyResult<PyCashFlowSchedule> {
    let flows = aggregation::extract_dated_flows(flows)?;
    let kind = extract_cf_kind(kind)?;
    let opts = opts.map_or_else(ScheduleBuildOpts::default, |o| o.inner.clone());
    Ok(PyCashFlowSchedule::from_inner(
        finstack_quant_cashflows::schedule_from_dated_flows(flows, kind, day_count.inner, opts)
            .map_err(crate::errors::core_to_py)?,
    ))
}

#[doc = pydoc!("finstack_quant.cashflows._schedule_json", "schedule_from_classified_flows")]
#[pyfunction]
#[pyo3(
    signature = (flows, day_count, opts=None),
    text_signature = "(flows, day_count, opts=None)"
)]
pub(super) fn schedule_from_classified_flows(
    flows: Vec<PyRef<'_, PyCashFlow>>,
    day_count: PyRef<'_, PyDayCount>,
    opts: Option<PyRef<'_, PyScheduleBuildOpts>>,
) -> PyResult<PyCashFlowSchedule> {
    let flows = flows.iter().map(|f| f.inner.clone()).collect();
    let opts = opts.map_or_else(ScheduleBuildOpts::default, |o| o.inner.clone());
    Ok(PyCashFlowSchedule::from_inner(
        finstack_quant_cashflows::schedule_from_classified_flows(flows, day_count.inner, opts),
    ))
}

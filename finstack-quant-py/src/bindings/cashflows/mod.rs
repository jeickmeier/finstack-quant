//! Python bindings for the `finstack-quant-cashflows` crate.

pub(crate) mod accrual;
pub(crate) mod aggregation;
pub(crate) mod builder;
pub(crate) mod fixings;
mod schema;

use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::builder::CashFlowMeta;
use finstack_quant_cashflows::{CashflowScheduleBuildSpec, ScheduleBuildOpts};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::bindings::core::cashflow::{extract_cf_kind, PyCashFlow};
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::money::PyMoney;
use builder::schedule::{PyCashFlowMeta, PyCashFlowSchedule};

#[doc = pydoc!("finstack_quant.cashflows", "build_cashflow_schedule_json")]
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

#[doc = pydoc!("finstack_quant.cashflows", "build_cashflow_schedule")]
#[pyfunction]
#[pyo3(signature = (spec, market=None), text_signature = "(spec, market=None)")]
fn build_cashflow_schedule(
    py: Python<'_>,
    spec: &Bound<'_, PyAny>,
    market: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyCashFlowSchedule> {
    let spec: CashflowScheduleBuildSpec =
        crate::bindings::module_utils::spec_from_py(py, spec, "CashflowScheduleBuildSpec")?;
    let market = crate::bindings::extract::extract_market_opt(py, market)?;
    py.detach(move || spec.build(market.as_ref()))
        .map(PyCashFlowSchedule::from_inner)
        .map_err(crate::errors::core_to_py)
}

#[doc = pydoc!("finstack_quant.cashflows", "validate_cashflow_schedule_json")]
#[pyfunction]
#[pyo3(text_signature = "(schedule_json)")]
fn validate_cashflow_schedule_json(py: Python<'_>, schedule_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_cashflows::validate_cashflow_schedule_json(schedule_json)
            .map_err(crate::errors::core_to_py)
    })
}

#[doc = pydoc!("finstack_quant.cashflows", "dated_flows_json")]
#[pyfunction]
#[pyo3(text_signature = "(schedule_json)")]
fn dated_flows_json(py: Python<'_>, schedule_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_cashflows::dated_flows_json(schedule_json).map_err(crate::errors::core_to_py)
    })
}

#[doc = pydoc!("finstack_quant.cashflows", "dated_flows")]
#[pyfunction]
#[pyo3(text_signature = "(schedule)")]
fn dated_flows<'py>(
    py: Python<'py>,
    schedule: PyRef<'py, PyCashFlowSchedule>,
) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
    builder::specs::date_money_to_py(
        py,
        &schedule
            .inner
            .dated_flows()
            .map_err(crate::errors::core_to_py)?,
    )
}

#[doc = pydoc!("finstack_quant.cashflows", "ScheduleBuildOpts")]
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
    #[doc = pydoc!("finstack_quant.cashflows", "ScheduleBuildOpts.__init__")]
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

    #[doc = pydoc!("finstack_quant.cashflows", "ScheduleBuildOpts.notional_hint")]
    #[getter]
    fn notional_hint(&self) -> Option<PyMoney> {
        self.inner.notional_hint.map(PyMoney::from_inner)
    }

    #[doc = pydoc!("finstack_quant.cashflows", "ScheduleBuildOpts.meta")]
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

#[doc = pydoc!("finstack_quant.cashflows", "schedule_from_dated_flows")]
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
        finstack_quant_cashflows::schedule_from_dated_flows(flows, kind, day_count.inner, opts)
            .map_err(crate::errors::core_to_py)?,
    ))
}

#[doc = pydoc!("finstack_quant.cashflows", "schedule_from_classified_flows")]
#[pyfunction]
#[pyo3(
    signature = (flows, day_count, opts=None),
    text_signature = "(flows, day_count, opts=None)"
)]
fn schedule_from_classified_flows(
    flows: Vec<PyRef<'_, PyCashFlow>>,
    day_count: PyRef<'_, PyDayCount>,
    opts: Option<PyRef<'_, PyScheduleBuildOpts>>,
) -> PyResult<PyCashFlowSchedule> {
    let flows = flows.iter().map(|f| f.inner.clone()).collect();
    let opts = opts.map_or_else(ScheduleBuildOpts::default, |o| o.inner.clone());
    finstack_quant_cashflows::schedule_from_classified_flows(flows, day_count.inner, opts)
        .map(PyCashFlowSchedule::from_inner)
        .map_err(crate::errors::core_to_py)
}

/// Register the `finstack_quant.cashflows` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "cashflows")?;
    m.setattr(
        "__doc__",
        "Cashflow schedule construction (typed and JSON), validation, and dated-flow extraction.",
    )?;

    builder::register(py, &m)?;
    accrual::register(py, &m)?;
    aggregation::register(py, &m)?;
    schema::register(py, &m)?;
    fixings::register(py, &m)?;

    m.add_class::<PyScheduleBuildOpts>()?;
    m.add_function(wrap_pyfunction!(build_cashflow_schedule, &m)?)?;
    m.add_function(wrap_pyfunction!(build_cashflow_schedule_json, &m)?)?;
    m.add_function(wrap_pyfunction!(dated_flows, &m)?)?;
    m.add_function(wrap_pyfunction!(dated_flows_json, &m)?)?;
    m.add_function(wrap_pyfunction!(schedule_from_classified_flows, &m)?)?;
    m.add_function(wrap_pyfunction!(schedule_from_dated_flows, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_cashflow_schedule_json, &m)?)?;

    let all = PyList::new(
        py,
        [
            "ScheduleBuildOpts",
            "accrual",
            "aggregation",
            "build_cashflow_schedule",
            "build_cashflow_schedule_json",
            "builder",
            "dated_flows",
            "dated_flows_json",
            "fixings",
            "schedule_from_classified_flows",
            "schedule_from_dated_flows",
            "schema",
            "validate_cashflow_schedule_json",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    Ok(())
}

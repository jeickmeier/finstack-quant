//! Python bindings for `finstack_quant_cashflows::aggregation`.

use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::aggregation::{
    aggregate_by_period, aggregate_cashflows_checked as rust_aggregate_cashflows_checked,
    calendar_year_ladder as rust_calendar_year_ladder, PeriodAggregation,
};
use finstack_quant_core::{dates::Date, money::Money};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};

use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::dates::periods::PyPeriod;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::macros::wire_methods;
use crate::errors::core_to_py;

/// Convert Python ``[(date, Money), ...]`` into Rust dated flows.
pub(crate) fn extract_dated_flows(
    flows: Vec<(Bound<'_, PyAny>, PyMoney)>,
) -> PyResult<Vec<(Date, Money)>> {
    flows
        .iter()
        .map(|(d, m)| Ok((py_to_date(d)?, m.inner)))
        .collect()
}

#[doc = pydoc!("finstack_quant.cashflows.aggregation", "PeriodAggregation")]
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

wire_methods!(
    PyPeriodAggregation,
    PeriodAggregation,
    "PeriodAggregation",
    module = "finstack_quant.cashflows.aggregation"
);

#[pymethods]
impl PyPeriodAggregation {
    #[doc = pydoc!("finstack_quant.cashflows.aggregation", "PeriodAggregation.periods")]
    #[getter]
    fn periods(&self) -> Vec<String> {
        self.inner.keys().map(ToString::to_string).collect()
    }

    #[doc = pydoc!("finstack_quant.cashflows.aggregation", "PeriodAggregation.get_period")]
    #[pyo3(text_signature = "(self, period)")]
    fn get_period<'py>(
        &self,
        py: Python<'py>,
        period: &str,
    ) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.inner
            .get_period(period)
            .map(|per_currency| Self::period_dict(py, per_currency))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.cashflows.aggregation", "PeriodAggregation.get_amount")]
    #[pyo3(text_signature = "(self, period, currency)")]
    fn get_amount(&self, period: &str, currency: &Bound<'_, PyAny>) -> PyResult<Option<PyMoney>> {
        let currency = extract_currency(currency)?;
        Ok(self
            .inner
            .get_amount(period, currency)
            .map(PyMoney::from_inner))
    }

    #[doc = pydoc!("finstack_quant.cashflows.aggregation", "PeriodAggregation.to_dict")]
    #[pyo3(text_signature = "(self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (period_id, per_currency) in self.inner.iter() {
            out.set_item(period_id.to_string(), Self::period_dict(py, per_currency)?)?;
        }
        Ok(out)
    }

    #[doc = pydoc!("finstack_quant.cashflows.aggregation", "PeriodAggregation.__getitem__")]
    fn __getitem__<'py>(&self, py: Python<'py>, period: &str) -> PyResult<Bound<'py, PyDict>> {
        self.inner
            .get_period(period)
            .map(|per_currency| Self::period_dict(py, per_currency))
            .unwrap_or_else(|| {
                Err(pyo3::exceptions::PyKeyError::new_err(format!(
                    "no flows in period '{period}'"
                )))
            })
    }

    #[doc = pydoc!("finstack_quant.cashflows.aggregation", "PeriodAggregation.__contains__")]
    fn __contains__(&self, period: &str) -> bool {
        self.inner.get_period(period).is_some()
    }

    #[doc = pydoc!("finstack_quant.cashflows.aggregation", "PeriodAggregation.__len__")]
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    #[doc = pydoc!("finstack_quant.cashflows.aggregation", "PeriodAggregation.to_dataframe")]
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

#[doc = pydoc!("finstack_quant.cashflows.aggregation", "aggregate_by_period")]
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

#[doc = pydoc!("finstack_quant.cashflows.aggregation", "aggregate_cashflows_checked")]
#[pyfunction]
#[pyo3(text_signature = "(flows, target)")]
fn aggregate_cashflows_checked(
    py: Python<'_>,
    flows: Vec<(Bound<'_, PyAny>, PyMoney)>,
    target: &Bound<'_, PyAny>,
) -> PyResult<PyMoney> {
    let flows = extract_dated_flows(flows)?;
    let target = extract_currency(target)?;
    py.detach(move || rust_aggregate_cashflows_checked(&flows, target))
        .map(PyMoney::from_inner)
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.cashflows.aggregation", "calendar_year_ladder")]
#[pyfunction]
#[pyo3(text_signature = "(dates, kinds, amounts, pvs)")]
fn calendar_year_ladder(
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
        rust_calendar_year_ladder(&dates, &kind_refs, &amounts, &pvs).map(|rows| {
            rows.into_iter()
                .map(|row| (row.year, row.non_principal, row.principal, row.pv))
                .collect()
        })
    })
    .map_err(core_to_py)
}

/// Register the `finstack_quant.cashflows.aggregation` submodule.
pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = crate::bindings::module_utils::new_submodule(parent, "aggregation")?;
    module.setattr(
        "__doc__",
        pydoc!("finstack_quant.cashflows.aggregation", "__module__"),
    )?;
    module.add_class::<PyPeriodAggregation>()?;
    module.add_function(wrap_pyfunction!(py_aggregate_by_period, &module)?)?;
    module.add_function(wrap_pyfunction!(aggregate_cashflows_checked, &module)?)?;
    module.add_function(wrap_pyfunction!(calendar_year_ladder, &module)?)?;

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

    crate::bindings::module_utils::attach_submodule(
        parent,
        &module,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;
    Ok(())
}

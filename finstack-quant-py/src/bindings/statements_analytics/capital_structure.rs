//! Debt stack by seniority (`capital_structure_summary`) over an evaluated
//! capital structure. The request is the Rust serde form, a dict or JSON text,
//! the same document WASM takes.

use super::extract_serde_any;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::extract::{extract_model_ref, extract_results_ref};
use crate::bindings::macros::wire_methods;
use crate::bindings::pandas_utils::serde_rows_to_dataframe_with_schema;
use crate::bindings::pydoc::pydoc;
use crate::bindings::valuations::convert::enum_to_py_string;
use crate::errors::statements_to_py;
use finstack_quant_statements_analytics::analysis::{
    self, CapitalStructureSummary, CapitalStructureSummaryConfig, CapitalStructureTranche,
    MaturityWallBucket,
};
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureTranche")]
#[pyclass(
    module = "finstack_quant.statements_analytics",
    name = "CapitalStructureTranche",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCapitalStructureTranche {
    /// Inner canonical Rust value.
    pub(crate) inner: CapitalStructureTranche,
}

wire_methods!(
    PyCapitalStructureTranche,
    CapitalStructureTranche,
    "CapitalStructureTranche",
    module = "finstack_quant.statements_analytics._capital_structure",
    to_dict
);

#[pymethods]
impl PyCapitalStructureTranche {
    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureTranche.kind")]
    #[getter]
    fn kind(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.kind)
    }

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureTranche.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CapitalStructureTranche(id={:?}, rank={}, balance={}, leverage={:?})",
            self.inner.id, self.inner.rank, self.inner.balance, self.inner.leverage
        )
    }
}

crate::bindings::macros::py_getters!(PyCapitalStructureTranche |s| {
    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureTranche.id")]
    id -> String = s.inner.id.clone();

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureTranche.rank")]
    rank -> u32 = s.inner.rank;

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureTranche.payment_class")]
    payment_class -> Option<String> = s.inner.payment_class.clone();

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureTranche.balance")]
    balance -> PyMoney = PyMoney::from_inner(s.inner.balance);

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureTranche.share_of_debt")]
    share_of_debt -> Option<f64> = s.inner.share_of_debt;

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureTranche.cumulative_debt")]
    cumulative_debt -> PyMoney = PyMoney::from_inner(s.inner.cumulative_debt);

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureTranche.leverage")]
    leverage -> Option<f64> = s.inner.leverage;

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureTranche.interest_coverage")]
    interest_coverage -> Option<f64> = s.inner.interest_coverage;

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureTranche.ltm_interest")]
    ltm_interest -> Option<PyMoney> = s.inner.ltm_interest.map(PyMoney::from_inner);
});

#[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "MaturityWallBucket")]
#[pyclass(
    module = "finstack_quant.statements_analytics",
    name = "MaturityWallBucket",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyMaturityWallBucket {
    /// Inner canonical Rust value.
    pub(crate) inner: MaturityWallBucket,
}

wire_methods!(
    PyMaturityWallBucket,
    MaturityWallBucket,
    "MaturityWallBucket",
    module = "finstack_quant.statements_analytics._capital_structure",
    to_dict
);

#[pymethods]
impl PyMaturityWallBucket {
    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "MaturityWallBucket(year={}, amount={})",
            self.inner.year, self.inner.amount
        )
    }
}

crate::bindings::macros::py_getters!(PyMaturityWallBucket |s| {
    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "MaturityWallBucket.year")]
    year -> i32 = s.inner.year;

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "MaturityWallBucket.amount")]
    amount -> PyMoney = PyMoney::from_inner(s.inner.amount);

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "MaturityWallBucket.share_of_debt")]
    share_of_debt -> Option<f64> = s.inner.share_of_debt;
});

#[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary")]
#[pyclass(
    module = "finstack_quant.statements_analytics",
    name = "CapitalStructureSummary",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCapitalStructureSummary {
    /// Inner canonical Rust value.
    pub(crate) inner: CapitalStructureSummary,
}

wire_methods!(
    PyCapitalStructureSummary,
    CapitalStructureSummary,
    "CapitalStructureSummary",
    module = "finstack_quant.statements_analytics._capital_structure",
    to_dict
);

#[pymethods]
impl PyCapitalStructureSummary {
    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .tranches
            .iter()
            .map(|tranche| {
                serde_json::json!({
                    "id": tranche.id,
                    "kind": tranche.kind,
                    "rank": tranche.rank,
                    "payment_class": tranche.payment_class,
                    "balance": tranche.balance.amount(),
                    "share_of_debt": tranche.share_of_debt,
                    "maturity": tranche.maturity.to_string(),
                    "cumulative_debt": tranche.cumulative_debt.amount(),
                    "leverage": tranche.leverage,
                    "interest_coverage": tranche.interest_coverage,
                    "ltm_interest": tranche.ltm_interest.map(|interest| interest.amount()),
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(
            py,
            &rows,
            &[
                ("id", "str"),
                ("kind", "str"),
                ("rank", "int64"),
                ("payment_class", "str"),
                ("balance", "float64"),
                ("share_of_debt", "float64"),
                ("maturity", "str"),
                ("cumulative_debt", "float64"),
                ("leverage", "float64"),
                ("interest_coverage", "float64"),
                ("ltm_interest", "float64"),
            ],
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CapitalStructureSummary(as_of={}, total_debt={}, tranches={})",
            self.inner.as_of,
            self.inner.total_debt,
            self.inner.tranches.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyCapitalStructureSummary |s| {
    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary.as_of")]
    as_of -> String = s.inner.as_of.to_string();

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary.currency")]
    currency -> String = s.inner.currency.to_string();

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary.total_debt")]
    total_debt -> PyMoney = PyMoney::from_inner(s.inner.total_debt);

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary.cash")]
    cash -> Option<PyMoney> = s.inner.cash.map(PyMoney::from_inner);

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary.net_debt")]
    net_debt -> Option<PyMoney> = s.inner.net_debt.map(PyMoney::from_inner);

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary.ltm_ebitda")]
    ltm_ebitda -> Option<PyMoney> = s.inner.ltm_ebitda.map(PyMoney::from_inner);

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary.ltm_periods_required")]
    ltm_periods_required -> u32 = s.inner.ltm_periods_required;

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary.ltm_periods_found")]
    ltm_periods_found -> u32 = s.inner.ltm_periods_found;

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary.total_leverage")]
    total_leverage -> Option<f64> = s.inner.total_leverage;

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary.net_leverage")]
    net_leverage -> Option<f64> = s.inner.net_leverage;

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary.total_coverage")]
    total_coverage -> Option<f64> = s.inner.total_coverage;

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary.tranches")]
    tranches -> Vec<PyCapitalStructureTranche> = s
        .inner
        .tranches
        .iter()
        .map(|inner| PyCapitalStructureTranche { inner: inner.clone() })
        .collect();

    #[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "CapitalStructureSummary.maturity_wall")]
    maturity_wall -> Vec<PyMaturityWallBucket> = s
        .inner
        .maturity_wall
        .iter()
        .map(|inner| PyMaturityWallBucket { inner: inner.clone() })
        .collect();
});

#[doc = pydoc!("finstack_quant.statements_analytics._capital_structure", "capital_structure_summary")]
#[pyfunction]
#[pyo3(text_signature = "(results, model, config)")]
fn capital_structure_summary<'py>(
    py: Python<'py>,
    results: &Bound<'py, PyAny>,
    model: &Bound<'py, PyAny>,
    config: &Bound<'py, PyAny>,
) -> PyResult<PyCapitalStructureSummary> {
    let results = extract_results_ref(results)?;
    let model = extract_model_ref(model)?;
    let config: CapitalStructureSummaryConfig = extract_serde_any(py, config, "config")?;
    let inner =
        analysis::capital_structure_summary(&results, &model, &config).map_err(statements_to_py)?;
    Ok(PyCapitalStructureSummary { inner })
}

/// Register the summary on the `statements_analytics` submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCapitalStructureTranche>()?;
    m.add_class::<PyMaturityWallBucket>()?;
    m.add_class::<PyCapitalStructureSummary>()?;
    m.add_function(pyo3::wrap_pyfunction!(capital_structure_summary, m)?)?;
    Ok(())
}

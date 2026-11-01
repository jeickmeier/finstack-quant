//! Typed collateral-pool report (`PoolReport`, `PoolConcentrationEntry`).

use pyo3::prelude::*;

use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    PoolConcentrationEntry, PoolReport,
};

use super::pool_stats::PyPoolStats;
use crate::bindings::core::money::PyMoney;
use crate::bindings::pandas_utils::serde_rows_to_dataframe_with_schema;
use crate::bindings::valuations::convert::money_to_py;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_report", "PoolConcentrationEntry")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "PoolConcentrationEntry",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPoolConcentrationEntry {
    /// Inner canonical Rust concentration row.
    pub(crate) inner: PoolConcentrationEntry,
}

crate::bindings::macros::wire_methods!(
    PyPoolConcentrationEntry,
    PoolConcentrationEntry,
    "PoolConcentrationEntry",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.pool_report",
    to_dict
);

#[pymethods]
impl PyPoolConcentrationEntry {
    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "PoolConcentrationEntry(key={:?}, balance={}, balance_pct={}, asset_count={})",
            self.inner.key, self.inner.balance, self.inner.balance_pct, self.inner.asset_count
        )
    }
}

crate::bindings::macros::py_getters!(PyPoolConcentrationEntry |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_report", "PoolConcentrationEntry.key")]
    key -> Option<String> = s.inner.key.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_report", "PoolConcentrationEntry.balance")]
    balance -> PyMoney = money_to_py(s.inner.balance);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_report", "PoolConcentrationEntry.balance_pct")]
    balance_pct -> f64 = s.inner.balance_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_report", "PoolConcentrationEntry.asset_count")]
    asset_count -> usize = s.inner.asset_count;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_report", "PoolReport")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "PoolReport",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPoolReport {
    /// Inner canonical Rust pool report.
    pub(crate) inner: PoolReport,
}

crate::bindings::macros::wire_methods!(
    PyPoolReport,
    PoolReport,
    "PoolReport",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.pool_report",
    to_dict
);

/// Wrap one concentration table for a getter.
fn entries(rows: &[PoolConcentrationEntry]) -> Vec<PyPoolConcentrationEntry> {
    rows.iter()
        .map(|inner| PyPoolConcentrationEntry {
            inner: inner.clone(),
        })
        .collect()
}

#[pymethods]
impl PyPoolReport {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_report", "PoolReport.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = [
            ("obligor", &self.inner.by_obligor),
            ("industry", &self.inner.by_industry),
            ("rating", &self.inner.by_rating),
        ]
        .into_iter()
        .flat_map(|(dimension, table)| {
            table.iter().map(move |row| {
                serde_json::json!({
                    "dimension": dimension,
                    "key": row.key,
                    "balance": row.balance.amount(),
                    "currency": row.balance.currency().to_string(),
                    "balance_pct": row.balance_pct,
                    "asset_count": row.asset_count,
                })
            })
        })
        .collect();
        serde_rows_to_dataframe_with_schema(
            py,
            &rows,
            &[
                ("dimension", "str"),
                ("key", "str"),
                ("balance", "float64"),
                ("currency", "str"),
                ("balance_pct", "float64"),
                ("asset_count", "int64"),
            ],
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "PoolReport(total_balance={}, warf={:?}, obligors={}, industries={}, ratings={})",
            self.inner.total_balance,
            self.inner.warf,
            self.inner.by_obligor.len(),
            self.inner.by_industry.len(),
            self.inner.by_rating.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyPoolReport |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_report", "PoolReport.total_balance")]
    total_balance -> PyMoney = money_to_py(s.inner.total_balance);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_report", "PoolReport.stats")]
    stats -> PyPoolStats = PyPoolStats { inner: s.inner.stats.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_report", "PoolReport.warf")]
    warf -> Option<f64> = s.inner.warf;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_report", "PoolReport.by_obligor")]
    by_obligor -> Vec<PyPoolConcentrationEntry> = entries(&s.inner.by_obligor);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_report", "PoolReport.by_industry")]
    by_industry -> Vec<PyPoolConcentrationEntry> = entries(&s.inner.by_industry);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_report", "PoolReport.by_rating")]
    by_rating -> Vec<PyPoolConcentrationEntry> = entries(&s.inner.by_rating);
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_report", "pool_report")]
#[pyfunction]
#[pyo3(text_signature = "(pool, as_of)")]
pub(crate) fn pool_report(
    pool: PyRef<'_, super::PyAssetPool>,
    as_of: &Bound<'_, PyAny>,
) -> PyResult<PyPoolReport> {
    let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
    finstack_quant_valuations::instruments::fixed_income::structured_credit::pool_report(
        &pool.inner,
        as_of,
    )
    .map(|inner| PyPoolReport { inner })
    .map_err(crate::errors::core_to_py)
}

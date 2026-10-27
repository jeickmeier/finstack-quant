//! Typed collateral-pool statistics (`PoolStats`).

use pyo3::prelude::*;

use finstack_quant_valuations::instruments::fixed_income::structured_credit::PoolStats;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "PoolStats")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "PoolStats",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPoolStats {
    /// Inner canonical Rust pool statistics.
    pub(crate) inner: PoolStats,
}

crate::bindings::macros::wire_methods!(
    PyPoolStats,
    PoolStats,
    "PoolStats",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.pool_stats",
    to_dict
);

/// Column order of [`PyPoolStats::to_dataframe`] (the serde field order).
const POOL_STATS_COLUMNS: &[&str] = &[
    "wac",
    "weighted_avg_spread_bp",
    "weighted_avg_maturity",
    "diversity_score",
    "num_obligors",
    "num_industries",
    "defaulted_balance_pct",
    "undrawn_commitment",
];

#[pymethods]
impl PyPoolStats {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "PoolStats.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            POOL_STATS_COLUMNS,
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "PoolStats(wac={}, weighted_avg_spread_bp={}, weighted_avg_maturity={}, diversity_score={})",
            self.inner.wac,
            self.inner.weighted_avg_spread_bp,
            self.inner.weighted_avg_maturity,
            self.inner.diversity_score
        )
    }
}

crate::bindings::macros::py_getters!(PyPoolStats |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "PoolStats.wac")]
    wac -> f64 = s.inner.wac;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "PoolStats.weighted_avg_spread_bp")]
    weighted_avg_spread_bp -> f64 = s.inner.weighted_avg_spread_bp;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "PoolStats.weighted_avg_maturity")]
    weighted_avg_maturity -> f64 = s.inner.weighted_avg_maturity;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "PoolStats.diversity_score")]
    diversity_score -> f64 = s.inner.diversity_score;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "PoolStats.num_obligors")]
    num_obligors -> usize = s.inner.num_obligors;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "PoolStats.num_industries")]
    num_industries -> usize = s.inner.num_industries;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "PoolStats.defaulted_balance_pct")]
    defaulted_balance_pct -> f64 = s.inner.defaulted_balance_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "PoolStats.undrawn_commitment")]
    undrawn_commitment -> f64 = s.inner.undrawn_commitment;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "calculate_pool_stats")]
#[pyfunction]
#[pyo3(text_signature = "(pool, as_of)")]
pub(crate) fn calculate_pool_stats(
    pool: PyRef<'_, super::PyAssetPool>,
    as_of: &Bound<'_, PyAny>,
) -> PyResult<PyPoolStats> {
    let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
    finstack_quant_valuations::instruments::fixed_income::structured_credit::calculate_pool_stats(
        &pool.inner,
        as_of,
    )
    .map(|inner| PyPoolStats { inner })
    .map_err(crate::errors::core_to_py)
}

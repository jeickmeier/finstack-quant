//! Typed collateral-pool statistics (`PoolStats`).

use pyo3::prelude::*;

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::value_error;
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

#[pymethods]
impl PyPoolStats {
    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "PoolStats(wac={:?}, weighted_avg_spread_bp={:?}, weighted_avg_maturity={:?}, diversity_score={:?})",
            self.inner.wac,
            self.inner.weighted_avg_spread_bp,
            self.inner.weighted_avg_maturity,
            self.inner.diversity_score
        )
    }
}

impl PyPoolStats {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let num_obligors = i64::try_from(self.inner.num_obligors)
            .map_err(|_| value_error("num_obligors overflows i64"))?;
        let num_industries = i64::try_from(self.inner.num_industries)
            .map_err(|_| value_error("num_industries overflows i64"))?;
        TableBuilder::new()
            .nullable_f64("wac", vec![self.inner.wac])?
            .nullable_f64(
                "weighted_avg_spread_bp",
                vec![self.inner.weighted_avg_spread_bp],
            )?
            .nullable_f64(
                "weighted_avg_maturity",
                vec![self.inner.weighted_avg_maturity],
            )?
            .nullable_f64("diversity_score", vec![self.inner.diversity_score])?
            .i64("num_obligors", vec![num_obligors])?
            .i64("num_industries", vec![num_industries])?
            .f64(
                "defaulted_balance_pct",
                vec![self.inner.defaulted_balance_pct],
            )?
            .f64("undrawn_commitment", vec![self.inner.undrawn_commitment])?
            .build()
    }
}

crate::bindings::macros::tabular_methods!(
    PyPoolStats,
    "PoolStats",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.pool_stats"
);

crate::bindings::macros::py_getters!(PyPoolStats |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "PoolStats.wac")]
    wac -> Option<f64> = s.inner.wac;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "PoolStats.weighted_avg_spread_bp")]
    weighted_avg_spread_bp -> Option<f64> = s.inner.weighted_avg_spread_bp;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "PoolStats.weighted_avg_maturity")]
    weighted_avg_maturity -> Option<f64> = s.inner.weighted_avg_maturity;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.pool_stats", "PoolStats.diversity_score")]
    diversity_score -> Option<f64> = s.inner.diversity_score;

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

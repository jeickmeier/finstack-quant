//! Typed collateral-pool statistics (`PoolStats`).

use pyo3::prelude::*;

use finstack_quant_valuations::instruments::fixed_income::structured_credit::PoolStats;

/// Collateral-pool statistics at one date (the return value of
/// :func:`calculate_pool_stats`): weighted-average coupon, spread and
/// maturity, Moody's diversity score, obligor and industry counts, the
/// defaulted share and the undrawn commitment.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import PoolStats
/// >>> stats = PoolStats.from_json(
/// ...     '{"wac": 0.07, "weighted_avg_spread_bp": 0.0, "weighted_avg_maturity": 10.0,'
/// ...     ' "diversity_score": 1.0, "num_obligors": 0, "num_industries": 0,'
/// ...     ' "defaulted_balance_pct": 0.0, "undrawn_commitment": 0.0}'
/// ... )
/// >>> stats.wac, list(stats.to_dataframe().columns)[:2]
/// (0.07, ['wac', 'weighted_avg_spread_bp'])
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

sc_wire_methods!(PyPoolStats, PoolStats, "PoolStats");

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
    /// Weighted-average coupon of the performing fixed-rate collateral, as
    /// an annual decimal (``0.07`` = 7%).
    #[getter]
    fn wac(&self) -> f64 {
        self.inner.wac
    }

    /// Weighted-average spread, in basis points, of the performing
    /// collateral that carries an explicit spread.
    #[getter]
    fn weighted_avg_spread_bp(&self) -> f64 {
        self.inner.weighted_avg_spread_bp
    }

    /// Balance-weighted average remaining maturity in years from the
    /// statistics date.
    #[getter]
    fn weighted_avg_maturity(&self) -> f64 {
        self.inner.weighted_avg_maturity
    }

    /// Moody's diversity score of the pool.
    #[getter]
    fn diversity_score(&self) -> f64 {
        self.inner.diversity_score
    }

    /// Number of distinct obligors.
    #[getter]
    fn num_obligors(&self) -> usize {
        self.inner.num_obligors
    }

    /// Number of distinct industries.
    #[getter]
    fn num_industries(&self) -> usize {
        self.inner.num_industries
    }

    /// Defaulted balance in percent points of the current total pool
    /// balance (``10.0`` = 10%).
    #[getter]
    fn defaulted_balance_pct(&self) -> f64 {
        self.inner.defaulted_balance_pct
    }

    /// Undrawn commitment across performing revolving and delayed-draw
    /// collateral, in pool-currency units.
    #[getter]
    fn undrawn_commitment(&self) -> f64 {
        self.inner.undrawn_commitment
    }

    /// The statistics as a one-row pandas ``DataFrame``.
    ///
    /// Columns: ``wac``, ``weighted_avg_spread_bp``,
    /// ``weighted_avg_maturity``, ``diversity_score``, ``num_obligors``,
    /// ``num_industries``, ``defaulted_balance_pct``, ``undrawn_commitment``.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     One row holding every statistic.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the statistics cannot be serialized.
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

/// Collateral-pool statistics at a date (mirrors Rust ``calculate_pool_stats``).
///
/// Parameters
/// ----------
/// pool : AssetPool
///     Collateral pool to summarise.
/// as_of : datetime.date | str
///     Date the remaining maturities are measured from, either a date-like
///     object or an ISO 8601 string.
///
/// Returns
/// -------
/// PoolStats
///     Weighted-average coupon, spread and maturity, diversity score,
///     obligor and industry counts, defaulted share and undrawn commitment.
///
/// Raises
/// ------
/// ValueError
///     If ``as_of`` is not a date, or the pool's balances are inconsistent
///     (mixed currencies, or both asset rows and representative lines).
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import StructuredCredit, calculate_pool_stats
/// >>> stats = calculate_pool_stats(StructuredCredit.example().pool, "2024-01-01")
/// >>> stats.wac, round(stats.weighted_avg_maturity, 1)
/// (0.07, 10.0)
#[pyfunction]
#[pyo3(text_signature = "(pool, as_of)")]
pub(crate) fn calculate_pool_stats(
    pool: PyRef<'_, super::PyAssetPool>,
    as_of: &Bound<'_, PyAny>,
) -> PyResult<PyPoolStats> {
    let as_of = crate::bindings::date_utils::extract_date(as_of)?;
    finstack_quant_valuations::instruments::fixed_income::structured_credit::calculate_pool_stats(
        &pool.inner,
        as_of,
    )
    .map(|inner| PyPoolStats { inner })
    .map_err(crate::errors::core_to_py)
}

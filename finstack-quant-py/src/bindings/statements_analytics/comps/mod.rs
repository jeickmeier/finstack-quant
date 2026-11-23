//! Python bindings for the comparable company analysis module.
//!
//! Exposes typed peer-set construction and cross-sectional peer analytics:
//!
//! - `CompanyMetrics`, `PeerFilter`, `PeerSet` (with `from_dataframe` and
//!   `from_universe`), `ScoringDimension`.
//! - Descriptive peer statistics (`peer_stats` -> `PeerStats`).
//! - Percentile rank and z-score of a subject within a peer distribution.
//! - Single-factor OLS regression (`regression_fair_value` -> `RegressionResult`).
//! - Canonical valuation multiple computation on `CompanyMetrics`.
//! - Multi-dimension composite rich/cheap scoring
//!   (`score_relative_value` -> `RelativeValueResult`).

mod peers;
mod scoring;

pub use peers::{PyCompanyMetrics, PyPeerFilter, PyPeerSet};
pub(super) use peers::{extract_company_metrics, period_basis_label};
use pyo3::prelude::*;
pub use scoring::{
    PyDimensionScore, PyPeerStats, PyRegressionResult, PyRelativeValueResult, PyScoringDimension,
};
use scoring::{
    compute_multiple, peer_stats, percentile_rank, regression_fair_value, score_relative_value,
    z_score,
};

/// Register comps bindings on the analytics submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCompanyMetrics>()?;
    m.add_class::<PyPeerFilter>()?;
    m.add_class::<PyPeerSet>()?;
    m.add_class::<PyScoringDimension>()?;
    m.add_class::<PyPeerStats>()?;
    m.add_class::<PyRegressionResult>()?;
    m.add_class::<PyDimensionScore>()?;
    m.add_class::<PyRelativeValueResult>()?;
    m.add_function(wrap_pyfunction!(percentile_rank, m)?)?;
    m.add_function(wrap_pyfunction!(z_score, m)?)?;
    m.add_function(wrap_pyfunction!(peer_stats, m)?)?;
    m.add_function(wrap_pyfunction!(regression_fair_value, m)?)?;
    m.add_function(wrap_pyfunction!(compute_multiple, m)?)?;
    m.add_function(wrap_pyfunction!(score_relative_value, m)?)?;
    Ok(())
}

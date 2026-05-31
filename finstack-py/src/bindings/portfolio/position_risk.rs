//! Python bindings for position-level VaR/ES decomposition and risk budgeting.
//!
//! Exposes covariance-based and historical Euler allocation engines, plus a
//! risk-budget evaluator, through numpy-friendly function signatures.
//! Inputs are plain `Vec<f64>` / `Vec<Vec<f64>>` so callers can pass numpy
//! arrays directly (PyO3 converts automatically). Results are returned as
//! nested `PyDict` structures rather than opaque `#[pyclass]` wrappers.

use crate::errors::core_to_py;
use finstack_portfolio::factor_model::{
    flatten_square_matrix as core_flatten_square_matrix, DecompositionConfig,
    HistoricalPositionDecomposer, ParametricPositionDecomposer, PositionRiskDecomposition,
    RiskBudget,
};
use finstack_portfolio::types::PositionId;
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Forward to the shared `factor_model::flatten_square_matrix` and remap the
/// `core::Error::Validation` shape into a `PyValueError` so the same matrix
/// validation diagnostics surface from both the Python and WASM bindings.
fn flatten_square_matrix(matrix: Vec<Vec<f64>>, n: usize, label: &str) -> PyResult<Vec<f64>> {
    core_flatten_square_matrix(matrix, n, label)
        .map_err(|e| crate::errors::value_error(e.to_string()))
}

/// Convert `Vec<String>` position ids to the Rust newtype.
fn to_position_ids(ids: Vec<String>) -> Vec<PositionId> {
    ids.into_iter().map(PositionId::new).collect()
}

/// Build the common output dict shared by VaR-focused decomposition functions.
fn var_decomposition_to_dict<'py>(
    py: Python<'py>,
    d: &PositionRiskDecomposition,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    out.set_item("portfolio_var", d.portfolio_var)?;
    out.set_item("portfolio_es", d.portfolio_es)?;
    out.set_item("confidence", d.confidence)?;
    out.set_item("n_positions", d.n_positions)?;
    // euler_residual, marginal_var, marginal_es, incremental_var are Option<f64>
    // and serialize as None/null for engines that cannot compute them
    // (e.g. historical mode for marginals and euler residual).
    out.set_item("euler_residual", d.euler_residual)?;

    let contribs = PyList::empty(py);
    for c in &d.var_contributions {
        let entry = PyDict::new(py);
        entry.set_item("position_id", c.position_id.as_str())?;
        entry.set_item("component_var", c.component_var)?;
        entry.set_item("marginal_var", c.marginal_var)?;
        entry.set_item("pct_contribution", c.relative_var)?;
        entry.set_item("incremental_var", c.incremental_var)?;
        contribs.append(entry)?;
    }
    out.set_item("contributions", contribs)?;

    Ok(out)
}

/// Build the common output dict for ES-focused decomposition functions.
fn es_decomposition_to_dict<'py>(
    py: Python<'py>,
    d: &PositionRiskDecomposition,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    out.set_item("portfolio_var", d.portfolio_var)?;
    out.set_item("portfolio_es", d.portfolio_es)?;
    out.set_item("confidence", d.confidence)?;
    out.set_item("n_positions", d.n_positions)?;

    let contribs = PyList::empty(py);
    for c in &d.es_contributions {
        let entry = PyDict::new(py);
        entry.set_item("position_id", c.position_id.as_str())?;
        entry.set_item("component_es", c.component_es)?;
        // marginal_es is Option<f64>; serializes as None in historical mode.
        entry.set_item("marginal_es", c.marginal_es)?;
        entry.set_item("pct_contribution", c.relative_es)?;
        contribs.append(entry)?;
    }
    out.set_item("contributions", contribs)?;

    Ok(out)
}

// ---------------------------------------------------------------------------
// Parametric (covariance-based) VaR decomposition
// ---------------------------------------------------------------------------

/// Decompose portfolio VaR into position contributions via parametric Euler
/// allocation (multivariate normal assumption).
///
/// Parameters
/// ----------
/// position_ids : list[str]
///     Position identifiers, length ``n``.
/// weights : list[float]
///     Position weights (fractions of portfolio value), length ``n``.
/// covariance : list[list[float]]
///     ``n x n`` symmetric positive semi-definite covariance matrix of
///     position returns (row-major).
/// confidence : float, default ``0.95``
///     Confidence level in ``(0, 1)``.
///
/// Returns
/// -------
/// dict
///     ``{portfolio_var, portfolio_es, confidence, n_positions,
///     euler_residual, contributions: [{position_id, component_var,
///     marginal_var, pct_contribution, incremental_var}, ...]}``.
///     Under normality, ``sum(component_var) == portfolio_var`` exactly.
#[pyfunction]
#[pyo3(signature = (position_ids, weights, covariance, confidence = 0.95))]
fn parametric_var_decomposition<'py>(
    py: Python<'py>,
    position_ids: Vec<String>,
    weights: Vec<f64>,
    covariance: Vec<Vec<f64>>,
    confidence: f64,
) -> PyResult<Bound<'py, PyDict>> {
    let n = weights.len();
    let cov_flat = flatten_square_matrix(covariance, n, "covariance")?;
    let ids = to_position_ids(position_ids);

    let mut config = DecompositionConfig::parametric_95();
    config.confidence = confidence;

    let result = py
        .detach(move || {
            ParametricPositionDecomposer.decompose_positions(&weights, &cov_flat, &ids, &config)
        })
        .map_err(core_to_py)?;

    var_decomposition_to_dict(py, &result)
}

/// Decompose portfolio Expected Shortfall into position contributions via
/// parametric Euler allocation.
///
/// Parameters
/// ----------
/// position_ids : list[str]
/// weights : list[float]
/// covariance : list[list[float]]
/// confidence : float, default ``0.95``
///
/// Returns
/// -------
/// dict
///     ``{portfolio_var, portfolio_es, confidence, n_positions,
///     contributions: [{position_id, component_es, marginal_es,
///     pct_contribution}, ...]}``.
///     Under normality, ``sum(component_es) == portfolio_es`` exactly.
#[pyfunction]
#[pyo3(signature = (position_ids, weights, covariance, confidence = 0.95))]
fn parametric_es_decomposition<'py>(
    py: Python<'py>,
    position_ids: Vec<String>,
    weights: Vec<f64>,
    covariance: Vec<Vec<f64>>,
    confidence: f64,
) -> PyResult<Bound<'py, PyDict>> {
    let n = weights.len();
    let cov_flat = flatten_square_matrix(covariance, n, "covariance")?;
    let ids = to_position_ids(position_ids);

    let mut config = DecompositionConfig::parametric_95();
    config.confidence = confidence;

    let result = py
        .detach(move || {
            ParametricPositionDecomposer.decompose_positions(&weights, &cov_flat, &ids, &config)
        })
        .map_err(core_to_py)?;

    es_decomposition_to_dict(py, &result)
}

// ---------------------------------------------------------------------------
// Historical VaR decomposition
// ---------------------------------------------------------------------------

/// Decompose portfolio VaR and ES from per-position scenario P&Ls via
/// historical simulation.
///
/// Parameters
/// ----------
/// position_ids : list[str]
///     Position identifiers, length ``n``.
/// position_pnls : list[list[float]]
///     Per-position P&L scenarios, shape ``(n, n_scenarios)``. That is,
///     ``position_pnls[i][t]`` is position ``i``'s P&L under scenario ``t``.
/// confidence : float, default ``0.95``
///
/// Returns
/// -------
/// dict
///     Same shape as :func:`parametric_var_decomposition`. The Euler property
///     holds only approximately under historical simulation.
#[pyfunction]
#[pyo3(signature = (position_ids, position_pnls, confidence = 0.95))]
fn historical_var_decomposition<'py>(
    py: Python<'py>,
    position_ids: Vec<String>,
    position_pnls: Vec<Vec<f64>>,
    confidence: f64,
) -> PyResult<Bound<'py, PyDict>> {
    let n = position_ids.len();
    if position_pnls.len() != n {
        return Err(crate::errors::value_error(format!(
            "position_pnls must have {n} rows (one per position), got {}",
            position_pnls.len()
        )));
    }
    if n == 0 {
        let ids = to_position_ids(position_ids);
        let config = DecompositionConfig::historical(confidence);
        let result = py
            .detach(move || HistoricalPositionDecomposer.decompose_from_pnls(&[], &ids, 0, &config))
            .map_err(core_to_py)?;
        return var_decomposition_to_dict(py, &result);
    }

    let n_scenarios = position_pnls[0].len();
    for (i, row) in position_pnls.iter().enumerate() {
        if row.len() != n_scenarios {
            return Err(crate::errors::value_error(format!(
                "position_pnls row {i} has {} scenarios, expected {n_scenarios}",
                row.len()
            )));
        }
    }

    let config = DecompositionConfig::historical(confidence);
    let result = py
        .detach(move || {
            // Transpose to row-major scenarios x positions layout expected by the engine.
            let mut flat = Vec::with_capacity(n_scenarios * n);
            for s in 0..n_scenarios {
                for row in &position_pnls {
                    flat.push(row[s]);
                }
            }
            let ids = to_position_ids(position_ids);
            HistoricalPositionDecomposer.decompose_from_pnls(&flat, &ids, n_scenarios, &config)
        })
        .map_err(core_to_py)?;

    var_decomposition_to_dict(py, &result)
}

// ---------------------------------------------------------------------------
// Risk budget evaluation
// ---------------------------------------------------------------------------

/// Evaluate a per-position risk budget against actual component VaRs.
///
/// Compares provided actual component VaRs against target fractions using
/// the Rust ``RiskBudget::evaluate_components`` engine. Inputs are the
/// minimum required for budget evaluation -- no ES or marginal data is
/// needed.
///
/// Parameters
/// ----------
/// position_ids : list[str]
///     Position identifiers, length ``n``.
/// actual_var : list[float]
///     Actual component VaR for each position, length ``n``.
/// target_var_pct : list[float]
///     Target fraction of portfolio VaR for each position, length ``n``.
///     Must sum to approximately ``1.0``.
/// portfolio_var : float
///     Total portfolio VaR used to translate target fractions into levels.
/// utilization_threshold : float, default ``1.20``
///     Utilization ratio above which a position is flagged as a breach.
///
/// Returns
/// -------
/// dict
///     ``{portfolio_var, total_overbudget, has_breach,
///     positions: [{position_id, actual_component_var, target_component_var,
///     target_pct, utilization, excess, breach}, ...]}``.
#[pyfunction]
#[pyo3(signature = (position_ids, actual_var, target_var_pct, portfolio_var, utilization_threshold = 1.20))]
fn evaluate_risk_budget<'py>(
    py: Python<'py>,
    position_ids: Vec<String>,
    actual_var: Vec<f64>,
    target_var_pct: Vec<f64>,
    portfolio_var: f64,
    utilization_threshold: f64,
) -> PyResult<Bound<'py, PyDict>> {
    let n = position_ids.len();
    if actual_var.len() != n {
        return Err(crate::errors::value_error(format!(
            "actual_var length ({}) must match position_ids length ({n})",
            actual_var.len()
        )));
    }
    if target_var_pct.len() != n {
        return Err(crate::errors::value_error(format!(
            "target_var_pct length ({}) must match position_ids length ({n})",
            target_var_pct.len()
        )));
    }

    // Use the narrow `evaluate_components` API so we do not have to
    // synthesise a full `PositionRiskDecomposition` (with ES and marginals
    // that would be dummy values) just to call `RiskBudget::evaluate`.
    let shared_ids: Vec<PositionId> = position_ids.into_iter().map(PositionId::new).collect();

    let mut targets: IndexMap<PositionId, f64> = IndexMap::with_capacity(n);
    for (id, &pct) in shared_ids.iter().zip(target_var_pct.iter()) {
        targets.insert(id.clone(), pct);
    }
    let budget = RiskBudget::new(targets).with_threshold(utilization_threshold);
    let result = py
        .detach(move || {
            budget.evaluate_components(
                shared_ids.iter().zip(actual_var.iter().copied()),
                portfolio_var,
            )
        })
        .map_err(core_to_py)?;

    let out = PyDict::new(py);
    out.set_item("portfolio_var", portfolio_var)?;
    out.set_item("total_overbudget", result.total_overbudget)?;
    out.set_item("has_breach", result.has_breach)?;
    out.set_item("utilization_threshold", utilization_threshold)?;

    let positions = PyList::empty(py);
    for (entry, target_pct) in result.positions.iter().zip(target_var_pct.iter()) {
        let d = PyDict::new(py);
        d.set_item("position_id", entry.position_id.as_str())?;
        d.set_item("actual_component_var", entry.actual_component_var)?;
        d.set_item("target_component_var", entry.target_component_var)?;
        d.set_item("target_pct", *target_pct)?;
        d.set_item("utilization", entry.utilization)?;
        d.set_item("excess", entry.excess)?;
        d.set_item("breach", entry.utilization > utilization_threshold)?;
        positions.append(d)?;
    }
    out.set_item("positions", positions)?;

    Ok(out)
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register position-risk decomposition functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parametric_var_decomposition, m)?)?;
    m.add_function(wrap_pyfunction!(parametric_es_decomposition, m)?)?;
    m.add_function(wrap_pyfunction!(historical_var_decomposition, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_risk_budget, m)?)?;
    Ok(())
}

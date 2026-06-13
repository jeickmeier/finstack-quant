//! Portfolio-level factor risk decomposition outputs and engines.
//!
//! This module lifts instrument-level market dependencies and sensitivities into
//! portfolio-level factor analytics. Typical usage is:
//!
//! 1. Build a [`crate::factor_model::FactorModel`] from a declarative
//!    [`finstack_factor_model::FactorModelConfig`].
//! 2. Use [`crate::factor_model::FactorModel::assign_factors`] to inspect how
//!    portfolio positions map to configured factors.
//! 3. Use [`crate::factor_model::FactorModel::compute_sensitivities`] to produce
//!    a weighted sensitivity matrix.
//! 4. Use [`crate::factor_model::FactorModel::analyze`] to decompose portfolio risk.
//!
//! The module exposes both closed-form covariance-based decomposition
//! ([`crate::factor_model::ParametricDecomposer`]) and simulation-based
//! tail-risk decomposition
//! ([`crate::factor_model::SimulationDecomposer`]). All engines assume the upstream sensitivity
//! engine has already scaled rows by position quantity, so downstream
//! decomposition works on portfolio exposures directly.
//!
//! # Conventions
//!
//! - Factor IDs and covariance axes must match exactly in content and order.
//! - Risk outputs are reported in the units implied by the configured
//!   [`finstack_factor_model::RiskMeasure`].
//! - Strict unmatched-dependency handling should be used when factor coverage is
//!   treated as part of the model contract rather than a best-effort mapping.
//!
//! # References
//!
//! - Meucci, factor risk and covariance aggregation:
//!   `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
//! - Parametric VaR conventions:
//!   `docs/REFERENCES.md#jpmorgan1996RiskMetrics`
//! - Coherent/tail-risk measures:
//!   `docs/REFERENCES.md#artzner1999CoherentRisk`

mod assignment;
mod credit_vol_forecast;
mod math;
mod model;
mod parametric;
mod position_risk;
mod risk_budget;
mod simulation;
mod traits;
mod types;
mod whatif;

pub use assignment::{FactorAssignmentReport, PositionAssignment, UnmatchedEntry};
pub use credit_vol_forecast::{
    build_credit_vol_report, CreditVolReport, FactorCovarianceForecast, LevelVolContribution,
    PositionVolContribution, VolHorizon,
};
pub use model::{FactorModel, FactorModelBuilder};
pub use parametric::ParametricDecomposer;
pub use position_risk::{
    DecompositionConfig, DecompositionMethod, HistoricalPositionDecomposer,
    ParametricPositionDecomposer, PositionEsContribution, PositionRiskDecomposition,
    PositionVarContribution, StressAttribution, StressPositionEntry, TailScenarioBreakdown,
};
pub use risk_budget::{PositionBudgetEntry, RiskBudget, RiskBudgetResult};
pub use simulation::SimulationDecomposer;
pub use traits::RiskDecomposer;
pub use types::{
    FactorContribution, PositionFactorContribution, PositionResidualContribution,
    ResidualContributionSource, RiskDecomposition,
};
pub use whatif::{
    FactorContributionDelta, PositionChange, StressResult, WhatIfEngine, WhatIfResult,
};

/// JS/Python-friendly ES contribution row derived from
/// [`PositionEsContribution`].
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PositionEsContributionView {
    /// Position identifier.
    pub position_id: String,
    /// Component Expected Shortfall allocated to the position.
    pub component_es: f64,
    /// Marginal Expected Shortfall, when available.
    pub marginal_es: Option<f64>,
    /// Fraction of total ES contributed by this position.
    pub pct_contribution: f64,
}

/// JS/Python-friendly ES decomposition view.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ParametricEsDecompositionView {
    /// Total portfolio VaR.
    pub portfolio_var: f64,
    /// Total portfolio Expected Shortfall.
    pub portfolio_es: f64,
    /// Confidence level used for ES.
    pub confidence: f64,
    /// Number of positions in the decomposition.
    pub n_positions: usize,
    /// Per-position ES contributions.
    pub contributions: Vec<PositionEsContributionView>,
}

/// JS/Python-friendly VaR contribution row derived from
/// [`PositionVarContribution`].
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PositionVarContributionView {
    /// Position identifier.
    pub position_id: String,
    /// Component VaR allocated to the position.
    pub component_var: f64,
    /// Marginal VaR, when available.
    pub marginal_var: Option<f64>,
    /// Fraction of total VaR contributed by this position.
    pub pct_contribution: f64,
    /// Incremental VaR, when available.
    pub incremental_var: Option<f64>,
}

/// JS/Python-friendly VaR decomposition view.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ParametricVarDecompositionView {
    /// Total portfolio VaR.
    pub portfolio_var: f64,
    /// Total portfolio Expected Shortfall.
    pub portfolio_es: f64,
    /// Confidence level used for VaR.
    pub confidence: f64,
    /// Number of positions in the decomposition.
    pub n_positions: usize,
    /// Euler residual, when computed by the engine.
    pub euler_residual: Option<f64>,
    /// Per-position VaR contributions.
    pub contributions: Vec<PositionVarContributionView>,
}

/// JS/Python-friendly risk-budget row derived from
/// [`PositionBudgetEntry`].
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PositionBudgetEntryView {
    /// Position identifier.
    pub position_id: String,
    /// Actual component VaR.
    pub actual_component_var: f64,
    /// Target component VaR.
    pub target_component_var: f64,
    /// Target share of portfolio VaR.
    pub target_pct: f64,
    /// Utilization ratio.
    pub utilization: f64,
    /// Over-budget amount.
    pub excess: f64,
    /// Whether utilization exceeds the configured threshold.
    pub breach: bool,
}

/// JS/Python-friendly risk-budget result view.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RiskBudgetResultView {
    /// Portfolio VaR used for target scaling.
    pub portfolio_var: f64,
    /// Sum of over-budget amounts.
    pub total_overbudget: f64,
    /// Whether any position breached the utilization threshold.
    pub has_breach: bool,
    /// Utilization threshold used for breach classification.
    pub utilization_threshold: f64,
    /// Per-position budget rows.
    pub positions: Vec<PositionBudgetEntryView>,
}

/// Convert a full position risk decomposition into the legacy VaR view.
#[must_use]
pub fn parametric_var_decomposition_view(
    decomposition: &PositionRiskDecomposition,
) -> ParametricVarDecompositionView {
    let contributions = decomposition
        .var_contributions
        .iter()
        .map(|c| PositionVarContributionView {
            position_id: c.position_id.as_str().to_owned(),
            component_var: c.component_var,
            marginal_var: c.marginal_var,
            pct_contribution: c.relative_var,
            incremental_var: c.incremental_var,
        })
        .collect();
    ParametricVarDecompositionView {
        portfolio_var: decomposition.portfolio_var,
        portfolio_es: decomposition.portfolio_es,
        confidence: decomposition.confidence,
        n_positions: decomposition.n_positions,
        euler_residual: decomposition.euler_residual,
        contributions,
    }
}

/// Convert a risk-budget result into the legacy binding view.
#[must_use]
pub fn risk_budget_result_view(
    result: &RiskBudgetResult,
    portfolio_var: f64,
    utilization_threshold: f64,
) -> RiskBudgetResultView {
    let portfolio_var_magnitude = portfolio_var.abs();
    let positions = result
        .positions
        .iter()
        .map(|entry| {
            let target_pct = if portfolio_var_magnitude > 1e-15 {
                entry.target_component_var / portfolio_var_magnitude
            } else if entry.target_component_var.abs() > 1e-15 {
                f64::INFINITY
            } else {
                0.0
            };
            PositionBudgetEntryView {
                position_id: entry.position_id.as_str().to_owned(),
                actual_component_var: entry.actual_component_var,
                target_component_var: entry.target_component_var,
                target_pct,
                utilization: entry.utilization,
                excess: entry.excess,
                breach: entry.utilization > utilization_threshold,
            }
        })
        .collect();
    RiskBudgetResultView {
        portfolio_var,
        total_overbudget: result.total_overbudget,
        has_breach: result.has_breach,
        utilization_threshold,
        positions,
    }
}

/// Convert a full position risk decomposition into the legacy ES-only view.
#[must_use]
pub fn parametric_es_decomposition_view(
    decomposition: &PositionRiskDecomposition,
) -> ParametricEsDecompositionView {
    let contributions = decomposition
        .es_contributions
        .iter()
        .map(|c| PositionEsContributionView {
            position_id: c.position_id.as_str().to_owned(),
            component_es: c.component_es,
            marginal_es: c.marginal_es,
            pct_contribution: c.relative_es,
        })
        .collect();
    ParametricEsDecompositionView {
        portfolio_var: decomposition.portfolio_var,
        portfolio_es: decomposition.portfolio_es,
        confidence: decomposition.confidence,
        n_positions: decomposition.n_positions,
        contributions,
    }
}

/// Flatten a row-major nested `Vec<Vec<f64>>` into a contiguous `Vec<f64>` after
/// validating squareness against `n`.
///
/// Returns the flat row-major buffer expected by [`ParametricPositionDecomposer`]
/// and [`HistoricalPositionDecomposer`]. Callers that already hold a flat buffer
/// should bypass this helper and pass the buffer directly.
///
/// # Errors
///
/// Returns [`finstack_core::Error::Validation`] when the matrix has the wrong
/// number of rows or any row has the wrong number of columns. The error
/// message includes the expected/actual dimensions and the offending row index
/// so the same diagnostic surfaces in both the Python and WASM bindings.
///
/// # Arguments
///
/// * `matrix` - Row-major nested vector (each inner vec is one row).
/// * `n` - Expected square dimension.
/// * `label` - Caller-provided label included in error messages (e.g. `"covariance"`).
pub fn flatten_square_matrix(
    matrix: Vec<Vec<f64>>,
    n: usize,
    label: &str,
) -> finstack_core::Result<Vec<f64>> {
    if matrix.len() != n {
        return Err(finstack_core::Error::Validation(format!(
            "{label} must have {n} rows, got {}",
            matrix.len()
        )));
    }
    let mut flat = Vec::with_capacity(n * n);
    for (i, row) in matrix.into_iter().enumerate() {
        if row.len() != n {
            return Err(finstack_core::Error::Validation(format!(
                "{label} row {i} must have {n} columns, got {}",
                row.len()
            )));
        }
        flat.extend(row);
    }
    Ok(flat)
}

/// Flatten per-position scenario P&Ls from `[n_positions][n_scenarios]` into
/// the scenario-major buffer expected by [`HistoricalPositionDecomposer`].
///
/// # Errors
///
/// Returns [`finstack_core::Error::Validation`] when the number of rows does
/// not equal `n_positions` or when rows have inconsistent scenario counts.
pub fn flatten_position_pnls(
    position_pnls: Vec<Vec<f64>>,
    n_positions: usize,
) -> finstack_core::Result<(Vec<f64>, usize)> {
    if position_pnls.len() != n_positions {
        return Err(finstack_core::Error::Validation(format!(
            "position_pnls must have {n_positions} rows, got {}",
            position_pnls.len()
        )));
    }
    if n_positions == 0 {
        return Ok((Vec::new(), 0));
    }
    let n_scenarios = position_pnls[0].len();
    for (i, row) in position_pnls.iter().enumerate() {
        if row.len() != n_scenarios {
            return Err(finstack_core::Error::Validation(format!(
                "position_pnls row {i} has {} scenarios, expected {n_scenarios}",
                row.len()
            )));
        }
    }
    let mut flat = Vec::with_capacity(n_scenarios * n_positions);
    for s in 0..n_scenarios {
        for row in &position_pnls {
            flat.push(row[s]);
        }
    }
    Ok((flat, n_scenarios))
}

#[cfg(test)]
mod tests {
    use super::flatten_square_matrix;

    #[test]
    fn flatten_square_matrix_round_trip() {
        let m = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let flat = flatten_square_matrix(m, 2, "cov").expect("valid 2x2");
        assert_eq!(flat, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn flatten_square_matrix_rejects_wrong_row_count() {
        let m = vec![vec![1.0, 2.0]];
        let err = flatten_square_matrix(m, 2, "cov").expect_err("missing row");
        assert!(err.to_string().contains("cov must have 2 rows"));
    }

    #[test]
    fn flatten_square_matrix_rejects_wrong_column_count() {
        let m = vec![vec![1.0, 2.0, 3.0], vec![1.0, 2.0]];
        let err = flatten_square_matrix(m, 2, "cov").expect_err("wrong row width");
        assert!(err.to_string().contains("row 0 must have 2 columns"));
    }
}

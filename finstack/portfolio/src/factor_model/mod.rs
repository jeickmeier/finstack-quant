//! Portfolio-level factor risk decomposition outputs and engines.
//!
//! This module lifts instrument-level market dependencies and sensitivities into
//! portfolio-level factor analytics. Typical usage is:
//!
//! 1. Build a [`FactorModel`] from a declarative
//!    [`finstack_factor_model::FactorModelConfig`].
//! 2. Use [`FactorModel::assign_factors`] to inspect how portfolio positions map
//!    to configured factors.
//! 3. Use [`FactorModel::compute_sensitivities`] to produce a weighted
//!    sensitivity matrix.
//! 4. Use [`FactorModel::analyze`] to decompose portfolio risk.
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
mod optimization;
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
pub use optimization::{FactorConstraint, FactorOptimizationResult};
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

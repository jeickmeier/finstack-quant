//! Serializable factor-risk reporting views and matrix input adapters.

use super::PositionRiskDecomposition;

/// Serializable Expected Shortfall contribution row.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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

/// Serializable Expected Shortfall decomposition view.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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

/// Convert a full position risk decomposition into the serializable ES view.
///
/// # Arguments
///
/// * `decomposition` - Position-level risk decomposition whose Expected
///   Shortfall contributions, confidence, and portfolio totals are copied into
///   the reporting representation.
#[must_use]
pub fn parametric_es_decomposition_view(
    decomposition: &PositionRiskDecomposition,
) -> ParametricEsDecompositionView {
    let contributions = decomposition
        .es_contributions
        .iter()
        .map(|contribution| PositionEsContributionView {
            position_id: contribution.position_id.clone(),
            component_es: contribution.component_es,
            marginal_es: contribution.marginal_es,
            pct_contribution: contribution.relative_es,
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

/// Flatten a row-major nested matrix after validating squareness against `n`.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when the matrix has the
/// wrong number of rows or any row has the wrong number of columns.
///
/// # Arguments
///
/// * `matrix` - Row-major nested vector with one inner vector per row.
/// * `n` - Expected square dimension.
/// * `label` - Caller-provided label included in validation messages.
pub fn flatten_square_matrix(
    matrix: Vec<Vec<f64>>,
    n: usize,
    label: &str,
) -> finstack_quant_core::Result<Vec<f64>> {
    if matrix.len() != n {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{label} must have {n} rows, got {}",
            matrix.len()
        )));
    }
    let mut flat = Vec::with_capacity(n * n);
    for (index, row) in matrix.into_iter().enumerate() {
        if row.len() != n {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{label} row {index} must have {n} columns, got {}",
                row.len()
            )));
        }
        flat.extend(row);
    }
    Ok(flat)
}

/// Flatten per-position scenario P&Ls into a scenario-major buffer.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when the number of rows
/// does not equal `n_positions` or rows have inconsistent scenario counts.
///
/// # Arguments
///
/// * `position_pnls` - Position-major P&L matrix with one row per position.
/// * `n_positions` - Expected number of position rows.
pub fn flatten_position_pnls(
    position_pnls: Vec<Vec<f64>>,
    n_positions: usize,
) -> finstack_quant_core::Result<(Vec<f64>, usize)> {
    if position_pnls.len() != n_positions {
        return Err(finstack_quant_core::Error::Validation(format!(
            "position_pnls must have {n_positions} rows, got {}",
            position_pnls.len()
        )));
    }
    if n_positions == 0 {
        return Ok((Vec::new(), 0));
    }
    let n_scenarios = position_pnls[0].len();
    for (index, row) in position_pnls.iter().enumerate() {
        if row.len() != n_scenarios {
            return Err(finstack_quant_core::Error::Validation(format!(
                "position_pnls row {index} has {} scenarios, expected {n_scenarios}",
                row.len()
            )));
        }
    }
    let mut flat = Vec::with_capacity(n_scenarios * n_positions);
    for scenario in 0..n_scenarios {
        for row in &position_pnls {
            flat.push(row[scenario]);
        }
    }
    Ok((flat, n_scenarios))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_square_matrix_validates_shape() {
        let flat = flatten_square_matrix(vec![vec![1.0, 2.0], vec![3.0, 4.0]], 2, "cov")
            .expect("valid matrix");
        assert_eq!(flat, vec![1.0, 2.0, 3.0, 4.0]);
        assert!(flatten_square_matrix(vec![vec![1.0, 2.0]], 2, "cov").is_err());
        assert!(
            flatten_square_matrix(vec![vec![1.0, 2.0, 3.0], vec![1.0, 2.0]], 2, "cov").is_err()
        );
    }
}

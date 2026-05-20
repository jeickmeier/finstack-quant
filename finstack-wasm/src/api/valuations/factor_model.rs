//! WASM bindings for factor-model sensitivities and risk decomposition.

use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

/// Compute first-order factor sensitivities and return the matrix as JSON.
///
/// Accepts a JSON array of positions, a JSON array of `FactorDefinition`,
/// a `MarketContext` JSON, an ISO 8601 date, and an optional `BumpSizeConfig`
/// JSON.  Returns a JSON object with `position_ids`, `factor_ids`, and a
/// row-major `data` matrix.
#[wasm_bindgen(js_name = computeFactorSensitivities)]
pub fn compute_factor_sensitivities(
    positions_json: &str,
    factors_json: &str,
    market_json: &str,
    as_of: &str,
    bump_config_json: Option<String>,
) -> Result<String, JsValue> {
    finstack_valuations::factor_model::compute_factor_sensitivities_json(
        positions_json,
        factors_json,
        market_json,
        as_of,
        bump_config_json.as_deref(),
    )
    .map_err(to_js_err)
}

/// Compute scenario P&L profiles via full repricing and return as JSON.
///
/// Same position/factor/market inputs as `computeFactorSensitivities`, plus
/// an optional `n_scenario_points` integer.
#[wasm_bindgen(js_name = computePnlProfiles)]
pub fn compute_pnl_profiles(
    positions_json: &str,
    factors_json: &str,
    market_json: &str,
    as_of: &str,
    bump_config_json: Option<String>,
    n_scenario_points: Option<usize>,
) -> Result<String, JsValue> {
    finstack_valuations::factor_model::compute_pnl_profiles_json(
        positions_json,
        factors_json,
        market_json,
        as_of,
        bump_config_json.as_deref(),
        n_scenario_points,
    )
    .map_err(to_js_err)
}

/// Decompose portfolio risk into factor and position contributions.
///
/// Uses the parametric (covariance-based) Euler decomposition.  Accepts
/// a JSON sensitivity matrix (same schema as the output of
/// `computeFactorSensitivities`), a `FactorCovarianceMatrix` JSON, and an
/// optional `RiskMeasure` JSON.
///
/// Returns a JSON object with `total_risk`, `measure`, `residual_risk`,
/// `factor_contributions` (array), and `position_factor_contributions` (array).
#[wasm_bindgen(js_name = decomposeFactorRisk)]
pub fn decompose_factor_risk(
    sensitivities_json: &str,
    covariance_json: &str,
    risk_measure_json: Option<String>,
) -> Result<String, JsValue> {
    #[derive(serde::Deserialize)]
    struct SensInput {
        position_ids: Vec<String>,
        factor_ids: Vec<String>,
        data: Vec<Vec<f64>>,
    }

    let input: SensInput = serde_json::from_str(sensitivities_json).map_err(to_js_err)?;
    let factor_ids: Vec<finstack_core::factor_model::FactorId> = input
        .factor_ids
        .iter()
        .map(finstack_core::factor_model::FactorId::new)
        .collect();

    let n_positions = input.position_ids.len();
    let n_factors = factor_ids.len();
    validate_sensitivity_data(&input.data, n_positions, n_factors).map_err(to_js_err)?;

    let mut matrix =
        finstack_valuations::factor_model::SensitivityMatrix::zeros(input.position_ids, factor_ids);
    for (pi, row) in input.data.iter().enumerate() {
        for (fi, &val) in row.iter().enumerate() {
            matrix.set_delta(pi, fi, val);
        }
    }

    let covariance: finstack_core::factor_model::FactorCovarianceMatrix =
        serde_json::from_str(covariance_json).map_err(to_js_err)?;

    let measure: finstack_core::factor_model::RiskMeasure = match risk_measure_json {
        Some(ref json) => serde_json::from_str(json).map_err(to_js_err)?,
        None => finstack_core::factor_model::RiskMeasure::Variance,
    };

    let decomposer = finstack_portfolio::factor_model::ParametricDecomposer;
    let result = finstack_portfolio::factor_model::RiskDecomposer::decompose(
        &decomposer,
        &matrix,
        &covariance,
        &measure,
    )
    .map_err(to_js_err)?;

    let output = serde_json::json!({
        "total_risk": result.total_risk,
        "measure": format!("{:?}", result.measure),
        "residual_risk": result.residual_risk,
        "factor_contributions": result.factor_contributions.iter().map(|c| {
            serde_json::json!({
                "factor_id": c.factor_id.to_string(),
                "absolute_risk": c.absolute_risk,
                "relative_risk": c.relative_risk,
                "marginal_risk": c.marginal_risk,
            })
        }).collect::<Vec<_>>(),
        "position_factor_contributions": result.position_factor_contributions.iter().map(|c| {
            serde_json::json!({
                "position_id": c.position_id.to_string(),
                "factor_id": c.factor_id.to_string(),
                "risk_contribution": c.risk_contribution,
            })
        }).collect::<Vec<_>>(),
    });
    serde_json::to_string(&output).map_err(to_js_err)
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Validate that `data` has exactly `n_positions` rows and that every row has
/// exactly `n_factors` columns.
///
/// This must be called before populating a `SensitivityMatrix` from JSON
/// input.  `SensitivityMatrix::set_delta` is only guarded by a
/// `debug_assert!`, which is compiled out in release WASM builds, so
/// out-of-bounds access in production would be an out-of-bounds Vec index —
/// an abort across the WASM boundary rather than a catchable JS error.
fn validate_sensitivity_data(
    data: &[Vec<f64>],
    n_positions: usize,
    n_factors: usize,
) -> Result<(), String> {
    if data.len() != n_positions {
        return Err(format!(
            "sensitivity data has {} row(s) but position_ids declares {} position(s)",
            data.len(),
            n_positions,
        ));
    }
    for (pi, row) in data.iter().enumerate() {
        if row.len() != n_factors {
            return Err(format!(
                "sensitivity data row {} has {} element(s) but factor_ids declares {} factor(s)",
                pi,
                row.len(),
                n_factors,
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_sensitivity_data;

    #[test]
    fn validate_rejects_too_many_rows() {
        // 3 data rows but only 2 positions declared — must error
        let data = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];
        let result = validate_sensitivity_data(&data, 2, 2);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("3"),
            "error should mention actual row count: {msg}"
        );
        assert!(
            msg.contains("2"),
            "error should mention declared positions: {msg}"
        );
    }

    #[test]
    fn validate_rejects_row_wider_than_factor_count() {
        // Row 1 has 3 elements but only 2 factors declared — must error
        let data = vec![vec![1.0, 2.0], vec![3.0, 4.0, 5.0]];
        let result = validate_sensitivity_data(&data, 2, 2);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("row 1"),
            "error should name the offending row: {msg}"
        );
        assert!(
            msg.contains("3"),
            "error should mention actual column count: {msg}"
        );
        assert!(
            msg.contains("2"),
            "error should mention declared factor count: {msg}"
        );
    }

    #[test]
    fn validate_rejects_row_narrower_than_factor_count() {
        // Row 0 has 1 element but 2 factors declared — must error
        let data = vec![vec![1.0]];
        let result = validate_sensitivity_data(&data, 1, 2);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("row 0"),
            "error should name the offending row: {msg}"
        );
    }

    #[test]
    fn validate_accepts_well_formed_data() {
        let data = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        assert!(validate_sensitivity_data(&data, 2, 2).is_ok());
    }

    #[test]
    fn validate_accepts_empty_matrix() {
        // Zero positions, zero factors, zero data rows — valid degenerate case
        let data: Vec<Vec<f64>> = vec![];
        assert!(validate_sensitivity_data(&data, 0, 0).is_ok());
    }
}

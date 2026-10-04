//! Risk budgeting for position-level VaR decomposition.
//!
//! A risk budget assigns a target share of total portfolio VaR to each
//! position (or group of positions). The budgeting engine compares actual
//! component VaR against targets and computes utilization ratios.

use finstack_quant_core::wire::NonFiniteFields;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Default maximum acceptable utilization before a budget breach is flagged.
///
/// 1.20 means a position may use up to 120% of its budgeted component VaR
/// before [`RiskBudgetResult::has_breach`] is raised. This is the single
/// source of truth for the default consumed by both language bindings.
pub const DEFAULT_UTILIZATION_THRESHOLD: f64 = 1.20;

/// Result of comparing actual risk decomposition against a risk budget.
///
/// Utilization is measured on the *consuming* side: a position's component
/// VaR is positive utilization when it carries the same sign as the portfolio
/// VaR (it consumes risk) and negative utilization when it offsets portfolio
/// risk (a diversifier / hedge). Diversifiers can never breach the budget and
/// never contribute to [`Self::total_overbudget`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct RiskBudgetResult {
    /// Per-position budget comparison.
    pub positions: Vec<PositionBudgetEntry>,

    /// Total over-budget amount: sum of positive consuming-side exceedances.
    pub total_overbudget: f64,

    /// Whether any position exceeds its utilization threshold.
    pub has_breach: bool,
}

/// Budget comparison for a single position.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct PositionBudgetEntry {
    /// Position identifier.
    pub position_id: String,

    /// Actual component VaR from the decomposition, signed as reported by
    /// the engine (loss convention: negative for risk consumers when the
    /// portfolio VaR is negative).
    pub actual_component_var: f64,

    /// Target component VaR level: target fraction times |portfolio VaR|.
    pub target_component_var: f64,

    /// Utilization ratio: consuming-side component VaR over the target level.
    ///
    /// The component is measured on the consuming side (positive when it has
    /// the same sign as portfolio VaR). Values > 1.0 indicate the position
    /// uses more risk than budgeted; values in (0, 1) indicate unused
    /// budget; **negative values indicate a diversifier** whose component
    /// VaR offsets portfolio risk — a diversifier can never breach.
    /// `±inf` marks a non-zero component against a zero target.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub utilization: f64,

    /// Over/under-budget amount on the consuming side: consuming component
    /// VaR minus the target level. Negative when under budget, and always
    /// negative for diversifiers.
    pub excess: f64,
}

impl NonFiniteFields for PositionBudgetEntry {
    const NON_FINITE_FIELDS: &'static [&'static str] = &["utilization"];
}

/// Evaluate a per-position risk budget from parallel binding-style arrays.
///
/// Owns numeric-domain, array-length and duplicate-position validation so
/// the Python `evaluate_risk_budget` function and the
/// WASM `evaluateRiskBudget` export share one behavior and one set of
/// diagnostics.
///
/// # Arguments
///
/// * `position_ids` - Position identifiers, one per entry of `actual_var` and
///   `target_var_pct`. Duplicates are rejected.
/// * `actual_var` - Finite actual component VaR per position (loss convention;
///   signs are kept — a component whose sign opposes `portfolio_var` is a
///   diversifier and reports negative utilization).
/// * `target_var_pct` - Finite target fraction in `[0, 1]` per position; a
///   non-empty budget must sum to ~1.0 (within 0.05).
/// * `portfolio_var` - Finite total portfolio VaR used to convert target fractions
///   into levels.
/// * `utilization_threshold` - Finite, strictly positive utilization ratio above which a breach is
///   flagged (see [`DEFAULT_UTILIZATION_THRESHOLD`]).
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when the array lengths
/// disagree, a position id is duplicated, the non-empty targets do not sum to
/// ~1.0, numeric inputs are non-finite, a target fraction is outside `[0, 1]`,
/// the threshold is non-positive, or non-zero component VaR is paired with
/// zero portfolio VaR.
pub fn evaluate_risk_budget_arrays(
    position_ids: Vec<String>,
    actual_var: &[f64],
    target_var_pct: &[f64],
    portfolio_var: f64,
    utilization_threshold: f64,
) -> finstack_quant_core::Result<RiskBudgetResult> {
    let n = position_ids.len();
    if actual_var.len() != n {
        return Err(finstack_quant_core::Error::Validation(format!(
            "actual_var length ({}) must match position_ids length ({n})",
            actual_var.len()
        )));
    }
    if target_var_pct.len() != n {
        return Err(finstack_quant_core::Error::Validation(format!(
            "target_var_pct length ({}) must match position_ids length ({n})",
            target_var_pct.len()
        )));
    }

    let mut seen = BTreeSet::new();
    for id in &position_ids {
        if !seen.insert(id.as_str()) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "duplicate position_id '{id}' in position_ids"
            )));
        }
    }
    if !portfolio_var.is_finite() {
        return Err(finstack_quant_core::Error::Validation(
            "portfolio_var must be finite".into(),
        ));
    }
    if !utilization_threshold.is_finite() || utilization_threshold <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "utilization_threshold must be finite and strictly positive".into(),
        ));
    }
    for (position_id, target) in position_ids.iter().zip(target_var_pct) {
        if !target.is_finite() || !(0.0..=1.0).contains(target) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "risk budget target for '{position_id}' must be finite and in [0, 1]"
            )));
        }
    }
    let target_sum: f64 = target_var_pct.iter().sum();
    if n > 0 && (target_sum - 1.0).abs() > 0.05 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "risk budget targets must sum to ~1.0, got {target_sum}"
        )));
    }
    for (position_id, component) in position_ids.iter().zip(actual_var) {
        if !component.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "component VaR for '{position_id}' must be finite"
            )));
        }
    }
    let portfolio_var_magnitude = portfolio_var.abs();
    if portfolio_var_magnitude <= 1e-15 && actual_var.iter().any(|c| c.abs() > 1e-15) {
        return Err(finstack_quant_core::Error::Validation(
            "portfolio VaR must be non-zero when component VaR is non-zero".to_string(),
        ));
    }

    // Consuming-side orientation: a component VaR with the same sign as
    // the portfolio VaR consumes risk; the opposite sign diversifies.
    // The magnitude of the portfolio VaR is used only to convert target
    // fractions into levels.
    let portfolio_sign = if portfolio_var < 0.0 { -1.0 } else { 1.0 };

    let mut positions = Vec::with_capacity(n);
    let mut total_overbudget = 0.0;
    let mut has_breach = false;

    for ((position_id, &signed_actual), &target_frac) in
        position_ids.into_iter().zip(actual_var).zip(target_var_pct)
    {
        // Positive when the position consumes portfolio risk, negative
        // for diversifiers.
        let consuming_actual = signed_actual * portfolio_sign;

        let target_component = target_frac * portfolio_var_magnitude;

        let utilization = if target_component.abs() > 1e-15 {
            consuming_actual / target_component
        } else if consuming_actual.abs() > 1e-15 {
            // Zero target, non-zero component: infinitely over budget on
            // the consuming side, infinitely under on the diversifying
            // side.
            if consuming_actual > 0.0 {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            }
        } else {
            // Both zero.
            1.0
        };

        // Over-budget only on the consuming side: a diversifier's excess
        // is always negative and never breaches.
        let excess = consuming_actual - target_component;
        if excess > 0.0 {
            total_overbudget += excess;
        }

        if utilization > utilization_threshold {
            has_breach = true;
        }

        positions.push(PositionBudgetEntry {
            position_id,
            actual_component_var: signed_actual,
            target_component_var: target_component,
            utilization,
            excess,
        });
    }

    Ok(RiskBudgetResult {
        positions,
        total_overbudget,
        has_breach,
    })
}

#[cfg(test)]
mod tests {
    use super::super::position::{
        DecompositionConfig, DecompositionMethod, ParametricPositionDecomposer,
        PositionRiskDecomposition, PositionVarContribution,
    };
    use super::*;

    type TestResult = finstack_quant_core::Result<()>;

    #[test]
    fn position_budget_entry_non_finite_fields_match_serde_sentinels() {
        let entry = PositionBudgetEntry {
            position_id: "P".to_string(),
            actual_component_var: f64::NAN,
            target_component_var: f64::NAN,
            utilization: f64::NAN,
            excess: f64::NAN,
        };
        let json = serde_json::to_value(&entry).expect("serialize");
        let mut sentinels: Vec<&str> = json
            .as_object()
            .expect("object")
            .iter()
            .filter(|(key, value)| value.is_string() && key.as_str() != "position_id")
            .map(|(key, _)| key.as_str())
            .collect();
        sentinels.sort_unstable();
        assert_eq!(sentinels, PositionBudgetEntry::NON_FINITE_FIELDS);
    }

    #[test]
    fn risk_budget_rejects_invalid_numeric_inputs_instead_of_reporting_no_breach() {
        let ids = vec!["P".to_string()];
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                evaluate_risk_budget_arrays(ids.clone(), &[value], &[1.0], -100.0, 1.2).is_err()
            );
            assert!(
                evaluate_risk_budget_arrays(ids.clone(), &[-100.0], &[1.0], value, 1.2).is_err()
            );
            assert!(
                evaluate_risk_budget_arrays(ids.clone(), &[-100.0], &[value], -100.0, 1.2).is_err()
            );
            assert!(
                evaluate_risk_budget_arrays(ids.clone(), &[-100.0], &[1.0], -100.0, value).is_err()
            );
        }
        for threshold in [0.0, -1.0] {
            assert!(
                evaluate_risk_budget_arrays(ids.clone(), &[-100.0], &[1.0], -100.0, threshold)
                    .is_err()
            );
        }
        assert!(evaluate_risk_budget_arrays(
            vec!["A".into(), "B".into()],
            &[-120.0, 20.0],
            &[-0.1, 1.1],
            -100.0,
            1.2,
        )
        .is_err());
    }

    /// Evaluate `(position_id, target fraction)` pairs against a decomposition
    /// whose contributions are listed in the same position order.
    fn evaluate(
        targets: &[(&str, f64)],
        utilization_threshold: f64,
        decomposition: &PositionRiskDecomposition,
    ) -> finstack_quant_core::Result<RiskBudgetResult> {
        let ids: Vec<String> = decomposition
            .var_contributions
            .iter()
            .map(|c| c.position_id.clone())
            .collect();
        assert!(ids
            .iter()
            .map(String::as_str)
            .eq(targets.iter().map(|(id, _)| *id)));
        let actual: Vec<f64> = decomposition
            .var_contributions
            .iter()
            .map(|c| c.component_var)
            .collect();
        let target_pct: Vec<f64> = targets.iter().map(|(_, pct)| *pct).collect();
        evaluate_risk_budget_arrays(
            ids,
            &actual,
            &target_pct,
            decomposition.portfolio_var,
            utilization_threshold,
        )
    }

    fn sample_decomposition() -> PositionRiskDecomposition {
        // Manually construct a decomposition for budget tests.
        PositionRiskDecomposition {
            portfolio_var: 100.0,
            portfolio_es: 120.0,
            confidence: 0.95,
            method: DecompositionMethod::Parametric,
            var_contributions: vec![
                PositionVarContribution {
                    position_id: String::from("A"),
                    component_var: 40.0,
                    relative_var: 0.40,
                    marginal_var: Some(0.10),
                    incremental_var: None,
                },
                PositionVarContribution {
                    position_id: String::from("B"),
                    component_var: 35.0,
                    relative_var: 0.35,
                    marginal_var: Some(0.09),
                    incremental_var: None,
                },
                PositionVarContribution {
                    position_id: String::from("C"),
                    component_var: 25.0,
                    relative_var: 0.25,
                    marginal_var: Some(0.08),
                    incremental_var: None,
                },
            ],
            es_contributions: Vec::new(),
            n_positions: 3,
            euler_residual: Some(0.0),
        }
    }

    #[test]
    fn risk_budget_utilization_calculation() -> TestResult {
        let decomp = sample_decomposition();

        let targets = [("A", 0.33), ("B", 0.34), ("C", 0.33)];

        let result = evaluate(&targets, DEFAULT_UTILIZATION_THRESHOLD, &decomp)?;

        // Position A: actual 40/100 = 40%, target 33% => over-budget.
        let a_entry = result
            .positions
            .iter()
            .find(|e| e.position_id == "A")
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation("Position A not found".to_string())
            })?;
        assert!(
            (a_entry.actual_component_var - 40.0).abs() < 1e-10,
            "actual_component_var = {}",
            a_entry.actual_component_var
        );
        assert!(
            (a_entry.target_component_var - 33.0).abs() < 1e-10,
            "target_component_var = {}",
            a_entry.target_component_var
        );
        assert!(
            (a_entry.utilization - 40.0 / 33.0).abs() < 1e-10,
            "utilization = {}",
            a_entry.utilization
        );
        assert!(a_entry.excess > 0.0);

        // Position C: actual 25/100 = 25%, target 33% => under-budget.
        let c_entry = result
            .positions
            .iter()
            .find(|e| e.position_id == "C")
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation("Position C not found".to_string())
            })?;
        assert!(c_entry.excess < 0.0);
        assert!(c_entry.utilization < 1.0);

        Ok(())
    }

    #[test]
    fn risk_budget_breach_detection() -> TestResult {
        let decomp = sample_decomposition();

        let targets = [
            ("A", 0.20), // Actual 40% vs target 20% => 200% utilization.
            ("B", 0.40),
            ("C", 0.40),
        ];

        let result = evaluate(&targets, 1.50, &decomp)?;

        assert!(result.has_breach, "should detect breach for position A");
        assert!(result.total_overbudget > 0.0);

        Ok(())
    }

    #[test]
    fn risk_budget_handles_negative_loss_convention_components() -> TestResult {
        let result = evaluate_risk_budget_arrays(
            vec!["A".to_string(), "B".to_string()],
            &[-40.0, -60.0],
            &[0.20, 0.80],
            -100.0,
            1.50,
        )?;

        let a_entry = result
            .positions
            .iter()
            .find(|entry| entry.position_id == "A")
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation("Position A not found".to_string())
            })?;
        assert!(result.has_breach);
        assert!(result.total_overbudget > 0.0);
        assert!((a_entry.utilization - 2.0).abs() < 1e-12);
        assert!(a_entry.excess > 0.0);
        Ok(())
    }

    // Diversifiers (component VaR opposite in sign to portfolio VaR) must
    // report negative utilization and can never breach; taking |component|
    // inverted them into apparent risk consumers.
    #[test]
    fn risk_budget_diversifier_has_negative_utilization_and_cannot_breach() -> TestResult {
        let weights = [1.0, 0.2];
        let covariance = [0.04, -0.03, -0.03, 0.09];
        let ids = [String::from("A"), String::from("B")];
        let config = DecompositionConfig::parametric_95();
        let decomp = ParametricPositionDecomposer.decompose_positions(
            &weights,
            &covariance,
            &ids,
            &config,
        )?;

        // B is a hedge: its component VaR carries the opposite sign of the
        // portfolio VaR.
        let portfolio_sign = decomp.portfolio_var.signum();
        let b_component = decomp
            .var_contributions
            .iter()
            .find(|c| c.position_id == "B")
            .map(|c| c.component_var)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation("Position B not found".to_string())
            })?;
        assert!(
            b_component * portfolio_sign < 0.0,
            "B must be a diversifier"
        );

        let targets = [("A", 0.9), ("B", 0.1)];
        let result = evaluate(&targets, DEFAULT_UTILIZATION_THRESHOLD, &decomp)?;

        let a_entry = result
            .positions
            .iter()
            .find(|entry| entry.position_id == "A")
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation("Position A not found".to_string())
            })?;
        let b_entry = result
            .positions
            .iter()
            .find(|entry| entry.position_id == "B")
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation("Position B not found".to_string())
            })?;

        // A consumes cv_A / (0.9 * sigma^2) = 0.034 / (0.9 * 0.0316) of its
        // budget (the z-score cancels).
        assert!(
            (a_entry.utilization - 0.034 / (0.9 * 0.0316)).abs() < 1e-9,
            "A utilization = {}",
            a_entry.utilization
        );
        assert!(
            b_entry.utilization < 0.0,
            "diversifier utilization must be negative, got {}",
            b_entry.utilization
        );
        assert!(
            b_entry.excess < 0.0,
            "a diversifier can never be over budget, excess = {}",
            b_entry.excess
        );
        assert!(!result.has_breach, "no position consumes above threshold");
        Ok(())
    }

    #[test]
    fn risk_budget_entry_serializes_non_finite_utilization() {
        let entry = PositionBudgetEntry {
            position_id: String::from("A"),
            actual_component_var: 1.0,
            target_component_var: 0.0,
            utilization: f64::INFINITY,
            excess: 1.0,
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(
            json.contains("\"utilization\":\"inf\""),
            "infinite utilization must survive the wire, got {json}"
        );
        let restored: PositionBudgetEntry = serde_json::from_str(&json).expect("deserialize");
        assert!(restored.utilization.is_infinite());
    }

    #[test]
    fn risk_budget_rejects_bad_target_sum() {
        let decomp = sample_decomposition();

        let targets = [("A", 0.5), ("B", 0.5), ("C", 0.5)];

        let result = evaluate(&targets, DEFAULT_UTILIZATION_THRESHOLD, &decomp);
        assert!(result.is_err());
    }

    #[test]
    fn evaluate_risk_budget_arrays_rejects_duplicate_position_ids() {
        let err = evaluate_risk_budget_arrays(
            vec!["A".to_string(), "A".to_string()],
            &[40.0, 60.0],
            &[0.5, 0.5],
            100.0,
            DEFAULT_UTILIZATION_THRESHOLD,
        )
        .expect_err("duplicate ids must be rejected");
        assert!(
            err.to_string().contains("duplicate position_id 'A'"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn evaluate_risk_budget_arrays_rejects_length_mismatches() {
        let err = evaluate_risk_budget_arrays(
            vec!["A".to_string(), "B".to_string()],
            &[40.0],
            &[0.5, 0.5],
            100.0,
            DEFAULT_UTILIZATION_THRESHOLD,
        )
        .expect_err("actual_var length mismatch must be rejected");
        assert!(
            err.to_string()
                .contains("actual_var length (1) must match position_ids length (2)"),
            "unexpected error: {err}"
        );

        let err = evaluate_risk_budget_arrays(
            vec!["A".to_string(), "B".to_string()],
            &[40.0, 60.0],
            &[1.0],
            100.0,
            DEFAULT_UTILIZATION_THRESHOLD,
        )
        .expect_err("target_var_pct length mismatch must be rejected");
        assert!(
            err.to_string()
                .contains("target_var_pct length (1) must match position_ids length (2)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn evaluate_risk_budget_arrays_flags_breach_over_threshold() -> TestResult {
        let result = evaluate_risk_budget_arrays(
            vec!["A".to_string(), "B".to_string()],
            &[40.0, 60.0],
            &[0.2, 0.8],
            100.0,
            1.50,
        )?;
        assert_eq!(result.positions.len(), 2);
        let a_entry = result
            .positions
            .iter()
            .find(|entry| entry.position_id == "A")
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation("Position A not found".to_string())
            })?;
        assert!((a_entry.utilization - 2.0).abs() < 1e-12);
        assert!(result.has_breach);
        Ok(())
    }

    #[test]
    fn risk_budget_with_real_decomposition() -> TestResult {
        // Run the full parametric decomposer then evaluate budget.
        let weights = [0.4, 0.35, 0.25];
        let covariance = [0.04, 0.01, 0.005, 0.01, 0.09, 0.02, 0.005, 0.02, 0.0625];
        let ids = [String::from("A"), String::from("B"), String::from("C")];
        let config = DecompositionConfig::parametric_95();

        let decomposer = ParametricPositionDecomposer;
        let decomp = decomposer.decompose_positions(&weights, &covariance, &ids, &config)?;

        let targets = [("A", 0.33), ("B", 0.34), ("C", 0.33)];

        let result = evaluate(&targets, DEFAULT_UTILIZATION_THRESHOLD, &decomp)?;

        assert_eq!(result.positions.len(), 3);

        // Verify utilization is computed correctly for each position:
        // consuming-side component (signed component times the sign of the
        // portfolio VaR) over the positive target level.
        let portfolio_sign = if decomp.portfolio_var < 0.0 {
            -1.0
        } else {
            1.0
        };
        for entry in &result.positions {
            if entry.target_component_var.abs() > 1e-15 {
                let expected_util =
                    entry.actual_component_var * portfolio_sign / entry.target_component_var;
                assert!(
                    (entry.utilization - expected_util).abs() < 1e-10,
                    "utilization mismatch for {}: got {}, expected {}",
                    entry.position_id,
                    entry.utilization,
                    expected_util
                );
                assert!(
                    entry.utilization > 0.0,
                    "all-long portfolio: every position consumes risk"
                );
            }
        }

        Ok(())
    }
}

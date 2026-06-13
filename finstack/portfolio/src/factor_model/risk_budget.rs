//! Risk budgeting for position-level VaR decomposition.
//!
//! A risk budget assigns a target share of total portfolio VaR to each
//! position (or group of positions). The budgeting engine compares actual
//! component VaR against targets and computes utilization ratios.

use crate::types::PositionId;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::position_risk::PositionRiskDecomposition;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Target risk allocation for a portfolio.
///
/// A risk budget assigns a target share of total portfolio VaR to each
/// position. The budgeting engine compares actual component VaR against
/// targets and computes utilization ratios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskBudget {
    /// Per-position target allocations.
    ///
    /// Keys are position IDs; values are target fractions of portfolio VaR
    /// (must sum to 1.0).
    pub targets: IndexMap<PositionId, f64>,

    /// Maximum acceptable utilization before triggering a rebalance alert.
    ///
    /// Default: 1.20 (120% of budget).
    pub utilization_threshold: f64,
}

impl Default for RiskBudget {
    fn default() -> Self {
        Self {
            targets: IndexMap::new(),
            utilization_threshold: 1.20,
        }
    }
}

/// Result of comparing actual risk decomposition against a risk budget.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskBudgetResult {
    /// Per-position budget comparison.
    pub positions: Vec<PositionBudgetEntry>,

    /// Total absolute over-budget amount (sum of exceedances).
    pub total_overbudget: f64,

    /// Whether any position exceeds its utilization threshold.
    pub has_breach: bool,
}

/// Budget comparison for a single position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionBudgetEntry {
    /// Position identifier.
    pub position_id: PositionId,

    /// Actual component VaR from decomposition.
    pub actual_component_var: f64,

    /// Target component VaR from budget.
    pub target_component_var: f64,

    /// Utilization ratio: actual / target.
    ///
    /// Values > 1.0 indicate the position uses more risk than budgeted.
    /// Values < 1.0 indicate unused risk budget.
    pub utilization: f64,

    /// Over/under-budget amount: actual - target.
    pub excess: f64,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl RiskBudget {
    /// Create a new risk budget from target allocations.
    ///
    /// # Arguments
    ///
    /// * `targets` - Per-position target fractions of portfolio VaR.
    pub fn new(targets: IndexMap<PositionId, f64>) -> Self {
        Self {
            targets,
            utilization_threshold: 1.20,
        }
    }

    /// Set a custom utilization threshold.
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.utilization_threshold = threshold;
        self
    }

    /// Compare a decomposition result against this budget.
    ///
    /// # Arguments
    ///
    /// * `decomposition` - Actual position-level VaR decomposition.
    ///
    /// # Returns
    ///
    /// Per-position budget utilization report.
    ///
    /// # Errors
    ///
    /// Returns an error if the budget targets do not sum close to 1.0 or
    /// if the decomposition has zero portfolio VaR when targets are
    /// non-empty.
    pub fn evaluate(
        &self,
        decomposition: &PositionRiskDecomposition,
    ) -> finstack_core::Result<RiskBudgetResult> {
        self.evaluate_components(
            decomposition
                .var_contributions
                .iter()
                .map(|c| (&c.position_id, c.component_var)),
            decomposition.portfolio_var,
        )
    }

    /// Compare raw per-position component VaRs against this budget.
    ///
    /// This is the narrow API used by binding layers that do not need to
    /// materialise a full [`PositionRiskDecomposition`]. [`evaluate`] is
    /// a thin wrapper over this method.
    ///
    /// # Arguments
    ///
    /// * `components` - Iterator of `(position_id, component_var)` pairs.
    /// * `portfolio_var` - Total portfolio VaR used to convert target
    ///   fractions into levels.
    ///
    /// # Errors
    ///
    /// Returns an error if the budget targets do not sum close to 1.0.
    ///
    /// [`evaluate`]: Self::evaluate
    pub fn evaluate_components<'a, I>(
        &self,
        components: I,
        portfolio_var: f64,
    ) -> finstack_core::Result<RiskBudgetResult>
    where
        I: IntoIterator<Item = (&'a PositionId, f64)>,
    {
        // Validate that targets sum to ~1.0.
        let target_sum: f64 = self.targets.values().sum();
        if !self.targets.is_empty() && (target_sum - 1.0).abs() > 0.05 {
            return Err(finstack_core::Error::Validation(format!(
                "risk budget targets must sum to ~1.0, got {target_sum}"
            )));
        }

        let actual_by_id: IndexMap<&PositionId, f64> = components.into_iter().collect();
        let portfolio_var_magnitude = portfolio_var.abs();
        if portfolio_var_magnitude <= 1e-15
            && actual_by_id
                .values()
                .any(|component| component.abs() > 1e-15)
        {
            return Err(finstack_core::Error::Validation(
                "portfolio VaR must be non-zero when component VaR is non-zero".to_string(),
            ));
        }

        let mut positions = Vec::with_capacity(self.targets.len());
        let mut total_overbudget = 0.0;
        let mut has_breach = false;

        for (position_id, &target_frac) in &self.targets {
            let actual_component = actual_by_id.get(position_id).copied().unwrap_or(0.0).abs();

            let target_component = target_frac * portfolio_var_magnitude;

            let utilization = if target_component.abs() > 1e-15 {
                actual_component / target_component
            } else if actual_component.abs() > 1e-15 {
                // Target is zero but actual is non-zero: infinite utilization.
                f64::INFINITY
            } else {
                // Both zero.
                1.0
            };

            let excess = actual_component - target_component;
            if excess > 0.0 {
                total_overbudget += excess;
            }

            if utilization > self.utilization_threshold {
                has_breach = true;
            }

            positions.push(PositionBudgetEntry {
                position_id: position_id.clone(),
                actual_component_var: actual_component,
                target_component_var: target_component,
                utilization,
                excess,
            });
        }

        for (position_id, actual_component_signed) in &actual_by_id {
            if self.targets.contains_key(*position_id) {
                continue;
            }
            let actual_component = actual_component_signed.abs();
            if actual_component <= 1e-15 {
                continue;
            }
            let excess = actual_component;
            total_overbudget += excess;
            has_breach = true;
            positions.push(PositionBudgetEntry {
                position_id: (*position_id).clone(),
                actual_component_var: actual_component,
                target_component_var: 0.0,
                utilization: f64::INFINITY,
                excess,
            });
        }

        Ok(RiskBudgetResult {
            positions,
            total_overbudget,
            has_breach,
        })
    }

    /// Suggest weight adjustments to bring utilization closer to targets.
    ///
    /// Uses the marginal VaR gradient to compute first-order weight changes:
    /// ```text
    /// delta_w_i proportional to (target_i - actual_i) / marginal_var_i
    /// ```
    ///
    /// Returns suggested weight deltas (positive = increase, negative = decrease).
    /// Does not enforce constraints (long-only, max weight, etc.) -- that is the
    /// optimizer's job.
    ///
    /// # Errors
    ///
    /// Returns an error if the decomposition does not carry analytical
    /// marginals (e.g. was produced in historical mode). In that case the
    /// caller must either re-run decomposition in parametric mode or
    /// supply marginals via an external finite-difference pass.
    pub fn suggest_rebalance(
        &self,
        decomposition: &PositionRiskDecomposition,
    ) -> finstack_core::Result<IndexMap<PositionId, f64>> {
        let portfolio_var = decomposition.portfolio_var;

        // Build maps by position ID, carrying marginals as Option so we can
        // cleanly surface "not analytically available" instead of silently
        // substituting zero.
        let actual_by_id: IndexMap<&PositionId, (f64, Option<f64>)> = decomposition
            .var_contributions
            .iter()
            .map(|c| (&c.position_id, (c.relative_var, c.marginal_var)))
            .collect();

        let mut deltas = IndexMap::new();

        for (position_id, &target_frac) in &self.targets {
            let (actual_frac, marginal_opt) = actual_by_id
                .get(position_id)
                .copied()
                .unwrap_or((0.0, None));

            let marginal = marginal_opt.ok_or_else(|| {
                finstack_core::Error::Validation(format!(
                    "risk-budget rebalance requires marginal VaR for position {position_id}, \
                     but the decomposition does not provide it (e.g. historical method). \
                     Re-run decomposition in parametric mode or supply marginals externally."
                ))
            })?;

            let gap = target_frac - actual_frac;

            // delta_w proportional to gap / marginal_var.
            let delta = if marginal.abs() > 1e-15 && portfolio_var.abs() > 1e-15 {
                gap / marginal * portfolio_var
            } else {
                0.0
            };

            deltas.insert(position_id.clone(), delta);
        }

        Ok(deltas)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factor_model::position_risk::{
        DecompositionConfig, DecompositionMethod, ParametricPositionDecomposer,
        PositionRiskDecomposition, PositionVarContribution,
    };

    type TestResult = finstack_core::Result<()>;

    fn sample_decomposition() -> PositionRiskDecomposition {
        // Manually construct a decomposition for budget tests.
        PositionRiskDecomposition {
            portfolio_var: 100.0,
            portfolio_es: 120.0,
            confidence: 0.95,
            method: DecompositionMethod::Parametric,
            var_contributions: vec![
                PositionVarContribution {
                    position_id: PositionId::new("A"),
                    component_var: 40.0,
                    relative_var: 0.40,
                    marginal_var: Some(0.10),
                    incremental_var: None,
                },
                PositionVarContribution {
                    position_id: PositionId::new("B"),
                    component_var: 35.0,
                    relative_var: 0.35,
                    marginal_var: Some(0.09),
                    incremental_var: None,
                },
                PositionVarContribution {
                    position_id: PositionId::new("C"),
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

        let mut targets = IndexMap::new();
        targets.insert(PositionId::new("A"), 0.33);
        targets.insert(PositionId::new("B"), 0.34);
        targets.insert(PositionId::new("C"), 0.33);

        let budget = RiskBudget::new(targets);
        let result = budget.evaluate(&decomp)?;

        // Position A: actual 40/100 = 40%, target 33% => over-budget.
        let a_entry = result
            .positions
            .iter()
            .find(|e| e.position_id == "A")
            .ok_or_else(|| finstack_core::Error::Validation("Position A not found".to_string()))?;
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
            .ok_or_else(|| finstack_core::Error::Validation("Position C not found".to_string()))?;
        assert!(c_entry.excess < 0.0);
        assert!(c_entry.utilization < 1.0);

        Ok(())
    }

    #[test]
    fn risk_budget_breach_detection() -> TestResult {
        let decomp = sample_decomposition();

        let mut targets = IndexMap::new();
        targets.insert(PositionId::new("A"), 0.20); // Actual 40% vs target 20% => 200% utilization.
        targets.insert(PositionId::new("B"), 0.40);
        targets.insert(PositionId::new("C"), 0.40);

        let budget = RiskBudget::new(targets).with_threshold(1.50);
        let result = budget.evaluate(&decomp)?;

        assert!(result.has_breach, "should detect breach for position A");
        assert!(result.total_overbudget > 0.0);

        Ok(())
    }

    #[test]
    fn risk_budget_handles_negative_loss_convention_components() -> TestResult {
        let mut targets = IndexMap::new();
        targets.insert(PositionId::new("A"), 0.20);
        targets.insert(PositionId::new("B"), 0.80);

        let budget = RiskBudget::new(targets).with_threshold(1.50);
        let components = [
            (&PositionId::new("A"), -40.0),
            (&PositionId::new("B"), -60.0),
        ];
        let result = budget.evaluate_components(components, -100.0)?;

        let a_entry = result
            .positions
            .iter()
            .find(|entry| entry.position_id == "A")
            .ok_or_else(|| finstack_core::Error::Validation("Position A not found".to_string()))?;
        assert!(result.has_breach);
        assert!(result.total_overbudget > 0.0);
        assert!((a_entry.utilization - 2.0).abs() < 1e-12);
        assert!(a_entry.excess > 0.0);
        Ok(())
    }

    #[test]
    fn risk_budget_flags_unbudgeted_nonzero_positions() -> TestResult {
        let mut targets = IndexMap::new();
        targets.insert(PositionId::new("A"), 1.0);

        let budget = RiskBudget::new(targets);
        let components = [
            (&PositionId::new("A"), 80.0),
            (&PositionId::new("UNBUDGETED"), 20.0),
        ];
        let result = budget.evaluate_components(components, 100.0)?;

        let unbudgeted = result
            .positions
            .iter()
            .find(|entry| entry.position_id == "UNBUDGETED")
            .ok_or_else(|| {
                finstack_core::Error::Validation("Unbudgeted position not found".to_string())
            })?;
        assert!(result.has_breach);
        assert_eq!(unbudgeted.target_component_var, 0.0);
        assert!(unbudgeted.utilization.is_infinite());
        assert!(unbudgeted.excess > 0.0);
        Ok(())
    }

    #[test]
    fn risk_budget_rebalance_suggestion() -> TestResult {
        let decomp = sample_decomposition();

        let mut targets = IndexMap::new();
        targets.insert(PositionId::new("A"), 0.33);
        targets.insert(PositionId::new("B"), 0.34);
        targets.insert(PositionId::new("C"), 0.33);

        let budget = RiskBudget::new(targets);
        let deltas = budget.suggest_rebalance(&decomp)?;

        // Position A is over-budget => should suggest decreasing weight (negative delta).
        let delta_a = deltas.get(&PositionId::new("A")).copied().unwrap_or(0.0);
        assert!(
            delta_a < 0.0,
            "delta for A should be negative (over-budget), got {delta_a}"
        );

        // Position C is under-budget => should suggest increasing weight (positive delta).
        let delta_c = deltas.get(&PositionId::new("C")).copied().unwrap_or(0.0);
        assert!(
            delta_c > 0.0,
            "delta for C should be positive (under-budget), got {delta_c}"
        );

        Ok(())
    }

    #[test]
    fn risk_budget_rebalance_errors_on_missing_marginals() {
        // Build a historical-style decomposition (marginals = None).
        let decomp = PositionRiskDecomposition {
            portfolio_var: 100.0,
            portfolio_es: 120.0,
            confidence: 0.95,
            method: DecompositionMethod::Historical,
            var_contributions: vec![PositionVarContribution {
                position_id: PositionId::new("A"),
                component_var: 100.0,
                relative_var: 1.0,
                marginal_var: None,
                incremental_var: None,
            }],
            es_contributions: Vec::new(),
            n_positions: 1,
            euler_residual: None,
        };

        let mut targets = IndexMap::new();
        targets.insert(PositionId::new("A"), 1.0);
        let budget = RiskBudget::new(targets);

        let result = budget.suggest_rebalance(&decomp);
        assert!(
            result.is_err(),
            "suggest_rebalance must error when marginals are unavailable"
        );
    }

    #[test]
    fn risk_budget_rejects_bad_target_sum() {
        let decomp = sample_decomposition();

        let mut targets = IndexMap::new();
        targets.insert(PositionId::new("A"), 0.5);
        targets.insert(PositionId::new("B"), 0.5);
        targets.insert(PositionId::new("C"), 0.5);

        let budget = RiskBudget::new(targets);
        let result = budget.evaluate(&decomp);
        assert!(result.is_err());
    }

    #[test]
    fn risk_budget_with_real_decomposition() -> TestResult {
        // Run the full parametric decomposer then evaluate budget.
        let weights = [0.4, 0.35, 0.25];
        let covariance = [0.04, 0.01, 0.005, 0.01, 0.09, 0.02, 0.005, 0.02, 0.0625];
        let ids = [
            PositionId::new("A"),
            PositionId::new("B"),
            PositionId::new("C"),
        ];
        let config = DecompositionConfig::parametric_95();

        let decomposer = ParametricPositionDecomposer;
        let decomp = decomposer.decompose_positions(&weights, &covariance, &ids, &config)?;

        let mut targets = IndexMap::new();
        targets.insert(PositionId::new("A"), 0.33);
        targets.insert(PositionId::new("B"), 0.34);
        targets.insert(PositionId::new("C"), 0.33);

        let budget = RiskBudget::new(targets);
        let result = budget.evaluate(&decomp)?;

        assert_eq!(result.positions.len(), 3);

        // Verify utilization is computed correctly for each position.
        for entry in &result.positions {
            if entry.target_component_var.abs() > 1e-15 {
                let expected_util = entry.actual_component_var / entry.target_component_var;
                assert!(
                    (entry.utilization - expected_util).abs() < 1e-10,
                    "utilization mismatch for {}: got {}, expected {}",
                    entry.position_id,
                    entry.utilization,
                    expected_util
                );
            }
        }

        Ok(())
    }
}

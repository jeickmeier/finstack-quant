//! Normalization Engine implementation.

use crate::adjustments::types::{
    Adjustment, AdjustmentCap, AdjustmentValue, AppliedAdjustment, CapBaseMode,
    NormalizationConfig, NormalizationResult,
};
use crate::error::{Error, Result};
use crate::evaluator::StatementResult;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::math::NeumaierAccumulator;
use indexmap::IndexMap;

/// Engine for calculating normalized metrics.
pub struct NormalizationEngine;

impl NormalizationEngine {
    /// Normalize a target node across every period present in the results.
    ///
    /// Each result starts with the reported target value, applies configured
    /// adjustments in their declaration order, then adds the capped adjustment
    /// total. Fixed adjustments default to zero for periods not listed. A
    /// percentage adjustment uses its referenced node's value for the same
    /// period. Self-referential caps use either the reported base or the
    /// progressively adjusted base according to [`CapBaseMode`].
    ///
    /// # Errors
    ///
    /// Returns an evaluation error if `target_node` is absent, an adjustment's
    /// referenced or cap-base node is missing, a percentage source lacks a
    /// value for a target period, or a cap is negative. Results are returned
    /// only after all periods are processed and are sorted chronologically.
    pub fn normalize(
        results: &StatementResult,
        config: &NormalizationConfig,
    ) -> Result<Vec<NormalizationResult>> {
        let _span = tracing::info_span!(
            "statements.normalize",
            target_node = config.target_node.as_str(),
            adjustments = config.adjustments.len(),
        )
        .entered();

        let mut normalization_results = Vec::new();

        // Get the target node values
        let target_values = results.nodes.get(&config.target_node).ok_or_else(|| {
            Error::eval(format!(
                "Target node '{}' not found in results",
                config.target_node
            ))
        })?;

        // Iterate over all periods where the target node has a value
        for (period_id, &base_value) in target_values {
            let mut applied_adjustments = Vec::new();
            let mut total_acc = NeumaierAccumulator::new();

            for adjustment in &config.adjustments {
                let raw_amount = Self::calculate_adjustment_value(adjustment, *period_id, results)?;

                let running_total = total_acc.total();
                let (capped_amount, is_capped) = if let Some(cap) = &adjustment.cap {
                    Self::apply_cap(
                        cap,
                        raw_amount,
                        &config.target_node,
                        running_total,
                        *period_id,
                        results,
                    )?
                } else {
                    (raw_amount, false)
                };

                applied_adjustments.push(AppliedAdjustment {
                    adjustment_id: adjustment.id.clone(),
                    name: adjustment.name.clone(),
                    raw_amount,
                    capped_amount,
                    is_capped,
                });

                total_acc.add(capped_amount);
            }

            normalization_results.push(NormalizationResult {
                period: *period_id,
                base_value,
                adjustments: applied_adjustments,
                final_value: base_value + total_acc.total(),
            });
        }

        // Sort by period for consistent output
        normalization_results.sort_by_key(|r| r.period);

        Ok(normalization_results)
    }

    /// Merge normalization results back into the main StatementResult object as a new node.
    pub fn merge_into_results(
        results: &mut StatementResult,
        normalization_results: &[NormalizationResult],
        output_node_id: &str,
    ) {
        let mut period_map = IndexMap::new();
        for res in normalization_results {
            period_map.insert(res.period, res.final_value);
        }
        results.nodes.insert(output_node_id.to_string(), period_map);
    }

    /// Calculate the raw value of an adjustment for a specific period.
    fn calculate_adjustment_value(
        adjustment: &Adjustment,
        period_id: PeriodId,
        results: &StatementResult,
    ) -> Result<f64> {
        match &adjustment.value {
            AdjustmentValue::Fixed { amounts } => Ok(*amounts.get(&period_id).unwrap_or(&0.0)),
            AdjustmentValue::PercentageOfNode {
                node_id,
                percentage,
            } => {
                let node_values = results.nodes.get(node_id).ok_or_else(|| {
                    Error::eval(format!("Reference node '{}' not found", node_id))
                })?;

                let value = node_values.get(&period_id).ok_or_else(|| {
                    Error::eval(format!(
                        "Reference node '{}' has no value for period {}",
                        node_id, period_id
                    ))
                })?;
                Ok(value * percentage)
            }
        }
    }

    /// Apply capping logic to an adjustment amount.
    ///
    /// For self-referential caps (`base_node == target_node`), the base is
    /// resolved according to [`AdjustmentCap::base_mode`]:
    /// - `Reported` (default): the cap denominator is the **reported**
    ///   (pre-adjustment) target value. Matches the standard LBO / credit
    ///   agreement definition of e.g. "add-backs capped at 25% of reported
    ///   EBITDA" — cap room is fixed for the period and does not widen as
    ///   earlier add-backs are applied.
    /// - `Progressive`: the cap denominator is `target_value + running_total`,
    ///   so earlier adjustments widen cap room for later ones.
    ///
    /// For caps referencing a different node, the base is always that node's
    /// reported value (that node is not being adjusted in this pass).
    fn apply_cap(
        cap: &AdjustmentCap,
        raw_amount: f64,
        target_node: &str,
        running_total: f64,
        period_id: PeriodId,
        results: &StatementResult,
    ) -> Result<(f64, bool)> {
        // A negative cap value would make `|raw| > cap_limit` always true and
        // then `signum * cap_limit` flips the adjustment's sign — silently
        // turning an EBITDA add-back into a deduction. Reject it.
        if cap.value < 0.0 {
            return Err(Error::eval(format!(
                "Adjustment cap value must be non-negative; got {}. A negative cap would flip the \
                 adjustment's sign (turning an add-back into a deduction).",
                cap.value
            )));
        }
        let cap_limit = if let Some(base_node) = &cap.base_node {
            let base_values = results
                .nodes
                .get(base_node)
                .ok_or_else(|| Error::eval(format!("Cap base node '{}' not found", base_node)))?;

            let node_value = *base_values.get(&period_id).unwrap_or(&0.0);
            let effective_base = if base_node == target_node {
                match cap.base_mode {
                    CapBaseMode::Reported => node_value,
                    CapBaseMode::Progressive => node_value + running_total,
                }
            } else {
                node_value
            };
            effective_base.max(0.0) * cap.value
        } else {
            cap.value
        };

        if raw_amount.abs() > cap_limit {
            Ok((raw_amount.signum() * cap_limit, true))
        } else {
            Ok((raw_amount, false))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adjustments::types::Adjustment;
    use crate::evaluator::StatementResult;
    use finstack_quant_core::dates::PeriodId;
    use indexmap::IndexMap;

    fn mock_results() -> StatementResult {
        let mut results = StatementResult::new();
        let mut ebitda = IndexMap::new();
        ebitda.insert(PeriodId::quarter(2025, 1), 100.0);
        ebitda.insert(PeriodId::quarter(2025, 2), 120.0);
        results.nodes.insert("EBITDA".to_string(), ebitda);

        let mut revenue = IndexMap::new();
        revenue.insert(PeriodId::quarter(2025, 1), 1000.0);
        revenue.insert(PeriodId::quarter(2025, 2), 1200.0);
        results.nodes.insert("Revenue".to_string(), revenue);

        results
    }

    #[test]
    fn test_fixed_adjustment() {
        let results = mock_results();
        let mut amounts = IndexMap::new();
        amounts.insert(PeriodId::quarter(2025, 1), 10.0);
        amounts.insert(PeriodId::quarter(2025, 2), 15.0);

        let adj = Adjustment::fixed("addback1", "Addback 1", amounts);
        let config = NormalizationConfig::new("EBITDA")
            .add_adjustment(adj)
            .expect("unique adjustment id");

        let normalized = NormalizationEngine::normalize(&results, &config)
            .expect("normalization should succeed for fixed adjustment");

        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0].final_value, 110.0); // 100 + 10
        assert_eq!(normalized[1].final_value, 135.0); // 120 + 15
    }

    #[test]
    fn test_percentage_adjustment() {
        let results = mock_results();
        // 5% of Revenue
        let adj = Adjustment::percentage("perc1", "Perc 1", "Revenue", 0.05);
        let config = NormalizationConfig::new("EBITDA")
            .add_adjustment(adj)
            .expect("unique adjustment id");

        let normalized = NormalizationEngine::normalize(&results, &config)
            .expect("normalization should succeed for percentage adjustment");

        // Q1: Revenue 1000 * 0.05 = 50. EBITDA 100 + 50 = 150.
        assert_eq!(normalized[0].adjustments[0].raw_amount, 50.0);
        assert_eq!(normalized[0].final_value, 150.0);
    }

    #[test]
    fn test_percentage_adjustment_errors_when_reference_period_missing() {
        let mut results = mock_results();
        results
            .nodes
            .get_mut("Revenue")
            .expect("Revenue node should exist")
            .shift_remove(&PeriodId::quarter(2025, 2));

        let adj = Adjustment::percentage("perc1", "Perc 1", "Revenue", 0.05);
        let config = NormalizationConfig::new("EBITDA")
            .add_adjustment(adj)
            .expect("unique adjustment id");

        let err = NormalizationEngine::normalize(&results, &config)
            .expect_err("missing reference period should fail");
        assert!(err
            .to_string()
            .contains("Reference node 'Revenue' has no value for period 2025Q2"));
    }

    #[test]
    fn test_capped_adjustment() {
        let results = mock_results();
        // Synergies: 50.0 fixed. Cap: 20% of EBITDA.
        // Q1 EBITDA = 100. Cap = 20. Raw = 50. Result = 20 (Capped).
        let mut amounts = IndexMap::new();
        amounts.insert(PeriodId::quarter(2025, 1), 50.0);

        let adj = Adjustment::fixed("syn", "Synergies", amounts)
            .with_cap(Some("EBITDA".to_string()), 0.20);

        let config = NormalizationConfig::new("EBITDA")
            .add_adjustment(adj)
            .expect("unique adjustment id");
        let normalized = NormalizationEngine::normalize(&results, &config)
            .expect("normalization should succeed for capped adjustment");

        let q1 = &normalized[0];
        assert_eq!(q1.adjustments[0].raw_amount, 50.0);
        assert_eq!(q1.adjustments[0].capped_amount, 20.0);
        assert!(q1.adjustments[0].is_capped);
        assert_eq!(q1.final_value, 120.0); // 100 + 20
    }

    #[test]
    fn test_merge_into_results() {
        let mut results = mock_results();
        let mut amounts = IndexMap::new();
        amounts.insert(PeriodId::quarter(2025, 1), 10.0);
        amounts.insert(PeriodId::quarter(2025, 2), 15.0);

        let adj = Adjustment::fixed("addback1", "Addback 1", amounts);
        let config = NormalizationConfig::new("EBITDA")
            .add_adjustment(adj)
            .expect("unique adjustment id");

        let normalized = NormalizationEngine::normalize(&results, &config)
            .expect("normalization should succeed for merge test");

        NormalizationEngine::merge_into_results(&mut results, &normalized, "Adjusted EBITDA");

        assert!(results.nodes.contains_key("Adjusted EBITDA"));
        let adjusted = results
            .nodes
            .get("Adjusted EBITDA")
            .expect("Adjusted EBITDA should be added");
        assert_eq!(
            *adjusted
                .get(&PeriodId::quarter(2025, 1))
                .expect("Q1 adjusted value present"),
            110.0
        );
        assert_eq!(
            *adjusted
                .get(&PeriodId::quarter(2025, 2))
                .expect("Q2 adjusted value present"),
            135.0
        );
    }

    #[test]
    fn test_capped_adjustment_does_not_use_negative_ebitda_as_positive_cap_room() {
        let mut results = StatementResult::new();
        let mut ebitda = IndexMap::new();
        ebitda.insert(PeriodId::quarter(2025, 1), -100.0);
        results.nodes.insert("EBITDA".to_string(), ebitda);

        let mut amounts = IndexMap::new();
        amounts.insert(PeriodId::quarter(2025, 1), 50.0);

        let adj = Adjustment::fixed("syn", "Synergies", amounts)
            .with_cap(Some("EBITDA".to_string()), 0.20);
        let config = NormalizationConfig::new("EBITDA")
            .add_adjustment(adj)
            .expect("unique adjustment id");

        let normalized = NormalizationEngine::normalize(&results, &config)
            .expect("normalization should succeed for negative EBITDA case");

        let q1 = &normalized[0];
        assert_eq!(q1.adjustments[0].raw_amount, 50.0);
        assert_eq!(q1.adjustments[0].capped_amount, 0.0);
        assert!(q1.adjustments[0].is_capped);
        assert_eq!(q1.final_value, -100.0);
    }
}

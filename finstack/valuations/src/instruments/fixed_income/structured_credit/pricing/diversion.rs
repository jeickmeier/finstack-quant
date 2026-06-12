//! Diversion rule system for structured credit waterfall distributions.
//!
//! This module provides a generalized framework for defining cash flow diversions
//! based on coverage test failures or custom conditions.

use crate::instruments::common_impl::validation;
use finstack_core::Result;
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

// ============================================================================
// DIVERSION RULES
// ============================================================================

/// Condition that triggers a diversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum DiversionCondition {
    /// Triggered when a coverage test fails.
    CoverageTestFailed {
        /// ID of the coverage test.
        test_id: String,
    },
    /// Custom expression (for future expression engine integration).
    CustomExpression {
        /// Expression string.
        expr: String,
    },
    /// Always active (for testing/debugging).
    Always,
}

/// A diversion rule that redirects cash from one tier to another.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiversionRule {
    /// Unique identifier for this rule.
    pub id: String,
    /// Source tier where cash is diverted from.
    pub source_tier_id: String,
    /// Target tier where cash is diverted to.
    pub target_tier_id: String,
    /// Condition that triggers the diversion.
    pub condition: DiversionCondition,
    /// Priority order for evaluation (lower = higher priority).
    pub priority: usize,
}

impl DiversionRule {
    /// Create a new diversion rule.
    pub fn new(
        id: impl Into<String>,
        source_tier_id: impl Into<String>,
        target_tier_id: impl Into<String>,
        condition: DiversionCondition,
        priority: usize,
    ) -> Self {
        Self {
            id: id.into(),
            source_tier_id: source_tier_id.into(),
            target_tier_id: target_tier_id.into(),
            condition,
            priority,
        }
    }

    /// Create a diversion rule triggered by coverage test failure.
    pub fn on_test_failure(
        id: impl Into<String>,
        source_tier_id: impl Into<String>,
        target_tier_id: impl Into<String>,
        test_id: impl Into<String>,
        priority: usize,
    ) -> Self {
        Self::new(
            id,
            source_tier_id,
            target_tier_id,
            DiversionCondition::CoverageTestFailed {
                test_id: test_id.into(),
            },
            priority,
        )
    }

    /// Check if this rule's condition is met.
    pub fn is_active(&self, test_results: &HashMap<String, bool>) -> bool {
        match &self.condition {
            DiversionCondition::CoverageTestFailed { test_id } => test_results
                .get(test_id)
                .map(|&passed| !passed)
                .unwrap_or(false),
            // No expression engine exists; `DiversionEngine::validate` rejects
            // rules carrying this condition so it is unreachable in a
            // validated engine.
            DiversionCondition::CustomExpression { .. } => false,
            DiversionCondition::Always => true,
        }
    }
}

// ============================================================================
// DIVERSION ENGINE
// ============================================================================

/// Engine for managing and validating diversion rules.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiversionEngine {
    /// Collection of diversion rules.
    rules: Vec<DiversionRule>,
}

impl DiversionEngine {
    /// Create a new diversion engine.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a diversion rule.
    pub fn add_rule(mut self, rule: DiversionRule) -> Self {
        self.rules.push(rule);
        self.rules.sort_by_key(|r| r.priority);
        self
    }

    /// Get all rules.
    pub fn rules(&self) -> &[DiversionRule] {
        &self.rules
    }

    /// Validate the diversion rules.
    ///
    /// Checks for:
    /// - Circular references (A → B → A)
    /// - Self-referencing rules (A → A)
    /// - Duplicate rule IDs
    /// - `CustomExpression` conditions (no expression engine exists to
    ///   evaluate them; silently treating them as inactive would hide a
    ///   misconfigured deal)
    pub fn validate(&self) -> Result<()> {
        let mut seen_ids: HashSet<&str> = HashSet::default();
        for rule in &self.rules {
            validation::require_with(seen_ids.insert(&rule.id), || {
                format!("Duplicate diversion rule ID: {}", rule.id)
            })?;
        }

        for rule in &self.rules {
            if let DiversionCondition::CustomExpression { expr } = &rule.condition {
                return Err(finstack_core::Error::Validation(format!(
                    "Diversion rule '{}' uses CustomExpression ({expr:?}), but no \
                     expression engine is available to evaluate it",
                    rule.id
                )));
            }
        }

        for rule in &self.rules {
            validation::require_with(rule.source_tier_id != rule.target_tier_id, || {
                format!(
                    "Diversion rule '{}' has self-reference: {} → {}",
                    rule.id, rule.source_tier_id, rule.target_tier_id
                )
            })?;
        }

        self.detect_cycles()?;

        Ok(())
    }

    /// Detect circular dependencies in diversion rules using DFS.
    fn detect_cycles(&self) -> Result<()> {
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::default();
        let mut all_nodes: HashSet<&str> = HashSet::default();

        for rule in &self.rules {
            all_nodes.insert(rule.source_tier_id.as_str());
            all_nodes.insert(rule.target_tier_id.as_str());
            graph
                .entry(rule.source_tier_id.as_str())
                .or_default()
                .push(rule.target_tier_id.as_str());
        }

        #[derive(PartialEq)]
        enum Color {
            White,
            Gray,
            Black,
        }

        let mut colors: HashMap<&str, Color> =
            all_nodes.iter().map(|&node| (node, Color::White)).collect();

        let mut path: Vec<&str> = Vec::new();

        fn dfs_visit<'a>(
            node: &'a str,
            graph: &HashMap<&'a str, Vec<&'a str>>,
            colors: &mut HashMap<&'a str, Color>,
            path: &mut Vec<&'a str>,
        ) -> Result<()> {
            path.push(node);
            colors.insert(node, Color::Gray);

            if let Some(neighbors) = graph.get(node) {
                for &neighbor in neighbors {
                    match colors.get(neighbor) {
                        Some(Color::White) => {
                            dfs_visit(neighbor, graph, colors, path)?;
                        }
                        Some(Color::Gray) => {
                            // When neighbor is Gray, it must be in the current path
                            if let Some(cycle_start) = path.iter().position(|&n| n == neighbor) {
                                let cycle: Vec<_> = path[cycle_start..]
                                    .iter()
                                    .chain(std::iter::once(&neighbor))
                                    .map(|s| s.to_string())
                                    .collect();
                                return Err(finstack_core::Error::Validation(format!(
                                    "Circular diversion detected: {}",
                                    cycle.join(" → ")
                                )));
                            }
                        }
                        Some(Color::Black) | None => {}
                    }
                }
            }

            colors.insert(node, Color::Black);
            path.pop();
            Ok(())
        }

        for &node in &all_nodes {
            if colors.get(node) == Some(&Color::White) {
                dfs_visit(node, &graph, &mut colors, &mut path)?;
            }
        }

        Ok(())
    }

    /// Get active diversions based on test results.
    ///
    /// When multiple active rules share a source tier, the highest-priority
    /// rule (lowest `priority` value) wins; lower-priority duplicates are
    /// ignored rather than silently overwriting the senior rule.
    pub fn get_active_diversions(
        &self,
        test_results: &HashMap<String, bool>,
    ) -> HashMap<String, String> {
        let mut active: HashMap<String, String> = HashMap::default();

        // `rules` is kept sorted by ascending priority in `add_rule`.
        for rule in &self.rules {
            if rule.is_active(test_results) {
                active
                    .entry(rule.source_tier_id.clone())
                    .or_insert_with(|| rule.target_tier_id.clone());
            }
        }

        active
    }
}

impl Default for DiversionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diversion_rule_creation() {
        let rule = DiversionRule::new(
            "rule1",
            "subordinated_interest",
            "senior_interest",
            DiversionCondition::CoverageTestFailed {
                test_id: "oc_test".into(),
            },
            1,
        );

        assert_eq!(rule.id, "rule1");
        assert_eq!(rule.source_tier_id, "subordinated_interest");
        assert_eq!(rule.target_tier_id, "senior_interest");
        assert_eq!(rule.priority, 1);
    }

    #[test]
    fn test_diversion_condition_evaluation() {
        let rule = DiversionRule::on_test_failure("rule1", "tier_a", "tier_b", "oc_test", 1);

        let mut test_results = HashMap::default();

        test_results.insert("oc_test".to_string(), true);
        assert!(!rule.is_active(&test_results));

        test_results.insert("oc_test".to_string(), false);
        assert!(rule.is_active(&test_results));
    }

    #[test]
    fn test_no_cycles_valid() {
        let engine = DiversionEngine::new()
            .add_rule(DiversionRule::new(
                "rule1",
                "tier_a",
                "tier_b",
                DiversionCondition::Always,
                1,
            ))
            .add_rule(DiversionRule::new(
                "rule2",
                "tier_b",
                "tier_c",
                DiversionCondition::Always,
                2,
            ));

        assert!(engine.validate().is_ok());
    }

    #[test]
    fn test_cycle_detection_simple() {
        let engine = DiversionEngine::new()
            .add_rule(DiversionRule::new(
                "rule1",
                "tier_a",
                "tier_b",
                DiversionCondition::Always,
                1,
            ))
            .add_rule(DiversionRule::new(
                "rule2",
                "tier_b",
                "tier_a",
                DiversionCondition::Always,
                2,
            ));

        let result = engine.validate();
        assert!(result.is_err());
        let err_msg = result.expect_err("should fail validation").to_string();
        assert!(err_msg.contains("Circular diversion"));
    }

    #[test]
    fn test_self_reference_detection() {
        let engine = DiversionEngine::new().add_rule(DiversionRule::new(
            "rule1",
            "tier_a",
            "tier_a",
            DiversionCondition::Always,
            1,
        ));

        let result = engine.validate();
        assert!(result.is_err());
        let err_msg = result.expect_err("should fail validation").to_string();
        assert!(err_msg.contains("self-reference"));
    }

    #[test]
    fn test_get_active_diversions() {
        let engine = DiversionEngine::new()
            .add_rule(DiversionRule::on_test_failure(
                "rule1", "tier_a", "tier_b", "test1", 1,
            ))
            .add_rule(DiversionRule::on_test_failure(
                "rule2", "tier_c", "tier_d", "test2", 2,
            ));

        let mut test_results = HashMap::default();
        test_results.insert("test1".to_string(), false);
        test_results.insert("test2".to_string(), true);

        let active = engine.get_active_diversions(&test_results);

        assert_eq!(active.len(), 1);
        assert_eq!(active.get("tier_a"), Some(&"tier_b".to_string()));
        assert_eq!(active.get("tier_c"), None);
    }

    /// Two active rules on the same source tier: the highest-priority rule
    /// (lowest priority value) must win, regardless of insertion order.
    #[test]
    fn test_duplicate_source_highest_priority_wins() {
        let engine = DiversionEngine::new()
            .add_rule(DiversionRule::on_test_failure(
                "low_prio",
                "tier_a",
                "tier_junk",
                "test1",
                5,
            ))
            .add_rule(DiversionRule::on_test_failure(
                "high_prio",
                "tier_a",
                "tier_senior",
                "test1",
                1,
            ));

        let mut test_results = HashMap::default();
        test_results.insert("test1".to_string(), false);

        let active = engine.get_active_diversions(&test_results);
        assert_eq!(active.len(), 1);
        assert_eq!(
            active.get("tier_a"),
            Some(&"tier_senior".to_string()),
            "highest-priority rule must win on duplicate source tiers"
        );
    }

    /// `CustomExpression` has no evaluation engine; validation must reject it
    /// instead of letting it silently evaluate to inactive.
    #[test]
    fn test_custom_expression_rejected_at_validation() {
        let engine = DiversionEngine::new().add_rule(DiversionRule::new(
            "rule1",
            "tier_a",
            "tier_b",
            DiversionCondition::CustomExpression {
                expr: "oc_ratio < 1.2".into(),
            },
            1,
        ));

        let result = engine.validate();
        assert!(result.is_err());
        let err_msg = result.expect_err("should fail validation").to_string();
        assert!(err_msg.contains("CustomExpression"));
    }
}

//! Scenario node data structure.
//!
//! Each node in the scenario tree contains the full state needed for
//! structured credit valuation at that point in time and state space.

#![allow(dead_code)]

use std::fmt;

use crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry_or_panic;

/// Unique identifier for a scenario node.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub(crate) struct ScenarioNodeId(pub usize);

impl ScenarioNodeId {
    /// Get the underlying index.
    pub(crate) fn index(&self) -> usize {
        self.0
    }
}

impl fmt::Display for ScenarioNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node({})", self.0)
    }
}

/// A node in the scenario tree.
///
/// Contains all state information needed for structured credit valuation
/// at this point in time and scenario.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub(crate) struct ScenarioNode {
    /// Unique node identifier
    pub id: ScenarioNodeId,

    /// Period index (0 = root, 1 = first period, etc.)
    pub period: usize,

    /// Time in years from valuation date
    pub time: f64,

    /// Parent node (None for root)
    pub parent: Option<ScenarioNodeId>,

    /// Child nodes (empty for terminal nodes)
    pub children: Vec<ScenarioNodeId>,

    /// Probability of transitioning from parent to this node
    pub transition_probability: f64,

    /// Cumulative probability from root to this node
    pub cumulative_probability: f64,

    // === Factor state ===
    /// Systematic factor realizations at this node
    pub factor_realizations: Vec<f64>,

    // === Behavioral state ===
    /// Single monthly mortality rate (SMM) at this node
    pub smm: f64,

    /// Monthly default rate (MDR) at this node
    pub mdr: f64,

    /// Recovery rate for defaults at this node
    pub recovery_rate: f64,

    // === AssetPool state ===
    /// Remaining pool balance at this node
    pub pool_balance: f64,

    /// Burnout factor (1.0 = no burnout)
    pub burnout_factor: f64,

    /// Loan seasoning in months
    pub seasoning: u32,

    // === Cumulative statistics ===
    /// Cumulative prepayments from root to this node
    pub cumulative_prepayments: f64,

    /// Cumulative defaults from root to this node
    pub cumulative_defaults: f64,

    /// Cumulative losses (defaults × LGD)
    pub cumulative_losses: f64,

    // === Cash flows ===
    /// Principal payment at this node
    pub principal_payment: f64,

    /// Interest payment at this node
    pub interest_payment: f64,

    /// Prepayment amount at this node
    pub prepayment_amount: f64,

    /// Default amount at this node
    pub default_amount: f64,

    /// Recovery amount at this node
    pub recovery_amount: f64,
}

impl ScenarioNode {
    /// Create a new root node.
    pub(crate) fn root(initial_balance: f64, initial_seasoning: u32) -> Self {
        let recovery_rate = embedded_registry_or_panic().default_recovery_rate();
        Self {
            id: ScenarioNodeId(0),
            period: 0,
            time: 0.0,
            parent: None,
            children: Vec::new(),
            transition_probability: 1.0,
            cumulative_probability: 1.0,
            factor_realizations: vec![0.0],
            smm: 0.0,
            mdr: 0.0,
            recovery_rate,
            pool_balance: initial_balance,
            burnout_factor: 1.0,
            seasoning: initial_seasoning,
            cumulative_prepayments: 0.0,
            cumulative_defaults: 0.0,
            cumulative_losses: 0.0,
            principal_payment: 0.0,
            interest_payment: 0.0,
            prepayment_amount: 0.0,
            default_amount: 0.0,
            recovery_amount: 0.0,
        }
    }

    /// Create a child node with the given transition.
    pub(crate) fn child(
        &self,
        id: ScenarioNodeId,
        transition_prob: f64,
        factors: Vec<f64>,
        smm: f64,
        mdr: f64,
        recovery_rate: f64,
    ) -> Self {
        let dt = 1.0 / 12.0; // Monthly
        let time = self.time + dt;
        let period = self.period + 1;

        Self {
            id,
            period,
            time,
            parent: Some(self.id),
            children: Vec::new(),
            transition_probability: transition_prob,
            cumulative_probability: self.cumulative_probability * transition_prob,
            factor_realizations: factors,
            smm,
            mdr,
            recovery_rate,
            // AssetPool state updated later
            pool_balance: self.pool_balance,
            burnout_factor: self.burnout_factor,
            seasoning: self.seasoning + 1,
            cumulative_prepayments: self.cumulative_prepayments,
            cumulative_defaults: self.cumulative_defaults,
            cumulative_losses: self.cumulative_losses,
            principal_payment: 0.0,
            interest_payment: 0.0,
            prepayment_amount: 0.0,
            default_amount: 0.0,
            recovery_amount: 0.0,
        }
    }

    /// Update pool state for prepayments and defaults.
    pub(crate) fn apply_cashflows(&mut self, scheduled_principal: f64, interest_rate: f64) {
        let dt = 1.0 / 12.0;
        let balance = self.pool_balance;

        // Interest on beginning balance
        self.interest_payment = balance * interest_rate * dt;

        // Defaults happen first
        let default_amt = balance * self.mdr;
        self.default_amount = default_amt;
        self.cumulative_defaults += default_amt;

        // Loss = default × LGD
        let loss = default_amt * (1.0 - self.recovery_rate);
        self.cumulative_losses += loss;

        // Recovery (delayed in reality, simplified here)
        self.recovery_amount = default_amt * self.recovery_rate;

        // Balance after defaults
        let balance_post_default = balance - default_amt;

        // Prepayments on surviving balance
        let prepay_amt = balance_post_default * self.smm;
        self.prepayment_amount = prepay_amt;
        self.cumulative_prepayments += prepay_amt;

        // Scheduled principal
        self.principal_payment = scheduled_principal.min(balance_post_default - prepay_amt);

        // Update pool balance
        self.pool_balance = balance_post_default - prepay_amt - self.principal_payment;
    }

    /// Check if this is a terminal (leaf) node.
    pub(crate) fn is_terminal(&self) -> bool {
        self.children.is_empty()
    }

    /// Check if this is the root node.
    pub(crate) fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    /// Get total cash flow at this node (principal + interest + prepay + recovery - defaults).
    pub(crate) fn total_cashflow(&self) -> f64 {
        self.principal_payment
            + self.interest_payment
            + self.prepayment_amount
            + self.recovery_amount
    }

    /// Get loss amount at this node.
    pub(crate) fn loss(&self) -> f64 {
        self.default_amount * (1.0 - self.recovery_rate)
    }
}

/// A path through the scenario tree from root to a terminal node.
#[derive(Debug, Clone)]
pub(crate) struct ScenarioPath {
    /// Node IDs along the path (root to terminal)
    pub nodes: Vec<ScenarioNodeId>,

    /// Path probability (product of transition probabilities)
    pub probability: f64,

    /// Terminal node pool balance
    pub terminal_balance: f64,
    /// Total cumulative prepayments along the path
    pub total_prepayments: f64,
    /// Total cumulative defaults along the path
    pub total_defaults: f64,
    /// Total cumulative losses along the path
    pub total_losses: f64,
}

impl ScenarioPath {
    /// Create a path from a vector of node IDs.
    pub(crate) fn from_nodes(nodes: Vec<ScenarioNodeId>, probability: f64) -> Self {
        Self {
            nodes,
            probability,
            terminal_balance: 0.0,
            total_prepayments: 0.0,
            total_defaults: 0.0,
            total_losses: 0.0,
        }
    }

    /// Get the length of the path (number of periods).
    pub(crate) fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the path is empty.
    pub(crate) fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_node() {
        let root = ScenarioNode::root(1_000_000.0, 12);

        assert!(root.is_root());
        assert!((root.pool_balance - 1_000_000.0).abs() < 1e-10);
        assert_eq!(root.seasoning, 12);
        assert!((root.cumulative_probability - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_child_node() {
        let root = ScenarioNode::root(1_000_000.0, 12);
        let child = root.child(ScenarioNodeId(1), 0.333, vec![0.5], 0.01, 0.002, 0.40);

        assert!(!child.is_root());
        assert_eq!(child.parent, Some(ScenarioNodeId(0)));
        assert_eq!(child.period, 1);
        assert_eq!(child.seasoning, 13);
        assert!((child.cumulative_probability - 0.333).abs() < 1e-10);
    }

    #[test]
    fn test_apply_cashflows() {
        let mut node = ScenarioNode::root(1_000_000.0, 12);
        node.smm = 0.01; // 1% SMM
        node.mdr = 0.002; // 0.2% MDR
        node.recovery_rate = 0.40;

        node.apply_cashflows(10_000.0, 0.05);

        // Defaults: 1,000,000 × 0.002 = 2,000
        assert!((node.default_amount - 2_000.0).abs() < 1e-6);

        // Prepayments: (1,000,000 - 2,000) × 0.01 = 9,980
        assert!((node.prepayment_amount - 9_980.0).abs() < 1e-6);

        // Balance: 1,000,000 - 2,000 - 9,980 - 10,000 = 978,020
        assert!((node.pool_balance - 978_020.0).abs() < 1e-6);
    }

    #[test]
    fn test_scenario_path() {
        let path = ScenarioPath::from_nodes(
            vec![ScenarioNodeId(0), ScenarioNodeId(1), ScenarioNodeId(4)],
            0.333,
        );

        assert_eq!(path.len(), 3);
        assert!(!path.is_empty());
        assert!((path.probability - 0.333).abs() < 1e-10);
    }
}

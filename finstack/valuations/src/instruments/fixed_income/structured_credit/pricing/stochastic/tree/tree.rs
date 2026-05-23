//! Scenario tree data structure.
//!
//! Non-recombining tree for stochastic structured credit analysis.
//! Designed for accuracy over speed, preserving full path information.

#![allow(dead_code)]

use super::{
    config::ScenarioTreeConfig,
    node::{ScenarioNode, ScenarioNodeId, ScenarioPath},
};
use crate::correlation::factor_model::FactorSpec;
use crate::correlation::recovery::RecoverySpec;
use finstack_core::math::standard_normal_inv_cdf;
use finstack_core::HashMap;

/// Error returned by the recombining `ScenarioTree`'s dispersion / tail-risk
/// accessors.
///
/// Lattice recombination ([`ScenarioTree::merge_nodes`]) probability-averages
/// each node's path-dependent cumulative losses, collapsing the loss
/// *distribution* at every node to a conditional mean. Any dispersion or
/// tail-risk quantity read off the terminal nodes — unexpected loss, VaR,
/// expected shortfall, a loss percentile — is therefore biased toward the
/// centre. Rather than return a silently-wrong number, those accessors fail
/// with this message; tail risk must be priced in Monte Carlo mode.
const RECOMBINING_TREE_TAIL_RISK_UNAVAILABLE: &str =
    "tail-risk / dispersion metrics are unavailable from the recombining \
     scenario tree: lattice recombination collapses path-dependent loss \
     dispersion to per-node conditional means. Price with \
     PricingMode::MonteCarlo for unexpected loss, VaR, or expected shortfall.";

/// Recombining scenario tree for structured credit.
///
/// Each node in the tree represents a possible state at a point in time,
/// including prepayment behavior, default behavior, and pool state.
///
/// # Example
///
/// ```text
/// use finstack_valuations::instruments::fixed_income::structured_credit::pricing::stochastic::tree::{
///     ScenarioTree, ScenarioTreeConfig,
/// };
///
/// let config = ScenarioTreeConfig::rmbs_standard(5.0, 0.045);
/// let tree = ScenarioTree::build(&config).expect("tree build should succeed");
///
/// // Compute expected terminal pool balance (unit notional)
/// let expected_balance = tree.expected_value(|n| n.pool_balance);
/// # let _ = expected_balance;
/// ```
#[derive(Debug, Clone)]
pub(crate) struct ScenarioTree {
    /// All nodes in the tree (index 0 = root)
    nodes: Vec<ScenarioNode>,

    /// Configuration used to build the tree
    config: ScenarioTreeConfig,

    /// Indices of terminal (leaf) nodes
    terminal_indices: Vec<usize>,
}

impl ScenarioTree {
    /// Build a scenario tree from configuration.
    ///
    /// # Errors
    /// Currently infallible but may fail if configuration is invalid.
    pub(crate) fn build(config: &ScenarioTreeConfig) -> Result<Self, String> {
        let mut tree = Self {
            nodes: Vec::with_capacity(config.estimate_total_nodes()),
            config: config.clone(),
            terminal_indices: Vec::new(),
        };

        // Create root node
        let root = ScenarioNode::root(config.initial_balance, config.initial_seasoning);
        tree.nodes.push(root);

        // Build the tree using recombining trinomial logic
        tree.build_recombining_tree()?;

        // Collect terminal node indices
        tree.terminal_indices = tree
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.is_terminal())
            .map(|(i, _)| i)
            .collect();

        Ok(tree)
    }

    /// Build the tree using recombining trinomial branching.
    ///
    /// Mirrors the shared lattice geometry implemented in
    /// `crate::models::trees` to keep node growth at O(n²).
    fn build_recombining_tree(&mut self) -> Result<(), String> {
        let mut layer_map: HashMap<(usize, i32), usize> = HashMap::default();
        layer_map.insert((0, 0), 0);

        // Extract primary volatility for moment-matched transition probabilities.
        // Trinomial tree with zero drift and unit step dx=1:
        //   p_up = p_down = σ²dt / 2,  p_mid = 1 - σ²dt
        // This matches E[ΔZ] = 0 and Var[ΔZ] = σ²dt.
        // Falls back to uniform weights when σ²dt ∉ (0, 1).
        let vol = match &self.config.factor_spec {
            FactorSpec::SingleFactor { volatility, .. } => *volatility,
            FactorSpec::TwoFactor { prepay_vol, .. } => *prepay_vol,
            FactorSpec::MultiFactor { volatilities, .. } => {
                volatilities.first().copied().unwrap_or(1.0)
            }
        };
        let dt = self.config.dt();
        let vol_sq_dt = vol * vol * dt;

        for period in 0..self.config.num_periods {
            let mut current_positions: Vec<(i32, usize)> = layer_map
                .iter()
                .filter(|((p, _), _)| *p == period)
                .map(|((_, pos), &idx)| (*pos, idx))
                .collect();
            current_positions.sort_by_key(|(pos, _)| *pos);

            for (position, parent_idx) in current_positions {
                let burnout_factor = self.nodes[parent_idx].burnout_factor;
                let branch_count = self
                    .config
                    .branching
                    .branches_at_node(vol_sq_dt)
                    .clamp(1, 3);
                let deltas: Vec<i32> = match branch_count {
                    1 => vec![0],
                    2 => vec![-1, 1],
                    _ => vec![-1, 0, 1],
                };

                for (branch_idx, delta) in deltas.iter().enumerate() {
                    let factors = self.generate_factors_stateless(branch_idx, deltas.len());
                    let smm = self.conditional_smm_stateless(&factors, burnout_factor);
                    let mdr = self.conditional_mdr_stateless(&factors);
                    let recovery = self.conditional_recovery(&factors);
                    // Moment-matched trinomial probabilities (zero drift, dx = 1):
                    //   p_down = p_up = σ²dt/2,  p_mid = 1 - σ²dt
                    // Falls back to uniform when moment matching is infeasible.
                    let trans_prob = if deltas.len() == 3 && vol_sq_dt > 0.0 && vol_sq_dt < 1.0 {
                        match *delta {
                            -1 | 1 => vol_sq_dt / 2.0,
                            0 => 1.0 - vol_sq_dt,
                            _ => 1.0 / deltas.len() as f64,
                        }
                    } else {
                        1.0 / deltas.len() as f64
                    };

                    let child_id = ScenarioNodeId(self.nodes.len());
                    let mut child = self.nodes[parent_idx]
                        .child(child_id, trans_prob, factors, smm, mdr, recovery);
                    let scheduled = self.scheduled_principal(child.period, child.pool_balance);
                    child.apply_cashflows(scheduled, self.config.pool_coupon);

                    let key = (period + 1, position + delta);
                    if let Some(&existing_idx) = layer_map.get(&key) {
                        let existing_id = self.nodes[existing_idx].id;
                        self.merge_nodes(existing_idx, child);
                        self.nodes[parent_idx].children.push(existing_id);
                    } else {
                        self.nodes[parent_idx].children.push(child_id);
                        self.nodes.push(child);
                        let idx = self.nodes.len() - 1;
                        layer_map.insert((period + 1, position + delta), idx);
                    }
                }
            }
        }

        Ok(())
    }

    /// Merge an incoming node into an existing recombined lattice node.
    ///
    /// # Loss-state collapse (item 7 — KNOWN LIMITATION)
    ///
    /// Recombination probability-weights *every* node field, including
    /// `cumulative_losses`, `cumulative_defaults` and `cumulative_prepayments`.
    /// Those are **path-dependent** quantities: two paths reaching the same
    /// `(period, position)` lattice node generally have *different* cumulative
    /// loss histories. Averaging them keeps the lattice O(n²) but collapses
    /// the loss *distribution* at each node to its conditional mean.
    ///
    /// Consequence: terminal-node losses carry only the mean loss per lattice
    /// state, not the true dispersion. Any tail-risk metric read off the
    /// recombined tree — unexpected loss (UL), VaR, expected shortfall (ES) —
    /// is therefore biased LOW: the recombination has averaged away exactly
    /// the dispersion those metrics measure.
    ///
    /// A correct fix requires either a non-recombining tree (exponential node
    /// growth) or carrying a full loss *distribution* per node — a structural
    /// redesign out of scope here. Until then: **use Monte Carlo mode
    /// (`PricingMode::MonteCarlo`) for tail-risk metrics.** The MC path runs
    /// each scenario through the full waterfall and aggregates per-path
    /// losses, so it preserves the loss dispersion the tree collapses. The
    /// tree's `unexpected_loss` / `expected_shortfall` / loss `percentile`
    /// accessors are documented accordingly.
    fn merge_nodes(&mut self, target_idx: usize, incoming: ScenarioNode) {
        let target = &mut self.nodes[target_idx];
        assert_eq!(
            target.period, incoming.period,
            "recombined scenario nodes must share the same period"
        );
        assert_eq!(
            target.seasoning, incoming.seasoning,
            "recombined scenario nodes must share the same seasoning"
        );
        let total_prob = target.cumulative_probability + incoming.cumulative_probability;
        if total_prob <= f64::EPSILON {
            return;
        }

        let weight_existing = target.cumulative_probability / total_prob;
        let weight_new = incoming.cumulative_probability / total_prob;

        target.smm = target.smm * weight_existing + incoming.smm * weight_new;
        target.mdr = target.mdr * weight_existing + incoming.mdr * weight_new;
        target.recovery_rate =
            target.recovery_rate * weight_existing + incoming.recovery_rate * weight_new;
        target.pool_balance =
            target.pool_balance * weight_existing + incoming.pool_balance * weight_new;
        target.burnout_factor =
            target.burnout_factor * weight_existing + incoming.burnout_factor * weight_new;
        target.principal_payment =
            target.principal_payment * weight_existing + incoming.principal_payment * weight_new;
        target.interest_payment =
            target.interest_payment * weight_existing + incoming.interest_payment * weight_new;
        target.prepayment_amount =
            target.prepayment_amount * weight_existing + incoming.prepayment_amount * weight_new;
        target.default_amount =
            target.default_amount * weight_existing + incoming.default_amount * weight_new;
        target.recovery_amount =
            target.recovery_amount * weight_existing + incoming.recovery_amount * weight_new;
        target.cumulative_prepayments = target.cumulative_prepayments * weight_existing
            + incoming.cumulative_prepayments * weight_new;
        target.cumulative_defaults = target.cumulative_defaults * weight_existing
            + incoming.cumulative_defaults * weight_new;
        target.cumulative_losses =
            target.cumulative_losses * weight_existing + incoming.cumulative_losses * weight_new;

        if target.factor_realizations.len() == incoming.factor_realizations.len() {
            for (existing, new_val) in target
                .factor_realizations
                .iter_mut()
                .zip(incoming.factor_realizations.iter())
            {
                *existing = *existing * weight_existing + *new_val * weight_new;
            }
        }

        target.cumulative_probability = total_prob;
    }

    /// Generate factor realizations for a branch (stateless version).
    ///
    /// Uses stratified sampling to ensure good coverage of the distribution.
    fn generate_factors_stateless(&self, branch_idx: usize, num_branches: usize) -> Vec<f64> {
        // Stratified sampling: divide normal distribution into equal-probability regions
        let n = num_branches as f64;
        let p = (branch_idx as f64 + 0.5) / n; // Midpoint of each stratum

        // Use standard normal inverse CDF from core library
        let z = standard_normal_inv_cdf(p);

        // Apply factor model structure
        match &self.config.factor_spec {
            FactorSpec::SingleFactor { volatility, .. } => {
                vec![z * volatility]
            }
            FactorSpec::TwoFactor {
                prepay_vol,
                credit_vol,
                correlation,
            } => {
                // Correlated factor generation via Cholesky decomposition:
                //   z2 = ρ·z1 + √(1-ρ²)·z2_indep
                // In a 1D recombining tree the independent component is set to
                // its expected value (0), so only the systematic (correlated)
                // component is captured through the tree branching structure.
                let z2 = correlation * z;
                vec![z * prepay_vol, z2 * credit_vol]
            }
            FactorSpec::MultiFactor { volatilities, .. } => {
                // Use first volatility scaled by z
                if let Some(vol) = volatilities.first() {
                    vec![z * vol]
                } else {
                    vec![z]
                }
            }
        }
    }

    /// Compute conditional SMM given factor realizations (stateless version).
    fn conditional_smm_stateless(&self, factors: &[f64], burnout_factor: f64) -> f64 {
        let factor = factors.first().copied().unwrap_or(0.0);
        let base_smm = self.config.prepay_spec.base_smm();

        // Get correlation from configuration
        let prepay_factor_loading = self.config.correlation.prepay_factor_loading();

        // Conditional SMM using factor model
        // Log-normal factor adjustment for non-negative rates
        let smm = base_smm * (prepay_factor_loading * factor).exp();

        // Apply burnout
        let smm_with_burnout = smm * burnout_factor;

        // Clamp to valid range
        smm_with_burnout.clamp(0.0, 0.50)
    }

    /// Compute conditional MDR given factor realizations (stateless version).
    fn conditional_mdr_stateless(&self, factors: &[f64]) -> f64 {
        let factor = factors.first().copied().unwrap_or(0.0);
        let base_mdr = self.config.default_spec.base_mdr();

        // Get correlation from configuration
        let default_factor_loading = self.config.correlation.default_factor_loading();

        // Conditional MDR using factor model
        // Log-normal factor adjustment for non-negative rates
        let mdr = base_mdr * (default_factor_loading * factor).exp();

        // Clamp to valid range
        mdr.clamp(0.0, 0.50)
    }

    /// Compute conditional recovery given factor realizations.
    fn conditional_recovery(&self, factors: &[f64]) -> f64 {
        match &self.config.recovery_spec {
            RecoverySpec::Constant { rate } => *rate,
            RecoverySpec::MarketCorrelated {
                mean_recovery,
                recovery_volatility,
                factor_correlation,
            } => {
                let factor = factors.first().copied().unwrap_or(0.0);
                // Recovery moves with factor (typically negative correlation)
                let recovery = mean_recovery + factor_correlation * recovery_volatility * factor;
                recovery.clamp(0.0, 1.0)
            }
        }
    }

    /// Calculate scheduled principal for a given period.
    fn scheduled_principal(&self, period: usize, pool_balance: f64) -> f64 {
        let remaining_periods = self.config.num_periods.saturating_sub(period) + 1;
        if remaining_periods == 0 {
            return 0.0;
        }

        let r = self.config.pool_coupon / 12.0;
        if r.abs() < 1e-10 {
            return pool_balance / remaining_periods as f64;
        }

        // Level payment amount
        let payment = pool_balance * r / (1.0 - (1.0 + r).powi(-(remaining_periods as i32)));
        let interest = pool_balance * r;

        (payment - interest).max(0.0)
    }

    // === Public accessors ===

    /// Get the root node.
    pub(crate) fn root(&self) -> &ScenarioNode {
        &self.nodes[0]
    }

    /// Get a node by ID.
    pub(crate) fn node(&self, id: ScenarioNodeId) -> Option<&ScenarioNode> {
        self.nodes.get(id.0)
    }

    /// Get all nodes.
    pub(crate) fn nodes(&self) -> &[ScenarioNode] {
        &self.nodes
    }

    /// Get the number of nodes.
    pub(crate) fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Get terminal nodes.
    pub(crate) fn terminal_nodes(&self) -> impl Iterator<Item = &ScenarioNode> {
        self.terminal_indices.iter().map(move |&i| &self.nodes[i])
    }

    /// Get the number of terminal nodes.
    pub(crate) fn num_terminal_nodes(&self) -> usize {
        self.terminal_indices.len()
    }

    /// Get all paths from root to terminal nodes.
    pub(crate) fn paths(&self) -> Vec<ScenarioPath> {
        let mut paths = Vec::with_capacity(self.terminal_indices.len());

        for &terminal_idx in &self.terminal_indices {
            let mut path_nodes = Vec::new();
            let mut current_idx = terminal_idx;

            // Walk back to root
            loop {
                let node = &self.nodes[current_idx];
                path_nodes.push(node.id);

                if let Some(parent) = node.parent {
                    current_idx = parent.0;
                } else {
                    break;
                }
            }

            // Reverse to get root-to-terminal order
            path_nodes.reverse();

            // Get terminal node for statistics
            let terminal = &self.nodes[terminal_idx];

            let mut path = ScenarioPath::from_nodes(path_nodes, terminal.cumulative_probability);
            path.terminal_balance = terminal.pool_balance;
            path.total_prepayments = terminal.cumulative_prepayments;
            path.total_defaults = terminal.cumulative_defaults;
            path.total_losses = terminal.cumulative_losses;

            paths.push(path);
        }

        paths
    }

    // === Statistical methods ===

    /// Compute expected value of a function over terminal nodes.
    pub(crate) fn expected_value<F>(&self, f: F) -> f64
    where
        F: Fn(&ScenarioNode) -> f64,
    {
        let mut sum = 0.0;
        let mut total_prob = 0.0;

        for &idx in &self.terminal_indices {
            let node = &self.nodes[idx];
            sum += node.cumulative_probability * f(node);
            total_prob += node.cumulative_probability;
        }

        if total_prob > 0.0 {
            sum / total_prob
        } else {
            0.0
        }
    }

    /// Variance of a function over terminal nodes.
    ///
    /// # Errors
    ///
    /// Always errors: a variance is a dispersion metric and the recombining
    /// lattice has collapsed each node's loss distribution to a conditional
    /// mean (see [`Self::merge_nodes`]). Use Monte Carlo mode.
    pub(crate) fn variance<F>(&self, _f: F) -> Result<f64, String>
    where
        F: Fn(&ScenarioNode) -> f64,
    {
        Err(RECOMBINING_TREE_TAIL_RISK_UNAVAILABLE.to_string())
    }

    /// Percentile of a function over terminal nodes.
    ///
    /// # Errors
    ///
    /// Always errors: terminal-node values are recombination-collapsed
    /// conditional means (see [`Self::merge_nodes`]), so a percentile read off
    /// them does not reflect the true distribution. Use Monte Carlo mode.
    pub(crate) fn percentile<F>(&self, _f: F, _p: f64) -> Result<f64, String>
    where
        F: Fn(&ScenarioNode) -> f64,
    {
        Err(RECOMBINING_TREE_TAIL_RISK_UNAVAILABLE.to_string())
    }

    /// Compute expected loss.
    pub(crate) fn expected_loss(&self) -> f64 {
        self.expected_value(|n| n.cumulative_losses)
    }

    /// Compute expected prepayments.
    pub(crate) fn expected_prepayments(&self) -> f64 {
        self.expected_value(|n| n.cumulative_prepayments)
    }

    /// Compute expected defaults.
    pub(crate) fn expected_defaults(&self) -> f64 {
        self.expected_value(|n| n.cumulative_defaults)
    }

    /// Unexpected loss (loss standard deviation).
    ///
    /// # Errors
    ///
    /// Always errors: the recombining tree cannot represent loss dispersion
    /// (see [`Self::merge_nodes`] and [`RECOMBINING_TREE_TAIL_RISK_UNAVAILABLE`]).
    /// Price in Monte Carlo mode for unexpected loss.
    pub(crate) fn unexpected_loss(&self) -> Result<f64, String> {
        Err(RECOMBINING_TREE_TAIL_RISK_UNAVAILABLE.to_string())
    }

    /// Expected shortfall (CVaR) at a given confidence level.
    ///
    /// # Errors
    ///
    /// Always errors: expected shortfall is a tail-risk metric and the
    /// recombining lattice collapses the loss dispersion it measures (see
    /// [`Self::merge_nodes`]). Use Monte Carlo mode for a faithful ES.
    pub(crate) fn expected_shortfall(&self, _confidence: f64) -> Result<f64, String> {
        Err(RECOMBINING_TREE_TAIL_RISK_UNAVAILABLE.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::BranchingSpec;

    #[test]
    fn test_build_simple_tree() {
        let config = ScenarioTreeConfig::new(3, 0.25, BranchingSpec::fixed(2));
        let tree = ScenarioTree::build(&config).expect("Failed to build tree");

        let expected_nodes = (config.num_periods + 1) * (config.num_periods + 2) / 2;
        assert_eq!(tree.num_nodes(), expected_nodes);

        // Binomial tree: terminal nodes = n + 1
        assert_eq!(tree.num_terminal_nodes(), config.num_periods + 1);
    }

    #[test]
    fn test_root_properties() {
        let config = ScenarioTreeConfig::new(3, 0.25, BranchingSpec::fixed(3))
            .with_initial_balance(2_000_000.0)
            .with_initial_seasoning(12);

        let tree = ScenarioTree::build(&config).expect("Failed to build tree");
        let root = tree.root();

        assert!(root.is_root());
        assert!((root.pool_balance - 2_000_000.0).abs() < 1e-6);
        assert_eq!(root.seasoning, 12);
    }

    #[test]
    fn test_paths() {
        let config = ScenarioTreeConfig::new(2, 1.0 / 6.0, BranchingSpec::fixed(2));
        let tree = ScenarioTree::build(&config).expect("Failed to build tree");

        let paths = tree.paths();

        // Recombining tree exposes unique terminal states
        assert_eq!(paths.len(), tree.num_terminal_nodes());

        for path in &paths {
            assert_eq!(path.len(), 3);
        }

        // Probabilities should sum to 1
        let total_prob: f64 = paths.iter().map(|p| p.probability).sum();
        assert!((total_prob - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_expected_value() {
        // Use realistic parameters: 12 periods over 1 year with binary branching
        // to keep tree size manageable but balance evolution meaningful
        let config = ScenarioTreeConfig::new(6, 0.5, BranchingSpec::fixed(2));
        let tree = ScenarioTree::build(&config).expect("Failed to build tree");

        // Expected pool balance should be less than initial (due to payments)
        let expected_balance = tree.expected_value(|n| n.pool_balance);

        // With 6 periods of amortization + prepayments + defaults, balance decreases
        assert!(
            expected_balance < config.initial_balance,
            "Balance should decrease due to payments"
        );
        assert!(expected_balance >= 0.0, "Balance should not be negative");
    }

    /// The recombining tree must refuse to report a loss percentile: lattice
    /// recombination has collapsed the per-node loss distribution, so any
    /// percentile would be silently biased — it must error, not guess.
    #[test]
    fn test_percentile_errors_on_recombining_tree() {
        let config = ScenarioTreeConfig::new(3, 0.25, BranchingSpec::fixed(3));
        let tree = ScenarioTree::build(&config).expect("Failed to build tree");

        assert!(
            tree.percentile(|n| n.cumulative_losses, 0.95).is_err(),
            "recombining-tree percentile must error rather than return a \
             dispersion-collapsed value"
        );
    }

    /// Expected shortfall and unexpected loss are tail-risk metrics; the
    /// recombining tree must error rather than report dispersion-collapsed
    /// values. Expected loss (a first moment) stays available.
    #[test]
    fn test_tail_risk_errors_on_recombining_tree() {
        let config = ScenarioTreeConfig::new(3, 0.25, BranchingSpec::fixed(3));
        let tree = ScenarioTree::build(&config).expect("Failed to build tree");

        assert!(tree.expected_loss() >= 0.0, "expected loss stays available");
        assert!(
            tree.expected_shortfall(0.95).is_err(),
            "recombining-tree expected shortfall must error"
        );
        assert!(
            tree.unexpected_loss().is_err(),
            "recombining-tree unexpected loss must error"
        );
    }

    #[test]
    fn test_standard_normal_inv_cdf() {
        // Test at known quantiles using core library function
        let z_50 = standard_normal_inv_cdf(0.5);
        assert!(z_50.abs() < 0.01); // Should be close to 0

        let z_975 = standard_normal_inv_cdf(0.975);
        assert!((z_975 - 1.96).abs() < 0.01);

        let z_025 = standard_normal_inv_cdf(0.025);
        assert!((z_025 + 1.96).abs() < 0.01);
    }

    #[test]
    fn test_rmbs_standard_tree() {
        let config = ScenarioTreeConfig::rmbs_standard(0.5, 0.045);
        let tree = ScenarioTree::build(&config).expect("Failed to build RMBS tree");

        // Should have 6 monthly periods
        assert!(tree.num_terminal_nodes() > 0);

        // Expected loss (a first moment) is unbiased under recombination;
        // unexpected loss is a dispersion metric and must error.
        assert!(tree.expected_loss() >= 0.0);
        assert!(tree.unexpected_loss().is_err());
    }
}

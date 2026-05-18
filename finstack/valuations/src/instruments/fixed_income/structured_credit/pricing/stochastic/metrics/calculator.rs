//! Stochastic metrics calculator.
//!
//! Computes risk metrics from scenario trees or Monte Carlo paths.

#![allow(dead_code)]

use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::{
    ScenarioTree, ScenarioTreeConfig,
};

/// Stochastic risk metrics for structured credit.
///
/// # Tree mode vs Monte Carlo mode
///
/// The first-moment fields (`expected_loss`, `expected_defaults`,
/// `expected_prepayments`, `expected_terminal_balance`, the correlations) are
/// exact under scenario-tree recombination — averaging commutes with the
/// recombination weighting. The **tail-risk fields are `Option`**: they are
/// `Some` only when computed from a dispersion-preserving source. A recombining
/// scenario tree collapses each node's loss distribution to a conditional mean
/// (see `ScenarioTree::merge_nodes`), so [`StochasticMetricsCalculator::compute_from_tree`]
/// leaves them `None` rather than report a dispersion-collapsed value. Price in
/// Monte Carlo mode for faithful tail risk.
#[derive(Debug, Clone)]
pub(crate) struct StochasticMetrics {
    // === Loss metrics ===
    /// Expected loss (probability-weighted average)
    pub expected_loss: f64,

    /// Unexpected loss (loss standard deviation). `None` from a recombining
    /// scenario tree — see the type-level note.
    pub unexpected_loss: Option<f64>,

    /// Loss skewness. `None` from a recombining scenario tree.
    pub loss_skewness: Option<f64>,

    /// Loss kurtosis (excess). `None` from a recombining scenario tree.
    pub loss_kurtosis: Option<f64>,

    // === Tail risk metrics ===
    /// Value at Risk at 95% confidence. `None` from a recombining scenario tree.
    pub var_95: Option<f64>,

    /// Value at Risk at 99% confidence. `None` from a recombining scenario tree.
    pub var_99: Option<f64>,

    /// Expected Shortfall at 95% confidence. `None` from a recombining tree.
    pub expected_shortfall_95: Option<f64>,

    /// Expected Shortfall at 99% confidence. `None` from a recombining tree.
    pub expected_shortfall_99: Option<f64>,

    // === Behavioral metrics ===
    /// Expected prepayment amount
    pub expected_prepayments: f64,

    /// Expected default amount
    pub expected_defaults: f64,

    /// Expected recovery amount
    pub expected_recoveries: f64,

    /// Expected terminal pool balance
    pub expected_terminal_balance: f64,

    // === Correlation metrics ===
    /// Prepay-default correlation (implied from scenarios)
    pub implied_prepay_default_correlation: f64,

    /// Loss-factor correlation
    pub loss_factor_correlation: f64,

    // === Scenario statistics ===
    /// Number of terminal scenarios
    pub num_scenarios: usize,

    /// Minimum loss across scenarios
    pub min_loss: f64,

    /// Maximum loss across scenarios
    pub max_loss: f64,
}

impl StochasticMetrics {
    /// Create metrics with all values set to zero (tail-risk fields `None`).
    pub(crate) fn zero() -> Self {
        Self {
            expected_loss: 0.0,
            unexpected_loss: None,
            loss_skewness: None,
            loss_kurtosis: None,
            var_95: None,
            var_99: None,
            expected_shortfall_95: None,
            expected_shortfall_99: None,
            expected_prepayments: 0.0,
            expected_defaults: 0.0,
            expected_recoveries: 0.0,
            expected_terminal_balance: 0.0,
            implied_prepay_default_correlation: 0.0,
            loss_factor_correlation: 0.0,
            num_scenarios: 0,
            min_loss: 0.0,
            max_loss: 0.0,
        }
    }

    /// Get loss ratio (EL / (EL + Expected Terminal Balance)).
    pub(crate) fn loss_ratio(&self) -> f64 {
        let total = self.expected_loss + self.expected_terminal_balance;
        if total > 1e-10 {
            self.expected_loss / total
        } else {
            0.0
        }
    }

    /// Coefficient of variation of loss (UL / EL).
    ///
    /// `None` when unexpected loss is unavailable (recombining-tree metrics)
    /// or when expected loss is effectively zero.
    pub(crate) fn loss_cv(&self) -> Option<f64> {
        let ul = self.unexpected_loss?;
        if self.expected_loss > 1e-10 {
            Some(ul / self.expected_loss)
        } else {
            None
        }
    }

    /// Get loss severity (EL / Expected Defaults).
    pub(crate) fn loss_severity(&self) -> f64 {
        if self.expected_defaults > 1e-10 {
            (self.expected_defaults - self.expected_recoveries) / self.expected_defaults
        } else {
            0.0
        }
    }
}

/// Calculator for stochastic risk metrics.
pub(crate) struct StochasticMetricsCalculator {
    notional: f64,
}

impl StochasticMetricsCalculator {
    /// Create a new metrics calculator.
    pub(crate) fn new(notional: f64) -> Self {
        Self {
            notional: notional.max(1.0),
        }
    }

    /// Compute metrics from a scenario tree.
    ///
    /// # Tail-risk fields are `None` (item 7)
    ///
    /// The recombining scenario tree averages path-dependent cumulative losses
    /// at each lattice node, collapsing the loss distribution to per-node
    /// conditional means. A variance / VaR / ES / skew / kurtosis read off the
    /// terminal nodes would therefore be biased toward the centre. Rather than
    /// return a silently-wrong number, the tail-risk fields of the returned
    /// [`StochasticMetrics`] (`unexpected_loss`, `var_95`, `var_99`,
    /// `expected_shortfall_95`, `expected_shortfall_99`, `loss_skewness`,
    /// `loss_kurtosis`) are left `None`. The expected-value fields are exact —
    /// averaging commutes with the recombination weighting for first moments.
    ///
    /// For tail risk, price in Monte Carlo mode, which runs each scenario
    /// through the full waterfall and aggregates per-path losses.
    pub(crate) fn compute_from_tree(&self, tree: &ScenarioTree) -> StochasticMetrics {
        let n = tree.num_terminal_nodes();
        if n == 0 {
            return StochasticMetrics::zero();
        }

        // Collect terminal node data
        let mut losses: Vec<(f64, f64)> = Vec::with_capacity(n);
        let mut prepayments: Vec<(f64, f64)> = Vec::with_capacity(n);
        let mut defaults: Vec<(f64, f64)> = Vec::with_capacity(n);
        let mut balances: Vec<(f64, f64)> = Vec::with_capacity(n);

        for node in tree.terminal_nodes() {
            let prob = node.cumulative_probability;
            let loss = node.cumulative_losses * self.notional;
            let prepay = node.cumulative_prepayments * self.notional;
            let default = node.cumulative_defaults * self.notional;
            let balance = node.pool_balance * self.notional;

            losses.push((loss, prob));
            prepayments.push((prepay, prob));
            defaults.push((default, prob));
            balances.push((balance, prob));
        }

        // Normalize probabilities
        let total_prob: f64 = losses.iter().map(|(_, p)| p).sum();
        if total_prob < 1e-10 {
            return StochasticMetrics::zero();
        }

        // Compute expected values
        let expected_loss = self.weighted_mean(&losses, total_prob);
        let expected_prepayments = self.weighted_mean(&prepayments, total_prob);
        let expected_defaults = self.weighted_mean(&defaults, total_prob);
        let expected_terminal_balance = self.weighted_mean(&balances, total_prob);

        // Tail-risk metrics are deliberately NOT computed from the tree:
        // recombination has collapsed each node's loss distribution to a
        // conditional mean, so a variance / VaR / ES / skew / kurtosis read off
        // the terminal nodes would be biased toward the centre. They are left
        // `None` — price in Monte Carlo mode for faithful tail risk.

        // Compute correlations
        let implied_corr = self.compute_implied_correlation(&prepayments, &defaults, total_prob);
        let loss_factor_corr = self.compute_loss_factor_correlation(tree);

        // Min/max loss
        let min_loss = losses.iter().map(|(l, _)| *l).fold(f64::INFINITY, f64::min);
        let max_loss = losses
            .iter()
            .map(|(l, _)| *l)
            .fold(f64::NEG_INFINITY, f64::max);

        // Expected recoveries
        let expected_recoveries = expected_defaults * (1.0 - self.compute_avg_lgd(tree));

        StochasticMetrics {
            expected_loss,
            unexpected_loss: None,
            loss_skewness: None,
            loss_kurtosis: None,
            var_95: None,
            var_99: None,
            expected_shortfall_95: None,
            expected_shortfall_99: None,
            expected_prepayments,
            expected_defaults,
            expected_recoveries,
            expected_terminal_balance,
            implied_prepay_default_correlation: implied_corr,
            loss_factor_correlation: loss_factor_corr,
            num_scenarios: n,
            min_loss,
            max_loss,
        }
    }

    /// Compute metrics from configuration (builds tree internally).
    pub(crate) fn compute_from_config(
        &self,
        config: &ScenarioTreeConfig,
    ) -> Result<StochasticMetrics, String> {
        let tree = ScenarioTree::build(config)?;
        Ok(self.compute_from_tree(&tree))
    }

    // === Private helpers ===

    fn weighted_mean(&self, values: &[(f64, f64)], total_prob: f64) -> f64 {
        values.iter().map(|(v, p)| v * p).sum::<f64>() / total_prob
    }

    fn weighted_variance(&self, values: &[(f64, f64)], mean: f64, total_prob: f64) -> f64 {
        values
            .iter()
            .map(|(v, p)| (v - mean).powi(2) * p)
            .sum::<f64>()
            / total_prob
    }

    fn compute_higher_moments(
        &self,
        values: &[(f64, f64)],
        mean: f64,
        std_dev: f64,
        total_prob: f64,
    ) -> (f64, f64) {
        if std_dev < 1e-10 {
            return (0.0, 0.0);
        }

        let mut m3 = 0.0;
        let mut m4 = 0.0;

        for (v, p) in values {
            let z = (v - mean) / std_dev;
            m3 += z.powi(3) * p;
            m4 += z.powi(4) * p;
        }

        let skewness = m3 / total_prob;
        let kurtosis = m4 / total_prob - 3.0; // Excess kurtosis

        (skewness, kurtosis)
    }

    fn compute_var(&self, values: &[(f64, f64)], confidence: f64) -> f64 {
        // Sort by loss value
        let mut sorted: Vec<(f64, f64)> = values.to_vec();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let total_prob: f64 = sorted.iter().map(|(_, p)| p).sum();
        let target = confidence * total_prob;

        let mut cumulative = 0.0;
        let mut last_loss = 0.0;
        for (loss, prob) in &sorted {
            last_loss = *loss;
            cumulative += prob;
            if cumulative >= target {
                return *loss;
            }
        }

        last_loss
    }

    /// Expected shortfall at `confidence`, computed consistently with
    /// [`Self::compute_var`].
    ///
    /// Item 8 — on a coarse discrete loss distribution the independent
    /// "average the worst `(1−c)` probability mass" estimator can land its
    /// tail boundary on a different discrete loss point than the VaR quantile,
    /// producing `ES < VaR` — an impossible ordering for a coherent risk pair.
    ///
    /// This computes ES as the probability-weighted mean of all losses
    /// `≥ VaR(c)`. By construction every loss in that set is `≥ VaR`, so the
    /// mean is `≥ VaR` — the `ES ≥ VaR` coherence inequality holds for any
    /// discretization. (The VaR loss point itself is included so the tail set
    /// is never empty.)
    fn compute_expected_shortfall(&self, values: &[(f64, f64)], confidence: f64) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        // Anchor ES to the SAME quantile VaR reports, then average losses at
        // or beyond it. This guarantees ES ≥ VaR on any discrete CDF.
        let var = self.compute_var(values, confidence);

        let mut tail_sum = 0.0;
        let mut tail_weight = 0.0;
        for (loss, prob) in values {
            if *loss >= var {
                tail_sum += loss * prob;
                tail_weight += prob;
            }
        }

        if tail_weight > 1e-10 {
            tail_sum / tail_weight
        } else {
            // Degenerate: no mass at/above VaR — fall back to VaR itself so
            // the coherence inequality still holds.
            var
        }
    }

    fn compute_implied_correlation(
        &self,
        prepayments: &[(f64, f64)],
        defaults: &[(f64, f64)],
        total_prob: f64,
    ) -> f64 {
        if prepayments.len() != defaults.len() || prepayments.is_empty() {
            return 0.0;
        }

        let mean_prepay = self.weighted_mean(prepayments, total_prob);
        let mean_default = self.weighted_mean(defaults, total_prob);

        let var_prepay = self.weighted_variance(prepayments, mean_prepay, total_prob);
        let var_default = self.weighted_variance(defaults, mean_default, total_prob);

        if var_prepay < 1e-10 || var_default < 1e-10 {
            return 0.0;
        }

        // Compute covariance
        let covariance: f64 = prepayments
            .iter()
            .zip(defaults.iter())
            .map(|((p, prob), (d, _))| (p - mean_prepay) * (d - mean_default) * prob)
            .sum::<f64>()
            / total_prob;

        covariance / (var_prepay.sqrt() * var_default.sqrt())
    }

    fn compute_loss_factor_correlation(&self, tree: &ScenarioTree) -> f64 {
        // Compute correlation between loss and first factor
        let mut loss_sum = 0.0;
        let mut factor_sum = 0.0;
        let mut prob_sum = 0.0;

        for node in tree.terminal_nodes() {
            let factor = node.factor_realizations.first().copied().unwrap_or(0.0);
            loss_sum += node.cumulative_losses * node.cumulative_probability;
            factor_sum += factor * node.cumulative_probability;
            prob_sum += node.cumulative_probability;
        }

        if prob_sum < 1e-10 {
            return 0.0;
        }

        let mean_loss = loss_sum / prob_sum;
        let mean_factor = factor_sum / prob_sum;

        let mut var_loss = 0.0;
        let mut var_factor = 0.0;
        let mut covariance = 0.0;

        for node in tree.terminal_nodes() {
            let factor = node.factor_realizations.first().copied().unwrap_or(0.0);
            let p = node.cumulative_probability;
            var_loss += (node.cumulative_losses - mean_loss).powi(2) * p;
            var_factor += (factor - mean_factor).powi(2) * p;
            covariance += (node.cumulative_losses - mean_loss) * (factor - mean_factor) * p;
        }

        var_loss /= prob_sum;
        var_factor /= prob_sum;
        covariance /= prob_sum;

        if var_loss < 1e-10 || var_factor < 1e-10 {
            return 0.0;
        }

        covariance / (var_loss.sqrt() * var_factor.sqrt())
    }

    fn compute_avg_lgd(&self, tree: &ScenarioTree) -> f64 {
        let mut lgd_sum = 0.0;
        let mut prob_sum = 0.0;

        for node in tree.terminal_nodes() {
            lgd_sum += (1.0 - node.recovery_rate) * node.cumulative_probability;
            prob_sum += node.cumulative_probability;
        }

        if prob_sum > 1e-10 {
            lgd_sum / prob_sum
        } else {
            0.60 // Default LGD
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::BranchingSpec;

    #[test]
    fn test_metrics_zero() {
        let metrics = StochasticMetrics::zero();
        assert!((metrics.expected_loss - 0.0).abs() < 1e-10);
        assert!(metrics.unexpected_loss.is_none());
    }

    /// `compute_from_tree` returns the unbiased expected-value metrics but
    /// leaves every tail-risk field `None`: the recombining tree has collapsed
    /// the loss dispersion those metrics measure.
    #[test]
    fn test_compute_from_tree_omits_tail_risk() {
        let config = ScenarioTreeConfig::new(3, 0.25, BranchingSpec::fixed(3));
        let tree = ScenarioTree::build(&config).expect("Tree should build");

        let calc = StochasticMetricsCalculator::new(1_000_000.0);
        let metrics = calc.compute_from_tree(&tree);

        // First-moment fields remain available and sane.
        assert!(metrics.num_scenarios > 0);
        assert!(metrics.expected_loss >= 0.0);

        // Tail-risk fields are unavailable from a recombining tree.
        assert!(metrics.unexpected_loss.is_none());
        assert!(metrics.var_95.is_none());
        assert!(metrics.var_99.is_none());
        assert!(metrics.expected_shortfall_95.is_none());
        assert!(metrics.expected_shortfall_99.is_none());
        assert!(metrics.loss_skewness.is_none());
        assert!(metrics.loss_kurtosis.is_none());
    }

    /// Item 8 — ES must be ≥ VaR at the same confidence on ANY discrete loss
    /// distribution, including coarse ones where the old "average the worst
    /// (1−c) mass" estimator could place its tail boundary below the VaR
    /// quantile and report `ES < VaR`.
    #[test]
    fn expected_shortfall_is_never_below_var_on_coarse_distribution() {
        let calc = StochasticMetricsCalculator::new(1.0);

        // A deliberately coarse, lumpy loss CDF: a few discrete loss points
        // with uneven probabilities — the regime where naive quantile vs
        // tail-average estimators disagree.
        let losses: Vec<(f64, f64)> = vec![
            (0.0, 0.50),
            (10_000.0, 0.30),
            (50_000.0, 0.15),
            (500_000.0, 0.05),
        ];

        for &c in &[0.90_f64, 0.95, 0.99] {
            let var = calc.compute_var(&losses, c);
            let es = calc.compute_expected_shortfall(&losses, c);
            assert!(
                es >= var - 1e-9,
                "ES {es} must be >= VaR {var} at confidence {c} — coherence \
                 requires ES ≥ VaR for any discretization"
            );
        }

        // Single-point distribution: ES and VaR must coincide, not diverge.
        let degenerate: Vec<(f64, f64)> = vec![(123_456.0, 1.0)];
        let var = calc.compute_var(&degenerate, 0.95);
        let es = calc.compute_expected_shortfall(&degenerate, 0.95);
        assert!(
            (es - var).abs() < 1e-6,
            "on a degenerate single-point loss distribution ES ({es}) must \
             equal VaR ({var})"
        );
    }

    #[test]
    fn test_loss_ratio() {
        let mut metrics = StochasticMetrics::zero();
        metrics.expected_loss = 50_000.0;
        metrics.expected_terminal_balance = 950_000.0;

        assert!((metrics.loss_ratio() - 0.05).abs() < 1e-6);
    }

    #[test]
    fn test_loss_cv() {
        let mut metrics = StochasticMetrics::zero();
        metrics.expected_loss = 100_000.0;
        metrics.unexpected_loss = Some(50_000.0);

        let cv = metrics.loss_cv().expect("loss_cv with UL present");
        assert!((cv - 0.5).abs() < 1e-6);

        // Without unexpected loss (recombining-tree metrics) CV is unavailable.
        metrics.unexpected_loss = None;
        assert!(metrics.loss_cv().is_none());
    }

    #[test]
    fn test_compute_from_config() {
        let config = ScenarioTreeConfig::new(2, 0.167, BranchingSpec::fixed(2));
        let calc = StochasticMetricsCalculator::new(1_000_000.0);
        let metrics = calc.compute_from_config(&config);

        assert!(metrics.is_ok());
        let metrics = metrics.expect("Metrics should compute");
        assert!(metrics.num_scenarios > 0);
    }
}

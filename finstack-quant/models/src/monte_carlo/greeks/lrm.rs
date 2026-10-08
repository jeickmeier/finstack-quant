//! Likelihood Ratio Method (LRM) for Greeks.
//!
//! Computes derivatives using the likelihood ratio (score function) method.
//! Works for discontinuous payoffs where pathwise differentiation fails.
//!
//! The key insight is: E[∂f/∂θ] = E[f * ∂ln(p)/∂θ]
//!
//! # Variance blow-up for short maturities and low volatility
//!
//! The GBM score functions scale like `1/(σ√T)` (delta) and `1/σ` (vega), so
//! the estimator variance grows without bound as `T → 0` or `σ → 0` — a
//! known structural limitation of the likelihood ratio method (Glasserman
//! 2003, §7.3). For short-dated or low-vol inputs prefer CRN finite
//! differences in [`finite_diff`](crate::monte_carlo::greeks::finite_diff),
//! and always check the returned standard error
//! before trusting the point estimate.
//!
//! Reference: Glasserman (2003) - "Monte Carlo Methods in Financial Engineering", Chapter 7.
//!

use crate::monte_carlo::estimate::Estimate;
use crate::monte_carlo::results::MoneyEstimate;
use crate::monte_carlo::OnlineStats;
use serde::{Deserialize, Serialize};

/// Price with likelihood-ratio delta and vega, all estimated from one set of
/// simulated paths.
///
/// Returned by
/// [`PathDependentPricer::price_with_lrm_greeks`](crate::monte_carlo::pricer::path_dependent::PathDependentPricer::price_with_lrm_greeks).
/// The Greeks are sample means of `discounted payoff x score`, so each carries
/// its own standard error and 95% confidence interval; they share the paths of
/// `price` and are correlated with it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LrmGreeks {
    /// Discounted price estimate in the payoff currency.
    pub price: MoneyEstimate,
    /// Delta: change in price per unit change in the initial spot.
    pub delta: Estimate,
    /// Vega: change in price per one volatility point (`0.01` of annualized
    /// volatility).
    pub vega: Estimate,
}

/// Summarize per-path contributions as a mean with its sampling error.
fn estimate(stats: &OnlineStats) -> Estimate {
    Estimate::new(
        stats.mean(),
        stats.stderr(),
        stats.confidence_interval(0.05),
        stats.count(),
    )
    .with_std_dev(stats.std_dev())
}

/// Compute delta using Likelihood Ratio Method for GBM.
///
/// For GBM, when the input is the standardized terminal shock `Z`, the score
/// function for delta is:
/// ```text
/// ∂ln(p)/∂S₀ = Z / (S₀ σ √T)
/// ```
///
/// where `Z = W_T / √T`. The `1/(σ√T)` factor makes the estimator variance
/// blow up as `T → 0` or `σ → 0`; see the [module docs](self) for
/// alternatives in that regime.
///
/// # Payoff contract
///
/// The terminal score is the score of the *terminal marginal density*, so
/// this estimator is unbiased **only for payoffs that are functions of `S_T`
/// alone** (European-style). For path-dependent payoffs (Asian averages,
/// barriers, lookbacks) only the first-transition density depends on `S₀`:
/// pass the *first-step* shock together with the *first step's* Δt instead
/// (Glasserman 2003, §7.3) — this is what
/// [`PathDependentPricer::price_with_lrm_greeks`](crate::monte_carlo::pricer::path_dependent::PathDependentPricer::price_with_lrm_greeks)
/// does. Feeding terminal shocks with a path-dependent payoff silently
/// biases delta (≈ ×0.5 for a uniform-fixing Asian).
///
/// # Arguments
///
/// * `payoffs` - Payoff values from MC paths
/// * `terminal_shocks` - Standardized terminal shocks (Z = W_T / √T)
/// * `initial_spot` - Initial spot price
/// * `volatility` - Annualized GBM volatility as a positive decimal, such as
///   `0.20` for 20%.
/// * `time_to_maturity` - Time to maturity
/// * `discount_factor` - Discount factor from maturity to valuation, applied
///   to each undiscounted path payoff.
///
/// # Returns
///
/// The delta estimate with its standard error and 95% confidence interval
/// over the supplied paths.
#[must_use]
pub fn lrm_delta(
    payoffs: &[f64],
    terminal_shocks: &[f64],
    initial_spot: f64,
    volatility: f64,
    time_to_maturity: f64,
    discount_factor: f64,
) -> Estimate {
    let mut stats = OnlineStats::new();
    let sqrt_t = time_to_maturity.sqrt();
    let score_multiplier = 1.0 / (initial_spot * volatility * sqrt_t);

    for (i, &payoff) in payoffs.iter().enumerate() {
        let z_t = terminal_shocks[i];
        let score = z_t * score_multiplier;
        let delta_contribution = discount_factor * payoff * score;
        stats.update(delta_contribution);
    }

    estimate(&stats)
}

/// Compute vega from precomputed per-path log-density score sums.
///
/// For a payoff measurable with respect to the *discretized path*, the σ
/// score of the joint transition density is the sum of per-step scores
/// (Glasserman 2003, §7.3):
/// ```text
/// ∂ln(p)/∂σ = Σᵢ [(zᵢ² − 1)/σ − √Δtᵢ·zᵢ]
/// ```
/// where `zᵢ` is the standardized shock of step i. Callers compute the sum
/// per path (e.g. by reconstructing `zᵢ` from consecutive spots under exact
/// GBM stepping) and pass one score per payoff.
///
/// Returns vega scaled by 0.01 (sensitivity per 1% volatility change), with
/// its standard error and 95% confidence interval over the supplied paths.
///
/// # Caveat
///
/// The LR estimator differentiates the path *density* only. If the payoff
/// functional itself depends explicitly on σ (e.g. a barrier payoff using a
/// σ-dependent Brownian-bridge crossing probability), the `E[∂f/∂σ]` term is
/// not captured and must be handled separately.
///
/// # Arguments
///
/// * `payoffs` - Undiscounted path payoffs aligned one-for-one with
///   `path_scores`.
/// * `path_scores` - Precomputed per-path sums of volatility log-density
///   scores over all simulated transitions.
/// * `discount_factor` - Discount factor from payoff horizon to valuation,
///   applied before estimating vega.
#[must_use]
pub fn lrm_vega_from_scores(
    payoffs: &[f64],
    path_scores: &[f64],
    discount_factor: f64,
) -> Estimate {
    let mut stats = OnlineStats::new();
    for (&payoff, &score) in payoffs.iter().zip(path_scores) {
        stats.update(discount_factor * payoff * score * 0.01);
    }
    estimate(&stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compute rho (sensitivity to the flat risk-free rate) using LRM.
    ///
    /// For discounted GBM payoffs with standardized terminal shock `Z`, the score is:
    /// ```text
    /// ∂ln(p)/∂r = √T Z / σ
    /// ```
    /// so the present-value contribution becomes:
    /// ```text
    /// e^{-rT} payoff × (√T Z / σ - T)
    /// ```
    fn lrm_rho(
        payoffs: &[f64],
        terminal_shocks: &[f64],
        volatility: f64,
        time_to_maturity: f64,
        discount_factor: f64,
    ) -> (f64, f64) {
        let mut stats = OnlineStats::new();
        let drift_score_multiplier = time_to_maturity.sqrt() / volatility;

        for (i, &payoff) in payoffs.iter().enumerate() {
            let z_t = terminal_shocks[i];
            let score = drift_score_multiplier * z_t - time_to_maturity;
            let rho_contribution = discount_factor * payoff * score;
            stats.update(rho_contribution);
        }

        (stats.mean(), stats.stderr())
    }

    #[test]
    fn test_lrm_delta_basic() {
        // Simple test with fixed payoffs and Wiener paths
        let payoffs = vec![10.0, 5.0, 15.0];
        let wiener = vec![0.5, -0.3, 0.8];

        let delta = lrm_delta(&payoffs, &wiener, 100.0, 0.2, 1.0, 1.0);

        // Delta should be finite
        assert!(delta.mean.is_finite());
        assert!(delta.stderr >= 0.0);
        assert_eq!(delta.num_paths, 3);
        assert!(delta.ci_95.0 < delta.mean && delta.mean < delta.ci_95.1);
    }

    #[test]
    fn test_lrm_rho() {
        let payoffs = vec![10.0, 8.0, 12.0];
        let z_t = vec![0.5, -0.2, 0.3];

        let (rho, _) = lrm_rho(&payoffs, &z_t, 0.2, 1.0, 0.95);

        let expected = 0.95
            * ((10.0 * (0.5 / 0.2 - 1.0))
                + (8.0 * (-0.2 / 0.2 - 1.0))
                + (12.0 * (0.3 / 0.2 - 1.0)))
            / 3.0;
        assert!((rho - expected).abs() < 1e-12);
    }

    #[test]
    fn test_lrm_zero_payoffs() {
        let payoffs = vec![0.0, 0.0, 0.0];
        let wiener = vec![0.1, 0.2, 0.3];

        let delta = lrm_delta(&payoffs, &wiener, 100.0, 0.2, 1.0, 1.0);

        // Zero payoffs should give zero Greeks
        assert_eq!(delta.mean, 0.0);
        let vega = lrm_vega_from_scores(&payoffs, &wiener, 1.0);
        assert_eq!(vega.mean, 0.0);
    }
}

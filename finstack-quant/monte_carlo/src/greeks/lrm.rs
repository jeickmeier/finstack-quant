//! Likelihood Ratio Method (LRM) for Greeks.
//!
//! Computes derivatives using the likelihood ratio (score function) method.
//! Works for discontinuous payoffs where pathwise fails.
//!
//! The key insight is: E[∂f/∂θ] = E[f * ∂ln(p)/∂θ]
//!
//! Reference: Glasserman (2003) - "Monte Carlo Methods in Financial Engineering", Chapter 7

use crate::online_stats::OnlineStats;

/// Compute delta using Likelihood Ratio Method for GBM.
///
/// For GBM, when the input is the standardized terminal shock `Z`, the score
/// function for delta is:
/// ```text
/// ∂ln(p)/∂S₀ = Z / (S₀ σ √T)
/// ```
///
/// where `Z = W_T / √T`.
///
/// # Payoff contract
///
/// The terminal score is the score of the *terminal marginal density*, so
/// this estimator is unbiased **only for payoffs that are functions of `S_T`
/// alone** (European-style). For path-dependent payoffs (Asian averages,
/// barriers, lookbacks) only the first-transition density depends on `S₀`:
/// pass the *first-step* shock together with the *first step's* Δt instead
/// (Glasserman 2003, §7.3) — this is what
/// [`PathDependentPricer::price_with_lrm_greeks`](crate::pricer::path_dependent::PathDependentPricer::price_with_lrm_greeks)
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
/// (delta estimate, standard error)
#[must_use]
pub fn lrm_delta(
    payoffs: &[f64],
    terminal_shocks: &[f64],
    initial_spot: f64,
    volatility: f64,
    time_to_maturity: f64,
    discount_factor: f64,
) -> (f64, f64) {
    let mut stats = OnlineStats::new();
    let sqrt_t = time_to_maturity.sqrt();
    let score_multiplier = 1.0 / (initial_spot * volatility * sqrt_t);

    for (i, &payoff) in payoffs.iter().enumerate() {
        let z_t = terminal_shocks[i];
        let score = z_t * score_multiplier;
        let delta_contribution = discount_factor * payoff * score;
        stats.update(delta_contribution);
    }

    (stats.mean(), stats.stderr())
}

/// Compute vega using Likelihood Ratio Method.
///
/// For GBM with terminal density parameterized by σ and standardized shock `Z`,
/// the full score function is:
/// ```text
/// ∂ln(p)/∂σ = (Z² - 1) / σ - √T Z
/// ```
///
/// The first term comes from the variance dependence and the second from the
/// drift dependence `-(σ²/2)T` on σ.
///
/// Returns Vega scaled by 0.01 (sensitivity per 1% volatility change).
///
/// # Arguments
///
/// * `payoffs` - Undiscounted terminal path payoffs, aligned one-for-one with
///   `terminal_shocks`.
/// * `terminal_shocks` - Standardized terminal Brownian shocks for the same
///   paths, `Z = W_T / sqrt(T)`.
/// * `volatility` - Annualized GBM volatility as a positive decimal.
/// * `time_to_maturity` - Option maturity measured in years on the model time
///   basis.
/// * `discount_factor` - Discount factor from maturity to valuation, applied
///   to each path payoff.
///
/// # Payoff contract
///
/// Unbiased **only for payoffs that are functions of `S_T` alone**. For
/// path-dependent payoffs the score of the joint path density is the sum of
/// per-transition scores `Σᵢ [(zᵢ² − 1)/σ − √Δtᵢ·zᵢ]`; compute those sums
/// per path and use [`lrm_vega_from_scores`].
///
/// # References
///
/// Glasserman (2003), *Monte Carlo Methods in Financial Engineering*, Prop 7.3.4.
#[must_use]
pub fn lrm_vega(
    payoffs: &[f64],
    terminal_shocks: &[f64],
    volatility: f64,
    time_to_maturity: f64,
    discount_factor: f64,
) -> (f64, f64) {
    let mut stats = OnlineStats::new();
    let sqrt_t = time_to_maturity.sqrt();

    for (i, &payoff) in payoffs.iter().enumerate() {
        let z_t = terminal_shocks[i];
        let score = (z_t * z_t - 1.0) / volatility - sqrt_t * z_t;
        let vega_contribution = discount_factor * payoff * score;
        stats.update(vega_contribution * 0.01);
    }

    (stats.mean(), stats.stderr())
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
/// its standard error.
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
) -> (f64, f64) {
    let mut stats = OnlineStats::new();
    for (&payoff, &score) in payoffs.iter().zip(path_scores) {
        stats.update(discount_factor * payoff * score * 0.01);
    }
    (stats.mean(), stats.stderr())
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

        let (delta, stderr) = lrm_delta(&payoffs, &wiener, 100.0, 0.2, 1.0, 1.0);

        // Delta should be finite
        assert!(delta.is_finite());
        assert!(stderr >= 0.0);
    }

    #[test]
    fn test_lrm_vega_basic() {
        let payoffs = vec![10.0, 5.0, 15.0];
        let wiener = vec![0.5, -0.3, 0.8];

        let (vega, stderr) = lrm_vega(&payoffs, &wiener, 0.2, 1.0, 1.0);

        assert!(vega.is_finite());
        assert!(stderr >= 0.0);
    }

    #[test]
    fn test_lrm_vega_score_formula() {
        // Verify the score function algebra: score = (W_T^2 - T)/(sigma*T) - W_T
        // For W_T=1.0, sigma=0.2, T=1.0:
        //   score = (1 - 1)/(0.2*1) - 1 = 0 - 1 = -1
        let payoffs = vec![1.0];
        let wiener = vec![1.0];
        let (vega, _) = lrm_vega(&payoffs, &wiener, 0.2, 1.0, 1.0);
        // vega = 1.0 * 1.0 * (-1.0) * 0.01 = -0.01
        assert!((vega - (-0.01)).abs() < 1e-12);

        // For W_T=0.0, sigma=0.2, T=1.0:
        //   score = (0 - 1)/(0.2*1) - 0 = -5
        let payoffs2 = vec![1.0];
        let wiener2 = vec![0.0];
        let (vega2, _) = lrm_vega(&payoffs2, &wiener2, 0.2, 1.0, 1.0);
        // vega = 1.0 * 1.0 * (-5.0) * 0.01 = -0.05
        assert!((vega2 - (-0.05)).abs() < 1e-12);
    }

    #[test]
    fn test_lrm_vega_standard_normal_formula_for_nonunit_maturity() {
        let payoffs = vec![1.0];
        let z_terminal = vec![1.2];
        let sigma = 0.4;
        let maturity: f64 = 0.25;

        let (vega, _) = lrm_vega(&payoffs, &z_terminal, sigma, maturity, 1.0);

        let expected_score =
            (z_terminal[0] * z_terminal[0] - 1.0) / sigma - maturity.sqrt() * z_terminal[0];
        let expected_vega = expected_score * 0.01;

        assert!(
            (vega - expected_vega).abs() < 1e-12,
            "expected vega {} but got {}",
            expected_vega,
            vega
        );
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

        let (delta, _) = lrm_delta(&payoffs, &wiener, 100.0, 0.2, 1.0, 1.0);

        // Zero payoffs should give zero Greeks
        assert_eq!(delta, 0.0);
    }
}

//! Basket option payoffs for multi-asset derivatives.
//!
//! Provides payoffs based on combinations of multiple underlying assets:
//! - Sum: Total of all asset values
//! - Average: Arithmetic mean
//! - Max: Best-of (maximum value)
//! - Min: Worst-of (minimum value)
//!
//! Uses existing `MultiGbmProcess` for correlated multi-asset simulation.
//!
//! # Performance Note
//!
//! State keys and basket-value scratch buffers are cached at construction time
//! to avoid allocation overhead on the hot path. This is critical for
//! performance when simulating millions of paths.

use crate::traits::state_keys;
use crate::traits::PathState;
use crate::traits::Payoff;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;

/// Pre-computed state keys for basket assets.
///
/// Returns cached `&'static str` keys for zero-cost lookups on the hot path.
/// `state_keys::indexed_spot` uses static literals for the first
/// `state_keys::INDEXED_SPOT_INLINE` indices; larger indices fall back to a
/// small intern cache.
fn make_spot_keys(num_assets: usize) -> Vec<&'static str> {
    (0..num_assets).map(state_keys::indexed_spot).collect()
}

/// Type of basket aggregation for multi-asset payoffs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BasketType {
    /// Sum of all asset values
    Sum,
    /// Arithmetic average of all asset values
    Average,
    /// Maximum asset value (best-of)
    Max,
    /// Minimum asset value (worst-of)
    Min,
}

/// Basket call option payoff.
///
/// A call option where the underlying is a basket of multiple assets.
/// The basket value is computed according to the `basket_type`.
#[derive(Debug, Clone)]
pub struct BasketCall {
    /// Strike price
    pub strike: f64,
    /// Notional amount
    pub notional: f64,
    /// Type of basket aggregation
    pub basket_type: BasketType,
    /// Number of assets in the basket
    pub num_assets: usize,
    /// Step index at maturity
    pub maturity_step: usize,
    /// Currency for the payoff
    pub currency: Currency,
    /// Pre-computed state keys for asset lookup (avoids allocation on hot path)
    spot_keys: Vec<&'static str>,
    /// Reusable scratch space for terminal asset values.
    asset_values: Vec<f64>,

    // State tracking (public for testing)
    /// Terminal basket value at maturity (public for testing)
    pub terminal_basket_value: f64,
}

impl BasketCall {
    /// Create a new basket call option.
    ///
    /// # Arguments
    ///
    /// * `strike` - Strike price
    /// * `notional` - Notional amount
    /// * `basket_type` - How to aggregate asset values
    /// * `num_assets` - Number of assets in the basket
    /// * `maturity_step` - Step index at maturity
    /// * `currency` - Currency for the payoff
    pub fn new(
        strike: f64,
        notional: f64,
        basket_type: BasketType,
        num_assets: usize,
        maturity_step: usize,
        currency: Currency,
    ) -> Self {
        Self {
            strike,
            notional,
            basket_type,
            num_assets,
            maturity_step,
            currency,
            spot_keys: make_spot_keys(num_assets),
            asset_values: Vec::with_capacity(num_assets),
            terminal_basket_value: 0.0,
        }
    }

    fn compute_basket_value_for(basket_type: BasketType, asset_values: &[f64]) -> f64 {
        match basket_type {
            BasketType::Sum => asset_values.iter().sum(),
            BasketType::Average => {
                let sum: f64 = asset_values.iter().sum();
                sum / asset_values.len() as f64
            }
            BasketType::Max => asset_values
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max),
            BasketType::Min => asset_values.iter().copied().fold(f64::INFINITY, f64::min),
        }
    }

    /// Compute basket value from asset values.
    pub fn compute_basket_value(&self, asset_values: &[f64]) -> f64 {
        Self::compute_basket_value_for(self.basket_type, asset_values)
    }
}

impl Payoff for BasketCall {
    /// Process a path event at maturity.
    ///
    /// Extracts asset values from path state using pre-cached keys.
    ///
    /// # Panics
    ///
    /// Panics if any asset's state key is missing or non-finite (e.g. the
    /// process `num_assets` does not match the basket) — see
    /// [`super::require_finite_state`]. A silent 0.0 default would pin
    /// worst-of baskets to zero.
    fn on_event(&mut self, state: &mut PathState) {
        // Update terminal value if at maturity
        if state.step == self.maturity_step {
            // Extract asset values using pre-cached keys and scratch space.
            self.asset_values.clear();
            for &key in &self.spot_keys {
                let value = super::require_finite_state(state.get(key), key, state.step);
                self.asset_values.push(value);
            }

            self.terminal_basket_value =
                Self::compute_basket_value_for(self.basket_type, &self.asset_values);
        }
    }

    fn value(&self, _currency: Currency) -> Money {
        // Call payoff: max(S - K, 0) * N
        let intrinsic = (self.terminal_basket_value - self.strike).max(0.0);
        Money::new(intrinsic * self.notional, self.currency)
    }

    fn reset(&mut self) {
        self.terminal_basket_value = 0.0;
        self.asset_values.clear();
    }

    fn max_event_step(&self) -> Option<usize> {
        Some(self.maturity_step)
    }
}

/// Basket put option payoff.
///
/// A put option where the underlying is a basket of multiple assets.
#[derive(Debug, Clone)]
pub struct BasketPut {
    /// Strike price
    pub strike: f64,
    /// Notional amount
    pub notional: f64,
    /// Type of basket aggregation
    pub basket_type: BasketType,
    /// Number of assets in the basket
    pub num_assets: usize,
    /// Step index at maturity
    pub maturity_step: usize,
    /// Currency for the payoff
    pub currency: Currency,
    /// Pre-computed state keys for asset lookup (avoids allocation on hot path)
    spot_keys: Vec<&'static str>,
    /// Reusable scratch space for terminal asset values.
    asset_values: Vec<f64>,

    // State tracking (public for testing)
    /// Terminal basket value at maturity (public for testing)
    pub terminal_basket_value: f64,
}

impl BasketPut {
    /// Create a new basket put option.
    pub fn new(
        strike: f64,
        notional: f64,
        basket_type: BasketType,
        num_assets: usize,
        maturity_step: usize,
        currency: Currency,
    ) -> Self {
        Self {
            strike,
            notional,
            basket_type,
            num_assets,
            maturity_step,
            currency,
            spot_keys: make_spot_keys(num_assets),
            asset_values: Vec::with_capacity(num_assets),
            terminal_basket_value: 0.0,
        }
    }

    fn compute_basket_value_for(basket_type: BasketType, asset_values: &[f64]) -> f64 {
        match basket_type {
            BasketType::Sum => asset_values.iter().sum(),
            BasketType::Average => {
                let sum: f64 = asset_values.iter().sum();
                sum / asset_values.len() as f64
            }
            BasketType::Max => asset_values
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max),
            BasketType::Min => asset_values.iter().copied().fold(f64::INFINITY, f64::min),
        }
    }

    /// Compute basket value from asset values.
    pub fn compute_basket_value(&self, asset_values: &[f64]) -> f64 {
        Self::compute_basket_value_for(self.basket_type, asset_values)
    }
}

impl Payoff for BasketPut {
    /// Process a path event at maturity.
    ///
    /// Extracts asset values from path state using pre-cached keys.
    ///
    /// # Panics
    ///
    /// Panics if any asset's state key is missing or non-finite — see
    /// [`super::require_finite_state`]. A silent 0.0 default would make a
    /// worst-of basket put pay the full strike.
    fn on_event(&mut self, state: &mut PathState) {
        if state.step == self.maturity_step {
            // Extract asset values using pre-cached keys and scratch space.
            self.asset_values.clear();
            for &key in &self.spot_keys {
                let value = super::require_finite_state(state.get(key), key, state.step);
                self.asset_values.push(value);
            }

            self.terminal_basket_value =
                Self::compute_basket_value_for(self.basket_type, &self.asset_values);
        }
    }

    fn value(&self, _currency: Currency) -> Money {
        // Put payoff: max(K - S, 0) * N
        let intrinsic = (self.strike - self.terminal_basket_value).max(0.0);
        Money::new(intrinsic * self.notional, self.currency)
    }

    fn reset(&mut self) {
        self.terminal_basket_value = 0.0;
        self.asset_values.clear();
    }

    fn max_event_step(&self) -> Option<usize> {
        Some(self.maturity_step)
    }
}

/// Exchange option (Margrabe formula).
///
/// Option to exchange asset 2 for asset 1: max(S_1 - S_2, 0).
/// This is a special case of a basket option with exact analytical pricing.
#[derive(Debug, Clone)]
pub struct ExchangeOption {
    /// Index of first asset
    pub asset1_idx: usize,
    /// Index of second asset
    pub asset2_idx: usize,
    /// Notional amount
    pub notional: f64,
    /// Step index at maturity
    pub maturity_step: usize,
    /// Currency for the payoff
    pub currency: Currency,
    /// Pre-computed state key for asset 1 (avoids allocation on hot path)
    key1: &'static str,
    /// Pre-computed state key for asset 2 (avoids allocation on hot path)
    key2: &'static str,

    // State tracking
    terminal_s1: f64,
    terminal_s2: f64,
}

impl ExchangeOption {
    /// Create a new exchange option.
    ///
    /// # Arguments
    ///
    /// * `asset1_idx` - Index of asset to receive
    /// * `asset2_idx` - Index of asset to deliver
    /// * `notional` - Notional amount
    /// * `maturity_step` - Step index at maturity
    /// * `currency` - Currency for the payoff
    pub fn new(
        asset1_idx: usize,
        asset2_idx: usize,
        notional: f64,
        maturity_step: usize,
        currency: Currency,
    ) -> Self {
        Self {
            asset1_idx,
            asset2_idx,
            notional,
            maturity_step,
            currency,
            key1: make_spot_keys(asset1_idx + 1)[asset1_idx],
            key2: make_spot_keys(asset2_idx + 1)[asset2_idx],
            terminal_s1: 0.0,
            terminal_s2: 0.0,
        }
    }
}

impl Payoff for ExchangeOption {
    /// Process a path event at maturity.
    ///
    /// Extracts terminal values for both assets from path state using
    /// pre-cached keys.
    ///
    /// # Panics
    ///
    /// Panics if either asset's state key is missing or non-finite — see
    /// [`super::require_finite_state`].
    fn on_event(&mut self, state: &mut PathState) {
        if state.step == self.maturity_step {
            self.terminal_s1 =
                super::require_finite_state(state.get(self.key1), self.key1, state.step);
            self.terminal_s2 =
                super::require_finite_state(state.get(self.key2), self.key2, state.step);
        }
    }

    fn value(&self, _currency: Currency) -> Money {
        // Exchange payoff: max(S_1 - S_2, 0) * N
        let intrinsic = (self.terminal_s1 - self.terminal_s2).max(0.0);
        Money::new(intrinsic * self.notional, self.currency)
    }

    fn reset(&mut self) {
        self.terminal_s1 = 0.0;
        self.terminal_s2 = 0.0;
    }

    fn max_event_step(&self) -> Option<usize> {
        Some(self.maturity_step)
    }
}

/// Margrabe formula for exchange option pricing (analytical benchmark).
///
/// Computes the exact price of an option to exchange asset 2 for asset 1.
///
/// # Arguments
///
/// * `s1` - Current price of asset 1
/// * `s2` - Current price of asset 2
/// * `sigma1` - Volatility of asset 1
/// * `sigma2` - Volatility of asset 2
/// * `rho` - Correlation between assets
/// * `time_to_maturity` - Time to maturity in years
/// * `q1` - Dividend yield of asset 1
/// * `q2` - Dividend yield of asset 2
///
/// # Returns
///
/// Analytical price of the exchange option
///
/// # Formula
///
/// ```text
/// V = S_1 e^{-q_1 T} Φ(d_1) - S_2 e^{-q_2 T} Φ(d_2)
/// where:
///   σ = √(σ_1² + σ_2² - 2ρσ_1σ_2)
///   d_1 = [ln(S_1/S_2) + (q_2 - q_1 + σ²/2)T] / (σ√T)
///   d_2 = d_1 - σ√T
/// ```
#[allow(clippy::too_many_arguments)]
pub fn margrabe_exchange_option(
    s1: f64,
    s2: f64,
    sigma1: f64,
    sigma2: f64,
    rho: f64,
    time_to_maturity: f64,
    q1: f64,
    q2: f64,
) -> f64 {
    use finstack_quant_core::math::special_functions::norm_cdf;

    // Combined volatility. The algebraic expression can land at −1e−18 in
    // floating point for ρ → 1 with σ₁ ≈ σ₂; clamp so the sqrt cannot
    // produce a NaN that slips past the degenerate-vol guard below.
    let sigma_sq = (sigma1 * sigma1 + sigma2 * sigma2 - 2.0 * rho * sigma1 * sigma2).max(0.0);
    let sigma = sigma_sq.sqrt();

    // Edge case: if combined volatility is zero, return intrinsic value
    if sigma < 1e-10 {
        // No uncertainty, so value is forward intrinsic
        let df1 = (-q1 * time_to_maturity).exp();
        let df2 = (-q2 * time_to_maturity).exp();
        return (s1 * df1 - s2 * df2).max(0.0);
    }

    let sigma_sqrt_t = sigma * time_to_maturity.sqrt();

    // d1 and d2
    let ln_ratio = (s1 / s2).ln();
    let d1 = (ln_ratio + (q2 - q1 + 0.5 * sigma_sq) * time_to_maturity) / sigma_sqrt_t;
    let d2 = d1 - sigma_sqrt_t;

    // Discount factors
    let df1 = (-q1 * time_to_maturity).exp();
    let df2 = (-q2 * time_to_maturity).exp();

    // Margrabe formula
    s1 * df1 * norm_cdf(d1) - s2 * df2 * norm_cdf(d2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discretization::exact::{ExactMultiGbm, ExactMultiGbmCorrelated};
    use crate::engine::McEngine;
    use crate::process::gbm::{GbmParams, MultiGbmProcess};
    use crate::rng::philox::PhiloxRng;

    #[test]
    fn test_basket_type_sum() {
        let basket = BasketCall::new(100.0, 1.0, BasketType::Sum, 3, 1, Currency::USD);

        let values = vec![30.0, 40.0, 50.0];
        assert_eq!(basket.compute_basket_value(&values), 120.0);
    }

    #[test]
    fn test_basket_type_average() {
        let basket = BasketCall::new(100.0, 1.0, BasketType::Average, 3, 1, Currency::USD);

        let values = vec![30.0, 40.0, 50.0];
        assert_eq!(basket.compute_basket_value(&values), 40.0);
    }

    #[test]
    fn test_basket_type_max() {
        let basket = BasketCall::new(100.0, 1.0, BasketType::Max, 3, 1, Currency::USD);

        let values = vec![30.0, 50.0, 40.0];
        assert_eq!(basket.compute_basket_value(&values), 50.0);
    }

    #[test]
    fn test_basket_type_min() {
        let basket = BasketCall::new(100.0, 1.0, BasketType::Min, 3, 1, Currency::USD);

        let values = vec![30.0, 50.0, 40.0];
        assert_eq!(basket.compute_basket_value(&values), 30.0);
    }

    #[test]
    fn test_margrabe_symmetry() {
        // Exchange option should be zero when assets are identical
        let price = margrabe_exchange_option(
            100.0, 100.0, // Same spot
            0.2, 0.2, // Same vol
            1.0, // Perfect correlation
            1.0, // 1 year
            0.0, 0.0, // No dividends
        );

        // Should be approximately zero (numerical tolerance)
        assert!(price.abs() < 1e-10);
    }

    #[test]
    fn test_margrabe_basic() {
        // Basic sanity check: option to exchange cheaper for more expensive
        let price = margrabe_exchange_option(
            110.0, 100.0, // S1 > S2
            0.2, 0.2, // Same vol
            0.5, // Moderate correlation
            1.0, // 1 year
            0.0, 0.0, // No dividends
        );

        // Should be positive (in the money)
        assert!(price > 0.0);
        // Rough bound: at least intrinsic value
        assert!(price >= 10.0);
    }

    #[test]
    fn test_basket_call_prices_with_multi_gbm_engine_state_keys() {
        let engine = McEngine::builder()
            .num_paths(8)
            .uniform_grid(1.0, 1)
            .parallel(false)
            .build()
            .expect("engine should build");
        let rng = PhiloxRng::new(42);
        let process = MultiGbmProcess::new(
            vec![
                GbmParams::new(0.0, 0.0, 0.0).unwrap(),
                GbmParams::new(0.0, 0.0, 0.0).unwrap(),
            ],
            None,
        )
        .unwrap();
        let disc = ExactMultiGbm::new();
        let initial_state = vec![100.0, 120.0];
        let payoff = BasketCall::new(100.0, 1.0, BasketType::Average, 2, 1, Currency::USD);

        let result = engine
            .price(
                &rng,
                &process,
                &disc,
                &initial_state,
                &payoff,
                Currency::USD,
                1.0,
            )
            .expect("basket pricing should succeed");

        assert!((result.mean.amount() - 10.0).abs() < 1e-12);
    }

    #[test]
    fn test_exchange_option_mc_matches_margrabe_benchmark() {
        let sigma1 = 0.20;
        let sigma2 = 0.25;
        let rho = 0.35;
        let initial_state = vec![105.0, 100.0];
        let corr = vec![1.0, rho, rho, 1.0];

        let engine = McEngine::builder()
            .num_paths(25_000)
            .uniform_grid(1.0, 252)
            .parallel(false)
            .build()
            .expect("engine should build");
        let rng = PhiloxRng::new(7);
        let process = MultiGbmProcess::new(
            vec![
                GbmParams::new(0.0, 0.0, sigma1).unwrap(),
                GbmParams::new(0.0, 0.0, sigma2).unwrap(),
            ],
            Some(corr.clone()),
        )
        .unwrap();
        let disc = ExactMultiGbmCorrelated::new(&corr, 2).expect("correlated discretization");
        let payoff = ExchangeOption::new(0, 1, 1.0, 252, Currency::USD);

        let result = engine
            .price(
                &rng,
                &process,
                &disc,
                &initial_state,
                &payoff,
                Currency::USD,
                1.0,
            )
            .expect("exchange option pricing should succeed");

        let expected = margrabe_exchange_option(
            initial_state[0],
            initial_state[1],
            sigma1,
            sigma2,
            rho,
            1.0,
            0.0,
            0.0,
        );

        assert!(
            (result.mean.amount() - expected).abs() < 0.35,
            "MC exchange option price {} should stay close to Margrabe benchmark {}",
            result.mean.amount(),
            expected
        );
    }

    #[test]
    fn test_margrabe_price_decreases_with_higher_correlation() {
        let low_corr = margrabe_exchange_option(100.0, 100.0, 0.20, 0.25, -0.25, 1.0, 0.0, 0.0);
        let high_corr = margrabe_exchange_option(100.0, 100.0, 0.20, 0.25, 0.90, 1.0, 0.0, 0.0);

        assert!(
            low_corr > high_corr,
            "Exchange option value should fall as correlation rises: low={} high={}",
            low_corr,
            high_corr
        );
    }
}

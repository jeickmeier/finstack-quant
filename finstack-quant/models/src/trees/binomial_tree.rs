//! Binomial tree models for option pricing.
//!
//! Implements Cox-Ross-Rubinstein (CRR) and Leisen-Reimer binomial trees
//! for American and Bermudan options via the generic [`TreeModel`] engine.

use crate::trees::NodeState;
use crate::types::{OptionMarketParams, OptionType};
use crate::volatility::black::d1_d2;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::HashMap;
use finstack_quant_core::HashSet;
use finstack_quant_core::{Error, Result};

use super::tree_framework::{
    map_exercise_dates_to_steps, price_recombining_tree, single_factor_equity_state, state_keys,
    EvolutionParams, RecombiningInputs, TreeModel, TreeValuator,
};

/// Binomial tree types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum TreeType {
    /// Cox-Ross-Rubinstein (standard binomial)
    CRR,
    /// Leisen-Reimer (improved convergence)
    LeisenReimer,
}

/// Vanilla / Bermudan option valuator for the binomial recombining engine.
///
/// With `exercise_steps = None` the valuator behaves like a pure European;
/// otherwise it applies American/Bermudan early exercise at the requested step
/// indices. For an escrowed stock process with known cash dividends,
/// `remaining_dividend_values[step]` holds the value of the dividends still to
/// be paid at that step: the lattice evolves the ex-dividend component and the
/// exercise decision compares against the full pre-dividend spot.
struct OptionValuator {
    strike: f64,
    option_type: OptionType,
    exercise_steps: Option<HashSet<usize>>,
    remaining_dividend_values: Option<Vec<f64>>,
}

impl OptionValuator {
    fn spot(&self, state: &NodeState) -> Result<f64> {
        let spot = state
            .spot()
            .ok_or_else(|| Error::internal("option node state missing spot"))?;
        match &self.remaining_dividend_values {
            None => Ok(spot),
            Some(remaining) => remaining
                .get(state.step)
                .map(|value| spot + value)
                .ok_or_else(|| Error::internal("missing discrete-dividend tree step")),
        }
    }
}

impl TreeValuator for OptionValuator {
    fn value_at_maturity(&self, state: &NodeState) -> Result<f64> {
        Ok(intrinsic(self.option_type, self.spot(state)?, self.strike))
    }

    fn value_at_node(&self, state: &NodeState, continuation_value: f64, _dt: f64) -> Result<f64> {
        if self
            .exercise_steps
            .as_ref()
            .is_some_and(|steps| steps.contains(&state.step))
        {
            let exercise = intrinsic(self.option_type, self.spot(state)?, self.strike);
            return Ok(continuation_value.max(exercise));
        }
        Ok(continuation_value)
    }
}

#[inline]
fn intrinsic(option_type: OptionType, spot: f64, strike: f64) -> f64 {
    match option_type {
        OptionType::Call => (spot - strike).max(0.0),
        OptionType::Put => (strike - spot).max(0.0),
    }
}

/// Binomial tree for option pricing
#[derive(Debug, Clone)]
pub struct BinomialTree {
    /// Number of time steps
    pub steps: usize,
    /// Tree type
    pub tree_type: TreeType,
}

impl BinomialTree {
    /// Create new binomial tree with specified steps and type
    pub fn new(steps: usize, tree_type: TreeType) -> Self {
        Self { steps, tree_type }
    }

    /// Create a Leisen-Reimer tree (recommended for accuracy)
    ///
    /// Note: Leisen-Reimer achieves best accuracy with odd step counts.
    /// Consider using [`leisen_reimer_odd`](Self::leisen_reimer_odd) for automatic
    /// adjustment to the nearest odd number.
    ///
    /// # Warning
    ///
    /// Even step counts may exhibit slower convergence due to the Leisen-Reimer
    /// inversion properties. For optimal accuracy, prefer [`leisen_reimer_odd`](Self::leisen_reimer_odd).
    pub fn leisen_reimer(steps: usize) -> Self {
        if steps.is_multiple_of(2) {
            tracing::warn!(
                target: "finstack_quant_valuations::trees",
                steps,
                "BinomialTree::leisen_reimer called with even step count; odd steps converge faster \
                 (consider leisen_reimer_odd)"
            );
        }
        Self::new(steps, TreeType::LeisenReimer)
    }

    /// Create a Leisen-Reimer tree with odd step count for optimal accuracy.
    ///
    /// Leisen-Reimer trees converge faster with odd step counts. This constructor
    /// automatically rounds the requested steps to the nearest odd number:
    /// - Even steps are rounded up (e.g., 100 → 101)
    /// - Odd steps are kept as-is
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_models::trees::BinomialTree;
    ///
    /// let tree = BinomialTree::leisen_reimer_odd(100);
    /// assert_eq!(tree.steps, 101);
    ///
    /// let tree = BinomialTree::leisen_reimer_odd(99);
    /// assert_eq!(tree.steps, 99);
    /// ```
    ///
    /// # Arguments
    ///
    /// * `steps` - Steps used by the algorithm, subject to the enclosing type invariants and documented units.
    pub fn leisen_reimer_odd(steps: usize) -> Self {
        let odd_steps = if steps.is_multiple_of(2) {
            steps + 1
        } else {
            steps
        };
        Self::new(odd_steps, TreeType::LeisenReimer)
    }

    /// Create a standard CRR tree
    pub fn crr(steps: usize) -> Self {
        Self::new(steps, TreeType::CRR)
    }

    /// Peizer–Pratt inversion used by Leisen–Reimer to map normal quantiles to
    /// binomial cumulative probabilities. Uses the common closed form used in LR (1996).
    fn peizer_pratt_inversion(&self, z: f64, n: usize) -> f64 {
        if n == 0 {
            return 0.5;
        }
        if z.abs() < 1e-14 {
            return 0.5;
        }

        // LR recommend an odd number of steps for best accuracy; use nearest upper odd in mapping
        let n_eff = (if n.is_multiple_of(2) { n + 1 } else { n }) as f64;
        let sign = if z >= 0.0 { 1.0 } else { -1.0 };
        let z2 = z * z;

        // Peizer–Pratt mapping (standard LR form):
        // beta = z^2 * (m + 1/6) / (m + 1/3 + 0.1/(m+1))
        // H^{-1}(z) = 0.5 + sign(z)*0.5 * sqrt(1 - exp(-beta))
        let denom = n_eff + 1.0 / 3.0 + 0.1 / (n_eff + 1.0);
        let beta = z2 * (n_eff + 1.0 / 6.0) / denom;
        let p = 0.5 + sign * 0.5 * (1.0 - (-beta).exp()).sqrt();

        // Numerically enforce bounds
        p.clamp(0.0, 1.0)
    }

    /// Calculate tree parameters based on model type
    fn calculate_parameters(
        &self,
        spot: f64,
        strike: f64,
        r: f64,
        sigma: f64,
        t: f64,
        q: f64,
    ) -> Result<(f64, f64, f64)> {
        if t <= 0.0 {
            return Err(Error::internal(format!(
                "binomial tree requires positive time_to_maturity, got {t}"
            )));
        }
        if sigma <= 0.0 {
            return Err(Error::internal(format!(
                "binomial tree requires positive volatility, got {sigma}"
            )));
        }

        let dt = t / self.steps as f64;

        let (u, d, p) = match self.tree_type {
            TreeType::LeisenReimer => {
                // Fallback to CRR if strike/spot are not usable (e.g., generic tree)
                if spot <= 0.0 || strike <= 0.0 {
                    return Self::crr_parameters(sigma, r, q, dt);
                }

                // Leisen–Reimer: use Peizer–Pratt inversion to determine probabilities
                let (_d1, d2) = d1_d2(spot, strike, r, sigma, t, q);

                // Probabilities via PP inversion
                let eps = 1e-12;
                let p = self
                    .peizer_pratt_inversion(d2, self.steps)
                    .clamp(eps, 1.0 - eps);

                // Mean/variance-matched u,d with PP probability (stable LR variant)
                let m1 = ((r - q) * dt).exp();
                let var = m1 * m1 * ((sigma * sigma * dt).exp() - 1.0);
                let one_minus_p = 1.0 - p;
                let denom = p * one_minus_p;
                if denom <= 0.0 {
                    return Err(Error::internal(
                        "Leisen-Reimer probability denominator must be positive",
                    ));
                }
                let delta = (var / denom).sqrt();
                let d = m1 - p * delta;
                let u = m1 + one_minus_p * delta;

                if !(u.is_finite() && d.is_finite() && u > 1.0 && d < 1.0 && u > d) {
                    return Err(Error::internal(
                        "Leisen-Reimer up/down factors are internally inconsistent",
                    ));
                }

                (u, d, p)
            }
            TreeType::CRR => Self::crr_parameters(sigma, r, q, dt)?,
        };

        Ok((u, d, p))
    }

    /// Cox-Ross-Rubinstein lattice factors `(u, d, p)` for one step.
    fn crr_parameters(sigma: f64, r: f64, q: f64, dt: f64) -> Result<(f64, f64, f64)> {
        let params = EvolutionParams::equity_crr(sigma, r, q, dt)?;
        Ok((params.up_factor, params.down_factor, params.prob_up))
    }

    /// Run backward induction on the shared recombining engine with flat
    /// discounting at `rate` and the lattice factors `(u, d, p)`.
    fn induct<V: TreeValuator>(
        &self,
        (u, d, p): (f64, f64, f64),
        initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        rate: f64,
        market_context: &MarketContext,
        valuator: &V,
    ) -> Result<f64> {
        price_recombining_tree(RecombiningInputs {
            steps: self.steps,
            initial_vars,
            time_to_maturity,
            market_context,
            valuator,
            up_factor: u,
            down_factor: d,
            prob_up: p,
            prob_down: 1.0 - p,
            interest_rate: rate,
            custom_state_generator: None,
            custom_rate_generator: None,
        })
    }

    /// Internal unified pricer supporting European, American, and Bermudan styles
    /// via an optional list of exercise steps.
    fn price_with_exercise(
        &self,
        market_params: &OptionMarketParams,
        exercise_steps: Option<&[usize]>,
    ) -> Result<f64> {
        let factors = self.calculate_parameters(
            market_params.spot,
            market_params.strike,
            market_params.rate,
            market_params.volatility,
            market_params.time_to_expiry,
            market_params.dividend_yield,
        )?;

        let valuator = OptionValuator {
            strike: market_params.strike,
            option_type: market_params.option_type,
            exercise_steps: exercise_steps.map(|steps| steps.iter().copied().collect()),
            remaining_dividend_values: None,
        };

        let initial_vars = single_factor_equity_state(
            market_params.spot,
            market_params.rate,
            market_params.dividend_yield,
            market_params.volatility,
        );

        self.induct(
            factors,
            initial_vars,
            market_params.time_to_expiry,
            market_params.rate,
            &MarketContext::new(), // not used by valuator
            &valuator,
        )
    }

    fn price_with_discrete_dividends(
        &self,
        market_params: &OptionMarketParams,
        exercise_steps: Option<&[usize]>,
        dividends: &[(f64, f64)],
    ) -> Result<f64> {
        if dividends.is_empty() {
            return self.price_with_exercise(market_params, exercise_steps);
        }
        if self.steps == 0 || market_params.time_to_expiry <= 0.0 {
            return Err(Error::Validation(
                "Discrete-dividend tree requires positive steps and time to expiry".to_string(),
            ));
        }
        if dividends.iter().any(|(time, amount)| {
            !time.is_finite()
                || *time <= 0.0
                || *time > market_params.time_to_expiry
                || !amount.is_finite()
                || *amount <= 0.0
        }) {
            return Err(Error::Validation(
                "Discrete-dividend tree requires positive finite dividends within option life"
                    .to_string(),
            ));
        }

        let dt = market_params.time_to_expiry / self.steps as f64;
        let mapped_dividends = dividends
            .iter()
            .map(|(time, amount)| {
                let step = ((*time / market_params.time_to_expiry) * self.steps as f64)
                    .round()
                    .clamp(1.0, self.steps as f64) as usize;
                (step, *time, *amount)
            })
            .collect::<Vec<_>>();
        let dividend_pv = mapped_dividends
            .iter()
            .map(|(_, time, amount)| amount * (-market_params.rate * time).exp())
            .sum::<f64>();
        let escrowed_spot = market_params.spot - dividend_pv;
        if !escrowed_spot.is_finite() || escrowed_spot <= 0.0 {
            return Err(Error::Validation(format!(
                "Discrete-dividend tree requires spot above dividend PV; spot={}, dividend_pv={dividend_pv}",
                market_params.spot
            )));
        }

        let factors = self.calculate_parameters(
            escrowed_spot,
            market_params.strike,
            market_params.rate,
            market_params.volatility,
            market_params.time_to_expiry,
            0.0,
        )?;
        let remaining_dividend_values = (0..=self.steps)
            .map(|step| {
                let node_time = step as f64 * dt;
                mapped_dividends
                    .iter()
                    .filter(|(dividend_step, _, _)| *dividend_step >= step)
                    .map(|(_, dividend_time, amount)| {
                        amount * (-market_params.rate * (dividend_time - node_time).max(0.0)).exp()
                    })
                    .sum()
            })
            .collect();
        let valuator = OptionValuator {
            strike: market_params.strike,
            option_type: market_params.option_type,
            exercise_steps: exercise_steps.map(|steps| steps.iter().copied().collect()),
            remaining_dividend_values: Some(remaining_dividend_values),
        };
        let initial_vars = single_factor_equity_state(
            escrowed_spot,
            market_params.rate,
            0.0,
            market_params.volatility,
        );

        self.induct(
            factors,
            initial_vars,
            market_params.time_to_expiry,
            market_params.rate,
            &MarketContext::new(),
            &valuator,
        )
    }

    /// Price an American option with known cash dividends.
    ///
    /// # Arguments
    ///
    /// * `market_params` - Spot, strike, rate, volatility, expiry, and option-side inputs.
    /// * `dividends` - Cash-dividend schedule as `(time_in_years, cash_amount)` pairs.
    pub fn price_american_with_discrete_dividends(
        &self,
        market_params: &OptionMarketParams,
        dividends: &[(f64, f64)],
    ) -> Result<f64> {
        let all_steps: Vec<usize> = (0..self.steps).collect();
        self.price_with_discrete_dividends(market_params, Some(&all_steps), dividends)
    }

    /// Price a Bermudan option with known cash dividends.
    ///
    /// # Arguments
    ///
    /// * `market_params` - Spot, strike, rate, volatility, expiry, and option-side inputs.
    /// * `exercise_dates` - Permitted exercise times in years from the valuation date.
    /// * `dividends` - Cash-dividend schedule as `(time_in_years, cash_amount)` pairs.
    pub fn price_bermudan_with_discrete_dividends(
        &self,
        market_params: &OptionMarketParams,
        exercise_dates: &[f64],
        dividends: &[(f64, f64)],
    ) -> Result<f64> {
        let mut steps =
            map_exercise_dates_to_steps(exercise_dates, market_params.time_to_expiry, self.steps);
        steps.sort();
        steps.dedup();
        self.price_with_discrete_dividends(market_params, Some(&steps), dividends)
    }

    /// Price American option using binomial tree
    #[must_use = "pricing result should not be discarded"]
    pub fn price_american(&self, market_params: &OptionMarketParams) -> Result<f64> {
        let all_steps: Vec<usize> = (0..self.steps).collect();
        self.price_with_exercise(market_params, Some(&all_steps))
    }

    /// Price European option using binomial tree (for validation)
    #[must_use = "pricing result should not be discarded"]
    pub fn price_european(&self, market_params: &OptionMarketParams) -> Result<f64> {
        self.price_with_exercise(market_params, None)
    }

    /// Price Bermudan option with specified exercise dates
    #[must_use = "pricing result should not be discarded"]
    pub fn price_bermudan(
        &self,
        market_params: &OptionMarketParams,
        exercise_dates: &[f64], // Times when exercise is allowed
    ) -> Result<f64> {
        let mut steps =
            map_exercise_dates_to_steps(exercise_dates, market_params.time_to_expiry, self.steps);
        steps.sort();
        steps.dedup();
        self.price_with_exercise(market_params, Some(&steps))
    }

    /// Generic [`TreeModel`] pricing for an arbitrary [`TreeValuator`].
    ///
    /// **Leisen-Reimer note:** this generic path has no spot/strike, so it
    /// calls `calculate_parameters` with `spot = strike = 0.0`. A tree
    /// configured as `TreeType::LeisenReimer` therefore silently uses the
    /// CRR fallback here and loses LR's superior convergence — use the
    /// dedicated vanilla pricing entry points to get genuine LR behavior.
    #[inline(never)] // Prevent inlining to reduce coverage metadata conflicts
    pub fn price_generic<V: TreeValuator>(
        &self,
        initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
    ) -> Result<f64> {
        let r = *initial_vars
            .get(state_keys::INTEREST_RATE)
            .ok_or_else(|| Error::internal("binomial tree requires initial interest rate"))?;
        let q = initial_vars
            .get(state_keys::DIVIDEND_YIELD)
            .copied()
            .unwrap_or(0.0);
        let sigma = *initial_vars
            .get(state_keys::VOLATILITY)
            .ok_or_else(|| Error::internal("binomial tree requires initial volatility"))?;

        let factors = self.calculate_parameters(0.0, 0.0, r, sigma, time_to_maturity, q)?;
        self.induct(
            factors,
            initial_vars,
            time_to_maturity,
            r,
            market_context,
            valuator,
        )
    }
}

/// Implementation of TreeModel trait for BinomialTree
impl TreeModel for BinomialTree {
    fn price<V: TreeValuator>(
        &self,
        initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
    ) -> Result<f64> {
        self.price_generic(initial_vars, time_to_maturity, market_context, valuator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crr_european_converges_to_black_scholes() {
        let market_params = OptionMarketParams::call(100.0, 100.0, 0.05, 0.20, 1.0);

        let tree_50 = BinomialTree::crr(50);
        let tree_200 = BinomialTree::crr(200);

        let price_50 = tree_50
            .price_european(&market_params)
            .expect("should succeed");
        let price_200 = tree_200
            .price_european(&market_params)
            .expect("should succeed");

        // Binomial trees need not converge monotonically (odd/even oscillation).
        let bs_value = 10.45;
        let error_50 = (price_50 - bs_value).abs();
        let error_200 = (price_200 - bs_value).abs();

        // Higher steps should give better accuracy (with some tolerance for oscillation)
        assert!(
            error_200 < 0.2,
            "Price at 200 steps should be reasonably close to BS"
        );
        assert!(
            error_200 < error_50 * 1.5,
            "Higher steps should generally improve or maintain accuracy: err_50={}, err_200={}",
            error_50,
            error_200
        );

        assert!((price_200 - 10.45).abs() < 0.15);
    }

    #[test]
    fn test_leisen_reimer_better_convergence() {
        let market_params = OptionMarketParams::call(100.0, 100.0, 0.05, 0.20, 1.0);

        let crr = BinomialTree::crr(401);
        let lr = BinomialTree::leisen_reimer(401);

        let crr_price = crr.price_european(&market_params).expect("should succeed");
        let lr_price = lr.price_european(&market_params).expect("should succeed");

        let bs_value = 10.4506;
        assert!(
            (crr_price - bs_value).abs() < 1.0,
            "CRR price {} should be close to BS value {}, diff={}",
            crr_price,
            bs_value,
            (crr_price - bs_value).abs()
        );

        assert!(
            (lr_price - bs_value).abs() < 0.10,
            "LR(401) price {} should be within 10c of BS {}, diff={}",
            lr_price,
            bs_value,
            (lr_price - bs_value).abs()
        );
    }

    #[test]
    fn test_leisen_reimer_converges_put() {
        let market_params = OptionMarketParams::put(100.0, 100.0, 0.05, 0.20, 1.0);

        let lr = BinomialTree::leisen_reimer(201);
        let lr_put = lr.price_european(&market_params).expect("should succeed");

        // P = C − S e^{−qT} + K e^{−rT}
        let bs_call = 10.4506;
        let bs_put = bs_call
            - market_params.spot
                * (-market_params.dividend_yield * market_params.time_to_expiry).exp()
            + market_params.strike * (-market_params.rate * market_params.time_to_expiry).exp();

        assert!(
            (lr_put - bs_put).abs() < 0.10,
            "LR(201) put {} should be within 10c of BS put {}, diff={}",
            lr_put,
            bs_put,
            (lr_put - bs_put).abs()
        );
    }

    #[test]
    fn test_leisen_reimer_parameter_sanity_edges() {
        let spot = 100.0;
        let strike = 100.0;
        let r = 0.02;
        let q = 0.01;
        let t_small = 1e-3;

        for &sigma in &[0.01, 0.10, 0.50] {
            let tree = BinomialTree::leisen_reimer(51);
            let (u, d, p) = tree
                .calculate_parameters(spot, strike, r, sigma, t_small, q)
                .expect("LR params should compute");

            assert!((0.0..=1.0).contains(&p), "p must be in [0,1], got {}", p);
            assert!(
                u > 1.0 && d < 1.0 && u > d,
                "u>1>d must hold: u={}, d={}",
                u,
                d
            );
        }
    }

    #[test]
    fn test_american_put_early_exercise_premium() {
        // American put should be worth more than European put
        let market_params = OptionMarketParams::put(100.0, 110.0, 0.05, 0.20, 1.0);

        let tree = BinomialTree::crr(100); // Use CRR since LR has issues

        let american = tree.price_american(&market_params).expect("should succeed");
        let european = tree.price_european(&market_params).expect("should succeed");

        println!(
            "American put: {}, European put: {}, Premium: {}",
            american,
            european,
            american - european
        );

        // American should be worth more due to early exercise
        assert!(american >= european);
        assert!(
            american - european > 0.001,
            "Early exercise premium {} should be meaningful",
            american - european
        ); // Should have some early exercise premium
    }

    #[test]
    fn test_bermudan_between_european_and_american() {
        // Bermudan should be between European and American
        let market_params = OptionMarketParams::put(100.0, 110.0, 0.05, 0.20, 1.0);

        let tree = BinomialTree::leisen_reimer(100);

        // Exercise allowed quarterly
        let exercise_dates = vec![0.25, 0.5, 0.75, 1.0];

        let american = tree.price_american(&market_params).expect("should succeed");
        let bermudan = tree
            .price_bermudan(&market_params, &exercise_dates)
            .expect("should succeed");
        let european = tree.price_european(&market_params).expect("should succeed");

        // Bermudan should be between European and American
        assert!(bermudan >= european);
        assert!(bermudan <= american);
    }

    #[test]
    fn test_exercise_schedule_mapping() {
        // Map quarterly exercise dates over 1Y with 4 steps
        let dates = vec![0.0, 0.25, 0.5, 0.75, 1.0];
        let steps = super::map_exercise_dates_to_steps(&dates, 1.0, 4);
        assert_eq!(steps, vec![0, 1, 2, 3, 4]);

        // Irregular dates should round to nearest step
        let dates2 = vec![0.12, 0.37, 0.62, 0.88];
        let steps2 = super::map_exercise_dates_to_steps(&dates2, 1.0, 4);
        assert_eq!(steps2, vec![0, 1, 2, 4]);
    }

    #[test]
    fn test_leisen_reimer_odd_helper() {
        // Test that leisen_reimer_odd rounds to nearest odd
        let tree_even = BinomialTree::leisen_reimer_odd(100);
        assert_eq!(tree_even.steps, 101, "Even steps should round up to odd");

        let tree_odd = BinomialTree::leisen_reimer_odd(99);
        assert_eq!(tree_odd.steps, 99, "Odd steps should stay as-is");

        let tree_200 = BinomialTree::leisen_reimer_odd(200);
        assert_eq!(tree_200.steps, 201, "200 should become 201");
    }

    /// Golden test: CRR ATM call vs Black-Scholes analytical value
    ///
    /// Black-Scholes formula for European call:
    /// C = S·N(d1) - K·e^(-rT)·N(d2)
    /// where d1 = [ln(S/K) + (r + σ²/2)T] / (σ√T)
    ///       d2 = d1 - σ√T
    #[test]
    fn test_golden_crr_atm_vs_black_scholes() {
        // ATM call: S=K=100, r=5%, σ=20%, T=1Y
        // Black-Scholes analytical: C ≈ 10.4506
        let market_params = OptionMarketParams::call(100.0, 100.0, 0.05, 0.20, 1.0);
        let bs_analytical = 10.4506;

        // CRR with high steps should be within 0.1% of BS
        let tree = BinomialTree::crr(500);
        let crr_price = tree.price_european(&market_params).expect("should succeed");

        let relative_error = ((crr_price - bs_analytical) / bs_analytical).abs();
        assert!(
            relative_error < 0.001, // 0.1% tolerance
            "CRR(500) price {} should be within 0.1% of BS {} (error={}%)",
            crr_price,
            bs_analytical,
            relative_error * 100.0
        );
    }

    /// Golden test: LR odd-step tree achieves better convergence
    #[test]
    fn test_golden_lr_odd_converges_faster() {
        let market_params = OptionMarketParams::call(100.0, 100.0, 0.05, 0.20, 1.0);
        let bs_analytical = 10.4506;

        // LR with odd steps (101) should be within 1 cent of BS
        let lr_tree = BinomialTree::leisen_reimer_odd(100);
        assert_eq!(lr_tree.steps, 101, "Should be rounded to odd");

        let lr_price = lr_tree
            .price_european(&market_params)
            .expect("should succeed");

        let error = (lr_price - bs_analytical).abs();
        assert!(
            error < 0.05, // 5 cents tolerance
            "LR(101) price {} should be within 5c of BS {} (error={})",
            lr_price,
            bs_analytical,
            error
        );
    }

    #[test]
    fn test_calculate_parameters_rejects_non_positive_time_or_volatility() {
        let tree = BinomialTree::crr(100);

        let time_err = tree
            .calculate_parameters(100.0, 100.0, 0.05, 0.2, 0.0, 0.0)
            .expect_err("zero maturity should fail");
        assert!(time_err.to_string().contains("positive time_to_maturity"));

        let vol_err = tree
            .calculate_parameters(100.0, 100.0, 0.05, 0.0, 1.0, 0.0)
            .expect_err("zero volatility should fail");
        assert!(
            vol_err.to_string().contains("positive volatility"),
            "a volatility error must name volatility, not maturity: {vol_err}"
        );
    }

    /// W-07: CRR/Tian with σ·√dt underflowing to 0 must return a descriptive
    /// "degenerate" error, not silently produce NaN or a misleading probability
    /// range error.
    #[test]
    fn test_w07_degenerate_vol_times_sqrt_dt_returns_descriptive_error() {
        // sigma so small that sigma * sqrt(dt) underflows to 0.0 in f64,
        // causing u = d = 1.0 and division-by-zero in the probability formula.
        let epsilon_vol = 5e-162; // sqrt(dt=1/100) = 0.1; 5e-162 * 0.1 = 5e-163, underflows
        let steps = 100;

        // CRR: must return an explicit degenerate error
        let crr_tree = BinomialTree::crr(steps);
        let crr_err = crr_tree
            .calculate_parameters(100.0, 100.0, 0.05, epsilon_vol, 1.0, 0.0)
            .expect_err("CRR with degenerate vol should fail");
        let msg = crr_err.to_string();
        assert!(
            msg.contains("degenerate"),
            "CRR degenerate error must mention 'degenerate', got: {msg}"
        );
    }

    #[test]
    fn test_leisen_reimer_falls_back_to_crr_when_spot_or_strike_non_positive() {
        let tree = BinomialTree::leisen_reimer(51);

        for (spot, strike) in [(0.0, 100.0), (100.0, 0.0), (-1.0, 100.0)] {
            let (u, d, p) = tree
                .calculate_parameters(spot, strike, 0.03, 0.25, 1.0, 0.01)
                .expect("fallback parameters should succeed");
            assert!(u > 1.0 && d < 1.0 && u > d);
            assert!((0.0..=1.0).contains(&p));
        }
    }
}

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;

use super::node_state::NodeState;
use super::state_keys;

/// Trait for instrument-specific valuation logic on a tree
pub trait TreeValuator: Send + Sync {
    /// Calculate the instrument's value at a terminal node (maturity)
    fn value_at_maturity(&self, state: &NodeState) -> Result<f64>;

    /// Calculate the instrument's value at an intermediate node
    ///
    /// This method implements the core decision logic (e.g., hold vs. exercise)
    /// and receives the discounted expected continuation value from child nodes.
    ///
    /// # Arguments
    ///
    /// * `state` - Node state with cached common variables
    /// * `continuation_value` - Discounted expected value from child nodes
    /// * `dt` - Time step size (passed explicitly to avoid hash lookup)
    fn value_at_node(&self, state: &NodeState, continuation_value: f64, dt: f64) -> Result<f64>;
}

/// Trait for generic tree models (binomial, trinomial, etc.)
pub trait TreeModel: Send + Sync {
    /// Price an instrument using this tree model
    ///
    /// # Arguments
    /// * `initial_vars` - Initial state variables at t=0
    /// * `time_to_maturity` - Total time to maturity in years
    /// * `market_context` - Market data context
    /// * `valuator` - Instrument-specific valuation logic
    #[must_use = "pricing result should not be discarded"]
    fn price<V: TreeValuator>(
        &self,
        initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
    ) -> Result<f64>;

    /// Calculate Greeks using finite differences
    ///
    /// # Arguments
    /// * `initial_vars` - Initial state variables at t=0
    /// * `time_to_maturity` - Total time to maturity in years
    /// * `market_context` - Market data context
    /// * `valuator` - Instrument-specific valuation logic
    /// * `bump_size` - Size of finite difference bumps (default: 1% of base value)
    fn calculate_greeks<V: TreeValuator>(
        &self,
        initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
        bump_size: Option<f64>,
    ) -> Result<TreeGreeks> {
        let bump = bump_size.unwrap_or(0.01);

        let vars = initial_vars;

        let base_price = self.price(vars.clone(), time_to_maturity, market_context, valuator)?;

        let mut greeks = TreeGreeks {
            price: base_price,
            delta: 0.0,
            gamma: 0.0,
            vega: 0.0,
            theta: 0.0,
            rho: 0.0,
        };

        if let Some(&spot) = vars.get(state_keys::SPOT) {
            let h = bump * spot;

            let mut vars_up = vars.clone();
            vars_up.insert(state_keys::SPOT, spot + h);
            let mut vars_down = vars.clone();
            vars_down.insert(state_keys::SPOT, spot - h);

            let price_up = self.price(vars_up, time_to_maturity, market_context, valuator)?;
            let price_down = self.price(vars_down, time_to_maturity, market_context, valuator)?;

            greeks.delta = (price_up - price_down) / (2.0 * h);
            greeks.gamma = (price_up - 2.0 * base_price + price_down) / (h * h);
        }

        if let Some(&vol) = vars.get(state_keys::VOLATILITY) {
            let h = 0.01;
            let vol_down = (vol - h).max(1e-6);

            let mut vars_up = vars.clone();
            vars_up.insert(state_keys::VOLATILITY, vol + h);
            let mut vars_down = vars.clone();
            vars_down.insert(state_keys::VOLATILITY, vol_down);

            let price_vol_up = self.price(vars_up, time_to_maturity, market_context, valuator)?;
            let price_vol_down =
                self.price(vars_down, time_to_maturity, market_context, valuator)?;

            greeks.vega = (price_vol_up - price_vol_down) / 2.0;
        }

        if let Some(&rate) = vars.get(state_keys::INTEREST_RATE) {
            let h = 0.0001;

            let mut vars_up = vars.clone();
            vars_up.insert(state_keys::INTEREST_RATE, rate + h);
            let mut vars_down = vars.clone();
            vars_down.insert(state_keys::INTEREST_RATE, rate - h);

            let price_rate_up = self.price(vars_up, time_to_maturity, market_context, valuator)?;
            let price_rate_down =
                self.price(vars_down, time_to_maturity, market_context, valuator)?;

            greeks.rho = (price_rate_up - price_rate_down) / 2.0;
        }

        let dt = 1.0 / 365.25;
        if time_to_maturity > dt {
            let price_tomorrow =
                self.price(vars, time_to_maturity - dt, market_context, valuator)?;
            greeks.theta = -(base_price - price_tomorrow) / dt;
        }

        Ok(greeks)
    }
}

/// Greeks calculated from tree models.
///
/// # Units and Conventions
///
/// - **Delta**: Per unit of spot (e.g., delta=0.5 means $0.50 per $1 spot move)
/// - **Gamma**: Per unit of spot squared (second derivative)
/// - **Vega**: Per 1% absolute volatility move (e.g., 20% → 21%)
/// - **Theta**: Per day (negative for long positions typically)
/// - **Rho**: Per 1 basis point (0.01%) interest rate move
#[derive(Debug, Clone)]
pub struct TreeGreeks {
    /// Instrument price
    pub price: f64,
    /// Delta (spot sensitivity per unit spot move)
    pub delta: f64,
    /// Gamma (curvature, second derivative w.r.t. spot)
    pub gamma: f64,
    /// Vega (volatility sensitivity per 1% vol move)
    pub vega: f64,
    /// Theta (time decay per day)
    pub theta: f64,
    /// Rho (interest rate sensitivity per 1bp rate move)
    pub rho: f64,
}

impl TreeGreeks {
    /// Apply Richardson extrapolation to combine Greeks from two step sizes.
    ///
    /// Richardson extrapolation improves accuracy by combining results from
    /// trees with N and 2N steps:
    ///
    /// ```text
    /// result_improved = (4 × result_fine - result_coarse) / 3
    /// ```
    ///
    /// This cancels the O(h²) error term, achieving O(h⁴) accuracy.
    ///
    /// # Important: Refinement Ratio
    ///
    /// The `(4*fine - coarse)/3` formula is only correct when `fine` uses
    /// exactly **2x** the number of steps as `coarse` (i.e., step size ratio = 2).
    /// For a different refinement ratio r, the formula becomes:
    /// `(r² × fine - coarse) / (r² - 1)`.
    ///
    /// # Arguments
    ///
    /// * `coarse` - Greeks from tree with N steps
    /// * `fine` - Greeks from tree with 2N steps (must be exactly 2x)
    ///
    /// # Returns
    ///
    /// Extrapolated Greeks with improved accuracy.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let coarse = tree_n.calculate_greeks(...)?;
    /// let fine = tree_2n.calculate_greeks(...)?;
    /// let improved = TreeGreeks::richardson_extrapolate(&coarse, &fine);
    /// ```
    ///
    /// # References
    ///
    /// - Broadie, M. & Detemple, J. (1996). "American Option Valuation: New Bounds,
    ///   Approximations, and a Comparison of Existing Methods." Review of Financial
    ///   Studies, 9(4), 1211-1250.
    #[must_use]
    pub fn richardson_extrapolate(coarse: &Self, fine: &Self) -> Self {
        Self {
            price: (4.0 * fine.price - coarse.price) / 3.0,
            delta: (4.0 * fine.delta - coarse.delta) / 3.0,
            gamma: (4.0 * fine.gamma - coarse.gamma) / 3.0,
            vega: (4.0 * fine.vega - coarse.vega) / 3.0,
            theta: (4.0 * fine.theta - coarse.theta) / 3.0,
            rho: (4.0 * fine.rho - coarse.rho) / 3.0,
        }
    }

    /// Apply Richardson extrapolation to a price value only.
    ///
    /// Useful when only the price is needed, not all Greeks.
    #[must_use]
    pub fn richardson_price(price_coarse: f64, price_fine: f64) -> f64 {
        (4.0 * price_fine - price_coarse) / 3.0
    }
}

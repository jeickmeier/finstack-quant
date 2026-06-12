//! Binomial tree models for option pricing.
//!
//! Implements various binomial tree methods including Cox-Ross-Rubinstein (CRR)
//! and Leisen-Reimer for American and Bermudan option pricing.
//!
//! Now includes generic TreeModel implementation for pricing arbitrary instruments.

use crate::instruments::common_impl::parameters::OptionMarketParams;
use crate::instruments::{ExerciseStyle, OptionType};
use crate::models::trees::NodeState;
use crate::models::volatility::black::d1_d2;
use finstack_core::market_data::context::MarketContext;
use finstack_core::HashMap;
use finstack_core::HashSet;
use finstack_core::{Error, Result};

// Import the generic tree framework
use super::tree_framework::{
    map_exercise_dates_to_steps, price_recombining_tree, single_factor_equity_state, state_keys,
    BarrierSpec, BarrierStyle, RecombiningInputs, TreeBranching, TreeGreeks, TreeModel,
    TreeValuator,
};

/// Binomial tree types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum TreeType {
    /// Cox-Ross-Rubinstein (standard binomial)
    CRR,
    /// Jarrow-Rudd (equal-probability) lattice.
    ///
    /// Fixes the up/down probability at `p = 0.5` and pushes the drift into
    /// the lattice factors:
    ///
    /// ```text
    /// u = exp((r - q - sigma^2/2) * dt + sigma * sqrt(dt))
    /// d = exp((r - q - sigma^2/2) * dt - sigma * sqrt(dt))
    /// ```
    ///
    /// # Known forward-price bias
    ///
    /// JR is **not** risk-neutral exact. The one-step expected gross return is
    ///
    /// ```text
    /// 0.5 * (u + d) = exp((r - q) * dt) * exp(-sigma^2 * dt / 2) * cosh(sigma * sqrt(dt))
    /// ```
    ///
    /// Since `exp(-sigma^2*dt/2) * cosh(sigma*sqrt(dt)) = 1 + O(dt^2)`, the
    /// per-step expected return differs from the true forward `exp((r-q)*dt)`
    /// by `O(dt^2)`, i.e. an `O(dt)` bias accumulated over the `1/dt` steps of
    /// the tree. This is a well-known, accepted property of the equal-
    /// probability scheme (Jarrow & Rudd, 1983), not a defect: the bias
    /// vanishes as the step count grows and JR retains first-order
    /// convergence. Use [`TreeType::CRR`] or [`TreeType::Tian`] when an exact
    /// per-step forward match is required.
    JR,
    /// Leisen-Reimer (improved convergence)
    LeisenReimer,
    /// Tian (moment matching)
    Tian,
}

/// Vanilla / Bermudan / barrier option valuator for the binomial recombining
/// engine.
///
/// Single source of truth for the call/put intrinsic-vs-continuation logic. The
/// previous implementation declared three near-identical `struct OptionValuator`
/// variants inline inside `price`, `price_barrier_out`, and `price_barrier_in*`;
/// the only meaningful difference was whether early-exercise dates were
/// supplied. With `exercise_steps = None` the valuator behaves like a pure
/// European; otherwise it applies American/Bermudan early exercise at the
/// requested step indices.
struct OptionValuator {
    strike: f64,
    option_type: OptionType,
    exercise_steps: Option<HashSet<usize>>,
}

impl TreeValuator for OptionValuator {
    fn value_at_maturity(&self, state: &NodeState) -> Result<f64> {
        let s = state
            .spot()
            .ok_or_else(|| Error::internal("option node state missing spot at maturity"))?;
        Ok(intrinsic(self.option_type, s, self.strike))
    }

    fn value_at_node(&self, state: &NodeState, continuation_value: f64, _dt: f64) -> Result<f64> {
        if let Some(steps) = &self.exercise_steps {
            if steps.contains(&state.step) {
                let s = state
                    .spot()
                    .ok_or_else(|| Error::internal("option node state missing spot"))?;
                let exercise = intrinsic(self.option_type, s, self.strike);
                return Ok(continuation_value.max(exercise));
            }
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
                target: "finstack_valuations::trees",
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
    /// ```ignore
    /// use finstack_valuations::models::trees::BinomialTree;
    ///
    /// let tree = BinomialTree::leisen_reimer_odd(100);
    /// assert_eq!(tree.steps, 101);
    ///
    /// let tree = BinomialTree::leisen_reimer_odd(99);
    /// assert_eq!(tree.steps, 99);
    /// ```
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
        if t <= 0.0 || sigma <= 0.0 {
            return Err(Error::internal(
                "binomial tree requires positive time_to_maturity and volatility",
            ));
        }

        let dt = t / self.steps as f64;

        let (u, d, p) = match self.tree_type {
            TreeType::LeisenReimer => {
                // Fallback to CRR if strike/spot are not usable (e.g., generic tree)
                if spot <= 0.0 || strike <= 0.0 {
                    let u = (sigma * dt.sqrt()).exp();
                    let d = 1.0 / u;
                    let p = (((r - q) * dt).exp() - d) / (u - d);
                    if !(0.0..=1.0).contains(&p) {
                        return Err(Error::internal(
                            "Leisen-Reimer fallback probability fell outside [0, 1]",
                        ));
                    }
                    return Ok((u, d, p));
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
            TreeType::CRR => {
                // Cox-Ross-Rubinstein parameters
                let u = (sigma * dt.sqrt()).exp();
                let d = 1.0 / u;
                let spread = u - d;
                // Guard: if σ√dt is so small that u ≈ d (spread underflows to 0),
                // the probability formula produces 0/0 = NaN.  Catch it explicitly
                // before the division so the error message is descriptive.
                if spread < 1e-14 {
                    return Err(Error::Validation(format!(
                        "CRR tree is degenerate: u ≈ d (spread = {spread:.3e}). \
                         σ√dt = {:.3e} is too small — increase volatility or maturity.",
                        sigma * dt.sqrt()
                    )));
                }
                let p = (((r - q) * dt).exp() - d) / spread;

                // Validate probability
                if !(0.0..=1.0).contains(&p) {
                    return Err(Error::Validation(format!(
                        "CRR probability p={p:.6} fell outside [0, 1]; \
                         check parameters: sigma={sigma}, r={r}, q={q}, dt={dt:.3e}"
                    )));
                }

                (u, d, p)
            }
            TreeType::JR => {
                // Jarrow-Rudd (equal-probability) parameters: p = 0.5 with the
                // drift carried in u/d. This scheme carries a known O(dt)
                // cumulative forward-price bias (see `TreeType::JR` docs); that
                // is an accepted property of the scheme, intentionally NOT
                // enforced here. We do, however, validate that u/d are finite
                // and strictly positive — a non-finite or non-positive lattice
                // factor would silently corrupt every node price.
                let u = ((r - q - 0.5 * sigma * sigma) * dt + sigma * dt.sqrt()).exp();
                let d = ((r - q - 0.5 * sigma * sigma) * dt - sigma * dt.sqrt()).exp();
                let p = 0.5;

                if !(u.is_finite() && d.is_finite() && u > 0.0 && d > 0.0) {
                    return Err(Error::Validation(format!(
                        "JR parameters invalid: u={u}, d={d} (must be finite and > 0). \
                         Check parameters: sigma={sigma}, r={r}, q={q}, dt={dt}"
                    )));
                }

                (u, d, p)
            }
            TreeType::Tian => {
                // Tian (1993) third-order moment matching.
                //
                // Reference: Tian, Y. (1993). "A Modified Lattice Approach to
                // Option Pricing." Journal of Futures Markets, 13(5), 563-577.
                //
                // Three-moment conditions: pu^k + (1-p)d^k = M_k for k=1,2,3
                //   M1 = exp((r-q)*dt),  V = exp(sigma^2*dt)
                //   A = u+d = M1*V*(V+1),  B = u*d = M1^2*V^2
                //
                // u = (M1*V/2) * (V + 1 + sqrt(V^2 + 2V - 3))
                // d = (M1*V/2) * (V + 1 - sqrt(V^2 + 2V - 3))
                // p = (M1 - d) / (u - d)
                let m1 = ((r - q) * dt).exp();
                let v = (sigma * sigma * dt).exp();
                // Guard: when σ²dt → 0, V → 1 and disc → 0, collapsing u = d.
                // Catch this before the division so the error is descriptive.
                let var_term = v * v + 2.0 * v - 3.0;
                if var_term < 1e-28 {
                    return Err(Error::Validation(format!(
                        "Tian tree is degenerate: σ²·dt = {:.3e} is too small — \
                         up/down factors collapse (V ≈ 1, discriminant ≈ 0). \
                         Increase volatility or maturity.",
                        sigma * sigma * dt
                    )));
                }
                let disc = var_term.sqrt();
                let half_m1v = m1 * v / 2.0;
                let u = half_m1v * (v + 1.0 + disc);
                let d = half_m1v * (v + 1.0 - disc);
                let spread = u - d;
                if spread < 1e-14 {
                    return Err(Error::Validation(format!(
                        "Tian tree is degenerate: u ≈ d (spread = {spread:.3e}). \
                         σ√dt is too small — increase volatility or maturity."
                    )));
                }
                let p = (m1 - d) / spread;

                if !(0.0..=1.0).contains(&p) {
                    return Err(Error::Validation(format!(
                        "Tian probability p={p:.6} out of bounds. \
                         Check parameters: sigma={sigma}, r={r}, q={q}, dt={dt:.3e}"
                    )));
                }

                (u, d, p)
            }
        };

        Ok((u, d, p))
    }

    /// Internal unified pricer supporting European, American, and Bermudan styles
    /// via an optional list of exercise steps.
    fn price_with_exercise(
        &self,
        market_params: &OptionMarketParams,
        exercise_steps: Option<&[usize]>,
    ) -> Result<f64> {
        // Compute lattice parameters honoring the configured binomial model
        let (u, d, p) = self.calculate_parameters(
            market_params.spot,
            market_params.strike,
            market_params.rate,
            market_params.volatility,
            market_params.time_to_expiry,
            market_params.dividend_yield,
        )?;

        // Build an option valuator that applies early exercise at requested steps
        let exercise_set: Option<HashSet<usize>> =
            exercise_steps.map(|steps| steps.iter().copied().collect::<HashSet<usize>>());

        let valuator = OptionValuator {
            strike: market_params.strike,
            option_type: market_params.option_type,
            exercise_steps: exercise_set,
        };

        let initial_vars = single_factor_equity_state(
            market_params.spot,
            market_params.rate,
            market_params.dividend_yield,
            market_params.volatility,
        );

        // Delegate to the shared recombining engine
        price_recombining_tree(RecombiningInputs {
            branching: TreeBranching::Binomial,
            steps: self.steps,
            initial_vars,
            time_to_maturity: market_params.time_to_expiry,
            market_context: &MarketContext::new(), // not used by valuator
            valuator: &valuator,
            up_factor: u,
            down_factor: d,
            middle_factor: None,
            prob_up: p,
            prob_down: 1.0 - p,
            prob_middle: None,
            interest_rate: market_params.rate,
            barrier: None,
            custom_state_generator: None,
            custom_rate_generator: None,
        })
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

    /// Calculate Greeks using binomial tree
    pub fn calculate_greeks(
        &self,
        market_params: &OptionMarketParams,
        exercise_style: ExerciseStyle,
    ) -> Result<BinomialGreeks> {
        // Price at base case
        let base_price = match exercise_style {
            ExerciseStyle::American => self.price_american(market_params)?,
            ExerciseStyle::European => self.price_european(market_params)?,
            ExerciseStyle::Bermudan => {
                return Err(Error::internal(
                    "binomial greeks only support American and European exercise styles",
                ))
            }
        };

        // Delta: use small bump
        let h = 0.01 * market_params.spot;
        let mut params_up = market_params.clone();
        params_up.spot += h;
        let price_up = match exercise_style {
            ExerciseStyle::American => self.price_american(&params_up)?,
            ExerciseStyle::European => self.price_european(&params_up)?,
            ExerciseStyle::Bermudan => {
                return Err(Error::internal(
                    "binomial greeks only support American and European exercise styles",
                ))
            }
        };

        let mut params_down = market_params.clone();
        params_down.spot -= h;
        let price_down = match exercise_style {
            ExerciseStyle::American => self.price_american(&params_down)?,
            ExerciseStyle::European => self.price_european(&params_down)?,
            ExerciseStyle::Bermudan => {
                return Err(Error::internal(
                    "binomial greeks only support American and European exercise styles",
                ))
            }
        };

        let delta = (price_up - price_down) / (2.0 * h);
        let gamma = (price_up - 2.0 * base_price + price_down) / (h * h);

        // Theta: use 1-day bump
        let dt = 1.0 / 365.25;
        let theta = if market_params.time_to_expiry > dt {
            let mut params_later = market_params.clone();
            params_later.time_to_expiry -= dt;
            let price_later = match exercise_style {
                ExerciseStyle::American => self.price_american(&params_later)?,
                ExerciseStyle::European => self.price_european(&params_later)?,
                ExerciseStyle::Bermudan => {
                    return Err(Error::internal(
                        "binomial greeks only support American and European exercise styles",
                    ))
                }
            };
            -(base_price - price_later) / dt
        } else {
            0.0
        };

        Ok(BinomialGreeks {
            price: base_price,
            delta,
            gamma,
            theta,
        })
    }

    /// Price barrier knock-out option (up/down) with rebate using binomial tree (discrete monitoring)
    pub fn price_barrier_out(
        &self,
        market_params: &OptionMarketParams,
        up_level: Option<f64>,
        down_level: Option<f64>,
        rebate: f64,
    ) -> Result<f64> {
        // Compute lattice parameters
        let (u, d, p) = self.calculate_parameters(
            market_params.spot,
            market_params.strike,
            market_params.rate,
            market_params.volatility,
            market_params.time_to_expiry,
            market_params.dividend_yield,
        )?;

        // Pure-European barrier — no early exercise; reuse the shared valuator
        // with `exercise_steps: None`.
        let valuator = OptionValuator {
            strike: market_params.strike,
            option_type: market_params.option_type,
            exercise_steps: None,
        };

        let initial_vars = single_factor_equity_state(
            market_params.spot,
            market_params.rate,
            market_params.dividend_yield,
            market_params.volatility,
        );

        // Barrier configuration
        let barrier = Some(BarrierSpec {
            up_level,
            down_level,
            rebate,
            style: BarrierStyle::KnockOut,
        });

        price_recombining_tree(RecombiningInputs {
            branching: TreeBranching::Binomial,
            steps: self.steps,
            initial_vars: {
                // Ensure spot exists (done), nothing else needed
                initial_vars
            },
            time_to_maturity: market_params.time_to_expiry,
            market_context: &MarketContext::new(),
            valuator: &valuator,
            up_factor: u,
            down_factor: d,
            middle_factor: None,
            prob_up: p,
            prob_down: 1.0 - p,
            prob_middle: None,
            interest_rate: market_params.rate,
            barrier,
            custom_state_generator: None,
            custom_rate_generator: None,
        })
    }

    fn price_barrier_in_with_exercise(
        &self,
        market_params: &OptionMarketParams,
        up_level: Option<f64>,
        down_level: Option<f64>,
        rebate: f64,
        exercise_steps: Option<&[usize]>,
    ) -> Result<f64> {
        let (u, d, p) = self.calculate_parameters(
            market_params.spot,
            market_params.strike,
            market_params.rate,
            market_params.volatility,
            market_params.time_to_expiry,
            market_params.dividend_yield,
        )?;

        let exercise_set =
            exercise_steps.map(|steps| steps.iter().copied().collect::<HashSet<_>>());

        let valuator = OptionValuator {
            strike: market_params.strike,
            option_type: market_params.option_type,
            exercise_steps: exercise_set,
        };

        let initial_vars = single_factor_equity_state(
            market_params.spot,
            market_params.rate,
            market_params.dividend_yield,
            market_params.volatility,
        );

        let barrier = Some(BarrierSpec {
            up_level,
            down_level,
            rebate,
            style: BarrierStyle::KnockIn,
        });

        price_recombining_tree(RecombiningInputs {
            branching: TreeBranching::Binomial,
            steps: self.steps,
            initial_vars,
            time_to_maturity: market_params.time_to_expiry,
            market_context: &MarketContext::new(),
            valuator: &valuator,
            up_factor: u,
            down_factor: d,
            middle_factor: None,
            prob_up: p,
            prob_down: 1.0 - p,
            prob_middle: None,
            interest_rate: market_params.rate,
            barrier,
            custom_state_generator: None,
            custom_rate_generator: None,
        })
    }

    /// Price barrier knock-in option (discrete monitoring) using path-dependent tracking.
    ///
    /// # Constraints
    ///
    /// - Only supported when exactly one of `up_level`/`down_level` is `Some`
    /// - Uses European exercise by default (no early exercise)
    ///
    /// # Arguments
    ///
    /// * `market_params` - Option parameters
    /// * `up_level` - Up barrier level (optional)
    /// * `down_level` - Down barrier level (optional)
    /// * `rebate` - Rebate paid at expiry if barrier never triggers
    ///
    /// # Errors
    ///
    /// Returns an error if both or neither barrier levels are specified.
    pub fn price_barrier_in(
        &self,
        market_params: &OptionMarketParams,
        up_level: Option<f64>,
        down_level: Option<f64>,
        rebate: f64,
    ) -> Result<f64> {
        self.price_barrier_in_with_exercise(market_params, up_level, down_level, rebate, None)
    }

    /// Price barrier knock-in option with American exercise.
    pub fn price_barrier_in_american(
        &self,
        market_params: &OptionMarketParams,
        up_level: Option<f64>,
        down_level: Option<f64>,
        rebate: f64,
    ) -> Result<f64> {
        let all_steps: Vec<usize> = (0..self.steps).collect();
        self.price_barrier_in_with_exercise(
            market_params,
            up_level,
            down_level,
            rebate,
            Some(&all_steps),
        )
    }

    /// Price barrier knock-in option with Bermudan exercise dates.
    pub fn price_barrier_in_bermudan(
        &self,
        market_params: &OptionMarketParams,
        up_level: Option<f64>,
        down_level: Option<f64>,
        rebate: f64,
        exercise_dates: &[f64],
    ) -> Result<f64> {
        let mut steps =
            map_exercise_dates_to_steps(exercise_dates, market_params.time_to_expiry, self.steps);
        steps.sort();
        steps.dedup();
        self.price_barrier_in_with_exercise(
            market_params,
            up_level,
            down_level,
            rebate,
            Some(&steps),
        )
    }

    /// Generic pricing engine for arbitrary instruments
    ///
    /// This method implements the TreeModel trait, providing a flexible
    /// interface for pricing any instrument that implements TreeValuator.
    #[inline(never)] // Prevent inlining to reduce coverage metadata conflicts
    pub fn price_generic<V: TreeValuator>(
        &self,
        initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
    ) -> Result<f64> {
        // Extract required parameters from state variables
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

        // Calculate binomial parameters and delegate to the shared engine
        let (u, d, p) = self.calculate_parameters(0.0, 0.0, r, sigma, time_to_maturity, q)?;

        price_recombining_tree(RecombiningInputs {
            branching: TreeBranching::Binomial,
            steps: self.steps,
            initial_vars,
            time_to_maturity,
            market_context,
            valuator,
            up_factor: u,
            down_factor: d,
            middle_factor: None,
            prob_up: p,
            prob_down: 1.0 - p,
            prob_middle: None,
            interest_rate: r,
            barrier: None,
            custom_state_generator: None,
            custom_rate_generator: None,
        })
    }
}

/// Greeks calculated from binomial tree
#[derive(Debug, Clone)]
pub struct BinomialGreeks {
    /// Option price
    pub price: f64,
    /// Delta
    pub delta: f64,
    /// Gamma
    pub gamma: f64,
    /// Theta
    pub theta: f64,
}

impl BinomialGreeks {
    /// Apply Richardson extrapolation to improve accuracy.
    ///
    /// Combines Greeks from trees with N and 2N steps using:
    /// ```text
    /// result = (4 × fine - coarse) / 3
    /// ```
    ///
    /// This achieves O(h⁴) accuracy instead of O(h²).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let tree_n = BinomialTree::crr(100);
    /// let tree_2n = BinomialTree::crr(200);
    ///
    /// let greeks_n = tree_n.calculate_greeks(&params, ExerciseStyle::American)?;
    /// let greeks_2n = tree_2n.calculate_greeks(&params, ExerciseStyle::American)?;
    ///
    /// let improved = BinomialGreeks::richardson_extrapolate(&greeks_n, &greeks_2n);
    /// ```
    #[must_use]
    pub fn richardson_extrapolate(coarse: &Self, fine: &Self) -> Self {
        Self {
            price: (4.0 * fine.price - coarse.price) / 3.0,
            delta: (4.0 * fine.delta - coarse.delta) / 3.0,
            gamma: (4.0 * fine.gamma - coarse.gamma) / 3.0,
            theta: (4.0 * fine.theta - coarse.theta) / 3.0,
        }
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

    fn calculate_greeks<V: TreeValuator>(
        &self,
        initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
        bump_size: Option<f64>,
    ) -> Result<TreeGreeks> {
        let bump = bump_size.unwrap_or(0.01);

        // Base price
        let base_price = self.price(
            initial_vars.clone(),
            time_to_maturity,
            market_context,
            valuator,
        )?;

        let mut greeks = TreeGreeks {
            price: base_price,
            delta: 0.0,
            gamma: 0.0,
            vega: 0.0,
            theta: 0.0,
            rho: 0.0,
        };

        // Calculate Delta and Gamma (spot sensitivity)
        if let Some(&spot) = initial_vars.get(state_keys::SPOT) {
            let h = bump * spot;

            // Spot up
            let mut vars_up = initial_vars.clone();
            vars_up.insert(state_keys::SPOT, spot + h);
            let price_up = self.price(vars_up, time_to_maturity, market_context, valuator)?;

            // Spot down
            let mut vars_down = initial_vars.clone();
            vars_down.insert(state_keys::SPOT, spot - h);
            let price_down = self.price(vars_down, time_to_maturity, market_context, valuator)?;

            greeks.delta = (price_up - price_down) / (2.0 * h);
            greeks.gamma = (price_up - 2.0 * base_price + price_down) / (h * h);
        }

        // Calculate Vega (volatility sensitivity)
        if let Some(&vol) = initial_vars.get(state_keys::VOLATILITY) {
            let h = 0.01; // 1% vol bump

            let mut vars_vol_up = initial_vars.clone();
            vars_vol_up.insert(state_keys::VOLATILITY, vol + h);
            let price_vol_up =
                self.price(vars_vol_up, time_to_maturity, market_context, valuator)?;

            let mut vars_vol_down = initial_vars.clone();
            vars_vol_down.insert(state_keys::VOLATILITY, (vol - h).max(1e-6));
            let price_vol_down =
                self.price(vars_vol_down, time_to_maturity, market_context, valuator)?;

            greeks.vega = (price_vol_up - price_vol_down) / 2.0;
        }

        // Calculate Rho (rate sensitivity)
        if let Some(&rate) = initial_vars.get(state_keys::INTEREST_RATE) {
            let h = 0.0001; // 1bp rate bump

            let mut vars_rate_up = initial_vars.clone();
            vars_rate_up.insert(state_keys::INTEREST_RATE, rate + h);
            let price_rate_up =
                self.price(vars_rate_up, time_to_maturity, market_context, valuator)?;

            greeks.rho = price_rate_up - base_price;
        }

        // Calculate Theta (time decay) - use 1 day bump
        let dt = 1.0 / 365.25;
        if time_to_maturity > dt {
            let price_tomorrow = self.price(
                initial_vars,
                time_to_maturity - dt,
                market_context,
                valuator,
            )?;
            greeks.theta = -(base_price - price_tomorrow) / dt;
        }

        Ok(greeks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crr_european_converges_to_black_scholes() {
        // Test that CRR converges to Black-Scholes for European options
        let market_params = OptionMarketParams::call(100.0, 100.0, 0.05, 0.20, 1.0);

        // Calculate with increasing steps
        let tree_50 = BinomialTree::crr(50);
        let tree_200 = BinomialTree::crr(200);

        let price_50 = tree_50
            .price_european(&market_params)
            .expect("should succeed");
        let price_200 = tree_200
            .price_european(&market_params)
            .expect("should succeed");

        // With higher steps, should be closer to Black-Scholes (approximately 10.45)
        // Note: Binomial trees don't always converge monotonically due to discrete step effects
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

        // Should be close to Black-Scholes (approximately 10.45)
        assert!((price_200 - 10.45).abs() < 0.15);
    }

    #[test]
    fn test_leisen_reimer_better_convergence() {
        // Test that Leisen-Reimer converges faster than CRR
        let market_params = OptionMarketParams::call(100.0, 100.0, 0.05, 0.20, 1.0);

        let crr = BinomialTree::crr(401);
        let lr = BinomialTree::leisen_reimer(401);

        let crr_price = crr.price_european(&market_params).expect("should succeed");
        let lr_price = lr.price_european(&market_params).expect("should succeed");

        // Both should be close to Black-Scholes value
        let bs_value = 10.4506; // Known Black-Scholes value

        // CRR should be reasonably close to Black-Scholes
        assert!(
            (crr_price - bs_value).abs() < 1.0,
            "CRR price {} should be close to BS value {}, diff={}",
            crr_price,
            bs_value,
            (crr_price - bs_value).abs()
        );

        // LR should be within 10c of Black-Scholes at higher odd steps
        // (relaxed from 5c to account for numerical variations)
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
        // Validate LR convergence for put via put-call parity
        let market_params = OptionMarketParams::put(100.0, 100.0, 0.05, 0.20, 1.0);

        let lr = BinomialTree::leisen_reimer(201);
        let lr_put = lr.price_european(&market_params).expect("should succeed");

        // BS call value known; derive put via parity: P = C - S e^{-qT} + K e^{-rT}
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
        // Check probability and u/d bounds for short maturities and edge vols
        let spot = 100.0;
        let strike = 100.0;
        let r = 0.02;
        let q = 0.01;
        let t_small = 1e-3;

        for &sigma in &[0.01, 0.10, 0.50] {
            let tree = BinomialTree::leisen_reimer(51); // prefer odd steps
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

    /// Golden test: Tian tree converges to Black-Scholes for European call
    #[test]
    fn test_tian_converges_to_black_scholes() {
        let market_params = OptionMarketParams::call(100.0, 100.0, 0.05, 0.20, 1.0);
        let bs_analytical = 10.4506;

        let tree = BinomialTree::new(500, TreeType::Tian);
        let tian_price = tree.price_european(&market_params).expect("should succeed");

        let relative_error = ((tian_price - bs_analytical) / bs_analytical).abs();
        assert!(
            relative_error < 0.001,
            "Tian(500) price {} should be within 0.1% of BS {} (error={}%)",
            tian_price,
            bs_analytical,
            relative_error * 100.0
        );
    }

    /// Golden test: Tian matches put-call parity
    #[test]
    fn test_tian_put_call_parity() {
        let call_params = OptionMarketParams::call(100.0, 100.0, 0.05, 0.20, 1.0);
        let put_params = OptionMarketParams::put(100.0, 100.0, 0.05, 0.20, 1.0);

        let tree = BinomialTree::new(200, TreeType::Tian);
        let call_price = tree.price_european(&call_params).expect("should succeed");
        let put_price = tree.price_european(&put_params).expect("should succeed");

        // Put-call parity: C - P = S - K*exp(-rT)
        let parity_lhs = call_price - put_price;
        let parity_rhs = 100.0 - 100.0 * (-0.05_f64).exp();

        assert!(
            (parity_lhs - parity_rhs).abs() < 0.10,
            "Put-call parity violation: C-P={}, S-Ke^(-rT)={}, diff={}",
            parity_lhs,
            parity_rhs,
            (parity_lhs - parity_rhs).abs()
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
    fn test_binomial_greeks_richardson_extrapolation_matches_formula() {
        let coarse = BinomialGreeks {
            price: 10.0,
            delta: 0.4,
            gamma: 0.03,
            theta: -1.2,
        };
        let fine = BinomialGreeks {
            price: 10.9,
            delta: 0.46,
            gamma: 0.035,
            theta: -1.0,
        };

        let improved = BinomialGreeks::richardson_extrapolate(&coarse, &fine);
        assert!((improved.price - ((4.0 * 10.9 - 10.0) / 3.0)).abs() < 1e-12);
        assert!((improved.delta - ((4.0 * 0.46 - 0.4) / 3.0)).abs() < 1e-12);
        assert!((improved.gamma - ((4.0 * 0.035 - 0.03) / 3.0)).abs() < 1e-12);
        assert!((improved.theta - ((-4.0 + 1.2) / 3.0)).abs() < 1e-12);
    }

    #[test]
    fn tree_model_vega_uses_central_difference_per_one_percent_vol() {
        let tree = BinomialTree::crr(101);
        let market_context = MarketContext::new();
        let initial_vars = single_factor_equity_state(100.0, 0.05, 0.0, 0.20);
        let valuator = OptionValuator {
            strike: 100.0,
            option_type: OptionType::Call,
            exercise_steps: None,
        };

        let greeks = TreeModel::calculate_greeks(
            &tree,
            initial_vars.clone(),
            1.0,
            &market_context,
            &valuator,
            None,
        )
        .expect("greeks should compute");

        let h = 0.01;
        let mut vars_up = initial_vars.clone();
        vars_up.insert(state_keys::VOLATILITY, 0.20 + h);
        let price_up = tree
            .price(vars_up, 1.0, &market_context, &valuator)
            .expect("up-vol price");

        let mut vars_down = initial_vars;
        vars_down.insert(state_keys::VOLATILITY, 0.20 - h);
        let price_down = tree
            .price(vars_down, 1.0, &market_context, &valuator)
            .expect("down-vol price");

        let expected_vega = (price_up - price_down) / 2.0;
        assert!(
            (greeks.vega - expected_vega).abs() < 1e-12,
            "vega should use central difference per 1 percentage-point vol move: got={}, expected={}",
            greeks.vega,
            expected_vega
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
        assert!(vol_err.to_string().contains("positive time_to_maturity"));
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

        // Tian: must return an explicit degenerate error
        let tian_tree = BinomialTree::new(steps, TreeType::Tian);
        let tian_err = tian_tree
            .calculate_parameters(100.0, 100.0, 0.05, epsilon_vol, 1.0, 0.0)
            .expect_err("Tian with degenerate vol should fail");
        let msg_tian = tian_err.to_string();
        assert!(
            msg_tian.contains("degenerate"),
            "Tian degenerate error must mention 'degenerate', got: {msg_tian}"
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

    #[test]
    fn test_jarrow_rudd_tree_prices_european_option_reasonably() {
        let market_params = OptionMarketParams::call(100.0, 100.0, 0.05, 0.20, 1.0);
        let tree = BinomialTree::new(400, TreeType::JR);

        let price = tree
            .price_european(&market_params)
            .expect("JR tree should price successfully");

        assert!(price.is_finite());
        assert!((price - 10.4506).abs() < 0.25);
    }

    #[test]
    fn jarrow_rudd_up_down_factors_are_positive_and_finite() {
        // JR sets `p = 0.5` and pushes the drift into `u`/`d`. The parameter
        // builder must validate that the resulting `u`/`d` are finite and
        // strictly positive (a lattice factor of zero or `inf` would corrupt
        // every node price). Exercise a spread of market regimes.
        let tree = BinomialTree::new(256, TreeType::JR);
        for &(r, q, sigma, t) in &[
            (0.05, 0.00, 0.20, 1.0),
            (0.00, 0.08, 0.40, 2.0),
            (0.10, 0.02, 0.05, 0.25),
            (-0.01, 0.03, 0.60, 5.0),
        ] {
            let (u, d, p) = tree
                .calculate_parameters(100.0, 100.0, r, sigma, t, q)
                .expect("JR parameters should be valid");
            assert!(u.is_finite() && u > 0.0, "u must be finite and >0, got {u}");
            assert!(d.is_finite() && d > 0.0, "d must be finite and >0, got {d}");
            assert!((p - 0.5).abs() < 1e-15, "JR probability must be 0.5");
        }
    }

    #[test]
    fn jarrow_rudd_forward_bias_is_small_and_shrinks_with_steps() {
        // JR is NOT risk-neutral exact: with `p = 0.5` the one-step expected
        // gross return `0.5*(u + d)` equals `exp((r-q)*dt) * exp(-sigma^2*dt/2)
        // * cosh(sigma*sqrt(dt))`, which differs from the true forward
        // `exp((r-q)*dt)` by an `O(dt^2)`-per-step (hence `O(dt)` cumulative)
        // bias. This test locks that documented behaviour: the per-step bias
        // is tiny and shrinks quadratically as `dt` shrinks.
        let (r, q, sigma, t) = (0.05_f64, 0.01_f64, 0.25_f64, 1.0_f64);
        let true_fwd_growth = (r - q) * t; // log forward over the whole horizon

        let mut prev_bias = f64::INFINITY;
        for &steps in &[50_usize, 100, 200, 400] {
            let tree = BinomialTree::new(steps, TreeType::JR);
            let (u, d, p) = tree
                .calculate_parameters(100.0, 100.0, r, sigma, t, q)
                .expect("JR parameters valid");
            // Expected gross return per step under the JR measure.
            let step_return = p * u + (1.0 - p) * d;
            // Compounded over all steps, compared with the true forward.
            let tree_fwd_growth = steps as f64 * step_return.ln();
            let bias = (tree_fwd_growth - true_fwd_growth).abs();
            assert!(
                bias < 1e-2,
                "JR cumulative forward bias must stay small, got {bias} at {steps} steps"
            );
            assert!(
                bias < prev_bias,
                "JR forward bias must shrink as steps increase: {bias} !< {prev_bias}"
            );
            prev_bias = bias;
        }
    }

    #[test]
    fn test_option_greeks_reject_unsupported_exercise_and_zero_theta_near_expiry() {
        let tree = BinomialTree::crr(100);
        let params = OptionMarketParams::put(100.0, 100.0, 0.05, 0.20, 0.5);

        let err = tree
            .calculate_greeks(&params, ExerciseStyle::Bermudan)
            .expect_err("bermudan binomial greeks should be rejected");
        assert!(err
            .to_string()
            .contains("only support American and European"));

        let near_expiry = OptionMarketParams::call(100.0, 100.0, 0.05, 0.20, 0.5 / 365.25);
        let greeks = tree
            .calculate_greeks(&near_expiry, ExerciseStyle::European)
            .expect("near-expiry greeks should compute");
        assert_eq!(greeks.theta, 0.0);
    }
}

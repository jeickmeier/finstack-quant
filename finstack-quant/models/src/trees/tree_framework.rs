//! Generic tree-based pricing framework for financial instruments.
//!
//! This module provides a lattice pricing engine that separates instrument
//! payoff logic (`TreeValuator`) from lattice evolution (the tree types), so the
//! same backward induction serves equity and short-rate trees.
//!
//! ## Serialization Policy
//!
//! Tree models and their parameter types are **transient runtime structures** and
//! do not implement `Serialize`/`Deserialize`. Tree configurations are created
//! on demand during pricing from market data, and no current use case requires
//! persisting them. If a future requirement emerges, add serde support only to
//! configuration structs (e.g. `EvolutionParams`) and keep runtime engine types
//! (`BinomialTree`, etc.) non-serializable.
//!
//! [`TreeDiscounting`] holds the per-node discount-factor conventions shared
//! by the short-rate and Hull-White lattices.

use finstack_quant_core::math::Compounding;
use finstack_quant_core::Result;

/// Trait for instrument-specific valuation logic on a tree
pub trait TreeValuator: Send + Sync {
    /// Calculate the instrument's value at a terminal node (maturity)
    fn value_at_maturity(&self, state: &NodeState) -> Result<f64>;

    /// Hold-vs-exercise (or other) decision at an intermediate node, given
    /// the discounted expected continuation value from child nodes.
    ///
    /// # Arguments
    ///
    /// * `state` - Typed node state: step index, OAS and the per-node values
    ///   the tree models
    /// * `continuation_value` - Discounted expected value from child nodes
    /// * `dt` - Time step size in years
    fn value_at_node(&self, state: &NodeState, continuation_value: f64, dt: f64) -> Result<f64>;
}

/// State handed to a [`TreeValuator`] at one lattice node.
///
/// Each tree fills the fields it models and leaves the rest `None`: equity
/// trees set `spot`, short-rate trees set `interest_rate`, and the
/// rates-credit tree sets `interest_rate`, `hazard_rate` and (at interior
/// nodes) `df`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NodeState {
    /// Time step index (0 to N)
    pub step: usize,
    /// Option-adjusted spread the tree is being priced at, in basis points
    /// (continuously compounded). Zero for trees priced without a spread.
    pub oas_bp: f64,
    /// Underlying asset price at this node
    pub spot: Option<f64>,
    /// Short rate at this node as a decimal annual rate
    pub interest_rate: Option<f64>,
    /// Hazard rate (default intensity) at this node as a decimal annual rate
    pub hazard_rate: Option<f64>,
    /// Discount factor over the interval starting at this node
    pub df: Option<f64>,
}

/// Per-node discount-factor conventions of the short-rate trees, expressed
/// over the canonical core [`Compounding`].
///
/// | Convention | Formula | Use Case |
/// |------------|---------|----------|
/// | `Continuous` | `exp(-r * dt)` | Default; matches continuous short-rate dynamics |
/// | `Simple` | `1 / (1 + r * dt)` | Money-market / Bloomberg BDT convention |
/// | `Annual` / `Periodic(n)` | `(1 + r/n)^(-n * dt)` | Bond-market conventions (`n = 2` US) |
///
/// The bases are clamped to a small positive floor so that pathological
/// inputs (deeply negative rates) never produce negative or NaN factors.
pub trait TreeDiscounting: Copy {
    /// Per-step discount factor for `rate` over the year-fraction step `dt`.
    fn tree_df(self, rate: f64, dt: f64) -> f64;

    /// Invert [`tree_df`](Self::tree_df): the per-step rate reproducing `df`
    /// over `dt`. Returns `0.0` (with a warning) for `dt ≈ 0` or `df <= 0`.
    fn tree_rate_from_df(self, df: f64, dt: f64) -> f64;

    /// Continuous rate `r_cont` with `exp(-r_cont * dt) = tree_df(rate, dt)`.
    fn tree_to_continuous(self, rate: f64, dt: f64) -> f64;
}

impl TreeDiscounting for Compounding {
    #[inline]
    fn tree_df(self, rate: f64, dt: f64) -> f64 {
        const FLOOR: f64 = 1e-15;
        match (self, self.periods_per_year().map(f64::from)) {
            (_, Some(n)) if 1.0 + rate / n < FLOOR => FLOOR.powf(-n * dt),
            (Compounding::Simple, None) if 1.0 + rate * dt < FLOOR => 1.0 / FLOOR,
            _ => self.df_from_rate(rate, dt),
        }
    }

    #[inline]
    fn tree_rate_from_df(self, df: f64, dt: f64) -> f64 {
        if dt.abs() < f64::EPSILON || df <= 0.0 {
            tracing::warn!(
                "tree_rate_from_df: degenerate input df={df:.6e}, dt={dt}, \
                 convention={self:?}; returning 0"
            );
            return 0.0;
        }
        self.rate_from_df(df, dt)
    }

    #[inline]
    fn tree_to_continuous(self, rate: f64, dt: f64) -> f64 {
        if dt.abs() < f64::EPSILON {
            return rate;
        }
        let d = self.tree_df(rate, dt);
        if d > 0.0 {
            -d.ln() / dt
        } else {
            tracing::warn!(
                "tree_to_continuous: non-positive DF {d:.6e} for rate={rate}, \
                 dt={dt}, convention={self:?}; falling back to raw rate"
            );
            rate
        }
    }
}

/// Node values and discounting of the lattice driving the induction.
#[derive(Clone, Copy)]
pub(crate) enum RecombiningLattice<'a> {
    /// Multiplicative asset-price lattice discounted at a flat rate. Node
    /// values reach the valuator as [`NodeState::spot`].
    Spot {
        /// Asset price at the root node
        spot: f64,
        /// Multiplicative factor for up move (e.g., exp(σ√dt))
        up_factor: f64,
        /// Multiplicative factor for down move (e.g., exp(-σ√dt))
        down_factor: f64,
        /// Continuously compounded risk-free rate per annum used for discounting
        interest_rate: f64,
    },
    /// Calibrated short-rate lattice. Node values reach the valuator as
    /// [`NodeState::interest_rate`].
    ShortRate {
        /// Short rate at `(step, node)`
        node_rate: &'a dyn Fn(usize, usize) -> f64,
        /// Continuously compounded discounting rate at `(step, node)`,
        /// including any option-adjusted spread
        discount_rate: &'a dyn Fn(usize, usize) -> f64,
        /// Continuously compounded option-adjusted spread in basis points at
        /// `(step, node)`, passed to the valuator as [`NodeState::oas_bp`]
        oas_bp: &'a dyn Fn(usize, usize) -> f64,
    },
}

/// Inputs for the shared binomial recombining engine.
#[derive(Clone)]
pub(crate) struct RecombiningInputs<'a, V: TreeValuator> {
    /// Number of time steps in the tree
    pub(crate) steps: usize,
    /// Time to maturity in years
    pub(crate) time_to_maturity: f64,
    /// Payoff valuator implementing TreeValuator trait
    pub(crate) valuator: &'a V,
    /// Risk-neutral probability of up move
    pub(crate) prob_up: f64,
    /// Risk-neutral probability of down move
    pub(crate) prob_down: f64,
    /// Node values and discounting
    pub(crate) lattice: RecombiningLattice<'a>,
}

/// Price an instrument on a binomial recombining tree with backward induction.
///
/// Node `i` at step `n` has `i` up moves and `n - i` down moves. Payoffs are
/// evaluated at maturity and expected values are discounted backward to the
/// root. The evolving node value is the spot (equity trees) or the short rate
/// (short-rate trees), handed to the valuator in the matching [`NodeState`]
/// field.
///
/// # Arguments
///
/// * `inputs` - Complete tree configuration: step count, horizon in years,
///   valuator, branch probabilities and the lattice supplying node values and
///   discounting
///
/// # Returns
///
/// Present value of the instrument at time 0
pub(crate) fn price_recombining_tree<V: TreeValuator>(
    inputs: RecombiningInputs<'_, V>,
) -> Result<f64> {
    if inputs.steps == 0 || !inputs.time_to_maturity.is_finite() || inputs.time_to_maturity <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "price_recombining_tree requires steps > 0 and finite positive time_to_maturity \
             (got steps={}, time_to_maturity={})",
            inputs.steps, inputs.time_to_maturity
        )));
    }
    let dt = inputs.time_to_maturity / inputs.steps as f64;

    // Constant discount factor of the flat-rate spot lattice.
    let flat_df = match inputs.lattice {
        RecombiningLattice::Spot { interest_rate, .. } => (-interest_rate * dt).exp(),
        RecombiningLattice::ShortRate { .. } => 1.0,
    };
    let get_df = |step: usize, node: usize| -> f64 {
        match inputs.lattice {
            RecombiningLattice::Spot { .. } => flat_df,
            RecombiningLattice::ShortRate { discount_rate, .. } => {
                (-discount_rate(step, node) * dt).exp()
            }
        }
    };

    let state_for = |step: usize, node: usize, node_value: f64| -> NodeState {
        match inputs.lattice {
            RecombiningLattice::Spot { interest_rate, .. } => NodeState {
                step,
                spot: Some(node_value),
                interest_rate: Some(interest_rate),
                ..NodeState::default()
            },
            RecombiningLattice::ShortRate { oas_bp, .. } => NodeState {
                step,
                oas_bp: oas_bp(step, node),
                interest_rate: Some(node_value),
                ..NodeState::default()
            },
        }
    };

    // Node values for one level. The spot lattice is walked incrementally
    // from `spot * d^step`, multiplying by `u/d` per node.
    let level_values = |step: usize, out: &mut Vec<f64>| {
        out.clear();
        match inputs.lattice {
            RecombiningLattice::ShortRate { node_rate, .. } => {
                out.extend((0..=step).map(|i| node_rate(step, i)));
            }
            RecombiningLattice::Spot {
                spot,
                up_factor,
                down_factor,
                ..
            } => {
                let ud_ratio = up_factor / down_factor;
                let mut value = spot * down_factor.powi(step as i32);
                for i in 0..=step {
                    out.push(value);
                    if i < step {
                        value *= ud_ratio;
                    }
                }
            }
        }
    };

    let mut level = Vec::with_capacity(inputs.steps + 1);
    level_values(inputs.steps, &mut level);
    let mut values = Vec::with_capacity(inputs.steps + 1);
    for (i, &node_value) in level.iter().enumerate() {
        let terminal_state = state_for(inputs.steps, i, node_value);
        values.push(inputs.valuator.value_at_maturity(&terminal_state)?);
    }

    for step in (0..inputs.steps).rev() {
        level_values(step, &mut level);
        for (i, &node_value) in level.iter().enumerate() {
            let continuation =
                get_df(step, i) * (inputs.prob_up * values[i + 1] + inputs.prob_down * values[i]);
            let node_state = state_for(step, i, node_value);
            values[i] = inputs
                .valuator
                .value_at_node(&node_state, continuation, dt)?;
        }
        values.pop();
    }

    Ok(values[0])
}

/// Multiplicative factors and transition probabilities for a CRR equity step.
#[derive(Debug, Clone)]
pub struct EvolutionParams {
    /// Annualized lognormal volatility as a decimal.
    pub volatility: f64,
    /// Gross multiplicative spot factor on an upward move.
    pub up_factor: f64,
    /// Gross multiplicative spot factor on a downward move.
    pub down_factor: f64,
    /// Risk-neutral upward probability in `[0, 1]`.
    pub prob_up: f64,
    /// Risk-neutral downward probability, equal to `1 - prob_up`.
    pub prob_down: f64,
}

impl EvolutionParams {
    /// Create evolution parameters for a single equity factor (CRR model).
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when the implied risk-neutral
    /// probability falls outside `[0, 1]` or any factor is non-positive (which
    /// happens for extreme combinations of `vol`, `drift`, `dt`). Release builds
    /// must enforce this — silent arbitrage in lattice probabilities is a
    /// production hazard.
    ///
    /// # Arguments
    ///
    /// * `volatility` - Annualized volatility in decimal units.
    /// * `risk_free_rate` - Risk free rate used by the algorithm, subject to the enclosing type invariants and documented units.
    /// * `dividend_yield` - Continuously compounded annual dividend yield in decimal units.
    /// * `dt` - Positive time-step width in year-fraction units.
    pub fn equity_crr(
        volatility: f64,
        risk_free_rate: f64,
        dividend_yield: f64,
        dt: f64,
    ) -> finstack_quant_core::Result<Self> {
        if !volatility.is_finite()
            || volatility < 0.0
            || !risk_free_rate.is_finite()
            || !dividend_yield.is_finite()
            || !dt.is_finite()
            || dt <= 0.0
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CRR evolution requires finite inputs with volatility >= 0 and dt > 0 \
                 (got vol={volatility}, r={risk_free_rate}, q={dividend_yield}, dt={dt})"
            )));
        }
        let u = (volatility * dt.sqrt()).exp();
        let d = 1.0 / u;
        let spread = u - d;
        // Guard: if vol*sqrt(dt) is so small that u ≈ d (spread underflows to 0),
        // the probability formula produces 0/0 = NaN.  Catch it explicitly
        // before the division so the error message is descriptive.
        if spread < 1e-14 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CRR evolution is degenerate: u ≈ d (spread = {spread:.3e}). \
                 vol·√dt = {:.3e} is too small — increase volatility or time step.",
                volatility * dt.sqrt()
            )));
        }
        let drift = risk_free_rate - dividend_yield;
        let p = ((drift * dt).exp() - d) / spread;

        if !(0.0..=1.0).contains(&p) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CRR probability p={p:.6} out of bounds [0,1] for vol={volatility}, \
                 r={risk_free_rate}, q={dividend_yield}, dt={dt:.3e}"
            )));
        }
        if !(u > 0.0 && d > 0.0) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CRR up/down factors must be positive: u={u}, d={d}"
            )));
        }

        Ok(Self {
            volatility,
            up_factor: u,
            down_factor: d,
            prob_up: p,
            prob_down: 1.0 - p,
        })
    }

    /// Recompute the risk-neutral branch probabilities for a different (per-step)
    /// drift, keeping the up/down factors fixed.
    ///
    /// The up/down factors of the CRR lattice depend only on
    /// `volatility` and `dt`, so re-deriving the parameters with a new drift
    /// leaves the node grid unchanged and the lattice recombining; only the
    /// branch probabilities vary. This lets a pricer drive each step's drift
    /// from the same per-step forward rate it uses for discounting, keeping the
    /// tree's stock forward consistent with a non-flat discount curve
    /// (martingale property holds step by step).
    ///
    /// # Arguments
    ///
    /// * `drift` - Net continuously compounded risk-neutral annual drift as a decimal, such as the step's forward rate minus dividend yield.
    /// * `dt` - Finite positive step width in years; the same width used to construct the spatial factors.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when the implied
    /// probabilities for the new drift fall outside `[0, 1]` (same checks as
    /// [`EvolutionParams::equity_crr`]).
    pub fn with_drift(&self, drift: f64, dt: f64) -> finstack_quant_core::Result<Self> {
        // Passing the drift as the rate with q = 0 reproduces the same net
        // drift; up/down factors depend only on (volatility, dt) and are identical.
        Self::equity_crr(self.volatility, drift, 0.0, dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// W-07: equity_crr with vol·√dt underflowing to 0 must return a descriptive
    /// "degenerate" error, not silently NaN-poison the probability.
    #[test]
    fn test_w07_equity_crr_degenerate_vol_sqrt_dt_returns_descriptive_error() {
        // vol so small that vol * sqrt(dt) underflows to 0 in f64
        let epsilon_vol = 5e-162; // * sqrt(0.01) = 5e-163 → underflows
        let dt = 0.01_f64;
        let err = EvolutionParams::equity_crr(epsilon_vol, 0.05, 0.0, dt)
            .expect_err("equity_crr with degenerate vol should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("degenerate"),
            "equity_crr degenerate error must mention 'degenerate', got: {msg}"
        );
    }

    /// Valuator recording the node state it is handed.
    struct StateProbe {
        seen: std::sync::Mutex<Vec<NodeState>>,
    }

    impl TreeValuator for StateProbe {
        fn value_at_maturity(&self, state: &NodeState) -> Result<f64> {
            self.record(state);
            Ok(0.0)
        }

        fn value_at_node(
            &self,
            state: &NodeState,
            continuation_value: f64,
            _dt: f64,
        ) -> Result<f64> {
            self.record(state);
            Ok(continuation_value)
        }
    }

    impl StateProbe {
        fn record(&self, state: &NodeState) {
            if let Ok(mut seen) = self.seen.lock() {
                seen.push(*state);
            }
        }

        fn price(lattice: RecombiningLattice<'_>) -> Vec<NodeState> {
            let probe = Self {
                seen: std::sync::Mutex::new(Vec::new()),
            };
            price_recombining_tree(RecombiningInputs {
                steps: 1,
                time_to_maturity: 1.0,
                valuator: &probe,
                prob_up: 0.5,
                prob_down: 0.5,
                lattice,
            })
            .expect("price");
            probe.seen.into_inner().expect("probe states")
        }
    }

    #[test]
    fn spot_lattice_hands_spot_and_flat_rate_to_the_valuator() {
        let seen = StateProbe::price(RecombiningLattice::Spot {
            spot: 100.0,
            up_factor: 2.0,
            down_factor: 0.5,
            interest_rate: 0.03,
        });
        let node = |step: usize, spot: f64| NodeState {
            step,
            spot: Some(spot),
            interest_rate: Some(0.03),
            ..NodeState::default()
        };
        assert_eq!(seen, [node(1, 50.0), node(1, 200.0), node(0, 100.0)]);
        assert_eq!(seen[0].spot, Some(50.0));
        assert_eq!(seen[0].interest_rate, Some(0.03));
        assert_eq!(seen[0].hazard_rate, None);
        assert_eq!(seen[0].df, None);
    }

    #[test]
    fn short_rate_lattice_hands_node_rate_and_oas_to_the_valuator() {
        let node_rate = |step: usize, node: usize| 0.01 * (step + node) as f64;
        let discount_rate = |_step: usize, _node: usize| 0.0;
        let oas_bp = |_step: usize, _node: usize| 125.0;
        let seen = StateProbe::price(RecombiningLattice::ShortRate {
            node_rate: &node_rate,
            discount_rate: &discount_rate,
            oas_bp: &oas_bp,
        });
        let node = |step: usize, rate: f64| NodeState {
            step,
            oas_bp: 125.0,
            interest_rate: Some(rate),
            ..NodeState::default()
        };
        assert_eq!(seen, [node(1, 0.01), node(1, 0.02), node(0, 0.0)]);
    }

    #[test]
    fn crr_evolution_satisfies_probability_invariants() {
        let crr = EvolutionParams::equity_crr(0.2, 0.05, 0.01, 0.25).expect("valid CRR params");
        assert!(crr.up_factor > 1.0);
        assert!(crr.down_factor < 1.0);
        assert!((crr.up_factor * crr.down_factor - 1.0).abs() < 1e-12);
        assert!(crr.prob_up >= 0.0 && crr.prob_up <= 1.0);
        assert!(crr.prob_down >= 0.0 && crr.prob_down <= 1.0);
        assert!((crr.prob_up + crr.prob_down - 1.0).abs() < 1e-12);
    }

    #[test]
    fn evolution_params_crr_rejects_unstable_params() {
        // dt large enough relative to vol that drift kicks p out of [0, 1].
        // Combined with extreme drift, the implied probability falls below zero
        // (or above one) — release builds must surface this rather than silently
        // produce an arbitrage-violating tree.
        let result = EvolutionParams::equity_crr(0.05, 5.0, 0.0, 1.0);
        assert!(
            result.is_err(),
            "CRR with extreme drift/vol/dt must error, not silently corrupt the tree"
        );
    }
}

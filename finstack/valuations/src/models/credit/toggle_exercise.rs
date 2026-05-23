//! Toggle exercise models for PIK/cash coupon decisions.
//!
//! Models the borrower's decision to pay-in-kind (PIK) or pay cash at each
//! coupon date. The toggle decision depends on observable credit state and can
//! follow a hard threshold rule, a stochastic (sigmoid) model, or an optimal
//! exercise strategy via nested Monte Carlo.
//!
//! # Supported Models
//!
//! - **Threshold**: PIK when a credit metric crosses a boundary (above or below).
//! - **Stochastic**: PIK probability is a smooth sigmoid function of credit state.
//! - **OptimalExercise**: Nested MC for optimal toggle decision.
//!
//! # Examples
//!
//! ```
//! use finstack_valuations::models::credit::toggle_exercise::{
//!     CreditState, CreditStateVariable, ThresholdDirection, ToggleExerciseModel,
//! };
//! use finstack_core::math::random::Pcg64Rng;
//!
//! let model = ToggleExerciseModel::threshold(
//!     CreditStateVariable::HazardRate, 0.15, ThresholdDirection::Above,
//! );
//! let state = CreditState { hazard_rate: 0.20, ..Default::default() };
//! let mut rng = Pcg64Rng::new(42);
//! assert!(model.should_pik(&state, &mut rng));
//! ```

use finstack_core::math::random::{Pcg64Rng, RandomNumberGenerator};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Observable credit state at a point in time.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreditState {
    /// Current hazard rate (annualised instantaneous default intensity).
    pub hazard_rate: f64,
    /// Distance-to-default (number of standard deviations from the default point).
    pub distance_to_default: Option<f64>,
    /// Leverage ratio (debt / assets).
    pub leverage: f64,
    /// Accreted (PIK-augmented) notional outstanding.
    pub accreted_notional: f64,
    /// Cash coupon amount due at this decision date.
    #[serde(default)]
    pub coupon_due: f64,
    /// Fair value of the firm's assets, if available.
    pub asset_value: Option<f64>,
}

/// Which credit metric drives the toggle decision.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum CreditStateVariable {
    /// Use the hazard rate.
    HazardRate,
    /// Use the distance-to-default.
    DistanceToDefault,
    /// Use the leverage ratio.
    Leverage,
}

impl FromStr for CreditStateVariable {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "hazard_rate" => Ok(Self::HazardRate),
            "distance_to_default" => Ok(Self::DistanceToDefault),
            "leverage" => Ok(Self::Leverage),
            other => Err(format!("unknown credit state variable: {other}")),
        }
    }
}

/// Direction for threshold comparison.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ThresholdDirection {
    /// PIK when state > threshold (e.g., hazard rate above limit).
    Above,
    /// PIK when state < threshold (e.g., distance-to-default below limit).
    Below,
}

impl FromStr for ThresholdDirection {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "above" => Ok(Self::Above),
            "below" => Ok(Self::Below),
            other => Err(format!("unknown threshold direction: {other}")),
        }
    }
}

/// Toggle exercise model for PIK/cash decision.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ToggleExerciseModel {
    /// Hard threshold: PIK when credit metric crosses boundary.
    Threshold(ThresholdToggle),
    /// Stochastic: PIK probability is smooth sigmoid of credit state.
    Stochastic(StochasticToggle),
    /// Optimal exercise via nested Monte Carlo simulation.
    OptimalExercise(OptimalToggle),
}

/// Hard threshold toggle configuration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ThresholdToggle {
    /// Credit metric to observe.
    pub state_variable: CreditStateVariable,
    /// Threshold value.
    pub threshold: f64,
    /// Direction for comparison.
    pub direction: ThresholdDirection,
}

/// Stochastic (sigmoid) toggle configuration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StochasticToggle {
    /// Credit metric to observe.
    pub state_variable: CreditStateVariable,
    /// Intercept of the logistic function: `P(PIK) = 1 / (1 + exp(-(intercept + sensitivity * state)))`.
    pub intercept: f64,
    /// Sensitivity (slope) of the logistic function with respect to the state variable.
    pub sensitivity: f64,
}

/// Optimal toggle configuration using nested Monte Carlo simulation.
///
/// At each coupon date the toggle runs a small nested MC to estimate the
/// equity value (call-option payoff on the firm's assets) under two
/// scenarios:
///
/// 1. **Cash** – the firm pays out the coupon, reducing asset value by
///    the coupon amount while notional stays unchanged.
/// 2. **PIK** – the coupon accretes to notional (no cash outflow), so
///    asset value is preserved but the default barrier rises.
///
/// PIK is elected when the estimated equity value under PIK exceeds
/// that under cash.  The nested simulation uses a simple GBM forward
/// evolution of asset value with a first-passage barrier check.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OptimalToggle {
    /// Number of nested Monte Carlo paths for continuation value estimation.
    /// Recommended range: 100–500.
    pub nested_paths: usize,
    /// Equity holder discount rate applied to the nested-MC payoff.
    ///
    /// Note: the nested simulation drifts under the risk-neutral measure
    /// (`risk_free_rate`) but discounts here, so the intermediate equity
    /// figures are decision inputs, not measure-consistent prices. The
    /// rate cancels in the cash-vs-PIK comparison and does not bias the
    /// elected branch.
    pub equity_discount_rate: f64,
    /// Annualised asset volatility for the nested GBM simulation.
    pub asset_vol: f64,
    /// Risk-free rate (continuous) used as drift in the nested simulation.
    pub risk_free_rate: f64,
    /// Forward-looking horizon in years for the nested simulation (e.g. 1.0).
    pub horizon: f64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the relevant state value for the toggle decision.
fn extract_state_value(state: &CreditState, variable: &CreditStateVariable) -> f64 {
    match variable {
        CreditStateVariable::HazardRate => state.hazard_rate,
        CreditStateVariable::DistanceToDefault => state.distance_to_default.unwrap_or(0.0),
        CreditStateVariable::Leverage => state.leverage,
    }
}

// ---------------------------------------------------------------------------
// Optimal toggle: nested Monte Carlo
// ---------------------------------------------------------------------------

/// Number of time steps per year in the nested GBM simulation.
const NESTED_STEPS_PER_YEAR: usize = 12;

/// Run a small nested Monte Carlo to decide cash vs PIK.
///
/// For each scenario (cash / PIK) we simulate `o.nested_paths` forward paths
/// of asset value using geometric Brownian motion over `o.horizon` years,
/// checking for a first-passage barrier breach at every time step.  The
/// equity value is `max(V(T) - barrier, 0)` discounted back; if the firm
/// defaults along the path the equity value is zero.
///
/// Returns `true` (elect PIK) when the estimated equity value under PIK
/// exceeds the equity value under cash.
///
/// # Measure note
///
/// The nested simulation uses a risk-neutral GBM drift (`risk_free_rate`)
/// but discounts the resulting payoff at the equity holder's
/// `equity_discount_rate`. These two rates need not agree, so the
/// `avg_equity_*` figures are **decision-only quantities, not equity
/// prices** — a measure-consistent valuation would discount at the same
/// rate used for the drift. Because the cash and PIK branches share an
/// identical model (same drift, same discount, same paths), the
/// measure inconsistency is a common factor that cancels in the
/// `pik > cash` comparison, so the toggle *decision* is unaffected. The
/// `equity_discount_rate` is therefore retained as a tunable knob that
/// does not bias the elected branch (see the `discount_rate_invariant`
/// test).
fn optimal_toggle_decision(
    o: &OptimalToggle,
    state: &CreditState,
    rng: &mut dyn RandomNumberGenerator,
) -> bool {
    let seed_bits = rng.next_u64();
    optimal_toggle_decision_seeded(o, state, seed_bits)
}

/// Derive a full-entropy `u64` seed from a uniform draw `u` in `[0, 1)`.
///
/// The naive `(u * u64::MAX as f64) as u64` is low-entropy: a `[0, 1)` double
/// carries at most ~52 mantissa bits, and scaling then truncating leaves the
/// low ~11 bits of the result effectively zero, so a whole band of distinct
/// uniforms collapses onto the same seed — weakening the antithetic toggle
/// pairing in [`should_pik_with_uniform`](ToggleExerciseModel::should_pik_with_uniform).
///
/// Instead, capture **every** bit of `u` losslessly via [`f64::to_bits`] and
/// run it through the SplitMix64 finalizer (Steele, Lea & Flood, 2014), whose
/// avalanche property diffuses each input bit across all 64 output bits. Two
/// adjacent doubles therefore produce well-separated, distinct seeds.
#[inline]
fn uniform_to_seed(u: f64) -> u64 {
    // SplitMix64 finalizing mix: a bijection on `u64`, so distinct inputs map
    // to distinct outputs (no collisions) while achieving full avalanche.
    let mut z = u.to_bits();
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn optimal_toggle_decision_seeded(o: &OptimalToggle, state: &CreditState, seed_bits: u64) -> bool {
    let v = state.asset_value.unwrap_or_else(|| {
        if state.leverage > 0.0 {
            state.accreted_notional / state.leverage
        } else {
            state.accreted_notional * 2.0
        }
    });
    let n = state.accreted_notional;

    if n <= 0.0 {
        return false;
    }

    let coupon = state.coupon_due.max(0.0);
    if coupon == 0.0 {
        return false;
    }

    let v_cash_start = (v - coupon).max(0.0);
    let barrier_cash = n;

    let v_pik_start = v;
    let barrier_pik = n + coupon;

    if v_cash_start <= barrier_cash {
        return true;
    }
    if v_pik_start <= barrier_pik {
        return false;
    }

    let model_cash = NestedEquityMcModel {
        sigma: o.asset_vol,
        risk_free_rate: o.risk_free_rate,
        discount_rate: o.equity_discount_rate,
        horizon: o.horizon,
    };

    let model_pik = NestedEquityMcModel {
        sigma: o.asset_vol,
        risk_free_rate: o.risk_free_rate,
        discount_rate: o.equity_discount_rate,
        horizon: o.horizon,
    };

    let avg_equity_cash = nested_equity_mc(
        o.nested_paths,
        v_cash_start,
        barrier_cash,
        model_cash,
        seed_bits,
    );

    let avg_equity_pik = nested_equity_mc(
        o.nested_paths,
        v_pik_start,
        barrier_pik,
        model_pik,
        seed_bits ^ 0x9E37_79B9_7F4A_7C15,
    );

    avg_equity_pik > avg_equity_cash
}

/// Estimate `E[max(V(T) - barrier, 0)]` via simple GBM with first-passage
/// default check, discounted at `discount_rate`.
fn nested_equity_mc(
    num_paths: usize,
    v_start: f64,
    barrier: f64,
    model: NestedEquityMcModel,
    base_seed: u64,
) -> f64 {
    let NestedEquityMcModel {
        sigma,
        risk_free_rate,
        discount_rate,
        horizon,
    } = model;

    if num_paths == 0 || horizon <= 0.0 || v_start <= 0.0 {
        return 0.0;
    }

    // Already in default at start.
    if v_start <= barrier {
        return 0.0;
    }

    let n_steps = ((horizon * NESTED_STEPS_PER_YEAR as f64).ceil() as usize).max(1);
    let dt = horizon / n_steps as f64;
    let sqrt_dt = dt.sqrt();
    let drift = (risk_free_rate - 0.5 * sigma * sigma) * dt;
    let discount_factor = (-discount_rate * horizon).exp();

    let mut total_payoff = 0.0;

    for path_idx in 0..num_paths {
        let mut nested_rng = Pcg64Rng::new_with_stream(base_seed, path_idx as u64);
        let mut v = v_start;
        let mut defaulted = false;

        for _step in 0..n_steps {
            let z = nested_rng.normal(0.0, 1.0);
            v *= (drift + sigma * sqrt_dt * z).exp();

            if v <= barrier {
                defaulted = true;
                break;
            }
        }

        if !defaulted {
            let equity = (v - barrier).max(0.0);
            total_payoff += equity * discount_factor;
        }
    }

    total_payoff / num_paths as f64
}

#[derive(Debug, Clone, Copy)]
struct NestedEquityMcModel {
    sigma: f64,
    risk_free_rate: f64,
    discount_rate: f64,
    horizon: f64,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl ToggleExerciseModel {
    /// Create a threshold toggle model.
    #[must_use]
    pub fn threshold(
        variable: CreditStateVariable,
        threshold: f64,
        direction: ThresholdDirection,
    ) -> Self {
        Self::Threshold(ThresholdToggle {
            state_variable: variable,
            threshold,
            direction,
        })
    }

    /// Create a stochastic (sigmoid) toggle model.
    #[must_use]
    pub fn stochastic(variable: CreditStateVariable, intercept: f64, sensitivity: f64) -> Self {
        Self::Stochastic(StochasticToggle {
            state_variable: variable,
            intercept,
            sensitivity,
        })
    }

    /// Returns `true` if the borrower elects PIK at this coupon date.
    pub fn should_pik(&self, state: &CreditState, rng: &mut dyn RandomNumberGenerator) -> bool {
        match self {
            Self::Threshold(t) => {
                let value = extract_state_value(state, &t.state_variable);
                match t.direction {
                    ThresholdDirection::Above => value > t.threshold,
                    ThresholdDirection::Below => value < t.threshold,
                }
            }
            Self::Stochastic(s) => {
                let value = extract_state_value(state, &s.state_variable);
                let p = 1.0 / (1.0 + (-s.intercept - s.sensitivity * value).exp());
                rng.uniform() < p
            }
            Self::OptimalExercise(o) => optimal_toggle_decision(o, state, rng),
        }
    }

    /// Deterministic variant of [`should_pik`](Self::should_pik) that uses a
    /// pre-generated uniform draw `u` in `[0, 1)` instead of pulling from a
    /// mutable RNG.
    ///
    /// This enables antithetic MC paths to share identical toggle randomness
    /// across the base and antithetic pair, preserving variance reduction
    /// effectiveness when toggle decisions are active.
    pub fn should_pik_with_uniform(&self, state: &CreditState, u: f64) -> bool {
        match self {
            Self::Threshold(t) => {
                let value = extract_state_value(state, &t.state_variable);
                match t.direction {
                    ThresholdDirection::Above => value > t.threshold,
                    ThresholdDirection::Below => value < t.threshold,
                }
            }
            Self::Stochastic(s) => {
                let value = extract_state_value(state, &s.state_variable);
                let p = 1.0 / (1.0 + (-s.intercept - s.sensitivity * value).exp());
                u < p
            }
            Self::OptimalExercise(o) => {
                // Derive the nested-MC seed with full entropy: see
                // `uniform_to_seed`. The naive `(u * u64::MAX as f64) as u64`
                // discards the low bits of `u` and weakens antithetic pairing.
                let seed_bits = uniform_to_seed(u);
                optimal_toggle_decision_seeded(o, state, seed_bits)
            }
        }
    }

    /// Returns the PIK fraction in `[0, 1]`.
    ///
    /// For threshold: returns `0.0` or `1.0`.
    /// For stochastic: returns `0.0` or `1.0` (sampled from probability).
    /// For optimal exercise: returns `0.0` or `1.0` (nested MC decision).
    pub fn pik_fraction(&self, state: &CreditState, rng: &mut dyn RandomNumberGenerator) -> f64 {
        if self.should_pik(state, rng) {
            1.0
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::math::random::Pcg64Rng;

    #[test]
    fn parses_credit_state_variable_names() {
        assert!(matches!(
            "hazard_rate".parse::<CreditStateVariable>(),
            Ok(CreditStateVariable::HazardRate)
        ));
        assert!(matches!(
            "distance_to_default".parse::<CreditStateVariable>(),
            Ok(CreditStateVariable::DistanceToDefault)
        ));
        assert!(matches!(
            "leverage".parse::<CreditStateVariable>(),
            Ok(CreditStateVariable::Leverage)
        ));
        assert_eq!(
            "spread".parse::<CreditStateVariable>().unwrap_err(),
            "unknown credit state variable: spread"
        );
    }

    #[test]
    fn parses_threshold_direction_names() {
        assert!(matches!(
            "above".parse::<ThresholdDirection>(),
            Ok(ThresholdDirection::Above)
        ));
        assert!(matches!(
            "below".parse::<ThresholdDirection>(),
            Ok(ThresholdDirection::Below)
        ));
        assert_eq!(
            "crossed".parse::<ThresholdDirection>().unwrap_err(),
            "unknown threshold direction: crossed"
        );
    }

    #[test]
    fn threshold_piks_above_threshold() {
        let model = ToggleExerciseModel::threshold(
            CreditStateVariable::HazardRate,
            0.15,
            ThresholdDirection::Above,
        );
        let mut rng = Pcg64Rng::new(42);
        let state_low = CreditState {
            hazard_rate: 0.10,
            ..Default::default()
        };
        let state_high = CreditState {
            hazard_rate: 0.20,
            ..Default::default()
        };
        assert!(!model.should_pik(&state_low, &mut rng));
        assert!(model.should_pik(&state_high, &mut rng));
    }

    #[test]
    fn threshold_piks_below_threshold() {
        let model = ToggleExerciseModel::threshold(
            CreditStateVariable::DistanceToDefault,
            2.0,
            ThresholdDirection::Below,
        );
        let mut rng = Pcg64Rng::new(42);
        let state_safe = CreditState {
            distance_to_default: Some(3.0),
            ..Default::default()
        };
        let state_stressed = CreditState {
            distance_to_default: Some(1.5),
            ..Default::default()
        };
        assert!(!model.should_pik(&state_safe, &mut rng));
        assert!(model.should_pik(&state_stressed, &mut rng));
    }

    #[test]
    fn stochastic_toggle_probability_increases_with_hazard() {
        let model = ToggleExerciseModel::stochastic(CreditStateVariable::HazardRate, -3.0, 20.0);
        // Run 10k samples at lambda=0.10 and lambda=0.20
        let count_low: usize = (0..10_000)
            .filter(|i| {
                let mut rng = Pcg64Rng::new(42 + *i as u64);
                let state = CreditState {
                    hazard_rate: 0.10,
                    ..Default::default()
                };
                model.should_pik(&state, &mut rng)
            })
            .count();
        let count_high: usize = (0..10_000)
            .filter(|i| {
                let mut rng = Pcg64Rng::new(42 + *i as u64);
                let state = CreditState {
                    hazard_rate: 0.20,
                    ..Default::default()
                };
                model.should_pik(&state, &mut rng)
            })
            .count();
        assert!(
            count_high > count_low,
            "Higher hazard should have more PIK: low={count_low}, high={count_high}"
        );
    }

    #[test]
    fn pik_fraction_returns_0_or_1_for_threshold() {
        let model = ToggleExerciseModel::threshold(
            CreditStateVariable::HazardRate,
            0.15,
            ThresholdDirection::Above,
        );
        let mut rng = Pcg64Rng::new(42);
        let state_above = CreditState {
            hazard_rate: 0.20,
            ..Default::default()
        };
        let state_below = CreditState {
            hazard_rate: 0.10,
            ..Default::default()
        };
        assert!((model.pik_fraction(&state_above, &mut rng) - 1.0).abs() < 1e-10);
        assert!((model.pik_fraction(&state_below, &mut rng) - 0.0).abs() < 1e-10);
    }

    // ========================================================================
    // Optimal toggle (nested MC) tests
    // ========================================================================

    /// Helper to build an `OptimalToggle` with typical parameters.
    ///
    /// Tests set `CreditState::coupon_due` explicitly; the discount rate is
    /// only the equity-holder discount rate.
    fn make_optimal_toggle(nested_paths: usize) -> OptimalToggle {
        OptimalToggle {
            nested_paths,
            equity_discount_rate: 0.02,
            asset_vol: 0.30,
            risk_free_rate: 0.03,
            horizon: 1.0,
        }
    }

    #[test]
    fn optimal_toggle_does_not_panic() {
        let model = ToggleExerciseModel::OptimalExercise(make_optimal_toggle(100));
        let mut rng = Pcg64Rng::new(42);
        let state = CreditState {
            hazard_rate: 0.20,
            leverage: 0.50,
            accreted_notional: 100.0,
            coupon_due: 2.0,
            asset_value: Some(200.0),
            ..Default::default()
        };
        // Should not panic; result is a boolean.
        let _ = model.should_pik(&state, &mut rng);
    }

    #[test]
    fn optimal_toggle_prefers_pik_when_stressed() {
        // When the coupon exceeds the equity cushion (V - N < coupon),
        // paying cash would push asset value below the default barrier.
        // PIK preserves cash and is the only viable survival strategy.
        //
        // Setup: V=104, N=100, coupon=5.
        //   Cash: V_start = 104-5 = 99 < barrier(100) → immediate default!
        //   PIK:  V_start = 104, barrier = 105        → cushion of -1 (also tight)
        //
        // The early-exit liquidity check triggers: cash is not viable,
        // so PIK must be elected.
        let model = ToggleExerciseModel::OptimalExercise(OptimalToggle {
            nested_paths: 200,
            equity_discount_rate: 0.05,
            asset_vol: 0.30,
            risk_free_rate: 0.03,
            horizon: 1.0,
        });
        let mut rng = Pcg64Rng::new(99);
        let n = 100.0;
        let v = 104.0; // equity cushion = 4, coupon = 5 → cash breaches barrier
        let state = CreditState {
            hazard_rate: 0.30,
            distance_to_default: Some(0.5),
            leverage: n / v,
            accreted_notional: n,
            coupon_due: 5.0,
            asset_value: Some(v),
        };
        assert!(
            model.should_pik(&state, &mut rng),
            "Stressed firm where cash breaches barrier should prefer PIK"
        );
    }

    #[test]
    fn optimal_toggle_prefers_cash_on_exact_tie() {
        // With zero drift and zero volatility, paying cash and PIK leave the
        // same terminal equity value. The model breaks economic ties by paying
        // cash, avoiding unnecessary debt accretion.
        let model = ToggleExerciseModel::OptimalExercise(OptimalToggle {
            nested_paths: 32,
            equity_discount_rate: 0.02,
            asset_vol: 0.0,
            risk_free_rate: 0.0,
            horizon: 1.0,
        });
        let mut rng = Pcg64Rng::new(99);
        let n = 100.0;
        let v = n * 3.0;
        let state = CreditState {
            hazard_rate: 0.02,
            distance_to_default: Some(5.0),
            leverage: n / v,
            accreted_notional: n,
            coupon_due: 2.0,
            asset_value: Some(v),
        };
        assert!(
            !model.should_pik(&state, &mut rng),
            "Equal continuation values should tie-break to cash"
        );
    }

    #[test]
    fn optimal_toggle_deterministic_with_same_seed() {
        let model = ToggleExerciseModel::OptimalExercise(make_optimal_toggle(200));
        let state = CreditState {
            hazard_rate: 0.10,
            leverage: 0.60,
            accreted_notional: 100.0,
            coupon_due: 2.0,
            asset_value: Some(166.67),
            ..Default::default()
        };

        let mut rng1 = Pcg64Rng::new(12345);
        let result1 = model.should_pik(&state, &mut rng1);

        let mut rng2 = Pcg64Rng::new(12345);
        let result2 = model.should_pik(&state, &mut rng2);

        assert_eq!(
            result1, result2,
            "Same seed must produce the same toggle decision"
        );
    }

    #[test]
    fn optimal_toggle_returns_false_when_notional_zero() {
        let model = ToggleExerciseModel::OptimalExercise(make_optimal_toggle(100));
        let mut rng = Pcg64Rng::new(42);
        let state = CreditState {
            hazard_rate: 0.10,
            leverage: 0.0,
            accreted_notional: 0.0,
            asset_value: Some(200.0),
            ..Default::default()
        };
        assert!(
            !model.should_pik(&state, &mut rng),
            "Zero notional should return false (nothing to toggle)"
        );
    }

    #[test]
    fn optimal_toggle_decision_invariant_to_discount_rate() {
        // The nested MC drifts risk-neutrally but discounts at
        // equity_discount_rate. That discount factor scales the cash and PIK
        // averages by an identical positive constant, so the cash-vs-PIK
        // decision must not depend on the discount rate. Verify across a
        // grid of states and discount rates.
        let states = [
            CreditState {
                hazard_rate: 0.10,
                leverage: 0.60,
                accreted_notional: 100.0,
                coupon_due: 2.0,
                asset_value: Some(166.67),
                ..Default::default()
            },
            CreditState {
                hazard_rate: 0.25,
                leverage: 0.85,
                accreted_notional: 100.0,
                coupon_due: 4.0,
                asset_value: Some(120.0),
                ..Default::default()
            },
            CreditState {
                hazard_rate: 0.05,
                leverage: 0.40,
                accreted_notional: 100.0,
                coupon_due: 1.0,
                asset_value: Some(250.0),
                ..Default::default()
            },
        ];

        for state in &states {
            let mut decisions = Vec::new();
            for &discount in &[0.0_f64, 0.02, 0.05, 0.10, 0.20] {
                let o = OptimalToggle {
                    nested_paths: 256,
                    equity_discount_rate: discount,
                    asset_vol: 0.30,
                    risk_free_rate: 0.03,
                    horizon: 1.0,
                };
                // Use a fixed seed so only the discount rate varies.
                decisions.push(optimal_toggle_decision_seeded(&o, state, 777));
            }
            assert!(
                decisions.windows(2).all(|w| w[0] == w[1]),
                "Toggle decision must be invariant to equity_discount_rate; got {decisions:?}"
            );
        }
    }

    #[test]
    fn uniform_to_seed_uses_full_entropy_of_the_mantissa() {
        // The old derivation `(u * u64::MAX as f64) as u64` collapsed the
        // ~52-bit mantissa of `u` into a value whose low ~11 bits were always
        // zero: a whole band of distinct uniforms mapped to the SAME seed,
        // weakening the antithetic toggle pairing. The fixed derivation must
        // give every adjacent double its own well-separated seed.

        // (a) The two smallest-distinguishable uniforms either side of 0.5
        //     must produce distinct seeds.
        let u0 = 0.5_f64;
        let u1 = f64::from_bits(u0.to_bits() + 1); // next representable double
        let u2 = f64::from_bits(u0.to_bits() + 2);
        let s0 = uniform_to_seed(u0);
        let s1 = uniform_to_seed(u1);
        let s2 = uniform_to_seed(u2);
        assert_ne!(s0, s1, "adjacent uniforms must not collide to one seed");
        assert_ne!(s1, s2, "adjacent uniforms must not collide to one seed");
        assert_ne!(s0, s2, "adjacent uniforms must not collide to one seed");

        // (b) A unit change in the low mantissa bit must avalanche across the
        //     whole 64-bit seed, not merely nudge a few high bits. Require a
        //     healthy Hamming distance (a poor mixer leaves most bits equal).
        let hamming = (s0 ^ s1).count_ones();
        assert!(
            hamming >= 16,
            "low-bit change must avalanche across the seed (Hamming distance \
             {hamming} too small — low-entropy derivation)"
        );

        // (c) A sweep of many uniforms differing only in their lowest
        //     mantissa bits must yield all-distinct seeds.
        let base = 0.123_456_789_f64;
        let mut seeds: Vec<u64> = (0..512)
            .map(|k| uniform_to_seed(f64::from_bits(base.to_bits() + k)))
            .collect();
        let count = seeds.len();
        seeds.sort_unstable();
        seeds.dedup();
        assert_eq!(
            seeds.len(),
            count,
            "512 distinct low-bit uniforms must map to 512 distinct seeds"
        );
    }

    #[test]
    fn nested_equity_mc_zero_vol_is_deterministic() {
        // With zero vol the asset value stays constant, so the equity
        // payoff is simply max(V * exp(r*T) - barrier, 0) * df.
        let v: f64 = 150.0;
        let barrier: f64 = 100.0;
        let r: f64 = 0.05;
        let horizon: f64 = 1.0;
        let discount: f64 = 0.05;

        let model = super::NestedEquityMcModel {
            sigma: 0.0,
            risk_free_rate: r,
            discount_rate: discount,
            horizon,
        };
        let result = super::nested_equity_mc(200, v, barrier, model, 42);

        let v_terminal = v * (r * horizon).exp();
        let expected = ((v_terminal - barrier).max(0.0)) * (-discount * horizon).exp();

        assert!(
            (result - expected).abs() < 1e-6,
            "Zero-vol MC should match deterministic value: got {result}, expected {expected}"
        );
    }
}

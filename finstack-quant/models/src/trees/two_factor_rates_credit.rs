//! Two-factor binomial tree: short rate + credit hazard (intensity).
//!
//! Models the joint evolution of the risk-free short rate and the credit hazard
//! rate using correlated binomial moves. Both factors are **calibrated** to their
//! respective conditional discount and survival targets via independent
//! Arrow-Debreu forward induction, analogous to Ho-Lee calibration. Callers
//! materialize those targets on one explicit valuation-origin time grid.
//!
//! # Volatility regimes
//!
//! One lattice covers all four regimes; there is no separate rate-only or
//! hazard-only tree, and no second model key. Each factor is deterministic
//! when its volatility is zero (`None` and `0.0` are equivalent), and the
//! curves are repriced exactly either way:
//!
//! | `rate_vol` | `hazard_vol` | Behavior |
//! | --- | --- | --- |
//! | zero | zero | Deterministic rates and deterministic credit |
//! | nonzero | zero | Stochastic rates, deterministic credit |
//! | zero | nonzero | Deterministic rates, stochastic credit |
//! | nonzero | nonzero | Correlated stochastic rates and credit |
//!
//! Callers provide an explicit [`RatesCreditConfig`]. Valuation-side override
//! resolution is intentionally outside this crate.
//!
//! # Units
//!
//! Both volatilities are **absolute** (normal), on the same scale as the
//! factor itself: `rate_vol = 0.01` is 100 bp/yr of short rate, and
//! `hazard_vol = 0.02` is 2 percentage points of hazard per √year. A hazard
//! volatility is **not** a relative credit-spread volatility — a CDS-option
//! quote such as `0.35` is roughly an order of magnitude too large to use
//! directly. Convert with
//! [`models::credit::market_anchored`](crate::credit::market_anchored):
//! `σ_λ = σ_fractional · λ_ref`. Heavy
//! [`HazardFloorSaturation`] is the symptom of having skipped that step.
//!
//! # Lattice convention
//!
//! Each factor evolves on an **additive normal** binomial lattice: from a node
//! with value `x`, the up move is `x + σ√Δt` and the down move is `x - σ√Δt`,
//! where `σ` is the factor's annualized **normal** volatility. The node spacing
//! `σ√Δt` is therefore in **absolute rate units** (rate per year), not log-rate
//! units. The lattice recombines: row `k` is uniformly spaced by `2σ√Δt`.
//!
//! # Mean reversion — accuracy limits
//!
//! When a factor's mean-reversion speed `κ` is non-zero, the one-step transition
//! probability is moment-matched to a mean-reverting drift. The drift
//! `μ = −κ·(x − x_ref)` and the lattice spacing `σ√Δt` are **both in absolute
//! rate units**, so the moment-matched up-probability is dimensionally coherent:
//!
//! ```text
//! p_up = ½ + μ·√Δt / (2σ) = ½ − κ·(x − x_ref)·√Δt / (2σ)
//! ```
//!
//! Here `x_ref` is the factor's calibrated initial instantaneous rate `x₀`
//! (the level the input curve implies at `t = 0`). Because **calibration and
//! pricing use the identical per-node probability**, the forward Arrow-Debreu
//! recursion and the backward induction remain exact duals and the tree
//! **reprices the input curves exactly for any κ** — the Ho-Lee theta absorbs
//! the first-moment bias entirely.
//!
//! **However**, an additive binomial lattice has only ONE free parameter (`p`),
//! so it can match either the conditional mean **or** the conditional variance,
//! not both. This implementation matches the mean. The conditional variance of
//! one step is `σ²Δt · 4p(1−p)`, which collapses as `p` moves away from ½.
//! Consequence: **option-value accuracy degrades as κ grows**. Concretely, a
//! node near the reversion reference at `κ = 0.15` retains roughly 80 % of the
//! intended conditional variance; by `κ = 0.3` only ~55 % remains. This tree
//! prices callable bonds and term loans, so option-value accuracy matters.
//!
//! For accurate mean-reverting optionality, use [`HullWhiteTree`] (the
//! Hull-White trinomial tree in the same module), which does not have this
//! variance-collapse limitation.
//!
//! This implementation enforces `κ ≤ 0.15` for each factor via a
//! [`Error::Validation`] returned from `calibrate()`. See
//! [`KAPPA_MAX`] for the threshold and its justification.
//!
//! (The earlier implementation calibrated with `p = ½` but priced with a
//! mean-reversion-dependent probability, so the tree no longer repriced the
//! discount curve; it also mixed a *log-rate* drift with the *absolute-rate*
//! lattice spacing, which was dimensionally incoherent.)
//!
//! # Calibration
//!
//! `calibrate()` must be called before `price()`. The calibration ensures:
//! - Tree-implied zero-coupon bond prices match the discount curve at every step
//! - Tree-implied survival probabilities match the hazard curve at every step
//!
//! The hazard factor is additive-normal, so low nodes can go negative. A
//! negative hazard is not a credit state, so node hazards pass through a
//! non-negative transform — **in calibration and in backward induction
//! alike**. Sharing one transform is what keeps the forward Arrow-Debreu
//! recursion and the backward induction exact duals, so the survival curve the
//! valuator sees is the one that was calibrated. The floored step equation is
//! non-linear in the per-step shift, so that shift is bracketed and bisected
//! rather than read off in closed form. Where the floor binds, the realized
//! hazard dispersion is smaller than `hazard_vol` asks;
//! [`RatesCreditTree::hazard_floor_saturation`] reports how much.
//!
//! Correlation feasibility is settled at calibration time. Two Bernoulli
//! marginals only admit correlations inside their Fréchet bounds, and mean
//! reversion skews the corner nodes' marginals away from ½, so a large `|ρ|`
//! can be unattainable. `calibrate()` rejects such a request with the
//! offending node and the lattice-wide maximum; the same figure is available
//! up front from [`RatesCreditTree::max_feasible_correlation`]. Pricing
//! therefore never meets an infeasible node and never silently alters a
//! marginal to accommodate one.
//!
//! # OAS
//!
//! Option-adjusted spread is read from `initial_vars["oas"]` (basis points) and
//! applied as a parallel shift to calibrated short rates during backward induction.
//! This matches the `ShortRateTree` convention.
//!
//! # Node-dependent floating resets
//!
//! A floating coupon whose index fixes in the future re-fixes off the rate
//! node it meets, not off today's curve. [`RatesCreditTree::price_with_node_coupons`]
//! values that dependence with **two distinct operators**:
//!
//! 1. **Forward derivation** ([`RatesCreditTree::conditional_discount_factors`]):
//!    the one-period node forward comes from tree-conditional risk-free
//!    discount factors built from the **raw calibrated rate nodes** — no OAS,
//!    no survival, no positive floor. Node forwards are a property of the
//!    calibrated risk-free lattice; the OAS never moves them.
//! 2. **Pricing-measure folding**: the coupon's node-dependent *increment*
//!    over its deterministic projection is folded into continuation at the
//!    reset slice using exactly the discounting backward induction applies —
//!    raw rate **plus** OAS, floored-hazard survival, and the correlated
//!    joint transitions.
//!
//! The deterministic projection of every coupon stays booked in the
//! valuator's cashflow vectors unchanged; only the increment is
//! node-dependent, so the stochastic path collapses onto today's projected
//! cashflows as `rate_vol → 0` by construction. For an option-free FRN with
//! zero rate/credit correlation the folded increment prices to zero
//! *exactly* (the classic result that a floater's forward-set leg is worth
//! its curve projection; cf. Tuckman & Serrat, *Fixed Income Securities*,
//! ch. 2 on floaters pricing to par), which the tests assert on the lattice.
//!
//! [`HullWhiteTree`]: super::hull_white_tree::HullWhiteTree

use finstack_quant_cashflows::builder::rate_helpers::{
    calculate_floating_rate, FloatingRateParams,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::BrentSolver;
use finstack_quant_core::HashMap;
use finstack_quant_core::{Error, Result};

use crate::monte_carlo::rng::philox::PhiloxRng;
use crate::monte_carlo::traits::RandomStream;

use super::short_rate_keys;
use super::tree_framework::{CachedValues, NodeState, TreeModel, TreeValuator};

/// Maximum allowed mean-reversion speed (κ) for either factor.
///
/// The probability shift `p = ½ + μ√Δt/(2σ)` pushes `p` away from ½, and the
/// conditional variance `σ²Δt·4p(1−p)` is understated by exactly that shift.
/// Writing `d = |x − x_ref|` for a node's displacement from the reversion
/// reference, the **variance retention** `4p(1−p)` at that node is
///
/// ```text
/// retention(d) = 1 − (κ·d·√Δt / σ)²
/// ```
///
/// so retention degrades with displacement, not uniformly across the lattice.
/// Substituting the outermost node's pure-geometry displacement
/// `d_max = steps·σ√Δt`, every σ and `steps` term cancels and the worst node
/// on the lattice retains
///
/// ```text
/// retention_min ≤ 1 − (κ·T)²        (zero once κ·T ≥ 1)
/// ```
///
/// This is a **ceiling**, not an equality: the calibrated θ displaces each row
/// off the symmetric `±k·σ√Δt` geometry, so on a sloped curve the edge node
/// sits *further* from the reference and realized retention lands at or below
/// the bound — measurably so, e.g. 0.36 against a 0.44 ceiling at
/// `κ = 0.15, T = 5, σ_r = 0.012, steps = 40`. The gap narrows as σ or `steps`
/// grows and the θ term shrinks relative to the lattice width.
///
/// **The binding quantity is `κ·T`, not `κ` alone.** At `κ = 0.15` the lattice
/// edge retains at most 44 % of its intended variance over a 5-year horizon,
/// and none at all beyond `T = 1/κ ≈ 6.7` years, where
/// `RatesCreditTree::mean_reverting_up_prob` clamps `p` to `0` or `1`: those
/// nodes become locally deterministic and — being degenerate Bernoulli
/// marginals carrying no correlation — drop the configured
/// `rate_credit_correlation` entirely. At `κ = 0.15, T = 10` roughly an eighth
/// of the lattice's nodes clamp. Callable bonds and term loans routinely run
/// past that horizon, so `KAPPA_MAX` alone does not bound the error.
///
/// This constant is therefore a coarse guard, not a proof of accuracy. Read
/// [`RatesCreditTree::rate_variance_retention`] and
/// [`RatesCreditTree::hazard_variance_retention`] after calibration for what
/// the configured `(κ, σ, T, steps)` actually produced; clamped nodes sit in
/// the wings and carry little state-price mass, but the diagnostic is what
/// turns "little" into a number.
///
/// Callers needing accurate mean-reverting optionality above this threshold,
/// or over a horizon where `κ·T` approaches 1, should use [`HullWhiteTree`],
/// whose trinomial branching preserves the conditional variance exactly.
///
/// [`HullWhiteTree`]: super::hull_white_tree::HullWhiteTree
pub const KAPPA_MAX: f64 = 0.15;

/// Conditional discount and survival targets for rates-credit calibration.
///
/// Every target is measured from the pricing origin represented by
/// `times[0]`. In particular, callers valuing after a curve's base date must
/// pass conditional ratios such as `D(date) / D(as_of)` and
/// `S(date) / S(as_of)`, rather than evaluating the curve at an elapsed time
/// from its original base date. Making those values explicit prevents the
/// model from silently mixing curve and valuation origins.
///
/// The current recombining additive-normal lattice requires an evenly spaced
/// grid. `times` therefore contains `steps + 1` coordinates starting at zero,
/// and both target arrays contain one value at every coordinate. Discount
/// factors may exceed one when rates are negative; survival probabilities
/// must be non-increasing and lie in `(0, 1]`.
#[derive(Debug, Clone, PartialEq)]
pub struct RatesCreditCalibrationTargets {
    /// Evenly spaced year-fraction coordinates, starting at `0.0`.
    pub times: Vec<f64>,
    /// Conditional risk-free discount factors, with the first value equal to
    /// `1.0`.
    pub discount_factors: Vec<f64>,
    /// Conditional survival probabilities, with the first value equal to
    /// `1.0`.
    pub survival_probabilities: Vec<f64>,
    /// Fractional recovery of principal in `[0, 1]`.
    pub recovery_rate: f64,
}

/// Joint transition probabilities from one rates-credit lattice node.
///
/// `up_down` means the rate factor moves up while the hazard factor moves
/// down; the other names follow the same rate-first ordering. The four values
/// are non-negative and sum to one. Their marginals exactly preserve the
/// factor transition probabilities used during calibration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RatesCreditTransition {
    /// Probability that both the rate and hazard factors move up.
    pub up_up: f64,
    /// Probability that the rate factor moves up and the hazard factor moves
    /// down.
    pub up_down: f64,
    /// Probability that the rate factor moves down and the hazard factor moves
    /// up.
    pub down_up: f64,
    /// Probability that both the rate and hazard factors move down.
    pub down_down: f64,
}

/// One state on a sampled rates-credit lattice path.
///
/// The interval weights apply from this state to the next grid point.
/// Terminal states use the identity weights `discount_to_next = 1`,
/// `survival_to_next = 1`, and `default_to_next = 0` because no interval
/// follows them. Recovery is deliberately absent: it is a product payoff and
/// timing convention, not part of factor-path generation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RatesCreditPathState {
    /// Zero-based lattice step.
    pub step: usize,
    /// Year fraction from the calibration origin.
    pub time: f64,
    /// Rate-node index within this step.
    pub rate_node: usize,
    /// Hazard-node index within this step.
    pub hazard_node: usize,
    /// Raw calibrated short rate used for risk-free discounting.
    pub short_rate: f64,
    /// Non-negative effective hazard rate used for survival weighting.
    pub hazard_rate: f64,
    /// Risk-free discount factor from this step to the next.
    pub discount_to_next: f64,
    /// Conditional survival probability from this step to the next.
    pub survival_to_next: f64,
    /// Conditional default probability from this step to the next.
    pub default_to_next: f64,
}

/// Compact restart point for a sampled rates-credit factor path.
///
/// The Philox draw offset is the lattice `step`, so the factor node indices
/// are the only stochastic state required to resume the same `(seed,
/// path_index, antithetic)` path without replaying its prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RatesCreditPathCheckpoint {
    /// First lattice step returned by a resumed segment.
    pub step: usize,
    /// Rate-factor node index at `step`.
    pub rate_node: usize,
    /// Hazard-factor node index at `step`.
    pub hazard_node: usize,
}

impl From<&RatesCreditPathState> for RatesCreditPathCheckpoint {
    fn from(state: &RatesCreditPathState) -> Self {
        Self {
            step: state.step,
            rate_node: state.rate_node,
            hazard_node: state.hazard_node,
        }
    }
}

/// Configuration for rates + credit two-factor tree.
///
/// Mean-reversion speeds (`rate_mean_reversion` and `hazard_mean_reversion`)
/// are validated by [`RatesCreditTree::calibrate`] against [`KAPPA_MAX`].
/// Values above that threshold return a [`Error::Validation`] — use
/// [`HullWhiteTree`] instead for strong mean reversion.
///
/// [`HullWhiteTree`]: super::hull_white_tree::HullWhiteTree
#[derive(Debug, Clone)]
pub struct RatesCreditConfig {
    /// Number of time steps
    pub steps: usize,
    /// Short-rate volatility (annualized, normal / Ho-Lee convention)
    pub rate_vol: f64,
    /// Credit hazard volatility (annualized, normal convention)
    pub hazard_vol: f64,
    /// Instantaneous correlation between rate and hazard shocks
    pub correlation: f64,
    /// Mean reversion speed for the short rate (`κ_r`, annualized, `0.0` = no
    /// reversion). Must be ≤ [`KAPPA_MAX`]; the rate factor reverts toward the
    /// discount-curve-implied `t = 0` instantaneous rate.
    pub rate_mean_reversion: f64,
    /// Mean reversion speed for the hazard rate (`κ_h`, annualized, `0.0` = no
    /// reversion). Must be ≤ [`KAPPA_MAX`]; the hazard factor reverts toward
    /// the hazard-curve-implied `t = 0` instantaneous hazard.
    pub hazard_mean_reversion: f64,
}

impl Default for RatesCreditConfig {
    /// Deterministic in both factors.
    ///
    /// The default deliberately carries **zero** volatility on each factor:
    /// the lattice still reprices the discount and survival curves exactly,
    /// it just carries no diffusion. A stochastic default would make
    /// `..Default::default()` construction silently price optionality that
    /// the caller never asked for, which is exactly how the bond path
    /// inherited a `0.20` hazard vol it never declared. Callers that want a
    /// stochastic factor must say so explicitly.
    fn default() -> Self {
        Self {
            steps: 100,
            rate_vol: 0.0,
            hazard_vol: 0.0,
            correlation: 0.0,
            rate_mean_reversion: 0.0,
            hazard_mean_reversion: 0.0,
        }
    }
}

/// Two-factor correlated binomial tree (short rate + hazard rate).
///
/// Both factors are calibrated to explicit conditional targets via
/// [`Self::calibrate`]. Calling `price()` without prior calibration returns an
/// error.
#[derive(Debug, Clone)]
pub struct RatesCreditTree {
    /// Rates-credit tree configuration
    pub config: RatesCreditConfig,
    /// Calibrated short-rate affine rows. Populated by `calibrate()`.
    calibrated_rates: Vec<FactorRow>,
    /// Calibrated hazard-rate affine rows. Populated by `calibrate()`.
    calibrated_hazards: Vec<FactorRow>,
    /// Recovery target supplied to `calibrate()`.
    recovery_rate: f64,
    /// Mean-reversion reference level for the rate factor (the calibrated
    /// `t = 0` instantaneous rate `r₀`). Populated by `calibrate()`. Pricing
    /// reverts the rate factor toward this level, matching the level used
    /// during calibration so the tree reprices the discount curve.
    rate_ref: f64,
    /// Mean-reversion reference level for the hazard factor (the calibrated
    /// `t = 0` instantaneous hazard `h₀`). Populated by `calibrate()`.
    hazard_ref: f64,
    /// Hazard floor-saturation diagnostic from the most recent `calibrate()`.
    hazard_floor_saturation: HazardFloorSaturation,
    /// Rate-factor variance retention from the most recent `calibrate()`.
    rate_variance_retention: VarianceRetention,
    /// Hazard-factor variance retention from the most recent `calibrate()`.
    hazard_variance_retention: VarianceRetention,
    /// Explicit calibration coordinates. Empty until calibration succeeds.
    calibration_times: Vec<f64>,
}

/// How much of the calibrated hazard lattice sits on the zero floor.
///
/// The hazard factor is additive-normal, so a wide lattice drives low nodes
/// negative. A negative hazard is not a credit state, so those nodes are
/// floored at zero — consistently in calibration and in pricing. Where the
/// floor binds, the lattice cannot spread as far downward as the configured
/// volatility asks, so the **realized** hazard volatility is lower than the
/// configured `hazard_vol`, and the calibrated theta compensates on the
/// upside to keep repricing the survival curve exactly.
///
/// The survival curve is still reproduced; what degrades is the dispersion
/// that drives credit optionality. Heavy saturation means the configured
/// `hazard_vol` is too large for the hazard level, which usually indicates a
/// **relative** spread volatility was supplied where an **absolute** hazard
/// volatility belongs (see
/// [`models::credit::market_anchored`](crate::credit::market_anchored)).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HazardFloorSaturation {
    /// Largest share of step-conditional state-price mass sitting at the
    /// zero floor across all calibrated steps, in `[0, 1]`.
    pub max_mass_at_floor: f64,
    /// Step index at which `max_mass_at_floor` occurred.
    pub worst_step: usize,
    /// Number of steps where the target survival probability was
    /// unreachable because the whole row floored. Non-zero means the
    /// lattice could not reprice the survival curve at those steps.
    pub unreachable_steps: usize,
}

impl HazardFloorSaturation {
    /// Whether any node hazard was floored during calibration.
    pub fn is_saturated(&self) -> bool {
        self.max_mass_at_floor > 0.0
    }
}

/// How much of a factor's intended conditional variance survives the
/// mean-reversion probability shift, across the calibrated lattice.
///
/// The additive binomial lattice has one free parameter per node, and
/// `RatesCreditTree::mean_reverting_up_prob` spends it on matching the
/// conditional **mean**. The conditional variance `σ²Δt·4p(1−p)` is therefore
/// understated wherever `p ≠ ½`, by the exact factor documented on
/// [`KAPPA_MAX`]: `retention(d) = 1 − (κ·d·√Δt/σ)²` at displacement `d` from
/// the reversion reference, bottoming out at `1 − (κ·T)²` on the lattice edge.
///
/// Curve repricing stays exact regardless — calibration and pricing apply the
/// identical clamped probability, so the forward and backward recursions remain
/// exact duals. What degrades is the dispersion that drives **option value**,
/// which is the whole point of using a lattice for a callable.
///
/// Where `p` clamps fully to `0` or `1` the node is locally deterministic and
/// its Bernoulli marginal is degenerate, so it can express no correlation at
/// all: `rate_credit_correlation` is silently inoperative there
/// (`RatesCreditTree::node_correlation_range` returns `None` and the
/// feasibility scan skips it). [`Self::clamped_nodes`] is what makes that
/// visible.
///
/// # Interpreting the numbers
///
/// Counts are over lattice **nodes**, not state-price mass. Clamped nodes sit
/// in the wings and carry exponentially little probability, so a small
/// `clamped_nodes` share is not itself alarming — it is a signal to check
/// `min_retention` and, if option value matters at this `(κ, T)`, to move to
/// [`HullWhiteTree`](super::hull_white_tree::HullWhiteTree).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VarianceRetention {
    /// Smallest `4p(1−p)` over every node the pricing induction visits, in
    /// `[0, 1]`. `1.0` means no mean-reversion distortion anywhere.
    pub min_retention: f64,
    /// Step index at which `min_retention` occurs.
    pub worst_step: usize,
    /// Nodes whose marginal clamped to `0` or `1` — zero local variance, and
    /// no expressible correlation.
    pub clamped_nodes: usize,
    /// Nodes scanned (every `(step, node)` the backward induction visits).
    pub total_nodes: usize,
}

impl Default for VarianceRetention {
    /// The undistorted limit: full variance everywhere, nothing scanned.
    fn default() -> Self {
        Self {
            min_retention: 1.0,
            worst_step: 0,
            clamped_nodes: 0,
            total_nodes: 0,
        }
    }
}

impl VarianceRetention {
    /// Whether any node lost its entire conditional variance to the clamp.
    ///
    /// True exactly when the configured `κ·T` reached `1` somewhere on the
    /// lattice. Correlation is inoperative at those nodes.
    pub fn has_clamped_nodes(&self) -> bool {
        self.clamped_nodes > 0
    }

    /// Share of scanned nodes that clamped, in `[0, 1]`.
    pub fn clamped_share(&self) -> f64 {
        if self.total_nodes == 0 {
            return 0.0;
        }
        self.clamped_nodes as f64 / self.total_nodes as f64
    }
}

/// A future floating-coupon reset valued at tree nodes.
///
/// Describes one coupon whose index rate fixes at `reset_step` (in the
/// future) and pays at `payment_step`. The instrument's **deterministic**
/// projection of this coupon stays booked exactly as before in the
/// valuator's step-indexed cashflow vector; the lattice adds only the
/// **node-dependent increment**
///
/// ```text
/// ΔC(i) = notional · accrual · timing_scale
///         · [ compose(base_index_rate + ΔF(i)) − compose(base_index_rate) ]
/// ΔF(i) = node_discount_forward(i) − base_discount_forward
/// ```
///
/// where `compose` is the existing floating-rate composition
/// ([`calculate_floating_rate`]: index floor/cap, gearing, spread, all-in
/// floor/cap) and `node_discount_forward(i)` is the simply-compounded
/// forward implied by the tree-conditional risk-free discount factor at
/// rate node `i` (see [`RatesCreditTree::conditional_discount_factors`]).
///
/// Anchoring the node index rate to the **emission's own**
/// `base_index_rate` implements the deterministic multi-curve basis
///
/// ```text
/// node_projection_forward
///   = node_discount_forward + (market_projection_forward − market_discount_forward)
/// ```
///
/// (additive deterministic basis in the sense of Mercurio (2009),
/// "Interest Rates and The Credit Crunch: New Formulas and Market
/// Models", §2), and makes the increment vanish identically as
/// `rate_vol → 0`, so the stochastic path collapses onto today's
/// deterministic projection by construction.
///
/// # Invariants
///
/// - `reset_step < payment_step ≤ steps`: reset and payment snap to
///   distinct tree slices; a coarser grid must be refined, not silently
///   collapsed.
/// - `accrual > 0`, all fields finite; `timing_scale > 0` carries the
///   `DF(payment_date)/DF(payment_slice)` timing correction the
///   deterministic booking applies via `value_at_step_time`.
#[derive(Debug, Clone)]
pub struct NodeCoupon {
    /// Tree slice at which the coupon's index rate fixes (accrual start).
    pub reset_step: usize,
    /// Tree slice at which the coupon pays (accrual end / payment date).
    pub payment_step: usize,
    /// Accrual fraction τ of the underlying period in the instrument's
    /// day count (the emitted flow's `accrual_factor`).
    pub accrual: f64,
    /// Outstanding principal the coupon accrues on.
    pub notional: f64,
    /// Index rate (pre-composition) the deterministic emission projected
    /// for this period (`CashFlowAccrual::projected_index_rate`).
    pub base_index_rate: f64,
    /// Market simply-compounded discount forward over the **snapped slice
    /// interval** `[t(reset_step), t(payment_step)]` with the same
    /// `accrual` denominator: `(DF(t_n)/DF(t_m) − 1) / accrual`.
    pub base_discount_forward: f64,
    /// Payment-timing correction `DF(payment_date)/DF(t(payment_step))`
    /// mirroring the deterministic booking's `value_at_step_time`.
    pub timing_scale: f64,
    /// Floating-rate composition parameters (index floor/cap, gearing,
    /// spread, all-in floor/cap) taken from the instrument's
    /// `FloatingRateSpec` — no second coupon specification.
    pub params: FloatingRateParams,
}

impl NodeCoupon {
    /// Validate the descriptor against the calibrated lattice geometry.
    fn validate(&self, steps: usize) -> Result<()> {
        if self.reset_step >= self.payment_step {
            return Err(Error::Validation(format!(
                "node coupon reset_step ({}) and payment_step ({}) must snap to \
                 distinct, ordered tree slices; the tree grid is too coarse for \
                 this coupon period. Increase tree_steps.",
                self.reset_step, self.payment_step
            )));
        }
        if self.payment_step > steps {
            return Err(Error::Validation(format!(
                "node coupon payment_step ({}) exceeds the lattice step count ({steps})",
                self.payment_step
            )));
        }
        for (label, value) in [
            ("accrual", self.accrual),
            ("notional", self.notional),
            ("base_index_rate", self.base_index_rate),
            ("base_discount_forward", self.base_discount_forward),
            ("timing_scale", self.timing_scale),
        ] {
            if !value.is_finite() {
                return Err(Error::Validation(format!(
                    "node coupon {label} must be finite, got {value}"
                )));
            }
        }
        if self.accrual <= 0.0 {
            return Err(Error::Validation(format!(
                "node coupon accrual must be positive, got {}",
                self.accrual
            )));
        }
        if self.timing_scale <= 0.0 {
            return Err(Error::Validation(format!(
                "node coupon timing_scale must be positive, got {}",
                self.timing_scale
            )));
        }
        Ok(())
    }
}

/// One recombining additive-normal factor row.
///
/// Every calibrated row is affine in its node index: `base + node * shift`.
/// Retaining that exact representation keeps factor storage linear in the
/// number of time steps while reconstructing the same node levels on demand.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FactorRow {
    base: f64,
    shift: f64,
    nodes: usize,
}

impl FactorRow {
    #[inline]
    fn value(self, node: usize) -> Option<f64> {
        (node < self.nodes).then(|| self.value_unchecked(node))
    }

    #[inline]
    fn value_unchecked(self, node: usize) -> f64 {
        debug_assert!(node < self.nodes);
        self.base + node as f64 * self.shift
    }

    #[inline]
    fn values(self) -> impl ExactSizeIterator<Item = f64> {
        (0..self.nodes).map(move |node| self.value_unchecked(node))
    }
}

/// Per-factor lattice geometry and calibration target shared by the rate and
/// hazard passes of [`RatesCreditTree::calibrate`].
#[derive(Debug, Clone, Copy)]
struct FactorGrid {
    /// Number of time steps.
    steps: usize,
    /// Step size in years.
    dt: f64,
    /// Annualised absolute (normal) volatility of this factor.
    sigma: f64,
    /// Whether node values are passed through the non-negative transform.
    /// Set for the hazard factor; the rate factor allows negative rates.
    floor_at_zero: bool,
}

impl RatesCreditTree {
    /// Create a new rates-credit tree with the given configuration.
    ///
    /// `calibrate()` must be called before `price()`.
    pub fn new(config: RatesCreditConfig) -> Self {
        Self {
            config,
            calibrated_rates: Vec::new(),
            calibrated_hazards: Vec::new(),
            recovery_rate: 0.0,
            rate_ref: 0.0,
            hazard_ref: 0.0,
            hazard_floor_saturation: HazardFloorSaturation::default(),
            rate_variance_retention: VarianceRetention::default(),
            hazard_variance_retention: VarianceRetention::default(),
            calibration_times: Vec::new(),
        }
    }

    /// Hazard floor-saturation diagnostic from the most recent `calibrate()`.
    ///
    /// See [`HazardFloorSaturation`] for how to read it.
    pub fn hazard_floor_saturation(&self) -> HazardFloorSaturation {
        self.hazard_floor_saturation
    }

    /// Rate-factor conditional-variance retention from the most recent
    /// `calibrate()`.
    ///
    /// See [`VarianceRetention`] for how to read it, and [`KAPPA_MAX`] for why
    /// `κ·T` rather than `κ` is the binding quantity. Returns the undistorted
    /// default (`min_retention = 1.0`) for an uncalibrated tree or one without
    /// rate mean reversion.
    pub fn rate_variance_retention(&self) -> VarianceRetention {
        self.rate_variance_retention
    }

    /// Hazard-factor conditional-variance retention from the most recent
    /// `calibrate()`.
    ///
    /// See [`VarianceRetention`]. Returns the undistorted default for an
    /// uncalibrated tree or one without hazard mean reversion.
    pub fn hazard_variance_retention(&self) -> VarianceRetention {
        self.hazard_variance_retention
    }

    /// Scan one calibrated factor's lattice for conditional-variance loss.
    ///
    /// Visits exactly the `(step, node)` pairs the backward induction visits
    /// (`step in 0..steps`, matching [`Self::scan_correlation_feasibility`] and
    /// `price_with_node_coupons`), evaluating the same per-node marginal
    /// probability pricing uses. Without mean reversion every `p` is exactly ½
    /// and retention is uniformly `1.0`, so the scan is skipped.
    fn scan_variance_retention(
        levels: &[FactorRow],
        steps: usize,
        reference: f64,
        kappa: f64,
        sigma: f64,
        dt: f64,
    ) -> VarianceRetention {
        if kappa <= 0.0 || levels.is_empty() {
            return VarianceRetention::default();
        }
        let mut out = VarianceRetention {
            min_retention: 1.0,
            worst_step: 0,
            clamped_nodes: 0,
            total_nodes: 0,
        };
        for (step, row) in levels.iter().enumerate().take(steps) {
            for x in row.values() {
                let p = Self::mean_reverting_up_prob(x, reference, kappa, sigma, dt);
                let retention = 4.0 * p * (1.0 - p);
                out.total_nodes += 1;
                if p <= 0.0 || p >= 1.0 {
                    out.clamped_nodes += 1;
                }
                if retention < out.min_retention {
                    out.min_retention = retention;
                    out.worst_step = step;
                }
            }
        }
        out
    }

    /// Node hazard actually used by calibration and pricing.
    ///
    /// The additive-normal factor can go negative on a wide lattice; a
    /// negative hazard is not a credit state. Both the forward Arrow-Debreu
    /// recursion and the backward induction pass every node hazard through
    /// this same transform, which is what makes them exact duals and lets the
    /// tree reproduce the survival curve as the valuator sees it.
    #[inline]
    fn effective_hazard(raw: f64) -> f64 {
        raw.max(0.0)
    }

    /// Validate explicit conditional calibration targets and return `(dt, T)`.
    fn validate_calibration_targets(
        &self,
        targets: &RatesCreditCalibrationTargets,
    ) -> Result<(f64, f64)> {
        let steps = self.config.steps;
        if steps == 0 {
            return Err(Error::Validation(
                "rates-credit tree calibration requires at least one step".to_string(),
            ));
        }
        let expected_len = steps + 1;
        for (name, len) in [
            ("times", targets.times.len()),
            ("discount_factors", targets.discount_factors.len()),
            (
                "survival_probabilities",
                targets.survival_probabilities.len(),
            ),
        ] {
            if len != expected_len {
                return Err(Error::Validation(format!(
                    "rates-credit calibration {name} must contain steps + 1 = \
                     {expected_len} values, got {len}"
                )));
            }
        }

        let origin = targets.times[0];
        if !origin.is_finite() || origin.abs() > 1e-12 {
            return Err(Error::Validation(format!(
                "rates-credit calibration times must start at 0.0, got {origin}"
            )));
        }
        let dt = targets.times[1] - origin;
        if !dt.is_finite() || dt <= 0.0 {
            return Err(Error::Validation(format!(
                "rates-credit calibration requires a positive finite first time step, got {dt}"
            )));
        }
        let spacing_tolerance = 1e-12_f64.max(dt.abs() * 1e-10);
        for (step, pair) in targets.times.windows(2).enumerate() {
            let actual_dt = pair[1] - pair[0];
            if !pair[1].is_finite()
                || actual_dt <= 0.0
                || (actual_dt - dt).abs() > spacing_tolerance
            {
                return Err(Error::Validation(format!(
                    "rates-credit calibration requires an evenly spaced, strictly increasing \
                     time grid; interval {step} has width {actual_dt}, expected {dt}"
                )));
            }
        }

        for (name, values) in [
            ("discount_factors", targets.discount_factors.as_slice()),
            (
                "survival_probabilities",
                targets.survival_probabilities.as_slice(),
            ),
        ] {
            if (values[0] - 1.0).abs() > 1e-12 {
                return Err(Error::Validation(format!(
                    "conditional {name} must start at 1.0, got {}",
                    values[0]
                )));
            }
            if let Some((index, value)) = values
                .iter()
                .copied()
                .enumerate()
                .find(|(_, value)| !value.is_finite() || *value <= 0.0)
            {
                return Err(Error::Validation(format!(
                    "rates-credit calibration {name}[{index}] must be positive and finite, \
                     got {value}"
                )));
            }
        }
        for (step, pair) in targets.survival_probabilities.windows(2).enumerate() {
            if pair[1] > pair[0] + 1e-12 || pair[1] > 1.0 + 1e-12 {
                return Err(Error::Validation(format!(
                    "conditional survival probabilities must be non-increasing and at most \
                     1.0; interval {step} moves from {} to {}",
                    pair[0], pair[1]
                )));
            }
        }
        if !targets.recovery_rate.is_finite() || !(0.0..=1.0).contains(&targets.recovery_rate) {
            return Err(Error::Validation(format!(
                "rates-credit recovery_rate must be finite and in [0, 1], got {}",
                targets.recovery_rate
            )));
        }

        for (name, value) in [
            ("rate_vol", self.config.rate_vol),
            ("hazard_vol", self.config.hazard_vol),
            ("rate_mean_reversion", self.config.rate_mean_reversion),
            ("hazard_mean_reversion", self.config.hazard_mean_reversion),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(Error::Validation(format!(
                    "rates-credit {name} must be non-negative and finite, got {value}"
                )));
            }
        }
        if !self.config.correlation.is_finite() || !(-1.0..=1.0).contains(&self.config.correlation)
        {
            return Err(Error::Validation(format!(
                "rates-credit correlation must be finite and in [-1, 1], got {}",
                self.config.correlation
            )));
        }

        for (name, kappa) in [
            ("rate_mean_reversion", self.config.rate_mean_reversion),
            ("hazard_mean_reversion", self.config.hazard_mean_reversion),
        ] {
            if kappa > KAPPA_MAX {
                return Err(Error::Validation(format!(
                    "{name} = {kappa:.4} exceeds the binomial-lattice limit \
                     (KAPPA_MAX = {KAPPA_MAX}). At this speed the conditional variance of \
                     the factor collapses to a fraction of its intended value, which \
                     degrades option-value accuracy for callable bonds and term loans. \
                     Use HullWhiteTree for mean reversion above this threshold."
                )));
            }
        }

        Ok((dt, targets.times[steps]))
    }

    /// Calibrate both factors to explicit conditional targets using
    /// Arrow-Debreu forward induction.
    ///
    /// The rate factor matches `discount_factors`, and the hazard factor
    /// matches `survival_probabilities`. Calibration commits atomically: when
    /// validation or fitting fails, any earlier successful calibration on this
    /// instance remains unchanged.
    ///
    /// # Arguments
    ///
    /// * `targets` - evenly spaced coordinates and conditional discount,
    ///   survival, and recovery inputs measured from the valuation origin
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when the grid, targets, recovery, or model
    /// configuration is invalid, or when the requested correlation is not
    /// attainable on the calibrated lattice.
    pub fn calibrate(&mut self, targets: &RatesCreditCalibrationTargets) -> Result<()> {
        let (dt, _) = self.validate_calibration_targets(targets)?;
        let steps = self.config.steps;

        // Build a complete candidate and commit only after every validation
        // passes. A failed recalibration must not leave mixed old/new factors.
        let mut candidate = Self::new(self.config.clone());
        candidate.recovery_rate = targets.recovery_rate;
        candidate.calibration_times = targets.times.clone();

        candidate.rate_ref = Self::initial_instantaneous(targets.discount_factors[1], dt);
        candidate.hazard_ref = Self::initial_instantaneous(targets.survival_probabilities[1], dt);

        let rate_vol = candidate.config.rate_vol;
        let rate_kappa = candidate.config.rate_mean_reversion;
        let rate_ref = candidate.rate_ref;
        let (calibrated_rates, _) = candidate.calibrate_factor_ho_lee(
            FactorGrid {
                steps,
                dt,
                sigma: rate_vol,
                floor_at_zero: false,
            },
            &targets.discount_factors,
            |r| Self::mean_reverting_up_prob(r, rate_ref, rate_kappa, rate_vol, dt),
        )?;
        candidate.calibrated_rates = calibrated_rates;

        let hazard_vol = candidate.config.hazard_vol;
        let hazard_kappa = candidate.config.hazard_mean_reversion;
        let hazard_ref = candidate.hazard_ref;
        let (calibrated_hazards, saturation) = candidate.calibrate_factor_ho_lee(
            FactorGrid {
                steps,
                dt,
                sigma: hazard_vol,
                floor_at_zero: true,
            },
            &targets.survival_probabilities,
            |h| Self::mean_reverting_up_prob(h, hazard_ref, hazard_kappa, hazard_vol, dt),
        )?;
        candidate.calibrated_hazards = calibrated_hazards;
        candidate.hazard_floor_saturation = saturation;

        candidate.rate_variance_retention = Self::scan_variance_retention(
            &candidate.calibrated_rates,
            steps,
            candidate.rate_ref,
            rate_kappa,
            rate_vol,
            dt,
        );
        candidate.hazard_variance_retention = Self::scan_variance_retention(
            &candidate.calibrated_hazards,
            steps,
            candidate.hazard_ref,
            hazard_kappa,
            hazard_vol,
            dt,
        );
        candidate.validate_correlation_feasibility(dt)?;

        *self = candidate;
        Ok(())
    }

    /// Verify the configured correlation is attainable at every node pair.
    ///
    /// Two Bernoulli marginals admit a correlation only inside their Fréchet
    /// bounds. With both mean reversions zero every marginal is exactly ½ and
    /// the whole range `[-1, 1]` is attainable, so the scan is skipped. Mean
    /// reversion skews the corner nodes' marginals away from ½ and shrinks the
    /// attainable interval sharply: with both speeds at [`KAPPA_MAX`] over a
    /// five-year lattice the feasible `|ρ|` measures about `0.12`
    /// (σ_r = 0.012, σ_λ = 0.05, 40 steps). The exact bound depends on the
    /// volatilities and step count as well — the skew scales like
    /// `κ·(x − x_ref)·√Δt / (2σ)` — so callers should read it from
    /// [`Self::max_feasible_correlation`] rather than assume a fixed number.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] naming the offending step and node pair,
    /// their marginal probabilities, and the largest `|ρ|` feasible across the
    /// whole lattice.
    fn validate_correlation_feasibility(&self, dt: f64) -> Result<()> {
        let rho = self.config.correlation;
        if rho == 0.0 {
            return Ok(());
        }
        // Without mean reversion every marginal is ½ and any |ρ| ≤ 1 is
        // attainable, so there is nothing to scan.
        if self.config.rate_mean_reversion == 0.0 && self.config.hazard_mean_reversion == 0.0 {
            return Ok(());
        }

        let (lattice_lo, lattice_hi, worst) = self.scan_correlation_feasibility(dt, Some(rho));
        let Some((step, i, j, p_r, p_h, node_lo, node_hi)) = worst else {
            return Ok(());
        };
        let max_feasible_abs = lattice_lo.abs().min(lattice_hi.abs()).max(0.0);
        Err(Error::Validation(format!(
            "rate_credit_correlation = {rho:.4} is not attainable on this lattice. \
             At step {step}, rate node {i} / hazard node {j}, the marginal \
             up-probabilities are p_r = {p_r:.4} and p_h = {p_h:.4}, whose Fréchet \
             bounds admit only ρ ∈ [{node_lo:.4}, {node_hi:.4}]. Across the whole \
             lattice the attainable interval is [{lattice_lo:.4}, {lattice_hi:.4}], \
             so the largest usable |ρ| is {max_feasible_abs:.4}. Mean reversion is \
             what narrows this: it skews the corner nodes' marginals away from ½. \
             Reduce |ρ|, or reduce the mean-reversion speeds \
             (rate κ = {kappa_r}, hazard κ = {kappa_h}).",
            kappa_r = self.config.rate_mean_reversion,
            kappa_h = self.config.hazard_mean_reversion,
        )))
    }

    /// Non-degenerate minimum and maximum marginal up-probabilities in a row.
    ///
    /// Degenerate marginals carry no correlation and therefore impose no
    /// Fréchet constraint. The returned indices preserve a useful diagnostic
    /// node when an extrema pair rejects the requested correlation.
    fn marginal_probability_extrema(
        row: FactorRow,
        reference: f64,
        kappa: f64,
        sigma: f64,
        dt: f64,
    ) -> Option<((usize, f64), (usize, f64))> {
        let mut minimum: Option<(usize, f64)> = None;
        let mut maximum: Option<(usize, f64)> = None;
        for (index, level) in row.values().enumerate() {
            let probability = Self::mean_reverting_up_prob(level, reference, kappa, sigma, dt);
            if probability <= 0.0 || probability >= 1.0 {
                continue;
            }
            if minimum.is_none_or(|(_, current)| probability < current) {
                minimum = Some((index, probability));
            }
            if maximum.is_none_or(|(_, current)| probability > current) {
                maximum = Some((index, probability));
            }
        }
        minimum.zip(maximum)
    }

    /// Lattice-wide Fréchet-admissible correlation interval.
    ///
    /// For Bernoulli marginals, the upper correlation bound decreases with
    /// the absolute distance between their logits, so a cross-extrema pair is
    /// binding. The lower bound is least negative at either the joint minima
    /// or the joint maxima. Checking all four combinations of the two
    /// marginal extrema is therefore exactly equivalent to the Cartesian
    /// node-pair scan, while reducing a `steps`-row scan from cubic to
    /// quadratic work.
    ///
    /// Returns the intersection over those binding pairs together with one
    /// pair (if any) at which `probe` falls outside the admissible interval.
    #[allow(clippy::type_complexity)]
    fn scan_correlation_feasibility(
        &self,
        dt: f64,
        probe: Option<f64>,
    ) -> (f64, f64, Option<(usize, usize, usize, f64, f64, f64, f64)>) {
        let mut lattice_lo = -1.0_f64;
        let mut lattice_hi = 1.0_f64;
        let mut worst = None;

        for step in 0..self.config.steps {
            let Some((rate_min, rate_max)) = Self::marginal_probability_extrema(
                self.calibrated_rates[step],
                self.rate_ref,
                self.config.rate_mean_reversion,
                self.config.rate_vol,
                dt,
            ) else {
                continue;
            };
            let Some((hazard_min, hazard_max)) = Self::marginal_probability_extrema(
                self.calibrated_hazards[step],
                self.hazard_ref,
                self.config.hazard_mean_reversion,
                self.config.hazard_vol,
                dt,
            ) else {
                continue;
            };

            for (i, p_r) in [rate_min, rate_max] {
                for (j, p_h) in [hazard_min, hazard_max] {
                    let Some((node_lo, node_hi)) = Self::node_correlation_range(p_r, p_h) else {
                        continue;
                    };
                    lattice_lo = lattice_lo.max(node_lo);
                    lattice_hi = lattice_hi.min(node_hi);
                    if probe.is_some_and(|rho| rho < node_lo || rho > node_hi) {
                        worst.get_or_insert((step, i, j, p_r, p_h, node_lo, node_hi));
                    }
                }
            }
        }
        (lattice_lo, lattice_hi, worst)
    }

    /// Largest `|ρ|` this calibrated lattice can express.
    ///
    /// The Fréchet bounds of two Bernoulli marginals limit how much
    /// correlation a node can carry, and mean reversion skews the corner
    /// nodes' marginals away from ½. This reports the binding constraint
    /// across the whole lattice, so a caller can pick a workable correlation
    /// instead of discovering the limit through a calibration failure.
    ///
    /// Returns `1.0` for an uncalibrated tree or one without mean reversion,
    /// where every correlation in `[-1, 1]` is attainable.
    pub fn max_feasible_correlation(&self) -> f64 {
        if self.calibrated_rates.is_empty()
            || self.config.steps == 0
            || (self.config.rate_mean_reversion == 0.0 && self.config.hazard_mean_reversion == 0.0)
        {
            return 1.0;
        }
        let Ok(dt) = self.calibrated_dt() else {
            return 1.0;
        };
        let (lo, hi, _) = self.scan_correlation_feasibility(dt, None);
        lo.abs().min(hi.abs()).max(0.0)
    }

    /// Fréchet-admissible correlation interval for two Bernoulli marginals.
    ///
    /// With `±1` coding the joint law is pinned by the single joint-up
    /// probability `a = P(up, up)`, which must satisfy
    /// `max(0, p_r + p_h − 1) ≤ a ≤ min(p_r, p_h)`, and the realized
    /// correlation is `ρ = (a − p_r·p_h) / √(var_r·var_h)`. Mapping the bounds
    /// on `a` through that relation gives the attainable `ρ` interval.
    ///
    /// Returns `None` when either marginal is degenerate (`p = 0` or `1`):
    /// that factor does not move at the node, so no correlation is
    /// expressible and the joint is the independent product.
    #[inline]
    fn node_correlation_range(p_r: f64, p_h: f64) -> Option<(f64, f64)> {
        let var_r = p_r * (1.0 - p_r);
        let var_h = p_h * (1.0 - p_h);
        let denom = (var_r * var_h).sqrt();
        if denom.is_nan() || denom <= 0.0 {
            return None;
        }
        let a_lo = (p_r + p_h - 1.0).max(0.0);
        let a_hi = p_r.min(p_h);
        Some(((a_lo - p_r * p_h) / denom, (a_hi - p_r * p_h) / denom))
    }

    /// Return the recovery rate from the most recent `calibrate()` call.
    pub fn recovery_rate(&self) -> f64 {
        self.recovery_rate
    }

    /// Calibrated short rate at node `(step, node_i)`.
    ///
    /// Returns an error if the tree has not been calibrated or the indices are
    /// out of bounds. Node `0` is the lowest rate, node `step` the highest
    /// (additive Ho-Lee lattice).
    pub fn rate_at_node(&self, step: usize, node: usize) -> Result<f64> {
        self.calibrated_rates
            .get(step)
            .and_then(|row| row.value(node))
            .ok_or_else(|| {
                Error::internal(format!(
                    "rates-credit tree rate node out of bounds: step={step}, node={node}"
                ))
            })
    }

    /// Calibrated hazard rate at node `(step, node_j)`.
    ///
    /// Returns an error if the tree has not been calibrated or the indices are
    /// out of bounds.
    pub fn hazard_at_node(&self, step: usize, node: usize) -> Result<f64> {
        self.calibrated_hazards
            .get(step)
            .and_then(|row| row.value(node))
            .ok_or_else(|| {
                Error::internal(format!(
                    "rates-credit tree hazard node out of bounds: step={step}, node={node}"
                ))
            })
    }

    /// Explicit time grid used by the most recent successful calibration.
    ///
    /// # Errors
    ///
    /// Returns an error when the tree has not been calibrated.
    pub fn time_grid(&self) -> Result<&[f64]> {
        if self.calibration_times.len() != self.config.steps + 1 {
            return Err(Error::internal(
                "rates-credit tree must be calibrated before reading its time grid",
            ));
        }
        Ok(&self.calibration_times)
    }

    /// Return the calibrated uniform step size.
    fn calibrated_dt(&self) -> Result<f64> {
        let times = self.time_grid()?;
        Ok(times[1] - times[0])
    }

    /// Validate that a legacy tree-pricing horizon matches calibration.
    fn validate_pricing_horizon(&self, time_to_maturity: f64) -> Result<f64> {
        let times = self.time_grid()?;
        let calibrated_horizon = times[self.config.steps];
        let tolerance = 1e-12_f64.max(calibrated_horizon.abs() * 1e-10);
        if !time_to_maturity.is_finite()
            || time_to_maturity <= 0.0
            || (time_to_maturity - calibrated_horizon).abs() > tolerance
        {
            return Err(Error::Validation(format!(
                "rates-credit pricing horizon {time_to_maturity} does not match the \
                 calibrated horizon {calibrated_horizon}"
            )));
        }
        self.calibrated_dt()
    }

    /// Joint movement probabilities from a calibrated node.
    ///
    /// # Arguments
    ///
    /// * `step` - interval start step in `0..config.steps`
    /// * `rate_node` - rate-factor node index in `0..=step`
    /// * `hazard_node` - hazard-factor node index in `0..=step`
    ///
    /// # Errors
    ///
    /// Returns an error when the tree is uncalibrated or any index is outside
    /// the calibrated lattice.
    pub fn transition_probabilities(
        &self,
        step: usize,
        rate_node: usize,
        hazard_node: usize,
    ) -> Result<RatesCreditTransition> {
        if step >= self.config.steps || rate_node > step || hazard_node > step {
            return Err(Error::Validation(format!(
                "rates-credit transition requires step < {} and node indices <= step; \
                 got step={step}, rate_node={rate_node}, hazard_node={hazard_node}",
                self.config.steps
            )));
        }
        let dt = self.calibrated_dt()?;
        let rate = self.rate_at_node(step, rate_node)?;
        let hazard = self.hazard_at_node(step, hazard_node)?;
        let p_rate_up = Self::mean_reverting_up_prob(
            rate,
            self.rate_ref,
            self.config.rate_mean_reversion,
            self.config.rate_vol,
            dt,
        );
        let p_hazard_up = Self::mean_reverting_up_prob(
            hazard,
            self.hazard_ref,
            self.config.hazard_mean_reversion,
            self.config.hazard_vol,
            dt,
        );
        let (up_up, up_down, down_up, down_down) = self.joint_probabilities(p_rate_up, p_hazard_up);
        Ok(RatesCreditTransition {
            up_up,
            up_down,
            down_up,
            down_down,
        })
    }

    /// Sample one deterministic joint factor path with a dedicated Philox
    /// substream.
    ///
    /// # Arguments
    ///
    /// * `seed` - root Philox seed shared by a reproducible simulation run
    /// * `path_index` - unique Philox substream identifier for this logical path
    /// * `antithetic` - when true, use `1 - u` for every uniform from the same
    ///   `(seed, path_index)` stream
    ///
    /// # Errors
    ///
    /// Returns an error when the tree is uncalibrated.
    pub fn sample_path(
        &self,
        seed: u64,
        path_index: u64,
        antithetic: bool,
    ) -> Result<Vec<RatesCreditPathState>> {
        let mut path = Vec::with_capacity(self.config.steps + 1);
        self.sample_path_into(seed, path_index, antithetic, &mut path)?;
        Ok(path)
    }

    /// Sample one deterministic joint factor path into caller-owned storage.
    ///
    /// Reusing `output` avoids one allocation per path in streamed or blocked
    /// simulations. Existing contents are cleared before the new path is
    /// written.
    ///
    /// # Arguments
    ///
    /// * `seed` - root Philox seed shared by a reproducible simulation run
    /// * `path_index` - unique Philox substream identifier for this logical path
    /// * `antithetic` - when true, mirror every uniform from the same Philox
    ///   substream
    /// * `output` - reusable destination receiving `config.steps + 1` states
    ///
    /// # Errors
    ///
    /// Returns an error when the tree is uncalibrated.
    pub fn sample_path_into(
        &self,
        seed: u64,
        path_index: u64,
        antithetic: bool,
        output: &mut Vec<RatesCreditPathState>,
    ) -> Result<()> {
        let dt = self.calibrated_dt()?;
        let times = self.time_grid()?;
        let mut rng = PhiloxRng::with_stream(seed, path_index);
        let mut uniform = [0.0_f64; 1];
        let mut rate_node = 0_usize;
        let mut hazard_node = 0_usize;

        output.clear();
        output.reserve(self.config.steps + 1);
        for (step, &time) in times.iter().enumerate() {
            let short_rate = self.rate_at_node(step, rate_node)?;
            let hazard_rate = Self::effective_hazard(self.hazard_at_node(step, hazard_node)?);
            let (discount_to_next, survival_to_next, default_to_next) = if step < self.config.steps
            {
                let discount = (-short_rate * dt).exp();
                let hazard_exponent = -hazard_rate * dt;
                let survival = hazard_exponent.exp();
                let default = -hazard_exponent.exp_m1();
                (discount, survival, default)
            } else {
                (1.0, 1.0, 0.0)
            };
            output.push(RatesCreditPathState {
                step,
                time,
                rate_node,
                hazard_node,
                short_rate,
                hazard_rate,
                discount_to_next,
                survival_to_next,
                default_to_next,
            });

            if step == self.config.steps {
                break;
            }
            let probabilities = self.transition_probabilities(step, rate_node, hazard_node)?;
            rng.fill_u01(&mut uniform);
            let draw = if antithetic {
                1.0 - uniform[0]
            } else {
                uniform[0]
            };
            let (rate_up, hazard_up) = Self::sampled_moves(probabilities, draw);
            rate_node += usize::from(rate_up);
            hazard_node += usize::from(hazard_up);
        }
        Ok(())
    }

    /// Resume a deterministic sampled factor path over one inclusive step range.
    ///
    /// The returned states are bit-for-bit identical to
    /// `sample_path(seed, path_index, antithetic)?[checkpoint.step..=end_step]`.
    /// This supports bounded-memory consumers that checkpoint product state at
    /// time-block boundaries and must not replay the full factor-path prefix.
    ///
    /// # Arguments
    ///
    /// * `seed` - Root Philox seed used for the original path.
    /// * `path_index` - Philox substream identifier used for the original path.
    /// * `antithetic` - Whether each uniform draw is mirrored as `1 - u`.
    /// * `checkpoint` - Factor node indices and first lattice step of the segment.
    /// * `end_step` - Last lattice step to return, inclusive.
    /// * `output` - Reusable destination cleared before the segment is written.
    ///
    /// # Errors
    ///
    /// Returns an error if the tree is uncalibrated, the checkpoint is outside
    /// the calibrated lattice, or `end_step` precedes the checkpoint or exceeds
    /// the calibrated horizon.
    pub fn sample_path_segment_into(
        &self,
        seed: u64,
        path_index: u64,
        antithetic: bool,
        checkpoint: RatesCreditPathCheckpoint,
        end_step: usize,
        output: &mut Vec<RatesCreditPathState>,
    ) -> Result<()> {
        if checkpoint.step > end_step
            || end_step > self.config.steps
            || checkpoint.rate_node > checkpoint.step
            || checkpoint.hazard_node > checkpoint.step
        {
            return Err(Error::Validation(format!(
                "rates-credit path segment requires checkpoint nodes <= step and {} <= end_step <= {}; got checkpoint=({}, {}, {}), end_step={end_step}",
                checkpoint.step,
                self.config.steps,
                checkpoint.step,
                checkpoint.rate_node,
                checkpoint.hazard_node
            )));
        }
        let dt = self.calibrated_dt()?;
        let times = self.time_grid()?;
        let draw_offset = u64::try_from(checkpoint.step).map_err(|_| {
            Error::Validation("rates-credit path checkpoint step exceeds Philox range".to_string())
        })?;
        let mut rng = PhiloxRng::with_stream_u01_offset(seed, path_index, draw_offset);
        let mut uniform = [0.0_f64; 1];
        let mut rate_node = checkpoint.rate_node;
        let mut hazard_node = checkpoint.hazard_node;

        output.clear();
        output.reserve(end_step - checkpoint.step + 1);
        for (step, &time) in times
            .iter()
            .enumerate()
            .take(end_step + 1)
            .skip(checkpoint.step)
        {
            let short_rate = self.rate_at_node(step, rate_node)?;
            let hazard_rate = Self::effective_hazard(self.hazard_at_node(step, hazard_node)?);
            let (discount_to_next, survival_to_next, default_to_next) = if step < self.config.steps
            {
                let discount = (-short_rate * dt).exp();
                let hazard_exponent = -hazard_rate * dt;
                (discount, hazard_exponent.exp(), -hazard_exponent.exp_m1())
            } else {
                (1.0, 1.0, 0.0)
            };
            output.push(RatesCreditPathState {
                step,
                time,
                rate_node,
                hazard_node,
                short_rate,
                hazard_rate,
                discount_to_next,
                survival_to_next,
                default_to_next,
            });

            if step == end_step {
                break;
            }
            let probabilities = self.transition_probabilities(step, rate_node, hazard_node)?;
            rng.fill_u01(&mut uniform);
            let draw = if antithetic {
                1.0 - uniform[0]
            } else {
                uniform[0]
            };
            let (rate_up, hazard_up) = Self::sampled_moves(probabilities, draw);
            rate_node += usize::from(rate_up);
            hazard_node += usize::from(hazard_up);
        }
        Ok(())
    }

    #[inline]
    fn sampled_moves(probabilities: RatesCreditTransition, draw: f64) -> (bool, bool) {
        let up_up_cutoff = probabilities.up_up;
        let up_down_cutoff = up_up_cutoff + probabilities.up_down;
        let down_up_cutoff = up_down_cutoff + probabilities.down_up;
        if draw < up_up_cutoff {
            (true, true)
        } else if draw < up_down_cutoff {
            (true, false)
        } else if draw < down_up_cutoff {
            (false, true)
        } else {
            (false, false)
        }
    }

    /// Initial instantaneous factor implied by the first target interval:
    /// `−ln(target(Δt))/Δt`.
    fn initial_instantaneous(target_dt: f64, dt: f64) -> f64 {
        -target_dt.ln() / dt
    }

    /// Moment-matched up-probability for a mean-reverting factor on the additive
    /// normal lattice.
    ///
    /// # Units
    ///
    /// Every quantity below is in **absolute rate units** (rate per year), so
    /// the formula is dimensionally coherent:
    /// - `x`, `x_ref`: rate level (per year)
    /// - drift `μ = −κ·(x − x_ref)`: rate per year
    /// - lattice up/down step `σ√Δt`: rate (per year, integrated over `√Δt`)
    ///
    /// The standard moment-matched binomial up-probability for a drift `μ` on a
    /// lattice with step `±σ√Δt` is
    ///
    /// ```text
    /// p_up = ½ + (μ·Δt) / (2·σ√Δt) = ½ + μ·√Δt / (2σ)
    /// ```
    ///
    /// which matches the conditional **mean** `E[Δx] = μ·Δt`. With `κ = 0`
    /// this reduces to `p_up = ½` (plain Ho-Lee).
    ///
    /// # Mean vs variance trade-off
    ///
    /// An additive binomial lattice has a single free parameter (`p`). This
    /// function uses it to match the conditional mean, leaving the conditional
    /// variance `σ²Δt · 4p(1−p)` understated once `p ≠ ½`. Discount-curve
    /// repricing is **exact for any κ** because calibration and pricing apply
    /// the **identical** clamped probability, so the forward Arrow-Debreu
    /// recursion and backward induction remain exact duals. However,
    /// **option-value accuracy degrades as κ grows** — a limitation that
    /// matters when pricing callable bonds and term loans. For κ beyond
    /// [`KAPPA_MAX`] the degradation becomes material; use [`HullWhiteTree`]
    /// instead.
    ///
    /// [`HullWhiteTree`]: super::hull_white_tree::HullWhiteTree
    #[inline]
    fn mean_reverting_up_prob(x: f64, x_ref: f64, kappa: f64, sigma: f64, dt: f64) -> f64 {
        if kappa <= 0.0 || dt <= 0.0 {
            return 0.5;
        }
        let sigma = sigma.max(1e-12);
        // Drift in absolute rate units (rate / year).
        let drift = -kappa * (x - x_ref);
        (0.5 + drift * dt.sqrt() / (2.0 * sigma)).clamp(0.0, 1.0)
    }

    /// Ho-Lee style calibration for a single factor.
    ///
    /// Builds a 1D binomial lattice with additive normal volatility (`sigma * sqrt(dt)`)
    /// and solves for a theta (drift) at each step so that the lattice-implied
    /// "discount factor" matches a target curve.
    ///
    /// - For the rate factor: conditional discount factors on the grid
    /// - For the hazard factor: conditional survival probabilities on the grid
    ///
    /// Both share the same mathematical structure: the product `exp(-x * dt)` over
    /// path nodes must match the target curve value at each maturity.
    ///
    /// `up_prob_fn` returns the up-transition probability for a node given its
    /// (post-theta) rate. It **must** be the identical function the pricing pass
    /// uses for backward induction: the forward Arrow-Debreu recursion below and
    /// the backward induction in `price()` are exact duals only when they share
    /// the same per-node probability, which is what makes the calibrated tree
    /// reprice the input curve when mean reversion is active.
    ///
    /// # Non-negative factors
    ///
    /// When `floor_at_zero` is set (the hazard factor), every node value is
    /// passed through [`Self::effective_hazard`] in **both** the state-price
    /// recursion and the theta solve, matching what `price()` applies. The
    /// floor makes the step equation non-linear in theta — `exp(-θΔt)` no
    /// longer factors out — so theta is bracketed and bisected instead of
    /// being read off in closed form. The rate factor is left unfloored:
    /// negative short rates are a legitimate market state, and the discounting
    /// in `price()` deliberately uses the raw calibrated rate.
    fn calibrate_factor_ho_lee(
        &self,
        grid: FactorGrid,
        targets: &[f64],
        up_prob_fn: impl Fn(f64) -> f64,
    ) -> Result<(Vec<FactorRow>, HazardFloorSaturation)> {
        let FactorGrid {
            steps,
            dt,
            sigma,
            floor_at_zero,
        } = grid;
        let mut rates = Vec::with_capacity(steps + 1);
        let sqrt_dt = dt.sqrt();
        let one_move = sigma * sqrt_dt;
        let node_shift = 2.0 * one_move;

        // Initial rate: r0 = -ln(target(dt)) / dt.
        let r0 = Self::initial_instantaneous(targets[1], dt);
        let initial = if floor_at_zero {
            Self::effective_hazard(r0)
        } else {
            r0
        };
        rates.push(FactorRow {
            base: initial,
            shift: node_shift,
            nodes: 1,
        });

        // Arrow-Debreu state prices
        let mut state_prices = vec![1.0];
        let mut saturation = HazardFloorSaturation::default();

        // Value actually used for discounting/survival at a node.
        let effective = |x: f64| -> f64 {
            if floor_at_zero {
                Self::effective_hazard(x)
            } else {
                x
            }
        };

        for step in 0..steps {
            let next_nodes = step + 2;
            let mut next_state_prices = vec![0.0; next_nodes];
            let current_row = rates[step];
            let next_rates_base = FactorRow {
                base: current_row.value_unchecked(0) - one_move,
                shift: node_shift,
                nodes: next_nodes,
            };

            // Propagate state prices and compute base rates (without theta).
            //
            // The transition probability uses the SAME mean-reversion-aware
            // function the pricing pass applies, evaluated on the (calibrated)
            // current-row rate. Forward induction here is the exact dual of the
            // backward induction in `price()` because both use this probability.
            for (i, current_rate) in current_row.values().enumerate() {
                let q = state_prices[i];
                let df_i = (-effective(current_rate) * dt).exp();
                let p_up = up_prob_fn(current_rate);

                // Up move (to node i+1)
                if i + 1 < next_nodes {
                    next_state_prices[i + 1] += q * df_i * p_up;
                }

                // Down move (to node i)
                if i < next_nodes {
                    next_state_prices[i] += q * df_i * (1.0 - p_up);
                }
            }

            // Solve for theta so the lattice reproduces the target curve at
            // the next maturity.
            let next_target_index = step + 2;
            let theta = if next_target_index <= steps {
                let p_target = targets[next_target_index];
                if !floor_at_zero {
                    // Unfloored: exp(-θΔt) factors out, so theta is exact.
                    let mut p_model_base = 0.0;
                    for (j, &q_next) in next_state_prices.iter().enumerate() {
                        p_model_base += q_next * (-next_rates_base.value_unchecked(j) * dt).exp();
                    }
                    if p_model_base > 0.0 && p_target > 0.0 {
                        -(p_target / p_model_base).ln() / dt
                    } else {
                        0.0
                    }
                } else {
                    Self::solve_floored_theta(
                        &next_state_prices,
                        next_rates_base,
                        dt,
                        p_target,
                        step + 1,
                        &mut saturation,
                    )
                }
            } else {
                0.0
            };

            // Apply the calibrated row shift without materializing its nodes.
            let next_rates = FactorRow {
                base: next_rates_base.base + theta,
                ..next_rates_base
            };
            if floor_at_zero {
                Self::record_floor_saturation(
                    next_rates,
                    &next_state_prices,
                    step + 1,
                    &mut saturation,
                );
            }
            rates.push(next_rates);
            state_prices = next_state_prices;
        }

        Ok((rates, saturation))
    }

    /// Solve the per-step hazard shift `θ` against the target survival
    /// probability under the non-negative transform.
    ///
    /// Finds `θ` with
    ///
    /// ```text
    /// Σ_j Q[j] · exp(−max(0, base_j + θ)·Δt) = target
    /// ```
    ///
    /// The left side is continuous and non-increasing in `θ`: it equals the
    /// total state-price mass `Σ_j Q[j]` once `θ` is low enough to floor the
    /// whole row, and tends to `0` as `θ` grows. A root therefore exists
    /// exactly when `0 < target < Σ_j Q[j]`. When the target exceeds the
    /// floored ceiling the survival curve is unreachable at this step: the
    /// shift is driven to the all-floored limit and the step is counted in
    /// `saturation.unreachable_steps` rather than failing, so a caller can
    /// still price while seeing that the configured hazard volatility is too
    /// large for the hazard level.
    fn solve_floored_theta(
        state_prices: &[f64],
        base: FactorRow,
        dt: f64,
        target: f64,
        step: usize,
        saturation: &mut HazardFloorSaturation,
    ) -> f64 {
        let survival_at = |theta: f64| -> f64 {
            state_prices
                .iter()
                .zip(base.values())
                .map(|(q, b)| q * (-Self::effective_hazard(b + theta) * dt).exp())
                .sum::<f64>()
        };

        let total_mass: f64 = state_prices.iter().sum();
        if target.is_nan() || target <= 0.0 || total_mass <= 0.0 {
            return 0.0;
        }
        // All-floored limit: θ low enough that *every* node hits zero hazard,
        // which is the highest survival the row can produce. That requires
        // θ ≤ −max_j(base_j) — the largest base node is the last one to floor.
        let max_base = base.values().fold(f64::NEG_INFINITY, f64::max);
        let theta_all_floored = -max_base;
        if target >= total_mass {
            // Unreachable: even zero hazard everywhere survives less than the
            // target asks (the target itself already exceeds the mass carried
            // into this step).
            saturation.unreachable_steps += 1;
            saturation.worst_step = step;
            saturation.max_mass_at_floor = 1.0_f64.max(saturation.max_mass_at_floor);
            return theta_all_floored;
        }

        // Expand upward from the all-floored point until survival drops below
        // the target, then solve in that bracket. Survival is monotone
        // non-increasing in θ, so the bracket holds exactly one root.
        let lo = theta_all_floored;
        let mut hi = theta_all_floored.max(0.0) + 1.0;
        let mut guard = 0;
        while survival_at(hi) > target && guard < 200 {
            hi = theta_all_floored + (hi - theta_all_floored) * 2.0;
            guard += 1;
        }
        BrentSolver::new()
            .solve_in_bracket(|theta| survival_at(theta) - target, lo, hi)
            .unwrap_or(0.5 * (lo + hi))
    }

    /// Record how much state-price mass sits on the zero hazard floor at a
    /// calibrated step.
    fn record_floor_saturation(
        rates: FactorRow,
        state_prices: &[f64],
        step: usize,
        saturation: &mut HazardFloorSaturation,
    ) {
        let total: f64 = state_prices.iter().sum();
        if total <= 0.0 {
            return;
        }
        let floored: f64 = rates
            .values()
            .zip(state_prices.iter())
            .filter(|(rate, _)| *rate <= 0.0)
            .map(|(_, q)| *q)
            .sum();
        let share = (floored / total).clamp(0.0, 1.0);
        if share > saturation.max_mass_at_floor {
            saturation.max_mass_at_floor = share;
            saturation.worst_step = step;
        }
    }

    /// Couple the two marginal up-probabilities into joint cell probabilities.
    ///
    /// The joint law of two Bernoullis has exactly one degree of freedom once
    /// the marginals are fixed, so it is built from the single joint-up
    /// probability
    ///
    /// ```text
    /// a = P(up, up) = p_r·p_h + ρ·√(var_r·var_h)
    /// ```
    ///
    /// and the remaining three cells follow by **subtraction**:
    ///
    /// ```text
    /// p_ud = p_r − a,  p_du = p_h − a,  p_dd = 1 − p_r − p_h + a
    /// ```
    ///
    /// Deriving them this way makes both marginals exact by construction —
    /// `p_uu + p_ud = p_r` and `p_uu + p_du = p_h` hold identically — which is
    /// what keeps the forward Arrow-Debreu recursion and the backward
    /// induction exact duals, so the calibrated tree still reprices both input
    /// curves. The previous formulation clamped all four cells independently
    /// and renormalised, which silently perturbed the marginals (and hence the
    /// curve repricing) whenever a cell hit the clamp.
    ///
    /// `a` is confined to the Fréchet interval
    /// `[max(0, p_r + p_h − 1), min(p_r, p_h)]` purely as a numerical guard:
    /// [`Self::validate_correlation_feasibility`] has already proved at
    /// calibration time that the configured `ρ` is attainable at every node,
    /// so the clamp is unreachable in a calibrated tree and is asserted as
    /// such in debug builds.
    #[inline]
    fn joint_probabilities(&self, p_r: f64, p_h: f64) -> (f64, f64, f64, f64) {
        let var_r = p_r * (1.0 - p_r);
        let var_h = p_h * (1.0 - p_h);
        let a_target = p_r * p_h + self.config.correlation * (var_r * var_h).sqrt();

        let a_lo = (p_r + p_h - 1.0).max(0.0);
        let a_hi = p_r.min(p_h);
        debug_assert!(
            a_target >= a_lo - 1e-9 && a_target <= a_hi + 1e-9,
            "calibration must have rejected an infeasible correlation before \
             pricing: p_r={p_r}, p_h={p_h}, a={a_target}, bounds=[{a_lo}, {a_hi}]"
        );
        let a = a_target.clamp(a_lo, a_hi);

        // Marginals are exact by construction.
        (a, p_r - a, p_h - a, 1.0 - p_r - p_h + a)
    }

    /// Tree-conditional risk-free discount factors `P(t_n → t_m | rate node)`.
    ///
    /// For each rate node `i` at `reset_step`, returns the conditional
    /// zero-coupon price to `payment_step` implied by backward induction on
    /// the **rate-marginal** lattice using the **raw calibrated rates** —
    /// no OAS shift, no survival, no positive floor. The rate marginal is
    /// Markov on its own binomial lattice because the joint transition
    /// probabilities are built from the marginals by Fréchet coupling
    /// (`p_uu + p_ud = p_r` identically), so conditioning on the rate node
    /// alone is exact.
    ///
    /// This is the **forward-derivation operator** for node-dependent
    /// floating coupons: the simply-compounded node forward over the
    /// schedule accrual fraction τ is `(1/P − 1)/τ`. It is deliberately a
    /// *different* operator from the pricing-measure folding in
    /// [`Self::price_with_node_coupons`] (which applies OAS, survival, and
    /// the correlated joint transitions): node forwards are a property of
    /// the calibrated risk-free lattice and must not move with the OAS.
    ///
    /// # Arguments
    ///
    /// * `reset_step` - slice at which the forward is observed
    /// * `payment_step` - slice at which the notional would be repaid
    /// * `time_to_maturity` - total lattice horizon in years (defines `Δt`)
    ///
    /// # Returns
    ///
    /// `P[i]` for `i in 0..=reset_step`, each in `(0, ∞)`; higher rate
    /// nodes produce smaller discount factors.
    ///
    /// # Errors
    ///
    /// Returns an error when the tree is uncalibrated, the steps are not
    /// strictly ordered within the lattice, or `time_to_maturity` differs
    /// from the calibrated horizon.
    pub fn conditional_discount_factors(
        &self,
        reset_step: usize,
        payment_step: usize,
        time_to_maturity: f64,
    ) -> Result<Vec<f64>> {
        let steps = self.config.steps;
        let dt = self.validate_pricing_horizon(time_to_maturity)?;
        if reset_step >= payment_step || payment_step > steps {
            return Err(Error::Validation(format!(
                "conditional discounting requires reset_step < payment_step <= steps, \
                 got reset_step={reset_step}, payment_step={payment_step}, steps={steps}"
            )));
        }
        // Backward induction on the rate marginal: value 1 at payment_step,
        // discount with the raw calibrated node rate, transition with the
        // same mean-reversion-aware probability pricing uses.
        let mut values = vec![1.0; payment_step + 1];
        for k in (reset_step..payment_step).rev() {
            let mut next = vec![0.0; k + 1];
            for (i, slot) in next.iter_mut().enumerate() {
                let r = self.calibrated_rates[k].value_unchecked(i);
                let p_up = Self::mean_reverting_up_prob(
                    r,
                    self.rate_ref,
                    self.config.rate_mean_reversion,
                    self.config.rate_vol,
                    dt,
                );
                let df = (-r * dt).exp();
                *slot = df * (p_up * values[i + 1] + (1.0 - p_up) * values[i]);
            }
            values = next;
        }
        Ok(values)
    }

    /// Pricing-measure fold of one node coupon's increment onto its reset
    /// slice.
    ///
    /// Returns the amount to add to the **continuation value** at each
    /// joint node `(i, j)` of `coupon.reset_step` (flattened with stride
    /// `max_nodes = steps + 1`). The unit claim — 1 paid at
    /// `payment_step` contingent on survival — is rolled back with exactly
    /// the operator the main backward induction applies to a deterministic
    /// cashflow at that slice: per-step discounting at the raw calibrated
    /// rate **plus the active OAS**, per-step survival at the floored node
    /// hazard, and the correlated joint transitions. The valuator applies the
    /// reset slice's own survival weighting when it wraps the continuation, so
    /// this fold deliberately stops one survival factor short at the reset
    /// slice. The payment-slice claim always seeds at `1`: cash paid at slice
    /// `m` must not be survival-weighted over the following interval
    /// `m -> m + 1`, whether or not `m` is terminal.
    ///
    /// The increment amount per rate node is `ΔC(i)` as documented on
    /// [`NodeCoupon`]; multiplying the unit fold by `ΔC(i)` is exact
    /// because the amount is fixed at the reset node.
    fn folded_increment_values(
        &self,
        coupon: &NodeCoupon,
        time_to_maturity: f64,
        oas_decimal: f64,
    ) -> Result<Vec<f64>> {
        let steps = self.config.steps;
        let dt = self.validate_pricing_horizon(time_to_maturity)?;
        let max_nodes = steps + 1;
        let n = coupon.reset_step;
        let m = coupon.payment_step;

        // Node-dependent increment amounts from the forward-derivation
        // operator (raw rates, no OAS, no survival).
        let p_rf = self.conditional_discount_factors(n, m, time_to_maturity)?;
        let base_composed = calculate_floating_rate(coupon.base_index_rate, &coupon.params);
        let mut delta_amounts = vec![0.0; n + 1];
        for (i, slot) in delta_amounts.iter_mut().enumerate() {
            let node_forward = (1.0 / p_rf[i] - 1.0) / coupon.accrual;
            let delta_f = node_forward - coupon.base_discount_forward;
            let bumped = calculate_floating_rate(coupon.base_index_rate + delta_f, &coupon.params);
            *slot =
                coupon.notional * coupon.accrual * coupon.timing_scale * (bumped - base_composed);
        }

        // Unit-claim roll-back on the joint lattice with the pricing
        // operator. `w` holds the claim value at the slice currently being
        // consumed, in the same "post-valuator" form the main induction's
        // value function carries.
        let mut w = vec![0.0; max_nodes * max_nodes];
        let mut w_next = vec![0.0; max_nodes * max_nodes];
        for i in 0..=m {
            for j in 0..=m {
                w[i * max_nodes + j] = 1.0;
            }
        }
        for k in (n + 1..m).rev() {
            for i in 0..=k {
                let r = self.calibrated_rates[k].value_unchecked(i);
                let p_r = Self::mean_reverting_up_prob(
                    r,
                    self.rate_ref,
                    self.config.rate_mean_reversion,
                    self.config.rate_vol,
                    dt,
                );
                let df = (-(r + oas_decimal) * dt).exp();
                for j in 0..=k {
                    let h = self.calibrated_hazards[k].value_unchecked(j);
                    let p_h = Self::mean_reverting_up_prob(
                        h,
                        self.hazard_ref,
                        self.config.hazard_mean_reversion,
                        self.config.hazard_vol,
                        dt,
                    );
                    let (p_uu, p_ud, p_du, p_dd) = self.joint_probabilities(p_r, p_h);
                    let expectation = p_uu * w[(i + 1) * max_nodes + (j + 1)]
                        + p_ud * w[(i + 1) * max_nodes + j]
                        + p_du * w[i * max_nodes + (j + 1)]
                        + p_dd * w[i * max_nodes + j];
                    let p_surv = (-Self::effective_hazard(h) * dt).exp();
                    w_next[i * max_nodes + j] = p_surv * df * expectation;
                }
            }
            std::mem::swap(&mut w, &mut w_next);
        }

        // Assemble at the reset slice: continuation-form (discount + joint
        // expectation, no survival — the valuator applies it), scaled by the
        // node's increment amount.
        let mut folded = vec![0.0; max_nodes * max_nodes];
        for (i, &delta) in delta_amounts.iter().enumerate() {
            let r = self.calibrated_rates[n].value_unchecked(i);
            let p_r = Self::mean_reverting_up_prob(
                r,
                self.rate_ref,
                self.config.rate_mean_reversion,
                self.config.rate_vol,
                dt,
            );
            let df = (-(r + oas_decimal) * dt).exp();
            for j in 0..=n {
                let h = self.calibrated_hazards[n].value_unchecked(j);
                let p_h = Self::mean_reverting_up_prob(
                    h,
                    self.hazard_ref,
                    self.config.hazard_mean_reversion,
                    self.config.hazard_vol,
                    dt,
                );
                let (p_uu, p_ud, p_du, p_dd) = self.joint_probabilities(p_r, p_h);
                let expectation = p_uu * w[(i + 1) * max_nodes + (j + 1)]
                    + p_ud * w[(i + 1) * max_nodes + j]
                    + p_du * w[i * max_nodes + (j + 1)]
                    + p_dd * w[i * max_nodes + j];
                folded[i * max_nodes + j] = delta * df * expectation;
            }
        }
        Ok(folded)
    }
}

impl RatesCreditTree {
    /// Price with node-dependent floating-coupon increments folded into
    /// continuation at their reset slices.
    ///
    /// This is the full backward induction of [`TreeModel::price`] plus, at
    /// each [`NodeCoupon::reset_step`], the coupon's node-dependent
    /// increment added to the continuation value **before** the valuator's
    /// exercise decision and survival weighting. Adding it to continuation
    /// gives the correct exercise economics: a redemption at the reset
    /// slice forfeits the not-yet-accrued coupon, while a redemption at
    /// the payment slice still pays it (matching the engine's
    /// coupon-paid-regardless convention). Because the folded unit claim
    /// carries no exercise decisions between reset and payment, callers
    /// must ensure no exercise step lies strictly inside a node coupon's
    /// `(reset_step, payment_step)` — the instrument engines validate
    /// this before pricing.
    ///
    /// Two distinct operators are involved, per the callable-integration
    /// design:
    ///
    /// 1. **Forward derivation** — raw calibrated rates, no OAS, no
    ///    survival ([`Self::conditional_discount_factors`]); sets the
    ///    coupon amount at each rate node.
    /// 2. **Pricing-measure folding** — the same per-node discounting the
    ///    main induction applies (raw rate + OAS, floored-hazard survival,
    ///    correlated joint transitions); moves that amount from payment to
    ///    reset.
    ///
    /// With an empty `node_coupons` slice this is exactly
    /// [`TreeModel::price`], which delegates here.
    ///
    /// # Arguments
    ///
    /// * `initial_vars` - initial state variables; `"oas"` (basis points)
    ///   is applied as a parallel shift to the calibrated short rates
    /// * `time_to_maturity` - total lattice horizon in years
    /// * `market_context` - market data passed through to the valuator
    /// * `valuator` - instrument value function driven by the induction
    /// * `node_coupons` - future floating-coupon increments to fold at
    ///   their reset slices
    ///
    /// # Errors
    ///
    /// Returns an error when the tree is uncalibrated, the supplied horizon
    /// differs from calibration, a descriptor violates its invariants (see
    /// [`NodeCoupon`]), or the valuator fails.
    pub fn price_with_node_coupons<V: TreeValuator>(
        &self,
        initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
        node_coupons: &[NodeCoupon],
    ) -> Result<f64> {
        let steps = self.config.steps;
        let dt = self.validate_pricing_horizon(time_to_maturity)?;

        // OAS from initial variables (bp units, same convention as ShortRateTree)
        let oas_decimal = initial_vars
            .get(short_rate_keys::OAS)
            .copied()
            .unwrap_or(0.0)
            / 10_000.0;

        // Fold every node coupon's increment onto its reset slice up front;
        // the claims are independent of the instrument value function, so
        // they precompute cleanly. The fold depends on the active OAS, so
        // an OAS solve re-folds on every objective evaluation by design.
        let mut folded_by_step: Vec<Option<Vec<f64>>> = vec![None; steps];
        for coupon in node_coupons {
            coupon.validate(steps)?;
            let folded = self.folded_increment_values(coupon, time_to_maturity, oas_decimal)?;
            match &mut folded_by_step[coupon.reset_step] {
                Some(existing) => {
                    for (slot, add) in existing.iter_mut().zip(folded.iter()) {
                        *slot += add;
                    }
                }
                slot @ None => *slot = Some(folded),
            }
        }

        // Pre-allocate flat double buffers for backward induction (zero
        // allocations in the loop). Row-major `[i * max_nodes + j]` storage is
        // cache-friendlier than a `Vec<Vec<f64>>` (no per-row pointer chase).
        let max_nodes = steps + 1;
        let mut curr_values: Vec<f64> = vec![0.0; max_nodes * max_nodes];
        let mut next_values: Vec<f64> = vec![0.0; max_nodes * max_nodes];

        // No valuator used with this tree reads node coordinates from
        // `state.vars`; they consume the cached `interest_rate`/`hazard_rate`
        // fields and `state.step`. Build each `NodeState` via `with_cached`
        // (supplying the per-node values directly) and skip the per-node
        // `HashMap` writes entirely — `initial_vars` passes through unchanged.

        // Initialize terminal values
        for i in 0..=steps {
            let r_t = self.calibrated_rates[steps].value_unchecked(i);
            for j in 0..=steps {
                let h_t = self.calibrated_hazards[steps].value_unchecked(j);
                let cached = CachedValues {
                    interest_rate: Some(r_t.max(1e-8)),
                    hazard_rate: Some(Self::effective_hazard(h_t)),
                    ..CachedValues::default()
                };
                let state = NodeState::with_cached(
                    steps,
                    time_to_maturity,
                    &initial_vars,
                    market_context,
                    cached,
                );
                curr_values[i * max_nodes + j] = valuator.value_at_maturity(&state)?;
            }
        }

        // Backward induction with double-buffering
        for k in (0..steps).rev() {
            for i in 0..=k {
                let r_t = self.calibrated_rates[k].value_unchecked(i);

                // Rate transition probability with mean reversion. This is the
                // SAME function used during calibration; using an identical
                // per-node probability is what makes the tree reprice the
                // discount curve when mean reversion is non-zero.
                let p_r = Self::mean_reverting_up_prob(
                    r_t,
                    self.rate_ref,
                    self.config.rate_mean_reversion,
                    self.config.rate_vol,
                    dt,
                );

                for j in 0..=k {
                    let h_t = self.calibrated_hazards[k].value_unchecked(j);

                    // Hazard transition probability with mean reversion (same
                    // function and reference level used during calibration).
                    let p_h = Self::mean_reverting_up_prob(
                        h_t,
                        self.hazard_ref,
                        self.config.hazard_mean_reversion,
                        self.config.hazard_vol,
                        dt,
                    );

                    // Joint probabilities
                    let (p_uu, p_ud, p_du, p_dd) = self.joint_probabilities(p_r, p_h);

                    // Continuation from four children at step k+1
                    let v_uu = curr_values[(i + 1) * max_nodes + (j + 1)];
                    let v_ud = curr_values[(i + 1) * max_nodes + j];
                    let v_du = curr_values[i * max_nodes + (j + 1)];
                    let v_dd = curr_values[i * max_nodes + j];

                    // Risk-free discounting with calibrated rate + OAS.
                    //
                    // The rate is NOT floored here: `calibrate_factor_ho_lee`
                    // discounts with the raw (un-floored) calibrated rate, so
                    // backward induction must do the same or the tree will not
                    // reprice the discount curve once a wide lattice produces
                    // negative node rates. The `1e-8` floor is still applied
                    // below to the INTEREST_RATE / HAZARD_RATE *state
                    // variables*, which shields valuators that cannot accept
                    // non-positive rates.
                    let df = (-(r_t + oas_decimal) * dt).exp();
                    let mut cont = df * (p_uu * v_uu + p_ud * v_ud + p_du * v_du + p_dd * v_dd);

                    // Node-dependent floating-coupon increments fold into
                    // continuation at their reset slice, ahead of the
                    // valuator's exercise decision and survival weighting.
                    if let Some(folded) = folded_by_step.get(k).and_then(|f| f.as_ref()) {
                        cont += folded[i * max_nodes + j];
                    }

                    // `r_t`/`h_t` are floored only for the cached *state*
                    // variables (shields valuators that reject non-positive
                    // rates); the discounting above intentionally uses the raw
                    // calibrated rate.
                    let cached = CachedValues {
                        interest_rate: Some(r_t.max(1e-8)),
                        hazard_rate: Some(Self::effective_hazard(h_t)),
                        df: Some(df),
                        ..CachedValues::default()
                    };
                    let state = NodeState::with_cached(
                        k,
                        k as f64 * dt,
                        &initial_vars,
                        market_context,
                        cached,
                    );
                    next_values[i * max_nodes + j] = valuator.value_at_node(&state, cont, dt)?;
                }
            }
            // Swap buffers (O(1) pointer swap, no data copy)
            std::mem::swap(&mut curr_values, &mut next_values);
        }

        Ok(curr_values[0])
    }
}

impl TreeModel for RatesCreditTree {
    fn price<V: TreeValuator>(
        &self,
        initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
    ) -> Result<f64> {
        self.price_with_node_coupons(
            initial_vars,
            time_to_maturity,
            market_context,
            valuator,
            &[],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::market_data::traits::Discounting;
    use finstack_quant_core::math::interp::InterpStyle;

    fn targets_from_curves(
        disc: &dyn Discounting,
        hazard: &HazardCurve,
        steps: usize,
        horizon: f64,
    ) -> RatesCreditCalibrationTargets {
        let discount_at_origin = disc.df(0.0);
        let survival_at_origin = hazard.sp(0.0);
        let times: Vec<f64> = (0..=steps)
            .map(|step| step as f64 * horizon / steps as f64)
            .collect();
        RatesCreditCalibrationTargets {
            discount_factors: times
                .iter()
                .map(|&time| disc.df(time) / discount_at_origin)
                .collect(),
            survival_probabilities: times
                .iter()
                .map(|&time| hazard.sp(time) / survival_at_origin)
                .collect(),
            times,
            recovery_rate: hazard.recovery_rate(),
        }
    }

    fn calibrate_for_test(
        tree: &mut RatesCreditTree,
        disc: &dyn Discounting,
        hazard: &HazardCurve,
        horizon: f64,
    ) -> Result<()> {
        let targets = targets_from_curves(disc, hazard, tree.config.steps, horizon);
        tree.calibrate(&targets)
    }

    /// The default config is deterministic in both factors, so
    /// `..Default::default()` construction can never silently price
    /// optionality the caller did not request.
    #[test]
    fn default_config_is_deterministic_in_both_factors() {
        let cfg = RatesCreditConfig::default();
        assert_eq!(cfg.rate_vol, 0.0);
        assert_eq!(cfg.hazard_vol, 0.0);
        assert_eq!(cfg.correlation, 0.0);
        assert_eq!(cfg.rate_mean_reversion, 0.0);
        assert_eq!(cfg.hazard_mean_reversion, 0.0);
    }

    #[test]
    fn calibrated_factors_retain_one_affine_descriptor_per_step() {
        let steps = 64;
        let horizon = 10.0;
        let times: Vec<f64> = (0..=steps)
            .map(|step| step as f64 * horizon / steps as f64)
            .collect();
        let targets = RatesCreditCalibrationTargets {
            discount_factors: times.iter().map(|&time| (-0.03 * time).exp()).collect(),
            survival_probabilities: times.iter().map(|&time| (-0.02 * time).exp()).collect(),
            times,
            recovery_rate: 0.4,
        };
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            rate_vol: 0.01,
            hazard_vol: 0.005,
            ..Default::default()
        });
        tree.calibrate(&targets).expect("calibrate affine rows");

        assert_eq!(tree.calibrated_rates.len(), steps + 1);
        assert_eq!(tree.calibrated_hazards.len(), steps + 1);
        for step in 0..=steps {
            let rate_row = tree.calibrated_rates[step];
            let hazard_row = tree.calibrated_hazards[step];
            assert_eq!(rate_row.nodes, step + 1);
            assert_eq!(hazard_row.nodes, step + 1);
            assert_eq!(
                tree.rate_at_node(step, 0).expect("first rate"),
                rate_row.base
            );
            assert_eq!(
                tree.rate_at_node(step, step).expect("last rate"),
                rate_row.base + step as f64 * rate_row.shift
            );
            assert_eq!(
                tree.hazard_at_node(step, step).expect("last hazard"),
                hazard_row.base + step as f64 * hazard_row.shift
            );
        }
    }

    #[test]
    fn explicit_conditional_targets_define_levels_and_horizon() {
        let steps = 8;
        let horizon = 2.0;
        let times: Vec<f64> = (0..=steps)
            .map(|step| step as f64 * horizon / steps as f64)
            .collect();
        let targets = RatesCreditCalibrationTargets {
            discount_factors: times.iter().map(|&time| (-0.03 * time).exp()).collect(),
            survival_probabilities: times.iter().map(|&time| (-0.02 * time).exp()).collect(),
            times: times.clone(),
            recovery_rate: 0.35,
        };
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            ..Default::default()
        });
        tree.calibrate(&targets)
            .expect("explicit targets calibrate");

        assert_eq!(tree.time_grid().expect("grid"), times);
        assert!((tree.rate_at_node(0, 0).expect("rate") - 0.03).abs() < 1e-12);
        assert!((tree.hazard_at_node(0, 0).expect("hazard") - 0.02).abs() < 1e-12);
        assert_eq!(tree.recovery_rate(), 0.35);

        let err = tree
            .conditional_discount_factors(0, 1, horizon + 0.25)
            .expect_err("pricing with a different horizon must fail");
        assert!(err.to_string().contains("does not match"));
    }

    #[test]
    fn calibration_rejects_origin_and_grid_ambiguity() {
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps: 2,
            ..Default::default()
        });
        let valid = RatesCreditCalibrationTargets {
            times: vec![0.0, 0.5, 1.0],
            discount_factors: vec![1.0, 0.98, 0.95],
            survival_probabilities: vec![1.0, 0.99, 0.97],
            recovery_rate: 0.4,
        };

        let mut shifted_origin = valid.clone();
        shifted_origin.discount_factors[0] = 0.99;
        assert!(tree.calibrate(&shifted_origin).is_err());

        let mut uneven = valid.clone();
        uneven.times[1] = 0.4;
        assert!(tree.calibrate(&uneven).is_err());

        let mut increasing_survival = valid;
        increasing_survival.survival_probabilities[2] = 1.01;
        assert!(tree.calibrate(&increasing_survival).is_err());
    }

    #[test]
    fn extrema_correlation_scan_matches_exhaustive_node_pairs() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 16;
        let horizon = 5.0;
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            rate_vol: 0.012,
            hazard_vol: 0.05,
            rate_mean_reversion: 0.10,
            hazard_mean_reversion: 0.08,
            ..Default::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, horizon).expect("calibrate");
        let dt = tree.calibrated_dt().expect("dt");
        let (extrema_lo, extrema_hi, _) = tree.scan_correlation_feasibility(dt, None);

        let mut exhaustive_lo = -1.0_f64;
        let mut exhaustive_hi = 1.0_f64;
        for step in 0..steps {
            for rate in tree.calibrated_rates[step].values() {
                let p_rate = RatesCreditTree::mean_reverting_up_prob(
                    rate,
                    tree.rate_ref,
                    tree.config.rate_mean_reversion,
                    tree.config.rate_vol,
                    dt,
                );
                for hazard in tree.calibrated_hazards[step].values() {
                    let p_hazard = RatesCreditTree::mean_reverting_up_prob(
                        hazard,
                        tree.hazard_ref,
                        tree.config.hazard_mean_reversion,
                        tree.config.hazard_vol,
                        dt,
                    );
                    if let Some((lo, hi)) =
                        RatesCreditTree::node_correlation_range(p_rate, p_hazard)
                    {
                        exhaustive_lo = exhaustive_lo.max(lo);
                        exhaustive_hi = exhaustive_hi.min(hi);
                    }
                }
            }
        }
        assert!((extrema_lo - exhaustive_lo).abs() < 1e-14);
        assert!((extrema_hi - exhaustive_hi).abs() < 1e-14);
    }

    #[test]
    fn sampled_paths_are_seeded_weighted_and_antithetic() {
        let steps = 12;
        let horizon = 3.0;
        let times: Vec<f64> = (0..=steps)
            .map(|step| step as f64 * horizon / steps as f64)
            .collect();
        let targets = RatesCreditCalibrationTargets {
            discount_factors: times.iter().map(|&time| (-0.025 * time).exp()).collect(),
            survival_probabilities: times.iter().map(|&time| (-0.015 * time).exp()).collect(),
            times,
            recovery_rate: 0.4,
        };
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            rate_vol: 0.01,
            hazard_vol: 0.01,
            correlation: 1.0,
            ..Default::default()
        });
        tree.calibrate(&targets).expect("calibrate");

        let path = tree.sample_path(42, 7, false).expect("path");
        let replay = tree.sample_path(42, 7, false).expect("replay");
        let antithetic = tree.sample_path(42, 7, true).expect("antithetic");
        assert_eq!(path, replay);
        assert_eq!(path.len(), steps + 1);
        assert_eq!(antithetic.len(), steps + 1);

        for (left, right) in path.iter().zip(&antithetic) {
            assert_eq!(left.step, right.step);
            assert_eq!(left.rate_node + right.rate_node, left.step);
            assert_eq!(left.hazard_node + right.hazard_node, left.step);
            assert!(left.discount_to_next.is_finite() && left.discount_to_next > 0.0);
            assert!((left.survival_to_next + left.default_to_next - 1.0).abs() < 1e-14);
        }
        let terminal = path.last().expect("terminal");
        assert_eq!(terminal.discount_to_next, 1.0);
        assert_eq!(terminal.survival_to_next, 1.0);
        assert_eq!(terminal.default_to_next, 0.0);

        let mut reused = vec![RatesCreditPathState {
            step: usize::MAX,
            time: f64::NAN,
            rate_node: 0,
            hazard_node: 0,
            short_rate: 0.0,
            hazard_rate: 0.0,
            discount_to_next: 0.0,
            survival_to_next: 0.0,
            default_to_next: 0.0,
        }];
        tree.sample_path_into(42, 7, false, &mut reused)
            .expect("reused path");
        assert_eq!(reused, path);

        for (source, mirrored) in [(&path, false), (&antithetic, true)] {
            for (start, end) in [(0, 3), (1, 7), (5, steps), (steps, steps)] {
                let checkpoint = RatesCreditPathCheckpoint::from(&source[start]);
                tree.sample_path_segment_into(42, 7, mirrored, checkpoint, end, &mut reused)
                    .expect("resumed segment");
                assert_eq!(reused, source[start..=end]);
            }
        }
    }

    #[test]
    fn sampled_joint_moves_preserve_skewed_correlated_marginals() {
        let steps = 8;
        let horizon = 2.0;
        let disc = sloped_discount_curve();
        let hazard = test_hazard_curve();
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            rate_vol: 0.01,
            hazard_vol: 0.02,
            correlation: 0.20,
            rate_mean_reversion: 0.10,
            hazard_mean_reversion: 0.08,
        });
        calibrate_for_test(&mut tree, &disc, &hazard, horizon).expect("calibrate");

        let transition = tree
            .transition_probabilities(4, 0, 4)
            .expect("skewed interior transition");
        let expected_rate_up = transition.up_up + transition.up_down;
        let expected_hazard_up = transition.up_up + transition.down_up;
        assert!(
            (expected_rate_up - 0.5).abs() > 0.01 || (expected_hazard_up - 0.5).abs() > 0.01,
            "test transition must have a nontrivial skew: {transition:?}"
        );
        assert!(
            (transition.up_up - expected_rate_up * expected_hazard_up).abs() > 1e-3,
            "test transition must carry nonzero correlation: {transition:?}"
        );

        let draws = 50_000_usize;
        let mut rng = PhiloxRng::with_stream(0x5eed, 17);
        let mut uniform = [0.0_f64; 1];
        let mut rate_ups = 0_usize;
        let mut hazard_ups = 0_usize;
        let mut joint_ups = 0_usize;
        for _ in 0..draws {
            rng.fill_u01(&mut uniform);
            let (rate_up, hazard_up) = RatesCreditTree::sampled_moves(transition, uniform[0]);
            rate_ups += usize::from(rate_up);
            hazard_ups += usize::from(hazard_up);
            joint_ups += usize::from(rate_up && hazard_up);
        }
        let empirical_rate_up = rate_ups as f64 / draws as f64;
        let empirical_hazard_up = hazard_ups as f64 / draws as f64;
        let empirical_joint_up = joint_ups as f64 / draws as f64;
        assert!((empirical_rate_up - expected_rate_up).abs() < 0.01);
        assert!((empirical_hazard_up - expected_hazard_up).abs() < 0.01);
        assert!((empirical_joint_up - transition.up_up).abs() < 0.01);
    }

    // Lattice safety: marginals, correlation feasibility, floor saturation

    /// The Fréchet construction preserves both marginals exactly and realizes
    /// the requested correlation, including at skewed marginals where the old
    /// clamp-and-renormalise formulation silently distorted them.
    #[test]
    fn joint_probabilities_preserve_marginals_at_skewed_nodes() {
        for &(p_r, p_h) in &[
            (0.5, 0.5),
            (0.2, 0.8),
            (0.12, 0.5),
            (0.875, 0.125),
            (0.35, 0.4),
        ] {
            let (lo, hi) = RatesCreditTree::node_correlation_range(p_r, p_h)
                .expect("non-degenerate marginals");
            // Sample inside the feasible interval, including both endpoints.
            for &rho in &[lo, lo * 0.5, 0.0, hi * 0.5, hi] {
                let tree = RatesCreditTree::new(RatesCreditConfig {
                    rate_vol: 0.01,
                    hazard_vol: 0.20,
                    correlation: rho,
                    ..RatesCreditConfig::default()
                });
                let (p_uu, p_ud, p_du, p_dd) = tree.joint_probabilities(p_r, p_h);

                for (label, cell) in [("uu", p_uu), ("ud", p_ud), ("du", p_du), ("dd", p_dd)] {
                    assert!(
                        cell >= -1e-15,
                        "p_{label} negative at p_r={p_r}, p_h={p_h}, rho={rho}: {cell}"
                    );
                }
                let sum = p_uu + p_ud + p_du + p_dd;
                assert!(
                    (sum - 1.0).abs() < 1e-14,
                    "probabilities must sum to 1: {sum}"
                );
                assert!(
                    (p_uu + p_ud - p_r).abs() < 1e-14,
                    "rate marginal distorted at p_r={p_r}, p_h={p_h}, rho={rho}: {}",
                    p_uu + p_ud
                );
                assert!(
                    (p_uu + p_du - p_h).abs() < 1e-14,
                    "hazard marginal distorted at p_r={p_r}, p_h={p_h}, rho={rho}: {}",
                    p_uu + p_du
                );

                // Realized correlation with ±1 coding.
                let var_r = p_r * (1.0 - p_r);
                let var_h = p_h * (1.0 - p_h);
                let realized = (p_uu - p_r * p_h) / (var_r * var_h).sqrt();
                assert!(
                    (realized - rho).abs() < 1e-12,
                    "realized correlation {realized} != requested {rho} at \
                     p_r={p_r}, p_h={p_h}"
                );
            }
        }
    }

    /// Without mean reversion every marginal is ½, so the whole `[-1, 1]`
    /// range stays feasible and calibration accepts extreme correlations.
    #[test]
    fn correlation_is_unconstrained_without_mean_reversion() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        for &rho in &[-1.0, -0.99, 0.0, 0.99, 1.0] {
            let mut tree = RatesCreditTree::new(RatesCreditConfig {
                steps: 30,
                rate_vol: 0.012,
                hazard_vol: 0.05,
                correlation: rho,
                ..RatesCreditConfig::default()
            });
            calibrate_for_test(&mut tree, &disc, &haz, 5.0)
                .unwrap_or_else(|e| panic!("rho={rho} must calibrate without mean reversion: {e}"));
        }
    }

    /// An infeasible correlation fails at `calibrate()` — before any pricing —
    /// and the message carries the offending node, its marginals, and the
    /// largest usable |ρ|. A request at that bound then calibrates and prices.
    #[test]
    fn infeasible_correlation_fails_at_calibration_with_bound() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let ttm = 5.0;

        let strong_reversion = |rho: f64| RatesCreditConfig {
            steps: 40,
            rate_vol: 0.012,
            hazard_vol: 0.05,
            correlation: rho,
            rate_mean_reversion: KAPPA_MAX,
            hazard_mean_reversion: KAPPA_MAX,
        };

        let mut tree = RatesCreditTree::new(strong_reversion(0.9));
        let err = calibrate_for_test(&mut tree, &disc, &haz, ttm)
            .expect_err("rho = 0.9 must be infeasible under strong mean reversion");
        let msg = err.to_string();
        for needle in [
            "rate_credit_correlation",
            "not attainable",
            "step",
            "largest usable",
            "Fréchet",
        ] {
            assert!(msg.contains(needle), "message must mention {needle}: {msg}");
        }

        // The same bound is available programmatically, so a caller can pick a
        // workable correlation without scraping the message. A tree calibrated
        // at zero correlation exposes it; a request inside it then prices.
        let mut probe = RatesCreditTree::new(strong_reversion(0.0));
        calibrate_for_test(&mut probe, &disc, &haz, ttm).expect("probe calibrates");
        let bound = probe.max_feasible_correlation();
        assert!(
            (0.0..1.0).contains(&bound),
            "reported bound {bound} must be a proper fraction"
        );
        assert!(
            msg.contains(&format!("{bound:.4}")),
            "the error must quote the same bound the accessor reports \
             ({bound:.4}): {msg}"
        );

        let mut feasible = RatesCreditTree::new(strong_reversion(bound * 0.95));
        calibrate_for_test(&mut feasible, &disc, &haz, ttm)
            .expect("a correlation inside the reported bound must calibrate");
        feasible
            .price(
                HashMap::<&'static str, f64>::default(),
                ttm,
                &MarketContext::new(),
                &DummyValuator,
            )
            .expect("and must price");
    }

    /// The floor-saturation diagnostic is silent when the hazard lattice never
    /// floors and positive when it does — the signal that a *relative* spread
    /// vol was supplied where an *absolute* hazard vol belongs.
    #[test]
    fn hazard_floor_saturation_reports_binding_floor() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let ttm = 5.0;

        // Hazard ~2-3.5% with a small absolute vol: the lattice stays positive.
        let mut calm = RatesCreditTree::new(RatesCreditConfig {
            steps: 40,
            hazard_vol: 0.001,
            ..RatesCreditConfig::default()
        });
        calibrate_for_test(&mut calm, &disc, &haz, ttm).expect("calibrate calm");
        let calm_saturation = calm.hazard_floor_saturation();
        assert!(
            !calm_saturation.is_saturated(),
            "a small absolute hazard vol must not floor: {calm_saturation:?}"
        );
        assert_eq!(calm_saturation.unreachable_steps, 0);

        // 0.20 absolute hazard vol against a ~3% hazard is roughly a 600%
        // relative vol: the floor must bind hard and say so.
        let mut violent = RatesCreditTree::new(RatesCreditConfig {
            steps: 40,
            hazard_vol: 0.20,
            ..RatesCreditConfig::default()
        });
        calibrate_for_test(&mut violent, &disc, &haz, ttm).expect("calibrate violent");
        let violent_saturation = violent.hazard_floor_saturation();
        assert!(
            violent_saturation.is_saturated() && violent_saturation.max_mass_at_floor > 0.25,
            "an absurd absolute hazard vol must report heavy floor saturation: \
             {violent_saturation:?}"
        );
    }

    /// `1 − (κ·T)²` is the lattice-edge variance-retention ceiling, and `κ·T`
    /// — not `κ` alone — is what binds. Realized retention sits at or below the
    /// ceiling because the calibrated theta pushes rows further from the
    /// reversion reference than the symmetric geometry alone would.
    #[test]
    fn variance_retention_is_bounded_by_one_minus_kappa_t_squared() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();

        for (kappa, ttm) in [(0.05_f64, 5.0_f64), (0.10, 5.0), (0.15, 5.0), (0.10, 8.0)] {
            let ceiling = 1.0 - (kappa * ttm).powi(2);
            // The bound holds across volatilities and step counts, and the gap
            // to it closes as the lattice widens relative to the theta drift.
            let mut previous_gap = f64::INFINITY;
            for (steps, rate_vol) in [(40usize, 0.012), (100, 0.012), (200, 0.03)] {
                let mut tree = RatesCreditTree::new(RatesCreditConfig {
                    steps,
                    rate_vol,
                    rate_mean_reversion: kappa,
                    ..RatesCreditConfig::default()
                });
                calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibrate");
                let retention = tree.rate_variance_retention();
                let label = format!("kappa={kappa}, T={ttm}, steps={steps}, sigma={rate_vol}");

                assert_eq!(
                    retention.total_nodes,
                    steps * (steps + 1) / 2,
                    "{label}: the scan must visit exactly the nodes backward \
                     induction does"
                );
                assert!(
                    !retention.has_clamped_nodes(),
                    "{label}: kappa*T = {:.2} < 1 must not clamp: {retention:?}",
                    kappa * ttm
                );
                assert!(
                    retention.min_retention <= ceiling + 1e-12,
                    "{label}: min_retention {} must not exceed the \
                     1 - (kappa*T)^2 ceiling {ceiling}",
                    retention.min_retention
                );
                // Non-trivial: the ceiling would be vacuous if retention sat
                // near zero regardless.
                assert!(
                    retention.min_retention > 0.5 * ceiling,
                    "{label}: min_retention {} should track the ceiling \
                     {ceiling}, not collapse",
                    retention.min_retention
                );

                let gap = ceiling - retention.min_retention;
                assert!(
                    gap <= previous_gap + 1e-9,
                    "{label}: the gap to the ceiling ({gap}) should not widen \
                     as the lattice widens (previous {previous_gap})"
                );
                previous_gap = gap;
            }
        }
    }

    /// Past `κ·T = 1` the wing marginals clamp: those nodes carry zero
    /// conditional variance and, being degenerate Bernoullis, express no
    /// correlation at all. `KAPPA_MAX` does not bound this — only the
    /// diagnostic reports it.
    #[test]
    fn long_horizon_clamping_is_reported_not_silent() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();

        // kappa*T = 0.15 * 5 = 0.75 < 1: no clamping.
        let mut short = RatesCreditTree::new(RatesCreditConfig {
            steps: 40,
            rate_vol: 0.012,
            rate_mean_reversion: KAPPA_MAX,
            ..RatesCreditConfig::default()
        });
        calibrate_for_test(&mut short, &disc, &haz, 5.0).expect("calibrate short");
        assert!(
            !short.rate_variance_retention().has_clamped_nodes(),
            "kappa*T = 0.75 must not clamp: {:?}",
            short.rate_variance_retention()
        );

        // kappa*T = 0.15 * 10 = 1.5 >= 1: the wings clamp, at a kappa the
        // KAPPA_MAX guard accepts. This is the gap the diagnostic closes.
        let mut long = RatesCreditTree::new(RatesCreditConfig {
            steps: 40,
            rate_vol: 0.012,
            rate_mean_reversion: KAPPA_MAX,
            ..RatesCreditConfig::default()
        });
        calibrate_for_test(&mut long, &disc, &haz, 10.0).expect("calibrate long");
        let retention = long.rate_variance_retention();
        assert!(
            retention.has_clamped_nodes(),
            "kappa*T = 1.5 must clamp the lattice wings: {retention:?}"
        );
        assert_eq!(
            retention.min_retention, 0.0,
            "a clamped marginal has zero conditional variance"
        );
        assert!(
            retention.clamped_share() > 0.0 && retention.clamped_share() < 1.0,
            "clamping must hit the wings, not the whole lattice: {}",
            retention.clamped_share()
        );
    }

    /// Without mean reversion every marginal is exactly one half, so the
    /// diagnostic reports the undistorted limit — and does so for the hazard
    /// factor independently of the rate factor.
    #[test]
    fn variance_retention_is_undistorted_without_mean_reversion() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();

        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps: 40,
            rate_vol: 0.012,
            hazard_vol: 0.01,
            hazard_mean_reversion: 0.08,
            ..RatesCreditConfig::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, 5.0).expect("calibrate");

        // Rate factor has no mean reversion: undistorted default.
        assert_eq!(tree.rate_variance_retention(), VarianceRetention::default());
        // Hazard factor does: it is scanned, and reports real numbers.
        let hazard = tree.hazard_variance_retention();
        assert!(hazard.total_nodes > 0, "hazard factor must be scanned");
        assert!(
            hazard.min_retention < 1.0 && hazard.min_retention > 0.0,
            "kappa*T = 0.4 must distort without clamping: {hazard:?}"
        );

        // An uncalibrated tree reports the undistorted default for both.
        let fresh = RatesCreditTree::new(RatesCreditConfig::default());
        assert_eq!(
            fresh.rate_variance_retention(),
            VarianceRetention::default()
        );
        assert_eq!(
            fresh.hazard_variance_retention(),
            VarianceRetention::default()
        );
    }

    /// Deterministic limits: the zero-vol lattice is the deterministic
    /// backward induction, and both vols tending to zero converge to it.
    /// There is no separate rate-only or hazard-only tree.
    #[test]
    fn zero_volatility_factors_are_deterministic_limits() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let ttm = 5.0;
        let ctx = MarketContext::new();

        let price_at = |rate_vol: f64, hazard_vol: f64| -> f64 {
            let mut tree = RatesCreditTree::new(RatesCreditConfig {
                steps: 40,
                rate_vol,
                hazard_vol,
                ..RatesCreditConfig::default()
            });
            calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibrate");
            tree.price(
                HashMap::<&'static str, f64>::default(),
                ttm,
                &ctx,
                &DummyValuator,
            )
            .expect("price")
        };

        // zero/zero reprices the discount curve (DummyValuator pays 1 and
        // passes continuation through, so the tree price is the ZCB price).
        let deterministic = price_at(0.0, 0.0);
        let market_df = disc.df(ttm);
        assert!(
            (deterministic - market_df).abs() * 10_000.0 < 1.0,
            "zero/zero must reprice the discount curve: tree={deterministic:.8}, \
             market={market_df:.8}"
        );

        // Each single-factor regime reprices the same curve.
        for (label, rate_vol, hazard_vol) in
            [("nonzero/zero", 0.01, 0.0), ("zero/nonzero", 0.0, 0.02)]
        {
            let price = price_at(rate_vol, hazard_vol);
            assert!(
                price.is_finite() && (price - market_df).abs() * 10_000.0 < 1.0,
                "{label} must reprice the discount curve: {price:.8} vs {market_df:.8}"
            );
        }

        // Both vols tending to zero converge to the zero/zero result.
        let mut previous = f64::INFINITY;
        for scale in [1e-2, 1e-3, 1e-4, 1e-5] {
            let gap = (price_at(0.01 * scale, 0.02 * scale) - deterministic).abs();
            assert!(
                gap <= previous + 1e-12,
                "convergence must be monotone as vols shrink: gap={gap}, previous={previous}"
            );
            previous = gap;
        }
        assert!(
            previous < 1e-9,
            "the vanishing-vol limit must reach the deterministic price, gap={previous}"
        );
    }

    /// The joint transition probabilities must realize the configured
    /// correlation exactly in the moderate regime (no clamping active).
    /// With balanced marginals `p_r = p_h = 0.5` and ±1 coding of the
    /// up/down moves, E[X] = E[Y] = 0 and Var[X] = Var[Y] = 1, so
    /// realized ρ = p_uu + p_dd − p_ud − p_du, and no cell can clamp for
    /// any |ρ| ≤ 1 (each cell is 0.25·(1 ± ρ) ∈ [0, 0.5]).
    ///
    /// Note the deliberate limitation this test does NOT cover: at skewed
    /// marginals a large |ρ| can exceed the Fréchet bound for two
    /// Bernoullis; the cell clamp + renormalisation then reduces the
    /// realized correlation (and shifts the marginals) with no diagnostic.
    /// See the doc comment on `joint_probabilities`.
    #[test]
    fn joint_probabilities_realize_configured_correlation_when_unclamped() {
        for &target_rho in &[-0.9, -0.5, 0.0, 0.5, 0.9] {
            let tree = RatesCreditTree::new(RatesCreditConfig {
                rate_vol: 0.01,
                hazard_vol: 0.20,
                correlation: target_rho,
                ..RatesCreditConfig::default()
            });
            let (p_uu, p_ud, p_du, p_dd) = tree.joint_probabilities(0.5, 0.5);

            let sum = p_uu + p_ud + p_du + p_dd;
            assert!((sum - 1.0).abs() < 1e-14, "probs must sum to 1, got {sum}");
            // Marginals preserved.
            assert!(
                (p_uu + p_ud - 0.5).abs() < 1e-14 && (p_uu + p_du - 0.5).abs() < 1e-14,
                "marginals distorted at rho={target_rho}: pr={}, ph={}",
                p_uu + p_ud,
                p_uu + p_du
            );
            // Realized correlation matches the configured value exactly.
            let realized = p_uu + p_dd - p_ud - p_du;
            assert!(
                (realized - target_rho).abs() < 1e-14,
                "realized correlation {realized} != configured {target_rho}"
            );
        }
    }

    struct DummyValuator;

    impl TreeValuator for DummyValuator {
        fn value_at_maturity(&self, _state: &NodeState) -> Result<f64> {
            Ok(1.0)
        }
        fn value_at_node(
            &self,
            _state: &NodeState,
            continuation_value: f64,
            _dt: f64,
        ) -> Result<f64> {
            Ok(continuation_value)
        }
    }

    fn test_base_date() -> finstack_quant_core::dates::Date {
        finstack_quant_core::dates::Date::from_calendar_date(2025, time::Month::January, 1)
            .expect("valid date")
    }

    fn sloped_discount_curve() -> DiscountCurve {
        DiscountCurve::builder("USD-OIS")
            .base_date(test_base_date())
            .knots([
                (0.0, 1.0),
                (1.0, 0.96),
                (2.0, 0.91),
                (3.0, 0.86),
                (5.0, 0.78),
                (10.0, 0.60),
            ])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("curve should build")
    }

    fn test_hazard_curve() -> HazardCurve {
        use finstack_quant_core::market_data::term_structures::ParInterp;
        HazardCurve::builder("TEST-HAZ")
            .base_date(test_base_date())
            .recovery_rate(0.4)
            .knots([(0.0, 0.02), (2.0, 0.025), (5.0, 0.03), (10.0, 0.035)])
            .par_interp(ParInterp::Linear)
            .build()
            .expect("hazard curve should build")
    }

    fn near_zero_discount_curve() -> DiscountCurve {
        DiscountCurve::builder("USD-OIS")
            .base_date(test_base_date())
            .knots([
                (0.0, 1.0),
                (1.0, (-0.000001_f64).exp()),
                (2.0, (-0.000002_f64).exp()),
                (5.0, (-0.000005_f64).exp()),
            ])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("curve should build")
    }

    fn near_zero_hazard_curve() -> HazardCurve {
        use finstack_quant_core::market_data::term_structures::ParInterp;
        HazardCurve::builder("LOW-HAZ")
            .base_date(test_base_date())
            .recovery_rate(0.4)
            .knots([(0.0, 1e-8), (2.0, 1e-8), (5.0, 1e-8)])
            .par_interp(ParInterp::Linear)
            .build()
            .expect("hazard curve should build")
    }

    /// Valuator that pays step-indexed cashflows under the same survival
    /// convention the bond/term-loan valuators use (no recovery, no
    /// exercise). Used to probe the node-coupon fold in isolation.
    struct SurvivalCashflowValuator {
        cashflows: Vec<f64>,
    }

    impl TreeValuator for SurvivalCashflowValuator {
        fn value_at_maturity(&self, state: &NodeState) -> Result<f64> {
            Ok(self.cashflows.get(state.step).copied().unwrap_or(0.0))
        }
        fn value_at_node(
            &self,
            state: &NodeState,
            continuation_value: f64,
            dt: f64,
        ) -> Result<f64> {
            let cash = self.cashflows.get(state.step).copied().unwrap_or(0.0);
            let risky_continuation = state.hazard_rate.map_or(continuation_value, |hazard| {
                (-hazard.max(0.0) * dt).exp() * continuation_value
            });
            Ok(cash + risky_continuation)
        }
    }

    /// With zero rate volatility every rate node collapses onto the
    /// calibrated deterministic path, so the tree-conditional discount
    /// factor must equal the market forward discount factor over the same
    /// slice interval.
    #[test]
    fn conditional_discount_factors_match_curve_at_zero_rate_vol() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 40;
        let ttm = 5.0;
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            rate_vol: 0.0,
            hazard_vol: 0.0,
            ..Default::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibration");

        let dt = ttm / steps as f64;
        let (n, m) = (16usize, 20usize);
        let market_fwd_df = disc.df(m as f64 * dt) / disc.df(n as f64 * dt);
        let conditional = tree
            .conditional_discount_factors(n, m, ttm)
            .expect("conditional discounting");
        assert_eq!(conditional.len(), n + 1);
        for (i, p) in conditional.iter().enumerate() {
            assert!(
                (p - market_fwd_df).abs() < 1e-9,
                "node {i}: conditional DF {p} must equal market forward DF {market_fwd_df} \
                 when the rate factor is deterministic"
            );
        }
    }

    /// Higher rate nodes must produce smaller conditional discount factors,
    /// and the derivation must ignore the OAS entirely (it takes no OAS
    /// input — asserted here by construction through the API shape).
    #[test]
    fn conditional_discount_factors_decrease_in_rate_node() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 40;
        let ttm = 5.0;
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            rate_vol: 0.015,
            hazard_vol: 0.0,
            ..Default::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibration");

        let conditional = tree
            .conditional_discount_factors(16, 24, ttm)
            .expect("conditional discounting");
        for pair in conditional.windows(2) {
            assert!(
                pair[1] < pair[0],
                "conditional DF must strictly decrease in the rate node: {conditional:?}"
            );
        }
        assert!(conditional.iter().all(|p| *p > 0.0 && p.is_finite()));
    }

    /// The razor identity behind the whole floating-reset design: for an
    /// increment defined off the slice-interval market forward with identity
    /// composition, the fold prices to **zero** when rates and credit are
    /// uncorrelated — for any rate vol, hazard vol, and OAS. The increment
    /// `N·((1/P − 1) − f·τ)` satisfies
    /// `E[D·(1/P − 1 − f·τ)·P] = (DF(n) − DF(m)) − f·τ·DF(m) = 0`, and with
    /// `ρ = 0` the survival and OAS factors multiply out node-independently.
    /// A non-zero correlation breaks the factorization and must move the
    /// value — that is precisely the coupon/discount-survival covariance the
    /// milestone exists to capture.
    #[test]
    fn pure_forward_increment_folds_to_zero_without_correlation() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 40;
        let ttm = 5.0;
        let dt = ttm / steps as f64;
        let (n, m) = (16usize, 20usize);
        let tau = (m - n) as f64 * dt;
        let slice_fwd = (disc.df(n as f64 * dt) / disc.df(m as f64 * dt) - 1.0) / tau;

        let coupon = NodeCoupon {
            reset_step: n,
            payment_step: m,
            accrual: tau,
            notional: 1_000_000.0,
            base_index_rate: slice_fwd,
            base_discount_forward: slice_fwd,
            timing_scale: 1.0,
            params: FloatingRateParams::default(),
        };
        let valuator = SurvivalCashflowValuator {
            cashflows: vec![0.0; steps + 1],
        };

        let price_with_rho = |rho: f64| -> f64 {
            let mut tree = RatesCreditTree::new(RatesCreditConfig {
                steps,
                rate_vol: 0.015,
                hazard_vol: 0.01,
                correlation: rho,
                ..Default::default()
            });
            calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibration");
            let mut vars = HashMap::<&'static str, f64>::default();
            vars.insert(short_rate_keys::OAS, 175.0);
            tree.price_with_node_coupons(
                vars,
                ttm,
                &MarketContext::new(),
                &valuator,
                std::slice::from_ref(&coupon),
            )
            .expect("pricing")
        };

        let uncorrelated = price_with_rho(0.0);
        assert!(
            uncorrelated.abs() < 1e-2,
            "pure forward increment must fold to ~zero at rho = 0 (got {uncorrelated})"
        );

        // Positive rate/credit correlation puts the high-coupon states on
        // the low-survival paths, so the coupon-specific covariance is
        // strictly negative — this is the isolated sign of the channel the
        // floating-reset milestone exists to capture. (At the instrument
        // level the total correlation response also carries the opposing
        // risky-discount covariance `E[S·D]` on every booked cashflow,
        // which exists for fixed bonds too.)
        let correlated = price_with_rho(0.5);
        assert!(
            correlated < -1.0,
            "positive correlation must make the folded increment strictly \
             negative (high coupons on low-survival paths), got {correlated}"
        );
    }

    /// A node-coupon increment paid on an interior slice must have the same
    /// value as an otherwise identical deterministic cashflow booked on that
    /// slice. In particular, positive hazard applies only through the interval
    /// ending at the payment slice; seeding the fold with survival from the
    /// payment slice to the next one would default-discount the coupon twice.
    #[test]
    fn interior_node_coupon_payment_matches_cashflow_under_positive_hazard() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 12;
        let ttm = 3.0;
        let dt = ttm / steps as f64;
        let (n, m) = (2usize, 7usize);
        let tau = (m - n) as f64 * dt;
        let notional = 1_000_000.0;
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            rate_vol: 0.0,
            hazard_vol: 0.0,
            ..Default::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibration");
        assert!(tree.hazard_at_node(m, 0).expect("hazard") > 0.0);

        let reset_df = tree
            .conditional_discount_factors(n, m, ttm)
            .expect("conditional discount")[0];
        let node_forward = (1.0 / reset_df - 1.0) / tau;
        let increment = notional * tau * node_forward;
        let coupon = NodeCoupon {
            reset_step: n,
            payment_step: m,
            accrual: tau,
            notional,
            base_index_rate: 0.0,
            base_discount_forward: 0.0,
            timing_scale: 1.0,
            params: FloatingRateParams::default(),
        };

        let mut vars = HashMap::<&'static str, f64>::default();
        vars.insert(short_rate_keys::OAS, 125.0);
        let folded = tree
            .price_with_node_coupons(
                vars.clone(),
                ttm,
                &MarketContext::new(),
                &SurvivalCashflowValuator {
                    cashflows: vec![0.0; steps + 1],
                },
                std::slice::from_ref(&coupon),
            )
            .expect("node-coupon price");

        let mut cashflows = vec![0.0; steps + 1];
        cashflows[m] = increment;
        let direct = tree
            .price(
                vars,
                ttm,
                &MarketContext::new(),
                &SurvivalCashflowValuator { cashflows },
            )
            .expect("direct cashflow price");

        assert!(
            (folded - direct).abs() < 1.0e-8,
            "interior payment must carry exactly the same discount and survival as current cash: folded={folded}, direct={direct}"
        );
    }

    /// The node-coupon path with an empty descriptor slice is exactly the
    /// plain pricing path.
    #[test]
    fn empty_node_coupons_match_plain_price() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 30;
        let ttm = 5.0;
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            rate_vol: 0.012,
            hazard_vol: 0.015,
            ..Default::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibration");

        let mut cashflows = vec![0.0; steps + 1];
        cashflows[steps] = 1_000_000.0;
        let valuator = SurvivalCashflowValuator { cashflows };

        let plain = tree
            .price(
                HashMap::<&'static str, f64>::default(),
                ttm,
                &MarketContext::new(),
                &valuator,
            )
            .expect("plain price");
        let with_empty = tree
            .price_with_node_coupons(
                HashMap::<&'static str, f64>::default(),
                ttm,
                &MarketContext::new(),
                &valuator,
                &[],
            )
            .expect("empty-coupon price");
        assert_eq!(plain, with_empty);
    }

    /// Descriptor invariants are enforced before any folding happens.
    #[test]
    fn node_coupon_descriptor_validation_rejects_bad_geometry() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 20;
        let ttm = 5.0;
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            rate_vol: 0.01,
            hazard_vol: 0.0,
            ..Default::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibration");
        let valuator = SurvivalCashflowValuator {
            cashflows: vec![0.0; steps + 1],
        };

        let base = NodeCoupon {
            reset_step: 4,
            payment_step: 8,
            accrual: 1.0,
            notional: 100.0,
            base_index_rate: 0.03,
            base_discount_forward: 0.03,
            timing_scale: 1.0,
            params: FloatingRateParams::default(),
        };

        let collapsed = NodeCoupon {
            payment_step: 4,
            ..base.clone()
        };
        let err = tree
            .price_with_node_coupons(
                HashMap::default(),
                ttm,
                &MarketContext::new(),
                &valuator,
                std::slice::from_ref(&collapsed),
            )
            .expect_err("collapsed reset/payment must be rejected");
        assert!(
            err.to_string().contains("too coarse"),
            "unexpected error: {err}"
        );

        let beyond = NodeCoupon {
            payment_step: steps + 1,
            ..base.clone()
        };
        assert!(tree
            .price_with_node_coupons(
                HashMap::default(),
                ttm,
                &MarketContext::new(),
                &valuator,
                std::slice::from_ref(&beyond),
            )
            .is_err());

        let bad_accrual = NodeCoupon {
            accrual: 0.0,
            ..base
        };
        assert!(tree
            .price_with_node_coupons(
                HashMap::default(),
                ttm,
                &MarketContext::new(),
                &valuator,
                std::slice::from_ref(&bad_accrual),
            )
            .is_err());
    }

    #[test]
    fn rates_credit_calibrated_prices_positive() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        // Both factors stochastic: stated explicitly now that the default is
        // deterministic.
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps: 40,
            rate_vol: 0.01,
            hazard_vol: 0.20,
            ..Default::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, 5.0).expect("calibration");

        let ctx = MarketContext::new();
        let vars = HashMap::<&'static str, f64>::default();
        let val = DummyValuator;
        let price = tree.price(vars, 5.0, &ctx, &val).expect("should succeed");
        assert!(price.is_finite() && price > 0.0);
    }

    #[test]
    fn uncalibrated_tree_returns_error() {
        let tree = RatesCreditTree::new(RatesCreditConfig::default());
        let ctx = MarketContext::new();
        let vars = HashMap::<&'static str, f64>::default();
        let val = DummyValuator;
        let result = tree.price(vars, 1.0, &ctx, &val);
        assert!(result.is_err(), "price() without calibrate() must fail");
    }

    /// Verify that tree-implied ZCB prices at each step match `disc.df(t)` within 1e-6.
    ///
    /// The DummyValuator passes continuation through unchanged and pays 1.0 at
    /// maturity, so tree price = ZCB price ≈ disc.df(T) for any number of steps.
    #[test]
    fn calibration_quality_zcb_repricing() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 60;
        let ttm = 5.0;

        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            rate_vol: 0.01,
            hazard_vol: 0.0, // no hazard vol → pure rate test
            ..Default::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibrate");

        let ctx = MarketContext::new();
        let vars = HashMap::<&'static str, f64>::default();
        let val = DummyValuator;
        let tree_price = tree.price(vars, ttm, &ctx, &val).expect("price");
        let market_df = disc.df(ttm);

        let error_bp = (tree_price - market_df).abs() * 10_000.0;
        assert!(
            error_bp < 1.0, // within 1 bp
            "ZCB repricing error = {:.4} bp (tree={:.8}, market={:.8})",
            error_bp,
            tree_price,
            market_df
        );
    }

    /// Verify that calibrated hazard rates reproduce the hazard curve's survival
    /// probabilities at each step, using Arrow-Debreu forward induction on the
    /// 1D hazard lattice.
    #[test]
    fn calibration_quality_survival_matching() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 50;
        let ttm = 5.0;
        let dt = ttm / steps as f64;

        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            hazard_vol: 0.20,
            ..Default::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibrate");

        // Forward-propagate Arrow-Debreu state prices through the calibrated
        // hazard lattice to compute model survival probability at each step.
        // The non-negative transform is applied here exactly as calibration
        // and `price()` apply it: a negative additive-normal node is not a
        // credit state, and all three passes must agree or the tree stops
        // reproducing the survival curve the valuator actually sees.
        let mut state_prices = vec![1.0_f64]; // Q_h[0] = 1.0

        for k in 0..steps {
            let next_nodes = k + 2;
            let mut next_sp = vec![0.0_f64; next_nodes];
            for j in 0..=k {
                let h_j = RatesCreditTree::effective_hazard(
                    tree.calibrated_hazards[k].value_unchecked(j),
                );
                let surv_df = (-h_j * dt).exp();
                let q = state_prices[j];
                // Up move to j+1, down move to j — p = 0.5 each (no mean reversion)
                if j + 1 < next_nodes {
                    next_sp[j + 1] += q * surv_df * 0.5;
                }
                next_sp[j] += q * surv_df * 0.5;
            }
            state_prices = next_sp;

            // Model survival probability at step k+1 = sum of state prices
            let model_sp: f64 = state_prices.iter().sum();
            let t = (k + 1) as f64 * dt;
            let market_sp = haz.sp(t);

            let error = (model_sp - market_sp).abs();
            assert!(
                error < 1e-6,
                "Survival mismatch at step {} (t={:.3}): model={:.8}, market={:.8}, err={:.2e}",
                k + 1,
                t,
                model_sp,
                market_sp,
                error
            );
        }
    }

    #[test]
    fn near_zero_rates_with_mean_reversion_price_finitely() {
        let disc = near_zero_discount_curve();
        let haz = near_zero_hazard_curve();
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps: 20,
            rate_vol: 0.20,
            hazard_vol: 0.20,
            rate_mean_reversion: 0.001,
            hazard_mean_reversion: 0.001,
            ..Default::default()
        });

        calibrate_for_test(&mut tree, &disc, &haz, 2.0).expect("calibration");

        let price = tree
            .price(
                HashMap::<&'static str, f64>::default(),
                2.0,
                &MarketContext::new(),
                &DummyValuator,
            )
            .expect("pricing should succeed");

        assert!(price.is_finite() && price > 0.0, "price={price}");
    }

    /// The tree must reprice the input discount curve **with mean reversion
    /// active**. The `DummyValuator` pays 1.0 at maturity and passes
    /// continuation through unchanged, so the tree price equals the implied
    /// ZCB price, which must match `disc.df(T)`.
    ///
    /// On the parent (`e7dd696da`) this test fails by hundreds of bp: the
    /// calibration assumed `p = 0.5` while pricing used a different
    /// mean-reversion-dependent probability, so the tree no longer repriced
    /// the curve once `rate_mean_reversion != 0`.
    ///
    /// κ values are capped at `KAPPA_MAX` (= 0.15) because above that threshold
    /// `calibrate()` returns a `Validation` error. The fix is still demonstrated
    /// by these values — the parent was off by > 1000 bp even at κ = 0.05.
    #[test]
    fn calibration_reprices_disc_curve_with_rate_mean_reversion() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let ttm = 5.0;

        for &kappa in &[0.05_f64, 0.10, 0.15] {
            let mut tree = RatesCreditTree::new(RatesCreditConfig {
                steps: 60,
                rate_vol: 0.012,
                hazard_vol: 0.0, // isolate the rate factor
                rate_mean_reversion: kappa,
                ..Default::default()
            });
            calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibrate");

            let ctx = MarketContext::new();
            let price = tree
                .price(
                    HashMap::<&'static str, f64>::default(),
                    ttm,
                    &ctx,
                    &DummyValuator,
                )
                .expect("price");
            let market_df = disc.df(ttm);
            let error_bp = (price - market_df).abs() * 10_000.0;
            assert!(
                error_bp < 1.0,
                "kappa={kappa}: ZCB repricing error {error_bp:.4} bp \
                 (tree={price:.8}, market={market_df:.8})",
            );
        }
    }

    /// The hazard factor must likewise reprice the survival curve when its own
    /// mean reversion is active.
    #[test]
    fn calibration_reprices_survival_with_hazard_mean_reversion() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 60;
        let ttm = 5.0;
        let dt = ttm / steps as f64;

        // κ values capped at KAPPA_MAX (0.15); above that calibrate() returns
        // a Validation error (see `mean_reversion_above_kappa_max_returns_validation_error`).
        for &kappa in &[0.05_f64, 0.10, 0.15] {
            let mut tree = RatesCreditTree::new(RatesCreditConfig {
                steps,
                hazard_vol: 0.20,
                hazard_mean_reversion: kappa,
                ..Default::default()
            });
            calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibrate");

            // Forward-propagate Arrow-Debreu state prices through the
            // calibrated hazard lattice using the SAME mean-reversion
            // probability the calibration applied. The summed state prices at
            // step k must equal the market survival probability at t_k.
            let mut state_prices = vec![1.0_f64];
            for k in 0..steps {
                let next_nodes = k + 2;
                let mut next_sp = vec![0.0_f64; next_nodes];
                for j in 0..=k {
                    // The raw additive-normal level drives the lattice
                    // dynamics (it is what recombines with uniform spacing, so
                    // the mean-reversion drift is measured on it); the
                    // non-negative transform applies only where the level is
                    // read as a credit intensity. Calibration and `price()`
                    // make exactly this split.
                    let raw_j = tree.hazard_at_node(k, j).expect("node");
                    let surv_df = (-RatesCreditTree::effective_hazard(raw_j) * dt).exp();
                    let q = state_prices[j];
                    let p_up = RatesCreditTree::mean_reverting_up_prob(
                        raw_j,
                        tree.hazard_ref,
                        kappa,
                        0.20,
                        dt,
                    );
                    if j + 1 < next_nodes {
                        next_sp[j + 1] += q * surv_df * p_up;
                    }
                    next_sp[j] += q * surv_df * (1.0 - p_up);
                }
                state_prices = next_sp;

                let model_sp: f64 = state_prices.iter().sum();
                let t = (k + 1) as f64 * dt;
                let market_sp = haz.sp(t);
                let error = (model_sp - market_sp).abs();
                assert!(
                    error < 1e-6,
                    "kappa={kappa}: survival mismatch at step {} (t={t:.3}): \
                     model={model_sp:.8}, market={market_sp:.8}, err={error:.2e}",
                    k + 1,
                );
            }
        }
    }

    /// At **zero** mean reversion the two-factor tree's rate factor must
    /// reproduce a standalone Ho-Lee `ShortRateTree` node-for-node: both use
    /// the identical additive lattice (`r ± σ√Δt`, `p = ½`, theta calibration).
    #[test]
    fn rate_factor_matches_short_rate_tree_at_zero_mean_reversion() {
        use super::super::short_rate_tree::{ShortRateTree, ShortRateTreeConfig};

        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 50;
        let ttm = 5.0;
        let vol = 0.011;

        let mut two_factor = RatesCreditTree::new(RatesCreditConfig {
            steps,
            rate_vol: vol,
            hazard_vol: 0.0,
            rate_mean_reversion: 0.0,
            ..Default::default()
        });
        calibrate_for_test(&mut two_factor, &disc, &haz, ttm).expect("calibrate 2F");

        let mut short_rate = ShortRateTree::new(ShortRateTreeConfig::ho_lee(steps, vol));
        short_rate.calibrate(&disc, ttm).expect("calibrate SR");

        // Compare every calibrated rate node. The terminal row (step == steps)
        // is geometry-only and excluded from both trees' discounting, so the
        // comparison covers the discounting rows 0..steps.
        for step in 0..steps {
            for node in 0..=step {
                let r_2f = two_factor.rate_at_node(step, node).expect("2F node");
                let r_sr = short_rate.rate_at_node(step, node).expect("SR node");
                assert!(
                    (r_2f - r_sr).abs() < 1e-10,
                    "rate mismatch at (step={step}, node={node}): \
                     two_factor={r_2f:.12}, short_rate={r_sr:.12}",
                );
            }
        }
    }

    /// Calibration must reject mean-reversion speeds above `KAPPA_MAX` with a
    /// `Validation` error that names the offending value, the threshold, and
    /// the variance-degradation reason. Values at or below the threshold pass.
    #[test]
    fn mean_reversion_above_kappa_max_returns_validation_error() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();

        // Exactly at the threshold: must succeed.
        let mut tree_at_limit = RatesCreditTree::new(RatesCreditConfig {
            steps: 20,
            rate_vol: 0.01,
            hazard_vol: 0.20,
            rate_mean_reversion: KAPPA_MAX,
            ..Default::default()
        });
        calibrate_for_test(&mut tree_at_limit, &disc, &haz, 5.0)
            .expect("kappa == KAPPA_MAX must succeed");

        // Just above the threshold: must fail with Validation.
        let over_rate = KAPPA_MAX + 0.01;
        let mut tree_over_rate = RatesCreditTree::new(RatesCreditConfig {
            steps: 20,
            rate_vol: 0.01,
            hazard_vol: 0.20,
            rate_mean_reversion: over_rate,
            ..Default::default()
        });
        match calibrate_for_test(&mut tree_over_rate, &disc, &haz, 5.0) {
            Err(Error::Validation(msg)) => {
                assert!(
                    msg.contains("rate_mean_reversion"),
                    "error message must name the field; got: {msg}"
                );
                assert!(
                    msg.contains("HullWhiteTree"),
                    "error message must point to HullWhiteTree; got: {msg}"
                );
            }
            other => panic!("expected Validation error for rate κ={over_rate}, got: {other:?}"),
        }

        // Hazard factor guard: just above the threshold.
        let over_hazard = KAPPA_MAX + 0.01;
        let mut tree_over_hazard = RatesCreditTree::new(RatesCreditConfig {
            steps: 20,
            rate_vol: 0.01,
            hazard_vol: 0.20,
            hazard_mean_reversion: over_hazard,
            ..Default::default()
        });
        match calibrate_for_test(&mut tree_over_hazard, &disc, &haz, 5.0) {
            Err(Error::Validation(msg)) => {
                assert!(
                    msg.contains("hazard_mean_reversion"),
                    "error message must name the field; got: {msg}"
                );
                assert!(
                    msg.contains("HullWhiteTree"),
                    "error message must point to HullWhiteTree; got: {msg}"
                );
            }
            other => panic!("expected Validation error for hazard κ={over_hazard}, got: {other:?}"),
        }
    }

    /// The moment-matched up-probability is dimensionally coherent and reduces
    /// to `½` at zero mean reversion / at the reference level.
    #[test]
    fn mean_reverting_up_prob_is_coherent() {
        let sigma = 0.01_f64;
        let dt = 0.05_f64;
        let kappa = 0.10_f64;
        let r_ref = 0.03_f64;

        // At the reference level the drift is zero -> p = 1/2.
        let p_at_ref = RatesCreditTree::mean_reverting_up_prob(r_ref, r_ref, kappa, sigma, dt);
        assert!((p_at_ref - 0.5).abs() < 1e-15, "p at ref should be 1/2");

        // Zero mean reversion -> p = 1/2 everywhere.
        let p_no_mr = RatesCreditTree::mean_reverting_up_prob(0.07, r_ref, 0.0, sigma, dt);
        assert!((p_no_mr - 0.5).abs() < 1e-15, "p without MR should be 1/2");

        // Above the reference level the drift is negative -> p < 1/2 (pull
        // down); below -> p > 1/2 (pull up). Symmetric about 1/2.
        let p_high = RatesCreditTree::mean_reverting_up_prob(r_ref + 0.02, r_ref, kappa, sigma, dt);
        let p_low = RatesCreditTree::mean_reverting_up_prob(r_ref - 0.02, r_ref, kappa, sigma, dt);
        assert!(
            p_high < 0.5 && p_low > 0.5,
            "mean reversion must pull toward ref"
        );
        assert!(
            ((p_high - 0.5) + (p_low - 0.5)).abs() < 1e-15,
            "probability must be symmetric about the reference level",
        );

        // Magnitude check: p = 1/2 + mu*sqrt(dt)/(2*sigma), mu = -kappa*(r-r_ref),
        // all in absolute rate units. With the inputs above:
        //   mu = -0.10 * 0.02 = -0.002 (rate/yr)
        //   p  = 0.5 + (-0.002)*sqrt(0.05)/(2*0.01) = 0.5 - 0.0223607...
        let expected = 0.5 + (-kappa * 0.02) * dt.sqrt() / (2.0 * sigma);
        assert!(
            (p_high - expected).abs() < 1e-14,
            "p_high={p_high}, expected={expected}"
        );

        // Always in [0, 1], even for an extreme node.
        let p_extreme =
            RatesCreditTree::mean_reverting_up_prob(r_ref + 100.0, r_ref, kappa, sigma, dt);
        assert!(
            (0.0..=1.0).contains(&p_extreme),
            "probability must stay in [0,1]"
        );
    }
}

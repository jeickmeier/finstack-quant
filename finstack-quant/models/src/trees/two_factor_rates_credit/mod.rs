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
//! Option-adjusted spread is passed to pricing as `oas_bp` (basis points) and
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
//! [`Error::Validation`]: finstack_quant_core::Error::Validation
//! [`HullWhiteTree`]: super::hull_white_tree::HullWhiteTree

pub use band_table::BandTable;

mod band_table;
mod calibration;
mod pricing;
mod sampling;
#[cfg(test)]
mod tests;
mod tree;

pub(crate) use tree::RatesCreditTransition;
pub use tree::{
    HazardFloorSaturation, KAPPA_MAX, NodeCoupon, RatesCreditCalibrationTargets, RatesCreditConfig,
    RatesCreditPathCheckpoint, RatesCreditPathState, RatesCreditTree, VarianceRetention,
};

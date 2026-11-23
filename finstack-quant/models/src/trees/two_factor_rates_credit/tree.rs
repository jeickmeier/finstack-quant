//! Core [`RatesCreditTree`] types, grid structures, and accessors.

use super::band_table::BandTable;
use finstack_quant_cashflows::builder::FloatingRateParams;
use finstack_quant_core::{Error, Result};

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
/// [`HullWhiteTree`]: crate::trees::hull_white_tree::HullWhiteTree
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
pub(crate) struct RatesCreditTransition {
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
/// [`HullWhiteTree`]: crate::trees::hull_white_tree::HullWhiteTree
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
    pub(super) config: RatesCreditConfig,
    /// Calibrated short-rate affine rows. Populated by `calibrate()`.
    pub(super) calibrated_rates: Vec<FactorRow>,
    /// Calibrated hazard-rate affine rows. Populated by `calibrate()`.
    pub(super) calibrated_hazards: Vec<FactorRow>,
    /// Recovery target supplied to `calibrate()`.
    pub(super) recovery_rate: f64,
    /// Mean-reversion reference level for the rate factor (the calibrated
    /// `t = 0` instantaneous rate `r₀`). Populated by `calibrate()`. Pricing
    /// reverts the rate factor toward this level, matching the level used
    /// during calibration so the tree reprices the discount curve.
    pub(super) rate_ref: f64,
    /// Mean-reversion reference level for the hazard factor (the calibrated
    /// `t = 0` instantaneous hazard `h₀`). Populated by `calibrate()`.
    pub(super) hazard_ref: f64,
    /// Hazard floor-saturation diagnostic from the most recent `calibrate()`.
    pub(super) hazard_floor_saturation: HazardFloorSaturation,
    /// Explicit calibration coordinates. Empty until calibration succeeds.
    pub(super) calibration_times: Vec<f64>,
    /// One-step rate terms of the nodes sampled paths usually visit.
    pub(super) rate_terms: BandTable<NodeTerms>,
    /// One-step hazard terms of the nodes sampled paths usually visit.
    pub(super) hazard_terms: BandTable<NodeTerms>,
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
/// [`HullWhiteTree`](crate::trees::hull_white_tree::HullWhiteTree).
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
/// ([`calculate_floating_rate`](finstack_quant_cashflows::builder::calculate_floating_rate): index floor/cap, gearing, spread, all-in
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
    pub(super) fn validate(&self, steps: usize) -> Result<()> {
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
pub(super) struct FactorRow {
    pub(super) base: f64,
    pub(super) shift: f64,
    pub(super) nodes: usize,
}

impl FactorRow {
    #[inline]
    pub(super) fn value(self, node: usize) -> Option<f64> {
        (node < self.nodes).then(|| self.value_unchecked(node))
    }

    #[inline]
    pub(super) fn value_unchecked(self, node: usize) -> f64 {
        debug_assert!(node < self.nodes);
        self.base + node as f64 * self.shift
    }

    #[inline]
    pub(super) fn values(self) -> impl ExactSizeIterator<Item = f64> {
        (0..self.nodes).map(move |node| self.value_unchecked(node))
    }
}

/// One-step terms of one factor node, as sampling consumes them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NodeTerms {
    /// Short rate, or the non-negative effective hazard.
    pub(super) level: f64,
    /// Marginal up-probability of the factor from this node.
    pub(super) up_prob: f64,
    /// `exp(-level * dt)` to the next slice; `1` on the terminal slice.
    pub(super) decay_to_next: f64,
    /// `1 - decay_to_next`, accurate for small exponents; `0` on the terminal
    /// slice.
    pub(super) complement_to_next: f64,
}

/// Nodes of row `step` that sampled paths usually visit.
///
/// A daily lattice has about `steps² / 2` nodes per factor, but a binomial walk
/// stays near the centre of each row, so sampling a path spends almost all its
/// time in a band four standard deviations of a driftless walk (`√step / 2`)
/// either side of the centre, plus a small margin. A factor with zero
/// volatility has one level per row, held at node zero.
pub(super) fn usual_band(row: FactorRow, step: usize) -> std::ops::Range<usize> {
    if row.shift == 0.0 {
        return 0..1;
    }
    let half_width = 2 * (step as f64).sqrt().ceil() as usize + 2;
    let centre = step / 2;
    centre.saturating_sub(half_width)..(centre + half_width + 1).min(row.nodes)
}

/// [`NodeTerms`] of every row's [`usual_band`], three transcendental calls per
/// path step turned into a lookup.
pub(super) fn node_terms_table(
    rows: &[FactorRow],
    terms_at: impl Fn(usize, f64) -> NodeTerms,
) -> BandTable<NodeTerms> {
    let bands = rows
        .iter()
        .enumerate()
        .map(|(step, &row)| usual_band(row, step));
    let Ok(table) = BandTable::build::<std::convert::Infallible>(bands, |step, node| {
        Ok(terms_at(step, rows[step].value_unchecked(node)))
    });
    table
}

/// Terms of `node` on a row of `table`; a zero-volatility row holds its one
/// level at node zero.
#[inline]
pub(super) fn held_node_terms(
    table: &BandTable<NodeTerms>,
    row: FactorRow,
    step: usize,
    node: usize,
) -> Option<NodeTerms> {
    table.get(step, if row.shift == 0.0 { 0 } else { node })
}

/// Per-factor lattice geometry and calibration target shared by the rate and
/// hazard passes of [`RatesCreditTree::calibrate`].
#[derive(Debug, Clone, Copy)]
pub(super) struct FactorGrid {
    /// Number of time steps.
    pub(super) steps: usize,
    /// Step size in years.
    pub(super) dt: f64,
    /// Annualised absolute (normal) volatility of this factor.
    pub(super) sigma: f64,
    /// Whether node values are passed through the non-negative transform.
    /// Set for the hazard factor; the rate factor allows negative rates.
    pub(super) floor_at_zero: bool,
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
            calibration_times: Vec::new(),
            rate_terms: BandTable::default(),
            hazard_terms: BandTable::default(),
        }
    }

    /// Return the immutable configuration used to construct and calibrate the tree.
    ///
    /// Construct and calibrate a new tree to change model parameters. Keeping
    /// this configuration fixed ensures pricing and sampling use the same
    /// transition law that fitted the discount and survival curves.
    ///
    /// ```compile_fail
    /// use finstack_quant_models::trees::two_factor_rates_credit::{RatesCreditConfig, RatesCreditTree};
    /// let mut tree = RatesCreditTree::new(RatesCreditConfig::default());
    /// tree.config.rate_mean_reversion = 0.15;
    /// ```
    ///
    /// ```compile_fail
    /// use finstack_quant_models::trees::two_factor_rates_credit::{RatesCreditConfig, RatesCreditTree};
    /// let tree = RatesCreditTree::new(RatesCreditConfig::default());
    /// tree.get_config().correlation = 1.0;
    /// ```
    pub fn get_config(&self) -> &RatesCreditConfig {
        &self.config
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
        self.variance_retention(
            &self.calibrated_rates,
            self.rate_ref,
            self.config.rate_mean_reversion,
            self.config.rate_vol,
        )
    }

    /// Hazard-factor conditional-variance retention from the most recent
    /// `calibrate()`.
    ///
    /// See [`VarianceRetention`]. Returns the undistorted default for an
    /// uncalibrated tree or one without hazard mean reversion.
    pub fn hazard_variance_retention(&self) -> VarianceRetention {
        self.variance_retention(
            &self.calibrated_hazards,
            self.hazard_ref,
            self.config.hazard_mean_reversion,
            self.config.hazard_vol,
        )
    }

    /// Scan one calibrated factor lattice for its variance retention; the
    /// undistorted default for an uncalibrated tree.
    pub(super) fn variance_retention(
        &self,
        levels: &[FactorRow],
        reference: f64,
        kappa: f64,
        sigma: f64,
    ) -> VarianceRetention {
        let Ok(dt) = self.calibrated_dt() else {
            return VarianceRetention::default();
        };
        Self::scan_variance_retention(levels, self.config.steps, reference, kappa, sigma, dt)
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
                Error::Internal(format!(
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
                Error::Internal(format!(
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
            return Err(Error::Internal(
                "rates-credit tree must be calibrated before reading its time grid".into(),
            ));
        }
        Ok(&self.calibration_times)
    }

    /// Return the calibrated uniform step size.
    pub(super) fn calibrated_dt(&self) -> Result<f64> {
        let times = self.time_grid()?;
        Ok(times[1] - times[0])
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
}

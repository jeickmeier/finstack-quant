//! Campisi-style benchmark-relative fixed-income attribution.
//!
//! # Single-period decomposition
//!
//! Each position's period return is decomposed (Campisi 2000) into:
//!
//! ```text
//! carry_j     = yield_annual_j × Δt                    // income effect
//! treasury_j  = −MD_j × Δy_tsy,j                       // duration / curve effect
//! spread_j    = −SD_j × Δs_j                           // spread effect
//! selection_j = r_j − carry_j − treasury_j − spread_j  // residual
//! ```
//!
//! `Δs_j` ([`FiPositionSnapshot::delta_spread`]) is the *absolute* spread
//! change. Supplying a relative change `Δs / s` instead would overstate the
//! spread effect by a factor of `1 / s` and dump the difference into selection.
//!
//! # Why there is no DTS spread mode
//!
//! Duration-Times-Spread (Ben Dor et al. 2007) re-expresses credit exposure as
//! the product `D · s` against a *relative* spread change. Under that
//! convention the three quantities of interest read:
//!
//! ```text
//!               absolute                DTS
//! return        R = −D · Δs             R = −(D · s) · (Δs / s)
//! volatility    σ_R ≈ D · σ_absolute    σ_R ≈ (D · s) · σ_relative
//! hedge ratio   H = D₁ / D₂             H = (D₁ · s₁) / (D₂ · s₂)
//! ```
//!
//! (Barclays QPS, "Managing Credit Exposure of CDS Portfolios: Adjusting DTS
//! for Market Beta", 23 Jan 2024 — the "paradigm change from absolute to
//! relative spread changes" slide.)
//!
//! The **return** row is an algebraic identity: `−(D · s)(Δs / s) ≡ −D · Δs`.
//! DTS earns its keep only in the volatility and hedge-ratio rows, where
//! `D · s` is a standalone risk quantity multiplied by a relative spread
//! volatility that is empirically far more stable across issuers and rating
//! bands than an absolute one. Ex-post attribution is handed a *realized* `Δs`,
//! so there is no volatility to model and no hedge to size — both conventions
//! reduce to the same number. A mode switch here could therefore only ever
//! relabel an identical result, so this module exposes one spread convention.
//! DTS belongs in risk and hedging surfaces, not in this decomposition.
//!
//! A corollary: the spread *level* [`FiPositionSnapshot::spread`] never enters
//! the arithmetic — nothing divides by it — so zero and negative spread levels
//! (including negative Z-spreads on rich bonds) are accepted as ordinary inputs
//! rather than rejected.
//!
//! # Benchmark-relative sector layer
//!
//! Positions are bucketed by `sector`. With `w_p,i`/`w_b,i` the sector
//! weights, `r_p,i`/`r_b,i` the weighted sector returns and `c_p,i`/`c_b,i`
//! the weighted sector component rates (carry, treasury, spread, selection):
//!
//! ```text
//! Allocation_i      = (w_p,i − w_b,i) · (r_b,i − r_b)          // Brinson-Fachler
//! ActiveCarry_i     = w_p,i · (carry_p,i − carry_b,i)
//! ActiveTreasury_i  = w_p,i · (treasury_p,i − treasury_b,i)
//! ActiveSpread_i    = w_p,i · (spread_p,i − spread_b,i)
//! Selection_i       = w_p,i · (selection_p,i − selection_b,i)
//! ```
//!
//! Because component rates sum to the sector return on each side, the five
//! effects telescope to the active return exactly (to floating-point
//! precision), given that weights sum to 1 on each side and that every present
//! sector's rate `r_·,i` is well-conditioned — its net weight not itself
//! vanishingly small relative to its gross weight (see the internal
//! `check_net_weight` guard):
//!
//! ```text
//! Σ_i [Alloc_i + w_p,i (r_p,i − r_b,i)]
//!   = Σ_i w_p,i r_p,i − Σ_i w_b,i r_b,i − r_b Σ_i (w_p,i − w_b,i)
//!   = r_p − r_b                        (weights sum to 1 on each side)
//! ```
//!
//! The cancellation of the `Σ_i w_p,i r_b,i` terms between `Alloc_i` and the
//! within-sector effects is what makes this exact, and it is also where a
//! near-cancelling sector does its damage: a sector netting *close to*, but
//! not exactly, zero has a numerically explosive rather than merely undefined
//! rate, so those two terms both grow without bound and their difference loses
//! precision proportionally. That is why the guard uses a relative bound
//! rather than an exact-equality test. Even among inputs that pass it, the
//! identity holds to floating-point precision, not to an arbitrarily tight
//! tolerance: a sector whose net weight sits just above the internal
//! `NET_WEIGHT_RELATIVE_TOLERANCE` bound is legal but still amplifies rounding
//! noise, so the reconstructed sum can differ from `active_return` by more
//! than the tightest tolerances asserted in this module's own tests (which use
//! golden fixtures with well-separated net weights, not adversarial
//! near-cancellation).
//!
//! This is the two-way Brinson-Fachler form (interaction folded into the
//! within-sector effects at portfolio weight), which keeps the component
//! split exact; the three-way split used in [`crate::brinson`] would need a
//! per-component interaction bucket.
//!
//! ## Off-benchmark sectors
//!
//! A sector held in the portfolio but absent from the benchmark has no
//! observable `r_b,i`. This module adopts the industry convention
//! `r_b,i := R_b` (the benchmark *total* return; componentwise the
//! benchmark-wide component returns, which sum to `R_b`): the allocation
//! term for such a sector is then identically zero —
//! `(w_p,i − 0)(R_b − R_b) = 0` — and its entire active contribution flows
//! through the four component effects. The naive alternative `r_b,i := 0`
//! charges `−w_p,i · R_b` of allocation for merely holding the sector, an
//! arbitrary penalty scaled by the benchmark's overall return level rather
//! than by any bet the manager took. The telescoping identity above is
//! insensitive to the choice: every `r_b,i` in the summed effects carries the
//! coefficient `−w_b,i`, which is zero for exactly these sectors, so the
//! substitution moves value between allocation and the component terms
//! without changing their sum. Symmetrically, a sector absent from the
//! portfolio reports `r_p,i := r_b,i` (reporting-only, since every effect
//! multiplies it by `w_p,i = 0`).
//!
//! # References
//!
//! * Campisi, S. (2000). "Primer on Fixed Income Performance Attribution."
//!   *Journal of Portfolio Management*, 26(4), 14–25. `docs/REFERENCES.md#campisi-2000`
//!
//! * Ben Dor, A., Dynkin, L., Hyman, J., Houweling, P., van Leeuwen, E., &
//!   Penninga, O. (2007). "DTS (Duration Times Spread)." *Journal of
//!   Portfolio Management*, 33(2), 77–100 — source of the DTS convention
//!   deliberately *not* offered here; see "Why there is no DTS spread mode". `docs/REFERENCES.md#ben-dor-2007-dts`
//!
//! * Brinson, G. P., & Fachler, N. (1985). "Measuring Non-US Equity Portfolio
//!   Performance." *Journal of Portfolio Management*, 11(3) — source of the
//!   `(w_p − w_b)(r_b,i − r_b)` allocation form used above. `docs/REFERENCES.md#brinson-fachler-1985`
//!
//! * Carino, D. (1999). "Combining Attribution Effects over Time." *Journal of
//!   Performance Measurement*, 3(4), 5–14 — multi-period smoothing applied by
//!   [`campisi_carino_link`]. `docs/REFERENCES.md#carino-1999`

use crate::brinson::{carino_link_effects, CarinoPeriod};
use crate::error::{Error, Result};
use finstack_quant_core::math::summation::NeumaierAccumulator;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Tolerance for the requirement that weights sum to 1.0 on each side.
const WEIGHT_TOLERANCE: f64 = 1e-6;

/// Relative tolerance on the ratio `|net weight| / gross weight` below which a
/// sector's per-unit rate is treated as too poorly conditioned to attribute,
/// in [`check_net_weight`].
///
/// A sector's rate is `contribution / weight` (see [`SideAgg::rate`]). At exact
/// cancellation (`weight == 0.0` with `abs_weight > 0.0`) that rate is
/// undefined; arbitrarily close to exact cancellation it is *defined* but
/// numerically explosive — as `weight -> 0` for a roughly fixed contribution,
/// the rate grows without bound, and with it the Brinson-Fachler allocation
/// `(w_p,i − w_b,i)(r_b,i − r_b)`, whose weight difference does not shrink
/// alongside the exploding rate. An exact-equality-only guard misses this: a
/// benchmark sector of `+0.40 @ 3%` and `-(0.40 - 1e-8) @ 0.5%` nets to
/// `1e-8`, not `0.0`, yet produces an allocation on the order of `3e5` against
/// an active return of `-81 bp`, and drives the telescoping identity's residual
/// to `-3.8e-11` — outside the `1e-12` tolerance this module's own tests
/// assert. Reusing [`WEIGHT_TOLERANCE`]'s existing 1e-6 precision floor for
/// this ratio keeps a single, already-load-bearing precision assumption for the
/// whole module: a net weight smaller than a millionth of its own gross weight
/// is, for the purposes of this module, indistinguishable from exact
/// cancellation.
const NET_WEIGHT_RELATIVE_TOLERANCE: f64 = 1e-6;

/// Relative reconciliation tolerance for inbound linked-period effects.
///
/// The floor is `1e-10` in ordinary return space. For near-cancelling,
/// long/short-generated effects whose gross magnitude is much larger than
/// their net active return, it scales with an overflow-safe L1 sector-effect
/// norm so valid outputs from [`campisi_attribution`] are not rejected solely
/// because cancellation amplified floating-point noise.
const LINK_RECONCILIATION_RELATIVE_TOLERANCE: f64 = 1e-10;

/// Streaming L1 scale that avoids summing absolute values at their original
/// magnitude.
#[derive(Default)]
struct ScaledL1Norm {
    scale: f64,
    normalized_sum: f64,
}

impl ScaledL1Norm {
    fn add(&mut self, value: f64) {
        let magnitude = value.abs();
        if magnitude > self.scale {
            self.normalized_sum = if self.scale == 0.0 {
                1.0
            } else {
                self.normalized_sum * (self.scale / magnitude) + 1.0
            };
            self.scale = magnitude;
        } else if self.scale > 0.0 {
            self.normalized_sum += magnitude / self.scale;
        }
    }

    fn tolerance(&self) -> f64 {
        if self.scale == 0.0 {
            return LINK_RECONCILIATION_RELATIVE_TOLERANCE;
        }
        let scaled_relative = LINK_RECONCILIATION_RELATIVE_TOLERANCE * self.scale;
        if self.normalized_sum > f64::MAX / scaled_relative {
            f64::MAX
        } else {
            (scaled_relative * self.normalized_sum).max(LINK_RECONCILIATION_RELATIVE_TOLERANCE)
        }
    }
}

/// Configuration for [`campisi_attribution`].
///
/// Deliberately a single field: the spread convention is not configurable, for
/// the reason set out under "Why there is no DTS spread mode" in the module
/// docs. `period_years` has no serde default — an omitted key fails closed
/// rather than silently assuming a period length.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiAttributionConfig {
    /// Length of the attribution period in years (e.g. `0.25` for a
    /// quarter). Scales `yield_annual` into the period carry.
    pub period_years: f64,
}

impl FiAttributionConfig {
    /// Create a config for the given period length.
    ///
    /// # Arguments
    ///
    /// * `period_years` - Attribution period length in years; must be finite
    ///   and positive.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_portfolio::fi_attribution::FiAttributionConfig;
    ///
    /// let config = FiAttributionConfig::new(0.25);
    /// assert_eq!(config.period_years, 0.25);
    /// ```
    pub fn new(period_years: f64) -> Self {
        Self { period_years }
    }
}

/// Plain-data snapshot of one position (or pre-aggregated bucket) for one
/// attribution period.
///
/// Weights are fractions of the whole portfolio (or benchmark) and must sum
/// to 1.0 within each side. Returns, yields and spreads are decimals
/// (`0.02` = 2 %); durations are in years.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiPositionSnapshot {
    /// Sector bucket label (industry / quality / any grouping).
    pub sector: String,
    /// Weight as a fraction of the whole side at period start.
    pub weight: f64,
    /// Realized total return for the period (decimal).
    pub total_return: f64,
    /// Annualized yield at period start (decimal; e.g. YTM).
    pub yield_annual: f64,
    /// Modified duration in years at period start.
    pub modified_duration: f64,
    /// Quote-reproducing Z-spread duration in years at period start.
    ///
    /// Must use the same Z-spread basis as [`Self::spread`] and
    /// [`Self::delta_spread`]; OAS, G-spread, and discount-margin durations
    /// are not compatible inputs.
    pub spread_duration: f64,
    /// Quote-reproducing Z-spread at period start (decimal).
    ///
    /// Carried for provenance and downstream reporting only — the
    /// decomposition never divides by it, so any finite value (including zero
    /// and negative levels) is accepted. See "Why there is no DTS spread mode"
    /// in the module docs.
    pub spread: f64,
    /// Change in the treasury/benchmark yield relevant to this position's
    /// duration bucket over the period (decimal).
    pub delta_treasury_yield: f64,
    /// Absolute change in the quote-reproducing Z-spread over the period
    /// (decimal).
    ///
    /// Must use the same Z-spread basis as [`Self::spread_duration`] and
    /// [`Self::spread`].
    pub delta_spread: f64,
}

/// Absolute Campisi component contributions for one side (portfolio or
/// benchmark), each `Σ_j w_j × component_j`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiComponents {
    /// Income effect `Σ w · y · Δt`.
    pub carry: f64,
    /// Treasury/duration effect `Σ w · (−MD · Δy)`.
    pub treasury: f64,
    /// Spread effect `Σ w · (−SD · Δs)`.
    pub spread: f64,
    /// Residual selection `Σ w · (r − explained)`.
    pub selection: f64,
    /// Sum of the four components — equals the side's total return.
    pub total: f64,
}

/// Per-sector benchmark-relative effects.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiSectorEffect {
    /// Sector label (mirrors [`FiPositionSnapshot::sector`]).
    pub sector: String,
    /// Portfolio weight in the sector.
    pub portfolio_weight: f64,
    /// Benchmark weight in the sector.
    pub benchmark_weight: f64,
    /// Portfolio sector return (weighted, per unit of sector weight).
    ///
    /// For a sector absent from the portfolio this mirrors
    /// [`Self::benchmark_return`] (`r_p,i := r_b,i`) — reporting-only, since
    /// every effect multiplies it by the zero portfolio weight.
    pub portfolio_return: f64,
    /// Benchmark sector return (weighted, per unit of sector weight).
    ///
    /// For a sector absent from the benchmark this is the benchmark *total*
    /// return `R_b` (the off-benchmark convention; see "Off-benchmark
    /// sectors" in the module docs), so [`Self::allocation`] is identically
    /// zero for such sectors and their active contribution flows through the
    /// component effects instead.
    pub benchmark_return: f64,
    /// Brinson-Fachler allocation `(w_p − w_b)(r_b,i − r_b)`.
    pub allocation: f64,
    /// Active income effect `w_p (carry_p,i − carry_b,i)`.
    pub active_carry: f64,
    /// Active duration positioning `w_p (treasury_p,i − treasury_b,i)`.
    pub active_treasury: f64,
    /// Active spread positioning `w_p (spread_p,i − spread_b,i)`.
    pub active_spread: f64,
    /// Security selection residual `w_p (selection_p,i − selection_b,i)`.
    pub selection: f64,
    /// Sum of the five effects — the sector's contribution to active return.
    pub total_active: f64,
}

/// Single-period Campisi benchmark-relative attribution result.
///
/// This type is *input-reachable*: [`campisi_carino_link`] consumes a slice of
/// these, and the Python/WASM bindings deserialize them from JSON. It therefore
/// denies unknown fields like the other inbound types in this module, so a
/// misspelled or stale key fails closed instead of being silently dropped.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiAttributionResult {
    /// Per-sector effects, in first-seen order (portfolio first, then
    /// benchmark-only sectors).
    pub sectors: Vec<FiSectorEffect>,
    /// Portfolio-side absolute Campisi split.
    pub portfolio_components: FiComponents,
    /// Benchmark-side absolute Campisi split.
    pub benchmark_components: FiComponents,
    /// Portfolio total return `Σ w_p,j r_p,j`.
    pub portfolio_return: f64,
    /// Benchmark total return `Σ w_b,j r_b,j`.
    pub benchmark_return: f64,
    /// Active return `portfolio_return − benchmark_return`.
    pub active_return: f64,
    /// Sum of sector allocation effects.
    pub total_allocation: f64,
    /// Sum of sector active carry effects.
    pub total_active_carry: f64,
    /// Sum of sector active treasury effects.
    pub total_active_treasury: f64,
    /// Sum of sector active spread effects.
    pub total_active_spread: f64,
    /// Sum of sector selection effects.
    pub total_selection: f64,
}

/// Report from reconciling the five effect totals against the active return,
/// mirroring [`crate::attribution`] reconciliation conventions.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiReconciliationReport {
    /// `active_return − (allocation + carry + treasury + spread + selection)`.
    pub total_residual: f64,
    /// Whether the residual is within tolerance.
    pub is_reconciled: bool,
    /// Tolerance used for the check.
    pub tolerance: f64,
}

impl FiAttributionResult {
    /// Check that the five effect totals reconstruct the active return.
    ///
    /// The decomposition reconciles by construction (the selection component
    /// is a residual), so this is a floating-point sanity gate, not a model
    /// check; `1e-10` is an appropriate tolerance for return-space values.
    ///
    /// It is not a substitute for [`campisi_attribution`]'s near-zero-net-weight
    /// input guard, and cannot stand in for it: a sector netting to a
    /// vanishingly small weight inflates the per-sector effects by orders of
    /// magnitude while its residual — the cancellation of two exploded terms —
    /// stays small enough to pass this gate.
    ///
    /// # Arguments
    ///
    /// * `tolerance` - Absolute tolerance in return units.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_portfolio::fi_attribution::{
    ///     campisi_attribution, FiAttributionConfig, FiPositionSnapshot,
    /// };
    ///
    /// let bucket = |sector: &str, weight: f64, r: f64, y: f64, md: f64| FiPositionSnapshot {
    ///     sector: sector.into(),
    ///     weight,
    ///     total_return: r,
    ///     yield_annual: y,
    ///     modified_duration: md,
    ///     spread_duration: 0.0,
    ///     spread: 0.0,
    ///     delta_treasury_yield: -0.001,
    ///     delta_spread: 0.0,
    /// };
    /// let portfolio = vec![bucket("GOVT", 1.0, 0.016, 0.04, 5.0)];
    /// let benchmark = vec![bucket("GOVT", 1.0, 0.015, 0.04, 5.5)];
    /// let result = campisi_attribution(&portfolio, &benchmark, &FiAttributionConfig::new(0.25))?;
    /// let report = result.reconciliation_check(1e-10);
    /// assert!(report.is_reconciled);
    /// # Ok::<(), finstack_quant_portfolio::Error>(())
    /// ```
    pub fn reconciliation_check(&self, tolerance: f64) -> FiReconciliationReport {
        let mut acc = NeumaierAccumulator::new();
        acc.add(self.total_allocation);
        acc.add(self.total_active_carry);
        acc.add(self.total_active_treasury);
        acc.add(self.total_active_spread);
        acc.add(self.total_selection);
        let total_residual = self.active_return - acc.total();
        FiReconciliationReport {
            total_residual,
            is_reconciled: total_residual.abs() <= tolerance,
            tolerance,
        }
    }
}

/// Per-position Campisi components (carry, treasury, spread) in return space.
///
/// Infallible: every input has already been checked for finiteness by
/// [`validate_snapshot`], and none of the three expressions divides by an
/// input. In particular the spread effect is linear in `delta_spread` and does
/// not touch the spread *level*, so `spread <= 0` needs no guard here.
fn position_components(s: &FiPositionSnapshot, config: &FiAttributionConfig) -> (f64, f64, f64) {
    let carry = s.yield_annual * config.period_years;
    let treasury = -s.modified_duration * s.delta_treasury_yield;
    let spread = -s.spread_duration * s.delta_spread;
    (carry, treasury, spread)
}

/// Validate finiteness of every numeric field of a snapshot.
fn validate_snapshot(s: &FiPositionSnapshot, side: &str) -> Result<()> {
    for (name, value) in [
        ("weight", s.weight),
        ("total_return", s.total_return),
        ("yield_annual", s.yield_annual),
        ("modified_duration", s.modified_duration),
        ("spread_duration", s.spread_duration),
        ("spread", s.spread),
        ("delta_treasury_yield", s.delta_treasury_yield),
        ("delta_spread", s.delta_spread),
    ] {
        if !value.is_finite() {
            return Err(Error::invalid_input(format!(
                "Campisi {side} input '{name}' for sector '{}' must be finite (got {value})",
                s.sector
            )));
        }
    }
    Ok(())
}

/// Weighted per-sector accumulators for one side.
///
/// `weight` is the *net* sector weight (long minus short) and `abs_weight` the
/// gross weight; the pair distinguishes "sector absent from this side"
/// (`abs_weight == 0`) from "sector present with offsetting positions"
/// (`abs_weight > 0`, `weight` zero or, per [`NET_WEIGHT_RELATIVE_TOLERANCE`],
/// numerically near zero relative to `abs_weight`), which [`check_net_weight`]
/// rejects.
#[derive(Clone, Copy, Default)]
struct SideAgg {
    weight: f64,
    abs_weight: f64,
    ret: f64,
    carry: f64,
    treasury: f64,
    spread: f64,
    selection: f64,
}

impl SideAgg {
    /// Sector rate: weighted contribution ÷ net sector weight.
    ///
    /// Returns `0.0` only when the sector is absent from this side, i.e. every
    /// contribution is likewise `0.0`. Sectors present with a zero — or
    /// numerically near-zero — net weight are rejected by [`check_net_weight`]
    /// before this is called, so the guard never silently discards a real
    /// contribution, and never divides through a weight so small that the
    /// resulting rate is numerically meaningless. The `.abs()`
    /// is load-bearing: net-short sectors (`weight < 0`) are legal and must
    /// take the division branch.
    fn rate(&self, contribution: f64) -> f64 {
        if self.weight.abs() > 0.0 {
            contribution / self.weight
        } else {
            0.0
        }
    }
}

/// Fail closed on a sector that is present on a side but nets to zero, or to a
/// weight that is numerically near zero *relative to its own gross weight* (a
/// long/short pair, a CDS hedge against a cash bond in the same bucket, a
/// fully- or nearly-fully-hedged sector).
///
/// Such a sector still contributes `Σ_j w_j r_j ≠ 0` to the side total. At
/// exact cancellation its per-unit rate `contribution / weight` is undefined
/// (`0 / 0`), so every per-sector effect would be forced to zero while the
/// contribution stayed in the side return. At near-cancellation the rate is
/// *defined* but grows without bound as the net weight shrinks, so the
/// allocation effect built from it can blow up to a numerically meaningless
/// magnitude — and the telescoping identity's residual with it — while
/// `active_return` still ties out against performance data and no `NaN` or
/// infinity ever appears for a finiteness check to catch. An exact-equality
/// check (`agg.weight == 0.0`) misses that regime entirely, so this compares
/// the ratio `|weight| / abs_weight` against [`NET_WEIGHT_RELATIVE_TOLERANCE`]
/// instead — a relative bound, so rescaling all weights uniformly (percent vs.
/// decimal) does not change whether it fires.
fn check_net_weight(sector: &str, agg: &SideAgg, side_name: &str) -> Result<()> {
    if agg.abs_weight > 0.0 && agg.weight.abs() <= NET_WEIGHT_RELATIVE_TOLERANCE * agg.abs_weight {
        return Err(Error::invalid_input(format!(
            "{side_name} sector '{sector}' has offsetting positions netting to a weight \
             ({}) that is zero, or numerically near zero, relative to its gross weight \
             ({}): a sector whose |net weight| does not exceed \
             {NET_WEIGHT_RELATIVE_TOLERANCE} times its gross weight cannot be attributed \
             because its per-unit rate contribution / weight is undefined or numerically \
             explosive. Split the offsetting positions into distinct sectors, or net them \
             into a single snapshot with a net weight well clear of that relative bound.",
            agg.weight, agg.abs_weight
        )));
    }
    Ok(())
}

/// Accumulate one side into per-sector aggregates and side totals.
///
/// Returns `(side_return, side_components)` and fills `sectors` (union map,
/// first-seen order preserved).
fn aggregate_side(
    snapshots: &[FiPositionSnapshot],
    config: &FiAttributionConfig,
    sectors: &mut IndexMap<String, (SideAgg, SideAgg)>,
    is_portfolio: bool,
) -> Result<(f64, FiComponents)> {
    let (side, side_name) = if is_portfolio {
        ("portfolio", "Portfolio")
    } else {
        ("benchmark", "Benchmark")
    };
    let mut sum_w = NeumaierAccumulator::new();
    let mut sum_r = NeumaierAccumulator::new();
    let mut sum_carry = NeumaierAccumulator::new();
    let mut sum_tsy = NeumaierAccumulator::new();
    let mut sum_spr = NeumaierAccumulator::new();
    let mut sum_sel = NeumaierAccumulator::new();

    for s in snapshots {
        validate_snapshot(s, side)?;
        let (carry, treasury, spread) = position_components(s, config);
        let selection = s.total_return - carry - treasury - spread;

        sum_w.add(s.weight);
        sum_r.add(s.weight * s.total_return);
        sum_carry.add(s.weight * carry);
        sum_tsy.add(s.weight * treasury);
        sum_spr.add(s.weight * spread);
        sum_sel.add(s.weight * selection);

        let entry = sectors.entry(s.sector.clone()).or_default();
        let agg = if is_portfolio {
            &mut entry.0
        } else {
            &mut entry.1
        };
        agg.weight += s.weight;
        agg.abs_weight += s.weight.abs();
        agg.ret += s.weight * s.total_return;
        agg.carry += s.weight * carry;
        agg.treasury += s.weight * treasury;
        agg.spread += s.weight * spread;
        agg.selection += s.weight * selection;
    }

    let total_w = sum_w.total();
    if (total_w - 1.0).abs() > WEIGHT_TOLERANCE {
        return Err(Error::invalid_input(format!(
            "{side_name} weights must sum to 1.0 (got {total_w})"
        )));
    }

    let ret = sum_r.total();
    let carry = sum_carry.total();
    let treasury = sum_tsy.total();
    let spread = sum_spr.total();
    let selection = sum_sel.total();
    Ok((
        ret,
        FiComponents {
            carry,
            treasury,
            spread,
            selection,
            total: carry + treasury + spread + selection,
        },
    ))
}

/// Compute a single-period Campisi benchmark-relative attribution.
///
/// Decomposes each side's return into carry, treasury, spread and selection
/// (Campisi 2000), then buckets by sector and splits the active return into
/// allocation plus four active component effects using Brinson-Fachler sign
/// conventions (see the module docs for the exact formulas and the
/// reconciliation proof).
///
/// A sector missing from one side is treated with zero weight on that side,
/// so the decomposition stays complete. A sector absent from the *benchmark*
/// uses the off-benchmark convention `r_b,i := R_b` (componentwise, the
/// benchmark-wide component returns): its allocation effect is identically
/// zero and its full active contribution appears in the component effects —
/// see "Off-benchmark sectors" in the module docs. A sector absent from the
/// *portfolio* reports `r_p,i := r_b,i` (reporting-only). A sector that is
/// *present* on a side
/// but whose positions net to zero — or to a weight smaller than `1e-6` of its
/// own gross weight (a long/short pair, a CDS hedge against a cash bond in the
/// same bucket, exactly or nearly offsetting) — has a per-unit rate that is
/// undefined or numerically explosive, and is rejected; see the errors below.
///
/// # Arguments
///
/// * `portfolio` - Portfolio position/bucket snapshots; weights must sum to 1.
/// * `benchmark` - Benchmark snapshots; weights must sum to 1.
/// * `config` - Attribution period length in years, used to convert annual
///   yields into the period's carry return; must be finite and positive.
///
/// # Errors
///
/// * [`Error::InvalidInput`] if either side is empty, any value is
///   non-finite, weights don't sum to 1.0 (±1e-6), `period_years` is not
///   finite and positive, or a sector has offsetting positions netting to a
///   weight that is zero, or no larger than `1e-6` times its own gross
///   weight, on either side.
///
/// The spread *level* is unconstrained: zero and negative spreads are accepted,
/// because the spread effect `−SD · Δs` never divides by it.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_portfolio::fi_attribution::{
///     campisi_attribution, FiAttributionConfig, FiPositionSnapshot,
/// };
///
/// let bucket = |sector: &str, weight: f64, r: f64, y: f64, md: f64| FiPositionSnapshot {
///     sector: sector.into(),
///     weight,
///     total_return: r,
///     yield_annual: y,
///     modified_duration: md,
///     spread_duration: 0.0,
///     spread: 0.0,
///     delta_treasury_yield: -0.001,
///     delta_spread: 0.0,
/// };
/// let portfolio = vec![bucket("GOVT", 0.6, 0.016, 0.04, 5.0), bucket("CORP", 0.4, 0.012, 0.06, 4.0)];
/// let benchmark = vec![bucket("GOVT", 0.5, 0.015, 0.04, 5.5), bucket("CORP", 0.5, 0.011, 0.055, 4.5)];
/// let result = campisi_attribution(&portfolio, &benchmark, &FiAttributionConfig::new(0.25))?;
/// assert!(result.reconciliation_check(1e-10).is_reconciled);
/// # Ok::<(), finstack_quant_portfolio::Error>(())
/// ```
pub fn campisi_attribution(
    portfolio: &[FiPositionSnapshot],
    benchmark: &[FiPositionSnapshot],
    config: &FiAttributionConfig,
) -> Result<FiAttributionResult> {
    if !config.period_years.is_finite() || config.period_years <= 0.0 {
        return Err(Error::invalid_input(format!(
            "Campisi config period_years must be finite and positive (got {})",
            config.period_years
        )));
    }
    if portfolio.is_empty() {
        return Err(Error::invalid_input(
            "Campisi attribution requires at least one portfolio snapshot",
        ));
    }
    if benchmark.is_empty() {
        return Err(Error::invalid_input(
            "Campisi attribution requires at least one benchmark snapshot",
        ));
    }

    // Union of sectors in first-seen order: portfolio first, then
    // benchmark-only sectors (deterministic IndexMap ordering).
    let mut sectors: IndexMap<String, (SideAgg, SideAgg)> = IndexMap::new();
    let (portfolio_return, portfolio_components) =
        aggregate_side(portfolio, config, &mut sectors, true)?;
    let (benchmark_return, benchmark_components) =
        aggregate_side(benchmark, config, &mut sectors, false)?;

    // A sector present on a side but netting to zero — or to a weight
    // numerically near zero relative to its gross weight — has a per-unit rate
    // that is undefined or explosive; attributing it would zero its five
    // effects while its contribution stayed in the side return, or blow those
    // effects up past any meaningful magnitude. Fail closed instead.
    for (sector, (p, b)) in &sectors {
        check_net_weight(sector, p, "Portfolio")?;
        check_net_weight(sector, b, "Benchmark")?;
    }

    let mut total_allocation = NeumaierAccumulator::new();
    let mut total_active_carry = NeumaierAccumulator::new();
    let mut total_active_treasury = NeumaierAccumulator::new();
    let mut total_active_spread = NeumaierAccumulator::new();
    let mut total_selection = NeumaierAccumulator::new();
    let mut sector_effects = Vec::with_capacity(sectors.len());

    for (sector, (p, b)) in sectors {
        // Off-benchmark convention (see the module docs): a sector absent
        // from the benchmark inherits the benchmark's *total* rates —
        // `r_b,i := R_b`, componentwise the benchmark-wide component returns,
        // which sum to `R_b` exactly. Its allocation term is then identically
        // zero (`(w_p,i − 0)(R_b − R_b)`), instead of the `−w_p,i · R_b`
        // penalty the naive `r_b,i = 0` convention charges for merely holding
        // an off-benchmark sector, and the whole active effect flows through
        // the component terms. The telescoping identity is untouched: every
        // `r_b,i` term in the summed effects carries the coefficient
        // `−w_b,i`, which is zero for exactly these sectors.
        let benchmark_absent = b.abs_weight == 0.0;
        let (r_b, carry_b, treasury_b, spread_b, selection_b) = if benchmark_absent {
            (
                benchmark_return,
                benchmark_components.carry,
                benchmark_components.treasury,
                benchmark_components.spread,
                benchmark_components.selection,
            )
        } else {
            (
                b.rate(b.ret),
                b.rate(b.carry),
                b.rate(b.treasury),
                b.rate(b.spread),
                b.rate(b.selection),
            )
        };
        // Mirror convention for sectors absent from the portfolio:
        // `r_p,i := r_b,i`. Purely reporting — every effect multiplies the
        // portfolio rate by `w_p,i = 0`.
        let r_p = if p.abs_weight == 0.0 {
            r_b
        } else {
            p.rate(p.ret)
        };
        let allocation = (p.weight - b.weight) * (r_b - benchmark_return);
        let active_carry = p.weight * (p.rate(p.carry) - carry_b);
        let active_treasury = p.weight * (p.rate(p.treasury) - treasury_b);
        let active_spread = p.weight * (p.rate(p.spread) - spread_b);
        let selection = p.weight * (p.rate(p.selection) - selection_b);

        total_allocation.add(allocation);
        total_active_carry.add(active_carry);
        total_active_treasury.add(active_treasury);
        total_active_spread.add(active_spread);
        total_selection.add(selection);

        sector_effects.push(FiSectorEffect {
            sector,
            portfolio_weight: p.weight,
            benchmark_weight: b.weight,
            portfolio_return: r_p,
            benchmark_return: r_b,
            allocation,
            active_carry,
            active_treasury,
            active_spread,
            selection,
            total_active: allocation + active_carry + active_treasury + active_spread + selection,
        });
    }

    Ok(FiAttributionResult {
        sectors: sector_effects,
        portfolio_components,
        benchmark_components,
        portfolio_return,
        benchmark_return,
        active_return: portfolio_return - benchmark_return,
        total_allocation: total_allocation.total(),
        total_active_carry: total_active_carry.total(),
        total_active_treasury: total_active_treasury.total(),
        total_active_spread: total_active_spread.total(),
        total_selection: total_selection.total(),
    })
}

/// One attribution period's raw inputs for multi-period linking.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiPeriodInput {
    /// Portfolio snapshots for the period.
    pub portfolio: Vec<FiPositionSnapshot>,
    /// Benchmark snapshots for the period.
    pub benchmark: Vec<FiPositionSnapshot>,
}

/// Carino-linked per-sector FI effects summed across periods.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct FiLinkedSectorEffect {
    /// Sector label.
    pub sector: String,
    /// Linked allocation effect.
    pub allocation: f64,
    /// Linked active carry effect.
    pub active_carry: f64,
    /// Linked active treasury effect.
    pub active_treasury: f64,
    /// Linked active spread effect.
    pub active_spread: f64,
    /// Linked selection effect.
    pub selection: f64,
    /// Sum of the five linked effects.
    pub total_active: f64,
}

/// Multi-period Carino-linked Campisi attribution.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct FiCarinoLinkedResult {
    /// Per-period single-period results, in chronological order.
    pub periods: Vec<FiAttributionResult>,
    /// Geometrically compounded portfolio return `∏(1 + r_p,t) − 1`.
    pub portfolio_return_compounded: f64,
    /// Geometrically compounded benchmark return.
    pub benchmark_return_compounded: f64,
    /// Per-sector linked effects; their grand total reconstructs the
    /// compounded active return exactly.
    pub linked_sectors: Vec<FiLinkedSectorEffect>,
    /// Sum of linked allocation effects.
    pub linked_allocation: f64,
    /// Sum of linked active carry effects.
    pub linked_active_carry: f64,
    /// Sum of linked active treasury effects.
    pub linked_active_treasury: f64,
    /// Sum of linked active spread effects.
    pub linked_active_spread: f64,
    /// Sum of linked selection effects.
    pub linked_selection: f64,
}

/// Validate one externally reachable single-period result before linking.
fn validate_campisi_link_period(period: &FiAttributionResult, index: usize) -> Result<()> {
    for (name, value) in [
        ("portfolio_return", period.portfolio_return),
        ("benchmark_return", period.benchmark_return),
        ("active_return", period.active_return),
        ("total_allocation", period.total_allocation),
        ("total_active_carry", period.total_active_carry),
        ("total_active_treasury", period.total_active_treasury),
        ("total_active_spread", period.total_active_spread),
        ("total_selection", period.total_selection),
    ] {
        if !value.is_finite() {
            return Err(Error::invalid_input(format!(
                "Campisi Carino period[{index}].{name} must be finite (got {value})"
            )));
        }
    }

    let expected_active = period.portfolio_return - period.benchmark_return;
    if !expected_active.is_finite() {
        return Err(Error::invalid_input(format!(
            "Campisi Carino period[{index}] portfolio_return - benchmark_return must be finite"
        )));
    }
    let return_scale = period
        .portfolio_return
        .abs()
        .max(period.benchmark_return.abs())
        .max(period.active_return.abs())
        .max(1.0);
    let return_tolerance = 1e-12 * return_scale;
    let active_residual = period.active_return - expected_active;
    if !active_residual.is_finite() {
        return Err(Error::invalid_input(format!(
            "Campisi Carino period[{index}] active-return residual must be finite"
        )));
    }
    if active_residual.abs() > return_tolerance {
        return Err(Error::invalid_input(format!(
            "Campisi Carino period[{index}].active_return ({}) does not agree with \
             portfolio_return - benchmark_return ({expected_active}) within return-scale \
             tolerance {return_tolerance}",
            period.active_return
        )));
    }

    const N_EFFECTS: usize = 5;
    let mut sector_totals = [NeumaierAccumulator::new(); N_EFFECTS];
    let mut effect_l1 = ScaledL1Norm::default();
    for (sector_index, sector) in period.sectors.iter().enumerate() {
        if !sector.total_active.is_finite() {
            return Err(Error::invalid_input(format!(
                "Campisi Carino period[{index}].sectors[{sector_index}] '{}' field total_active \
                 must be finite (got {})",
                sector.sector, sector.total_active
            )));
        }
        let mut sector_sum = NeumaierAccumulator::new();
        let mut sector_l1 = ScaledL1Norm::default();
        for (effect_index, (name, value)) in [
            ("allocation", sector.allocation),
            ("active_carry", sector.active_carry),
            ("active_treasury", sector.active_treasury),
            ("active_spread", sector.active_spread),
            ("selection", sector.selection),
        ]
        .into_iter()
        .enumerate()
        {
            if !value.is_finite() {
                return Err(Error::invalid_input(format!(
                    "Campisi Carino period[{index}].sectors[{sector_index}] '{}' field {name} \
                     must be finite (got {value})",
                    sector.sector
                )));
            }
            sector_totals[effect_index].add(value);
            sector_sum.add(value);
            sector_l1.add(value);
            effect_l1.add(value);
        }
        let actual_total = sector_sum.total();
        let sector_residual = actual_total - sector.total_active;
        if !sector_residual.is_finite() {
            return Err(Error::invalid_input(format!(
                "Campisi Carino period[{index}].sectors[{sector_index}] '{}' total_active \
                 reconciliation residual must be finite",
                sector.sector
            )));
        }
        let sector_tolerance = sector_l1.tolerance();
        if sector_residual.abs() > sector_tolerance {
            return Err(Error::invalid_input(format!(
                "Campisi Carino period[{index}].sectors[{sector_index}] '{}' total_active {} \
                 does not reconcile to its five effects sum {actual_total} within scale-aware \
                 tolerance {sector_tolerance}",
                sector.sector, sector.total_active
            )));
        }
    }
    let tolerance = effect_l1.tolerance();

    let declared_totals = [
        ("total_allocation", period.total_allocation),
        ("total_active_carry", period.total_active_carry),
        ("total_active_treasury", period.total_active_treasury),
        ("total_active_spread", period.total_active_spread),
        ("total_selection", period.total_selection),
    ];
    let mut declared_sum = NeumaierAccumulator::new();
    for ((name, declared), sector_total) in declared_totals.into_iter().zip(sector_totals) {
        let actual = sector_total.total();
        let residual = actual - declared;
        if !residual.is_finite() {
            return Err(Error::invalid_input(format!(
                "Campisi Carino period[{index}] {name} reconciliation residual must be finite"
            )));
        }
        if residual.abs() > tolerance {
            return Err(Error::invalid_input(format!(
                "Campisi Carino period[{index}] sector effects sum to {actual} for {name}, \
                 which does not reconcile to the declared total {declared} within scale-aware \
                 tolerance {tolerance} (scaled L1 effect scale {}, normalized L1 sum {})",
                effect_l1.scale, effect_l1.normalized_sum
            )));
        }
        declared_sum.add(declared);
    }

    let effect_total = declared_sum.total();
    let reconciliation_residual = effect_total - period.active_return;
    if !reconciliation_residual.is_finite() {
        return Err(Error::invalid_input(format!(
            "Campisi Carino period[{index}] effect reconciliation residual must be finite"
        )));
    }
    if reconciliation_residual.abs() > tolerance {
        return Err(Error::invalid_input(format!(
            "Campisi Carino period[{index}] effect totals sum to {effect_total}, which does not \
             reconcile to active_return {} within scale-aware tolerance {tolerance} \
             (scaled L1 effect scale {}, normalized L1 sum {})",
            period.active_return, effect_l1.scale, effect_l1.normalized_sum,
        )));
    }

    Ok(())
}

/// Apply Carino (1999) smoothing to per-period Campisi results so the five
/// arithmetic effects reconstruct the geometrically compounded active return.
///
/// Reuses the smoothing coefficient from [`crate::brinson`] (`k_t / K`
/// rescaling); this function only adapts it to the five-component FI
/// decomposition.
///
/// # Arguments
///
/// * `periods` - Chronologically ordered results with identical sector
///   ordering across periods.
///
/// # Errors
///
/// * [`Error::InvalidInput`] if `periods` is empty; sector ordering differs
///   across periods; any consumed top-level return/effect or per-sector
///   linked effect or `total_active` is non-finite; a declared `active_return`
///   does not agree with `portfolio_return - benchmark_return`; each sector's
///   `total_active` does not reconcile to its five component effects;
///   per-sector effects do not reconcile to their declared top-level totals;
///   the five totals do not reconcile to `active_return` within an
///   overflow-safe, scale-aware L1 tolerance; any return identity or
///   reconciliation residual is non-finite; or a return is at or below
///   −100 % (Carino domain, see [`crate::brinson::carino_link`]).
pub fn campisi_carino_link(periods: &[FiAttributionResult]) -> Result<FiCarinoLinkedResult> {
    let Some(first) = periods.first() else {
        return Err(Error::invalid_input(
            "Campisi Carino linking requires at least one period",
        ));
    };
    let sector_names: Vec<String> = first.sectors.iter().map(|e| e.sector.clone()).collect();
    for (idx, p) in periods.iter().enumerate().skip(1) {
        let same_order = p.sectors.len() == sector_names.len()
            && p.sectors
                .iter()
                .zip(sector_names.iter())
                .all(|(e, n)| e.sector == *n);
        if !same_order {
            return Err(Error::invalid_input(format!(
                "Campisi Carino linking requires identical sector ordering across all periods \
                 (period {idx} differs from period 0)"
            )));
        }
    }

    for (index, p) in periods.iter().enumerate() {
        validate_campisi_link_period(p, index)?;
    }

    const N_EFFECTS: usize = 5;
    let effect_periods: Vec<CarinoPeriod<N_EFFECTS>> = periods
        .iter()
        .map(|p| CarinoPeriod {
            portfolio_return: p.portfolio_return,
            benchmark_return: p.benchmark_return,
            rows: p
                .sectors
                .iter()
                .map(|e| {
                    [
                        e.allocation,
                        e.active_carry,
                        e.active_treasury,
                        e.active_spread,
                        e.selection,
                    ]
                })
                .collect(),
        })
        .collect();
    let linked = carino_link_effects(&effect_periods)?;

    let mut linked_sectors = Vec::with_capacity(sector_names.len());
    let mut totals = [NeumaierAccumulator::new(); N_EFFECTS];
    for (name, [allocation, active_carry, active_treasury, active_spread, selection]) in
        sector_names.into_iter().zip(linked.rows)
    {
        for (total, value) in totals.iter_mut().zip([
            allocation,
            active_carry,
            active_treasury,
            active_spread,
            selection,
        ]) {
            total.add(value);
        }
        linked_sectors.push(FiLinkedSectorEffect {
            sector: name,
            allocation,
            active_carry,
            active_treasury,
            active_spread,
            selection,
            total_active: allocation + active_carry + active_treasury + active_spread + selection,
        });
    }

    let [linked_allocation, linked_active_carry, linked_active_treasury, linked_active_spread, linked_selection] =
        totals.map(|value| value.total());
    Ok(FiCarinoLinkedResult {
        periods: periods.to_vec(),
        portfolio_return_compounded: linked.portfolio_return_compounded,
        benchmark_return_compounded: linked.benchmark_return_compounded,
        linked_sectors,
        linked_allocation,
        linked_active_carry,
        linked_active_treasury,
        linked_active_spread,
        linked_selection,
    })
}

/// Compute per-period Campisi attributions and Carino-link them.
///
/// Canonical entry point for bindings that receive raw period snapshots.
///
/// # Arguments
///
/// * `periods` - Chronologically ordered period inputs. Every period must
///   produce the same sector set in the same first-seen order.
/// * `config` - Shared period length.
///
/// # Errors
///
/// Propagates any [`campisi_attribution`] or [`campisi_carino_link`] error.
pub fn campisi_carino_link_from_snapshots(
    periods: &[FiPeriodInput],
    config: &FiAttributionConfig,
) -> Result<FiCarinoLinkedResult> {
    let results = periods
        .iter()
        .map(|p| campisi_attribution(&p.portfolio, &p.benchmark, config))
        .collect::<Result<Vec<_>>>()?;
    campisi_carino_link(&results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn snap(
        sector: &str,
        weight: f64,
        total_return: f64,
        yield_annual: f64,
        modified_duration: f64,
        spread_duration: f64,
        spread: f64,
        delta_treasury_yield: f64,
        delta_spread: f64,
    ) -> FiPositionSnapshot {
        FiPositionSnapshot {
            sector: sector.to_string(),
            weight,
            total_return,
            yield_annual,
            modified_duration,
            spread_duration,
            spread,
            delta_treasury_yield,
            delta_spread,
        }
    }

    /// Hand-worked golden fixture: 2 sectors × 2 positions per side,
    /// quarterly period.
    fn golden_portfolio() -> Vec<FiPositionSnapshot> {
        vec![
            snap("GOVT", 0.30, 0.0155, 0.040, 5.0, 0.0, 0.0, -0.0010, 0.0),
            snap("GOVT", 0.20, 0.0190, 0.045, 8.0, 0.0, 0.0, -0.0010, 0.0),
            snap(
                "CORP", 0.30, 0.0120, 0.060, 4.0, 3.8, 0.0150, -0.0010, 0.0020,
            ),
            snap(
                "CORP", 0.20, 0.0118, 0.070, 6.0, 5.5, 0.0250, -0.0010, 0.0020,
            ),
        ]
    }

    fn golden_benchmark() -> Vec<FiPositionSnapshot> {
        vec![
            snap("GOVT", 0.45, 0.0155, 0.038, 6.0, 0.0, 0.0, -0.0010, 0.0),
            snap("GOVT", 0.15, 0.0195, 0.042, 9.0, 0.0, 0.0, -0.0010, 0.0),
            snap(
                "CORP", 0.25, 0.0090, 0.055, 5.0, 4.8, 0.0120, -0.0010, 0.0020,
            ),
            snap(
                "CORP", 0.15, 0.0100, 0.065, 7.0, 6.5, 0.0200, -0.0010, 0.0020,
            ),
        ]
    }

    fn assert_close(actual: f64, expected: f64, label: &str) {
        assert!(
            (actual - expected).abs() < 1e-12,
            "{label}: got {actual}, expected {expected}"
        );
    }

    #[test]
    fn campisi_single_period_matches_hand_worked_golden() {
        let config = FiAttributionConfig::new(0.25);
        let r = campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config)
            .expect("valid golden inputs");

        // Headline returns.
        assert_close(r.portfolio_return, 0.01441, "portfolio_return");
        assert_close(r.benchmark_return, 0.01365, "benchmark_return");
        assert_close(r.active_return, 0.00076, "active_return");

        // Absolute Campisi split, portfolio side.
        assert_close(r.portfolio_components.carry, 0.01325, "p carry");
        assert_close(r.portfolio_components.treasury, 0.0055, "p treasury");
        assert_close(r.portfolio_components.spread, -0.00448, "p spread");
        assert_close(r.portfolio_components.selection, 0.00014, "p selection");
        assert_close(r.portfolio_components.total, 0.01441, "p total");

        // Absolute Campisi split, benchmark side.
        assert_close(r.benchmark_components.carry, 0.011725, "b carry");
        assert_close(r.benchmark_components.treasury, 0.00635, "b treasury");
        assert_close(r.benchmark_components.spread, -0.00435, "b spread");
        assert_close(r.benchmark_components.selection, -0.000075, "b selection");

        // Sector effects, in portfolio-first-seen order.
        assert_eq!(r.sectors.len(), 2);
        let govt = &r.sectors[0];
        assert_eq!(govt.sector, "GOVT");
        assert_close(govt.portfolio_weight, 0.50, "govt w_p");
        assert_close(govt.benchmark_weight, 0.60, "govt w_b");
        assert_close(govt.portfolio_return, 0.0169, "govt r_p");
        assert_close(govt.benchmark_return, 0.0165, "govt r_b");
        assert_close(govt.allocation, -0.000285, "govt allocation");
        assert_close(govt.active_carry, 0.000375, "govt active_carry");
        assert_close(govt.active_treasury, -0.000275, "govt active_treasury");
        assert_close(govt.active_spread, 0.0, "govt active_spread");
        assert_close(govt.selection, 0.0001, "govt selection");
        assert_close(govt.total_active, -0.000085, "govt total_active");

        let corp = &r.sectors[1];
        assert_eq!(corp.sector, "CORP");
        assert_close(corp.allocation, -0.0004275, "corp allocation");
        assert_close(corp.active_carry, 0.00065625, "corp active_carry");
        assert_close(corp.active_treasury, -0.000475, "corp active_treasury");
        assert_close(corp.active_spread, 0.0009575, "corp active_spread");
        assert_close(corp.selection, 0.00013375, "corp selection");
        assert_close(corp.total_active, 0.000845, "corp total_active");

        // Totals.
        assert_close(r.total_allocation, -0.0007125, "total_allocation");
        assert_close(r.total_active_carry, 0.00103125, "total_active_carry");
        assert_close(r.total_active_treasury, -0.00075, "total_active_treasury");
        assert_close(r.total_active_spread, 0.0009575, "total_active_spread");
        assert_close(r.total_selection, 0.00023375, "total_selection");

        // Reconciliation by construction.
        let recon = r.reconciliation_check(1e-10);
        assert!(recon.is_reconciled, "residual {}", recon.total_residual);
    }

    /// The five effects must reconstruct active return exactly even when a
    /// sector exists on only one side (zero weight on the other).
    #[test]
    fn campisi_handles_one_sided_sector() {
        let config = FiAttributionConfig::new(0.25);
        let portfolio = vec![
            snap("CORE", 0.80, 0.0150, 0.048, 5.0, 0.0, 0.0, -0.0010, 0.0),
            snap(
                "EXTRA", 0.20, 0.0210, 0.070, 3.0, 4.0, 0.0300, -0.0010, -0.0010,
            ),
        ];
        let benchmark = vec![snap(
            "CORE", 1.0, 0.0140, 0.044, 5.5, 0.0, 0.0, -0.0010, 0.0,
        )];

        let r = campisi_attribution(&portfolio, &benchmark, &config).expect("valid inputs");
        let reconstructed = r.total_allocation
            + r.total_active_carry
            + r.total_active_treasury
            + r.total_active_spread
            + r.total_selection;
        assert!(
            (reconstructed - r.active_return).abs() < 1e-12,
            "components {reconstructed} must equal active {}",
            r.active_return
        );
        assert!(r.reconciliation_check(1e-10).is_reconciled);

        // The zero-weight `rate` guard exists exactly for this case: a sector
        // genuinely absent from a side. It must keep working after the
        // zero-net-weight fail-closed check was added.
        let extra = &r.sectors[1];
        assert_eq!(extra.sector, "EXTRA");
        assert_close(extra.portfolio_weight, 0.20, "extra w_p");
        assert_close(extra.benchmark_weight, 0.0, "extra w_b");
        // Off-benchmark convention (M-2 fix): r_b,EXTRA := R_b = 0.0140, so
        // no allocation is charged for merely holding the sector. This pin
        // was deliberately updated from the old `0.0` convention.
        assert_close(extra.benchmark_return, 0.0140, "extra r_b := R_b");
        assert_close(extra.allocation, 0.0, "extra allocation is zero");
        assert!(
            extra.active_carry.abs() > 0.0,
            "an absent benchmark sector must still produce real portfolio-side effects"
        );
    }

    /// M-2 regression: an off-benchmark sector must not be charged the
    /// `−w_p,i · R_b` allocation penalty that the old `r_b,i = 0` convention
    /// produced. Under the documented convention (`r_b,i := R_b` for sectors
    /// absent from the benchmark) its allocation is exactly zero and the
    /// whole active effect flows through the component terms, while the
    /// telescoping identity and the headline active return are unchanged.
    /// Fixture (from the audit): portfolio CORE 80% @ 1.50% + EXTRA 20% @
    /// 2.10%; benchmark CORE 100% @ 1.40%; all component inputs zero so each
    /// side's return is pure selection. Hand-worked expectations:
    ///
    /// ```text
    /// R_p = 0.8·0.015 + 0.2·0.021 = 0.0162, R_b = 0.014, active = +22 bp
    /// CORE : alloc = (0.8−1.0)(0.014−0.014) = 0, sel = 0.8(0.015−0.014) = 0.0008
    /// EXTRA: r_b,EXTRA := R_b = 0.014
    ///        alloc = (0.2−0)(0.014−0.014) = 0, sel = 0.2(0.021−0.014) = 0.0014
    /// totals: allocation = 0, selection = 0.0022 = active
    /// ```
    #[test]
    fn campisi_off_benchmark_sector_uses_benchmark_total_return_convention() {
        let config = FiAttributionConfig::new(0.25);
        let portfolio = vec![
            snap("CORE", 0.80, 0.0150, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            snap("EXTRA", 0.20, 0.0210, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        ];
        let benchmark = vec![snap("CORE", 1.0, 0.0140, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)];

        let r = campisi_attribution(&portfolio, &benchmark, &config).expect("valid inputs");
        assert_close(r.active_return, 0.0022, "active_return");

        let core = &r.sectors[0];
        assert_eq!(core.sector, "CORE");
        assert_close(core.allocation, 0.0, "core allocation");
        assert_close(core.selection, 0.0008, "core selection");

        let extra = &r.sectors[1];
        assert_eq!(extra.sector, "EXTRA");
        assert_close(
            extra.benchmark_return,
            0.0140,
            "off-benchmark sector reports r_b,i := R_b",
        );
        assert_close(extra.allocation, 0.0, "extra allocation must be zero");
        assert_close(extra.selection, 0.0014, "extra selection");

        assert_close(r.total_allocation, 0.0, "total_allocation");
        assert_close(r.total_selection, 0.0022, "total_selection");
        assert!(r.reconciliation_check(1e-12).is_reconciled);
    }

    /// Companion convention for sectors absent from the *portfolio*: the
    /// reported `portfolio_return` mirrors the sector's benchmark rate
    /// (`r_p,i := r_b,i`) instead of a spurious 0. This is reporting-only —
    /// every effect multiplies it by `w_p,i = 0` — and the allocation charge
    /// for the underweight is unchanged.
    #[test]
    fn campisi_benchmark_only_sector_reports_benchmark_rate_for_portfolio_return() {
        let config = FiAttributionConfig::new(0.25);
        let portfolio = vec![snap("CORE", 1.0, 0.0150, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)];
        let benchmark = vec![
            snap("CORE", 0.80, 0.0140, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            snap("BONLY", 0.20, 0.0180, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        ];

        let r = campisi_attribution(&portfolio, &benchmark, &config).expect("valid inputs");
        // R_b = 0.8·0.014 + 0.2·0.018 = 0.0148
        assert_close(r.benchmark_return, 0.0148, "benchmark_return");
        let bonly = r
            .sectors
            .iter()
            .find(|s| s.sector == "BONLY")
            .expect("BONLY sector present");
        assert_close(
            bonly.portfolio_return,
            0.0180,
            "portfolio-absent sector reports r_p,i := r_b,i",
        );
        // Allocation for the underweight is the ordinary Brinson-Fachler
        // charge: (0 − 0.2)(0.018 − 0.0148) = −0.00064.
        assert_close(bonly.allocation, -0.00064, "BONLY allocation");
        assert!(r.reconciliation_check(1e-12).is_reconciled);
    }

    /// A sector that is *present* on a side but whose positions net to exactly
    /// zero weight (a long/short pair, a CDS hedge against a cash bond in the
    /// same bucket) has no defined per-unit rate. Silently zeroing its five
    /// effects leaves its real contribution `Σ w_j r_j ≠ 0` in the side total,
    /// breaking the telescoping identity while `active_return` still ties out
    /// against performance data. It must fail closed instead.
    #[test]
    fn campisi_rejects_zero_net_weight_sector_on_portfolio_side() {
        let config = FiAttributionConfig::new(0.25);
        let portfolio = vec![
            snap("CORE", 1.00, 0.0150, 0.048, 5.0, 0.0, 0.0, -0.0010, 0.0),
            snap("HEDGE", 0.50, 0.0400, 0.060, 3.0, 0.0, 0.0, -0.0010, 0.0),
            snap("HEDGE", -0.50, 0.0100, 0.020, 1.0, 0.0, 0.0, -0.0010, 0.0),
        ];
        let benchmark = vec![snap(
            "CORE", 1.0, 0.0140, 0.044, 5.5, 0.0, 0.0, -0.0010, 0.0,
        )];

        let err = campisi_attribution(&portfolio, &benchmark, &config)
            .expect_err("zero-net-weight sector must be rejected");
        let message = err.to_string();
        assert!(
            message.contains("HEDGE"),
            "error must name the sector: {message}"
        );
        assert!(
            message.contains("Portfolio"),
            "error must name the offending side: {message}"
        );
        assert!(
            message.contains("zero"),
            "error must explain the zero-net-weight cause: {message}"
        );
    }

    /// Same failure on the benchmark side, where the reviewer measured a
    /// reported attribution with the *opposite sign* to the actual active
    /// return.
    #[test]
    fn campisi_rejects_zero_net_weight_sector_on_benchmark_side() {
        let config = FiAttributionConfig::new(0.25);
        let portfolio = vec![snap(
            "CORE", 1.0, 0.0150, 0.048, 5.0, 0.0, 0.0, -0.0010, 0.0,
        )];
        let benchmark = vec![
            snap("CORE", 1.00, 0.0140, 0.044, 5.5, 0.0, 0.0, -0.0010, 0.0),
            snap("HEDGE", 0.40, 0.0300, 0.055, 4.0, 0.0, 0.0, -0.0010, 0.0),
            snap("HEDGE", -0.40, 0.0050, 0.015, 1.0, 0.0, 0.0, -0.0010, 0.0),
        ];

        let err = campisi_attribution(&portfolio, &benchmark, &config)
            .expect_err("zero-net-weight benchmark sector must be rejected");
        let message = err.to_string();
        assert!(
            message.contains("HEDGE"),
            "error must name the sector: {message}"
        );
        assert!(
            message.contains("Benchmark"),
            "error must name the offending side: {message}"
        );
    }

    /// The net-weight guard must reject *near* cancellation, not only *exact*
    /// cancellation. An exact-equality test (`agg.weight == 0.0`) lets a
    /// sector net to an arbitrarily small, non-zero weight through
    /// unrejected; [`SideAgg::rate`] then divides by that near-zero net
    /// weight, so the sector's rate — and the allocation effect built from it
    /// — grows without bound while no error is reported.
    ///
    /// Benchmark sector "HEDGE" holds `+0.40 @ 3%` and `-(0.40 - eps) @ 0.5%`
    /// (a CDS hedge against a cash bond in the same bucket), so its net
    /// weight is `eps` against a gross weight of `0.80 - eps`. The portfolio
    /// holds an ordinary `0.30` in HEDGE, so the Brinson-Fachler allocation
    /// `(w_p − w_b)(r_b,i − r_b)` multiplies the exploded benchmark rate by a
    /// non-tiny weight difference rather than cancelling it away.
    ///
    /// Pins the boundary in both directions. At `eps = 1e-8` (ratio ~1.25e-8,
    /// far below `NET_WEIGHT_RELATIVE_TOLERANCE`) the pre-fix code reported
    /// no error and produced a HEDGE allocation of `3.0e5` — 30 million bp —
    /// against an active return of `-81 bp`, with the telescoping identity
    /// missing by `-3.8e-11`, outside the `1e-12` tolerance this module's own
    /// tests assert. It must now be rejected, naming the sector and the side.
    /// At `eps = 1e-2` (ratio ~1.3e-2, comfortably above the bound) the input
    /// is legitimate and must still be accepted, with a bounded allocation.
    #[test]
    fn campisi_rejects_near_zero_net_weight_sector_before_rate_blows_up() {
        let config = FiAttributionConfig::new(0.25);
        let fixture = |eps: f64| {
            let portfolio = vec![
                snap("CORE", 0.70, 0.0150, 0.048, 5.0, 0.0, 0.0, -0.0010, 0.0),
                snap("HEDGE", 0.30, 0.0180, 0.050, 4.0, 0.0, 0.0, -0.0010, 0.0),
            ];
            let benchmark = vec![
                snap(
                    "CORE",
                    1.0 - eps,
                    0.0140,
                    0.044,
                    5.5,
                    0.0,
                    0.0,
                    -0.0010,
                    0.0,
                ),
                snap("HEDGE", 0.40, 0.0300, 0.055, 4.0, 0.0, 0.0, -0.0010, 0.0),
                snap(
                    "HEDGE",
                    -(0.40 - eps),
                    0.0050,
                    0.015,
                    1.0,
                    0.0,
                    0.0,
                    -0.0010,
                    0.0,
                ),
            ];
            (portfolio, benchmark)
        };

        let (portfolio, benchmark) = fixture(1e-8);
        let err = campisi_attribution(&portfolio, &benchmark, &config).expect_err(
            "near-zero net weight (ratio ~1e-8) must be rejected, not silently divided through",
        );
        let message = err.to_string();
        assert!(
            message.contains("HEDGE"),
            "error must name the sector: {message}"
        );
        assert!(
            message.contains("Benchmark"),
            "error must name the offending side: {message}"
        );

        let (portfolio, benchmark) = fixture(1e-2);
        let r = campisi_attribution(&portfolio, &benchmark, &config)
            .expect("net weight well clear of the relative bound must be accepted");
        let hedge = &r.sectors[1];
        assert_eq!(hedge.sector, "HEDGE");
        assert!(
            hedge.allocation.abs() < 10.0,
            "a legitimate small-but-not-near-zero net weight must not produce an exploded \
             allocation: {}",
            hedge.allocation
        );
    }

    /// Negative net sector weights are legal (a net-short sector) and must be
    /// attributed by dividing through, not zeroed. Pins the `.abs()` in
    /// [`SideAgg::rate`]'s guard: dropping it silently zeroes the entire
    /// decomposition for every net-short sector.
    #[test]
    fn campisi_attributes_net_short_sector() {
        let config = FiAttributionConfig::new(0.25);
        let portfolio = vec![
            snap("CORE", 1.20, 0.0150, 0.048, 5.0, 0.0, 0.0, -0.0010, 0.0),
            snap("SHORT", -0.20, 0.0100, 0.030, 2.0, 0.0, 0.0, -0.0010, 0.0),
        ];
        let benchmark = vec![snap(
            "CORE", 1.0, 0.0140, 0.044, 5.5, 0.0, 0.0, -0.0010, 0.0,
        )];

        let r = campisi_attribution(&portfolio, &benchmark, &config).expect("net-short is legal");
        let short = &r.sectors[1];
        assert_eq!(short.sector, "SHORT");
        assert_close(short.portfolio_weight, -0.20, "short w_p");
        // r_p = (−0.2 × 0.0100) / −0.2 = 0.0100; carry rate = 0.030 × 0.25.
        assert_close(short.portfolio_return, 0.0100, "short r_p");
        // SHORT is absent from the benchmark, so under the off-benchmark
        // convention (M-2 fix; see the module docs) its benchmark component
        // rates are the benchmark-wide ones: Carry_b = 0.044 × 0.25 = 0.011,
        // Treasury_b = −5.5 × −0.0010 = 0.0055. These pins were deliberately
        // updated from the old `r_b,i = 0` convention.
        assert_close(
            short.active_carry,
            -0.20 * (0.0075 - 0.011),
            "short active_carry",
        );
        // treasury_p = −2.0 × −0.0010 = 0.0020.
        assert_close(
            short.active_treasury,
            -0.20 * (0.0020 - 0.0055),
            "short active_treasury",
        );
        // Off-benchmark allocation is identically zero under the convention.
        assert_close(short.allocation, 0.0, "short allocation");
        assert!(r.reconciliation_check(1e-12).is_reconciled);
    }

    #[test]
    fn campisi_rejects_weights_not_summing_to_one() {
        let config = FiAttributionConfig::new(0.25);
        let mut portfolio = golden_portfolio();
        portfolio[0].weight = 0.10; // now sums to 0.80
        let err = campisi_attribution(&portfolio, &golden_benchmark(), &config)
            .expect_err("weights must sum to 1");
        assert!(err.to_string().contains("Portfolio weights"), "{err}");
    }

    /// Pins [`WEIGHT_TOLERANCE`] itself. The test above misses by 0.20, which
    /// still trips under an absurdly loose tolerance; these two cases bracket
    /// the documented ±1e-6 boundary so loosening or tightening it fails here.
    #[test]
    fn campisi_weight_tolerance_is_pinned_at_1e_minus_6() {
        let config = FiAttributionConfig::new(0.25);

        let mut just_over = golden_portfolio();
        just_over[0].weight += 2e-6; // sums to 1 + 2e-6 > tolerance
        let err = campisi_attribution(&just_over, &golden_benchmark(), &config)
            .expect_err("1 + 2e-6 must be rejected at a 1e-6 tolerance");
        assert!(err.to_string().contains("Portfolio weights"), "{err}");

        let mut just_under = golden_portfolio();
        just_under[0].weight += 5e-7; // sums to 1 + 5e-7 < tolerance
        assert!(
            campisi_attribution(&just_under, &golden_benchmark(), &config).is_ok(),
            "1 + 5e-7 must be accepted at a 1e-6 tolerance"
        );
    }

    #[test]
    fn campisi_rejects_non_finite_inputs() {
        let config = FiAttributionConfig::new(0.25);
        let mut portfolio = golden_portfolio();
        portfolio[2].delta_spread = f64::NAN;
        let err = campisi_attribution(&portfolio, &golden_benchmark(), &config)
            .expect_err("NaN must be rejected");
        assert!(err.to_string().contains("finite"), "{err}");
    }

    #[test]
    fn campisi_rejects_bad_period_years() {
        let mut config = FiAttributionConfig::new(0.0);
        let err = campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config)
            .expect_err("period_years must be positive");
        assert!(err.to_string().contains("period_years"), "{err}");
        config.period_years = f64::INFINITY;
        assert!(campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config).is_err());
    }

    #[test]
    fn campisi_rejects_empty_sides() {
        let config = FiAttributionConfig::new(0.25);
        assert!(campisi_attribution(&[], &golden_benchmark(), &config).is_err());
        assert!(campisi_attribution(&golden_portfolio(), &[], &config).is_err());
    }

    #[test]
    fn snapshot_serde_denies_unknown_fields_and_uses_stable_names() {
        let json = r#"{
            "sector": "CORP", "weight": 0.5, "total_return": 0.01,
            "yield_annual": 0.06, "modified_duration": 4.0,
            "spread_duration": 3.8, "spread": 0.015,
            "delta_treasury_yield": -0.001, "delta_spread": 0.002
        }"#;
        let snap: FiPositionSnapshot = serde_json::from_str(json).expect("stable names");
        assert_eq!(snap.sector, "CORP");

        let bad = r#"{
            "sector": "CORP", "weight": 0.5, "total_return": 0.01,
            "yield_annual": 0.06, "modified_duration": 4.0,
            "spread_duration": 3.8, "spread": 0.015,
            "delta_treasury_yield": -0.001, "delta_spread": 0.002,
            "surprise": 1.0
        }"#;
        assert!(serde_json::from_str::<FiPositionSnapshot>(bad).is_err());

        let config: FiAttributionConfig =
            serde_json::from_str(r#"{"period_years": 0.25}"#).expect("config parses");
        assert!((config.period_years - 0.25).abs() < 1e-15);

        // `period_years` is the config's only field and carries no serde
        // default: an empty object must fail rather than assume a period.
        assert!(serde_json::from_str::<FiAttributionConfig>("{}").is_err());

        // The removed `spread_mode` key must now fail closed as an unknown
        // field, so a stale caller is told rather than silently ignored.
        assert!(serde_json::from_str::<FiAttributionConfig>(
            r#"{"period_years": 0.25, "spread_mode": "dts"}"#
        )
        .is_err());
    }

    /// The spread effect is `−SD · Δs` and never touches the spread *level*,
    /// so the level is inert: zero and negative spreads must be accepted and
    /// must not perturb any effect. This replaces the old pair of DTS-mode
    /// tests (mode-equivalence, and the DTS rejection of a non-positive
    /// spread), which pinned a mode that no longer exists.
    #[test]
    fn campisi_spread_level_is_inert_and_may_be_zero_or_negative() {
        let config = FiAttributionConfig::new(0.25);
        let build = |p_spread: f64, b_spread: f64| {
            (
                vec![snap(
                    "CORP", 1.0, 0.0120, 0.060, 4.0, 3.8, p_spread, -0.0010, 0.0020,
                )],
                vec![snap(
                    "CORP", 1.0, 0.0090, 0.055, 5.0, 4.8, b_spread, -0.0010, 0.0020,
                )],
            )
        };

        // Baseline: ordinary positive spread levels.
        let (portfolio, benchmark) = build(0.0150, 0.0120);
        let base = campisi_attribution(&portfolio, &benchmark, &config).expect("positive spreads");
        // w_p (−SD_p Δs − (−SD_b Δs)) = 1.0 × (−3.8 + 4.8) × 0.0020.
        assert_close(base.total_active_spread, 0.0020, "baseline active_spread");

        // A zero spread on the portfolio side with a non-zero SD × Δs term is
        // exactly what the old DTS mode rejected. It is now a legal input.
        let (zero_p, zero_b) = build(0.0, 0.0120);
        let zero = campisi_attribution(&zero_p, &zero_b, &config).expect("zero spread is legal");

        // Rich bonds can have negative quote-reproducing Z-spreads.
        let (neg_p, neg_b) = build(-0.0025, -0.0040);
        let negative =
            campisi_attribution(&neg_p, &neg_b, &config).expect("negative spread is legal");

        for (label, r) in [("zero", &zero), ("negative", &negative)] {
            assert_close(
                r.total_active_spread,
                base.total_active_spread,
                &format!("{label} active_spread must match the baseline"),
            );
            assert_close(
                r.total_selection,
                base.total_selection,
                &format!("{label} selection must match the baseline"),
            );
            assert!(r.reconciliation_check(1e-10).is_reconciled);
        }
    }

    /// Carino linking must rescale the five FI effects so their linked sum
    /// reconstructs the geometric compounded active return exactly; with two
    /// identical periods every linked component is its arithmetic two-period
    /// sum times the uniform scale `geometric_active / arithmetic_active`.
    #[test]
    fn campisi_carino_link_matches_compounded_active_return() {
        let config = FiAttributionConfig::new(0.25);
        let period = FiPeriodInput {
            portfolio: golden_portfolio(),
            benchmark: golden_benchmark(),
        };
        let linked = campisi_carino_link_from_snapshots(&[period.clone(), period], &config)
            .expect("carino link");

        // Compounded returns (hand-worked): 1.01441^2 − 1, 1.01365^2 − 1.
        let rp = 1.01441_f64.powi(2) - 1.0;
        let rb = 1.01365_f64.powi(2) - 1.0;
        assert!((linked.portfolio_return_compounded - rp).abs() < 1e-12);
        assert!((linked.benchmark_return_compounded - rb).abs() < 1e-12);

        let geometric_active = rp - rb;
        let reconstructed = linked.linked_allocation
            + linked.linked_active_carry
            + linked.linked_active_treasury
            + linked.linked_active_spread
            + linked.linked_selection;
        assert!(
            (reconstructed - geometric_active).abs() < 1e-10,
            "linked effects {reconstructed} must equal geometric active {geometric_active}"
        );

        // Smoothing must not be a no-op: arithmetic ≠ geometric here.
        let arithmetic_active = 2.0 * 0.00076;
        assert!((arithmetic_active - geometric_active).abs() > 1e-7);

        // Identical periods ⇒ identical k_t ⇒ one uniform scale factor.
        let scale = geometric_active / arithmetic_active;
        assert!(
            (linked.linked_active_spread - 2.0 * 0.0009575 * scale).abs() < 1e-12,
            "linked_active_spread must be uniformly Carino-scaled"
        );
        assert!((linked.linked_allocation - 2.0 * -0.0007125 * scale).abs() < 1e-12);

        // Sector ordering preserved.
        let names: Vec<&str> = linked
            .linked_sectors
            .iter()
            .map(|s| s.sector.as_str())
            .collect();
        assert_eq!(names, ["GOVT", "CORP"]);

        // Every per-sector linked effect, field by field, against the golden
        // single-period value × 2 periods × the uniform Carino scale. Without
        // these the grand totals stay correct even if the per-sector record
        // transposes two effects or drops one from `total_active`, because the
        // totals accumulate from the pre-record locals.
        let golden: [(&str, [f64; 5]); 2] = [
            ("GOVT", [-0.000285, 0.000375, -0.000275, 0.0, 0.0001]),
            (
                "CORP",
                [-0.0004275, 0.00065625, -0.000475, 0.0009575, 0.00013375],
            ),
        ];
        for (linked_sector, (name, effects)) in linked.linked_sectors.iter().zip(golden) {
            assert_eq!(linked_sector.sector, name);
            let [allocation, carry, treasury, spread, selection] =
                effects.map(|value| 2.0 * value * scale);
            assert_close(
                linked_sector.allocation,
                allocation,
                &format!("{name} linked allocation"),
            );
            assert_close(
                linked_sector.active_carry,
                carry,
                &format!("{name} linked active_carry"),
            );
            assert_close(
                linked_sector.active_treasury,
                treasury,
                &format!("{name} linked active_treasury"),
            );
            assert_close(
                linked_sector.active_spread,
                spread,
                &format!("{name} linked active_spread"),
            );
            assert_close(
                linked_sector.selection,
                selection,
                &format!("{name} linked selection"),
            );
            assert_close(
                linked_sector.total_active,
                allocation + carry + treasury + spread + selection,
                &format!("{name} linked total_active"),
            );
        }
    }

    #[test]
    fn campisi_link_rejects_active_return_mismatch() {
        let config = FiAttributionConfig::new(0.25);
        let mut period =
            campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config).unwrap();
        period.active_return += 0.001;

        let err = campisi_carino_link(&[period])
            .expect_err("declared active return must match portfolio minus benchmark");
        assert!(
            err.to_string().contains("active_return"),
            "error must name the mismatched field: {err}"
        );
    }

    #[test]
    fn campisi_link_rejects_materially_tampered_top_level_totals() {
        let config = FiAttributionConfig::new(0.25);
        let mut period =
            campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config).unwrap();
        period.total_selection += 0.001;

        let err = campisi_carino_link(&[period])
            .expect_err("linking must reject inconsistent totals rather than repair them");
        assert!(
            err.to_string().contains("declared total"),
            "error must name the violated reconciliation: {err}"
        );
    }

    #[test]
    fn campisi_link_rejects_non_finite_and_inconsistent_sector_effects() {
        let config = FiAttributionConfig::new(0.25);
        let period =
            campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config).unwrap();

        let mut non_finite_total = period.clone();
        non_finite_total.total_active_carry = f64::NAN;
        let err = campisi_carino_link(&[non_finite_total])
            .expect_err("NaN top-level effects consumed by validation must be rejected");
        assert!(
            err.to_string().contains("total_active_carry"),
            "error must name the non-finite top-level field: {err}"
        );

        let mut non_finite = period.clone();
        non_finite.sectors[0].active_spread = f64::NAN;
        let err = campisi_carino_link(&[non_finite])
            .expect_err("NaN sector effects consumed by linking must be rejected");
        assert!(
            err.to_string().contains("active_spread"),
            "error must name the non-finite sector field: {err}"
        );

        let mut inconsistent = period;
        inconsistent.sectors[0].selection += 0.001;
        inconsistent.sectors[0].total_active += 0.001;
        let err = campisi_carino_link(&[inconsistent])
            .expect_err("sector effects must reconcile with declared top-level totals");
        assert!(
            err.to_string().contains("total_selection"),
            "error must name the inconsistent declared total: {err}"
        );
    }

    fn neutralize_campisi_effects(period: &mut FiAttributionResult) {
        period.total_allocation = 0.0;
        period.total_active_carry = 0.0;
        period.total_active_treasury = 0.0;
        period.total_active_spread = 0.0;
        period.total_selection = 0.0;
        for sector in &mut period.sectors {
            sector.allocation = 0.0;
            sector.active_carry = 0.0;
            sector.active_treasury = 0.0;
            sector.active_spread = 0.0;
            sector.selection = 0.0;
            sector.total_active = 0.0;
        }
    }

    #[test]
    fn campisi_link_rejects_active_mismatch_when_additive_return_scale_overflows() {
        let config = FiAttributionConfig::new(0.25);
        let mut period =
            campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config).unwrap();
        neutralize_campisi_effects(&mut period);
        period.portfolio_return = 1e308;
        period.benchmark_return = 9e307;
        period.active_return = 0.0;

        let err = campisi_carino_link(&[period])
            .expect_err("finite return magnitudes must not overflow the validation scale");
        assert!(err.to_string().contains("active_return"), "{err}");
    }

    #[test]
    fn campisi_link_rejects_non_finite_expected_active_explicitly() {
        let config = FiAttributionConfig::new(0.25);
        let mut period =
            campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config).unwrap();
        neutralize_campisi_effects(&mut period);
        period.portfolio_return = f64::MAX;
        period.benchmark_return = -f64::MAX;
        period.active_return = 0.0;

        let err = campisi_carino_link(&[period])
            .expect_err("overflowed portfolio-minus-benchmark return must be rejected");
        assert!(
            err.to_string()
                .contains("portfolio_return - benchmark_return must be finite"),
            "{err}"
        );
    }

    #[test]
    fn campisi_link_accepts_huge_finite_cancelling_effects() {
        let config = FiAttributionConfig::new(0.25);
        let mut period =
            campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config).unwrap();
        neutralize_campisi_effects(&mut period);
        period.portfolio_return = 0.01;
        period.benchmark_return = 0.01;
        period.active_return = 0.0;
        period.total_allocation = f64::MAX;
        period.total_active_carry = -f64::MAX;
        period.sectors[0].allocation = f64::MAX;
        period.sectors[0].active_carry = -f64::MAX;

        let linked = campisi_carino_link(&[period])
            .expect("scaled L1 tolerance must not overflow on finite cancelling effects");
        assert_eq!(linked.linked_allocation, f64::MAX);
        assert_eq!(linked.linked_active_carry, -f64::MAX);
    }

    #[test]
    fn campisi_link_rejects_non_finite_reconciliation_residual_explicitly() {
        let config = FiAttributionConfig::new(0.25);
        let mut period =
            campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config).unwrap();
        neutralize_campisi_effects(&mut period);
        period.portfolio_return = 0.01;
        period.benchmark_return = 0.01;
        period.active_return = 0.0;
        period.total_allocation = f64::MAX;
        period.total_active_carry = f64::MAX;
        period.total_active_treasury = -f64::MAX;
        period.sectors[0].allocation = f64::MAX;
        period.sectors[0].active_carry = f64::MAX;
        period.sectors[0].active_treasury = -f64::MAX;
        period.sectors[0].total_active = f64::MAX;

        let err = campisi_carino_link(&[period])
            .expect_err("overflowed effect reconciliation must be rejected");
        assert!(
            err.to_string()
                .contains("reconciliation residual must be finite"),
            "{err}"
        );
    }

    #[test]
    fn campisi_link_rejects_non_finite_sector_total_active() {
        let config = FiAttributionConfig::new(0.25);
        let mut period =
            campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config).unwrap();
        period.sectors[0].total_active = f64::NAN;

        let err = campisi_carino_link(&[period])
            .expect_err("non-finite declared sector total must be rejected");
        assert!(err.to_string().contains("total_active"), "{err}");
    }

    #[test]
    fn campisi_link_rejects_inconsistent_sector_total_active() {
        let config = FiAttributionConfig::new(0.25);
        let mut period =
            campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config).unwrap();
        period.sectors[0].total_active += 0.001;

        let err = campisi_carino_link(&[period])
            .expect_err("declared sector total must reconcile to its five effects");
        assert!(
            err.to_string().contains("total_active") && err.to_string().contains("five effects"),
            "{err}"
        );
    }

    #[test]
    fn campisi_link_accepts_generated_near_cancelling_output() {
        let config = FiAttributionConfig::new(0.25);
        let eps = 1.1e-6;
        let portfolio = vec![
            snap("CORE", 0.70, 0.4, 0.048, 5.0, 0.0, 0.0, -0.001, 0.0),
            snap("HEDGE", 0.30, 0.8, 0.050, 4.0, 0.0, 0.0, -0.001, 0.0),
        ];
        let benchmark = vec![
            snap("CORE", 1.0 - eps, 0.1, 0.044, 5.5, 0.0, 0.0, -0.001, 0.0),
            snap("HEDGE", 0.40, 1.0, 0.055, 4.0, 0.0, 0.0, -0.001, 0.0),
            snap(
                "HEDGE",
                -(0.40 - eps),
                -0.2,
                0.015,
                1.0,
                0.0,
                0.0,
                -0.001,
                0.0,
            ),
        ];
        let period = campisi_attribution(&portfolio, &benchmark, &config)
            .expect("the sector remains just outside the near-zero rejection boundary");
        let gross: f64 = period
            .sectors
            .iter()
            .map(|sector| {
                sector.allocation.abs()
                    + sector.active_carry.abs()
                    + sector.active_treasury.abs()
                    + sector.active_spread.abs()
                    + sector.selection.abs()
            })
            .sum();
        assert!(
            gross > 1_000.0,
            "fixture must exercise cancellation between large effects (gross {gross})"
        );

        campisi_carino_link(&[period])
            .expect("scale-aware validation must accept generated near-cancelling output");
    }

    #[test]
    fn campisi_carino_link_rejects_empty_and_inconsistent_periods() {
        assert!(campisi_carino_link(&[]).is_err());

        let config = FiAttributionConfig::new(0.25);
        let p1 = campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config)
            .expect("period 1");
        let mut p2 = p1.clone();
        p2.sectors[0].sector = "DIFFERENT".to_string();
        let err = campisi_carino_link(&[p1, p2]).expect_err("sector ordering must match");
        assert!(err.to_string().contains("sector ordering"), "{err}");
    }

    /// [`FiAttributionResult`] is an *input* to [`campisi_carino_link`] and to
    /// both bindings, so it must round-trip its own serialization exactly and
    /// reject unknown keys — a stale or misspelled field must not be silently
    /// dropped into a result that then "reconciles" on partial data.
    #[test]
    fn attribution_result_round_trips_and_denies_unknown_fields() {
        let config = FiAttributionConfig::new(0.25);
        let result = campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config)
            .expect("golden inputs");
        let json = serde_json::to_string(&result).expect("serialize result");

        // Round-trip: `deny_unknown_fields` must not reject our own output.
        let parsed: FiAttributionResult = serde_json::from_str(&json).expect("round-trip");
        assert!((parsed.active_return - result.active_return).abs() < 1e-15);
        assert_eq!(parsed.sectors.len(), result.sectors.len());
        // And it still links, i.e. the round-tripped value is fully usable.
        assert!(campisi_carino_link(&[parsed]).is_ok());

        // Top-level unknown key.
        let bogus = json.replacen('{', r#"{"bogus_field": 1.0,"#, 1);
        assert!(
            serde_json::from_str::<FiAttributionResult>(&bogus).is_err(),
            "unknown top-level field must be rejected"
        );

        // Nested unknown keys in the two inbound child types.
        let bogus_sector = json.replacen(r#"{"sector""#, r#"{"bogus_field": 1.0, "sector""#, 1);
        assert_ne!(bogus_sector, json, "fixture must actually mutate a sector");
        assert!(
            serde_json::from_str::<FiAttributionResult>(&bogus_sector).is_err(),
            "unknown field inside FiSectorEffect must be rejected"
        );
        let bogus_components = json.replacen(r#""carry""#, r#""bogus_field": 1.0, "carry""#, 1);
        assert_ne!(
            bogus_components, json,
            "fixture must actually mutate a components block"
        );
        assert!(
            serde_json::from_str::<FiAttributionResult>(&bogus_components).is_err(),
            "unknown field inside FiComponents must be rejected"
        );
    }

    /// [`FiAttributionResult::reconciliation_check`] is reachable from the
    /// bindings as JSON, so its report must serialize under stable names.
    #[test]
    fn reconciliation_report_serializes_under_stable_names() {
        let config = FiAttributionConfig::new(0.25);
        let result = campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config)
            .expect("golden inputs");
        let json =
            serde_json::to_string(&result.reconciliation_check(1e-10)).expect("serialize report");
        let value: serde_json::Value = serde_json::from_str(&json).expect("parse report");
        assert_eq!(value["is_reconciled"], serde_json::json!(true));
        assert_eq!(value["tolerance"], serde_json::json!(1e-10));
        assert!(value["total_residual"].as_f64().expect("residual").abs() <= 1e-10);
    }
}

//! Hierarchical duration-cell x sector grid attribution (Dynkin, Hyman &
//! Vankudre 1998, Appendix A).
//!
//! Lehman's grid attribution splits benchmark-relative fixed-income
//! performance into a two-level hierarchy: coarse duration cells (the
//! "curve" axis) crossed with sectors *within* each cell (the "sector" axis),
//! plus a security-selection residual. Unlike [`crate::fi_attribution`]
//! (which decomposes *component* returns — carry/treasury/spread/selection —
//! within a flat sector grouping), this module decomposes the *positioning*
//! of weight across a two-level cell x sector grid using only weights and
//! total returns.
//!
//! # Notation
//!
//! For cell `t` and sector `s` within it, on side `k ∈ {P, B}` (portfolio /
//! benchmark):
//!
//! ```text
//! x_t^k   = cell weight            = Σ_s y_st^k
//! y_st^k  = (cell, sector) weight  = Σ_j w_j   (positions in cell t, sector s)
//! z_st^k  = within-cell share      = y_st^k / x_t^k
//! r_st^k  = (cell, sector) return  = (Σ_j w_j r_j) / y_st^k
//! r_t^k   = cell return            = Σ_s z_st^k r_st^k = (Σ_s y_st^k r_st^k) / x_t^k
//! r^k     = side return            = Σ_t x_t^k r_t^k
//! ```
//!
//! # Formulas
//!
//! ```text
//! e^curve_t     = (x_t^P − x_t^B)(r_t^B − r^B)
//! e^sector_st   = x_t^P (z_st^P − z_st^B)(r_st^B − r_t^B)
//! e^security_st = y_st^P (r_st^P − r_st^B)
//! Σ_t e^curve_t + Σ_st e^sector_st + Σ_st e^security_st ≡ r^P − r^B
//! ```
//!
//! # Telescoping identity
//!
//! The three sums telescope to the active return, given per-side weight
//! normalization (`Σ_t x_t^k = 1`), within-cell share normalization (`Σ_s
//! z_st^k = 1` for every cell present on side `k`), and every present
//! bucket's net weight clearing the fail-closed check below:
//!
//! ```text
//! Σ_st e^security_st = Σ_st y_st^P r_st^P − Σ_st y_st^P r_st^B
//!                     = r^P − Σ_st y_st^P r_st^B
//! Σ_st e^sector_st    = Σ_st y_st^P r_st^B − Σ_t x_t^P r_t^B      (Σ_s z_st^P = Σ_s z_st^B = 1)
//! Σ_t  e^curve_t      = Σ_t x_t^P r_t^B − r^B                    (Σ_t x_t^P = Σ_t x_t^B = 1)
//! ------------------------------------------------------------------------
//! Sum                 = r^P − r^B
//! ```
//!
//! This is *not* a universal exactness guarantee: the identity holds to
//! floating-point precision **only for well-conditioned inputs**, and
//! degrades continuously and measurably as any bucket's net weight
//! approaches the fail-closed guard's own boundary — the guard bounds how
//! explosive a bucket's rate can become, it does not bound how much
//! numerical noise that rate injects into the reconstructed sum. Measured on
//! a fixture with a ~1% return spread (a bucket netting to `eps` relative to
//! its own gross weight, `eps` decreasing toward the guard's `1e-6`
//! boundary):
//!
//! ```text
//! eps = 1e-3  ->  reconciliation residual ~  1.1e-13   (float noise floor)
//! eps = 1e-4  ->  reconciliation residual ~ -7.1e-12
//! eps = 1e-5  ->  reconciliation residual ~  5.7e-10
//! eps = 1e-6  ->  reconciliation residual ~ -5.7e-08   (just inside the guard)
//! ```
//!
//! A wider return spread amplifies the same effect: the same `eps = 1e-6`
//! bucket against a ~100% return spread measured a residual around `-8.6e-6`
//! — roughly 150x worse than the ~1% case, because the residual scales with
//! both the near-cancellation amplification *and* the magnitude of the
//! return differences it amplifies. So a caller relying on this module's
//! output tying out to a tight (sub-`1e-9`) tolerance must itself check how
//! close its own buckets' net weights sit to zero, not just that
//! [`grid_attribution()`] returned `Ok`; the fixtures in this module's own
//! tests all use net weights well clear of the guard's boundary (or are
//! adversarial in other ways, not in near-cancellation), and their tight
//! tolerances (`1e-12`, `1e-15`) hold *for those specific fixtures*, not as a
//! bound on every valid input.
//!
//! The per-side weight-sum validation, within-cell share normalization, and
//! near-zero-net-weight rejection enforced by [`grid_attribution()`] are
//! therefore not cosmetic input checks — without them the identity does not
//! hold even approximately — but clearing them is necessary, not sufficient,
//! for a *tight* reconciliation; see `NET_WEIGHT_RELATIVE_TOLERANCE`'s own
//! doc comment for why that bound was deliberately not tightened further to
//! close this gap.
//!
//! # Out-of-benchmark fallbacks
//!
//! A bucket with portfolio positions but no benchmark counterpart has an
//! undefined per-unit benchmark rate. Lehman's convention assigns it a
//! return "typical of the sector" — the portfolio's own rate — with zero
//! benchmark weight, so its selection effect collapses to zero and the whole
//! effect flows to the coarser level:
//!
//! - **Cell absent from the benchmark** (`x_t^B = 0`, i.e. no benchmark
//!   positions in the cell at all): `r_t^B := r_t^P`, and every sector's
//!   benchmark share/return within the cell falls back to the portfolio's
//!   (`z_st^B := z_st^P`, `r_st^B := r_st^P`). Both `e^sector_st` and
//!   `e^security_st` collapse to zero for every sector in the cell; the
//!   entire effect surfaces as `e^curve_t = x_t^P (r_t^P − r^B)`.
//! - **(Cell, sector) absent from the benchmark but the cell is present**
//!   (`x_t^B ≠ 0`, `y_st^B = 0`): `z_st^B` is naturally `0` (no fallback
//!   needed there), but `r_st^B` has no data, so `r_st^B := r_st^P`.
//!   `e^security_st` collapses to zero; the effect surfaces as
//!   `e^sector_st = x_t^P z_st^P (r_st^P − r_t^B)`.
//!
//! A bucket with *no* portfolio positions needs no fallback at all: every
//! term above carries a portfolio-side weight factor (`x_t^P` or `y_st^P`)
//! that is already zero.
//!
//! # Fail-closed validation
//!
//! - Each side's weights must sum to `1.0` within `±1e-6`.
//! - A bucket (cell, or cell+sector) that has positions on a side but nets to
//!   zero, or to a weight smaller than an internal relative tolerance
//!   (1e-6 of its own gross weight; e.g. a long/short pair in the same
//!   bucket that nearly, or exactly, offsets), has an undefined-or-explosive
//!   per-unit rate: at exact cancellation the rate `weighted_return / weight`
//!   is `0 / 0`; arbitrarily close to it, the rate is finite but grows
//!   without bound as the net weight shrinks, so a check that only rejects
//!   the exact-zero case still lets a bucket's rate — and every effect
//!   derived from it — blow up into a numerically meaningless value while
//!   reporting no error. Rather than silently zeroing its effects (which
//!   would leave its non-zero contribution to the side return unexplained
//!   and break the telescoping identity) or only catching the exact-zero
//!   case, [`grid_attribution()`] rejects any bucket whose net weight fails
//!   the relative bound, naming the bucket and the side.
//! - A bucket that nets to a weight *outside* that near-zero band (including
//!   negative / net-short) is ordinary input and is attributed normally by
//!   dividing through the net weight, not the gross weight.
//!
//! # Ordering
//!
//! Cells and, within each cell, sectors are emitted in first-appearance
//! order: portfolio positions are scanned before benchmark positions, so
//! portfolio-seen buckets sort first, followed by benchmark-only buckets
//! ([`indexmap::IndexMap`] preserves insertion order deterministically).
//!
//! # References
//!
//! * Dynkin, L., Hyman, J., & Vankudre, P. (1998). "Attribution of Portfolio
//!   Performance Relative to an Index." Lehman Brothers Fixed Income
//!   Research, March 1998, Appendix A — source of the curve/sector/selection
//!   grid decomposition and the out-of-benchmark fallback convention used
//!   above. `docs/REFERENCES.md#dynkin-hyman-vankudre-1998`

use crate::brinson::{carino_link_effects, CarinoPeriod};
use crate::error::{Error, Result};
use finstack_quant_core::math::summation::NeumaierAccumulator;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Tolerance for the requirement that weights sum to 1.0 on each side.
const WEIGHT_TOLERANCE: f64 = 1e-6;

/// Relative tolerance on the ratio `|net weight| / gross weight` below which
/// a bucket's per-unit rate is treated as too poorly conditioned to
/// attribute, in [`check_zero_net_weight`].
///
/// A bucket's rate is `weighted_return / weight` (see [`BucketAgg::rate`]).
/// At exact cancellation (`weight == 0.0` with `abs_weight > 0.0`) that rate
/// is undefined; arbitrarily close to exact cancellation it is *defined* but
/// numerically explosive — as `weight -> 0` for a roughly fixed
/// `weighted_return`, the rate, and every curve/sector/selection effect
/// derived from it, grows without bound. An exact-equality-only guard misses
/// this: a benchmark bucket of `+0.5` and `-(0.5 - 1e-8)` in the same cell
/// nets to `1e-8`, not `0.0`, yet produces a curve effect on the order of
/// `2.5e5` against a realistic sub-1% active return.
///
/// # What this bound does, and does not, guarantee
///
/// This bound exists to cap how *explosive* an accepted bucket's rate can
/// become; it is not, and cannot be made to be, a promise that the
/// reconstructed `total_curve + total_sector + total_selection` reconciles
/// to `active_return` within any particular tolerance — see the module-level
/// "Telescoping identity" section for measured reconciliation residuals as
/// large as `~5.7e-8` (on a ~1% return spread) or `~8.6e-6` (on a ~100%
/// spread) for an *accepted*, in-bound bucket sitting right at this
/// tolerance. Tightening this constant would shrink those residuals, but at
/// a real cost: reaching a `1e-12`-tight reconciliation guarantee across
/// realistic return spreads would require a bound near `1e-3`, which would
/// reject a bucket netting to 0.1% of its own gross weight — an entirely
/// plausible near-hedge in a long/short credit portfolio, not a numerical
/// artifact to be filtered out. `1e-6` is therefore a deliberate trade-off,
/// reusing [`WEIGHT_TOLERANCE`]'s existing precision floor for the whole
/// module: it closes the catastrophic case this bound was added for (silent,
/// unbounded rates from near-cancellation) without rejecting legitimate
/// hedged positions this module must still be able to attribute. Callers
/// needing a tight reconciliation tolerance must additionally check how
/// close their own buckets' net weights sit to this boundary, not merely
/// that [`grid_attribution()`] returned `Ok`.
const NET_WEIGHT_RELATIVE_TOLERANCE: f64 = 1e-6;

/// Relative reconciliation tolerance for inbound linked-period effects.
///
/// The floor is `1e-10` in ordinary return space. For near-cancelling,
/// long/short-generated effects whose gross magnitude is much larger than
/// their net active return, it scales with an overflow-safe L1 effect norm so
/// valid outputs from [`grid_attribution`] are not rejected solely because
/// cancellation amplified floating-point noise.
const LINK_RECONCILIATION_RELATIVE_TOLERANCE: f64 = 1e-10;

/// Compute a relative tolerance from an L1 norm without forming an
/// overflow-prone sum of absolute values.
fn scaled_l1_tolerance(values: &[f64]) -> (f64, f64, f64) {
    let scale = values
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    if scale == 0.0 {
        return (LINK_RECONCILIATION_RELATIVE_TOLERANCE, 0.0, 0.0);
    }

    let normalized_sum: f64 = values.iter().map(|value| value.abs() / scale).sum();
    let scaled_relative = LINK_RECONCILIATION_RELATIVE_TOLERANCE * scale;
    let tolerance = if normalized_sum > f64::MAX / scaled_relative {
        f64::MAX
    } else {
        scaled_relative * normalized_sum
    }
    .max(LINK_RECONCILIATION_RELATIVE_TOLERANCE);
    (tolerance, scale, normalized_sum)
}

/// One position (or pre-aggregated bucket) in a duration-cell x sector grid,
/// for one period and one side (portfolio or benchmark).
///
/// Weights are fractions of the whole side and must sum to `1.0` (within
/// `WEIGHT_TOLERANCE`) across all positions on that side. Returns are
/// decimals (`0.02` = 2 %).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GridPosition {
    /// Duration-cell label. Cell labels may come from
    /// [`crate::excess_return::duration_cell_label`], but any string key is
    /// accepted — this module has no dependency on how cells were built.
    pub cell: String,
    /// Sector bucket label within the cell.
    pub sector: String,
    /// Weight as a fraction of the whole side at period start (decimal).
    pub weight: f64,
    /// Realized total return for the period (decimal).
    pub total_return: f64,
}

/// Per-cell curve (duration-cell positioning) effect.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GridCellEffect {
    /// Duration-cell label.
    pub cell: String,
    /// Portfolio net weight in the cell, `x_t^P`.
    pub portfolio_weight: f64,
    /// Benchmark net weight in the cell, `x_t^B`.
    pub benchmark_weight: f64,
    /// Benchmark cell return, `r_t^B`. For a cell absent from the benchmark
    /// this is the out-of-benchmark fallback (`r_t^P`); see the module docs.
    pub benchmark_cell_return: f64,
    /// Curve effect `(x_t^P − x_t^B)(r_t^B − r^B)`.
    pub curve_effect: f64,
}

/// Per-(cell, sector) within-cell sector-allocation effect.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GridSectorEffect {
    /// Duration-cell label.
    pub cell: String,
    /// Sector label within the cell.
    pub sector: String,
    /// Allocation effect `x_t^P (z_st^P − z_st^B)(r_st^B − r_t^B)`.
    pub allocation_effect: f64,
}

/// Per-(cell, sector) security-selection effect.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GridSelectionEffect {
    /// Duration-cell label.
    pub cell: String,
    /// Sector label within the cell.
    pub sector: String,
    /// Selection effect `y_st^P (r_st^P − r_st^B)`.
    pub selection_effect: f64,
}

/// Single-period hierarchical grid attribution result.
///
/// This type is *input-reachable*: multi-period linking (Task 5) consumes a
/// slice of these, and the Python/WASM bindings deserialize them from JSON.
/// It therefore denies unknown fields, like the other inbound types in this
/// module, so a misspelled or stale key fails closed instead of being
/// silently dropped.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GridAttributionResult {
    /// Portfolio total return `r^P = Σ_t x_t^P r_t^P`.
    pub portfolio_return: f64,
    /// Benchmark total return `r^B = Σ_t x_t^B r_t^B`.
    pub benchmark_return: f64,
    /// Active return `r^P − r^B`.
    pub active_return: f64,
    /// Per-cell curve effects, in first-appearance order.
    pub curve_effects: Vec<GridCellEffect>,
    /// Per-(cell, sector) allocation effects, in first-appearance order
    /// (cells, then sectors within each cell).
    pub sector_effects: Vec<GridSectorEffect>,
    /// Per-(cell, sector) selection effects, in the same order as
    /// `sector_effects`.
    pub selection_effects: Vec<GridSelectionEffect>,
    /// Sum of the curve effects.
    pub total_curve: f64,
    /// Sum of the sector allocation effects.
    pub total_sector: f64,
    /// Sum of the selection effects.
    pub total_selection: f64,
}

/// Net/gross weight plus weighted-return accumulation for one bucket (a cell,
/// or a (cell, sector) pair) on one side.
///
/// `weight` is the *net* bucket weight (long minus short) and `abs_weight`
/// the gross weight; the pair distinguishes "bucket absent from this side"
/// (`abs_weight == 0`) from "bucket present with offsetting positions"
/// (`abs_weight > 0`, `weight` zero or, per [`NET_WEIGHT_RELATIVE_TOLERANCE`],
/// numerically near zero relative to `abs_weight`), which
/// [`check_zero_net_weight`] rejects.
#[derive(Clone, Copy, Default)]
struct BucketAgg {
    weight: f64,
    abs_weight: f64,
    weighted_return: f64,
}

impl BucketAgg {
    /// Fold one position's weight/return into this bucket.
    fn add(&mut self, weight: f64, total_return: f64) {
        self.weight += weight;
        self.abs_weight += weight.abs();
        self.weighted_return += weight * total_return;
    }

    /// Weighted-average return `weighted_return / weight`.
    ///
    /// Returns `0.0` only when the bucket carries no net weight, i.e. it is
    /// either absent from this side or was rejected by
    /// [`check_zero_net_weight`] before this is ever consulted for a real
    /// contribution. The bare (non-`.abs()`) comparison against `weight`
    /// elsewhere in this module is intentional: net-short buckets
    /// (`weight < 0`) are legal and must still divide through.
    fn rate(&self) -> f64 {
        if self.weight != 0.0 {
            self.weighted_return / self.weight
        } else {
            0.0
        }
    }
}

/// Per-(cell, sector) aggregates for both sides, keyed by sector within a
/// cell, in first-appearance order.
type SectorMap = IndexMap<String, (BucketAgg, BucketAgg)>;

/// Validate finiteness of a position's numeric fields.
fn validate_position(p: &GridPosition, side: &str) -> Result<()> {
    for (name, value) in [("weight", p.weight), ("total_return", p.total_return)] {
        if !value.is_finite() {
            return Err(Error::invalid_input(format!(
                "Grid attribution {side} input '{name}' for cell '{}' sector '{}' must be \
                 finite (got {value})",
                p.cell, p.sector
            )));
        }
    }
    Ok(())
}

/// Fail closed on a bucket that is present on a side but nets to zero, or to
/// a weight that is numerically near zero *relative to its own gross
/// weight* (a long/short pair, a hedge against a cash position in the same
/// bucket, that exactly or nearly offsets).
///
/// Such a bucket still contributes `Σ_j w_j r_j ≠ 0` to the side total. At
/// exact cancellation its per-unit rate `weighted_return / weight` is
/// undefined (`0 / 0`); at near-cancellation the rate is *defined* but grows
/// without bound as the net weight shrinks toward zero, so every effect
/// derived from it can blow up to a numerically meaningless magnitude while
/// `active_return` still ties out against performance data, silently
/// breaking the telescoping identity (see the module docs) without ever
/// producing a `NaN` or infinity that a finiteness check would catch. An
/// exact-equality check (`agg.weight == 0.0`) misses this near-cancellation
/// regime entirely, so this compares the ratio `|weight| / abs_weight`
/// against [`NET_WEIGHT_RELATIVE_TOLERANCE`] instead — a relative bound so
/// rescaling all weights uniformly (e.g. percent vs. decimal) does not
/// change whether it fires.
fn check_zero_net_weight(bucket: &str, agg: &BucketAgg, side_name: &str) -> Result<()> {
    if agg.abs_weight > 0.0 && agg.weight.abs() <= NET_WEIGHT_RELATIVE_TOLERANCE * agg.abs_weight {
        return Err(Error::invalid_input(format!(
            "{side_name} bucket '{bucket}' has offsetting positions netting to a weight \
             ({}) that is zero, or numerically near zero, relative to its gross weight ({}): \
             a bucket whose |net weight| does not exceed {NET_WEIGHT_RELATIVE_TOLERANCE} times \
             its gross weight cannot be attributed because its per-unit rate (weighted return \
             / weight) is undefined or numerically explosive. Split the offsetting positions \
             into distinct buckets, or net them into a single position with a net weight well \
             clear of that relative bound.",
            agg.weight, agg.abs_weight
        )));
    }
    Ok(())
}

/// Accumulate one side's positions into the cell x sector grid.
///
/// Returns the side's total return `Σ_j w_j r_j` and validates that the
/// side's weights sum to `1.0` within `WEIGHT_TOLERANCE`.
fn aggregate_side(
    positions: &[GridPosition],
    cells: &mut IndexMap<String, SectorMap>,
    is_portfolio: bool,
) -> Result<f64> {
    let (side, side_name) = if is_portfolio {
        ("portfolio", "Portfolio")
    } else {
        ("benchmark", "Benchmark")
    };
    let mut sum_w = NeumaierAccumulator::new();
    let mut sum_r = NeumaierAccumulator::new();

    for p in positions {
        validate_position(p, side)?;
        sum_w.add(p.weight);
        sum_r.add(p.weight * p.total_return);

        let sector_map = cells.entry(p.cell.clone()).or_default();
        let entry = sector_map.entry(p.sector.clone()).or_default();
        let agg = if is_portfolio {
            &mut entry.0
        } else {
            &mut entry.1
        };
        agg.add(p.weight, p.total_return);
    }

    let total_w = sum_w.total();
    if (total_w - 1.0).abs() > WEIGHT_TOLERANCE {
        return Err(Error::invalid_input(format!(
            "{side_name} weights must sum to 1.0 (got {total_w})"
        )));
    }

    Ok(sum_r.total())
}

/// Compute a single-period hierarchical duration-cell x sector grid
/// attribution (Dynkin, Hyman & Vankudre 1998, Appendix A).
///
/// Decomposes the active return into a curve effect per duration cell, a
/// within-cell sector-allocation effect, and a security-selection residual
/// per (cell, sector) — see the module docs for the exact formulas, the
/// out-of-benchmark fallback convention, and a proof that the three sums
/// telescope to the active return for well-conditioned inputs (see the
/// module-level "Telescoping identity" section for how the reconciliation
/// degrades as a bucket's net weight approaches the fail-closed guard's own
/// boundary).
///
/// Dynkin, L., Hyman, J., & Vankudre, P. (1998). "Attribution of Portfolio
/// Performance Relative to an Index." Lehman Brothers Fixed Income Research,
/// March 1998, Appendix A.
///
/// # Arguments
///
/// * `portfolio` - Portfolio position/bucket snapshots; weights must sum to
///   `1.0` within `±1e-6`.
/// * `benchmark` - Benchmark snapshots; weights must sum to `1.0` within
///   `±1e-6`.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if any weight or return is non-finite,
/// either side's weights don't sum to `1.0` (±1e-6), or a (cell) or
/// (cell, sector) bucket has positions on a side but nets to a weight that
/// is zero, or numerically near zero (within 1e-6 relative to its own gross
/// weight).
///
/// # Examples
///
/// ```rust
/// use finstack_quant_portfolio::grid_attribution::{grid_attribution, GridPosition};
///
/// let pos = |cell: &str, sector: &str, weight: f64, total_return: f64| GridPosition {
///     cell: cell.into(),
///     sector: sector.into(),
///     weight,
///     total_return,
/// };
/// let portfolio = vec![
///     pos("0.0-3.0", "GOVT", 0.5, 0.012),
///     pos("0.0-3.0", "CORP", 0.5, 0.020),
/// ];
/// let benchmark = vec![
///     pos("0.0-3.0", "GOVT", 0.6, 0.010),
///     pos("0.0-3.0", "CORP", 0.4, 0.018),
/// ];
/// let result = grid_attribution(&portfolio, &benchmark)?;
/// let reconstructed = result.total_curve + result.total_sector + result.total_selection;
/// assert!((reconstructed - result.active_return).abs() < 1e-12);
/// # Ok::<(), finstack_quant_portfolio::Error>(())
/// ```
pub fn grid_attribution(
    portfolio: &[GridPosition],
    benchmark: &[GridPosition],
) -> Result<GridAttributionResult> {
    // Union of cells (and, within each, sectors) in first-appearance order:
    // portfolio positions are scanned before benchmark positions.
    let mut cells: IndexMap<String, SectorMap> = IndexMap::new();
    let portfolio_return = aggregate_side(portfolio, &mut cells, true)?;
    let benchmark_return = aggregate_side(benchmark, &mut cells, false)?;
    let active_return = portfolio_return - benchmark_return;

    // Cell-level totals, derived by summing the per-sector aggregates within
    // each cell. Iteration order matches `cells` exactly, since it is built
    // by iterating `cells` itself.
    let mut cell_totals: IndexMap<String, (BucketAgg, BucketAgg)> = IndexMap::new();
    for (cell, sector_map) in &cells {
        let mut p = BucketAgg::default();
        let mut b = BucketAgg::default();
        for (_, (sp, sb)) in sector_map {
            p.weight += sp.weight;
            p.abs_weight += sp.abs_weight;
            p.weighted_return += sp.weighted_return;
            b.weight += sb.weight;
            b.abs_weight += sb.abs_weight;
            b.weighted_return += sb.weighted_return;
        }
        cell_totals.insert(cell.clone(), (p, b));
    }

    // Fail closed on zero-net-weight buckets before computing any effects,
    // at both the cell level and the (cell, sector) level.
    for (cell, (p, b)) in &cell_totals {
        check_zero_net_weight(cell, p, "Portfolio")?;
        check_zero_net_weight(cell, b, "Benchmark")?;
    }
    for (cell, sector_map) in &cells {
        for (sector, (sp, sb)) in sector_map {
            let bucket = format!("{cell}/{sector}");
            check_zero_net_weight(&bucket, sp, "Portfolio")?;
            check_zero_net_weight(&bucket, sb, "Benchmark")?;
        }
    }

    let mut total_curve = NeumaierAccumulator::new();
    let mut total_sector = NeumaierAccumulator::new();
    let mut total_selection = NeumaierAccumulator::new();
    let mut curve_effects = Vec::with_capacity(cell_totals.len());
    let mut sector_effects = Vec::new();
    let mut selection_effects = Vec::new();

    for ((cell, sector_map), (_, (p, b))) in cells.iter().zip(cell_totals.iter()) {
        let x_p = p.weight;
        let x_b = b.weight;
        // A cell absent from the benchmark has zero net *and* zero gross
        // weight there (the zero-net-weight check above already ruled out
        // "present but netting to zero"), so `x_b == 0.0` is exactly the
        // out-of-benchmark case from the module docs.
        let cell_absent_from_benchmark = x_b == 0.0;

        let r_t_p = p.rate();
        let r_t_b = if cell_absent_from_benchmark {
            r_t_p
        } else {
            b.rate()
        };

        let curve_effect = (x_p - x_b) * (r_t_b - benchmark_return);
        total_curve.add(curve_effect);
        curve_effects.push(GridCellEffect {
            cell: cell.clone(),
            portfolio_weight: x_p,
            benchmark_weight: x_b,
            benchmark_cell_return: r_t_b,
            curve_effect,
        });

        for (sector, (sp, sb)) in sector_map {
            let z_p = if x_p != 0.0 { sp.weight / x_p } else { 0.0 };
            let r_st_p = sp.rate();

            let (z_b, r_st_b) = if cell_absent_from_benchmark {
                (z_p, r_st_p)
            } else if sb.weight == 0.0 {
                // (cell, sector) absent from the benchmark, but the cell is
                // present: z_st^B = 0 is already correct (no fallback
                // needed there); only the return needs a fallback.
                (0.0, r_st_p)
            } else {
                (sb.weight / x_b, sb.rate())
            };

            let allocation_effect = x_p * (z_p - z_b) * (r_st_b - r_t_b);
            let selection_effect = sp.weight * (r_st_p - r_st_b);

            total_sector.add(allocation_effect);
            total_selection.add(selection_effect);

            sector_effects.push(GridSectorEffect {
                cell: cell.clone(),
                sector: sector.clone(),
                allocation_effect,
            });
            selection_effects.push(GridSelectionEffect {
                cell: cell.clone(),
                sector: sector.clone(),
                selection_effect,
            });
        }
    }

    Ok(GridAttributionResult {
        portfolio_return,
        benchmark_return,
        active_return,
        curve_effects,
        sector_effects,
        selection_effects,
        total_curve: total_curve.total(),
        total_sector: total_sector.total(),
        total_selection: total_selection.total(),
    })
}

/// Multi-period Carino-linked hierarchical grid attribution.
///
/// Links only the three top-level effects (`total_curve`, `total_sector`,
/// `total_selection`) across periods, so their sum reconstructs the
/// geometrically compounded active return exactly. This is a deliberate
/// scope decision: per-cell / per-(cell, sector) multi-period linking is
/// deferred. Because the Carino scale `k_t / K` (see `carino_coefficient`)
/// is a single per-period scalar multiplier, extending this to link
/// `curve_effects`, `sector_effects`, and `selection_effects` element-wise —
/// the way [`crate::fi_attribution::campisi_carino_link`] links its five
/// per-sector effects — is a mechanical extension if ever needed: it would
/// only require validating consistent cell/sector ordering across periods,
/// mirroring [`crate::brinson::carino_link`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct GridCarinoLinkedResult {
    /// Per-period single-period grid attribution results, in chronological
    /// order.
    pub periods: Vec<GridAttributionResult>,
    /// Geometrically compounded portfolio return, `∏_t (1 + r_p,t) − 1`.
    pub portfolio_return_compounded: f64,
    /// Geometrically compounded benchmark return, `∏_t (1 + r_b,t) − 1`.
    pub benchmark_return_compounded: f64,
    /// Sum of per-period Carino-scaled curve effects.
    ///
    /// `linked_curve + linked_sector + linked_selection` reconstructs
    /// `portfolio_return_compounded − benchmark_return_compounded` exactly.
    pub linked_curve: f64,
    /// Sum of per-period Carino-scaled sector allocation effects.
    pub linked_sector: f64,
    /// Sum of per-period Carino-scaled selection effects.
    pub linked_selection: f64,
}

/// Validate one externally reachable single-period result before linking.
fn validate_grid_link_period(period: &GridAttributionResult, index: usize) -> Result<()> {
    for (name, value) in [
        ("portfolio_return", period.portfolio_return),
        ("benchmark_return", period.benchmark_return),
        ("active_return", period.active_return),
        ("total_curve", period.total_curve),
        ("total_sector", period.total_sector),
        ("total_selection", period.total_selection),
    ] {
        if !value.is_finite() {
            if matches!(
                name,
                "portfolio_return" | "benchmark_return" | "active_return"
            ) {
                return Err(Error::invalid_input(format!(
                    "Grid Carino linking requires finite period returns: \
                     period[{index}].{name} must be finite (got {value})"
                )));
            }
            return Err(Error::invalid_input(format!(
                "Grid Carino period[{index}].{name} must be finite (got {value})"
            )));
        }
    }

    let expected_active = period.portfolio_return - period.benchmark_return;
    if !expected_active.is_finite() {
        return Err(Error::invalid_input(format!(
            "Grid Carino period[{index}] portfolio_return - benchmark_return must be finite"
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
            "Grid Carino period[{index}] active-return residual must be finite"
        )));
    }
    if active_residual.abs() > return_tolerance {
        return Err(Error::invalid_input(format!(
            "Grid Carino period[{index}].active_return ({}) does not agree with \
             portfolio_return - benchmark_return ({expected_active}) within return-scale \
             tolerance {return_tolerance}",
            period.active_return
        )));
    }

    let effect_values = [
        period.total_curve,
        period.total_sector,
        period.total_selection,
    ];
    let (tolerance, effect_scale, normalized_l1) = scaled_l1_tolerance(&effect_values);
    let mut effects = NeumaierAccumulator::new();
    for value in effect_values {
        effects.add(value);
    }
    let effect_total = effects.total();
    let reconciliation_residual = effect_total - period.active_return;
    if !reconciliation_residual.is_finite() {
        return Err(Error::invalid_input(format!(
            "Grid Carino period[{index}] effect reconciliation residual must be finite"
        )));
    }
    if reconciliation_residual.abs() > tolerance {
        return Err(Error::invalid_input(format!(
            "Grid Carino period[{index}] effect totals sum to {effect_total}, which does not \
             reconcile to active_return {} within scale-aware tolerance {tolerance} \
             (scaled L1 effect scale {effect_scale}, normalized L1 sum {normalized_l1})",
            period.active_return
        )));
    }

    Ok(())
}

/// Apply Carino (1999) smoothing to a sequence of per-period hierarchical
/// grid attribution results ([`grid_attribution()`]) so the three top-level
/// arithmetic effects reconstruct the *geometrically compounded* active
/// return exactly, mirroring [`crate::brinson::carino_link`] and
/// [`crate::fi_attribution::campisi_carino_link`].
///
/// Each period contributes its three top-level effects (`total_curve`,
/// `total_sector`, `total_selection`) scaled by `k_t / K`, where `k_t` is the
/// single-period Carino coefficient and `K` is the coefficient for the
/// compounded portfolio/benchmark returns over the whole horizon (see
/// `carino_coefficient`). Mixed period lengths are inherent: there is no
/// `period_years` input, so periods of any duration or position-count may be
/// linked together.
///
/// Carino, D. R. (1999). "Combining Attribution Effects Over Time." *Journal
/// of Performance Measurement*, Winter 1999/2000, 5-14.
///
///
/// # Arguments
///
/// * `periods` - Chronologically ordered single-period grid attribution
///   results.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if `periods` is empty; any consumed return
/// or top-level effect is non-finite; a declared `active_return` does not
/// agree with `portfolio_return - benchmark_return`; the three effect totals
/// do not reconcile to `active_return` within an overflow-safe, scale-aware
/// L1 tolerance; any return identity or reconciliation residual is
/// non-finite; or any per-period or compounded return is at or below −100 %
/// (the Carino formula's domain; see `carino_coefficient`).
///
/// # Examples
///
/// ```rust
/// use finstack_quant_portfolio::grid_attribution::{
///     grid_attribution, grid_carino_link, GridPosition,
/// };
///
/// let pos = |cell: &str, sector: &str, weight: f64, total_return: f64| GridPosition {
///     cell: cell.into(),
///     sector: sector.into(),
///     weight,
///     total_return,
/// };
/// let portfolio = vec![pos("0.0-3.0", "GOVT", 1.0, 0.012)];
/// let benchmark = vec![pos("0.0-3.0", "GOVT", 1.0, 0.010)];
/// let period = grid_attribution(&portfolio, &benchmark)?;
///
/// let linked = grid_carino_link(&[period.clone(), period])?;
/// let sum = linked.linked_curve + linked.linked_sector + linked.linked_selection;
/// let active = linked.portfolio_return_compounded - linked.benchmark_return_compounded;
/// assert!((sum - active).abs() < 1e-12);
/// # Ok::<(), finstack_quant_portfolio::Error>(())
/// ```
pub fn grid_carino_link(periods: &[GridAttributionResult]) -> Result<GridCarinoLinkedResult> {
    if periods.is_empty() {
        return Err(Error::invalid_input(
            "Grid Carino linking requires at least one period",
        ));
    }

    for (index, p) in periods.iter().enumerate() {
        validate_grid_link_period(p, index)?;
    }

    let effect_periods: Vec<CarinoPeriod<3>> = periods
        .iter()
        .map(|p| CarinoPeriod {
            portfolio_return: p.portfolio_return,
            benchmark_return: p.benchmark_return,
            rows: vec![[p.total_curve, p.total_sector, p.total_selection]],
        })
        .collect();
    let linked = carino_link_effects(&effect_periods)?;
    let [linked_curve, linked_sector, linked_selection] = linked.rows[0];

    Ok(GridCarinoLinkedResult {
        periods: periods.to_vec(),
        portfolio_return_compounded: linked.portfolio_return_compounded,
        benchmark_return_compounded: linked.benchmark_return_compounded,
        linked_curve,
        linked_sector,
        linked_selection,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(cell: &str, sector: &str, weight: f64, total_return: f64) -> GridPosition {
        GridPosition {
            cell: cell.into(),
            sector: sector.into(),
            weight,
            total_return,
        }
    }

    fn close(actual: f64, expected: f64, tol: f64, label: &str) {
        assert!(
            (actual - expected).abs() < tol,
            "{label}: got {actual}, expected {expected} (tol {tol})"
        );
    }

    /// Hand-verified golden fixture (re-derived twice by hand):
    ///
    /// ```text
    /// Benchmark: C1 "0.0-3.0": GOVT 0.30@0.010, CORP 0.20@0.020  => x1=0.50, r1=0.014
    ///            C2 "3.0-6.0": GOVT 0.25@0.030, CORP 0.25@0.040  => x2=0.50, r2=0.035
    ///            r^B = 0.0245
    /// Portfolio: C1: GOVT 0.20@0.012, CORP 0.20@0.025            => x1=0.40
    ///            C2: GOVT 0.30@0.028, CORP 0.30@0.045            => x2=0.60
    ///            r^P = 0.0293, active = 0.0048
    /// curve:   C1 (0.40-0.50)(0.014-0.0245)=+0.00105 ; C2 (0.60-0.50)(0.035-0.0245)=+0.00105
    /// sector:  C1 0.40*[(0.5-0.6)(0.010-0.014)+(0.5-0.4)(0.020-0.014)] = 0.0004 ; C2 = 0
    /// select:  0.20(0.002)+0.20(0.005)+0.30(-0.002)+0.30(0.005) = 0.0023
    /// totals:  0.0021 + 0.0004 + 0.0023 = 0.0048 (equals active return)
    /// ```
    #[test]
    fn grid_attribution_matches_hand_derived_golden() {
        let p = |cell: &str, sector: &str, w: f64, r: f64| GridPosition {
            cell: cell.into(),
            sector: sector.into(),
            weight: w,
            total_return: r,
        };
        let portfolio = vec![
            p("0.0-3.0", "GOVT", 0.20, 0.012),
            p("0.0-3.0", "CORP", 0.20, 0.025),
            p("3.0-6.0", "GOVT", 0.30, 0.028),
            p("3.0-6.0", "CORP", 0.30, 0.045),
        ];
        let benchmark = vec![
            p("0.0-3.0", "GOVT", 0.30, 0.010),
            p("0.0-3.0", "CORP", 0.20, 0.020),
            p("3.0-6.0", "GOVT", 0.25, 0.030),
            p("3.0-6.0", "CORP", 0.25, 0.040),
        ];
        let r = grid_attribution(&portfolio, &benchmark).expect("valid golden inputs");
        let c = |a: f64, b: f64| assert!((a - b).abs() < 1e-12, "{a} vs {b}");
        c(r.portfolio_return, 0.0293);
        c(r.benchmark_return, 0.0245);
        c(r.active_return, 0.0048);
        c(r.total_curve, 0.0021);
        c(r.total_sector, 0.0004);
        c(r.total_selection, 0.0023);
        c(r.curve_effects[0].curve_effect, 0.00105);
        c(r.curve_effects[1].curve_effect, 0.00105);
        c(
            r.total_curve + r.total_sector + r.total_selection,
            r.active_return,
        );

        // Reconciliation-by-subtraction guard: a `total_*` field back-solved
        // from `active_return` and the *other* totals would still satisfy
        // every assertion above (the identity holds by construction), while
        // silently decoupling from the effects vector it is supposed to
        // summarize. Tie each total directly to an independent sum over its
        // own emitted `Vec`, not to the cross-total identity.
        let sum_curve: f64 = r.curve_effects.iter().map(|e| e.curve_effect).sum();
        let sum_sector: f64 = r.sector_effects.iter().map(|e| e.allocation_effect).sum();
        let sum_selection: f64 = r.selection_effects.iter().map(|e| e.selection_effect).sum();
        assert_eq!(
            r.total_curve, sum_curve,
            "total_curve must equal the sum of curve_effects, not a value back-solved from \
             active_return and the other totals"
        );
        assert_eq!(
            r.total_sector, sum_sector,
            "total_sector must equal the sum of sector_effects"
        );
        assert_eq!(
            r.total_selection, sum_selection,
            "total_selection must equal the sum of selection_effects"
        );
    }

    /// A cell the portfolio holds but the benchmark entirely lacks falls
    /// back to `r_t^B := r_t^P` and, for every sector within it,
    /// `z_st^B := z_st^P` / `r_st^B := r_st^P`: every sector's selection *and*
    /// allocation effect must be exactly zero and the whole effect must
    /// surface as curve, while the identity still closes exactly.
    ///
    /// Cell "B" deliberately holds *two* sectors (IG, HY) with different
    /// returns, not one. With a single sector, `z_p = 1` trivially forces
    /// `allocation_effect == 0` for *any* value of `z_b` (`0`, `z_p`, or
    /// anything else), so a single-sector fixture cannot distinguish the
    /// correct `z_st^B := z_st^P` fallback from the wrong `z_st^B := 0`
    /// fallback. With two sectors of unequal share, `z_st^B := 0` produces
    /// non-zero, sign-opposite per-sector allocations that merely cancel in
    /// the cell total — so each sector's allocation must be checked
    /// individually, not just the reconciliation identity.
    #[test]
    fn out_of_benchmark_cell_flows_to_curve_with_zero_selection() {
        let portfolio = vec![
            pos("A", "GOVT", 0.5, 0.02),
            pos("B", "IG", 0.3, 0.01), // benchmark has no "B" cell at all
            pos("B", "HY", 0.2, 0.06),
        ];
        let benchmark = vec![pos("A", "GOVT", 1.0, 0.01)];

        let r = grid_attribution(&portfolio, &benchmark).expect("valid inputs");

        let reconstructed = r.total_curve + r.total_sector + r.total_selection;
        close(reconstructed, r.active_return, 1e-12, "reconciliation");

        let cell_b = r
            .curve_effects
            .iter()
            .find(|e| e.cell == "B")
            .expect("cell B present");
        close(
            cell_b.benchmark_weight,
            0.0,
            1e-15,
            "cell B benchmark weight",
        );
        // r_t^P = (0.3*0.01 + 0.2*0.06) / 0.5 = 0.03.
        close(
            cell_b.benchmark_cell_return,
            0.03,
            1e-12,
            "cell B r_t^B fallback = r_t^P",
        );
        // (0.5 - 0.0) * (0.03 - 0.01) = 0.01, the entire out-of-benchmark
        // effect surfaces as curve.
        close(cell_b.curve_effect, 0.01, 1e-12, "cell B curve effect");

        for sector in ["IG", "HY"] {
            let sel = r
                .selection_effects
                .iter()
                .find(|e| e.cell == "B" && e.sector == sector)
                .unwrap_or_else(|| panic!("selection entry for B/{sector}"));
            close(
                sel.selection_effect,
                0.0,
                1e-15,
                &format!("out-of-benchmark selection is zero for B/{sector}"),
            );

            let alloc = r
                .sector_effects
                .iter()
                .find(|e| e.cell == "B" && e.sector == sector)
                .unwrap_or_else(|| panic!("allocation entry for B/{sector}"));
            // z_st^B := z_st^P for every sector in an out-of-benchmark cell,
            // so (z_p - z_b) == 0 regardless of the sector's own share —
            // this must hold individually for IG (share 0.6) and HY (share
            // 0.4), not just cancel out in the cell total.
            close(
                alloc.allocation_effect,
                0.0,
                1e-15,
                &format!("out-of-benchmark allocation is zero for B/{sector}"),
            );
        }
    }

    /// A (cell, sector) bucket absent from the benchmark *while the cell
    /// itself is present* (`x_t^B != 0`, `y_st^B == 0`) takes the narrower
    /// fallback: `z_st^B` is naturally `0` (no fallback needed there), but
    /// `r_st^B := r_st^P`.
    ///
    /// This value is structurally invisible to any reconciliation-only
    /// check: with `z_st^B = 0`, `r_st^B` enters `selection_effect` with
    /// coefficient `-y_st^P` and `allocation_effect` with coefficient
    /// `+x_t^P * z_st^P = +y_st^P` — equal and opposite, so it cancels
    /// exactly out of every total regardless of what `r_st^B` fallback value
    /// is used. Both effects must therefore be pinned individually.
    ///
    /// Fixture and hand-derived values from the review: cell A holds
    /// portfolio GOVT 0.6@0.01 and HY 0.4@0.05; benchmark holds only GOVT
    /// 1.0@0.012 (so cell A is present on both sides, but HY is
    /// benchmark-absent). `r_t^B = 0.012`, `z_p^HY = 0.4`, `z_b^HY = 0`,
    /// `r_st^P_HY = 0.05`, fallback `r_st^B_HY := 0.05`:
    /// `allocation_effect = 1.0 * 0.4 * (0.05 - 0.012) = 0.0152`,
    /// `selection_effect = 0.4 * (0.05 - 0.05) = 0`.
    #[test]
    fn portfolio_only_sector_in_benchmark_present_cell_pins_allocation_and_selection() {
        let portfolio = vec![pos("A", "GOVT", 0.6, 0.01), pos("A", "HY", 0.4, 0.05)];
        let benchmark = vec![pos("A", "GOVT", 1.0, 0.012)];

        let r = grid_attribution(&portfolio, &benchmark).expect("valid inputs");

        let alloc_hy = r
            .sector_effects
            .iter()
            .find(|e| e.cell == "A" && e.sector == "HY")
            .expect("allocation entry for A/HY");
        let sel_hy = r
            .selection_effects
            .iter()
            .find(|e| e.cell == "A" && e.sector == "HY")
            .expect("selection entry for A/HY");

        close(
            sel_hy.selection_effect,
            0.0,
            1e-15,
            "selection is zero for a benchmark-absent sector (r_st^B fallback = r_st^P)",
        );
        close(
            alloc_hy.allocation_effect,
            0.0152,
            1e-12,
            "hand-derived allocation effect for A/HY",
        );

        let reconstructed = r.total_curve + r.total_sector + r.total_selection;
        close(reconstructed, r.active_return, 1e-12, "reconciliation");
    }

    /// Cell/sector labels must appear in first-appearance order: portfolio
    /// positions scanned before benchmark positions, both at the cell level
    /// and, within each cell, at the sector level. "X" is present on both
    /// sides (portfolio sectors S1, S2 first, then benchmark-only S9
    /// appended); "Y" is portfolio-only; "Z" is benchmark-only and must sort
    /// last among cells.
    #[test]
    fn ordering_is_first_appearance_portfolio_before_benchmark_only() {
        let portfolio = vec![
            pos("X", "S1", 0.3, 0.01),
            pos("X", "S2", 0.2, 0.02),
            pos("Y", "S1", 0.5, 0.03),
        ];
        let benchmark = vec![
            pos("X", "S1", 0.35, 0.015),
            pos("X", "S9", 0.05, 0.025),
            pos("Z", "S3", 0.6, 0.035),
        ];

        let r = grid_attribution(&portfolio, &benchmark).expect("valid ordering fixture");

        let cell_labels: Vec<&str> = r.curve_effects.iter().map(|e| e.cell.as_str()).collect();
        assert_eq!(
            cell_labels,
            vec!["X", "Y", "Z"],
            "cells must be first-appearance order, portfolio before benchmark-only"
        );

        let sector_labels: Vec<(&str, &str)> = r
            .sector_effects
            .iter()
            .map(|e| (e.cell.as_str(), e.sector.as_str()))
            .collect();
        assert_eq!(
            sector_labels,
            vec![
                ("X", "S1"),
                ("X", "S2"),
                ("X", "S9"),
                ("Y", "S1"),
                ("Z", "S3"),
            ],
            "sectors must be first-appearance within each cell, portfolio before benchmark-only"
        );

        let selection_labels: Vec<(&str, &str)> = r
            .selection_effects
            .iter()
            .map(|e| (e.cell.as_str(), e.sector.as_str()))
            .collect();
        assert_eq!(
            selection_labels, sector_labels,
            "selection_effects must share sector_effects' ordering"
        );
    }

    /// A cell can net to exactly zero weight even when *every individual
    /// sector within it* nets to a non-zero weight: GOVT +0.5 and CORP -0.5
    /// in the same cell each have a well-defined per-sector rate, but the
    /// cell-level aggregate is exactly zero with non-zero gross weight. This
    /// is a distinct failure mode from the (cell, sector)-level guard (which
    /// this fixture does not trip) and requires its own cell-level check.
    #[test]
    fn cell_level_zero_net_weight_with_nonzero_sector_nets_fails_closed() {
        let portfolio = vec![
            pos("A", "GOVT", 0.5, 0.01),
            pos("A", "CORP", -0.5, 0.02),
            pos("B", "X", 1.0, 0.03),
        ];
        let benchmark = vec![pos("B", "X", 1.0, 0.025)];

        let err = grid_attribution(&portfolio, &benchmark)
            .expect_err("cell netting to zero despite non-zero sector nets must be rejected");
        let message = err.to_string();
        assert!(
            message.contains("bucket 'A'"),
            "error must name the cell: {message}"
        );
        assert!(
            message.contains("Portfolio"),
            "error must name the side: {message}"
        );
    }

    /// C1 GOVT nets to exactly zero on the portfolio side (a +0.5/-0.5
    /// offsetting pair) despite carrying real positions; the per-unit rate
    /// is undefined and must be rejected, naming the cell, sector and side.
    #[test]
    fn zero_net_weight_bucket_with_positions_fails_closed() {
        let portfolio = vec![
            pos("0.0-3.0", "GOVT", 0.5, 0.01),
            pos("0.0-3.0", "GOVT", -0.5, 0.02),
            pos("0.0-3.0", "CORP", 1.0, 0.03),
        ];
        let benchmark = vec![pos("0.0-3.0", "CORP", 1.0, 0.025)];

        let err = grid_attribution(&portfolio, &benchmark)
            .expect_err("zero-net-weight bucket must be rejected");
        let message = err.to_string();
        assert!(
            message.contains("0.0-3.0"),
            "error must name the cell: {message}"
        );
        assert!(
            message.contains("GOVT"),
            "error must name the sector: {message}"
        );
        assert!(
            message.contains("Portfolio"),
            "error must name the side: {message}"
        );
    }

    /// Executed regression for the whole-branch review's Important-1
    /// finding: the zero-net-weight guard must reject *near* cancellation,
    /// not only *exact* cancellation. A single exact-equality test
    /// (`agg.weight == 0.0`) lets a benchmark cell net to an arbitrarily
    /// small, non-zero weight through unrejected; [`BucketAgg::rate`] then
    /// divides by that near-zero net weight, so the rate — and every effect
    /// derived from it — grows without bound as the net weight shrinks,
    /// silently breaking the telescoping identity while reporting no error.
    ///
    /// Benchmark cell "X" holds GOVT `+0.5@2%` and CORP `-(0.5 - eps)@1%`
    /// (two sectors, so this trips the *cell*-level check specifically, not
    /// either sector's own well-defined per-sector rate — mirroring
    /// `cell_level_zero_net_weight_with_nonzero_sector_nets_fails_closed`'s
    /// fixture shape). Net weight is `eps`, gross weight is `1 - eps`, so the
    /// ratio is `eps / (1 - eps) ~= eps`. Cell "Y" carries the rest of the
    /// benchmark side so both fixtures still sum to `1.0`.
    ///
    /// Pins the boundary in both directions: `eps = 1e-8` (ratio far below
    /// `NET_WEIGHT_RELATIVE_TOLERANCE`, matching the coordinator's own
    /// executed evidence of a curve effect on the order of `2.5e5`) must be
    /// rejected, naming the cell and side; `eps = 1e-2` (ratio comfortably
    /// above the tolerance) must be accepted and produce a bounded, sane
    /// curve effect rather than a numerically exploded one.
    #[test]
    fn near_zero_net_weight_bucket_fails_closed_before_rate_blows_up() {
        let fixture = |eps: f64| {
            let portfolio = vec![pos("X", "GOVT", 0.5, 0.018), pos("Y", "GOVT", 0.5, 0.02)];
            let benchmark = vec![
                pos("X", "GOVT", 0.5, 0.02),
                pos("X", "CORP", -(0.5 - eps), 0.01),
                pos("Y", "GOVT", 1.0 - eps, 0.015),
            ];
            (portfolio, benchmark)
        };

        // eps = 1e-8: net/gross ratio ~1e-8, far below the 1e-6 relative
        // bound. Must be rejected, naming the cell and the side.
        let (portfolio, benchmark) = fixture(1e-8);
        let err = grid_attribution(&portfolio, &benchmark).expect_err(
            "near-zero net weight (ratio ~1e-8) must be rejected, not silently divided through",
        );
        let message = err.to_string();
        assert!(
            message.contains("bucket 'X'"),
            "error must name the cell: {message}"
        );
        assert!(
            message.contains("Benchmark"),
            "error must name the side: {message}"
        );

        // eps = 1e-2: net/gross ratio ~1e-2, comfortably above the bound.
        // Must be accepted, and the resulting curve effect must stay within
        // a sane magnitude — not the ~2.5e5-scale blowup the coordinator
        // measured at eps=1e-8 — confirming the guard's boundary rejects
        // genuine near-cancellation without over-rejecting a legitimate,
        // merely small, net weight.
        let (portfolio, benchmark) = fixture(1e-2);
        let r = grid_attribution(&portfolio, &benchmark)
            .expect("net weight well clear of the relative bound must be accepted");
        let cell_x = r
            .curve_effects
            .iter()
            .find(|e| e.cell == "X")
            .expect("cell X present");
        assert!(
            cell_x.curve_effect.abs() < 10.0,
            "a legitimate small-but-not-near-zero net weight must not produce an exploded \
             curve effect: {}",
            cell_x.curve_effect
        );
    }

    /// A bucket netting to a non-zero, negative (net-short) weight must be
    /// attributed by dividing through the net weight, not zeroed. Pins the
    /// non-`.abs()` guard in [`BucketAgg::rate`] (the Campisi F4 lesson:
    /// dropping this silently zeroes every net-short bucket).
    #[test]
    fn net_short_bucket_is_attributed_not_zeroed() {
        let portfolio = vec![pos("A", "CORE", 1.2, 0.02), pos("A", "SHORT", -0.2, 0.05)];
        let benchmark = vec![pos("A", "CORE", 0.9, 0.015), pos("A", "SHORT", 0.1, 0.04)];

        let r = grid_attribution(&portfolio, &benchmark).expect("net-short is legal");

        let alloc_short = r
            .sector_effects
            .iter()
            .find(|e| e.cell == "A" && e.sector == "SHORT")
            .expect("SHORT allocation entry");
        let sel_short = r
            .selection_effects
            .iter()
            .find(|e| e.cell == "A" && e.sector == "SHORT")
            .expect("SHORT selection entry");

        // Hand-derived: x_p = x_b = 1.0 (single cell), z_p^SHORT = -0.2,
        // z_b^SHORT = 0.1, r_t^B = 0.0175, r_st^B^SHORT = 0.04.
        assert!(
            alloc_short.allocation_effect.abs() > 1e-6,
            "must not be silently zeroed"
        );
        assert!(
            sel_short.selection_effect.abs() > 1e-6,
            "must not be silently zeroed"
        );
        close(
            alloc_short.allocation_effect,
            -0.00675,
            1e-12,
            "SHORT allocation effect",
        );
        close(
            sel_short.selection_effect,
            -0.002,
            1e-12,
            "SHORT selection effect",
        );

        let reconstructed = r.total_curve + r.total_sector + r.total_selection;
        close(reconstructed, r.active_return, 1e-12, "reconciliation");
    }

    /// Three cells x three sectors, irregular weights, a negative return,
    /// and two one-sided (cell, sector) buckets (one portfolio-only, one
    /// benchmark-only) within cells that are otherwise present on both
    /// sides. The telescoping identity must still close to float precision.
    #[test]
    fn adversarial_three_cell_fixture_reconciles_algebraically() {
        let portfolio = vec![
            pos("0-2", "GOVT", 0.15, 0.01),
            pos("0-2", "IG", 0.10, -0.02), // negative return
            pos("2-5", "GOVT", 0.20, 0.015),
            pos("2-5", "HY", 0.05, 0.04), // portfolio-only bucket in "2-5"
            pos("5-10", "IG", 0.30, 0.03), // portfolio-only bucket in "5-10"
            pos("5-10", "HY", 0.20, 0.06),
        ];
        let benchmark = vec![
            pos("0-2", "GOVT", 0.25, 0.008),
            pos("0-2", "IG", 0.15, -0.01),
            pos("2-5", "GOVT", 0.10, 0.012),
            pos("2-5", "IG", 0.10, 0.02), // benchmark-only bucket in "2-5"
            pos("5-10", "HY", 0.40, 0.05),
        ];

        let r = grid_attribution(&portfolio, &benchmark).expect("valid adversarial inputs");
        let reconstructed = r.total_curve + r.total_sector + r.total_selection;
        assert!(
            (reconstructed - r.active_return).abs() < 1e-15,
            "reconstructed {reconstructed} vs active {}",
            r.active_return
        );
    }

    /// Pins `WEIGHT_TOLERANCE` itself, in both directions: `1 + 5e-7` must
    /// be accepted, `1 + 2e-6` must be rejected.
    #[test]
    fn weight_tolerance_is_pinned_at_1e_minus_6_both_directions() {
        let base_portfolio = vec![
            pos("0.0-3.0", "GOVT", 0.20, 0.012),
            pos("0.0-3.0", "CORP", 0.20, 0.025),
            pos("3.0-6.0", "GOVT", 0.30, 0.028),
            pos("3.0-6.0", "CORP", 0.30, 0.045),
        ];
        let benchmark = vec![
            pos("0.0-3.0", "GOVT", 0.30, 0.010),
            pos("0.0-3.0", "CORP", 0.20, 0.020),
            pos("3.0-6.0", "GOVT", 0.25, 0.030),
            pos("3.0-6.0", "CORP", 0.25, 0.040),
        ];

        let mut just_over = base_portfolio.clone();
        just_over[0].weight += 2e-6;
        let err = grid_attribution(&just_over, &benchmark)
            .expect_err("1 + 2e-6 must be rejected at a 1e-6 tolerance");
        assert!(err.to_string().contains("Portfolio weights"), "{err}");

        let mut just_under = base_portfolio;
        just_under[0].weight += 5e-7;
        assert!(
            grid_attribution(&just_under, &benchmark).is_ok(),
            "1 + 5e-7 must be accepted at a 1e-6 tolerance"
        );
    }

    #[test]
    fn grid_attribution_result_serde_round_trips() {
        let portfolio = vec![pos("0.0-3.0", "GOVT", 1.0, 0.012)];
        let benchmark = vec![pos("0.0-3.0", "GOVT", 1.0, 0.010)];
        let r = grid_attribution(&portfolio, &benchmark).expect("valid inputs");

        let json = serde_json::to_string(&r).expect("serializes");
        let round_tripped: GridAttributionResult =
            serde_json::from_str(&json).expect("deserializes");

        close(
            round_tripped.portfolio_return,
            r.portfolio_return,
            1e-15,
            "portfolio_return",
        );
        close(
            round_tripped.benchmark_return,
            r.benchmark_return,
            1e-15,
            "benchmark_return",
        );
        close(
            round_tripped.active_return,
            r.active_return,
            1e-15,
            "active_return",
        );
        close(
            round_tripped.total_curve,
            r.total_curve,
            1e-15,
            "total_curve",
        );
        close(
            round_tripped.total_sector,
            r.total_sector,
            1e-15,
            "total_sector",
        );
        close(
            round_tripped.total_selection,
            r.total_selection,
            1e-15,
            "total_selection",
        );
        assert_eq!(round_tripped.curve_effects.len(), r.curve_effects.len());
        assert_eq!(round_tripped.sector_effects.len(), r.sector_effects.len());
        assert_eq!(
            round_tripped.selection_effects.len(),
            r.selection_effects.len()
        );
        assert_eq!(round_tripped.curve_effects[0].cell, r.curve_effects[0].cell);
    }

    #[test]
    fn grid_position_serde_denies_unknown_fields() {
        let json = r#"{
            "cell": "0.0-3.0", "sector": "GOVT", "weight": 0.5, "total_return": 0.01
        }"#;
        let parsed: GridPosition = serde_json::from_str(json).expect("stable names parse");
        assert_eq!(parsed.cell, "0.0-3.0");

        let bad = r#"{
            "cell": "0.0-3.0", "sector": "GOVT", "weight": 0.5, "total_return": 0.01,
            "surprise": 1.0
        }"#;
        assert!(
            serde_json::from_str::<GridPosition>(bad).is_err(),
            "unknown field must be rejected"
        );
    }

    #[test]
    fn grid_attribution_rejects_non_finite_inputs() {
        let mut portfolio = vec![pos("0.0-3.0", "GOVT", 1.0, 0.01)];
        portfolio[0].total_return = f64::NAN;
        let benchmark = vec![pos("0.0-3.0", "GOVT", 1.0, 0.008)];
        let err = grid_attribution(&portfolio, &benchmark).expect_err("NaN must be rejected");
        assert!(err.to_string().contains("finite"), "{err}");
    }

    /// The Task-4 golden fixture, built through the public [`grid_attribution`]
    /// entry point (not hand-assembled) so the per-period effect vectors are
    /// guaranteed internally consistent: `r^P = 0.0293`, `r^B = 0.0245`,
    /// `total_curve = 0.0021`, `total_sector = 0.0004`,
    /// `total_selection = 0.0023`.
    fn golden_period() -> GridAttributionResult {
        let portfolio = vec![
            pos("0.0-3.0", "GOVT", 0.20, 0.012),
            pos("0.0-3.0", "CORP", 0.20, 0.025),
            pos("3.0-6.0", "GOVT", 0.30, 0.028),
            pos("3.0-6.0", "CORP", 0.30, 0.045),
        ];
        let benchmark = vec![
            pos("0.0-3.0", "GOVT", 0.30, 0.010),
            pos("0.0-3.0", "CORP", 0.20, 0.020),
            pos("3.0-6.0", "GOVT", 0.25, 0.030),
            pos("3.0-6.0", "CORP", 0.25, 0.040),
        ];
        grid_attribution(&portfolio, &benchmark).expect("golden fixture is valid")
    }

    #[test]
    fn linked_effects_reconstruct_compounded_active_return() {
        // Two copies of the Task-4 golden period.
        // R_p = 1.0293² − 1 = 0.05945849 ; R_b = 1.0245² − 1 = 0.04960025.
        let period = golden_period();
        let linked = grid_carino_link(&[period.clone(), period]).expect("two valid periods");
        let close = |a: f64, b: f64, tol: f64| assert!((a - b).abs() < tol, "{a} vs {b}");
        close(linked.portfolio_return_compounded, 0.05945849, 1e-8);
        close(linked.benchmark_return_compounded, 0.04960025, 1e-8);
        // The defining property: linked effects sum to geometric active exactly.
        let sum = linked.linked_curve + linked.linked_sector + linked.linked_selection;
        close(
            sum,
            linked.portfolio_return_compounded - linked.benchmark_return_compounded,
            1e-12,
        );
        // Each effect equals 2 · scale · per-period effect; assert per-effect values via the
        // scale recomputed in-test from the Carino formula (identity-based, no fragile hand
        // logs). This also pins the effect->accumulator mapping: swapping linked_sector and
        // linked_selection would fail these assertions since 0.0004 != 0.0023.
        let k = ((1.0293f64).ln() - (1.0245f64).ln()) / 0.0048;
        let kk = ((1.05945849f64).ln() - (1.04960025f64).ln()) / (0.05945849 - 0.04960025);
        let scale = k / kk;
        close(linked.linked_curve, 2.0 * scale * 0.0021, 1e-10);
        close(linked.linked_sector, 2.0 * scale * 0.0004, 1e-10);
        close(linked.linked_selection, 2.0 * scale * 0.0023, 1e-10);
        assert_eq!(
            linked.periods.len(),
            2,
            "periods passthrough must preserve every input period"
        );
    }

    #[test]
    fn grid_link_rejects_materially_tampered_effect_totals() {
        let period = GridAttributionResult {
            portfolio_return: 0.05,
            benchmark_return: 0.02,
            active_return: 0.03,
            curve_effects: Vec::new(),
            sector_effects: Vec::new(),
            selection_effects: Vec::new(),
            total_curve: 0.01,
            total_sector: 0.01,
            total_selection: 0.005,
        };

        let err = grid_carino_link(&[period])
            .expect_err("linking must reject inconsistent totals rather than repair them");
        assert!(
            err.to_string().contains("effect totals"),
            "error must name the violated reconciliation: {err}"
        );
    }

    #[test]
    fn grid_link_rejects_active_return_mismatch() {
        let mut period = golden_period();
        period.active_return += 0.001;

        let err = grid_carino_link(&[period])
            .expect_err("declared active return must match portfolio minus benchmark");
        assert!(
            err.to_string().contains("active_return"),
            "error must name the mismatched field: {err}"
        );
    }

    #[test]
    fn grid_link_rejects_non_finite_effects() {
        let mut period = golden_period();
        period.total_sector = f64::NAN;

        let err = grid_carino_link(&[period]).expect_err("NaN top-level effects must be rejected");
        assert!(
            err.to_string().contains("total_sector"),
            "error must name the non-finite field: {err}"
        );
    }

    fn synthetic_grid_period(
        portfolio_return: f64,
        benchmark_return: f64,
        active_return: f64,
        effects: [f64; 3],
    ) -> GridAttributionResult {
        GridAttributionResult {
            portfolio_return,
            benchmark_return,
            active_return,
            curve_effects: Vec::new(),
            sector_effects: Vec::new(),
            selection_effects: Vec::new(),
            total_curve: effects[0],
            total_sector: effects[1],
            total_selection: effects[2],
        }
    }

    #[test]
    fn grid_link_rejects_active_mismatch_when_additive_return_scale_overflows() {
        let period = synthetic_grid_period(1e308, 9e307, 0.0, [0.0; 3]);
        let err = grid_carino_link(&[period])
            .expect_err("finite return magnitudes must not overflow the validation scale");
        assert!(err.to_string().contains("active_return"), "{err}");
    }

    #[test]
    fn grid_link_rejects_non_finite_expected_active_explicitly() {
        let period = synthetic_grid_period(f64::MAX, -f64::MAX, 0.0, [0.0; 3]);
        let err = grid_carino_link(&[period])
            .expect_err("overflowed portfolio-minus-benchmark return must be rejected");
        assert!(
            err.to_string()
                .contains("portfolio_return - benchmark_return must be finite"),
            "{err}"
        );
    }

    #[test]
    fn grid_link_accepts_huge_finite_cancelling_effects() {
        let period = synthetic_grid_period(0.01, 0.01, 0.0, [f64::MAX, -f64::MAX, 0.0]);
        let linked = grid_carino_link(&[period])
            .expect("scaled L1 tolerance must not overflow on finite cancelling effects");
        assert_eq!(linked.linked_curve, f64::MAX);
        assert_eq!(linked.linked_sector, -f64::MAX);
    }

    #[test]
    fn grid_link_rejects_non_finite_reconciliation_residual_explicitly() {
        let period = synthetic_grid_period(0.01, 0.01, 0.0, [f64::MAX, f64::MAX, -f64::MAX]);
        let err = grid_carino_link(&[period])
            .expect_err("overflowed effect reconciliation must be rejected");
        assert!(
            err.to_string()
                .contains("reconciliation residual must be finite"),
            "{err}"
        );
    }

    #[test]
    fn grid_link_accepts_generated_near_cancelling_output() {
        let eps = 1.1e-6;
        let portfolio = vec![pos("X", "GOVT", 0.5, 0.8), pos("Y", "GOVT", 0.5, -0.2)];
        let benchmark = vec![
            pos("X", "GOVT", 0.5, 1.0),
            pos("X", "CORP", -(0.5 - eps), -0.2),
            pos("Y", "GOVT", 1.0 - eps, 0.1),
        ];
        let period = grid_attribution(&portfolio, &benchmark)
            .expect("the bucket remains just outside the near-zero rejection boundary");
        let gross =
            period.total_curve.abs() + period.total_sector.abs() + period.total_selection.abs();
        assert!(
            gross > 1_000.0,
            "fixture must exercise cancellation between large effects (gross {gross})"
        );

        grid_carino_link(&[period])
            .expect("scale-aware validation must accept generated near-cancelling output");
    }

    #[test]
    fn link_rejects_non_finite_period_returns() {
        let mut bad = golden_period();
        bad.portfolio_return = f64::NAN;
        let err = grid_carino_link(&[bad]).expect_err("NaN portfolio_return must be rejected");
        let message = err.to_string();
        assert!(
            message.contains("finite period returns"),
            "error must name the finiteness requirement (this pins the dedicated per-period \
             finiteness guard, distinct from carino_coefficient's own finiteness check on the \
             compounded/per-period Carino coefficients): {message}"
        );
    }

    #[test]
    fn grid_carino_linked_result_serde_round_trips() {
        let period = golden_period();
        let linked = grid_carino_link(&[period.clone(), period]).expect("two valid periods");

        let json = serde_json::to_string(&linked).expect("serializes");
        let round_tripped: GridCarinoLinkedResult =
            serde_json::from_str(&json).expect("deserializes");

        assert_eq!(
            round_tripped.periods.len(),
            linked.periods.len(),
            "periods passthrough must survive a serde round trip"
        );
        close(
            round_tripped.portfolio_return_compounded,
            linked.portfolio_return_compounded,
            1e-15,
            "portfolio_return_compounded",
        );
        close(
            round_tripped.benchmark_return_compounded,
            linked.benchmark_return_compounded,
            1e-15,
            "benchmark_return_compounded",
        );
        close(
            round_tripped.linked_curve,
            linked.linked_curve,
            1e-15,
            "linked_curve",
        );
        close(
            round_tripped.linked_sector,
            linked.linked_sector,
            1e-15,
            "linked_sector",
        );
        close(
            round_tripped.linked_selection,
            linked.linked_selection,
            1e-15,
            "linked_selection",
        );
    }

    /// Both rejection paths, with message assertions (not merely "is an
    /// error"): empty input, and a period return at/below −100 %, which is
    /// outside the Carino formula's `ln(1 + r)` domain.
    #[test]
    fn link_rejects_empty_and_sub_minus_100_percent_returns() {
        let empty: Vec<GridAttributionResult> = Vec::new();
        let err = grid_carino_link(&empty).expect_err("empty periods must be rejected");
        assert!(
            err.to_string().contains("at least one period"),
            "empty-input error must explain the requirement: {err}"
        );

        let mut sub_100 = golden_period();
        sub_100.portfolio_return = -1.5; // 1 + r = -0.5 <= 0, outside ln(1+r)'s domain.
        sub_100.active_return = sub_100.portfolio_return - sub_100.benchmark_return;
        sub_100.total_curve = sub_100.active_return;
        sub_100.total_sector = 0.0;
        sub_100.total_selection = 0.0;
        let err = grid_carino_link(&[sub_100]).expect_err("return <= -100% must be rejected");
        let message = err.to_string();
        assert!(
            message.contains("-100%") || message.contains("100 %"),
            "sub-minus-100%-return error must name the domain violation: {message}"
        );
    }

    /// Two periods with genuinely different effect vectors and returns (not
    /// two copies of the golden, which is already covered by the
    /// reconstruction test above): the golden two-cell period, and a single
    /// (cell, sector) period whose entire active return surfaces as
    /// selection (`total_curve = total_sector = 0`). Mixed period lengths
    /// (position counts) are inherent — there is no `period_years` input.
    #[test]
    fn mixed_length_periods_link_and_reconcile() {
        let golden = golden_period();

        let small_portfolio = vec![pos("A", "X", 1.0, 0.03)];
        let small_benchmark = vec![pos("A", "X", 1.0, 0.02)];
        let small = grid_attribution(&small_portfolio, &small_benchmark)
            .expect("single-cell period is valid");
        // Sanity: this period's effect vector genuinely differs from the
        // golden's (0.0021, 0.0004, 0.0023) — it is a pure-selection period.
        assert_eq!(small.total_curve, 0.0);
        assert_eq!(small.total_sector, 0.0);
        let close = |a: f64, b: f64, tol: f64| assert!((a - b).abs() < tol, "{a} vs {b}");
        close(small.total_selection, 0.01, 1e-12);

        let linked = grid_carino_link(&[golden, small]).expect("mixed-length periods link");
        let sum = linked.linked_curve + linked.linked_sector + linked.linked_selection;
        let active = linked.portfolio_return_compounded - linked.benchmark_return_compounded;
        close(sum, active, 1e-12);
    }
}

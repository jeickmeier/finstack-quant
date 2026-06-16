//! Currency-preserving aggregation of cashflows into `Period`s.
//!
//! # Period Contract
//!
//! All aggregation functions bucket flows into half-open intervals
//! `[period.start, period.end)`: a flow dated exactly on `period.end` belongs
//! to the *next* period. Periods must be sorted by start date and
//! non-overlapping; the public entry points validate this and return
//! [`finstack_quant_core::Error::Validation`] otherwise.
//!
//! # Summation Policy
//!
//! Per-currency totals (nominal and PV) are accumulated as Neumaier-compensated
//! `f64` sums over `Money::amount()` values. No per-flow ISO-4217 rounding is
//! applied during accumulation; the final total is constructed via `Money` from
//! the compensated sum. Results are deterministic given sorted inputs (the
//! public wrappers sort unsorted inputs by date before accumulating).
//!
//! # Historical-Flow PV Convention
//!
//! PV aggregation functions assign **zero PV** to flows dated on or before the
//! valuation base date (`DateContext::base`). Historical flows still appear in
//! plain amount aggregation (`aggregate_by_period`). This matches the
//! convention documented by
//! [`CashFlowSchedule::to_period_dataframe`](crate::builder::CashFlowSchedule::to_period_dataframe).

use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Period, PeriodId};
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::Money;

use indexmap::IndexMap;

// =============================================================================
// Generic Flow Iterator
// =============================================================================

/// Trait for types that have an associated date.
///
/// This allows generic iteration over different flow types (DatedFlow, CashFlow)
/// without code duplication.
trait HasDate {
    fn flow_date(&self) -> Date;
}

impl HasDate for crate::DatedFlow {
    fn flow_date(&self) -> Date {
        self.0
    }
}

impl HasDate for CashFlow {
    fn flow_date(&self) -> Date {
        self.date
    }
}

/// Helper to iterate over periods and yield the slice of flows belonging to each period.
///
/// Assumes flows are sorted by date. Implements O(n + m) behavior by maintaining
/// a cursor position across the sorted flows array.
///
/// # Arguments
///
/// * `flows` - Sorted flows by date (any type implementing `HasDate`)
/// * `periods` - Period definitions with start/end boundaries
///
/// # Returns
///
/// Iterator yielding `(Period, &[T])` pairs where the flow slice contains
/// all flows with `period.start <= date < period.end`.
fn iter_by_period<'a, T: HasDate>(
    flows: &'a [T],
    periods: &'a [Period],
) -> impl Iterator<Item = (&'a Period, &'a [T])> + 'a {
    debug_assert!(
        flows
            .windows(2)
            .all(|w| w[0].flow_date() <= w[1].flow_date()),
        "iter_by_period requires flows to be sorted by date"
    );

    let mut flow_idx = 0;
    let n = flows.len();

    periods.iter().map(move |p| {
        // Skip flows before this period
        while flow_idx < n && flows[flow_idx].flow_date() < p.start {
            flow_idx += 1;
        }

        let start_idx = flow_idx;

        // Find end of flows for this period
        while flow_idx < n && flows[flow_idx].flow_date() < p.end {
            flow_idx += 1;
        }

        (p, &flows[start_idx..flow_idx])
    })
}

/// Validate the aggregation period contract: sorted by start, non-overlapping,
/// and free of duplicate `PeriodId`s.
///
/// The flow-bucketing cursor in [`iter_by_period`] never rewinds, so unsorted
/// or overlapping periods would silently drop flows; duplicate ids would
/// silently overwrite earlier results. Both are rejected loudly instead.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if periods are not sorted by
/// start date, overlap (a period starts before the previous period ends), or
/// share a `PeriodId`.
pub(crate) fn validate_periods(periods: &[Period]) -> finstack_quant_core::Result<()> {
    for w in periods.windows(2) {
        if w[1].start < w[0].start {
            return Err(finstack_quant_core::Error::Validation(format!(
                "aggregation periods must be sorted by start date: period '{}' (start {}) follows period '{}' (start {})",
                w[1].id, w[1].start, w[0].id, w[0].start
            )));
        }
        if w[1].start < w[0].end {
            return Err(finstack_quant_core::Error::Validation(format!(
                "aggregation periods must be non-overlapping (half-open [start, end)): period '{}' starts {} before period '{}' ends {}",
                w[1].id, w[1].start, w[0].id, w[0].end
            )));
        }
    }
    let mut seen: std::collections::HashSet<PeriodId> = std::collections::HashSet::new();
    for p in periods {
        if !seen.insert(p.id) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "aggregation periods contain duplicate PeriodId '{}'",
                p.id
            )));
        }
    }
    Ok(())
}

/// Currency-preserving aggregation of cashflows into `Period`s.
///
/// Groups cashflows by time period while preserving currency separation.
/// Returns a map: `PeriodId -> (Currency -> Money)`. Per-currency sums use
/// Neumaier-compensated f64 accumulation (see module docs).
///
/// See unit tests and `examples/` for usage.
fn aggregate_by_period_sorted(
    sorted: &[crate::DatedFlow],
    periods: &[Period],
) -> IndexMap<PeriodId, IndexMap<Currency, Money>> {
    let mut out: IndexMap<PeriodId, IndexMap<Currency, Money>> = IndexMap::new();
    let mut per_ccy: IndexMap<Currency, NeumaierAccumulator> = IndexMap::new();

    for (p, flows_in_period) in iter_by_period(sorted, periods) {
        if flows_in_period.is_empty() {
            continue;
        }

        per_ccy.clear();
        for &(_d, m) in flows_in_period {
            let ccy = m.currency();
            per_ccy.entry(ccy).or_default().add(m.amount());
        }
        let mut result: IndexMap<Currency, Money> = IndexMap::with_capacity(per_ccy.len());
        for (&ccy, acc) in &per_ccy {
            result.insert(ccy, Money::new(acc.total(), ccy));
        }
        out.insert(p.id, result);
    }
    out
}

/// Aggregate cashflows by period with currency preservation.
///
/// Public wrapper that sorts flows before aggregation. For pre-sorted inputs,
/// this performs O(n log n) sort + O(n+m) aggregation.
///
/// Flows are bucketed into half-open intervals `[period.start, period.end)`:
/// a flow dated exactly on `period.end` belongs to the next period. Periods
/// must be sorted by start date, non-overlapping, and have unique ids.
///
/// # Arguments
///
/// * `flows` - Dated cashflows to aggregate. Inputs do not need to be pre-sorted.
/// * `periods` - Sorted, disjoint reporting periods using half-open intervals
///   `[period.start, period.end)`.
///
/// # Returns
///
/// Map from `PeriodId` to currency-indexed nominal cashflow sums. Periods with
/// no cashflows are omitted from the result. Per-currency sums use
/// Neumaier-compensated f64 accumulation (no per-flow ISO-4217 rounding).
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if periods are unsorted,
/// overlapping, or contain duplicate `PeriodId`s.
///
/// # Performance
///
/// - Uses `sort_unstable_by_key` for ~5-10% faster sorting vs stable sort
/// - The `#[inline(never)]` attribute was removed to allow compiler optimization
/// - Benchmarks show 2-5% improvement on hot paths overall
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::aggregation::aggregate_by_period;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::{Date, Period, PeriodId};
/// use finstack_quant_core::money::Money;
/// use time::Month;
///
/// let flows = vec![(
///     Date::from_calendar_date(2025, Month::March, 15).expect("valid date"),
///     Money::new(100.0, Currency::USD),
/// )];
/// let periods = vec![Period {
///     id: PeriodId::quarter(2025, 1),
///     start: Date::from_calendar_date(2025, Month::January, 1).expect("valid date"),
///     end: Date::from_calendar_date(2025, Month::April, 1).expect("valid date"),
///     is_actual: true,
/// }];
///
/// let aggregated = aggregate_by_period(&flows, &periods)?;
/// assert!(aggregated.contains_key(&PeriodId::quarter(2025, 1)));
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn aggregate_by_period(
    flows: &[crate::DatedFlow],
    periods: &[Period],
) -> finstack_quant_core::Result<IndexMap<PeriodId, IndexMap<Currency, Money>>> {
    validate_periods(periods)?;
    if flows.is_empty() || periods.is_empty() {
        return Ok(IndexMap::new());
    }
    let is_sorted = flows.windows(2).all(|w| w[0].0 <= w[1].0);
    if is_sorted {
        return Ok(aggregate_by_period_sorted(flows, periods));
    }
    let mut sorted: Vec<crate::DatedFlow> = flows.to_vec();
    sorted.sort_unstable_by_key(|(d, _)| *d);
    Ok(aggregate_by_period_sorted(&sorted, periods))
}

// =============================================================================
// Precision-Preserving Aggregation
// =============================================================================

use finstack_quant_core::market_data::traits::{Discounting, Survival};

/// Currency-checked single-currency aggregation with explicit target currency.
///
/// - Empty input returns `Ok(0 target)`.
/// - All flows must match `target` currency; otherwise returns `Error::CurrencyMismatch`.
/// - Sums `Money::amount()` values with a Neumaier-compensated f64 accumulator;
///   no per-flow ISO-4217 rounding is applied during accumulation.
///
/// # Arguments
///
/// * `flows` - Dated cashflows to aggregate.
/// * `target` - Required currency for every flow and for the returned total.
///
/// # Returns
///
/// Single `Money` total in `target` currency.
///
/// # Errors
///
/// Returns `CurrencyMismatch` if any flow currency differs from `target`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::aggregation::aggregate_cashflows_checked;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::money::Money;
/// use time::Month;
///
/// let flows = vec![(
///     Date::from_calendar_date(2025, Month::January, 15).expect("valid date"),
///     Money::new(25.0, Currency::USD),
/// )];
///
/// let total =
///     aggregate_cashflows_checked(&flows, Currency::USD).expect("aggregation succeeds");
/// assert_eq!(total.currency(), Currency::USD);
/// ```
pub fn aggregate_cashflows_checked(
    flows: &[crate::DatedFlow],
    target: Currency,
) -> finstack_quant_core::Result<Money> {
    let mut acc = NeumaierAccumulator::default();
    for &(_d, m) in flows {
        if m.currency() != target {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: target,
                actual: m.currency(),
            });
        }
        acc.add(m.amount());
    }
    Ok(Money::new(acc.total(), target))
}

// =============================================================================
// Pre-Period PV Aggregation
// =============================================================================

/// Shared implementation for PV aggregation across plain and credit-adjusted variants.
///
/// Buckets flows into half-open intervals `[period.start, period.end)` and
/// validates the sorted/disjoint period contract before accumulating.
fn pv_by_period_generic<T, F>(
    sorted: &[T],
    periods: &[Period],
    disc: &dyn Discounting,
    hazard: Option<&dyn Survival>,
    date_ctx: &DateContext<'_>,
    mut value_fn: F,
) -> finstack_quant_core::Result<IndexMap<PeriodId, IndexMap<Currency, Money>>>
where
    T: HasDate,
    F: FnMut(&T, f64, f64) -> Money,
{
    validate_periods(periods)?;
    // Pre-size the outer map to avoid reallocations during insertion.
    let mut out: IndexMap<PeriodId, IndexMap<Currency, Money>> =
        IndexMap::with_capacity(periods.len());
    // Reusable buffer for per-currency accumulation across periods.
    let mut per_ccy: IndexMap<Currency, NeumaierAccumulator> = IndexMap::with_capacity(4);
    // Reusable buffer for building the inner result map.
    let mut result_buf: IndexMap<Currency, Money> = IndexMap::with_capacity(4);

    for (p, flows_in_period) in iter_by_period(sorted, periods) {
        if flows_in_period.is_empty() {
            continue;
        }

        per_ccy.clear();
        for flow in flows_in_period {
            let (_t, df, sp) = time_discount_survival(flow.flow_date(), disc, hazard, date_ctx)?;
            let pv = value_fn(flow, df, sp);
            let ccy = pv.currency();
            per_ccy.entry(ccy).or_default().add(pv.amount());
        }

        // Skip periods with no value (all flows filtered to zero)
        if per_ccy.is_empty() {
            continue;
        }

        // Build result from accumulated per-currency values, reusing the buffer.
        result_buf.clear();
        for (&ccy, acc) in &per_ccy {
            result_buf.insert(ccy, Money::try_new(acc.total(), ccy)?);
        }
        out.insert(p.id, result_buf.clone());
    }

    Ok(out)
}

fn pv_by_period_precomputed(
    sorted: &[CashFlow],
    pv_per_flow: &[Money],
    periods: &[Period],
) -> finstack_quant_core::Result<IndexMap<PeriodId, IndexMap<Currency, Money>>> {
    debug_assert_eq!(sorted.len(), pv_per_flow.len());
    validate_periods(periods)?;
    // Pre-size the outer map to avoid reallocations.
    let mut out: IndexMap<PeriodId, IndexMap<Currency, Money>> =
        IndexMap::with_capacity(periods.len());
    // Reusable buffers for per-currency accumulation and result building.
    let mut per_ccy: IndexMap<Currency, NeumaierAccumulator> = IndexMap::with_capacity(4);
    let mut result_buf: IndexMap<Currency, Money> = IndexMap::with_capacity(4);
    let mut flow_idx = 0usize;
    let n = sorted.len();

    for p in periods {
        while flow_idx < n && sorted[flow_idx].date < p.start {
            flow_idx += 1;
        }

        per_ccy.clear();
        while flow_idx < n && sorted[flow_idx].date < p.end {
            let pv = pv_per_flow[flow_idx];
            per_ccy.entry(pv.currency()).or_default().add(pv.amount());
            flow_idx += 1;
        }

        if !per_ccy.is_empty() {
            result_buf.clear();
            for (&ccy, acc) in &per_ccy {
                result_buf.insert(ccy, Money::try_new(acc.total(), ccy)?);
            }
            out.insert(p.id, result_buf.clone());
        }
    }

    Ok(out)
}

/// Checked variant that works directly on `CashFlow` slices without intermediate allocation.
///
/// Filters out `DefaultedNotional` flows during PV computation. Requires flows
/// to be pre-sorted by date (as guaranteed by `CashFlowSchedule`).
///
/// Flows dated on or before the valuation `base` contribute **zero PV**
/// (historical-flow convention, matching the DataFrame export); they still
/// occupy their period bucket so totals reconcile with plain aggregation.
pub(crate) fn pv_by_period_cashflows_sorted_checked(
    sorted: &[CashFlow],
    periods: &[Period],
    disc: &dyn Discounting,
    base: Date,
    dc: DayCount,
    dc_ctx: DayCountContext<'_>,
    hazard: Option<&dyn Survival>,
) -> finstack_quant_core::Result<IndexMap<PeriodId, IndexMap<Currency, Money>>> {
    let date_ctx = DateContext::new(base, dc, dc_ctx);
    pv_by_period_generic(sorted, periods, disc, hazard, &date_ctx, |cf, df, sp| {
        // Historical flows (date <= base) carry zero PV by convention.
        if cf.kind == CFKind::DefaultedNotional || cf.date <= base {
            return Money::new(0.0, cf.amount.currency());
        }
        let pv_amount = cf.amount.amount() * df * sp;
        Money::new(pv_amount, cf.amount.currency())
    })
}

/// Parameters for date and day-count calculations.
///
/// This is primarily an internal helper type used by PV aggregation functions.
/// Most users should use the higher-level aggregation functions which
/// construct this internally. Exposed for advanced use cases requiring
/// direct control over day-count context.
pub struct DateContext<'a> {
    /// Base date for time calculations.
    pub base: Date,
    /// Day-count convention to use.
    pub dc: DayCount,
    /// Day-count context for calendar and holiday handling.
    pub dc_ctx: DayCountContext<'a>,
}

impl<'a> DateContext<'a> {
    /// Create a new date context.
    ///
    /// # Arguments
    ///
    /// * `base` - Valuation or anchor date used for year-fraction calculations.
    /// * `dc` - Day-count convention used to map dates into year fractions.
    /// * `dc_ctx` - Supplemental day-count context such as frequency or calendar.
    ///
    /// # Returns
    ///
    /// New [`DateContext`] instance carrying the provided inputs.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::aggregation::DateContext;
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    /// use time::Month;
    ///
    /// let ctx = DateContext::new(
    ///     Date::from_calendar_date(2025, Month::January, 1).expect("valid date"),
    ///     DayCount::Act365F,
    ///     DayCountContext::default(),
    /// );
    ///
    /// assert_eq!(ctx.dc, DayCount::Act365F);
    /// ```
    pub fn new(base: Date, dc: DayCount, dc_ctx: DayCountContext<'a>) -> Self {
        Self { base, dc, dc_ctx }
    }
}

/// Credit-adjusted PV of a single cashflow under [`RecoveryTiming::AtPaymentDate`].
///
/// # Recovery timing
///
/// Under this convention recovery is valued as if realized on the scheduled
/// payment date T:
///
/// ```text
/// PV_recovery = amount · df(T) · r · (1 − sp(T))
/// ```
///
/// This is the closed-form "end-of-interval" approximation and underestimates
/// PV relative to the "recovery paid at default time τ" form by roughly
/// `r · (df(τ_mid) − df(T))` per interval — typically ≤ 1 bp for 5Y horizons
/// on liquid credit. For integrated / default-midpoint semantics use
/// [`RecoveryTiming::AtDefaultIntegrated`] (see
/// [`pv_by_period_credit_adjusted_detailed_with_timing`]).
pub(crate) fn credit_adjusted_period_pv(
    cf: &CashFlow,
    df: f64,
    sp: f64,
    recovery_rate: Option<f64>,
    base: Date,
) -> Money {
    // Historical flows (date <= valuation base) carry zero PV by convention,
    // matching the DataFrame export and the plain PV path.
    if cf.kind == CFKind::DefaultedNotional || cf.date <= base {
        return Money::new(0.0, cf.amount.currency());
    }

    // Recovery and AccruedOnDefault are realized post-default cash flows
    // from the already-defaulted portion of the notional. They are
    // discounted at their scheduled dates without survival adjustment
    // because default has already occurred for this portion.
    if matches!(cf.kind, CFKind::Recovery | CFKind::AccruedOnDefault) {
        return Money::new(cf.amount.amount() * df, cf.amount.currency());
    }

    let recovery_term = if let Some(r) = recovery_rate {
        match cf.kind {
            CFKind::Amortization | CFKind::Notional | CFKind::PrePayment => r * (1.0 - sp),
            _ => 0.0,
        }
    } else {
        0.0
    };

    let pv_factor = df * (sp + recovery_term);
    let amount = cf.amount;
    Money::new(amount.amount() * pv_factor, amount.currency())
}

/// Recovery-leg timing convention for credit-adjusted PV aggregation.
///
/// Controls how the recovery cashflow `r · (1 − sp)` on surviving principal
/// flows is placed in time:
///
/// * [`AtPaymentDate`](Self::AtPaymentDate) — recovery is assumed paid on the
///   scheduled payment date `T`. This is the closed-form "end-of-interval"
///   approximation and is the historical default.
/// * [`AtDefaultIntegrated`](Self::AtDefaultIntegrated) — recovery is
///   integrated over the interval `(T_prev, T]` using the ISDA "default at
///   midpoint" closed form: the expected default mass `sp(T_prev) − sp(T)`
///   is discounted at the interval midpoint. This reduces the ~1 bp bias
///   from the closed form for curve-upward-sloping discount and hazard
///   shapes.
///
/// `T_prev` for the first principal flow is the valuation base date
/// (`DateContext::base`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RecoveryTiming {
    /// Recovery realized on the scheduled payment date (closed-form default).
    #[default]
    AtPaymentDate,
    /// Recovery integrated over the interval `(T_prev, T]` using the ISDA
    /// "default at midpoint" approximation: `r · amount · df(t_mid) · (sp(T_prev) − sp(T))`.
    AtDefaultIntegrated,
}

/// Compute signed year fraction, discount factor, and survival probability
/// for a given cashflow date.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if the discount curve or
/// survival curve produces a non-finite value at the computed time point;
/// PV aggregation errors loudly instead of panicking inside `Money::new`.
fn time_discount_survival(
    d: Date,
    disc: &dyn Discounting,
    hazard: Option<&dyn Survival>,
    ctx: &DateContext<'_>,
) -> finstack_quant_core::Result<(f64, f64, f64)> {
    // Compute year fraction from base to cashflow date - propagate errors
    let t = if d == ctx.base {
        0.0
    } else if d > ctx.base {
        ctx.dc.year_fraction(ctx.base, d, ctx.dc_ctx)?
    } else {
        -ctx.dc.year_fraction(d, ctx.base, ctx.dc_ctx)?
    };

    // Get discount factor
    let df = disc.df(t);
    if !df.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "discount curve returned non-finite df ({df}) at t={t} (date {d})"
        )));
    }

    // Get survival probability if hazard curve provided
    let sp = hazard.map(|h| h.sp(t)).unwrap_or(1.0);
    if !sp.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "survival curve returned non-finite sp ({sp}) at t={t} (date {d})"
        )));
    }

    Ok((t, df, sp))
}

/// Currency-preserving aggregation of cashflow present values by period with credit adjustment and recovery support.
///
/// Like [`crate::builder::CashFlowSchedule::pv_by_period`], but works on full
/// `CashFlow` objects (preserving `CFKind`) and supports credit adjustment + recovery.
/// This allows applying recovery rates to principal flows while assuming zero recovery for interest flows.
///
/// # Recovery Logic
///
/// If `recovery_rate` is `Some(R)`:
/// - **Amortization/Notional**: PV includes recovery term: `PV = Amount * DF * (SP + R * (1 - SP))`
/// - **Others (Interest/Fees)**: PV assumes zero recovery: `PV = Amount * DF * SP`
///
/// If `recovery_rate` is `None`, falls back to zero recovery for all flows (`PV = Amount * DF * SP`).
///
/// # Recovery Rationale
///
/// This follows standard credit modeling convention where:
/// - Principal claims (Amortization, Notional, PrePayment) have recovery value in default
/// - Interest/fee claims are typically subordinate and assumed to have zero recovery
///
/// # Recovery and Explicit Recovery Flows
///
/// The `recovery_rate` term `R * (1 - SP)` is applied only to **surviving**
/// principal flows. Explicit `Recovery` and `AccruedOnDefault` cashflows in the
/// schedule are discounted at their scheduled dates without survival adjustment
/// because they represent realized post-default cash. `DefaultedNotional` flows
/// are zeroed since they represent the removed principal. This avoids
/// double-counting because `DefaultedNotional` removes the defaulted portion
/// from the surviving pool before the `R * (1 - SP)` credit adjustment is
/// applied to remaining principal.
///
/// # Historical Flows and Period Contract
///
/// Flows dated on or before `date_ctx.base` contribute **zero PV** by
/// convention (matching the DataFrame export and the plain PV path). Periods
/// must be sorted by start, non-overlapping (half-open `[start, end)`), and
/// have unique ids.
///
/// # Errors
///
/// Returns an error if:
/// - `hazard` curve is `None`
/// - `recovery_rate` is outside the valid range `[0.0, 1.0]`
/// - periods are unsorted, overlapping, or contain duplicate ids
/// - a curve returns a non-finite value at a required time point
///
/// # Arguments
///
/// * `flows` - Full cashflows including `CFKind`, amount, and payment date.
/// * `periods` - Reporting periods using half-open intervals
///   `[period.start, period.end)`.
/// * `disc` - Discount curve used for present value calculation.
/// * `hazard` - Survival curve used to produce default-adjusted PVs.
/// * `recovery_rate` - Optional recovery assumption for principal-like flows.
/// * `date_ctx` - Valuation date and day-count configuration used to convert
///   dates into year fractions.
///
/// # Returns
///
/// Map from `PeriodId` to currency-indexed present values. Periods with no
/// flows are omitted from the result.
///
/// Credit-adjusted period PV aggregation with configurable recovery timing.
///
/// Uses [`RecoveryTiming::default`] when callers do not need to override how the
/// recovery leg on surviving principal flows is placed in time.
///
/// # Errors
///
/// Same error conditions as the credit-adjusted period PV contract above.
pub(crate) fn pv_by_period_credit_adjusted_detailed_with_timing(
    flows: &[CashFlow],
    periods: &[Period],
    disc: &dyn Discounting,
    hazard: Option<&dyn Survival>,
    recovery_rate: Option<f64>,
    timing: RecoveryTiming,
    date_ctx: DateContext<'_>,
) -> finstack_quant_core::Result<IndexMap<PeriodId, IndexMap<Currency, Money>>> {
    // Validate recovery rate is in [0, 1] if provided
    if let Some(r) = recovery_rate {
        if !(0.0..=1.0).contains(&r) {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::Invalid,
            ));
        }
    }

    // Guard against double-counting recovery: when the schedule contains
    // explicit DefaultedNotional flows AND a non-zero recovery_rate is
    // supplied, the surviving principal would get recovery applied twice
    // (once via the explicit Recovery cashflow, once via R*(1-SP) on the
    // remaining amortization stream). Reject this combination.
    if recovery_rate.is_some() && flows.iter().any(|cf| cf.kind == CFKind::DefaultedNotional) {
        return Err(finstack_quant_core::Error::Validation(
            "pv_by_period_credit_adjusted_detailed: schedule contains explicit \
             DefaultedNotional flows; pass recovery_rate=None to avoid \
             double-counting recovery from both explicit events and hazard curve"
                .into(),
        ));
    }

    if flows.is_empty() || periods.is_empty() {
        return Ok(IndexMap::new());
    }
    let hazard = hazard.ok_or_else(|| {
        finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
            id: "hazard curve".to_string(),
        })
    })?;
    let is_sorted = flows.windows(2).all(|w| w[0].date <= w[1].date);

    match timing {
        RecoveryTiming::AtPaymentDate => {
            let base = date_ctx.base;
            let pv_fn = |cf: &CashFlow, df: f64, sp: f64| {
                credit_adjusted_period_pv(cf, df, sp, recovery_rate, base)
            };
            if is_sorted {
                return pv_by_period_generic(flows, periods, disc, Some(hazard), &date_ctx, pv_fn);
            }
            let mut sorted: Vec<CashFlow> = flows.to_vec();
            sorted.sort_unstable_by_key(|cf| cf.date);
            pv_by_period_generic(&sorted, periods, disc, Some(hazard), &date_ctx, pv_fn)
        }
        RecoveryTiming::AtDefaultIntegrated => {
            // Pre-compute per-flow PVs carrying the integrated recovery leg,
            // then reduce to the standard (cf, df, sp) closure form by looking
            // up the pre-computed value. State (previous principal-like date)
            // must be threaded across the full sorted sequence.
            let owned: Vec<CashFlow>;
            let sorted: &[CashFlow] = if is_sorted {
                flows
            } else {
                let mut s: Vec<CashFlow> = flows.to_vec();
                s.sort_unstable_by_key(|cf| cf.date);
                owned = s;
                &owned
            };
            let pv_per_flow =
                precompute_integrated_pv(sorted, disc, hazard, recovery_rate, &date_ctx)?;
            pv_by_period_precomputed(sorted, &pv_per_flow, periods)
        }
    }
}

/// Compute per-flow credit-adjusted PV under `RecoveryTiming::AtDefaultIntegrated`.
///
/// For surviving principal flows, the recovery leg uses the ISDA "default at
/// midpoint" approximation over the interval `(T_prev, T]` where `T_prev` is
/// the previous principal-like date (initialized to `date_ctx.base`).
fn precompute_integrated_pv(
    sorted: &[CashFlow],
    disc: &dyn Discounting,
    hazard: &dyn Survival,
    recovery_rate: Option<f64>,
    date_ctx: &DateContext<'_>,
) -> finstack_quant_core::Result<Vec<Money>> {
    let mut out: Vec<Money> = Vec::with_capacity(sorted.len());
    // Boundary T_prev for the principal date group currently being processed.
    let mut prev_principal: Date = date_ctx.base;
    // Date of the principal group currently being processed. Principal flows
    // sharing a date all receive the same (T_prev, T] default mass; T_prev
    // advances once per distinct principal date, never within a date group
    // (a same-date second principal flow must not see a zero-width interval).
    let mut current_principal_date: Option<Date> = None;
    for cf in sorted {
        let ccy = cf.amount.currency();

        // Historical flows (date <= valuation base) carry zero PV by
        // convention, matching the DataFrame export and the plain PV path.
        if cf.date <= date_ctx.base {
            out.push(Money::new(0.0, ccy));
            continue;
        }

        let (t_next, df_t, sp_t) = time_discount_survival(cf.date, disc, Some(hazard), date_ctx)?;

        // DefaultedNotional: zeroed (identical to AtPaymentDate path)
        if cf.kind == CFKind::DefaultedNotional {
            out.push(Money::new(0.0, ccy));
            continue;
        }
        // Realised post-default: discounted at scheduled date, no SP
        if matches!(cf.kind, CFKind::Recovery | CFKind::AccruedOnDefault) {
            out.push(Money::try_new(cf.amount.amount() * df_t, ccy)?);
            continue;
        }

        let is_principal = matches!(
            cf.kind,
            CFKind::Amortization | CFKind::Notional | CFKind::PrePayment
        );

        if is_principal && current_principal_date != Some(cf.date) {
            // Entering a new principal date group: the boundary becomes the
            // previous group's date (or the valuation base for the first group).
            if let Some(d) = current_principal_date {
                prev_principal = d;
            }
            current_principal_date = Some(cf.date);
        }

        let mut pv = cf.amount.amount() * df_t * sp_t;

        if let Some(r) = recovery_rate {
            if is_principal {
                // Integrate recovery leg over (T_prev, T] using midpoint default timing.
                let (t_prev, _df_prev, sp_prev) =
                    time_discount_survival(prev_principal, disc, Some(hazard), date_ctx)?;
                let t_mid = 0.5 * (t_prev + t_next);
                let df_mid = disc.df(t_mid);
                if !df_mid.is_finite() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "discount curve returned non-finite df ({df_mid}) at midpoint t={t_mid}"
                    )));
                }
                let d_sp = sp_prev - sp_t;
                // d_sp can go slightly negative from curve noise; clamp to avoid
                // sign inversion (recovery is a non-negative cashflow expectation).
                let d_sp_pos = d_sp.max(0.0);
                pv += r * cf.amount.amount() * df_mid * d_sp_pos;
            }
        }

        out.push(Money::try_new(pv, ccy)?);
    }
    Ok(out)
}

#[cfg(test)]
mod compensated_sum_tests {
    use super::*;

    #[test]
    fn preserves_small_addend() {
        let mut acc = NeumaierAccumulator::default();
        acc.add(1.0);
        acc.add(1e-16);
        acc.add(-1.0);
        let result = acc.total();
        assert!(
            result > 0.0,
            "Neumaier should preserve small addend (non-zero): got {}",
            result
        );
        assert!(
            (result - 1e-16).abs() < 1e-16,
            "Neumaier should preserve small addend close to 1e-16: got {}",
            result
        );
    }

    #[test]
    fn large_sum_accuracy() {
        let mut acc = NeumaierAccumulator::default();
        for _ in 0..10_000 {
            acc.add(0.1);
        }
        let result = acc.total();
        assert!(
            (result - 1000.0).abs() < 1e-10,
            "Neumaier sum of 10k x 0.1 should be ~1000.0, got {}",
            result
        );
    }

    #[test]
    fn beats_naive_drift() {
        let mut naive = 0.0_f64;
        let mut acc = NeumaierAccumulator::default();
        for _ in 0..100_000 {
            naive += 0.1;
            acc.add(0.1);
        }
        let naive_error = (naive - 10_000.0).abs();
        let neumaier_error = (acc.total() - 10_000.0).abs();
        assert!(
            neumaier_error < naive_error,
            "Neumaier error ({}) should be less than naive error ({})",
            neumaier_error,
            naive_error
        );
    }
}

#[cfg(test)]
mod period_contract_tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Period, PeriodId};
    use finstack_quant_core::market_data::traits::TermStructure;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use time::Month;

    fn d(y: i32, m: u8, day: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("valid month"), day)
            .expect("valid date")
    }

    fn period(id: PeriodId, start: Date, end: Date) -> Period {
        Period {
            id,
            start,
            end,
            is_actual: true,
        }
    }

    struct UnitDiscount {
        base: Date,
    }

    impl TermStructure for UnitDiscount {
        fn id(&self) -> &CurveId {
            static ID: std::sync::LazyLock<CurveId> = std::sync::LazyLock::new(|| "unit".into());
            &ID
        }
    }

    impl Discounting for UnitDiscount {
        fn base_date(&self) -> Date {
            self.base
        }
        fn df(&self, _t: f64) -> f64 {
            1.0
        }
    }

    /// Discount curve that returns NaN for any positive time point.
    struct NanDiscount {
        base: Date,
    }

    impl TermStructure for NanDiscount {
        fn id(&self) -> &CurveId {
            static ID: std::sync::LazyLock<CurveId> = std::sync::LazyLock::new(|| "nan".into());
            &ID
        }
    }

    impl Discounting for NanDiscount {
        fn base_date(&self) -> Date {
            self.base
        }
        fn df(&self, t: f64) -> f64 {
            if t > 0.0 {
                f64::NAN
            } else {
                1.0
            }
        }
    }

    #[test]
    fn aggregate_by_period_rejects_unsorted_periods() {
        let flows = vec![(d(2025, 3, 15), Money::new(100.0, Currency::USD))];
        let periods = vec![
            period(PeriodId::quarter(2025, 2), d(2025, 4, 1), d(2025, 7, 1)),
            period(PeriodId::quarter(2025, 1), d(2025, 1, 1), d(2025, 4, 1)),
        ];
        let err = aggregate_by_period(&flows, &periods).expect_err("unsorted periods rejected");
        assert!(format!("{err}").contains("sorted"), "got: {err}");
    }

    #[test]
    fn aggregate_by_period_rejects_overlapping_periods() {
        let flows = vec![(d(2025, 3, 15), Money::new(100.0, Currency::USD))];
        let periods = vec![
            period(PeriodId::quarter(2025, 1), d(2025, 1, 1), d(2025, 5, 1)),
            period(PeriodId::quarter(2025, 2), d(2025, 4, 1), d(2025, 7, 1)),
        ];
        let err = aggregate_by_period(&flows, &periods).expect_err("overlapping periods rejected");
        assert!(format!("{err}").contains("non-overlapping"), "got: {err}");
    }

    #[test]
    fn aggregate_by_period_rejects_duplicate_period_ids() {
        let flows = vec![(d(2025, 3, 15), Money::new(100.0, Currency::USD))];
        let periods = vec![
            period(PeriodId::quarter(2025, 1), d(2025, 1, 1), d(2025, 4, 1)),
            period(PeriodId::quarter(2025, 1), d(2025, 4, 1), d(2025, 7, 1)),
        ];
        let err = aggregate_by_period(&flows, &periods).expect_err("duplicate ids rejected");
        assert!(format!("{err}").contains("duplicate"), "got: {err}");
    }

    #[test]
    fn aggregate_by_period_preserves_currency_separation() {
        // Two currencies through aggregate_by_period: per-currency map outputs
        // are separate and no cross-currency summation occurs.
        let flows = vec![
            (d(2025, 2, 1), Money::new(100.0, Currency::USD)),
            (d(2025, 2, 15), Money::new(70.0, Currency::EUR)),
            (d(2025, 3, 1), Money::new(50.0, Currency::USD)),
        ];
        let periods = vec![period(
            PeriodId::quarter(2025, 1),
            d(2025, 1, 1),
            d(2025, 4, 1),
        )];

        let out = aggregate_by_period(&flows, &periods).expect("aggregation succeeds");
        let q1 = out.get(&PeriodId::quarter(2025, 1)).expect("Q1 present");
        assert_eq!(q1.len(), 2, "one entry per currency");
        assert!((q1[&Currency::USD].amount() - 150.0).abs() < 1e-12);
        assert!((q1[&Currency::EUR].amount() - 70.0).abs() < 1e-12);
        assert_eq!(q1[&Currency::USD].currency(), Currency::USD);
        assert_eq!(q1[&Currency::EUR].currency(), Currency::EUR);
    }

    #[test]
    fn boundary_flow_buckets_into_next_period_half_open() {
        // A flow exactly on a period boundary belongs to the NEXT period
        // (half-open [start, end) convention).
        let boundary = d(2025, 4, 1);
        let flows = vec![(boundary, Money::new(100.0, Currency::USD))];
        let periods = vec![
            period(PeriodId::quarter(2025, 1), d(2025, 1, 1), d(2025, 4, 1)),
            period(PeriodId::quarter(2025, 2), d(2025, 4, 1), d(2025, 7, 1)),
        ];

        let out = aggregate_by_period(&flows, &periods).expect("aggregation succeeds");
        assert!(!out.contains_key(&PeriodId::quarter(2025, 1)));
        assert!((out[&PeriodId::quarter(2025, 2)][&Currency::USD].amount() - 100.0).abs() < 1e-12);
    }

    #[test]
    fn pv_by_period_errors_on_nan_discount_curve() {
        let base = d(2025, 1, 1);
        let flows = vec![CashFlow {
            date: d(2025, 6, 1),
            reset_date: None,
            amount: Money::new(100.0, Currency::USD),
            kind: CFKind::Fixed,
            accrual_factor: 0.5,
            rate: None,
        }];
        let periods = vec![period(PeriodId::annual(2025), base, d(2026, 1, 1))];
        let disc = NanDiscount { base };

        let result = pv_by_period_cashflows_sorted_checked(
            &flows,
            &periods,
            &disc,
            base,
            DayCount::Act365F,
            DayCountContext::default(),
            None,
        );
        let err = result.expect_err("NaN df must error, not panic");
        assert!(format!("{err}").contains("non-finite"), "got: {err}");
    }

    #[test]
    fn pv_by_period_zeroes_historical_flows() {
        // Flows dated on or before the valuation base get zero PV (matching
        // the DataFrame convention); they still appear in nominal aggregation.
        let base = d(2025, 4, 1);
        let flows = vec![
            CashFlow {
                date: d(2025, 2, 1), // historical
                reset_date: None,
                amount: Money::new(100.0, Currency::USD),
                kind: CFKind::Fixed,
                accrual_factor: 0.25,
                rate: None,
            },
            CashFlow {
                date: base, // exactly on base: also historical by convention
                reset_date: None,
                amount: Money::new(50.0, Currency::USD),
                kind: CFKind::Fixed,
                accrual_factor: 0.25,
                rate: None,
            },
            CashFlow {
                date: d(2025, 6, 1), // future
                reset_date: None,
                amount: Money::new(200.0, Currency::USD),
                kind: CFKind::Fixed,
                accrual_factor: 0.25,
                rate: None,
            },
        ];
        let periods = vec![period(PeriodId::annual(2025), d(2025, 1, 1), d(2026, 1, 1))];
        let disc = UnitDiscount { base };

        let out = pv_by_period_cashflows_sorted_checked(
            &flows,
            &periods,
            &disc,
            base,
            DayCount::Act365F,
            DayCountContext::default(),
            None,
        )
        .expect("pv aggregation succeeds");

        // With df = 1.0 only the future flow contributes PV.
        let pv = out[&PeriodId::annual(2025)][&Currency::USD].amount();
        assert!((pv - 200.0).abs() < 1e-12, "expected 200, got {pv}");

        // Nominal aggregation still includes the historical flows.
        let dated: Vec<crate::DatedFlow> = flows.iter().map(|cf| (cf.date, cf.amount)).collect();
        let nominal = aggregate_by_period(&dated, &periods).expect("nominal aggregation");
        let total = nominal[&PeriodId::annual(2025)][&Currency::USD].amount();
        assert!((total - 350.0).abs() < 1e-12, "expected 350, got {total}");
    }
}

#[cfg(test)]
mod credit_pv_tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Period, PeriodId};
    use finstack_quant_core::market_data::traits::TermStructure;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use time::Month;

    fn d(y: i32, m: u8, day: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("valid month"), day)
            .expect("valid date")
    }

    struct FlatDiscount {
        base: Date,
    }

    impl TermStructure for FlatDiscount {
        fn id(&self) -> &CurveId {
            static ID: std::sync::LazyLock<CurveId> = std::sync::LazyLock::new(|| "test".into());
            &ID
        }
    }

    impl Discounting for FlatDiscount {
        fn base_date(&self) -> Date {
            self.base
        }
        fn df(&self, _t: f64) -> f64 {
            1.0
        }
    }

    struct FlatSurvival;

    impl TermStructure for FlatSurvival {
        fn id(&self) -> &CurveId {
            static ID: std::sync::LazyLock<CurveId> = std::sync::LazyLock::new(|| "hzd".into());
            &ID
        }
    }

    impl Survival for FlatSurvival {
        fn sp(&self, _t: f64) -> f64 {
            0.95
        }
    }

    fn make_period(base: Date, end: Date) -> Period {
        Period {
            id: PeriodId::quarter(base.year(), 1),
            start: base,
            end,
            is_actual: false,
        }
    }

    fn flow(date: Date, amount: f64, kind: CFKind) -> CashFlow {
        CashFlow {
            date,
            reset_date: None,
            amount: Money::new(amount, Currency::USD),
            kind,
            accrual_factor: 0.0,
            rate: None,
        }
    }

    #[test]
    fn rejects_defaulted_notional_with_recovery_rate() {
        let base = d(2025, 1, 1);
        let flows = vec![
            CashFlow {
                date: d(2025, 6, 1),
                reset_date: None,
                amount: Money::new(100_000.0, Currency::USD),
                kind: CFKind::DefaultedNotional,
                accrual_factor: 0.0,
                rate: None,
            },
            CashFlow {
                date: d(2025, 12, 1),
                reset_date: None,
                amount: Money::new(900_000.0, Currency::USD),
                kind: CFKind::Amortization,
                accrual_factor: 0.0,
                rate: None,
            },
        ];
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let disc = FlatDiscount { base };
        let hazard = FlatSurvival;
        let ctx = DateContext::new(base, DayCount::Act365F, DayCountContext::default());

        let result = pv_by_period_credit_adjusted_detailed_with_timing(
            &flows,
            &periods,
            &disc,
            Some(&hazard),
            Some(0.40),
            RecoveryTiming::default(),
            ctx,
        );
        assert!(
            result.is_err(),
            "should reject DefaultedNotional + recovery_rate"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("DefaultedNotional"),
            "error message should mention DefaultedNotional: {}",
            err_msg
        );
    }

    #[test]
    fn allows_defaulted_notional_without_recovery_rate() {
        let base = d(2025, 1, 1);
        let flows = vec![
            CashFlow {
                date: d(2025, 6, 1),
                reset_date: None,
                amount: Money::new(100_000.0, Currency::USD),
                kind: CFKind::DefaultedNotional,
                accrual_factor: 0.0,
                rate: None,
            },
            CashFlow {
                date: d(2025, 12, 1),
                reset_date: None,
                amount: Money::new(900_000.0, Currency::USD),
                kind: CFKind::Amortization,
                accrual_factor: 0.0,
                rate: None,
            },
        ];
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let disc = FlatDiscount { base };
        let hazard = FlatSurvival;
        let ctx = DateContext::new(base, DayCount::Act365F, DayCountContext::default());

        let result = pv_by_period_credit_adjusted_detailed_with_timing(
            &flows,
            &periods,
            &disc,
            Some(&hazard),
            None,
            RecoveryTiming::default(),
            ctx,
        );
        assert!(
            result.is_ok(),
            "should allow DefaultedNotional without recovery_rate"
        );
    }

    #[test]
    fn credit_adjusted_period_pv_matches_for_sorted_and_unsorted_flows() {
        let base = d(2025, 1, 1);
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let disc = FlatDiscount { base };
        let hazard = FlatSurvival;
        let sorted = vec![
            flow(d(2025, 3, 1), 1_000_000.0, CFKind::Amortization),
            flow(d(2025, 6, 1), 50_000.0, CFKind::Fixed),
            flow(d(2025, 9, 1), 10_000.0, CFKind::Fee),
            flow(d(2025, 11, 1), 25_000.0, CFKind::Recovery),
        ];
        let unsorted = vec![sorted[2], sorted[0], sorted[3], sorted[1]];

        let sorted_result = pv_by_period_credit_adjusted_detailed_with_timing(
            &sorted,
            &periods,
            &disc,
            Some(&hazard),
            Some(0.40),
            RecoveryTiming::default(),
            DateContext::new(base, DayCount::Act365F, DayCountContext::default()),
        )
        .expect("sorted flows should price");
        let unsorted_result = pv_by_period_credit_adjusted_detailed_with_timing(
            &unsorted,
            &periods,
            &disc,
            Some(&hazard),
            Some(0.40),
            RecoveryTiming::default(),
            DateContext::new(base, DayCount::Act365F, DayCountContext::default()),
        )
        .expect("unsorted flows should price");

        assert_eq!(sorted_result, unsorted_result);
    }

    #[test]
    fn recovery_timing_default_matches_at_payment_date() {
        let base = d(2025, 1, 1);
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let disc = FlatDiscount { base };
        let hazard = FlatSurvival;
        let flows = vec![
            flow(d(2025, 4, 1), 500_000.0, CFKind::Amortization),
            flow(d(2025, 10, 1), 500_000.0, CFKind::Amortization),
        ];

        let default_ctx = DateContext::new(base, DayCount::Act365F, DayCountContext::default());
        let explicit_ctx = DateContext::new(base, DayCount::Act365F, DayCountContext::default());

        let default_out = pv_by_period_credit_adjusted_detailed_with_timing(
            &flows,
            &periods,
            &disc,
            Some(&hazard),
            Some(0.40),
            RecoveryTiming::default(),
            default_ctx,
        )
        .expect("default pricing");
        let explicit_out = pv_by_period_credit_adjusted_detailed_with_timing(
            &flows,
            &periods,
            &disc,
            Some(&hazard),
            Some(0.40),
            RecoveryTiming::AtPaymentDate,
            explicit_ctx,
        )
        .expect("explicit AtPaymentDate pricing");

        assert_eq!(default_out, explicit_out);
    }

    #[test]
    fn recovery_timing_integrated_matches_hand_computed_under_flat_curves() {
        // Under flat df=1 and flat sp=0.95, the recovery leg for a single
        // principal flow over interval (base, T] collapses to:
        //   PV_surv = amount · df(T) · sp(T) = amount · 1 · 0.95
        //   PV_rec  = r · amount · df(t_mid) · (sp(base) - sp(T))
        //           = r · amount · 1 · (1 - 0.95)   [sp(base) must be 1]
        //
        // Our FlatSurvival returns 0.95 at every t, so d_sp = 0.95 - 0.95 = 0
        // for t_prev == base. Thus for flat hazard the integrated recovery
        // contribution is 0 (no default mass in the interval), which is the
        // correct degenerate behaviour.
        let base = d(2025, 1, 1);
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let disc = FlatDiscount { base };
        let hazard = FlatSurvival;
        let flows = vec![flow(d(2025, 12, 1), 1_000_000.0, CFKind::Amortization)];
        let ctx = DateContext::new(base, DayCount::Act365F, DayCountContext::default());

        let out = pv_by_period_credit_adjusted_detailed_with_timing(
            &flows,
            &periods,
            &disc,
            Some(&hazard),
            Some(0.40),
            RecoveryTiming::AtDefaultIntegrated,
            ctx,
        )
        .expect("integrated pricing");

        let pv = out
            .get(&periods[0].id)
            .and_then(|m| m.get(&Currency::USD))
            .map(|m| m.amount())
            .expect("single flow in single period");

        // With FlatSurvival constant at 0.95 (not 1 at base), d_sp=0, so
        // recovery term vanishes and PV reduces to amount · sp = 950_000.
        let expected = 1_000_000.0 * 1.0 * 0.95;
        assert!(
            (pv - expected).abs() < 1e-6,
            "expected {}, got {}",
            expected,
            pv
        );
    }

    #[test]
    fn recovery_timing_integrated_adds_default_mass_for_declining_survival() {
        // Hand-computed sanity check with a curve where sp steps down.
        struct StepSurvival;
        impl TermStructure for StepSurvival {
            fn id(&self) -> &CurveId {
                static ID: std::sync::LazyLock<CurveId> =
                    std::sync::LazyLock::new(|| "step".into());
                &ID
            }
        }
        impl Survival for StepSurvival {
            fn sp(&self, t: f64) -> f64 {
                // sp(0) = 1.0, decays linearly to 0.8 at t=1.
                (1.0 - 0.2 * t).clamp(0.0, 1.0)
            }
        }

        let base = d(2025, 1, 1);
        // Period end must strictly exceed the flow date because
        // `iter_by_period` uses half-open `[start, end)` semantics. We keep
        // the flow at exactly one year out (so `sp(T) = 0.8`) and extend the
        // period end by one day.
        let periods = vec![make_period(base, d(2026, 1, 2))];
        let disc = FlatDiscount { base };
        let hazard = StepSurvival;
        // Single principal flow at one full year out.
        let flows = vec![flow(d(2026, 1, 1), 1_000_000.0, CFKind::Amortization)];
        let ctx = DateContext::new(base, DayCount::Act365F, DayCountContext::default());

        let integrated = pv_by_period_credit_adjusted_detailed_with_timing(
            &flows,
            &periods,
            &disc,
            Some(&hazard),
            Some(0.40),
            RecoveryTiming::AtDefaultIntegrated,
            DateContext::new(base, DayCount::Act365F, DayCountContext::default()),
        )
        .expect("integrated pricing");
        let at_pay = pv_by_period_credit_adjusted_detailed_with_timing(
            &flows,
            &periods,
            &disc,
            Some(&hazard),
            Some(0.40),
            RecoveryTiming::AtPaymentDate,
            ctx,
        )
        .expect("at-payment-date pricing");

        // Under flat df=1, both paths put recovery mass (sp(base) - sp(T)) = 0.2
        // at df=1. So PVs match exactly. The integrated path only diverges when
        // df has curvature across the interval.
        let v_integrated = integrated
            .get(&periods[0].id)
            .and_then(|m| m.get(&Currency::USD))
            .map(|m| m.amount())
            .expect("price exists");
        let v_at_pay = at_pay
            .get(&periods[0].id)
            .and_then(|m| m.get(&Currency::USD))
            .map(|m| m.amount())
            .expect("price exists");
        // Hand computation:
        //   PV_surv = 1_000_000 · 1 · 0.8 = 800_000
        //   PV_rec  = 0.40 · 1_000_000 · 1 · 0.2 = 80_000
        //   PV_tot  = 880_000
        let expected = 1_000_000.0 * 0.8 + 0.40 * 1_000_000.0 * 0.2;
        assert!((v_integrated - expected).abs() < 1e-6);
        assert!((v_at_pay - expected).abs() < 1e-6);
    }

    #[test]
    fn recovery_timing_integrated_handles_same_date_principal_flows() {
        // Two principal flows (Amortization + PrePayment) on the SAME date
        // must each receive the full (T_prev, T] default mass. Previously the
        // second flow saw a zero-width interval (T, T] and its recovery leg
        // vanished. Under flat df = 1, AtDefaultIntegrated must agree with
        // AtPaymentDate exactly.
        struct LinearSurvival;
        impl TermStructure for LinearSurvival {
            fn id(&self) -> &CurveId {
                static ID: std::sync::LazyLock<CurveId> = std::sync::LazyLock::new(|| "lin".into());
                &ID
            }
        }
        impl Survival for LinearSurvival {
            fn sp(&self, t: f64) -> f64 {
                (1.0 - 0.2 * t).clamp(0.0, 1.0)
            }
        }

        let base = d(2025, 1, 1);
        let periods = vec![make_period(base, d(2026, 1, 2))];
        let disc = FlatDiscount { base };
        let hazard = LinearSurvival;
        let same_date = d(2026, 1, 1); // exactly one year out: sp(T) = 0.8
        let flows = vec![
            flow(same_date, 600_000.0, CFKind::Amortization),
            flow(same_date, 400_000.0, CFKind::PrePayment),
        ];

        let integrated = pv_by_period_credit_adjusted_detailed_with_timing(
            &flows,
            &periods,
            &disc,
            Some(&hazard),
            Some(0.40),
            RecoveryTiming::AtDefaultIntegrated,
            DateContext::new(base, DayCount::Act365F, DayCountContext::default()),
        )
        .expect("integrated pricing");
        let at_pay = pv_by_period_credit_adjusted_detailed_with_timing(
            &flows,
            &periods,
            &disc,
            Some(&hazard),
            Some(0.40),
            RecoveryTiming::AtPaymentDate,
            DateContext::new(base, DayCount::Act365F, DayCountContext::default()),
        )
        .expect("at-payment-date pricing");

        let v_integrated = integrated
            .get(&periods[0].id)
            .and_then(|m| m.get(&Currency::USD))
            .map(finstack_quant_core::money::Money::amount)
            .expect("integrated pv");
        let v_at_pay = at_pay
            .get(&periods[0].id)
            .and_then(|m| m.get(&Currency::USD))
            .map(finstack_quant_core::money::Money::amount)
            .expect("at-pay pv");

        // Hand computation (df = 1, sp(T) = 0.8, recovery mass 0.2 each):
        //   PV = 1_000_000 · 0.8 + 0.40 · 1_000_000 · 0.2 = 880_000
        let expected = 1_000_000.0 * 0.8 + 0.40 * 1_000_000.0 * 0.2;
        assert!(
            (v_integrated - expected).abs() < 1e-6,
            "integrated: expected {expected}, got {v_integrated}"
        );
        assert!(
            (v_integrated - v_at_pay).abs() < 1e-6,
            "integrated ({v_integrated}) must match at-payment-date ({v_at_pay}) under flat df"
        );
    }

    #[test]
    fn recovery_timing_integrated_uses_matching_pv_after_skipped_flows() {
        let base = d(2025, 1, 1);
        let periods = vec![Period {
            id: PeriodId::quarter(2025, 3),
            start: d(2025, 7, 1),
            end: d(2026, 1, 1),
            is_actual: false,
        }];
        let disc = FlatDiscount { base };
        let hazard = FlatSurvival;
        let flows = vec![
            flow(d(2025, 3, 1), 111.0, CFKind::Fixed),
            flow(d(2025, 10, 1), 1_000.0, CFKind::Fixed),
        ];

        let out = pv_by_period_credit_adjusted_detailed_with_timing(
            &flows,
            &periods,
            &disc,
            Some(&hazard),
            None,
            RecoveryTiming::AtDefaultIntegrated,
            DateContext::new(base, DayCount::Act365F, DayCountContext::default()),
        )
        .expect("integrated pricing");

        let pv = out
            .get(&periods[0].id)
            .and_then(|m| m.get(&Currency::USD))
            .map(|m| m.amount())
            .expect("period pv");
        let expected = 1_000.0 * 0.95;
        assert!(
            (pv - expected).abs() < 1e-9,
            "expected {}, got {}",
            expected,
            pv
        );
    }
}

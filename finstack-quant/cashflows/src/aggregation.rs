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
//! plain amount aggregation (`aggregate_by_period`).

use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Period, PeriodId};
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::Money;

use crate::primitives::is_cash_settlement_kind;
use indexmap::IndexMap;

/// Per-period, per-currency totals produced by [`aggregate_by_period`] and
/// [`crate::builder::CashFlowSchedule::pv_by_period`].
///
/// Wraps the ordered `period -> currency -> amount` map (insertion order
/// follows the supplied reporting periods; empty periods are omitted) and
/// dereferences to it, so map-style access keeps working. [`Self::rows`]
/// flattens the map into `(period, currency, amount)` rows for tabular
/// export. Serializes as the nested map with `PeriodId` labels as keys.
/// Construction and deserialization reject currency keys that differ from
/// their monetary values.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::aggregation::{aggregate_by_period, PeriodAggregation};
/// use finstack_quant_core::{currency::Currency, dates::{Date, Period, PeriodId}, money::Money};
/// use time::Month;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let flows = vec![(Date::from_calendar_date(2025, Month::March, 15).expect("date"), Money::from((100_i64, Currency::USD)))];
/// let periods = vec![Period {
///     id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
///     start: Date::from_calendar_date(2025, Month::January, 1).expect("date"),
///     end: Date::from_calendar_date(2025, Month::April, 1).expect("date"),
///     is_actual: true,
/// }];
/// let agg: PeriodAggregation = aggregate_by_period(&flows, &periods)?;
/// assert_eq!(agg.rows().len(), 1);
/// assert!(agg.contains_key(&PeriodId::quarter(2025, 1).expect("valid period fixture")));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct PeriodAggregation(IndexMap<PeriodId, IndexMap<Currency, Money>>);

impl PeriodAggregation {
    /// Flatten into `(period, currency, amount)` rows in map order.
    #[must_use]
    pub fn rows(&self) -> Vec<(PeriodId, Currency, Money)> {
        self.0
            .iter()
            .flat_map(|(period, per_currency)| {
                per_currency
                    .iter()
                    .map(move |(currency, amount)| (*period, *currency, *amount))
            })
            .collect()
    }

    /// Consume the wrapper and return the underlying nested map.
    #[must_use]
    pub fn into_inner(self) -> IndexMap<PeriodId, IndexMap<Currency, Money>> {
        self.0
    }
}

impl TryFrom<IndexMap<PeriodId, IndexMap<Currency, Money>>> for PeriodAggregation {
    type Error = finstack_quant_core::Error;

    /// Construct totals whose currency keys match their monetary values.
    ///
    /// # Arguments
    ///
    /// * `map` - Ordered reporting-period totals, with each amount in the
    ///   currency named by its enclosing key. No FX conversion is performed.
    ///
    /// # Errors
    ///
    /// Returns a currency mismatch when an amount differs from its currency key.
    fn try_from(map: IndexMap<PeriodId, IndexMap<Currency, Money>>) -> Result<Self, Self::Error> {
        for per_currency in map.values() {
            for (&currency, amount) in per_currency {
                if amount.currency() != currency {
                    return Err(finstack_quant_core::Error::CurrencyMismatch {
                        expected: currency,
                        actual: amount.currency(),
                    });
                }
            }
        }
        Ok(Self(map))
    }
}

impl<'de> serde::Deserialize<'de> for PeriodAggregation {
    /// Deserialize totals, rejecting currency keys that disagree with their amounts.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serialized reporting-period map using ISO currency
    ///   keys and currency-tagged monetary amounts.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let map: IndexMap<PeriodId, IndexMap<Currency, Money>> =
            serde::Deserialize::deserialize(deserializer)?;
        Self::try_from(map).map_err(serde::de::Error::custom)
    }
}

impl std::ops::Deref for PeriodAggregation {
    type Target = IndexMap<PeriodId, IndexMap<Currency, Money>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Date used to bucket a flow into a reporting period.
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

/// Yield each period with the date-sorted flow slice in `[start, end)`.
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
        while flow_idx < n && flows[flow_idx].flow_date() < p.start {
            flow_idx += 1;
        }

        let start_idx = flow_idx;

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
    for p in periods {
        if p.start >= p.end {
            return Err(finstack_quant_core::Error::Validation(format!(
                "aggregation period '{}' must have start {} strictly before end {}",
                p.id, p.start, p.end
            )));
        }
    }
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
fn aggregate_by_period_sorted(
    sorted: &[crate::DatedFlow],
    periods: &[Period],
) -> finstack_quant_core::Result<IndexMap<PeriodId, IndexMap<Currency, Money>>> {
    let mut out: IndexMap<PeriodId, IndexMap<Currency, Money>> = IndexMap::new();
    let mut per_currency: IndexMap<Currency, NeumaierAccumulator> = IndexMap::new();

    for (p, flows_in_period) in iter_by_period(sorted, periods) {
        if flows_in_period.is_empty() {
            continue;
        }

        per_currency.clear();
        for &(_d, m) in flows_in_period {
            let ccy = m.currency();
            per_currency.entry(ccy).or_default().add(m.amount());
        }
        let mut result: IndexMap<Currency, Money> = IndexMap::with_capacity(per_currency.len());
        for (&ccy, acc) in &per_currency {
            result.insert(ccy, Money::new(acc.total(), ccy)?);
        }
        out.insert(p.id, result);
    }
    Ok(out)
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
/// overlapping, or contain duplicate `PeriodId`s, and a conversion error when a
/// per-currency total is non-finite or exceeds the `Decimal` range.
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
///     Money::from((100_i64, Currency::USD)),
/// )];
/// let periods = vec![Period {
///     id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
///     start: Date::from_calendar_date(2025, Month::January, 1).expect("valid date"),
///     end: Date::from_calendar_date(2025, Month::April, 1).expect("valid date"),
///     is_actual: true,
/// }];
///
/// let aggregated = aggregate_by_period(&flows, &periods)?;
/// assert!(aggregated.contains_key(&PeriodId::quarter(2025, 1).expect("valid period fixture")));
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn aggregate_by_period(
    flows: &[crate::DatedFlow],
    periods: &[Period],
) -> finstack_quant_core::Result<PeriodAggregation> {
    validate_periods(periods)?;
    if flows.is_empty() || periods.is_empty() {
        return Ok(PeriodAggregation::default());
    }
    let is_sorted = flows.windows(2).all(|w| w[0].0 <= w[1].0);
    if is_sorted {
        return aggregate_by_period_sorted(flows, periods).and_then(PeriodAggregation::try_from);
    }
    let mut sorted: Vec<crate::DatedFlow> = flows.to_vec();
    sorted.sort_unstable_by_key(|(d, _)| *d);
    aggregate_by_period_sorted(&sorted, periods).and_then(PeriodAggregation::try_from)
}

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
///     Money::from((25_i64, Currency::USD)),
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
    Money::new(acc.total(), target)
}

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
    F: FnMut(&T, f64, f64) -> (Currency, f64),
{
    validate_periods(periods)?;
    let mut out: IndexMap<PeriodId, IndexMap<Currency, Money>> =
        IndexMap::with_capacity(periods.len());
    let mut per_currency: IndexMap<Currency, NeumaierAccumulator> = IndexMap::with_capacity(4);
    let mut result_buf: IndexMap<Currency, Money> = IndexMap::with_capacity(4);

    for (p, flows_in_period) in iter_by_period(sorted, periods) {
        if flows_in_period.is_empty() {
            continue;
        }

        per_currency.clear();
        for flow in flows_in_period {
            let (_t, df, sp) = time_discount_survival(flow.flow_date(), disc, hazard, date_ctx)?;
            let (ccy, pv) = value_fn(flow, df, sp);
            per_currency.entry(ccy).or_default().add(pv);
        }

        if per_currency.is_empty() {
            continue;
        }

        result_buf.clear();
        for (&ccy, acc) in &per_currency {
            result_buf.insert(ccy, Money::new(acc.total(), ccy)?);
        }
        out.insert(p.id, result_buf.clone());
    }

    Ok(out)
}

fn pv_by_period_precomputed(
    sorted: &[CashFlow],
    pv_per_flow: &[(Currency, f64)],
    periods: &[Period],
) -> finstack_quant_core::Result<IndexMap<PeriodId, IndexMap<Currency, Money>>> {
    debug_assert_eq!(sorted.len(), pv_per_flow.len());
    validate_periods(periods)?;
    let mut out: IndexMap<PeriodId, IndexMap<Currency, Money>> =
        IndexMap::with_capacity(periods.len());
    let mut per_currency: IndexMap<Currency, NeumaierAccumulator> = IndexMap::with_capacity(4);
    let mut result_buf: IndexMap<Currency, Money> = IndexMap::with_capacity(4);
    let mut flow_idx = 0usize;
    let n = sorted.len();

    for p in periods {
        while flow_idx < n && sorted[flow_idx].date < p.start {
            flow_idx += 1;
        }

        per_currency.clear();
        while flow_idx < n && sorted[flow_idx].date < p.end {
            let (ccy, pv) = pv_per_flow[flow_idx];
            per_currency.entry(ccy).or_default().add(pv);
            flow_idx += 1;
        }

        if !per_currency.is_empty() {
            result_buf.clear();
            for (&ccy, acc) in &per_currency {
                result_buf.insert(ccy, Money::new(acc.total(), ccy)?);
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
    day_count: DayCount,
    day_count_context: DayCountContext<'_>,
    hazard: Option<&dyn Survival>,
) -> finstack_quant_core::Result<IndexMap<PeriodId, IndexMap<Currency, Money>>> {
    let date_ctx = DateContext::new(base, day_count, day_count_context);
    pv_by_period_generic(sorted, periods, disc, hazard, &date_ctx, |cf, df, sp| {
        let ccy = cf.amount.currency();
        if !is_cash_settlement_kind(cf.kind) || cf.date <= base {
            return (ccy, 0.0);
        }
        (ccy, cf.amount.amount() * df * sp)
    })
}

/// Valuation date and day-count inputs for PV aggregation.
pub struct DateContext<'a> {
    /// Base date for time calculations.
    pub base: Date,
    /// Day-count convention to use.
    pub day_count: DayCount,
    /// Day-count context for calendar and holiday handling.
    pub day_count_context: DayCountContext<'a>,
}

impl<'a> DateContext<'a> {
    /// Construct a date context from a valuation date, day-count convention, and context.
    ///
    /// # Arguments
    ///
    /// * `base` - Valuation or anchor date used for year-fraction calculations.
    /// * `day_count` - Day-count convention used to map dates into year fractions.
    /// * `day_count_context` - Supplemental day-count context such as frequency or calendar.
    ///
    /// # Returns
    ///
    /// Constructed [`DateContext`] holding `base`, `day_count`, and `day_count_context`.
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
    /// assert_eq!(ctx.day_count, DayCount::Act365F);
    /// ```
    pub fn new(base: Date, day_count: DayCount, day_count_context: DayCountContext<'a>) -> Self {
        Self {
            base,
            day_count,
            day_count_context,
        }
    }
}

/// Calendar-year non-principal / principal / PV totals for one reporting year.
#[derive(Debug, Clone, PartialEq)]
pub struct CalendarYearLadderRow {
    /// Calendar year of the grouped cashflows (Gregorian, as on `Date::year`).
    pub year: i32,
    /// Sum of amounts that are not [`CFKind::is_principal_like`] in that year,
    /// in the flow's native units (interest, fees, recovery, and unknown-kind
    /// flows are not mixed in: unknown labels are rejected).
    pub non_principal: f64,
    /// Sum of [`CFKind::is_principal_like`] amounts in that year.
    pub principal: f64,
    /// Sum of present values in that year, in the same units as `non_principal`.
    pub pv: f64,
}

/// Group dated cashflows into a calendar-year non-principal / principal / PV ladder.
///
/// Kind labels use [`CFKind::parse_label`]. Unknown labels are rejected rather
/// than silently treated as interest. The `non_principal` column is every
/// classified kind that is not [`CFKind::is_principal_like`] (interest, fees,
/// recovery, accrued-on-default, …).
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when the four slices
/// have different lengths, a kind label is unknown, or an amount or PV is
/// non-finite.
///
/// # Arguments
///
/// * `dates` - Payment dates; the Gregorian year of each date is the bucket.
/// * `kind_labels` - Cashflow kind labels (`"fixed"`, `"notional"`, `"coupon"`,
///   `"principal"`, …). ASCII case is ignored; unknown labels error.
/// * `amounts` - Signed finite cashflow amounts, one per date, in native currency units.
/// * `pvs` - Finite present values, one per date, in the same units as `amounts`.
///
/// # Examples
///
/// ```
/// use finstack_quant_cashflows::aggregation::calendar_year_ladder;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// let dates = [
///     Date::from_calendar_date(2027, Month::March, 15).expect("valid"),
///     Date::from_calendar_date(2034, Month::March, 15).expect("valid"),
/// ];
/// let rows = calendar_year_ladder(&dates, &["coupon", "principal"], &[100.0, 1000.0], &[90.0, 700.0])
///     .expect("ladder");
/// assert_eq!(rows.first().map(|row| row.year), Some(2027));
/// assert!((rows.last().expect("row").principal - 1000.0).abs() < 1e-12);
/// ```
pub fn calendar_year_ladder(
    dates: &[Date],
    kind_labels: &[&str],
    amounts: &[f64],
    pvs: &[f64],
) -> finstack_quant_core::Result<Vec<CalendarYearLadderRow>> {
    if dates.len() != kind_labels.len() || dates.len() != amounts.len() || dates.len() != pvs.len()
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "calendar_year_ladder requires equal lengths, got dates={}, kinds={}, amounts={}, pvs={}",
            dates.len(),
            kind_labels.len(),
            amounts.len(),
            pvs.len()
        )));
    }

    let mut by_year: std::collections::BTreeMap<i32, [NeumaierAccumulator; 3]> =
        std::collections::BTreeMap::new();
    for (((date, kind_label), amount), pv) in dates
        .iter()
        .zip(kind_labels.iter())
        .zip(amounts.iter())
        .zip(pvs.iter())
    {
        if !amount.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "calendar_year_ladder amount must be finite, got {amount}"
            )));
        }
        if !pv.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "calendar_year_ladder pv must be finite, got {pv}"
            )));
        }
        let kind = CFKind::parse_label(kind_label).map_err(|err| {
            finstack_quant_core::Error::Validation(format!(
                "calendar_year_ladder unknown cashflow kind '{kind_label}': {err}"
            ))
        })?;
        let slot = by_year.entry(date.year()).or_insert_with(|| {
            [
                NeumaierAccumulator::new(),
                NeumaierAccumulator::new(),
                NeumaierAccumulator::new(),
            ]
        });
        if kind.is_principal_like() {
            slot[1].add(*amount);
        } else {
            slot[0].add(*amount);
        }
        slot[2].add(*pv);
    }

    Ok(by_year
        .into_iter()
        .map(
            |(year, [non_principal, principal, pv])| CalendarYearLadderRow {
                year,
                non_principal: non_principal.total(),
                principal: principal.total(),
                pv: pv.total(),
            },
        )
        .collect())
}

/// Survival-weighted settlement PV, excluding any estimated principal recovery.
fn credit_adjusted_period_pv(cf: &CashFlow, df: f64, sp: f64, base: Date) -> f64 {
    if !is_cash_settlement_kind(cf.kind) || cf.date <= base {
        return 0.0;
    }

    // Recovery / AccruedOnDefault are already post-default; discount without survival.
    if matches!(cf.kind, CFKind::Recovery | CFKind::AccruedOnDefault) {
        return cf.amount.amount() * df;
    }

    cf.amount.amount() * df * sp
}

/// Present values of a complete cashflow stream under payment-date recovery.
///
/// Recovery is estimated only while principal is funded, then discounted to
/// its scheduled repayment date. Future draws and PIK capitalization establish
/// principal on their economic balance dates. Within each currency, repayments
/// consume funded principal in first-in, first-out order. This is an explicit
/// allocation convention for the payment-date approximation, not a claim that
/// recovery is actually paid at maturity. Coupons have zero recovery and no
/// accrued-on-default term; this is not the ISDA CDS standard model.
///
/// The stream must include all future funding, capitalization, and principal
/// repayments. Opening funded principal is the remaining repayments less future
/// increases, independently per currency. Omitted historical funding is therefore
/// treated as already funded at `date_ctx.base`; historical cash itself has zero
/// PV. For each funded amount `N`, recovery is `N * R * DF(T) * (S(F) - S(T))`,
/// where `F` is its funding date, floored at the valuation date, and `T` its
/// repayment date. A single row cannot establish this funding history.
/// Explicit principal deltas determine face amounts independently of discounted
/// or premium cash settlements. Once the economic principal reduction is before
/// valuation, an unpaid settlement is a cash receivable: its cash amount is the
/// recoverable claim, consistently for raw and normalized schedules.
/// Capitalization paid early is paired with reductions on the same economic
/// principal date before allocating the remaining FIFO lots, and has zero
/// funded-principal recovery. Zero-cash balance
/// replay rows never create another recoverable settlement claim.
///
/// # Arguments
///
/// * `cashflows` - Complete classified cashflow stream, in any order. Principal
///   currency is preserved; non-cash rows have zero settlement PV.
/// * `discount_factors` - One finite non-negative discount factor per cashflow,
///   from the valuation date to that row's payment date in its native currency.
/// * `hazard` - Optional survival curve. Probabilities are conditional on survival
///   at the valuation date; `None` gives risk-free cashflows and requires no recovery.
/// * `recovery_rate` - Optional recovery rate in `[0, 1]` applied to defaulted
///   funded principal; `None` excludes estimated recovery. Explicit defaulted
///   principal cannot be combined with this estimated recovery term.
/// * `date_ctx` - Valuation date and date basis used for survival curves without
///   their own date origin. Flows on or before its base date have zero PV.
///
/// # Returns
///
/// Native-currency present values in exactly the same order as `cashflows`.
///
/// # Errors
///
/// Returns an error for mismatched lengths, non-finite or out-of-range inputs,
/// increasing future survival, inconsistent funding/repayment amounts, or a
/// recovery assumption without a hazard curve.
pub fn credit_adjusted_cashflow_pvs(
    cashflows: &[CashFlow],
    discount_factors: &[f64],
    hazard: Option<&dyn Survival>,
    recovery_rate: Option<f64>,
    date_ctx: DateContext<'_>,
) -> finstack_quant_core::Result<Vec<f64>> {
    if cashflows.len() != discount_factors.len() {
        return Err(finstack_quant_core::Error::Validation(
            "cashflows and discount factors must have matching lengths".into(),
        ));
    }
    validate_recovery(cashflows, hazard, recovery_rate)?;
    let mut pvs = Vec::with_capacity(cashflows.len());
    for (cashflow, &discount_factor) in cashflows.iter().zip(discount_factors) {
        if !discount_factor.is_finite() || discount_factor < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "discount factor must be finite and non-negative; got {discount_factor}"
            )));
        }
        let survival = if cashflow.date <= date_ctx.base {
            1.0
        } else {
            survival_on_date(cashflow.date, hazard, &date_ctx)?
        };
        pvs.push(credit_adjusted_period_pv(
            cashflow,
            discount_factor,
            survival,
            date_ctx.base,
        ));
    }
    if let Some(recovery) = recovery_rate.filter(|r| *r > 0.0) {
        let funding = funded_principal(cashflows, date_ctx.base)?;
        for (index, lots) in funding.iter().enumerate() {
            if lots.is_empty() {
                continue;
            }
            let pay_date = cashflows[index].date;
            let end_sp = survival_on_date(pay_date, hazard, &date_ctx)?;
            let mut mass = NeumaierAccumulator::new();
            for &(start, amount) in lots {
                let start_sp = survival_on_date(start, hazard, &date_ctx)?;
                mass.add(amount * default_mass(start_sp, end_sp)?);
            }
            pvs[index] += recovery * discount_factors[index] * mass.total();
        }
    }
    Ok(pvs)
}

fn validate_recovery(
    cashflows: &[CashFlow],
    hazard: Option<&dyn Survival>,
    recovery_rate: Option<f64>,
) -> finstack_quant_core::Result<()> {
    if let Some(rate) = recovery_rate {
        if !rate.is_finite() || !(0.0..=1.0).contains(&rate) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "recovery rate must be finite and in [0, 1]; got {rate}"
            )));
        }
        if hazard.is_none() {
            return Err(finstack_quant_core::Error::Validation(
                "estimated principal recovery requires a hazard curve".into(),
            ));
        }
        if cashflows
            .iter()
            .any(|cf| cf.kind == CFKind::DefaultedNotional)
        {
            return Err(finstack_quant_core::Error::Validation(
                "schedule contains explicit DefaultedNotional flows; pass recovery_rate=None \
                 to avoid double-counting recovery from explicit events and the hazard curve"
                    .into(),
            ));
        }
    }
    Ok(())
}

fn principal_repayment_amount(cf: &CashFlow, base: Date) -> f64 {
    if !matches!(
        cf.kind,
        CFKind::Amortization | CFKind::Notional | CFKind::PrePayment | CFKind::RevolvingRepayment
    ) || cf.amount.amount() <= 0.0
    {
        return 0.0;
    }
    if cf.get_balance_date() < base {
        return cf.amount.amount();
    }
    match cf.principal_delta {
        Some(delta) if delta.amount() < 0.0 => -delta.amount(),
        Some(delta) if delta.amount() > 0.0 => 0.0,
        // A zero delta can preserve an unsettled cash claim after its economic
        // balance movement has already been incorporated into opening notional.
        _ => cf.amount.amount().max(0.0),
    }
}

fn principal_increase(cf: &CashFlow) -> finstack_quant_core::Result<f64> {
    if let Some(delta) = cf.principal_delta {
        if delta.currency() != cf.amount.currency() {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: cf.amount.currency(),
                actual: delta.currency(),
            });
        }
        return Ok(delta.amount().max(0.0));
    }
    Ok(match cf.kind {
        CFKind::Pik => cf.amount.amount().max(0.0),
        CFKind::Notional | CFKind::RevolvingDraw => (-cf.amount.amount()).max(0.0),
        _ => 0.0,
    })
}

/// Assign each remaining repayment to FIFO dated funding lots, independently
/// per currency. Already-funded opening principal is inferred from the complete
/// remaining principal stream, so historical funding is never counted twice.
fn funded_principal(
    cashflows: &[CashFlow],
    base: Date,
) -> finstack_quant_core::Result<Vec<Vec<(Date, f64)>>> {
    let mut result = vec![Vec::new(); cashflows.len()];
    let mut by_currency: IndexMap<Currency, Vec<usize>> = IndexMap::new();
    for (index, cf) in cashflows.iter().enumerate() {
        by_currency
            .entry(cf.amount.currency())
            .or_default()
            .push(index);
    }
    for (currency, indices) in by_currency {
        let mut increases = Vec::new();
        let mut capitalizations: std::collections::BTreeMap<Date, f64> =
            std::collections::BTreeMap::new();
        let mut repayments = Vec::new();
        let mut opening = NeumaierAccumulator::new();
        for &index in &indices {
            let cf = &cashflows[index];
            if cf.get_balance_date() > base {
                let increase = principal_increase(cf)?;
                if increase > 0.0 {
                    if cf.kind == CFKind::Pik || cf.amount.amount() == 0.0 {
                        *capitalizations.entry(cf.get_balance_date()).or_default() += increase;
                    } else {
                        increases.push((cf.get_balance_date(), increase));
                    }
                }
            }
        }
        for index in indices {
            let cf = &cashflows[index];
            let mut repayment = principal_repayment_amount(cf, base);
            let balance_date = cf.get_balance_date();
            // An early payment of capitalized interest extinguishes that
            // same-date capitalization, even when other funded principal is
            // available. Economic-only residual replay rows retain this pairing
            // after the cash itself has settled and been filtered away.
            if balance_date > base && (cf.date < balance_date || cf.amount.amount() == 0.0) {
                let early_face = if cf.amount.amount() == 0.0 {
                    cf.principal_delta
                        .map_or(0.0, |delta| (-delta.amount()).max(0.0))
                } else {
                    repayment
                };
                if let Some(increase) = capitalizations.get_mut(&balance_date) {
                    let capitalized = early_face.min(*increase);
                    *increase -= capitalized;
                    if cf.date > base && repayment > 0.0 && capitalized > 0.0 {
                        result[index].push((cf.date, capitalized));
                        repayment -= capitalized;
                    }
                }
            }
            if cf.date > base && repayment > 0.0 {
                repayments.push((index, repayment));
                opening.add(repayment);
            }
        }
        if repayments.is_empty() {
            // Economic-only replay rows after the final cash settlement do not
            // create another recoverable cash claim.
            continue;
        }
        increases.extend(
            capitalizations
                .into_iter()
                .filter(|&(_, amount)| amount > 0.0),
        );
        for &(_, increase) in &increases {
            opening.add(-increase);
        }
        increases.sort_by_key(|&(date, _)| date);
        repayments.sort_by_key(|&(index, _)| cashflows[index].date);
        let scale = repayments
            .iter()
            .map(|&(_, amount)| amount)
            .fold(1.0, f64::max);
        let tolerance = scale * 1e-12;
        if opening.total() < -tolerance {
            return Err(finstack_quant_core::Error::Validation(format!(
                "future {currency} funding exceeds remaining principal repayments; \
                 recovery requires a complete principal stream"
            )));
        }
        let mut lots = std::collections::VecDeque::new();
        if opening.total() > 0.0 {
            lots.push_back((base, opening.total()));
        }
        lots.extend(increases);
        for (index, mut remaining) in repayments {
            let payment = &cashflows[index];
            while remaining > 0.0 {
                let Some((funded_at, available)) = lots.front_mut() else {
                    if remaining <= tolerance {
                        break;
                    }
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "{currency} principal repayment on {} exceeds principal funded by that date",
                        payment.date
                    )));
                };
                if *funded_at > payment.date {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "{currency} principal repayment on {} precedes its cash funding date {funded_at}",
                        payment.date
                    )));
                }
                let amount = remaining.min(*available);
                result[index].push((*funded_at, amount));
                remaining -= amount;
                *available -= amount;
                if *available <= 0.0 {
                    lots.pop_front();
                }
            }
        }
    }
    Ok(result)
}

fn default_mass(start_sp: f64, end_sp: f64) -> finstack_quant_core::Result<f64> {
    if end_sp > start_sp + 1e-12 {
        return Err(finstack_quant_core::Error::Validation(
            "survival probability must not increase over a funded principal interval".into(),
        ));
    }
    Ok((start_sp - end_sp).max(0.0))
}

/// Recovery-leg timing convention for credit-adjusted PV aggregation.
///
/// Controls when estimated recovery on funded principal is discounted:
///
/// * [`AtPaymentDate`](Self::AtPaymentDate) — recovery is assumed paid on the
///   scheduled payment date `T`. This is the closed-form "end-of-interval"
///   approximation. FIFO funding lots determine the default exposure start.
/// * [`AtDefaultIntegrated`](Self::AtDefaultIntegrated) — recovery is
///   split at principal-event dates within each funding lot's lifetime, with
///   each interval's default mass discounted at its midpoint. This remains a
///   midpoint approximation rather than an exact continuous-time integral.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RecoveryTiming {
    /// Recovery realized on the scheduled payment date (closed-form default).
    #[default]
    AtPaymentDate,
    /// Recovery on each funded interval discounted at its midpoint.
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
    let t =
        disc.day_count()
            .signed_year_fraction(disc.base_date(), d, DayCountContext::default())?;
    let df = disc.df_between_dates(ctx.base, d).map_err(|err| {
        finstack_quant_core::Error::Validation(format!(
            "non-finite or invalid relative discount factor at {d}: {err}"
        ))
    })?;
    if !df.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "discount curve returned non-finite relative df ({df}) at t={t} (date {d})"
        )));
    }
    Ok((t, df, survival_on_date(d, hazard, ctx)?))
}

fn survival_on_date(
    d: Date,
    hazard: Option<&dyn Survival>,
    ctx: &DateContext<'_>,
) -> finstack_quant_core::Result<f64> {
    if d == ctx.base {
        return Ok(1.0);
    }
    let sp = match hazard {
        Some(h) => match h.base_date() {
            Some(h_base) => {
                let h_day_count = h.day_count();
                let t_d =
                    h_day_count.signed_year_fraction(h_base, d, DayCountContext::default())?;
                let t_asof = h_day_count.signed_year_fraction(
                    h_base,
                    ctx.base,
                    DayCountContext::default(),
                )?;
                let sp_d = h.sp(t_d);
                let sp_asof = h.sp(t_asof);
                if !sp_asof.is_finite() || sp_asof <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "survival curve returned invalid base survival ({sp_asof}) at {}",
                        ctx.base
                    )));
                }
                sp_d / sp_asof
            }
            None => {
                let t_h =
                    ctx.day_count
                        .signed_year_fraction(ctx.base, d, DayCountContext::default())?;
                h.sp(t_h)
            }
        },
        None => 1.0,
    };
    if !sp.is_finite() || sp < 0.0 || (d > ctx.base && sp > 1.0) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "survival probability must be finite and in [0, 1] at future date {d}; got {sp}"
        )));
    }
    Ok(sp)
}

/// Currency-preserving credit-adjusted PVs of complete cashflow streams.
///
/// Principal recovery follows dated funding lots as described in
/// [`credit_adjusted_cashflow_pvs`]. Explicit recovery and accrued-on-default
/// cash are discounted without survival weighting. Explicit defaulted notional
/// cannot be combined with estimated recovery. Non-cash rows and already-settled
/// cashflows have zero PV.
///
/// # Arguments
///
/// * `flows` - Complete classified cashflow stream, including future funding
///   and capitalization; unsorted input is accepted.
/// * `periods` - Sorted non-overlapping reporting periods using half-open
///   intervals `[period.start, period.end)` and unique period identifiers.
/// * `disc` - Discount curve supplying relative discount factors from the valuation date.
/// * `hazard` - Required survival curve, conditional on survival at valuation.
/// * `recovery_rate` - Optional recovery fraction in `[0, 1]` on funded principal.
/// * `timing` - Payment-date or funded-interval midpoint recovery approximation.
/// * `date_ctx` - Valuation date and date basis for survival curves without their own origin.
///
/// # Returns
///
/// Reporting-period native-currency PV totals; empty periods are omitted.
///
/// # Errors
///
/// Returns an error for invalid periods, curves, recovery assumptions, or an
/// inconsistent funding/repayment stream.
pub(crate) fn pv_by_period_credit_adjusted_detailed_with_timing(
    flows: &[CashFlow],
    periods: &[Period],
    disc: &dyn Discounting,
    hazard: Option<&dyn Survival>,
    recovery_rate: Option<f64>,
    timing: RecoveryTiming,
    date_ctx: DateContext<'_>,
) -> finstack_quant_core::Result<IndexMap<PeriodId, IndexMap<Currency, Money>>> {
    validate_periods(periods)?;
    validate_recovery(flows, hazard, recovery_rate)?;
    if flows.is_empty() || periods.is_empty() {
        return Ok(IndexMap::new());
    }
    let hazard = hazard.ok_or_else(|| {
        finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
            id: "hazard curve".to_string(),
        })
    })?;
    let owned;
    let sorted = if flows.windows(2).all(|w| w[0].date <= w[1].date) {
        flows
    } else {
        let mut rows = flows.to_vec();
        rows.sort_by_key(|cf| cf.date);
        owned = rows;
        &owned
    };
    let discounts: Vec<f64> = sorted
        .iter()
        .map(|cf| {
            if cf.date <= date_ctx.base {
                Ok(1.0)
            } else {
                disc.df_between_dates(date_ctx.base, cf.date)
            }
        })
        .collect::<finstack_quant_core::Result<_>>()?;
    let pvs = match timing {
        RecoveryTiming::AtPaymentDate => {
            credit_adjusted_cashflow_pvs(sorted, &discounts, Some(hazard), recovery_rate, date_ctx)?
        }
        RecoveryTiming::AtDefaultIntegrated => {
            precompute_integrated_pv(sorted, &discounts, disc, hazard, recovery_rate, date_ctx)?
        }
    };
    let pv_per_flow: Vec<_> = sorted
        .iter()
        .zip(pvs)
        .map(|(cf, pv)| (cf.amount.currency(), pv))
        .collect();
    pv_by_period_precomputed(sorted, &pv_per_flow, periods)
}

/// Value recovery only over each funding lot's funded lifetime, splitting its
/// default integral at all principal-event dates in that currency. Prefix sums
/// avoid revisiting every interval for each repayment.
fn precompute_integrated_pv(
    flows: &[CashFlow],
    discounts: &[f64],
    disc: &dyn Discounting,
    hazard: &dyn Survival,
    recovery_rate: Option<f64>,
    date_ctx: DateContext<'_>,
) -> finstack_quant_core::Result<Vec<f64>> {
    let mut out = credit_adjusted_cashflow_pvs(
        flows,
        discounts,
        Some(hazard),
        None,
        DateContext::new(
            date_ctx.base,
            date_ctx.day_count,
            date_ctx.day_count_context,
        ),
    )?;
    let Some(recovery) = recovery_rate.filter(|rate| *rate > 0.0) else {
        return Ok(out);
    };
    let funding = funded_principal(flows, date_ctx.base)?;
    let mut dates: IndexMap<Currency, std::collections::BTreeSet<Date>> = IndexMap::new();
    for (cf, lots) in flows.iter().zip(&funding) {
        if !lots.is_empty() {
            let events = dates.entry(cf.amount.currency()).or_default();
            events.insert(cf.date);
            events.extend(lots.iter().map(|&(start, _)| start));
        }
    }
    let mut integrals: IndexMap<Currency, std::collections::BTreeMap<Date, f64>> = IndexMap::new();
    let t_asof = disc.day_count().signed_year_fraction(
        disc.base_date(),
        date_ctx.base,
        DayCountContext::default(),
    )?;
    for (currency, event_dates) in dates {
        let mut integral = NeumaierAccumulator::new();
        let mut previous_date = date_ctx.base;
        let mut previous_survival = 1.0;
        let prefix = integrals.entry(currency).or_default();
        prefix.insert(date_ctx.base, 0.0);
        for date in event_dates {
            let t_start = disc.day_count().signed_year_fraction(
                disc.base_date(),
                previous_date,
                DayCountContext::default(),
            )?;
            let t_end = disc.day_count().signed_year_fraction(
                disc.base_date(),
                date,
                DayCountContext::default(),
            )?;
            let sp = survival_on_date(date, Some(hazard), &date_ctx)?;
            let midpoint_df = disc.df_between_times(t_asof, 0.5 * (t_start + t_end))?;
            if !midpoint_df.is_finite() || midpoint_df < 0.0 {
                return Err(finstack_quant_core::Error::Validation(
                    "midpoint discount factor must be finite and non-negative".into(),
                ));
            }
            integral.add(midpoint_df * default_mass(previous_survival, sp)?);
            prefix.insert(date, integral.total());
            previous_date = date;
            previous_survival = sp;
        }
    }
    for (index, (cf, lots)) in flows.iter().zip(funding).enumerate() {
        if let Some(prefix) = integrals.get(&cf.amount.currency()) {
            for (start, amount) in lots {
                let integrated_mass = prefix[&cf.date] - prefix[&start];
                out[index] += recovery * amount * integrated_mass;
            }
        }
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
mod period_aggregation_tests {
    use super::*;

    #[test]
    fn checked_map_construction_rejects_currency_mismatch() {
        let period = PeriodId::quarter(2025, 1).expect("valid period");
        let mut per_currency = IndexMap::new();
        per_currency.insert(Currency::USD, Money::from((100_i64, Currency::EUR)));
        let mut map = IndexMap::new();
        map.insert(period, per_currency);

        assert!(matches!(
            PeriodAggregation::try_from(map),
            Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: Currency::USD,
                actual: Currency::EUR,
            })
        ));
    }

    #[test]
    fn serde_rejects_currency_mismatch() {
        let wire = r#"{"2025Q1":{"USD":{"amount":"100","currency":"EUR"}}}"#;
        let error = serde_json::from_str::<PeriodAggregation>(wire)
            .expect_err("mislabeled currency must fail");
        assert!(error.to_string().contains("USD"));
        assert!(error.to_string().contains("EUR"));
    }

    #[test]
    fn checked_map_and_serde_preserve_valid_currency_totals() {
        let period = PeriodId::quarter(2025, 1).expect("valid period");
        let mut per_currency = IndexMap::new();
        per_currency.insert(Currency::USD, Money::from((100_i64, Currency::USD)));
        per_currency.insert(Currency::EUR, Money::from((200_i64, Currency::EUR)));
        let mut map = IndexMap::new();
        map.insert(period, per_currency);
        let totals = PeriodAggregation::try_from(map).expect("matching currencies");
        let wire = serde_json::to_string(&totals).expect("serialize totals");
        let restored: PeriodAggregation = serde_json::from_str(&wire).expect("valid totals");

        assert_eq!(restored, totals);
        assert_eq!(
            restored.rows(),
            vec![
                (period, Currency::USD, Money::from((100_i64, Currency::USD))),
                (period, Currency::EUR, Money::from((200_i64, Currency::EUR))),
            ]
        );
    }
}

#[cfg(test)]
mod period_contract_tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Period, PeriodId};
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

    impl Discounting for UnitDiscount {
        fn id(&self) -> &CurveId {
            static ID: std::sync::LazyLock<CurveId> = std::sync::LazyLock::new(|| "unit".into());
            &ID
        }

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

    impl Discounting for NanDiscount {
        fn id(&self) -> &CurveId {
            static ID: std::sync::LazyLock<CurveId> = std::sync::LazyLock::new(|| "nan".into());
            &ID
        }

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
        let flows = vec![(d(2025, 3, 15), Money::from((100_i64, Currency::USD)))];
        let periods = vec![
            period(
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                d(2025, 4, 1),
                d(2025, 7, 1),
            ),
            period(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                d(2025, 1, 1),
                d(2025, 4, 1),
            ),
        ];
        let err = aggregate_by_period(&flows, &periods).expect_err("unsorted periods rejected");
        assert!(format!("{err}").contains("sorted"), "got: {err}");
    }

    #[test]
    fn aggregate_by_period_rejects_overlapping_periods() {
        let flows = vec![(d(2025, 3, 15), Money::from((100_i64, Currency::USD)))];
        let periods = vec![
            period(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                d(2025, 1, 1),
                d(2025, 5, 1),
            ),
            period(
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                d(2025, 4, 1),
                d(2025, 7, 1),
            ),
        ];
        let err = aggregate_by_period(&flows, &periods).expect_err("overlapping periods rejected");
        assert!(format!("{err}").contains("non-overlapping"), "got: {err}");
    }

    #[test]
    fn aggregate_by_period_rejects_duplicate_period_ids() {
        let flows = vec![(d(2025, 3, 15), Money::from((100_i64, Currency::USD)))];
        let periods = vec![
            period(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                d(2025, 1, 1),
                d(2025, 4, 1),
            ),
            period(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                d(2025, 4, 1),
                d(2025, 7, 1),
            ),
        ];
        let err = aggregate_by_period(&flows, &periods).expect_err("duplicate ids rejected");
        assert!(format!("{err}").contains("duplicate"), "got: {err}");
    }

    #[test]
    fn aggregate_by_period_preserves_currency_separation() {
        // Two currencies through aggregate_by_period: per-currency map outputs
        // are separate and no cross-currency summation occurs.
        let flows = vec![
            (d(2025, 2, 1), Money::from((100_i64, Currency::USD))),
            (d(2025, 2, 15), Money::from((70_i64, Currency::EUR))),
            (d(2025, 3, 1), Money::from((50_i64, Currency::USD))),
        ];
        let periods = vec![period(
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            d(2025, 1, 1),
            d(2025, 4, 1),
        )];

        let out = aggregate_by_period(&flows, &periods).expect("aggregation succeeds");
        let q1 = out
            .get(&PeriodId::quarter(2025, 1).expect("valid period fixture"))
            .expect("Q1 present");
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
        let flows = vec![(boundary, Money::from((100_i64, Currency::USD)))];
        let periods = vec![
            period(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                d(2025, 1, 1),
                d(2025, 4, 1),
            ),
            period(
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                d(2025, 4, 1),
                d(2025, 7, 1),
            ),
        ];

        let out = aggregate_by_period(&flows, &periods).expect("aggregation succeeds");
        assert!(!out.contains_key(&PeriodId::quarter(2025, 1).expect("valid period fixture")));
        assert!(
            (out[&PeriodId::quarter(2025, 2).expect("valid period fixture")][&Currency::USD]
                .amount()
                - 100.0)
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn pv_by_period_errors_on_nan_discount_curve() {
        let base = d(2025, 1, 1);
        let flows = vec![CashFlow::new(
            d(2025, 6, 1),
            None,
            Money::from((100_i64, Currency::USD)),
            CFKind::Fixed,
            0.5,
            None,
        )];
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
            CashFlow::new(
                d(2025, 2, 1),
                None,
                Money::from((100_i64, Currency::USD)),
                CFKind::Fixed,
                0.25,
                None,
            ),
            CashFlow::new(
                base,
                None,
                Money::from((50_i64, Currency::USD)),
                CFKind::Fixed,
                0.25,
                None,
            ),
            CashFlow::new(
                d(2025, 6, 1),
                None,
                Money::from((200_i64, Currency::USD)),
                CFKind::Fixed,
                0.25,
                None,
            ),
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

    impl Discounting for FlatDiscount {
        fn id(&self) -> &CurveId {
            static ID: std::sync::LazyLock<CurveId> = std::sync::LazyLock::new(|| "test".into());
            &ID
        }

        fn base_date(&self) -> Date {
            self.base
        }
        fn df(&self, _t: f64) -> f64 {
            1.0
        }
    }

    struct FlatSurvival;

    impl Survival for FlatSurvival {
        fn id(&self) -> &CurveId {
            static ID: std::sync::LazyLock<CurveId> = std::sync::LazyLock::new(|| "hzd".into());
            &ID
        }

        fn sp(&self, _t: f64) -> f64 {
            0.95
        }
    }

    fn make_period(base: Date, end: Date) -> Period {
        Period {
            id: PeriodId::quarter(base.year(), 1).expect("valid period fixture"),
            start: base,
            end,
            is_actual: false,
        }
    }

    fn flow(date: Date, amount: f64, kind: CFKind) -> CashFlow {
        CashFlow::new(
            date,
            None,
            Money::new(amount, Currency::USD).expect("valid money fixture"),
            kind,
            0.0,
            None,
        )
    }

    /// Linearly decaying survival without a base date: `sp(t) = 1 − 0.1·t`.
    struct LinearSurvival;

    impl Survival for LinearSurvival {
        fn id(&self) -> &CurveId {
            static ID: std::sync::LazyLock<CurveId> =
                std::sync::LazyLock::new(|| "hzd-linear".into());
            &ID
        }

        fn sp(&self, t: f64) -> f64 {
            1.0 - 0.1 * t
        }
    }

    struct AnnualSurvival;

    impl Survival for AnnualSurvival {
        fn id(&self) -> &CurveId {
            static ID: std::sync::LazyLock<CurveId> =
                std::sync::LazyLock::new(|| "annual-survival".into());
            &ID
        }

        fn sp(&self, t: f64) -> f64 {
            0.8_f64.powf(t)
        }
    }

    fn funded_pvs(flows: &[CashFlow], base: Date, timing: RecoveryTiming) -> Vec<f64> {
        let ctx = DateContext::new(base, DayCount::Act365F, DayCountContext::default());
        let discounts = vec![1.0; flows.len()];
        match timing {
            RecoveryTiming::AtPaymentDate => credit_adjusted_cashflow_pvs(
                flows,
                &discounts,
                Some(&AnnualSurvival),
                Some(0.4),
                ctx,
            )
            .expect("funded payment-date PVs"),
            RecoveryTiming::AtDefaultIntegrated => precompute_integrated_pv(
                flows,
                &discounts,
                &FlatDiscount { base },
                &AnnualSurvival,
                Some(0.4),
                ctx,
            )
            .expect("funded midpoint PVs"),
        }
    }

    #[test]
    fn forward_funding_has_no_recovery_before_advance_and_rows_match_periods() {
        let base = d(2025, 1, 1);
        let flows = vec![
            flow(d(2026, 1, 1), -100.0, CFKind::Notional),
            flow(d(2027, 1, 1), 100.0, CFKind::Notional),
        ];
        for timing in [
            RecoveryTiming::AtPaymentDate,
            RecoveryTiming::AtDefaultIntegrated,
        ] {
            let pvs = funded_pvs(&flows, base, timing);
            let expected = -100.0 * 0.8 + 100.0 * 0.64 + 0.4 * 100.0 * (0.8 - 0.64);
            assert!((pvs.iter().sum::<f64>() - expected).abs() < 1e-10);
            let periods = [make_period(base, d(2027, 1, 2))];
            let aggregate = pv_by_period_credit_adjusted_detailed_with_timing(
                &flows,
                &periods,
                &FlatDiscount { base },
                Some(&AnnualSurvival),
                Some(0.4),
                timing,
                DateContext::new(base, DayCount::Act365F, DayCountContext::default()),
            )
            .expect("period PV");
            assert!((aggregate[&periods[0].id][&Currency::USD].amount() - expected).abs() < 1e-10);
        }
    }

    #[test]
    fn funded_recovery_discount_timing_matches_independent_interval_formula() {
        struct ExponentialDiscount {
            base: Date,
        }

        impl Discounting for ExponentialDiscount {
            fn id(&self) -> &CurveId {
                static ID: std::sync::LazyLock<CurveId> =
                    std::sync::LazyLock::new(|| "recovery-discount".into());
                &ID
            }

            fn base_date(&self) -> Date {
                self.base
            }

            fn df(&self, t: f64) -> f64 {
                (-0.05 * t).exp()
            }
        }

        let base = d(2025, 1, 1);
        // The curve starts one Act/365F year before valuation, exercising
        // relative discounting for both settlement dates and recovery midpoints.
        let discount = ExponentialDiscount {
            base: d(2024, 1, 2),
        };
        let flows = [
            flow(d(2026, 1, 1), -100.0, CFKind::Notional),
            flow(d(2027, 1, 1), 40.0, CFKind::Amortization),
            flow(d(2028, 1, 1), 60.0, CFKind::Notional),
        ];
        let periods = [make_period(base, d(2028, 1, 2))];
        let df = |years: f64| (-0.05 * years).exp();
        let settlement = -100.0 * df(1.0) * 0.8 + 40.0 * df(2.0) * 0.64 + 60.0 * df(3.0) * 0.512;
        let payment_recovery =
            0.4 * (40.0 * df(2.0) * (0.8 - 0.64) + 60.0 * df(3.0) * (0.8 - 0.512));
        // No default recovery before year-one funding. The funded principal
        // is 100 over (1, 2] and 60 over (2, 3], discounted at each midpoint.
        let midpoint_recovery =
            0.4 * (100.0 * df(1.5) * (0.8 - 0.64) + 60.0 * df(2.5) * (0.64 - 0.512));
        assert!(midpoint_recovery > payment_recovery);

        for (timing, recovery) in [
            (RecoveryTiming::AtPaymentDate, payment_recovery),
            (RecoveryTiming::AtDefaultIntegrated, midpoint_recovery),
        ] {
            let pv = pv_by_period_credit_adjusted_detailed_with_timing(
                &flows,
                &periods,
                &discount,
                Some(&AnnualSurvival),
                Some(0.4),
                timing,
                DateContext::new(base, DayCount::Act365F, DayCountContext::default()),
            )
            .expect("funded recovery with nonzero discount rate");
            let actual = pv[&periods[0].id][&Currency::USD].amount();
            let expected = settlement + recovery;
            assert!(
                (actual - expected).abs() < 1e-10,
                "{timing:?}: {actual} != {expected}"
            );
        }
    }

    #[test]
    fn funded_amortization_draws_and_pik_preserve_dated_exposure() {
        let base = d(2025, 1, 1);
        let scenarios = [
            (
                vec![
                    flow(d(2024, 1, 1), -100.0, CFKind::Notional),
                    flow(d(2026, 1, 1), 40.0, CFKind::Amortization),
                    flow(d(2027, 1, 1), 60.0, CFKind::Notional),
                ],
                40.0 * (0.8 + 0.4 * 0.2) + 60.0 * (0.64 + 0.4 * 0.36),
            ),
            (
                vec![
                    flow(d(2026, 1, 1), -50.0, CFKind::RevolvingDraw),
                    flow(d(2027, 1, 1), 150.0, CFKind::RevolvingRepayment),
                ],
                -50.0 * 0.8 + 150.0 * 0.64 + 0.4 * (100.0 * 0.36 + 50.0 * 0.16),
            ),
            (
                vec![
                    flow(d(2026, 1, 1), 5.0, CFKind::Pik),
                    flow(d(2027, 1, 1), 105.0, CFKind::Notional),
                ],
                105.0 * 0.64 + 0.4 * (100.0 * 0.36 + 5.0 * 0.16),
            ),
        ];
        for (flows, expected) in scenarios {
            for timing in [
                RecoveryTiming::AtPaymentDate,
                RecoveryTiming::AtDefaultIntegrated,
            ] {
                let actual = funded_pvs(&flows, base, timing).iter().sum::<f64>();
                assert!(
                    (actual - expected).abs() < 1e-10,
                    "{timing:?}: {actual} != {expected}"
                );
            }
        }
    }

    #[test]
    fn funding_is_currency_separated_and_same_day_advances_can_be_repaid() {
        let base = d(2025, 1, 1);
        let mut euro = flow(d(2027, 1, 1), 100.0, CFKind::Notional);
        euro.amount = Money::from((100_i64, Currency::EUR));
        let flows = vec![
            flow(d(2026, 1, 1), 40.0, CFKind::Amortization),
            euro,
            flow(d(2026, 1, 1), -100.0, CFKind::Notional),
            flow(d(2027, 1, 1), 60.0, CFKind::Notional),
        ];
        for timing in [
            RecoveryTiming::AtPaymentDate,
            RecoveryTiming::AtDefaultIntegrated,
        ] {
            let pvs = funded_pvs(&flows, base, timing);
            assert!((pvs[0] - 32.0).abs() < 1e-10);
            assert!((pvs[1] - 78.4).abs() < 1e-10);
            assert!((pvs[2] + 80.0).abs() < 1e-10);
            assert!((pvs[3] - 42.24).abs() < 1e-10);
        }
    }

    #[test]
    fn explicit_principal_deltas_determine_recovery_independently_of_cash_price() {
        let base = d(2025, 1, 1);
        for repayment_cash in [90.0, 110.0] {
            let flows = vec![
                flow(d(2026, 1, 1), -90.0, CFKind::Notional)
                    .with_principal_delta(Money::from((100_i64, Currency::USD))),
                flow(d(2027, 1, 1), repayment_cash, CFKind::Amortization)
                    .with_principal_delta(Money::from((-100_i64, Currency::USD))),
            ];
            for timing in [
                RecoveryTiming::AtPaymentDate,
                RecoveryTiming::AtDefaultIntegrated,
            ] {
                let actual = funded_pvs(&flows, base, timing).iter().sum::<f64>();
                let expected = -90.0 * 0.8 + repayment_cash * 0.64 + 100.0 * 0.4 * 0.16;
                assert!((actual - expected).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn early_paid_final_pik_has_no_recovery_before_capitalization() {
        let base = d(2025, 8, 1);
        let payment = d(2025, 8, 29);
        let capitalization = d(2025, 8, 31);
        let mut pik = flow(payment, 5.0, CFKind::Pik);
        pik.principal_date = Some(capitalization);
        let mut redemption = flow(payment, 105.0, CFKind::Notional);
        redemption.principal_date = Some(capitalization);
        redemption.principal_delta = Some(Money::from((-105_i64, Currency::USD)));
        for timing in [
            RecoveryTiming::AtPaymentDate,
            RecoveryTiming::AtDefaultIntegrated,
        ] {
            let pvs = funded_pvs(&[pik.clone(), redemption.clone()], base, timing);
            let survival = 0.8_f64.powf(28.0 / 365.0);
            let expected = 105.0 * survival + 100.0 * 0.4 * (1.0 - survival);
            assert!((pvs.iter().sum::<f64>() - expected).abs() < 1e-10);
            let settled = funded_pvs(&[pik.clone(), redemption.clone()], d(2025, 8, 30), timing);
            assert_eq!(settled, vec![0.0, 0.0]);
            let replay = [
                flow(capitalization, 0.0, CFKind::Notional)
                    .with_principal_delta(Money::from((5_i64, Currency::USD))),
                flow(capitalization, 0.0, CFKind::Notional)
                    .with_principal_delta(Money::from((-105_i64, Currency::USD))),
            ];
            assert_eq!(funded_pvs(&replay, d(2025, 8, 30), timing), vec![0.0, 0.0]);

            // An independent later claim must not absorb capitalization that
            // was extinguished by this already-paid early redemption.
            let later = flow(d(2026, 8, 30), 100.0, CFKind::Notional);
            let mut with_later = replay.to_vec();
            with_later.push(later.clone());
            let remaining = funded_pvs(&with_later, d(2025, 8, 30), timing);
            assert!((remaining[2] - 88.0).abs() < 1e-10);
            let raw_remaining = funded_pvs(
                &[pik.clone(), redemption.clone(), later.clone()],
                d(2025, 8, 30),
                timing,
            );
            assert_eq!(raw_remaining, remaining);
            let before_payment = funded_pvs(
                &[pik.clone(), redemption.clone(), later.clone()],
                base,
                timing,
            );
            let later_alone = funded_pvs(&[later], base, timing);
            assert!((before_payment[2] - later_alone[0]).abs() < 1e-10);
        }
    }

    #[test]
    fn pending_settlement_recovery_agrees_before_and_after_normalization() {
        let base = d(2025, 11, 5);
        for cash in [90.0, 110.0] {
            let mut raw = flow(d(2025, 11, 10), cash, CFKind::Amortization)
                .with_principal_delta(Money::from((-100_i64, Currency::USD)));
            raw.principal_date = Some(d(2025, 11, 1));
            let mut normalized = raw.clone();
            normalized.principal_delta = Some(Money::from((0_i64, Currency::USD)));
            for timing in [
                RecoveryTiming::AtPaymentDate,
                RecoveryTiming::AtDefaultIntegrated,
            ] {
                let raw_pv = funded_pvs(&[raw.clone()], base, timing);
                let normalized_pv = funded_pvs(&[normalized.clone()], base, timing);
                assert_eq!(raw_pv, normalized_pv);
                let survival = 0.8_f64.powf(5.0 / 365.0);
                assert!((raw_pv[0] - cash * (survival + 0.4 * (1.0 - survival))).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn batch_pv_rejects_invalid_survival_probability() {
        struct InvalidSurvival;
        impl Survival for InvalidSurvival {
            fn id(&self) -> &CurveId {
                AnnualSurvival.id()
            }
            fn sp(&self, _: f64) -> f64 {
                -0.01
            }
        }
        let error = credit_adjusted_cashflow_pvs(
            &[flow(d(2026, 1, 1), 100.0, CFKind::Notional)],
            &[1.0],
            Some(&InvalidSurvival),
            Some(0.4),
            DateContext::new(d(2025, 1, 1), DayCount::Act365F, DayCountContext::default()),
        )
        .expect_err("invalid survival must fail");
        assert!(error.to_string().contains("survival probability"));
    }

    #[test]
    fn integrated_timing_zeroes_pik_flows() {
        // PIK is a capitalization event, not cash: its value is already carried
        // by the inflated notional redemption. Both recovery-timing modes must
        // zero it identically.
        let base = d(2025, 1, 1);
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let disc = FlatDiscount { base };
        let hazard = FlatSurvival;
        let flows = vec![
            flow(d(2025, 6, 1), 50_000.0, CFKind::Fixed),
            flow(d(2025, 6, 1), 40_000.0, CFKind::Pik),
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
        .expect("integrated timing prices");

        let pv = out[&PeriodId::quarter(2025, 1).expect("valid period fixture")][&Currency::USD]
            .amount();
        let expected = 50_000.0 * 0.95;
        assert!(
            (pv - expected).abs() < 1e-9,
            "PIK must carry zero PV under integrated timing: got {pv}, expected {expected}"
        );
    }

    #[test]
    fn revolving_repayment_recovery_applies_under_payment_date_timing() {
        // RevolvingRepayment is a principal claim; the payment-date path must
        // apply the same R·(1−SP) recovery term the integrated path applies.
        let base = d(2025, 1, 1);
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let disc = FlatDiscount { base };
        let hazard = FlatSurvival;
        let flows = vec![flow(d(2025, 6, 1), 100_000.0, CFKind::RevolvingRepayment)];

        let out = pv_by_period_credit_adjusted_detailed_with_timing(
            &flows,
            &periods,
            &disc,
            Some(&hazard),
            Some(0.40),
            RecoveryTiming::AtPaymentDate,
            DateContext::new(base, DayCount::Act365F, DayCountContext::default()),
        )
        .expect("payment-date timing prices");

        let pv = out[&PeriodId::quarter(2025, 1).expect("valid period fixture")][&Currency::USD]
            .amount();
        let expected = 100_000.0 * (0.95 + 0.40 * 0.05);
        assert!(
            (pv - expected).abs() < 1e-9,
            "revolving repayment must earn recovery on default: got {pv}, expected {expected}"
        );
    }

    #[test]
    fn integrated_recovery_skips_negative_principal_draws() {
        // Future funding is survival-weighted cash, and recovery begins only
        // once that draw creates funded exposure.
        let base = d(2025, 1, 1);
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let disc = FlatDiscount { base };
        let hazard = LinearSurvival;
        let flows = vec![
            flow(d(2025, 6, 1), -500_000.0, CFKind::Notional),
            flow(d(2025, 12, 1), 500_000.0, CFKind::Amortization),
        ];

        let out = pv_by_period_credit_adjusted_detailed_with_timing(
            &flows,
            &periods,
            &disc,
            Some(&hazard),
            Some(0.40),
            RecoveryTiming::AtDefaultIntegrated,
            DateContext::new(base, DayCount::Act365F, DayCountContext::default()),
        )
        .expect("negative draws must not corrupt integrated recovery");

        let pv = out[&PeriodId::quarter(2025, 1).expect("valid period fixture")][&Currency::USD]
            .amount();
        let t1 = 151.0 / 365.0; // 2025-01-01 → 2025-06-01
        let t2 = 334.0 / 365.0; // 2025-01-01 → 2025-12-01
        let sp1 = 1.0 - 0.1 * t1;
        let sp2 = 1.0 - 0.1 * t2;
        // Draw: survival-weighted cash PV only. Amortization: survival-weighted
        // PV plus recovery integrated only over (draw date, repayment date].
        let expected = -500_000.0 * sp1 + 500_000.0 * sp2 + 0.40 * 500_000.0 * (sp1 - sp2);
        assert!(
            (pv - expected).abs() < 1e-6,
            "got {pv}, expected {expected}"
        );
    }

    #[test]
    fn aggregate_by_period_errors_on_decimal_overflow() {
        // Two near-Decimal-max flows in one period overflow the Decimal range
        // when materialized as Money; that must surface as Err, not a panic.
        let base = d(2025, 1, 1);
        let periods = vec![make_period(base, d(2026, 1, 1))];
        let flows: Vec<crate::DatedFlow> = vec![
            (
                d(2025, 3, 1),
                Money::new(7.0e28, Currency::USD).expect("valid money fixture"),
            ),
            (
                d(2025, 6, 1),
                Money::new(7.0e28, Currency::USD).expect("valid money fixture"),
            ),
        ];

        let res = aggregate_by_period(&flows, &periods);
        assert!(res.is_err(), "Decimal-overflow sums must error, not panic");
    }

    #[test]
    fn rejects_defaulted_notional_with_recovery_rate() {
        let base = d(2025, 1, 1);
        let flows = vec![
            CashFlow::new(
                d(2025, 6, 1),
                None,
                Money::from((100_000_i64, Currency::USD)),
                CFKind::DefaultedNotional,
                0.0,
                None,
            ),
            CashFlow::new(
                d(2025, 12, 1),
                None,
                Money::from((900_000_i64, Currency::USD)),
                CFKind::Amortization,
                0.0,
                None,
            ),
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
            CashFlow::new(
                d(2025, 6, 1),
                None,
                Money::from((100_000_i64, Currency::USD)),
                CFKind::DefaultedNotional,
                0.0,
                None,
            ),
            CashFlow::new(
                d(2025, 12, 1),
                None,
                Money::from((900_000_i64, Currency::USD)),
                CFKind::Amortization,
                0.0,
                None,
            ),
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
        let unsorted = vec![
            sorted[2].clone(),
            sorted[0].clone(),
            sorted[3].clone(),
            sorted[1].clone(),
        ];

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
        // Conditional survival at the valuation date is one. With zero
        // discount rates, the entire 5% default mass is recovered identically
        // under payment-date and interval-midpoint recovery.
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

        let expected = 1_000_000.0 * (0.95 + 0.4 * 0.05);
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
        impl Survival for StepSurvival {
            fn id(&self) -> &CurveId {
                static ID: std::sync::LazyLock<CurveId> =
                    std::sync::LazyLock::new(|| "step".into());
                &ID
            }

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
        impl Survival for LinearSurvival {
            fn id(&self) -> &CurveId {
                static ID: std::sync::LazyLock<CurveId> = std::sync::LazyLock::new(|| "lin".into());
                &ID
            }

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
            id: PeriodId::quarter(2025, 3).expect("valid period fixture"),
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

#[cfg(test)]
mod calendar_year_ladder_tests {
    use super::*;
    use finstack_quant_core::dates::Date;
    use time::Month;

    fn d(y: i32, m: u8, day: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("valid month"), day)
            .expect("valid date")
    }

    #[test]
    fn calendar_year_ladder_splits_coupon_and_principal_by_year() {
        let dates = [d(2027, 3, 15), d(2027, 9, 15), d(2034, 3, 15)];
        let kinds = ["coupon", "fixed", "Notional"];
        let amounts = [212_500.0, 212_500.0, 10_000_000.0];
        let pvs = [208_000.0, 206_000.0, 7_100_000.0];
        let rows = calendar_year_ladder(&dates, &kinds, &amounts, &pvs).expect("ladder");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].year, 2027);
        assert!((rows[0].non_principal - 425_000.0).abs() < 1e-9);
        assert!((rows[0].principal).abs() < 1e-12);
        assert_eq!(rows[1].year, 2034);
        assert!((rows[1].principal - 10_000_000.0).abs() < 1e-9);
        assert!((rows[1].pv - 7_100_000.0).abs() < 1e-9);
    }

    #[test]
    fn calendar_year_ladder_rejects_length_mismatch() {
        let err = calendar_year_ladder(&[d(2027, 1, 1)], &["fixed"], &[1.0], &[])
            .expect_err("length mismatch");
        assert!(err.to_string().contains("equal lengths"));
    }

    #[test]
    fn calendar_year_ladder_rejects_unknown_kind() {
        let err = calendar_year_ladder(&[d(2027, 1, 1)], &["redemption"], &[1.0], &[0.9])
            .expect_err("unknown kind");
        assert!(
            err.to_string().contains("unknown cashflow kind"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn calendar_year_ladder_rejects_non_finite_amount_or_pv() {
        let amount_err = calendar_year_ladder(&[d(2027, 1, 1)], &["fixed"], &[f64::NAN], &[1.0])
            .expect_err("nan amount");
        assert!(amount_err.to_string().contains("amount must be finite"));
        let pv_err = calendar_year_ladder(&[d(2027, 1, 1)], &["fixed"], &[1.0], &[f64::INFINITY])
            .expect_err("inf pv");
        assert!(pv_err.to_string().contains("pv must be finite"));
    }
}

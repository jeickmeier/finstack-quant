//! Period bucketing, [`PeriodAggregation`], and calendar-year ladders.

use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Period, PeriodId};
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::Money;

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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct PeriodAggregation(pub(super) IndexMap<PeriodId, IndexMap<Currency, Money>>);

impl PeriodAggregation {
    /// Read one period's currency totals by its exact display label.
    ///
    /// # Arguments
    ///
    /// * `period` - Exact case-sensitive `PeriodId` display label, such as
    ///   `"2025Q1"`. Whitespace is not trimmed and aliases are not parsed.
    ///
    /// # Returns
    ///
    /// A borrowed currency map, including an empty map if one was supplied
    /// during construction, or `None` for an absent or malformed label.
    #[must_use]
    pub fn get_period(&self, period: &str) -> Option<&IndexMap<Currency, Money>> {
        self.0
            .iter()
            .find(|(id, _)| id.to_string() == period)
            .map(|(_, per_currency)| per_currency)
    }

    /// Amount aggregated for one period code and currency.
    ///
    /// # Arguments
    ///
    /// * `period` - Period code exactly as the aggregation spells it (the
    ///   `Display` form of [`PeriodId`], for example `"2025Q1"`). An unknown
    ///   or malformed code is a miss, not an error.
    /// * `currency` - Currency bucket to read inside that period.
    ///
    /// # Returns
    ///
    /// The aggregated amount, or `None` when the period has no flows or no
    /// flows in `currency`.
    #[must_use]
    pub fn get_amount(&self, period: &str, currency: Currency) -> Option<Money> {
        self.get_period(period)
            .and_then(|per_currency| per_currency.get(&currency))
            .copied()
    }

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

/// Visit each non-empty `[period.start, period.end)` slice of date-sorted items.
pub(super) fn for_each_period_bucket<T>(
    sorted: &[T],
    periods: &[Period],
    date_of: fn(&T) -> Date,
    mut on_bucket: impl FnMut(&Period, usize, &[T]) -> finstack_quant_core::Result<()>,
) -> finstack_quant_core::Result<()> {
    debug_assert!(
        sorted.windows(2).all(|w| date_of(&w[0]) <= date_of(&w[1])),
        "period bucketing requires flows to be sorted by date"
    );

    let mut flow_idx = 0;
    let n = sorted.len();

    for p in periods {
        while flow_idx < n && date_of(&sorted[flow_idx]) < p.start {
            flow_idx += 1;
        }
        let bucket_start = flow_idx;
        while flow_idx < n && date_of(&sorted[flow_idx]) < p.end {
            flow_idx += 1;
        }
        if flow_idx > bucket_start {
            on_bucket(p, bucket_start, &sorted[bucket_start..flow_idx])?;
        }
    }
    Ok(())
}

/// Accumulate nominal [`Money`] values across non-overlapping reporting periods
/// using exact checked `Decimal` addition per currency.
pub(super) fn sum_by_period_money(
    sorted: &[(Date, Money)],
    periods: &[Period],
) -> finstack_quant_core::Result<PeriodAggregation> {
    let mut out: IndexMap<PeriodId, IndexMap<Currency, Money>> =
        IndexMap::with_capacity(periods.len());
    let mut per_currency: IndexMap<Currency, Money> = IndexMap::new();

    for_each_period_bucket(
        sorted,
        periods,
        |row| row.0,
        |p, _, bucket| {
            per_currency.clear();
            for &(_, amount) in bucket {
                let ccy = amount.currency();
                if let Some(existing) = per_currency.get_mut(&ccy) {
                    *existing = existing.checked_add(amount)?;
                } else {
                    per_currency.insert(ccy, amount);
                }
            }
            out.insert(p.id, std::mem::take(&mut per_currency));
            Ok(())
        },
    )?;
    Ok(PeriodAggregation(out))
}

/// Validate the aggregation period contract: sorted by start, non-overlapping,
/// and free of duplicate `PeriodId`s.
///
/// The flow-bucketing cursor in [`for_each_period_bucket`] never rewinds, so unsorted
/// or overlapping periods would silently drop flows; duplicate ids would
/// silently overwrite earlier results. Both are rejected loudly instead.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if periods are not sorted by
/// start date, overlap (a period starts before the previous period ends), or
/// share a `PeriodId`.
pub(super) fn validate_periods(periods: &[Period]) -> finstack_quant_core::Result<()> {
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
/// no cashflows are omitted from the result. Per-currency sums use exact
/// checked `Decimal` addition via [`Money::checked_add`] (no per-flow ISO-4217
/// rounding).
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if periods are unsorted,
/// overlapping, or contain duplicate `PeriodId`s, or a conversion/overflow
/// error if a currency sum exceeds the representable `Decimal` range.
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
    flows: &[(Date, Money)],
    periods: &[Period],
) -> finstack_quant_core::Result<PeriodAggregation> {
    validate_periods(periods)?;
    if flows.is_empty() || periods.is_empty() {
        return Ok(PeriodAggregation::default());
    }
    let sorted: std::borrow::Cow<'_, [(Date, Money)]> =
        if flows.windows(2).all(|w| w[0].0 <= w[1].0) {
            std::borrow::Cow::Borrowed(flows)
        } else {
            let mut rows = flows.to_vec();
            rows.sort_by_key(|(d, _)| *d);
            std::borrow::Cow::Owned(rows)
        };
    sum_by_period_money(&sorted, periods)
}

/// Currency-checked single-currency aggregation with explicit target currency.
///
/// - Empty input returns `Ok(0 target)`.
/// - All flows must match `target` currency; otherwise returns `Error::CurrencyMismatch`.
/// - Sums [`Money`] values with exact checked `Decimal` addition via
///   [`Money::checked_add`]; no per-flow ISO-4217 rounding is applied during
///   accumulation.
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
/// Returns `CurrencyMismatch` if any flow currency differs from `target`, or an
/// arithmetic overflow error if the cumulative `Decimal` sum exceeds the
/// representable range.
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
    flows: &[(Date, Money)],
    target: Currency,
) -> finstack_quant_core::Result<Money> {
    let mut total = Money::zero(target);
    for &(_d, m) in flows {
        total = total.checked_add(m)?;
    }
    Ok(total)
}

/// Calendar-year non-principal / principal / PV totals for one reporting year.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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
/// Kind labels use canonical `CFKind` names after trimming whitespace and
/// ignoring ASCII case. Unknown labels are rejected rather than silently treated as interest. The `non_principal` column is every
/// classified kind that is not [`CFKind::is_principal_like`] (interest, fees,
/// recovery, accrued-on-default, …).
///
/// The ladder is a cash ladder: rows whose kind is not
/// [`CFKind::is_cash_settlement`] (PIK capitalization, defaulted-notional
/// write-down) are validated but contribute to no column, so a year holding
/// only such rows has no ladder row (record CORE-CONVENTIONS-015 #1).
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
/// * `kind_labels` - Canonical cashflow kind labels such as `"fixed"`, `"notional"`,
///   or `"pre_payment"`. Surrounding whitespace and ASCII case are ignored; unknown labels error.
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
/// let rows = calendar_year_ladder(&dates, &["fixed", "notional"], &[100.0, 1000.0], &[90.0, 700.0])
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

    calendar_year_ladder_rows(dates.iter().zip(kind_labels).zip(amounts).zip(pvs).map(
        |(((&date, kind_label), &amount), &pv)| {
            let kind = kind_label
                .trim()
                .parse::<CFKind>()
                .map_err(|err| match err {
                    finstack_quant_core::Error::Validation(message) => {
                        finstack_quant_core::Error::Validation(format!(
                            "calendar_year_ladder {message}"
                        ))
                    }
                    other => other,
                });
            (date, kind, amount, pv)
        },
    ))
}

/// Accumulate typed rows in input order, preserving compensated summation.
pub(crate) fn calendar_year_ladder_rows(
    rows: impl IntoIterator<Item = (Date, finstack_quant_core::Result<CFKind>, f64, f64)>,
) -> finstack_quant_core::Result<Vec<CalendarYearLadderRow>> {
    let mut by_year: std::collections::BTreeMap<i32, [NeumaierAccumulator; 3]> =
        std::collections::BTreeMap::new();
    for (date, kind, amount, pv) in rows {
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
        let kind = kind?;
        if !kind.is_cash_settlement() {
            continue;
        }
        let slot = by_year.entry(date.year()).or_insert_with(|| {
            [
                NeumaierAccumulator::new(),
                NeumaierAccumulator::new(),
                NeumaierAccumulator::new(),
            ]
        });
        if kind.is_principal_like() {
            slot[1].add(amount)?;
        } else {
            slot[0].add(amount)?;
        }
        slot[2].add(pv)?;
    }

    by_year
        .into_iter()
        .map(|(year, [non_principal, principal, pv])| {
            Ok(CalendarYearLadderRow {
                year,
                non_principal: non_principal.get_total()?,
                principal: principal.get_total()?,
                pv: pv.get_total()?,
            })
        })
        .collect()
}

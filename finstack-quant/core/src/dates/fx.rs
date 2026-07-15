//! FX date utilities for joint calendar adjustments and spot rolls.
//!
//! These helpers implement the common two-calendar rule used in FX settlement:
//! a date is valid only if it is a business day in both the base-currency and
//! quote-currency calendars. They intentionally model only that joint-calendar
//! rule; additional settlement calendars such as USD must be supplied
//! explicitly by the caller when market convention requires them.
//!
//! # FX spot-date convention (CLS-consistent)
//!
//! The market convention for rolling trade date to spot is **asymmetric in
//! USD** ([`fx_spot_date`]):
//!
//! - **Intermediate days** (days counted toward T+n before the value date) must
//!   be business days in every *non-USD* currency calendar. A USD holiday on an
//!   intermediate day does **not** delay spot — e.g. EUR/USD traded Thu
//!   2025-07-03 (Fri Jul 4 is a US holiday) still settles Mon 2025-07-07, not
//!   Tue 2025-07-08.
//! - **The final value date** must be a business day in *all* calendars: both
//!   currencies and, for non-USD crosses, USD as well (the cross settles via
//!   two USD legs). If the T+n candidate fails, spot rolls forward to the next
//!   date good in all calendars.
//! - **T+0** requests return the trade date unchanged (no adjustment), matching
//!   [`add_joint_business_days`].
//!
//! The symmetric helper [`add_joint_business_days`] gates every counted day on
//! all supplied calendars; use [`fx_spot_date`] when the USD asymmetry matters.
//!
//! # Typical usage
//!
//! - [`adjust_joint_calendar`] for applying a business-day convention on a joint calendar
//! - [`add_joint_business_days`] for T+n style symmetric joint counting
//! - [`fx_spot_date`] for market-convention (USD-aware) spot rolling
//!
//! # References
//!
//! - Business-day conventions:
//!   `docs/REFERENCES.md#isda-2006-definitions`
//! - FX settlement / spot-lag conventions: CLS settlement rules; see also
//!   (FX spot lag finding)

use crate::dates::{
    adjust, available_calendars, calendar_by_id, BusinessDayConvention, CompositeCalendar, Date,
    HolidayCalendar, WEEKENDS_ONLY,
};
use crate::{Error, Result};
use time::Duration;

/// Resolve a calendar ID to a calendar reference.
///
/// # Errors
///
/// Returns an error if the calendar ID is not recognized.
/// If `cal_id` is `None`, returns the weekends-only calendar (does not error).
///
/// # Examples
///
/// ```
/// # use finstack_quant_core::dates::fx::resolve_calendar;
/// // Valid calendar ID
/// let cal = resolve_calendar(Some("nyse")).expect("NYSE calendar should exist");
///
/// // Explicit None uses weekends-only calendar (no error)
/// let weekends = resolve_calendar(None).expect("None should use weekends-only");
///
/// // Unknown calendar ID errors
/// let err = resolve_calendar(Some("unknown_cal"));
/// assert!(err.is_err());
/// ```
pub fn resolve_calendar(cal_id: Option<&str>) -> Result<&'static dyn HolidayCalendar> {
    if let Some(id) = cal_id {
        if let Some(resolved) = calendar_by_id(id) {
            return Ok(resolved);
        }

        // Error instead of silent fallback
        return Err(Error::calendar_not_found_with_suggestions(
            id,
            available_calendars(),
        ));
    }

    // Only use weekends_only if explicitly None (not as fallback)
    Ok(&WEEKENDS_ONLY)
}

fn with_joint_calendar<R>(
    base: &dyn HolidayCalendar,
    quote: &dyn HolidayCalendar,
    f: impl FnOnce(&CompositeCalendar<'_>) -> R,
) -> R {
    let calendars = [base, quote];
    let joint = CompositeCalendar::new(&calendars);
    f(&joint)
}

/// Adjust a date so it is a business day for both base and quote calendars.
///
/// Applies the business day convention on the true union calendar, where a day
/// is a holiday if either currency market is closed.
///
/// # Errors
///
/// Returns an error if:
/// - Either calendar ID is not recognized (see [`resolve_calendar`])
/// - Date adjustment fails
pub fn adjust_joint_calendar(
    date: Date,
    bdc: BusinessDayConvention,
    base_cal_id: Option<&str>,
    quote_cal_id: Option<&str>,
) -> Result<Date> {
    let base_cal = resolve_calendar(base_cal_id)?;
    let quote_cal = resolve_calendar(quote_cal_id)?;

    with_joint_calendar(base_cal, quote_cal, |joint_calendar| {
        adjust(date, bdc, joint_calendar)
    })
}

fn advance_business_days(
    start: Date,
    n_days: u32,
    is_business_day: impl Fn(Date) -> bool,
) -> Result<Date> {
    let max_iters = (n_days.saturating_mul(10).saturating_add(25)).max(1000);
    let mut date = start;
    let mut count = 0;
    let mut iters = 0;

    while count < n_days && iters < max_iters {
        date += Duration::days(1);
        if is_business_day(date) {
            count += 1;
        }
        iters += 1;
    }

    if count < n_days {
        return Err(Error::Input(
            crate::error::InputError::JointCalendarIterationLimitExceeded {
                start,
                n_days,
                max_iters,
            },
        ));
    }
    Ok(date)
}

/// Add N business days on all supplied calendars.
///
/// A day is counted only when it is a business day on both currency calendars
/// and, when provided, the settlement calendar.
///
/// # T+0 behavior
///
/// With `n_days = 0` the function returns `start` **unadjusted**, even when
/// `start` is not a business day on one or both calendars. T+0 (same-day)
/// settlement pairs that require a good business day must adjust separately,
/// e.g. via [`adjust_joint_calendar`].
///
/// # Arguments
///
/// * `start` - Starting date
/// * `n_days` - Number of joint business days to add
/// * `base_cal_id` - Optional calendar ID for the base currency
/// * `quote_cal_id` - Optional calendar ID for the quote currency
/// * `settlement_cal_id` - Optional third settlement calendar
///
/// # Returns
///
/// The date that is `n_days` joint business days after `start`.
///
/// # Errors
///
/// Returns an error if:
/// - Either calendar ID is not recognized (see [`resolve_calendar`])
/// - Too many iterations needed (>5x the requested days), suggesting a calendar configuration issue
///
/// For FX cross pairs that do not involve USD (e.g. EUR/JPY), USD-good days
/// matter because the cross settles as two USD legs. This helper implements the
/// **strict (conservative) variant**: every counted day — including the
/// intermediate day(s) **and the final value date** — must be good in USD as
/// well as in the two named currencies. Passing `settlement_cal_id =
/// Some("usny")` enforces that; `None` reproduces the two-calendar rule of
/// [`add_joint_business_days`].
///
/// # Convention note
///
/// The market convention implemented by [`fx_spot_date`] is asymmetric: USD
/// does not gate intermediate counted days, but it **does** gate the final value
/// date. This helper is symmetric and stricter when a settlement calendar is
/// supplied: that calendar gates every counted day, including intermediates
/// and the final value date. Use [`fx_spot_date`] for the USD-aware market rule;
/// use this helper only when symmetric joint-calendar counting is intended.
///
/// # Arguments
///
/// * `start` - Starting date
/// * `n_days` - Number of joint business days to add
pub fn add_joint_business_days(
    start: Date,
    n_days: u32,
    base_cal_id: Option<&str>,
    quote_cal_id: Option<&str>,
    settlement_cal_id: Option<&str>,
) -> Result<Date> {
    let base_cal = resolve_calendar(base_cal_id)?;
    let quote_cal = resolve_calendar(quote_cal_id)?;
    let settlement_cal = match settlement_cal_id {
        Some(id) => Some(resolve_calendar(Some(id))?),
        None => None,
    };

    advance_business_days(start, n_days, |date| {
        base_cal.is_business_day(date)
            && quote_cal.is_business_day(date)
            && settlement_cal.is_none_or(|calendar| calendar.is_business_day(date))
    })
}

/// Roll a trade date to spot using the market (CLS-consistent) FX convention,
/// which is **asymmetric in USD**.
///
/// Counting toward T+n:
///
/// - **Intermediate days** must be business days in every **non-USD** currency
///   calendar. The USD calendar does *not* gate intermediate days: a US holiday
///   at T+1 does not delay spot for a USD pair (nor for a cross).
/// - **The final value date** must be a business day in **all** calendars —
///   both currency calendars and, when supplied, the USD calendar (crosses
///   settle via two USD legs). If the T+n candidate is not good in all
///   calendars, spot rolls forward to the next date that is.
/// - `spot_lag_days == 0` returns `trade_date` unchanged (T+0 behavior matches
///   [`add_joint_business_days`]).
///
/// `usd_cal_id` names the USD settlement calendar (typically `"usny"`). When it
/// matches `base_cal_id` or `quote_cal_id` (case-insensitive), that side is
/// treated as the USD leg and excluded from intermediate-day gating. For
/// non-USD crosses, pass the USD calendar so the final value date is also
/// checked against USD. Passing `None` reproduces the symmetric two-calendar
/// rule of [`add_joint_business_days`].
///
/// # Arguments
///
/// * `trade_date` - The trade execution date
/// * `spot_lag_days` - Business days to spot (typically 2; 1 for e.g. USD/CAD)
/// * `base_cal_id` - Optional calendar ID for the base currency
/// * `quote_cal_id` - Optional calendar ID for the quote currency
/// * `usd_cal_id` - Optional USD settlement calendar ID (e.g. `Some("usny")`)
///
/// # Errors
///
/// Returns an error if any calendar ID is unrecognized, or if the iteration
/// limit is exceeded (suggesting a calendar configuration issue).
///
/// # Examples
///
/// ```
/// # use finstack_quant_core::dates::create_date;
/// # use time::Month;
/// # use finstack_quant_core::dates::fx::fx_spot_date;
/// // EUR/USD traded Thu 2025-07-03. Fri 2025-07-04 is a US holiday but a good
/// // EUR day, so it still counts as the first intermediate day; spot is
/// // Mon 2025-07-07 (good in both), not Tue 2025-07-08.
/// let trade = create_date(2025, Month::July, 3).unwrap();
/// let spot = fx_spot_date(trade, 2, Some("target2"), Some("usny"), Some("usny")).unwrap();
/// assert_eq!(spot, create_date(2025, Month::July, 7).unwrap());
/// ```
pub fn fx_spot_date(
    trade_date: Date,
    spot_lag_days: u32,
    base_cal_id: Option<&str>,
    quote_cal_id: Option<&str>,
    usd_cal_id: Option<&str>,
) -> Result<Date> {
    let base_cal = resolve_calendar(base_cal_id)?;
    let quote_cal = resolve_calendar(quote_cal_id)?;
    let usd_cal = match usd_cal_id {
        Some(id) => Some(resolve_calendar(Some(id))?),
        None => None,
    };

    if spot_lag_days == 0 {
        return Ok(trade_date);
    }

    let is_usd = |id: Option<&str>| matches!((id, usd_cal_id), (Some(a), Some(u)) if a.eq_ignore_ascii_case(u));
    let base_is_usd = is_usd(base_cal_id);
    let quote_is_usd = is_usd(quote_cal_id);

    // Intermediate days are gated by each currency's own calendar EXCEPT USD.
    let intermediate_good = |d: Date| {
        (base_is_usd || base_cal.is_business_day(d))
            && (quote_is_usd || quote_cal.is_business_day(d))
    };
    // The final value date must be good on ALL calendars (incl. USD).
    let final_good = |d: Date| {
        base_cal.is_business_day(d)
            && quote_cal.is_business_day(d)
            && usd_cal.is_none_or(|c| c.is_business_day(d))
    };

    let max_iters: u32 = (spot_lag_days.saturating_mul(10).saturating_add(25)).max(1000);
    let mut iters: u32 = 0;
    let mut date = trade_date;
    let mut count = 0u32;

    while count < spot_lag_days && iters < max_iters {
        date += Duration::days(1);
        if intermediate_good(date) {
            count += 1;
        }
        iters += 1;
    }

    // Roll the candidate value date forward until it is good in all calendars.
    while !final_good(date) && iters < max_iters {
        date += Duration::days(1);
        iters += 1;
    }

    if count < spot_lag_days || !final_good(date) {
        return Err(Error::Input(
            crate::error::InputError::JointCalendarIterationLimitExceeded {
                start: trade_date,
                n_days: spot_lag_days,
                max_iters,
            },
        ));
    }

    Ok(date)
}

// ============================================================================
// Batch-optimized variants (pre-resolved calendars)
// ============================================================================

/// Pre-resolved calendar pair for batch FX date operations.
///
/// Use this when processing many dates with the same calendar pair to avoid
/// repeated registry lookups. The calendars are resolved once and can be
/// reused for multiple operations.
///
/// # Example
///
/// ```
/// # use finstack_quant_core::dates::{create_date, BusinessDayConvention};
/// # use time::Month;
/// # use finstack_quant_core::dates::fx::ResolvedCalendarPair;
/// // Resolve once
/// let cals = ResolvedCalendarPair::resolve(Some("nyse"), Some("gblo"))
///     .expect("Valid calendars");
///
/// // Use many times without registry lookup overhead
/// for _ in 0..1000 {
///     let start = create_date(2024, Month::January, 15).unwrap();
///     let _result = cals
///         .add_joint_business_days(start, 2)
///         .expect("valid joint calendar roll");
/// }
/// ```
pub struct ResolvedCalendarPair {
    base: &'static dyn HolidayCalendar,
    quote: &'static dyn HolidayCalendar,
}

impl ResolvedCalendarPair {
    /// Resolve a calendar pair from optional calendar IDs.
    ///
    /// # Errors
    ///
    /// Returns an error if either calendar ID is not recognized.
    pub fn resolve(base_cal_id: Option<&str>, quote_cal_id: Option<&str>) -> Result<Self> {
        let base = resolve_calendar(base_cal_id)?;
        let quote = resolve_calendar(quote_cal_id)?;
        Ok(Self { base, quote })
    }

    /// Check if a date is a business day on both calendars.
    #[inline]
    pub fn is_joint_business_day(&self, date: Date) -> bool {
        self.base.is_business_day(date) && self.quote.is_business_day(date)
    }

    /// Add N business days using pre-resolved calendars.
    ///
    /// This is the batch-optimized variant of [`add_joint_business_days`]. Use this
    /// when processing many dates with the same calendar pair to avoid repeated
    /// registry lookups.
    ///
    /// # Arguments
    ///
    /// * `start` - Starting date
    /// * `n_days` - Number of joint business days to add
    ///
    /// # Returns
    ///
    /// The date that is `n_days` joint business days after `start`.
    pub fn add_joint_business_days(&self, start: Date, n_days: u32) -> Result<Date> {
        advance_business_days(start, n_days, |date| self.is_joint_business_day(date))
    }

    /// Adjust a date using pre-resolved calendars.
    ///
    /// This is the batch-optimized variant of [`adjust_joint_calendar`].
    ///
    /// # Arguments
    ///
    /// * `date` - Date to adjust
    /// * `bdc` - Business day convention
    ///
    /// # Returns
    ///
    /// The adjusted date that is a business day on both calendars.
    pub fn adjust_joint_calendar(&self, date: Date, bdc: BusinessDayConvention) -> Result<Date> {
        with_joint_calendar(self.base, self.quote, |joint_calendar| {
            adjust(date, bdc, joint_calendar)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::create_date;
    use time::Month;

    struct Jan29Holiday;
    struct Jan30And31Holidays;

    impl HolidayCalendar for Jan29Holiday {
        fn is_holiday(&self, date: Date) -> bool {
            date.year() == 2025 && date.month() == Month::January && date.day() == 29
        }
    }

    impl HolidayCalendar for Jan30And31Holidays {
        fn is_holiday(&self, date: Date) -> bool {
            date.year() == 2025 && date.month() == Month::January && matches!(date.day(), 30 | 31)
        }
    }

    static JAN_29_HOLIDAY: Jan29Holiday = Jan29Holiday;
    static JAN_30_AND_31_HOLIDAYS: Jan30And31Holidays = Jan30And31Holidays;

    #[test]
    fn test_add_joint_business_days_no_holidays() {
        // Test with weekends-only calendars (no holidays)
        let start = create_date(2024, Month::January, 15).unwrap(); // Monday
        let result = add_joint_business_days(
            start, 2, None, // weekends-only
            None, // weekends-only
            None,
        )
        .expect("Should succeed with no holidays");

        // 2 business days from Monday (Jan 15) should be Wednesday (Jan 17)
        let expected = create_date(2024, Month::January, 17).unwrap();
        assert_eq!(result, expected, "Should add 2 business days");
    }

    #[test]
    fn test_add_joint_business_days_base_holiday() {
        // Test when base calendar has a holiday
        // Using NYSE (US markets closed on holidays) vs weekends-only
        let start = create_date(2024, Month::January, 12).unwrap(); // Friday before MLK day (Jan 15, 2024)

        let result = add_joint_business_days(
            start,
            3,
            Some("nyse"), // NYSE closed on MLK day (Jan 15)
            None,         // weekends-only
            None,
        )
        .expect("Should succeed");

        // From Friday Jan 12:
        // - Skip Sat 13, Sun 14 (weekend)
        // - Skip Mon 15 (MLK day, NYSE closed)
        // - Count Tue 16 (business day 1)
        // - Count Wed 17 (business day 2)
        // - Count Thu 18 (business day 3)
        let expected = create_date(2024, Month::January, 18).unwrap();
        assert_eq!(result, expected, "Should skip MLK day on NYSE calendar");
    }

    #[test]
    fn test_add_joint_business_days_quote_holiday() {
        // Test when quote calendar has a holiday
        // Using weekends-only vs UK calendar
        let start = create_date(2024, Month::December, 23).unwrap(); // Monday before Christmas

        let result = add_joint_business_days(
            start,
            3,
            None,         // weekends-only
            Some("gblo"), // UK/London closed on Dec 25-26
            None,
        )
        .expect("Should succeed");

        // From Monday Dec 23:
        // - Count Tue 24 (business day 1, both calendars open)
        // - Skip Wed 25 (Christmas, GBLO closed)
        // - Skip Thu 26 (Boxing Day, GBLO closed)
        // - Count Fri 27 (business day 2)
        // - Skip Sat 28, Sun 29 (weekend)
        // - Count Mon 30 (business day 3)
        let expected = create_date(2024, Month::December, 30).unwrap();
        assert_eq!(result, expected, "Should skip UK holidays");
    }

    #[test]
    fn test_add_joint_business_days_both_holidays() {
        // Test when both calendars have holidays (joint closure)
        // New Year's Day is typically closed on both NYSE and GBLO
        let start = create_date(2023, Month::December, 29).unwrap(); // Friday before New Year

        let result = add_joint_business_days(
            start,
            2,
            Some("nyse"), // NYSE closed on Jan 1
            Some("gblo"), // GBLO closed on Jan 1
            None,
        )
        .expect("Should succeed");

        // From Friday Dec 29:
        // - Skip Sat 30, Sun 31 (weekend)
        // - Skip Mon Jan 1 (New Year's Day, both closed)
        // - Count Tue Jan 2 (business day 1, both open)
        // - Count Wed Jan 3 (business day 2, both open)
        let expected = create_date(2024, Month::January, 3).unwrap();
        assert_eq!(result, expected, "Should skip joint holidays");
    }

    #[test]
    fn test_add_joint_business_days_near_holiday() {
        // Test T+2 spot rolling near a holiday
        let trade_date = create_date(2024, Month::January, 12).unwrap(); // Friday before MLK day

        let spot_date = add_joint_business_days(
            trade_date,
            2, // T+2
            Some("nyse"),
            None,
            None,
        )
        .expect("Should succeed");

        // From Friday Jan 12, T+2 joint business days:
        // - Skip Sat 13, Sun 14 (weekend)
        // - Skip Mon 15 (MLK day, NYSE closed)
        // - Count Tue 16 (business day 1)
        // - Count Wed 17 (business day 2)
        let expected = create_date(2024, Month::January, 17).unwrap();
        assert_eq!(spot_date, expected, "T+2 should skip MLK day");
    }

    #[test]
    fn test_resolve_calendar_unknown_id() {
        // Test that unknown calendar IDs error
        let result = resolve_calendar(Some("unknown_calendar_id"));

        assert!(result.is_err(), "Unknown calendar ID should error");

        let err = result.err().expect("unknown calendar must error");
        assert!(
            matches!(err, Error::Input(ref e) if matches!(
                e,
                crate::error::InputError::CalendarNotFound { .. }
            )),
            "Should be CalendarNotFound error, got: {:?}",
            err
        );
    }

    #[test]
    fn test_resolve_calendar_explicit_none() {
        // Test that explicit None uses weekends-only without error
        let result = resolve_calendar(None);

        assert!(result.is_ok(), "Explicit None should not error");

        let cal = result.unwrap();
        // Verify it's the weekends-only calendar by checking behavior
        // Saturdays and Sundays should not be business days
        let saturday = create_date(2024, Month::January, 13).unwrap();
        let sunday = create_date(2024, Month::January, 14).unwrap();
        let monday = create_date(2024, Month::January, 15).unwrap();

        assert!(
            !cal.is_business_day(saturday),
            "Saturday should not be business day"
        );
        assert!(
            !cal.is_business_day(sunday),
            "Sunday should not be business day"
        );
        assert!(
            cal.is_business_day(monday),
            "Monday should be business day (no holidays)"
        );
    }

    #[test]
    fn test_add_joint_business_days_zero_days() {
        // Edge case: adding 0 days should return the start date
        let start = create_date(2024, Month::January, 15).unwrap();
        let result = add_joint_business_days(start, 0, None, None, None).expect("Should succeed");

        assert_eq!(result, start, "Adding 0 days should return start date");
    }

    #[test]
    fn test_adjust_joint_calendar_unknown_base() {
        // Test that adjust_joint_calendar errors on unknown base calendar
        let date = create_date(2024, Month::January, 15).unwrap();
        let result = adjust_joint_calendar(
            date,
            BusinessDayConvention::Following,
            Some("unknown_base"),
            None,
        );

        assert!(result.is_err(), "Unknown base calendar should error");
    }

    #[test]
    fn test_adjust_joint_calendar_unknown_quote() {
        // Test that adjust_joint_calendar errors on unknown quote calendar
        let date = create_date(2024, Month::January, 15).unwrap();
        let result = adjust_joint_calendar(
            date,
            BusinessDayConvention::Following,
            None,
            Some("unknown_quote"),
        );

        assert!(result.is_err(), "Unknown quote calendar should error");
    }

    #[test]
    fn test_add_joint_business_days_unknown_calendar() {
        let trade_date = create_date(2024, Month::January, 15).unwrap();
        let result = add_joint_business_days(trade_date, 2, Some("unknown_cal"), None, None);

        assert!(result.is_err(), "Unknown calendar should error");
    }

    #[test]
    fn test_adjust_joint_calendar_uses_union_calendar_modified_following() {
        let pair = ResolvedCalendarPair {
            base: &JAN_29_HOLIDAY,
            quote: &JAN_30_AND_31_HOLIDAYS,
        };

        let date = create_date(2025, Month::January, 29).unwrap();
        let adjusted = pair
            .adjust_joint_calendar(date, BusinessDayConvention::ModifiedFollowing)
            .expect("Union calendar adjustment should succeed");

        let expected = create_date(2025, Month::January, 28).unwrap();
        assert_eq!(
            adjusted, expected,
            "ModifiedFollowing must be evaluated on the joint calendar, not sequentially"
        );
    }

    #[test]
    fn fx_spot_date_usd_pair_intermediate_us_holiday_does_not_delay() {
        // Review finding : EUR/USD traded Thu
        // 2025-07-03; Fri 2025-07-04 is a US holiday but a good EUR day, so it
        // counts as an intermediate day. Market spot is Mon 2025-07-07.
        let trade = create_date(2025, Month::July, 3).unwrap();

        let spot = fx_spot_date(trade, 2, Some("target2"), Some("usny"), Some("usny"))
            .expect("fx spot roll");
        assert_eq!(
            spot,
            create_date(2025, Month::July, 7).unwrap(),
            "US holiday at T+1 must not delay EUR/USD spot"
        );

        // The symmetric joint rule (pre-fix behavior) lands one day later.
        let symmetric = add_joint_business_days(trade, 2, Some("target2"), Some("usny"), None)
            .expect("symmetric roll");
        assert_eq!(symmetric, create_date(2025, Month::July, 8).unwrap());
    }

    #[test]
    fn fx_spot_date_final_date_us_holiday_rolls_forward() {
        // EUR/USD traded Wed 2025-07-02: T+2 candidate is Fri 2025-07-04 (good
        // EUR day) but the value date itself is a US holiday, so spot must roll
        // to Mon 2025-07-07.
        let trade = create_date(2025, Month::July, 2).unwrap();
        let spot = fx_spot_date(trade, 2, Some("target2"), Some("usny"), Some("usny"))
            .expect("fx spot roll");
        assert_eq!(
            spot,
            create_date(2025, Month::July, 7).unwrap(),
            "a US holiday on the final value date must roll spot forward"
        );
    }

    #[test]
    fn fx_spot_date_cross_pair_usd_gates_final_date_only() {
        // EUR/JPY traded Thu 2025-07-03: Fri 2025-07-04 (US holiday) is good in
        // EUR and JPY so it counts as an intermediate day; the final value date
        // Mon 2025-07-07 is good in all three calendars.
        let trade = create_date(2025, Month::July, 3).unwrap();
        let spot = fx_spot_date(trade, 2, Some("target2"), Some("jpto"), Some("usny"))
            .expect("fx spot roll");
        assert_eq!(
            spot,
            create_date(2025, Month::July, 7).unwrap(),
            "USD must not gate intermediate days for a non-USD cross"
        );
    }

    #[test]
    fn fx_spot_date_non_usd_pair_without_usd_matches_symmetric_rule() {
        // Without a USD calendar, fx_spot_date reproduces the two-calendar
        // joint rule exactly (EUR/GBP over the 2024 UK Christmas break).
        let trade = create_date(2024, Month::December, 23).unwrap();
        let spot =
            fx_spot_date(trade, 2, Some("target2"), Some("gblo"), None).expect("fx spot roll");
        let symmetric = add_joint_business_days(trade, 2, Some("target2"), Some("gblo"), None)
            .expect("symmetric roll");
        assert_eq!(spot, symmetric);
    }

    #[test]
    fn fx_spot_date_t_plus_zero_returns_trade_date() {
        // Documented T+0 behavior: return the trade date unchanged.
        let trade = create_date(2025, Month::July, 4).unwrap();
        let spot =
            fx_spot_date(trade, 0, Some("target2"), Some("usny"), Some("usny")).expect("T+0");
        assert_eq!(spot, trade);
    }

    #[test]
    fn symmetric_settlement_calendar_honours_usd_holiday_on_cross() {
        // EUR/JPY traded Wed 2025-07-02. The two-currency T+2 lands on Fri
        // 2025-07-04 (good in EUR and JPY). US Independence Day closes USD on
        // that date, so the cross's spot value date must roll to Mon 2025-07-07.
        let trade = create_date(2025, Month::July, 2).unwrap();

        let spot_two_cal = add_joint_business_days(trade, 2, Some("target2"), Some("jpto"), None)
            .expect("two-calendar roll");
        assert_eq!(
            spot_two_cal,
            create_date(2025, Month::July, 4).unwrap(),
            "without USD settlement, spot lands on the US holiday"
        );

        let spot_with_usd =
            add_joint_business_days(trade, 2, Some("target2"), Some("jpto"), Some("usny"))
                .expect("three-calendar roll");
        assert_eq!(
            spot_with_usd,
            create_date(2025, Month::July, 7).unwrap(),
            "USD holiday must shift the cross spot date to the next all-good day"
        );
    }
}

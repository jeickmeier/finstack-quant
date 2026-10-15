//! FX date utilities for spot settlement rolls and standard pair settlement lags.
//!
//! [`fx_spot_date`] counts each currency's settlement lag separately on that
//! currency's own calendar (CAD and TRY settle one day sooner than other
//! currencies), enforces the pair's lag as a weekday minimum, then moves the
//! later date forward until every final-date calendar is open. USD holidays do
//! not delay the USD leg's intermediate days for ordinary pairs, while USD/MXN
//! and South American pairs count USD business days. The USD calendar must also
//! be open on the final date of every USD pair and non-USD cross. T+0 returns
//! the trade date unchanged; signed business-day arithmetic remains in
//! [`DateExt::add_business_days`]. Symmetric joint-calendar counting lives in
//! [`crate::dates::add_joint_business_days`].
//!
//! # Typical usage
//!
//! - [`fx_spot_date`] for market-convention (USD-aware) spot rolling
//! - [`fx_standard_settlement_days`] for the pair-aware T+1 / T+2 spot lag
//! - [`crate::dates::adjust_joint_calendar`] for applying a business-day convention on a joint calendar
//! - [`crate::dates::add_joint_business_days`] for T+n style symmetric joint counting
//!
//! # References
//!
//! - Business-day conventions: `docs/REFERENCES.md#isda-2006-definitions`
//! - FX spot-date conventions (T+1 pairs, per-currency counting, USD
//!   intermediate-day and Latin American rules): "Foreign exchange date
//!   conventions", Wikipedia,
//!   <https://en.wikipedia.org/wiki/Foreign_exchange_date_conventions>, and
//!   the EBS value-date calendar (CME Group),
//!   <https://www.cmegroup.com/content/dam/cmegroup/documents/ebs_value-date-calendar.pdf>.

use crate::currency::Currency;
use crate::dates::calendar::business_days::add_business_days_on;
use crate::dates::{
    adjust, calendar_by_id_strict, resolve_calendar, BusinessDayConvention, CompositeCalendar,
    Date, DateExt, HolidayCalendar,
};
use crate::{Error, Result};

/// Standard FX spot lag in business days for a currency pair.
///
/// Returns `1` for USD/CAD, USD/TRY, USD/PHP and USD/RUB, the T+1 spot pairs
/// of the interbank convention, and `2` for every other pair, including
/// USD/MXN, EUR/USD and crosses such as EUR/CAD. Offshore-only T+1 markets
/// (for example offshore USD/KZT) are not inferred; pass an explicit lag.
///
/// # Arguments
///
/// * `base` - Base currency of the pair (the currency being priced).
/// * `quote` - Quote currency of the pair (the pricing currency).
///
/// # Returns
///
/// Spot settlement lag in business days (`1` or `2`).
pub fn fx_standard_settlement_days(base: Currency, quote: Currency) -> u32 {
    let ((Currency::USD, other) | (other, Currency::USD)) = (base, quote) else {
        return 2;
    };
    match other {
        Currency::CAD | Currency::TRY | Currency::PHP | Currency::RUB => 1,
        _ => 2,
    }
}

/// Earliest settlement lag of one currency leg under the market standard.
///
/// CAD and TRY settle one business day after trade; every other currency
/// settles two business days after trade (EBS value-date convention). The
/// pair's own lag still applies as a minimum, so CAD/TRY settles T+2.
fn currency_settlement_days(currency: Currency) -> u32 {
    match currency {
        Currency::CAD | Currency::TRY => 1,
        _ => 2,
    }
}

/// Currencies whose USD leg counts USD business days (not weekdays) during
/// the spot clock: two clear US working days are required.
const USD_BUSINESS_DAY_COUNT_CURRENCIES: [Currency; 7] = [
    Currency::MXN,
    Currency::ARS,
    Currency::BRL,
    Currency::CLP,
    Currency::COP,
    Currency::PEN,
    Currency::UYU,
];

/// Roll a trade date to spot under the pair's contractual spot lag.
///
/// Each currency leg counts business days on its own calendar: its market lag
/// (CAD and TRY one day, other currencies two) capped at the pair lag, plus any
/// days by which `settlement_days` exceeds the pair standard. The USD leg of an
/// ordinary pair counts weekdays, because a US holiday is a good intermediate
/// day; for USD/MXN and the South American currencies (ARS, BRL, CLP, COP, PEN,
/// UYU) the USD leg counts US business days, also when that currency trades
/// against a non-USD currency. `settlement_days` weekdays is the pair minimum.
/// The later date then rolls forward until every final-date calendar (both
/// currencies and USD) is open. `settlement_days == 0` returns the trade date
/// unchanged.
///
/// `settlement_days` is the contractual lag: callers pass
/// [`fx_standard_settlement_days`] for market spot, or a shorter or longer lag
/// (tom-next T+1, forward-start T+3) that is honored as given.
///
/// # Arguments
///
/// * `trade_date` - The trade execution date from which settlement is calculated.
/// * `base_currency` - Base currency; its calendar counts the first leg.
/// * `quote_currency` - Quote currency; its calendar counts the second leg.
/// * `settlement_days` - Contractual spot lag in business days, from 0 through 31.
/// * `base_cal_id` - Optional base-currency calendar; `None` uses weekends only.
/// * `quote_cal_id` - Optional quote-currency calendar; `None` uses weekends only.
/// * `usd_cal_id` - USD settlement calendar. Required for non-USD crosses; for a
///   USD pair `None` falls back to the USD leg's own calendar.
///
/// # Errors
///
/// Returns an error if a required calendar is absent or unknown, the lag is
/// above 31 days, a date leaves the supported range, or no required business
/// day is found within 100 calendar days.
///
/// # Examples
///
/// ```
/// # use finstack_quant_core::dates::create_date;
/// # use time::Month;
/// # use finstack_quant_core::dates::fx::fx_spot_date;
/// # use finstack_quant_core::currency::Currency;
/// // EUR/USD traded Thu 2025-07-03. Fri 2025-07-04 is a US holiday but a good
/// // intermediate day under the EUR/USD convention; spot is Mon 2025-07-07.
/// let trade = create_date(2025, Month::July, 3).unwrap();
/// let spot = fx_spot_date(trade, Currency::EUR, Currency::USD, 2,
///     Some("target2"), Some("usny"), Some("usny")).unwrap();
/// assert_eq!(spot, create_date(2025, Month::July, 7).unwrap());
///
/// // A contractual T+1 (tom) EUR/USD trade settles the next joint business day.
/// let tom = fx_spot_date(create_date(2025, Month::January, 15).unwrap(), Currency::EUR,
///     Currency::USD, 1, Some("target2"), Some("usny"), Some("usny")).unwrap();
/// assert_eq!(tom, create_date(2025, Month::January, 16).unwrap());
/// ```
pub fn fx_spot_date(
    trade_date: Date,
    base_currency: Currency,
    quote_currency: Currency,
    settlement_days: u32,
    base_cal_id: Option<&str>,
    quote_cal_id: Option<&str>,
    usd_cal_id: Option<&str>,
) -> Result<Date> {
    let base_cal = resolve_calendar(base_cal_id)?;
    let quote_cal = resolve_calendar(quote_cal_id)?;
    if settlement_days == 0 {
        return Ok(trade_date);
    }
    let usd_cal = match usd_cal_id {
        Some(id) => Some(calendar_by_id_strict(id)?),
        None if base_currency == Currency::USD => Some(base_cal),
        None if quote_currency == Currency::USD => Some(quote_cal),
        None => None,
    };
    let Some(usd_cal) = usd_cal else {
        return Err(Error::Validation(
            "FX spot settlement for a non-USD cross requires a USD calendar".into(),
        ));
    };
    const MAX_FX_SPOT_LAG_DAYS: u32 = 31;
    if settlement_days > MAX_FX_SPOT_LAG_DAYS {
        return Err(Error::Validation(format!(
            "FX spot lag {settlement_days} exceeds the supported range 0..={MAX_FX_SPOT_LAG_DAYS}"
        )));
    }

    // Days beyond the pair standard extend every leg; a shorter contractual
    // lag caps every leg.
    let standard = fx_standard_settlement_days(base_currency, quote_currency);
    let capped = settlement_days.min(standard);
    let extension = settlement_days.saturating_sub(standard);
    let leg_lag = |currency: Currency| currency_settlement_days(currency).min(capped) + extension;
    let usd_counts_business_days = USD_BUSINESS_DAY_COUNT_CURRENCIES
        .iter()
        .any(|currency| *currency == base_currency || *currency == quote_currency);
    let leg = |currency: Currency, calendar: &dyn HolidayCalendar| -> Result<Date> {
        let lag = leg_lag(currency);
        if currency == Currency::USD {
            // US holidays are good intermediate days unless the pair needs
            // clear US business days.
            if usd_counts_business_days {
                add_business_days_on(trade_date, lag, usd_cal)
            } else {
                add_weekdays_on(trade_date, lag)
            }
        } else {
            add_business_days_on(trade_date, lag, calendar)
        }
    };
    let mut date = leg(base_currency, base_cal)?
        .max(leg(quote_currency, quote_cal)?)
        .max(add_weekdays_on(trade_date, settlement_days)?);
    let is_cross = base_currency != Currency::USD && quote_currency != Currency::USD;
    if is_cross && usd_counts_business_days {
        date = date.max(add_business_days_on(trade_date, settlement_days, usd_cal)?);
    }
    let final_calendars = [base_cal, quote_cal, usd_cal];
    adjust(
        date,
        BusinessDayConvention::Following,
        &CompositeCalendar::new(&final_calendars),
    )
}

fn add_weekdays_on(start: Date, n_days: u32) -> Result<Date> {
    let n_days = i32::try_from(n_days)
        .map_err(|_| Error::Validation("weekday offset exceeds the supported range".into()))?;
    start.add_weekdays(n_days)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::{
        add_joint_business_days, adjust_joint_calendar, create_date, WEEKENDS_ONLY,
    };
    use time::Month;

    fn spot(
        trade_date: Date,
        base_currency: Currency,
        quote_currency: Currency,
        settlement_days: u32,
        base_cal_id: Option<&str>,
        quote_cal_id: Option<&str>,
        usd_cal_id: Option<&str>,
    ) -> Result<Date> {
        fx_spot_date(
            trade_date,
            base_currency,
            quote_currency,
            settlement_days,
            base_cal_id,
            quote_cal_id,
            usd_cal_id,
        )
    }

    #[test]
    fn fx_date_range_boundaries_return_errors() {
        assert!(add_joint_business_days(Date::MAX, 1, None, None, None).is_err());
        assert!(spot(Date::MAX, Currency::EUR, Currency::USD, 1, None, None, None).is_err());
        assert_eq!(
            add_joint_business_days(Date::MAX, 0, None, None, None).unwrap(),
            Date::MAX
        );
        assert_eq!(
            spot(Date::MIN, Currency::EUR, Currency::USD, 0, None, None, None).unwrap(),
            Date::MIN
        );
    }

    #[test]
    fn fx_standard_settlement_days_t1_pairs_and_default() {
        assert_eq!(fx_standard_settlement_days(Currency::USD, Currency::CAD), 1);
        assert_eq!(fx_standard_settlement_days(Currency::CAD, Currency::USD), 1);
        assert_eq!(fx_standard_settlement_days(Currency::USD, Currency::TRY), 1);
        assert_eq!(fx_standard_settlement_days(Currency::TRY, Currency::USD), 1);
        assert_eq!(fx_standard_settlement_days(Currency::USD, Currency::MXN), 2);
        assert_eq!(fx_standard_settlement_days(Currency::EUR, Currency::USD), 2);
    }

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
    fn fx_spot_date_usd_pair_intermediate_us_holiday_does_not_delay() {
        // Review finding : EUR/USD traded Thu
        // 2025-07-03; Fri 2025-07-04 is a US holiday but a good EUR day, so it
        // counts as an intermediate day. Market spot is Mon 2025-07-07.
        let trade = create_date(2025, Month::July, 3).unwrap();

        let spot = spot(
            trade,
            Currency::EUR,
            Currency::USD,
            2,
            Some("target2"),
            Some("usny"),
            Some("usny"),
        )
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
        let spot = spot(
            trade,
            Currency::EUR,
            Currency::USD,
            2,
            Some("target2"),
            Some("usny"),
            Some("usny"),
        )
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
        let spot = spot(
            trade,
            Currency::EUR,
            Currency::JPY,
            2,
            Some("target2"),
            Some("jpto"),
            Some("usny"),
        )
        .expect("fx spot roll");
        assert_eq!(
            spot,
            create_date(2025, Month::July, 7).unwrap(),
            "USD must not gate intermediate days for a non-USD cross"
        );
    }

    #[test]
    fn fx_spot_date_non_usd_cross_requires_usd_calendar() {
        let trade = create_date(2024, Month::December, 23).unwrap();
        let error = spot(
            trade,
            Currency::EUR,
            Currency::GBP,
            2,
            Some("target2"),
            Some("gblo"),
            None,
        )
        .expect_err("cross spot requires USD final-date calendar");
        assert!(error.to_string().contains("requires a USD calendar"));
    }

    #[test]
    fn fx_spot_date_t_plus_zero_returns_trade_date() {
        // Documented T+0 behavior: return the trade date unchanged.
        let trade = create_date(2025, Month::July, 4).unwrap();
        let spot = spot(
            trade,
            Currency::EUR,
            Currency::USD,
            0,
            Some("target2"),
            Some("usny"),
            Some("usny"),
        )
        .expect("T+0");
        assert_eq!(spot, trade);
    }
    #[test]
    fn signed_business_day_arithmetic_is_separate_from_spot_settlement() {
        let trade = create_date(2025, Month::January, 15).unwrap();
        let prior = trade
            .add_business_days(-1, &WEEKENDS_ONLY)
            .expect("signed business-day arithmetic");
        assert_eq!(prior, create_date(2025, Month::January, 14).unwrap());
    }

    #[test]
    fn fx_spot_date_rejects_unbounded_lags_before_iteration() {
        let trade = create_date(2025, Month::January, 15).unwrap();
        let error = spot(
            trade,
            Currency::EUR,
            Currency::USD,
            u32::MAX,
            Some("target2"),
            Some("usny"),
            Some("usny"),
        )
        .expect_err("extreme lag must be rejected");
        assert!(error.to_string().contains("supported range"));
    }

    #[test]
    fn fx_spot_date_uses_individual_currency_lags_and_pair_holiday_rules() {
        let cad_chf_trade = create_date(2018, Month::August, 3).unwrap();
        let cad_chf = spot(
            cad_chf_trade,
            Currency::CAD,
            Currency::CHF,
            2,
            Some("cato"),
            Some("chzh"),
            Some("usny"),
        )
        .expect("CAD/CHF spot date");
        assert_eq!(cad_chf, create_date(2018, Month::August, 7).unwrap());

        let south_america_trade = create_date(2025, Month::July, 3).unwrap();
        let usd_brl = spot(
            south_america_trade,
            Currency::USD,
            Currency::BRL,
            2,
            Some("usny"),
            Some("brbd"),
            Some("usny"),
        )
        .expect("USD/BRL spot date");
        assert_eq!(usd_brl, create_date(2025, Month::July, 8).unwrap());

        let usd_mxn = spot(
            south_america_trade,
            Currency::USD,
            Currency::MXN,
            2,
            Some("usny"),
            None,
            Some("usny"),
        )
        .expect("USD/MXN spot date");
        assert_eq!(usd_mxn, create_date(2025, Month::July, 8).unwrap());
    }

    #[test]
    fn explicit_lag_below_standard_is_honored() {
        // EUR/USD tom (T+1) traded Wed 2025-01-15 settles Thu 2025-01-16.
        let tom = spot(
            create_date(2025, Month::January, 15).unwrap(),
            Currency::EUR,
            Currency::USD,
            1,
            Some("target2"),
            Some("usny"),
            Some("usny"),
        )
        .expect("EUR/USD tom");
        assert_eq!(tom, create_date(2025, Month::January, 16).unwrap());
    }

    #[test]
    fn lag_above_standard_counts_business_days_on_each_leg() {
        // EUR/USD T+3 traded Tue 2024-12-24: TARGET2 is closed on 25-26 Dec,
        // so the EUR leg counts 27, 30 and 31 Dec.
        let t3 = spot(
            create_date(2024, Month::December, 24).unwrap(),
            Currency::EUR,
            Currency::USD,
            3,
            Some("target2"),
            Some("usny"),
            Some("usny"),
        )
        .expect("EUR/USD T+3");
        assert_eq!(t3, create_date(2024, Month::December, 31).unwrap());
    }

    #[test]
    fn latin_american_usd_leg_counts_on_the_usd_calendar() {
        // USD/MXN with only the USD settlement calendar supplied: Fri
        // 2025-07-04 is a US holiday, so two clear US business days end Tue
        // 2025-07-08.
        let usd_mxn = spot(
            create_date(2025, Month::July, 3).unwrap(),
            Currency::USD,
            Currency::MXN,
            2,
            None,
            None,
            Some("usny"),
        )
        .expect("USD/MXN spot");
        assert_eq!(usd_mxn, create_date(2025, Month::July, 8).unwrap());
    }

    #[test]
    fn usd_php_and_usd_rub_are_t_plus_one_pairs() {
        for other in [Currency::CAD, Currency::TRY, Currency::PHP, Currency::RUB] {
            assert_eq!(fx_standard_settlement_days(Currency::USD, other), 1);
            assert_eq!(fx_standard_settlement_days(other, Currency::USD), 1);
        }
        assert_eq!(fx_standard_settlement_days(Currency::USD, Currency::MXN), 2);
        assert_eq!(fx_standard_settlement_days(Currency::EUR, Currency::CAD), 2);
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

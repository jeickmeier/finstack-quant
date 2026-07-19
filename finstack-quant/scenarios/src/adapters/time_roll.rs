//! Time roll-forward adapter with carry/theta calculations.
//!
//! Implements the `OperationSpec::TimeRollForward` variant by advancing the
//! valuation date, recomputing time-dependent instrument metrics, and returning
//! a structured report of the resulting P&L decomposition.

use crate::engine::ExecutionContext;
use crate::error::Result;
use crate::utils::parse_period_to_days;
use crate::TimeRollMode;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, HolidayCalendar, Tenor, WEEKENDS_ONLY};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use indexmap::IndexMap;

/// Report from time roll-forward operation.
///
/// # Examples
/// ```rust
/// use finstack_quant_scenarios::RollForwardReport;
/// use indexmap::IndexMap;
/// use time::macros::date;
///
/// let report = RollForwardReport {
///     old_date: date!(2025 - 01 - 01),
///     new_date: date!(2025 - 02 - 01),
///     days: 31,
///     instrument_carry: vec![],
///     total_carry: IndexMap::new(),
///     failed_instruments: vec![],
/// };
/// assert_eq!(report.days, 31);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RollForwardReport {
    /// Original as-of date.
    pub old_date: finstack_quant_core::dates::Date,

    /// New as-of date after roll.
    pub new_date: finstack_quant_core::dates::Date,

    /// Calendar days between `old_date` and `new_date`.
    ///
    /// This is always a calendar-day span, in every [`TimeRollMode`] — including
    /// [`TimeRollMode::BusinessDays`], where the *target date* is business-day
    /// adjusted but the span back to `old_date` is still counted in calendar
    /// days. Downstream ACT/365F annualization depends on this.
    pub days: i64,

    /// Per-instrument carry accrual (if instruments provided), grouped by currency.
    pub instrument_carry: Vec<(String, IndexMap<Currency, Money>)>,

    /// Total P&L from carry, grouped by currency.
    pub total_carry: IndexMap<Currency, Money>,
    /// Instruments whose carry calculation failed but did not abort the roll.
    pub failed_instruments: Vec<(String, String)>,
}

/// Apply a time roll-forward operation.
///
/// The function advances the valuation date by the requested period and computes
/// theta/carry for each instrument (if a portfolio is supplied). Theta is defined
/// as the PV change resulting purely from the passage of time while holding
/// market data constant.
///
/// # Arguments
/// - `ctx`: Execution context providing the mutable valuation date, market data,
///   and optional instruments.
/// - `period_str`: Period string such as `"1D"`, `"1W"`, `"1M"`, or `"1Y"`.
/// - `mode`: Roll interpretation (business-day aware vs approximate days).
///
/// # Returns
/// [`RollForwardReport`] summarising the new date and P&L breakdown.
///
/// # Errors
/// - [`Error::InvalidPeriod`](crate::error::Error::InvalidPeriod) if the period
///   string cannot be parsed.
/// - Propagates any errors encountered while revaluing instruments.
///
/// # References
///
/// - Day-count and business-day conventions: `docs/REFERENCES.md#isda-2006-definitions`
/// - Period notation: `docs/REFERENCES.md#iso-8601`
///
/// # Examples
/// ```ignore
/// use finstack_quant_scenarios::ExecutionContext;
/// use finstack_quant_scenarios::apply_time_roll_forward;
/// use finstack_quant_scenarios::TimeRollMode;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_statements::FinancialModelSpec;
/// use time::macros::date;
///
/// # fn main() -> finstack_quant_scenarios::Result<()> {
/// let mut market = MarketContext::new();
/// let mut model = FinancialModelSpec::new("demo", vec![]);
/// let as_of = date!(2025 - 01 - 01);
/// let mut ctx = ExecutionContext {
///     market: &mut market,
///     model: Some(&mut model),
///     instruments: None,
///     rate_bindings: None,
///     calendar: None,
///     as_of,
/// };
/// // 2025-01-01 + 1M is Saturday 2025-02-01, so ModifiedFollowing carries the
/// // target to Monday 2025-02-03 -> 33 calendar days.
/// let report = apply_time_roll_forward(&mut ctx, "1M", TimeRollMode::BusinessDays)?;
/// assert_eq!(report.new_date, date!(2025 - 02 - 03));
/// assert_eq!(report.days, 33);
/// # Ok(())
/// # }
/// ```
pub fn apply_time_roll_forward(
    ctx: &mut ExecutionContext,
    period_str: &str,
    mode: TimeRollMode,
) -> Result<RollForwardReport> {
    use crate::error::Error;

    let old_date = ctx.as_of;
    let (new_date, day_shift) = match mode {
        TimeRollMode::Approximate => {
            let days = parse_period_to_days(period_str)?;
            let new_date = old_date + time::Duration::days(days);
            (new_date, days)
        }
        TimeRollMode::CalendarDays => {
            let tenor =
                Tenor::parse(period_str).map_err(|e| Error::InvalidPeriod(e.to_string()))?;
            let target = tenor.add_to_date(old_date, None, BusinessDayConvention::Unadjusted)?;
            let days = (target - old_date).whole_days();
            (target, days)
        }
        TimeRollMode::BusinessDays => {
            let tenor =
                Tenor::parse(period_str).map_err(|e| Error::InvalidPeriod(e.to_string()))?;
            // `Tenor::add_to_date` discards the business-day convention entirely
            // when no calendar is supplied, which would silently degrade this
            // mode into an exact copy of `CalendarDays` and land horizons on
            // weekends. Fall back to core's `WEEKENDS_ONLY`, the documented
            // calendar for APIs whose calendar identifier is optional, so the
            // mode always performs a real adjustment. Callers wanting holiday
            // awareness supply `ctx.calendar`.
            let calendar: &dyn HolidayCalendar = ctx.calendar.unwrap_or(&WEEKENDS_ONLY);
            let target = tenor.add_to_date(
                old_date,
                Some(calendar),
                BusinessDayConvention::ModifiedFollowing,
            )?;
            let days = (target - old_date).whole_days();
            (target, days)
        }
    };

    // Guard against backward rolls. Market data roll-forward, carry accrual,
    // and horizon attribution are all defined for forward time; a negative
    // period (e.g. "-1M") would silently corrupt the pipeline. Reject here
    // so the error is produced at the source of the bad period string.
    if day_shift < 0 {
        return Err(Error::InvalidPeriod(format!(
            "TimeRollForward period '{period_str}' produced a backward shift ({day_shift} days); \
             only forward rolls are supported"
        )));
    }

    // Calculate carry and market value changes for instruments BEFORE rolling curves
    // This ensures we capture the true carry (time value change with constant curves)
    let (instrument_carry, total_carry, failed_instruments) =
        if let Some(instruments) = ctx.instruments.as_ref() {
            calculate_instrument_pnl(instruments, ctx.market, old_date, new_date)?
        } else {
            (Vec::new(), IndexMap::new(), Vec::new())
        };

    // Roll all curves forward (adjusts base dates, shifts knots, filters expired).
    // Realized-forward semantics : every curve
    // realizes its forwards as the base date advances (discount curves
    // renormalize by DF(dt), hazard curves preserve hazard rates, forward
    // curves preserve forwards, inflation rebases CPI, price/vol-index curves
    // set spot to the old forward). Vol surfaces, FX spot, and fixings stay
    // static.
    let rolled_market = ctx.market.roll_forward(day_shift)?;

    // Replace market context with rolled version
    *ctx.market = rolled_market;

    // Update as_of in context
    ctx.as_of = new_date;

    Ok(RollForwardReport {
        old_date,
        new_date,
        days: day_shift,
        instrument_carry,
        total_carry,
        failed_instruments,
    })
}

/// Return type of [`calculate_instrument_pnl`]: `(per-instrument carry,
/// total carry, failed instruments with reason)`.
type InstrumentPnlResult = (
    Vec<(String, IndexMap<Currency, Money>)>,
    IndexMap<Currency, Money>,
    Vec<(String, String)>,
);

/// Calculate P&L breakdown for instruments.
///
/// Theta (carry) is calculated as:
///   Carry = PV(end_date) - PV(start_date) + Sum(Cashflows from start to end)
///
/// This accounts for:
/// - Pull-to-par effects (PV change)
/// - Coupon/interest net cashflows during the period
/// - Principal payments during the period
///
/// This is consistent with the theta metric definition in valuations.
///
/// # Failure handling
///
/// If either the start-date or end-date valuation returns an error, the
/// instrument is recorded in the `failed_instruments` return slot with the
/// underlying error message and is *excluded* from `instrument_carry` /
/// `total_carry`. This prevents partial cashflow-only carry lines from
/// contaminating the aggregate while still surfacing the failure in the
/// `RollForwardReport`.
///
/// # Cashflow window convention
///
/// Cashflows are included when their payment date satisfies
/// `start_date < date <= end_date` (i.e. T+0 excluded, T+N included). A coupon
/// paid on the roll-forward target date counts toward carry; a coupon paid on
/// the starting valuation date does not.
fn calculate_instrument_pnl(
    instruments: &[Box<dyn Instrument>],
    market: &finstack_quant_core::market_data::context::MarketContext,
    old_date: finstack_quant_core::dates::Date,
    new_date: finstack_quant_core::dates::Date,
) -> Result<InstrumentPnlResult> {
    let mut instrument_carry: Vec<(String, IndexMap<Currency, Money>)> = Vec::new();
    let mut total_carry: IndexMap<Currency, Money> = IndexMap::new();
    let mut failed_instruments = Vec::new();

    for instrument in instruments {
        let inst_id = instrument.id().to_string();

        // Valuation at both ends is required to compute carry. Swallowing errors
        // here and falling through to cashflow-only accumulation would produce a
        // misleading "pure coupon" carry number with no failure indication.
        let pv_old = match instrument.value(market, old_date) {
            Ok(v) => v,
            Err(err) => {
                failed_instruments.push((inst_id.clone(), format!("t0 valuation failed: {err}")));
                continue;
            }
        };
        let pv_new = match instrument.value(market, new_date) {
            Ok(v) => v,
            Err(err) => {
                failed_instruments.push((inst_id.clone(), format!("t1 valuation failed: {err}")));
                continue;
            }
        };

        let mut pv_change_by_ccy: IndexMap<Currency, Money> = IndexMap::new();
        match pv_new.checked_sub(pv_old) {
            Ok(diff) => {
                pv_change_by_ccy.insert(diff.currency(), diff);
            }
            Err(err) => {
                failed_instruments.push((inst_id.clone(), format!("pv diff failed: {err}")));
                continue;
            }
        }

        let cashflows_during_period =
            collect_instrument_cashflows(instrument.as_ref(), market, old_date, new_date);

        let mut carry_by_ccy = pv_change_by_ccy;
        for (ccy, flow) in cashflows_during_period {
            carry_by_ccy
                .entry(ccy)
                .and_modify(|m| *m += flow)
                .or_insert(flow);
        }

        for (ccy, amount) in &carry_by_ccy {
            total_carry
                .entry(*ccy)
                .and_modify(|m| *m += *amount)
                .or_insert(*amount);
        }

        instrument_carry.push((inst_id.clone(), carry_by_ccy));
    }

    Ok((instrument_carry, total_carry, failed_instruments))
}

/// Collect cashflows for an instrument during a period, grouped by currency.
///
/// The cashflow window is half-open: `(start_date, end_date]`. Cashflows
/// exactly at `start_date` are excluded (they are assumed to have been
/// captured by the previous roll's "end_date" or to have already been paid
/// at `t = 0`) while cashflows exactly at `end_date` are included. This
/// keeps successive [`apply_time_roll_forward`] calls conservative under
/// concatenation and avoids double-counting coupons landing on roll
/// boundaries.
fn collect_instrument_cashflows(
    instrument: &dyn Instrument,
    market: &finstack_quant_core::market_data::context::MarketContext,
    start_date: finstack_quant_core::dates::Date,
    end_date: finstack_quant_core::dates::Date,
) -> IndexMap<Currency, Money> {
    let mut result: IndexMap<Currency, Money> = IndexMap::new();

    if let Ok(flows) = instrument.dated_cashflows(market, start_date) {
        for (date, money) in flows.into_iter() {
            if date > start_date && date <= end_date {
                let ccy = money.currency();
                result
                    .entry(ccy)
                    .and_modify(|m| *m += money)
                    .or_insert(money);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ExecutionContext;
    use crate::TimeRollMode;
    use finstack_quant_core::dates::{Date, DateExt, DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_statements::FinancialModelSpec;
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
    use finstack_quant_valuations::instruments::{Attributes, Bond, Instrument, PricingOptions};
    use finstack_quant_valuations::metrics::MetricId;
    use time::macros::date;
    use time::Month;

    #[test]
    fn roll_forward_report_keeps_only_live_fields() {
        let report = RollForwardReport {
            old_date: date!(2025 - 01 - 01),
            new_date: date!(2025 - 02 - 01),
            days: 31,
            instrument_carry: Vec::new(),
            total_carry: IndexMap::new(),
            failed_instruments: Vec::new(),
        };

        assert_eq!(report.days, 31);
    }

    #[test]
    fn apply_time_roll_forward_reports_bond_carry() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid base date");
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
            .build()
            .expect("valid discount curve");

        let mut market = MarketContext::new().insert(curve);
        let mut model = FinancialModelSpec::new("test", vec![]);
        let mut instruments: Vec<Box<dyn Instrument>> = vec![Box::new(
            Bond::builder()
                .id("BOND1".into())
                .notional(Money::new(100.0, Currency::USD))
                .issue_date(base_date)
                .maturity(base_date + time::Duration::days(730))
                .cashflow_spec(
                    CashflowSpec::fixed(0.05, Tenor::annual(), DayCount::Thirty360)
                        .expect("finite test coupon"),
                )
                .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
                .credit_curve_id_opt(None)
                .instrument_pricing_overrides(InstrumentPricingOverrides::default())
                .attributes(Attributes::new())
                .build()
                .expect("valid bond"),
        )];

        let pv_base = instruments
            .first()
            .expect("bond instrument")
            .as_ref()
            .value(&market, base_date)
            .expect("pv at base as_of before roll")
            .amount();

        // 2025-01-01 + 1M lands on Saturday 2025-02-01; ModifiedFollowing under
        // the weekends-only fallback carries it to Monday 2025-02-03.
        let expected_date = base_date + time::Duration::days(33);
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: Some(&mut instruments),
            rate_bindings: None,
            calendar: None,
            as_of: base_date,
        };

        let report = apply_time_roll_forward(&mut ctx, "1M", TimeRollMode::BusinessDays)
            .expect("time roll succeeds");
        assert_eq!(ctx.as_of, expected_date);
        assert_eq!(report.new_date, expected_date);
        assert_eq!(report.days, 33);
        assert!(
            !report.instrument_carry.is_empty(),
            "expected instrument carry to be populated"
        );

        let bond_carry = report
            .instrument_carry
            .iter()
            .find(|(id, _)| id == "BOND1")
            .expect("BOND1 should have carry entry");
        assert!(
            bond_carry.1.get(&Currency::USD).is_some(),
            "bond carry should have USD amount"
        );
        assert!(
            report.total_carry.get(&Currency::USD).is_some(),
            "total carry should have USD entry"
        );

        let rolled = instruments
            .first()
            .expect("bond instrument")
            .as_ref()
            .price_with_metrics(
                &market,
                report.new_date,
                &[MetricId::Theta],
                PricingOptions::default(),
            )
            .expect("metrics at rolled as_of");
        assert!(rolled.value.amount().is_finite());
        assert_ne!(rolled.value.amount(), pv_base);
        let theta = *rolled
            .measures
            .get("theta")
            .expect("theta at rolled horizon");
        assert!(theta.is_finite());
    }

    /// Roll a bare context and return `(new_date, days)` for the given mode.
    fn roll_dates(base_date: Date, period: &str, mode: TimeRollMode) -> (Date, i64) {
        let mut market = MarketContext::new();
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: base_date,
        };
        let report = apply_time_roll_forward(&mut ctx, period, mode).expect("time roll succeeds");
        (report.new_date, report.days)
    }

    /// `BusinessDays` must not silently degrade into `CalendarDays`.
    ///
    /// `Tenor::add_to_date` discards the business-day convention when no
    /// calendar is supplied, which previously made the two modes bit-identical
    /// and landed horizons on weekends. The weekends-only fallback keeps them
    /// distinct whenever the unadjusted target is not a business day.
    #[test]
    fn business_days_mode_differs_from_calendar_days_on_weekend_targets() {
        // Wed 2025-01-01 + 1M = Sat 2025-02-01.
        let base_date = date!(2025 - 01 - 01);

        let (cal_date, cal_days) = roll_dates(base_date, "1M", TimeRollMode::CalendarDays);
        assert_eq!(
            cal_date,
            date!(2025 - 02 - 01),
            "calendar mode is unadjusted"
        );
        assert_eq!(cal_days, 31);
        assert!(cal_date.is_weekend(), "test premise: target is a weekend");

        let (bus_date, bus_days) = roll_dates(base_date, "1M", TimeRollMode::BusinessDays);
        assert_eq!(
            bus_date,
            date!(2025 - 02 - 03),
            "ModifiedFollowing should carry Saturday to Monday"
        );
        assert_eq!(bus_days, 33);

        assert_ne!(
            cal_date, bus_date,
            "BusinessDays must not be an alias for CalendarDays"
        );
    }

    /// ModifiedFollowing must roll *back* rather than cross a month boundary.
    #[test]
    fn business_days_mode_honours_modified_following_month_end() {
        // Fri 2025-04-25 + 1W = Fri 2025-05-02 (a business day, unchanged).
        let (plain, _) = roll_dates(date!(2025 - 04 - 25), "1W", TimeRollMode::BusinessDays);
        assert_eq!(plain, date!(2025 - 05 - 02));

        // Mon 2025-03-31 + 2M = Sat 2025-05-31, the last day of May. Plain
        // Following would cross into June, so ModifiedFollowing must step back
        // to Friday 2025-05-30.
        let (month_end, days) = roll_dates(date!(2025 - 03 - 31), "2M", TimeRollMode::BusinessDays);
        assert_eq!(
            month_end,
            date!(2025 - 05 - 30),
            "ModifiedFollowing must not cross the month boundary"
        );
        assert_eq!(days, 60);
    }

    /// When the unadjusted target is already a business day, the two
    /// calendar-resolving modes must agree exactly.
    #[test]
    fn business_and_calendar_modes_agree_on_business_day_targets() {
        // Wed 2025-01-15 + 3M = Tue 2025-04-15, already a business day.
        let base_date = date!(2025 - 01 - 15);
        let calendar = roll_dates(base_date, "3M", TimeRollMode::CalendarDays);
        let business = roll_dates(base_date, "3M", TimeRollMode::BusinessDays);
        assert_eq!(calendar, business);
        assert_eq!(calendar.0, date!(2025 - 04 - 15));
    }

    /// Realized-forward roll semantics : after
    /// `TimeRollForward`, the rolled discount curve satisfies
    /// `DF_rolled(T - dt) = DF_old(T) / DF_old(dt)`, so the booked carry
    /// equals the PV move and the post-roll market state is consistent.
    #[test]
    fn apply_time_roll_forward_realizes_discount_forwards() {
        use finstack_quant_core::dates::DayCountContext;

        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid base date");
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (1.0, 0.98), (2.0, 0.955), (5.0, 0.90)])
            .build()
            .expect("valid discount curve");
        let old_curve = curve.clone();

        let mut market = MarketContext::new().insert(curve);
        let mut model = FinancialModelSpec::new("test", vec![]);
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: base_date,
        };

        let report = apply_time_roll_forward(&mut ctx, "1M", TimeRollMode::CalendarDays)
            .expect("time roll succeeds");

        let rolled_curve = market.get_discount("USD-OIS").expect("rolled curve");
        let dt = old_curve
            .day_count()
            .year_fraction(base_date, report.new_date, DayCountContext::default())
            .expect("year fraction");
        let df_dt = old_curve.df(dt);

        for t_old in [1.0_f64, 2.0, 5.0] {
            let expected = old_curve.df(t_old) / df_dt;
            let actual = rolled_curve.df(t_old - dt);
            assert!(
                (actual - expected).abs() < 1e-12,
                "DF_rolled({}) should equal DF_old({t_old})/DF_old(dt): {actual} vs {expected}",
                t_old - dt
            );
        }
    }
}

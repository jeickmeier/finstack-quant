//! Time roll-forward adapter with carry/theta calculations.
//!
//! Implements the `OperationSpec::TimeRollForward` variant by advancing the
//! valuation date, recomputing time-dependent instrument metrics, and returning
//! a structured report of the resulting P&L decomposition.

use crate::engine::ExecutionContext;
use crate::error::Result;
use crate::utils::parse_period_to_days;
use crate::TimeRollMode;
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, Tenor};
use finstack_core::money::Money;
use finstack_valuations::instruments::DynInstrument;
use indexmap::IndexMap;

/// Report from time roll-forward operation.
///
/// # Examples
/// ```rust
/// use finstack_scenarios::RollForwardReport;
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
    pub old_date: finstack_core::dates::Date,

    /// New as-of date after roll.
    pub new_date: finstack_core::dates::Date,

    /// Number of days rolled forward.
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
/// ```rust,no_run
/// use finstack_scenarios::ExecutionContext;
/// use finstack_scenarios::adapters::time_roll::apply_time_roll_forward;
/// use finstack_scenarios::TimeRollMode;
/// use finstack_core::market_data::context::MarketContext;
/// use finstack_statements::FinancialModelSpec;
/// use time::macros::date;
///
/// # fn main() -> finstack_scenarios::Result<()> {
/// let mut market = MarketContext::new();
/// let mut model = FinancialModelSpec::new("demo", vec![]);
/// let as_of = date!(2025 - 01 - 01);
/// let mut ctx = ExecutionContext {
///     market: &mut market,
///     model: &mut model,
///     instruments: None,
///     rate_bindings: None,
///     calendar: None,
///     as_of,
/// };
/// let report = apply_time_roll_forward(&mut ctx, "1M", TimeRollMode::BusinessDays)?;
/// assert_eq!(report.days, 31);
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
            let target = tenor.add_to_date(
                old_date,
                ctx.calendar,
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

    // Roll all curves forward (adjusts base dates, shifts knots, filters expired)
    // This is the "constant curves" scenario - rates at calendar dates stay the same,
    // but maturities are re-measured from the new base date
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
    instruments: &[Box<DynInstrument>],
    market: &finstack_core::market_data::context::MarketContext,
    old_date: finstack_core::dates::Date,
    new_date: finstack_core::dates::Date,
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
    instrument: &DynInstrument,
    market: &finstack_core::market_data::context::MarketContext,
    start_date: finstack_core::dates::Date,
    end_date: finstack_core::dates::Date,
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
    use finstack_core::dates::{Date, DayCount, Tenor};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::Money;
    use finstack_statements::FinancialModelSpec;
    use finstack_valuations::instruments::fixed_income::bond::CashflowSpec;
    use finstack_valuations::instruments::pricing_overrides::PricingOverrides;
    use finstack_valuations::instruments::{Attributes, Bond, DynInstrument, PricingOptions};
    use finstack_valuations::metrics::MetricId;
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
        let mut instruments: Vec<Box<DynInstrument>> = vec![Box::new(
            Bond::builder()
                .id("BOND1".into())
                .notional(Money::new(100.0, Currency::USD))
                .issue_date(base_date)
                .maturity(base_date + time::Duration::days(730))
                .cashflow_spec(
                    CashflowSpec::fixed(0.05, Tenor::annual(), DayCount::Thirty360)
                        .expect("finite test coupon"),
                )
                .discount_curve_id(finstack_core::types::CurveId::new("USD-OIS"))
                .credit_curve_id_opt(None)
                .pricing_overrides(PricingOverrides::default())
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

        let expected_date = base_date + time::Duration::days(31);
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: &mut model,
            instruments: Some(&mut instruments),
            rate_bindings: None,
            calendar: None,
            as_of: base_date,
        };

        let report = apply_time_roll_forward(&mut ctx, "1M", TimeRollMode::BusinessDays)
            .expect("time roll succeeds");
        assert_eq!(ctx.as_of, expected_date);
        assert_eq!(report.new_date, expected_date);
        assert_eq!(report.days, 31);
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
}

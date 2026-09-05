//! Time roll-forward adapter with carry/theta calculations.
//!
//! Implements the `OperationSpec::TimeRollForward` variant by advancing the
//! valuation date, recomputing time-dependent instrument metrics, and returning
//! a structured report of the resulting P&L decomposition.

use crate::engine::{ExecutionContext, RollForwardReport};
use crate::error::Result;
use crate::TimeRollMode;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, HolidayCalendar, Tenor, WEEKENDS_ONLY};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use indexmap::IndexMap;

/// Apply a time roll-forward operation.
///
/// The function advances the valuation date by the requested period and computes
/// theta/carry for each instrument (if a portfolio is supplied). Theta is
/// realized-forward: carry is `PV_rolled(t1) - PV(t0) + CF_(t0,t1]`, where
/// `t1` is valued on the market after
/// [`MarketContext::roll_forward`](finstack_quant_core::market_data::context::MarketContext::roll_forward)
/// (curves realize their forwards). It is not a frozen-knot lookup of the
/// unrolled market at a shifted `as_of`.
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
/// ```
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
    apply_time_roll_forward_with_credit(ctx, period_str, mode, &[], None)
}

pub(crate) fn apply_time_roll_forward_with_credit(
    ctx: &mut ExecutionContext,
    period_str: &str,
    mode: TimeRollMode,
    hazard_rolls: &[(
        finstack_quant_core::types::CurveId,
        finstack_quant_core::types::CurveId,
    )],
    provider: Option<&dyn finstack_quant_valuations::recalibration::RecalibrationProvider>,
) -> Result<RollForwardReport> {
    use crate::error::Error;

    let old_date = ctx.as_of;
    let tenor = Tenor::parse(period_str).map_err(|e| Error::InvalidPeriod(e.to_string()))?;
    let (new_date, day_shift) = match mode {
        TimeRollMode::Approximate => {
            let days = tenor.to_days_approx();
            let new_date = old_date + time::Duration::days(days);
            (new_date, days)
        }
        TimeRollMode::CalendarDays => {
            let target = tenor.add_to_date(old_date, None, BusinessDayConvention::Unadjusted)?;
            let days = (target - old_date).whole_days();
            (target, days)
        }
        TimeRollMode::BusinessDays => {
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

    // Roll all curves forward (adjusts base dates, shifts knots, filters expired).
    // Realized-forward semantics: every curve realizes its forwards as the
    // base date advances (discount curves renormalize by DF(dt), hazard
    // curves preserve hazard rates, forward curves preserve forwards,
    // inflation rebases CPI, price/vol-index curves set spot to the old
    // forward). Vol surfaces, FX spot, and fixings stay static.
    let mut rolled_market = ctx.market.roll_forward(day_shift)?;
    for (hazard_id, discount_id) in hazard_rolls {
        use finstack_quant_valuations::recalibration::{
            HazardRecalibrationAction, HazardRecalibrationRequest,
        };
        let provider = provider.ok_or_else(|| {
            Error::Core(finstack_quant_valuations::recalibration::provider_missing(
                "hazard_horizon_replay",
            ))
        })?;
        let rebuilt = provider.rebuild_hazard_curve(&HazardRecalibrationRequest {
            hazard: ctx.market.get_hazard(hazard_id)?,
            source_market: std::sync::Arc::new(ctx.market.clone()),
            target_market: std::sync::Arc::new(rolled_market.clone()),
            discount_curve_id: discount_id.clone(),
            doc_clause: None,
            cds_valuation_convention: None,
            deal_quote_override: None,
            action: HazardRecalibrationAction::TimeRollReplay,
        })?;
        rolled_market = rolled_market.insert(rebuilt.as_ref().clone());
    }

    // Carry uses PV(t0) and cashflows on the pre-roll market, and PV(t1) on
    // the rolled market — the same realized-forward theta as valuations.
    let (instrument_carry, total_carry, failed_instruments) =
        if let Some(instruments) = ctx.instruments.as_ref() {
            calculate_instrument_pnl(instruments, ctx.market, &rolled_market, old_date, new_date)?
        } else {
            (Vec::new(), IndexMap::new(), Vec::new())
        };

    *ctx.market = rolled_market;
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
/// Realized-forward theta (carry) is:
///   Carry = PV_rolled(end_date) - PV(start_date) + Sum(Cashflows from start to end)
///
/// `start_date` is valued on the pre-roll market; `end_date` is valued on
/// the rolled market (curves have realized their forwards). Cashflows in
/// `(start_date, end_date]` are collected from the pre-roll market.
///
/// This accounts for:
/// - Pull-to-par and realized-forward roll-down
/// - Coupon/interest net cashflows during the period
/// - Principal payments during the period
///
/// This matches the valuations theta metric (`compute_theta_breakdown`):
/// value on the rolled market at `t1`.
///
/// # Failure handling
///
/// If either endpoint valuation or cashflow collection returns an error, the
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
#[cfg(not(target_arch = "wasm32"))]
fn time_roll_parallel_threshold() -> usize {
    #[cfg(test)]
    {
        2
    }
    #[cfg(not(test))]
    {
        64
    }
}

fn should_parallel_time_roll(instrument_count: usize) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = instrument_count;
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        instrument_count >= time_roll_parallel_threshold()
    }
}

enum InstrumentPnlRow {
    Carry(String, IndexMap<Currency, Money>),
    Failed(String, String),
}

fn one_instrument_pnl(
    instrument: &dyn Instrument,
    market: &finstack_quant_core::market_data::context::MarketContext,
    rolled: &finstack_quant_core::market_data::context::MarketContext,
    old_date: finstack_quant_core::dates::Date,
    new_date: finstack_quant_core::dates::Date,
) -> InstrumentPnlRow {
    let inst_id = instrument.id().to_string();
    let pv_old = match instrument.value(market, old_date) {
        Ok(v) => v,
        Err(err) => {
            return InstrumentPnlRow::Failed(inst_id, format!("t0 valuation failed: {err}"));
        }
    };
    let pv_new = match instrument.value(rolled, new_date) {
        Ok(v) => v,
        Err(err) => {
            return InstrumentPnlRow::Failed(inst_id, format!("t1 valuation failed: {err}"));
        }
    };
    let mut carry_by_currency: IndexMap<Currency, Money> = IndexMap::new();
    match pv_new.checked_sub(pv_old) {
        Ok(diff) => {
            carry_by_currency.insert(diff.currency(), diff);
        }
        Err(err) => {
            return InstrumentPnlRow::Failed(inst_id, format!("pv diff failed: {err}"));
        }
    }
    let flows = match collect_instrument_cashflows(instrument, market, old_date, new_date) {
        Ok(flows) => flows,
        Err(err) => {
            return InstrumentPnlRow::Failed(inst_id, format!("cashflow collection failed: {err}"))
        }
    };
    for (ccy, flow) in flows {
        carry_by_currency
            .entry(ccy)
            .and_modify(|m| *m += flow)
            .or_insert(flow);
    }
    InstrumentPnlRow::Carry(inst_id, carry_by_currency)
}

fn fold_instrument_pnl_rows(rows: Vec<InstrumentPnlRow>) -> InstrumentPnlResult {
    let mut instrument_carry: Vec<(String, IndexMap<Currency, Money>)> = Vec::new();
    let mut total_carry: IndexMap<Currency, Money> = IndexMap::new();
    let mut failed_instruments = Vec::new();
    for row in rows {
        match row {
            InstrumentPnlRow::Failed(id, reason) => failed_instruments.push((id, reason)),
            InstrumentPnlRow::Carry(id, carry_by_currency) => {
                for (ccy, amount) in &carry_by_currency {
                    total_carry
                        .entry(*ccy)
                        .and_modify(|m| *m += *amount)
                        .or_insert(*amount);
                }
                instrument_carry.push((id, carry_by_currency));
            }
        }
    }
    (instrument_carry, total_carry, failed_instruments)
}

fn calculate_instrument_pnl(
    instruments: &[Box<dyn Instrument>],
    market: &finstack_quant_core::market_data::context::MarketContext,
    rolled: &finstack_quant_core::market_data::context::MarketContext,
    old_date: finstack_quant_core::dates::Date,
    new_date: finstack_quant_core::dates::Date,
) -> Result<InstrumentPnlResult> {
    let map_one = |instrument: &dyn Instrument| {
        one_instrument_pnl(instrument, market, rolled, old_date, new_date)
    };
    let rows = if should_parallel_time_roll(instruments.len()) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use rayon::prelude::*;
            instruments
                .par_iter()
                .map(|instrument| map_one(instrument.as_ref()))
                .collect()
        }
        #[cfg(target_arch = "wasm32")]
        {
            instruments
                .iter()
                .map(|instrument| map_one(instrument.as_ref()))
                .collect()
        }
    } else {
        instruments
            .iter()
            .map(|instrument| map_one(instrument.as_ref()))
            .collect()
    };
    Ok(fold_instrument_pnl_rows(rows))
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
) -> Result<IndexMap<Currency, Money>> {
    let mut result: IndexMap<Currency, Money> = IndexMap::new();

    {
        let flows = instrument.dated_cashflows(market, start_date)?;
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

    Ok(result)
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

    #[derive(Clone)]
    struct MissingCashflows(Attributes);

    impl finstack_quant_cashflows::CashflowScheduleSource for MissingCashflows {
        fn raw_cashflow_schedule(
            &self,
            _: &MarketContext,
            _: Date,
        ) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule>
        {
            Err(finstack_quant_core::Error::Validation(
                "missing fixing for coupon".into(),
            ))
        }
    }

    impl Instrument for MissingCashflows {
        fn id(&self) -> &str {
            "missing-cashflows"
        }
        fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
            finstack_quant_valuations::pricer::InstrumentType::Bond
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
        fn attributes(&self) -> &Attributes {
            &self.0
        }
        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.0
        }
        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }
        fn base_value(&self, _: &MarketContext, _: Date) -> finstack_quant_core::Result<Money> {
            Money::new(100.0, Currency::USD)
        }
        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
        {
            Ok(Default::default())
        }
    }

    #[test]
    fn cashflow_failure_excludes_instrument_from_carry() {
        let mut market = MarketContext::new();
        let mut instruments: Vec<Box<dyn Instrument>> =
            vec![Box::new(MissingCashflows(Attributes::new()))];
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: Some(&mut instruments),
            rate_bindings: None,
            calendar: None,
            as_of: date!(2025 - 01 - 01),
        };
        let report =
            apply_time_roll_forward(&mut ctx, "1M", TimeRollMode::CalendarDays).expect("roll");
        assert!(report.instrument_carry.is_empty());
        assert!(report.total_carry.is_empty());
        assert_eq!(report.failed_instruments.len(), 1);
        assert!(report.failed_instruments[0]
            .1
            .contains("cashflow collection failed:"));
    }

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
                .notional(Money::from((100_i64, Currency::USD)))
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

    /// Realized-forward theta: carry uses PV on the rolled market at t1,
    /// not a frozen-knot lookup of the unrolled curve at a shifted as_of.
    #[test]
    fn apply_time_roll_forward_carry_uses_rolled_market_theta() {
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
                .notional(Money::from((100_i64, Currency::USD)))
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

        let instrument = instruments.first().expect("bond instrument").as_ref();
        let pv_old = instrument
            .value(&market, base_date)
            .expect("pv at t0 on unrolled market");

        // 2025-01-01 + 1M calendar is 2025-02-01 (31 days, unadjusted).
        let new_date = base_date + time::Duration::days(31);
        let mut cf_sum = Money::from((0_i64, Currency::USD));
        if let Ok(flows) = instrument.dated_cashflows(&market, base_date) {
            for (date, money) in flows {
                if date > base_date && date <= new_date {
                    cf_sum += money;
                }
            }
        }

        let rolled = market
            .roll_forward(31)
            .expect("roll market for expected t1");
        let pv_new = instrument
            .value(&rolled, new_date)
            .expect("pv at t1 on rolled market");
        let mut expected = pv_new.checked_sub(pv_old).expect("pv diff");
        expected += cf_sum;
        let expected_carry = expected.amount();

        let mut ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: Some(&mut instruments),
            rate_bindings: None,
            calendar: None,
            as_of: base_date,
        };

        let report = apply_time_roll_forward(&mut ctx, "1M", TimeRollMode::CalendarDays)
            .expect("time roll succeeds");
        assert_eq!(ctx.as_of, new_date);
        assert_eq!(report.days, 31);

        let actual = report
            .total_carry
            .get(&Currency::USD)
            .expect("USD carry")
            .amount();
        assert!(
            (actual - expected_carry).abs() < 1e-10,
            "total_carry[USD] should equal value(rolled, t1) - value(old, t0) + CF_(t0,t1]: \
             actual={actual} expected={expected_carry}"
        );
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

    #[test]
    fn approximate_mode_preserves_core_month_rounding() {
        let base_date = date!(2025 - 01 - 01);
        assert_eq!(
            roll_dates(base_date, "6M", TimeRollMode::Approximate).1,
            183
        );
        assert_eq!(
            roll_dates(base_date, "12M", TimeRollMode::Approximate).1,
            365
        );
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

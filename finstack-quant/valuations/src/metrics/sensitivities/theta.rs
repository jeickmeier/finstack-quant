//! Shared utilities for theta (time decay) calculations.
//!
//! Provides period parsing, date rolling, and a generic theta calculator that
//! works for any instrument implementing the `Instrument` trait.
//!
//! # Quick Start
//!
//! ## Example 1: Computing 1-Day Theta for an Equity Option
//!
//! ## Example 2: Computing Custom Period Theta (1 Week)
//!
//! ## Example 3: Bond Carry (Theta with Coupon Accrual)
//!
//! ## Example 4: Computing Theta Near Expiry
//!
//! When an instrument expires before the theta period ends, theta is automatically
//! capped at the expiry date:
//!
//! # How Theta is Calculated
//!
//! Theta represents the total carry (profit/loss) from holding an instrument over a time period:
//!
//! ```text
//! Theta = PV(t + period) - PV(t) + Cashflows(t, t + period)
//! ```
//!
//! Where:
//! - `PV(t)` = present value at valuation date (base value)
//! - `PV(t + period)` = present value at rolled forward date
//! - `Cashflows(t, t + period)` = sum of net cashflows during the period (signed canonical schedule)
//!
//! ## Components
//!
//! 1. **Pull-to-par effect**: Change in present value due to passage of time
//!    - For bonds: Price converges to par as maturity approaches
//!    - For options: Time value decays (typically negative theta)
//!
//! 2. **Cashflows**: Interest, coupons, or other payments during the period
//!    - Bonds: Accrued interest, coupon payments
//!    - Swaps: Net interest payments
//!    - Options: Usually zero (no interim cashflows)
//!
//! ## Sign Convention
//!
//! - **Negative theta**: Instrument loses value over time (e.g., long options)
//! - **Positive theta**: Instrument gains value over time (e.g., short options, carry trades)
//! - **Zero theta**: No time-dependent value change (rare)

use crate::instruments::{Bond, Deposit};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThetaPeriod {
    Days(i64),
    Months(i32),
    Years(i32),
}

fn parse_theta_period(period: &str) -> Result<ThetaPeriod> {
    let input = period.trim();
    let period = input.to_uppercase();
    if input.is_empty() {
        return Err(finstack_quant_core::Error::Validation(
            "Theta period must not be empty".to_string(),
        ));
    }

    let (num_str, unit) = if let Some(pos) = period.find(|c: char| c.is_alphabetic()) {
        (&period[..pos], &period[pos..])
    } else {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Theta period '{input}' must include a unit suffix (D, W, M, or Y)"
        )));
    };

    let num_i64: i64 = num_str.parse().map_err(|_| {
        finstack_quant_core::Error::Validation(format!(
            "Theta period '{input}' has invalid numeric value '{num_str}'"
        ))
    })?;

    if num_i64 < 0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Theta period must be non-negative, got '{period}'"
        )));
    }

    match unit {
        "D" => Ok(ThetaPeriod::Days(num_i64)),
        "W" => Ok(ThetaPeriod::Days(num_i64 * 7)),
        "M" => Ok(ThetaPeriod::Months(i32::try_from(num_i64).map_err(
            |_| {
                finstack_quant_core::Error::Validation(format!(
                    "Theta period '{input}' month value is too large"
                ))
            },
        )?)),
        "Y" => Ok(ThetaPeriod::Years(i32::try_from(num_i64).map_err(
            |_| {
                finstack_quant_core::Error::Validation(format!(
                    "Theta period '{input}' year value is too large"
                ))
            },
        )?)),
        _ => Err(finstack_quant_core::Error::Validation(format!(
            "Theta period '{input}' has unsupported unit '{unit}' (expected D, W, M, or Y)"
        ))),
    }
}

/// Calculate the rolled forward date for theta calculation.
///
/// Advances the base date by the specified period, but caps at the expiry date if the
/// instrument expires before the period ends.
///
/// Notes:
/// - `"D"` and `"W"` are treated as fixed day increments.
/// - `"M"` and `"Y"` are treated as **calendar** month/year rolls (EOM-aware).
///
/// # Arguments
/// * `base_date` - Starting valuation date
/// * `period_str` - Period string (e.g., "1D", "1W", "1M")
/// * `expiry_date` - Optional instrument expiry date
///
/// # Returns
/// The rolled forward date, capped at expiry if applicable
///
/// # Calendar vs. Day Rolling
///
/// `Months(n)` and `Years(n)` use **EOM-aware calendar arithmetic** via
/// `add_months`; `Days(n)` uses a fixed 24-hour duration. These differ at
/// month boundaries:
///
/// - From `2025-01-31`, `Months(1)` rolls to `2025-02-28` (EOM clamped).
/// - From `2025-01-31`, `Days(30)` rolls to `2025-03-02`.
///
/// Theta computed over a "one month" period therefore depends on which
/// `ThetaPeriod` variant the caller selects. Use `Months` for theta that
/// reflects calendar-month carry; use `Days` for fixed-duration theta.
pub(crate) fn calculate_theta_date(
    base_date: Date,
    period_str: &str,
    expiry_date: Option<Date>,
) -> Result<Date> {
    let rolled_date = match parse_theta_period(period_str)? {
        ThetaPeriod::Days(n) => base_date + time::Duration::days(n),
        ThetaPeriod::Months(n) => base_date.add_months(n),
        ThetaPeriod::Years(n) => base_date.add_months(n * 12),
    };

    // Cap at expiry if instrument expires before the rolled date
    if let Some(expiry) = expiry_date {
        if rolled_date > expiry {
            return Ok(expiry);
        }
    }

    Ok(rolled_date)
}

/// Collect period economic cash that occurs during a time period.
///
/// Uses the full `cashflow_schedule()` so that each flow's [`CFKind`] is
/// available for filtering. A flow enters the sum when its **payment date**
/// falls in the receipt interval defined by the instrument valuation boundary and
/// the period-cash policy below includes it. Amounts are signed: positive is cash
/// to the long holder, negative is cash paid by the long holder.
///
/// # Period-cash policy
///
/// Included (signed):
/// - Coupons and stub interest: [`CFKind::Fixed`], [`CFKind::FloatReset`],
///   [`CFKind::InflationCoupon`], [`CFKind::Stub`], [`CFKind::AccruedOnDefault`]
/// - Fees: [`CFKind::Fee`], [`CFKind::CommitmentFee`], [`CFKind::UsageFee`],
///   [`CFKind::FacilityFee`], [`CFKind::MarginInterest`]
/// - Principal that is period cash: [`CFKind::Notional`] (signed, so XCCY
///   exchanges and amortizing redemptions are included),
///   [`CFKind::Amortization`], [`CFKind::PrePayment`],
///   [`CFKind::RevolvingDraw`], [`CFKind::RevolvingRepayment`],
///   [`CFKind::Recovery`]
///
/// Excluded (not instrument cash income):
/// - [`CFKind::Pik`] (capitalized, not paid)
/// - [`CFKind::DefaultedNotional`] (write-down, not a cash transfer)
/// - Collateral/margin transfers: IM/VM post/return and collateral
///   substitution
/// - An opening notional draw (`CFKind::Notional` < 0 on
///   `Bond.issue_date` or a deposit's effective start): trade-level
///   purchase / funding cash, not in PV and not buy-and-hold period
///   income
///
/// For start-of-day PV (same-day cash included), receipts fall in
/// `[start_date, end_date)`. For PV excluding same-day settled cash, receipts
/// fall in `(start_date, end_date]`. The instrument declares this convention
/// through `Instrument::includes_valuation_date_cashflows`.
///
/// # Arguments
///
/// * `instrument` - Instrument whose full cashflow schedule is queried.
/// * `curves` - Market context used to construct the schedule and convert
///   eligible receipts into `base_currency` at each payment date using this
///   snapshot's FX provider. Missing FX rates propagate as errors.
/// * `start_date` - Opening valuation date; receipt inclusion follows the instrument PV boundary.
/// * `end_date` - Closing valuation date; receipt inclusion follows the instrument PV boundary.
/// * `base_currency` - Reporting currency of the returned economic-income sum.
///
/// # Returns
/// Sum of cashflow amounts in the period (converted to base currency)
pub fn collect_cashflows_in_period(
    instrument: &dyn crate::instruments::Instrument,
    curves: &finstack_quant_core::market_data::context::MarketContext,
    start_date: Date,
    end_date: Date,
    base_currency: Currency,
) -> Result<f64> {
    let schedule = instrument.cashflow_schedule(curves, start_date)?;
    collect_cashflows_from_flows(
        schedule.get_flows(),
        curves,
        start_date,
        end_date,
        base_currency,
        opening_notional_draw_date(instrument),
        instrument.includes_valuation_date_cashflows(),
    )
}

pub(crate) fn collect_cashflows_in_period_cached(
    context: &mut crate::metrics::MetricContext,
    start_date: Date,
    end_date: Date,
    base_currency: Currency,
) -> Result<f64> {
    let curves = std::sync::Arc::clone(&context.curves);
    let skip_issue_draw_on = opening_notional_draw_date(context.instrument.as_ref());
    let include_same_day = context.instrument.includes_valuation_date_cashflows();
    let flows = context.tagged_cashflows_cached()?;
    collect_cashflows_from_flows(
        flows,
        &curves,
        start_date,
        end_date,
        base_currency,
        skip_issue_draw_on,
        include_same_day,
    )
}

fn opening_notional_draw_date(instrument: &dyn crate::instruments::Instrument) -> Option<Date> {
    if let Some(bond) = instrument.as_any().downcast_ref::<Bond>() {
        return Some(bond.issue_date);
    }
    if let Some(deposit) = instrument.as_any().downcast_ref::<Deposit>() {
        return deposit.effective_start_date().ok();
    }
    None
}

/// Whether `kind` is period economic cash for total-return / theta add-back.
///
/// See [`collect_cashflows_in_period`] for the full policy. Coupons are
/// collected on payment date; signed principal is included when it is
/// period cash (XCCY exchanges, amortizing redemptions, revolving
/// draws/repayments).
fn is_period_economic_cash(kind: CFKind) -> bool {
    match kind {
        CFKind::Fixed
        | CFKind::FloatReset
        | CFKind::InflationCoupon
        | CFKind::Stub
        | CFKind::Fee
        | CFKind::CommitmentFee
        | CFKind::UsageFee
        | CFKind::FacilityFee
        | CFKind::Notional
        | CFKind::Amortization
        | CFKind::PrePayment
        | CFKind::RevolvingDraw
        | CFKind::RevolvingRepayment
        | CFKind::Recovery
        | CFKind::AccruedOnDefault
        | CFKind::MarginInterest => true,
        // Excluded: PIK (not cash), defaulted-notional write-downs,
        // IM/VM/collateral substitution, and any future `CFKind` variant.
        _ => false,
    }
}

fn collect_cashflows_from_flows(
    flows: &[finstack_quant_core::cashflow::CashFlow],
    curves: &finstack_quant_core::market_data::context::MarketContext,
    start_date: Date,
    end_date: Date,
    base_currency: Currency,
    skip_issue_draw_on: Option<Date>,
    include_same_day: bool,
) -> Result<f64> {
    let mut sum = finstack_quant_core::money::Money::from((0_i64, base_currency));
    for cf in flows {
        let received = if include_same_day {
            cf.date >= start_date && cf.date < end_date
        } else {
            cf.date > start_date && cf.date <= end_date
        };
        if received && is_period_economic_cash(cf.kind) {
            if cf.kind == CFKind::Notional
                && cf.amount.amount() < 0.0
                && skip_issue_draw_on == Some(cf.date)
            {
                continue;
            }
            sum = sum.checked_add(curves.convert_money(cf.amount, base_currency, cf.date)?)?;
        }
    }
    Ok(sum.amount())
}

/// Last economic payment or contractual expiry, whichever is later.
/// Cashflow dates retain business-day adjustment and payment lag; option
/// exercise expiry remains unchanged on the instrument itself.
pub(crate) fn theta_termination_date(
    context: &mut crate::metrics::MetricContext,
) -> Result<Option<Date>> {
    context
        .instrument
        .last_payment_date(&context.curves, context.as_of)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ThetaBreakdown {
    total: f64,
    carry: f64,
    roll_down: f64,
    /// Calendar days actually rolled (theta_period capped at expiry).
    horizon_days: f64,
}

fn compute_theta_breakdown(context: &mut crate::metrics::MetricContext) -> Result<ThetaBreakdown> {
    let expiry_date = theta_termination_date(context)?;
    let period_str = context
        .get_metric_overrides()
        .and_then(|po| po.theta_period.as_deref())
        .unwrap_or("1D");

    let rolled_date = calculate_theta_date(context.as_of, period_str, expiry_date)?;

    if rolled_date <= context.as_of {
        tracing::warn!(
            as_of = %context.as_of,
            rolled_date = %rolled_date,
            "Theta calculation rolled to or before the valuation date; returning 0.0"
        );
        return Ok(ThetaBreakdown {
            total: 0.0,
            carry: 0.0,
            roll_down: 0.0,
            horizon_days: 0.0,
        });
    }

    // Theta uses value() (instrument-economics-signed PV) for both base and rolled dates.
    let base_pv = context
        .reprice_money(&context.curves, context.as_of)?
        .amount();
    let base_currency = context.base_value.currency();

    let horizon_days = (rolled_date - context.as_of).whole_days();
    let rolled_market = context.curves.roll_forward(horizon_days)?;
    let rolled_pv = context.reprice_money(&rolled_market, rolled_date)?.amount();

    let start_date = context.as_of;
    let carry =
        collect_cashflows_in_period_cached(context, start_date, rolled_date, base_currency)?;

    let roll_down = rolled_pv - base_pv;

    Ok(ThetaBreakdown {
        total: carry + roll_down,
        carry,
        roll_down,
        horizon_days: horizon_days as f64,
    })
}

fn store_theta_breakdown(context: &mut crate::metrics::MetricContext, breakdown: ThetaBreakdown) {
    context
        .computed
        .insert(crate::metrics::MetricId::ThetaCarry, breakdown.carry);
    context
        .computed
        .insert(crate::metrics::MetricId::ThetaRollDown, breakdown.roll_down);
    // Stamp the realized horizon so consumers rescaling these period totals
    // (e.g. P&L attribution) can normalize instead of assuming 1 day.
    context.computed.insert(
        crate::metrics::MetricId::ThetaPeriodDays,
        breakdown.horizon_days,
    );
}

/// Universal theta calculator that works with any instrument via the Instrument trait.
///
/// Computes theta as the total carry from rolling the valuation date forward:
///   Theta = PV(end_date) - PV(start_date) + Sum(Cashflows from start to end)
///
/// This calculator works with `dyn Instrument` directly, using the trait's `value()` method,
/// and is registered as the default theta calculator for all instruments.
#[derive(Default)]
pub(crate) struct GenericThetaAny;

impl crate::metrics::MetricCalculator for GenericThetaAny {
    fn calculate(&self, context: &mut crate::metrics::MetricContext) -> Result<f64> {
        let breakdown = compute_theta_breakdown(context)?;
        store_theta_breakdown(context, breakdown);
        Ok(breakdown.total)
    }
}

// Unit tests (internal helpers)

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use time::macros::date;
    use time::Month;

    fn test_date() -> Date {
        date!(2025 - 01 - 01)
    }

    // Theta date calculation

    #[test]
    fn calculate_theta_date_no_expiry() {
        let base = test_date();
        let rolled = calculate_theta_date(base, "1D", None).expect("roll 1D");
        let expected = Date::from_calendar_date(2025, Month::January, 2).expect("expected date");
        assert_eq!(rolled, expected);
    }

    #[test]
    fn calculate_theta_date_one_week() {
        let base = test_date();
        let rolled = calculate_theta_date(base, "1W", None).expect("roll 1W");
        let expected = Date::from_calendar_date(2025, Month::January, 8).expect("expected date");
        assert_eq!(rolled, expected);
    }

    #[test]
    fn calculate_theta_date_one_month() {
        let base = test_date();
        let rolled = calculate_theta_date(base, "1M", None).expect("roll 1M");
        let expected = Date::from_calendar_date(2025, Month::February, 1).expect("expected date");
        assert_eq!(rolled, expected);
    }

    #[test]
    fn calculate_theta_date_with_expiry_cap() {
        let base = test_date();
        let expiry = Date::from_calendar_date(2025, Month::January, 5).expect("expiry date");

        let rolled = calculate_theta_date(base, "1W", Some(expiry)).expect("roll 1W");
        assert_eq!(rolled, expiry);
    }

    #[test]
    fn calculate_theta_date_before_expiry() {
        let base = test_date();
        let expiry = Date::from_calendar_date(2025, Month::February, 1).expect("expiry date");

        let rolled = calculate_theta_date(base, "1D", Some(expiry)).expect("roll 1D");
        let expected = Date::from_calendar_date(2025, Month::January, 2).expect("expected date");
        assert_eq!(rolled, expected);
    }

    #[test]
    fn calculate_theta_date_exactly_at_expiry() {
        let base = test_date();
        let expiry = Date::from_calendar_date(2025, Month::January, 31).expect("expiry date");

        let rolled = calculate_theta_date(base, "30D", Some(expiry)).expect("roll 30D");
        assert_eq!(rolled, expiry);
    }

    #[test]
    fn calculate_theta_date_already_past_expiry() {
        let base = Date::from_calendar_date(2025, Month::February, 1).expect("base date");
        let expiry = test_date();

        let rolled = calculate_theta_date(base, "1D", Some(expiry)).expect("roll 1D");
        assert_eq!(rolled, expiry);
    }

    #[test]
    fn calculate_theta_date_various_periods() {
        let base = test_date();

        let rolled_3m = calculate_theta_date(base, "3M", None).expect("roll 3M");
        assert_eq!(
            rolled_3m,
            Date::from_calendar_date(2025, Month::April, 1).expect("expected date")
        );

        let rolled_1y = calculate_theta_date(base, "1Y", None).expect("roll 1Y");
        assert_eq!(
            rolled_1y,
            Date::from_calendar_date(2026, Month::January, 1).expect("expected date")
        );
    }

    #[test]
    fn invalid_theta_period_error_includes_input() {
        let err = calculate_theta_date(test_date(), "12Q", None).expect_err("bad unit should fail");

        assert!(
            err.to_string().contains("12Q"),
            "theta period error should include the offending input, got: {err}"
        );
    }

    #[test]
    fn theta_workflow_short_dated_expiry_capped() {
        let base = test_date();
        let expiry = Date::from_calendar_date(2025, Month::January, 6).expect("expiry date");

        let theta_date_1d = calculate_theta_date(base, "1D", Some(expiry)).expect("roll 1D");
        assert_eq!(
            theta_date_1d,
            Date::from_calendar_date(2025, Month::January, 2).expect("expected date")
        );

        let theta_date_1w = calculate_theta_date(base, "1W", Some(expiry)).expect("roll 1W");
        assert_eq!(theta_date_1w, expiry);
    }

    fn test_flow(day: u8, amount: f64, kind: CFKind) -> finstack_quant_core::cashflow::CashFlow {
        use finstack_quant_core::money::Money;
        finstack_quant_core::cashflow::CashFlow::new(
            Date::from_calendar_date(2025, Month::January, day).expect("flow date"),
            None,
            Money::new(amount, Currency::USD).expect("valid money fixture"),
            kind,
            0.5,
            None,
        )
    }

    #[test]
    fn period_cash_converts_each_payment_at_its_date() {
        use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
        use finstack_quant_core::money::Money;
        struct DatedFx;
        impl FxProvider for DatedFx {
            fn rate(
                &self,
                from: Currency,
                to: Currency,
                on: Date,
                _policy: FxConversionPolicy,
            ) -> Result<f64> {
                assert_eq!((from, to), (Currency::EUR, Currency::USD));
                Ok(if on.day() == 2 { 1.1 } else { 1.2 })
            }
        }
        let market = finstack_quant_core::market_data::context::MarketContext::new()
            .insert_fx(FxMatrix::new(std::sync::Arc::new(DatedFx)));
        let mut euro_receipt = test_flow(2, 100.0, CFKind::Fixed);
        euro_receipt.amount = Money::from((100_i64, Currency::EUR));
        let mut euro_payment = test_flow(3, -50.0, CFKind::Fixed);
        euro_payment.amount = Money::from((-50_i64, Currency::EUR));
        let flows = [
            euro_receipt,
            euro_payment,
            test_flow(4, 10.0, CFKind::Fixed),
        ];
        let sum = collect_cashflows_from_flows(
            &flows,
            &market,
            date!(2025 - 01 - 01),
            date!(2025 - 01 - 10),
            Currency::USD,
            None,
            true,
        )
        .unwrap();
        assert!((sum - 60.0).abs() < 1e-9);
        let missing_fx = finstack_quant_core::market_data::context::MarketContext::new();
        assert!(collect_cashflows_from_flows(
            &flows,
            &missing_fx,
            date!(2025 - 01 - 01),
            date!(2025 - 01 - 10),
            Currency::USD,
            None,
            true
        )
        .is_err());
    }

    #[test]
    fn period_cash_includes_signed_principal_and_excludes_non_cash() {
        let start = date!(2025 - 01 - 01);
        let end = date!(2025 - 01 - 10);
        let flows = [
            test_flow(2, 25_000.0, CFKind::Fixed),
            test_flow(3, -1_000_000.0, CFKind::Notional),
            test_flow(4, 50_000.0, CFKind::Amortization),
            test_flow(5, 10_000.0, CFKind::Pik),
            test_flow(6, -100_000.0, CFKind::DefaultedNotional),
            test_flow(10, 25_000.0, CFKind::Fixed), // exclusive end
        ];

        let sum = collect_cashflows_from_flows(
            &flows,
            &finstack_quant_core::market_data::context::MarketContext::new(),
            start,
            end,
            Currency::USD,
            None,
            true,
        )
        .expect("collect");
        // coupon 25k + signed notional -1mm + amort 50k; PIK/default/end-date excluded
        assert!(
            (sum - (25_000.0 - 1_000_000.0 + 50_000.0)).abs() < 1e-9,
            "signed principal must enter period cash, got {sum}"
        );
    }

    #[test]
    fn period_cash_skips_bond_issue_draw() {
        let start = date!(2025 - 01 - 01);
        let end = date!(2025 - 01 - 10);
        let issue = date!(2025 - 01 - 02);
        let flows = [
            test_flow(2, -100.0, CFKind::Notional),
            test_flow(3, -50.0, CFKind::Notional),
        ];
        let sum = collect_cashflows_from_flows(
            &flows,
            &finstack_quant_core::market_data::context::MarketContext::new(),
            start,
            end,
            Currency::USD,
            Some(issue),
            true,
        )
        .expect("collect");
        assert!(
            (sum + 50.0).abs() < 1e-12,
            "issue-date draw is excluded; later signed notional remains, got {sum}"
        );
    }

    #[test]
    fn opening_notional_draw_date_uses_deposit_effective_start() {
        let deposit = Deposit::example().expect("example deposit");
        let expected = deposit.effective_start_date().expect("effective start");
        assert_eq!(opening_notional_draw_date(&deposit), Some(expected));
        assert_ne!(
            expected, deposit.start_date,
            "example deposit applies spot lag so effective start differs from trade date"
        );
    }

    #[test]
    fn period_cash_is_half_open_on_payment_date() {
        let start = date!(2025 - 01 - 05);
        let end = date!(2025 - 01 - 06);
        let flows = [
            test_flow(5, 10.0, CFKind::Fixed),
            test_flow(6, 20.0, CFKind::Fixed),
        ];
        let sum = collect_cashflows_from_flows(
            &flows,
            &finstack_quant_core::market_data::context::MarketContext::new(),
            start,
            end,
            Currency::USD,
            None,
            true,
        )
        .expect("collect");
        assert!(
            (sum - 10.0).abs() < 1e-12,
            "only the payment-date flow in [start, end) is income, got {sum}"
        );
    }
}

//! Shared utilities for theta (time decay) calculations.
//!
//! Provides date rolling, and a generic theta calculator that
//! works for any instrument implementing the `Instrument` trait.
//!
//! When an instrument expires before the theta period ends, theta is automatically
//! capped at the expiry date.
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
use finstack_quant_core::dates::{BusinessDayConvention, Date, Tenor};
use finstack_quant_core::Result;

/// Theta horizon for a metric context: `metric_pricing_overrides.theta_period`,
/// else one day.
pub(crate) fn theta_period(context: &crate::metrics::MetricContext) -> Tenor {
    context
        .get_metric_pricing_overrides()
        .and_then(|po| po.theta_period)
        .unwrap_or_else(Tenor::daily)
}

/// Calculate the rolled forward date for theta calculation.
///
/// Advances the base date by the specified period, but caps at the expiry date if the
/// instrument expires before the period ends.
///
/// Notes:
/// - Day and week tenors are fixed day increments.
/// - Month and year tenors are **calendar** month/year rolls (EOM-aware).
///
/// # Arguments
/// * `base_date` - Starting valuation date
/// * `period` - Theta horizon (`metric_pricing_overrides.theta_period`), e.g. 1D, 1W, 1M
/// * `expiry_date` - Optional instrument expiry date
///
/// # Returns
/// The rolled forward date, capped at expiry if applicable
///
/// # Calendar vs. Day Rolling
///
/// Month and year tenors use **EOM-aware calendar arithmetic** via
/// `add_months`; day tenors add a fixed number of days. These differ at
/// month boundaries:
///
/// - From `2025-01-31`, `1M` rolls to `2025-02-28` (EOM clamped).
/// - From `2025-01-31`, `30D` rolls to `2025-03-02`.
///
/// # Errors
///
/// Returns an error when a month or year count cannot be represented for
/// date arithmetic.
pub(crate) fn calculate_theta_date(
    base_date: Date,
    period: Tenor,
    expiry_date: Option<Date>,
) -> Result<Date> {
    let rolled_date = period.add_to_date(base_date, None, BusinessDayConvention::Unadjusted)?;

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
///   [`CFKind::FacilityFee`], [`CFKind::LcFee`], [`CFKind::FrontingFee`],
///   [`CFKind::MarginInterest`]
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
/// * `base_currency` - Reporting currency of the returned economic cash sum, including principal.
///
/// # Returns
/// Signed sum of eligible economic cash in the period, including principal,
/// converted to `base_currency`; capitalized and collateral transfers are excluded.
pub fn collect_cashflows_in_period(
    instrument: &dyn crate::instruments::Instrument,
    curves: &finstack_quant_core::market_data::context::MarketContext,
    start_date: Date,
    end_date: Date,
    base_currency: Currency,
) -> Result<f64> {
    collect_period_cash(instrument, curves, start_date, end_date, base_currency)
        .map(|cash| cash.total.amount())
}

/// Signed cash receipts used by total-return and income attribution.
#[derive(Debug, Clone, Copy)]
pub struct PeriodCash {
    /// All eligible economic cash in the requested reporting currency,
    /// including principal repayments and signed notional exchanges.
    pub total: finstack_quant_core::money::Money,
    /// Coupon, interest and fee cash in the same reporting currency as
    /// `total`; principal, recovery and notional exchanges are excluded.
    pub income: finstack_quant_core::money::Money,
}

/// Collect period cash while separating income from principal receipts.
///
/// Uses the payment-date interval and economic-cash exclusions documented by
/// [`collect_cashflows_in_period`]. Each eligible payment is converted once
/// at its payment-date FX rate. Principal enters `total` without being labeled
/// coupon or interest income.
///
/// # Arguments
///
/// * `instrument` - Instrument supplying classified cashflows and its
///   same-day settlement convention.
/// * `curves` - Market snapshot used to construct the schedule and convert
///   each signed receipt at its payment date; missing FX rates fail the call.
/// * `start_date` - Opening valuation date; the instrument's PV boundary
///   determines whether same-day payments are included.
/// * `end_date` - Closing valuation date; the complementary PV boundary
///   determines whether same-day payments are included.
/// * `base_currency` - Reporting currency shared by the returned total and income.
///
/// # Errors
///
/// Propagates cashflow construction, FX conversion and monetary addition errors.
pub fn collect_period_cash(
    instrument: &dyn crate::instruments::Instrument,
    curves: &finstack_quant_core::market_data::context::MarketContext,
    start_date: Date,
    end_date: Date,
    base_currency: Currency,
) -> Result<PeriodCash> {
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
    collect_period_cash_cached(context, start_date, end_date, base_currency)
        .map(|cash| cash.total.amount())
}

pub(crate) fn collect_period_cash_cached(
    context: &mut crate::metrics::MetricContext,
    start_date: Date,
    end_date: Date,
    base_currency: Currency,
) -> Result<PeriodCash> {
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

/// Interest and fee cash; principal settlement is economic cash but not income.
fn is_period_income(kind: CFKind) -> bool {
    match kind {
        CFKind::Fixed
        | CFKind::FloatReset
        | CFKind::InflationCoupon
        | CFKind::Stub
        | CFKind::Fee
        | CFKind::CommitmentFee
        | CFKind::UsageFee
        | CFKind::FacilityFee
        | CFKind::LcFee
        | CFKind::FrontingFee
        | CFKind::AccruedOnDefault
        | CFKind::MarginInterest => true,
        // Excluded: PIK (not cash), defaulted-notional write-downs,
        // IM/VM/collateral substitution, and any future `CFKind` variant.
        _ => false,
    }
}

fn is_period_economic_cash(kind: CFKind) -> bool {
    is_period_income(kind)
        || matches!(
            kind,
            CFKind::Notional
                | CFKind::Amortization
                | CFKind::PrePayment
                | CFKind::RevolvingDraw
                | CFKind::RevolvingRepayment
                | CFKind::Recovery
        )
}

fn collect_cashflows_from_flows(
    flows: &[finstack_quant_core::cashflow::CashFlow],
    curves: &finstack_quant_core::market_data::context::MarketContext,
    start_date: Date,
    end_date: Date,
    base_currency: Currency,
    skip_issue_draw_on: Option<Date>,
    include_same_day: bool,
) -> Result<PeriodCash> {
    let zero = finstack_quant_core::money::Money::from((0_i64, base_currency));
    let mut cash = PeriodCash {
        total: zero,
        income: zero,
    };
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
            let payment = curves.convert_money(cf.amount, base_currency, cf.date)?;
            cash.total = cash.total.checked_add(payment)?;
            if is_period_income(cf.kind) {
                cash.income = cash.income.checked_add(payment)?;
            }
        }
    }
    Ok(cash)
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
    let period = theta_period(context);

    let rolled_date = calculate_theta_date(context.as_of, period, expiry_date)?;

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
    let schedule = context
        .instrument
        .cashflow_schedule(&context.curves, context.as_of)?;
    let projected_market = finstack_quant_cashflows::fixings::materialize_fixings(
        &context.curves,
        [&schedule],
        context.as_of,
        rolled_date,
    )?;
    let rolled_market = projected_market.roll_forward(horizon_days)?;
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

    fn tenor(period: &str) -> Tenor {
        Tenor::parse(period).expect("valid theta period fixture")
    }

    // Theta date calculation

    #[test]
    fn calculate_theta_date_no_expiry() {
        let base = test_date();
        let rolled = calculate_theta_date(base, tenor("1D"), None).expect("roll 1D");
        let expected = Date::from_calendar_date(2025, Month::January, 2).expect("expected date");
        assert_eq!(rolled, expected);
    }

    #[test]
    fn calculate_theta_date_one_week() {
        let base = test_date();
        let rolled = calculate_theta_date(base, tenor("1W"), None).expect("roll 1W");
        let expected = Date::from_calendar_date(2025, Month::January, 8).expect("expected date");
        assert_eq!(rolled, expected);
    }

    #[test]
    fn calculate_theta_date_one_month() {
        let base = test_date();
        let rolled = calculate_theta_date(base, tenor("1M"), None).expect("roll 1M");
        let expected = Date::from_calendar_date(2025, Month::February, 1).expect("expected date");
        assert_eq!(rolled, expected);
    }

    #[test]
    fn calculate_theta_date_with_expiry_cap() {
        let base = test_date();
        let expiry = Date::from_calendar_date(2025, Month::January, 5).expect("expiry date");

        let rolled = calculate_theta_date(base, tenor("1W"), Some(expiry)).expect("roll 1W");
        assert_eq!(rolled, expiry);
    }

    #[test]
    fn calculate_theta_date_before_expiry() {
        let base = test_date();
        let expiry = Date::from_calendar_date(2025, Month::February, 1).expect("expiry date");

        let rolled = calculate_theta_date(base, tenor("1D"), Some(expiry)).expect("roll 1D");
        let expected = Date::from_calendar_date(2025, Month::January, 2).expect("expected date");
        assert_eq!(rolled, expected);
    }

    #[test]
    fn calculate_theta_date_exactly_at_expiry() {
        let base = test_date();
        let expiry = Date::from_calendar_date(2025, Month::January, 31).expect("expiry date");

        let rolled = calculate_theta_date(base, tenor("30D"), Some(expiry)).expect("roll 30D");
        assert_eq!(rolled, expiry);
    }

    #[test]
    fn calculate_theta_date_already_past_expiry() {
        let base = Date::from_calendar_date(2025, Month::February, 1).expect("base date");
        let expiry = test_date();

        let rolled = calculate_theta_date(base, tenor("1D"), Some(expiry)).expect("roll 1D");
        assert_eq!(rolled, expiry);
    }

    #[test]
    fn calculate_theta_date_various_periods() {
        let base = test_date();

        let rolled_3m = calculate_theta_date(base, tenor("3M"), None).expect("roll 3M");
        assert_eq!(
            rolled_3m,
            Date::from_calendar_date(2025, Month::April, 1).expect("expected date")
        );

        let rolled_1y = calculate_theta_date(base, tenor("1Y"), None).expect("roll 1Y");
        assert_eq!(
            rolled_1y,
            Date::from_calendar_date(2026, Month::January, 1).expect("expected date")
        );
    }

    #[test]
    fn theta_workflow_short_dated_expiry_capped() {
        let base = test_date();
        let expiry = Date::from_calendar_date(2025, Month::January, 6).expect("expiry date");

        let theta_date_1d = calculate_theta_date(base, tenor("1D"), Some(expiry)).expect("roll 1D");
        assert_eq!(
            theta_date_1d,
            Date::from_calendar_date(2025, Month::January, 2).expect("expected date")
        );

        let theta_date_1w = calculate_theta_date(base, tenor("1W"), Some(expiry)).expect("roll 1W");
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
        assert!((sum.total.amount() - 60.0).abs() < 1e-9);
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
        assert_eq!(sum.income.amount(), 25_000.0);
        // coupon 25k + signed notional -1mm + amort 50k; PIK/default/end-date excluded
        assert!(
            (sum.total.amount() - (25_000.0 - 1_000_000.0 + 50_000.0)).abs() < 1e-9,
            "signed principal must enter period cash, got {sum:?}"
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
            (sum.total.amount() + 50.0).abs() < 1e-12,
            "issue-date draw is excluded; later signed notional remains, got {sum:?}"
        );
    }

    #[test]
    fn opening_notional_draw_date_uses_deposit_effective_start() {
        let mut deposit = Deposit::example().expect("example deposit");
        // Saturday start rolls to Monday under ModifiedFollowing on the NYSE calendar.
        deposit.start_date = date!(2024 - 01 - 06);
        deposit.calendar_id = Some("nyse".into());
        let expected = deposit.effective_start_date().expect("effective start");
        assert_eq!(expected, date!(2024 - 01 - 08));
        assert_eq!(opening_notional_draw_date(&deposit), Some(expected));
    }

    #[test]
    fn money_market_maturity_separates_principal_from_interest_income() {
        use crate::instruments::rates::repo::{CollateralSpec, Repo};
        use crate::instruments::Instrument;
        use finstack_quant_core::dates::DayCount;
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::money::Money;

        let principal = Money::from((1_000_000_i64, Currency::USD));
        let start = date!(2025 - 01 - 06);
        let maturity = date!(2025 - 04 - 07);
        let market = MarketContext::new();
        for rate in [0.0, 0.05] {
            let deposit = Deposit::builder()
                .id("DEPOSIT-INCOME".into())
                .notional(principal)
                .start_date(start)
                .maturity(maturity)
                .fixed_rate_opt(Some(rust_decimal::Decimal::try_from(rate).expect("rate")))
                .day_count(DayCount::Act360)
                .discount_curve_id("USD-OIS".into())
                .build()
                .expect("deposit");
            let repo = Repo::term(
                "REPO-INCOME",
                principal,
                CollateralSpec::new("BOND", 100.0, "BOND-PX"),
                rate,
                start,
                maturity,
                "USD-OIS",
            )
            .expect("repo");
            let expected_interest = principal.amount() * rate * (91.0 / 360.0);
            for instrument in [&deposit as &dyn Instrument, &repo as &dyn Instrument] {
                let cash = collect_period_cash(
                    instrument,
                    &market,
                    date!(2025 - 04 - 06),
                    date!(2025 - 04 - 08),
                    Currency::USD,
                )
                .expect("maturity cash");
                assert!((cash.income.amount() - expected_interest).abs() < 1e-8);
                assert_eq!(
                    cash.total.checked_sub(cash.income).expect("principal"),
                    principal
                );
                assert!(
                    (cash.total.amount() - principal.amount() - expected_interest).abs() < 1e-8
                );
                if rate == 0.0 {
                    assert_eq!(cash.income.amount(), 0.0);
                }
            }
        }
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
            (sum.total.amount() - 10.0).abs() < 1e-12,
            "only the payment-date flow in [start, end) is income, got {sum:?}"
        );
    }
}

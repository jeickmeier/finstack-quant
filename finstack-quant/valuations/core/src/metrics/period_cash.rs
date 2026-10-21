//! Period cash: signed economic cash an instrument pays over a horizon, split
//! into income and principal, for theta and total-return attribution.

use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::Result;

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
        instrument.notional_draw_date(),
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
    let skip_issue_draw_on = context.instrument.notional_draw_date();
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

/// Eligible economic payments by date, with the same filtering and FX basis
/// as period-cash aggregation. Used to accumulate intermediate cash at yield.
pub(crate) fn period_cash_by_date_cached(
    context: &mut crate::metrics::MetricContext,
    end_date: Date,
) -> Result<Vec<(Date, f64)>> {
    let curves = std::sync::Arc::clone(&context.curves);
    let start_date = context.as_of;
    let currency = context.base_value.currency();
    let skip = context.instrument.notional_draw_date();
    let same_day = context.instrument.includes_valuation_date_cashflows();
    context
        .tagged_cashflows_cached()?
        .iter()
        .map(|flow| {
            collect_cashflows_from_flows(
                std::slice::from_ref(flow),
                &curves,
                start_date,
                end_date,
                currency,
                skip,
                same_day,
            )
            .map(|cash| (flow.date, cash.total.amount()))
        })
        .filter(|result| !matches!(result, Ok((_, amount)) if *amount == 0.0))
        .collect()
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

pub(crate) fn collect_cashflows_from_flows(
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
            let payment = curves
                .convert_money(cf.amount, base_currency, cf.date)?
                .amount;
            cash.total = cash.total.checked_add(payment)?;
            if is_period_income(cf.kind) {
                cash.income = cash.income.checked_add(payment)?;
            }
        }
    }
    Ok(cash)
}

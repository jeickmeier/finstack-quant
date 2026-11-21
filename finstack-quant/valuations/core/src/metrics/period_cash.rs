//! Period cash: signed economic cash an instrument pays over a horizon, split
//! into income and principal, for theta and total-return attribution.

use finstack_quant_core::Result;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;

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

/// Collect period economic cash while separating income from principal receipts.
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
/// - Coupons and stub interest: [`CFKind::Fixed`], [`CFKind::Floating`],
///   [`CFKind::InflationCoupon`], [`CFKind::Stub`], [`CFKind::AccruedOnDefault`]
/// - Fees: [`CFKind::Fee`], [`CFKind::CommitmentFee`], [`CFKind::UsageFee`],
///   [`CFKind::FacilityFee`], [`CFKind::LcFee`], [`CFKind::FrontingFee`],
///   [`CFKind::MarginInterest`]
/// - Principal that is period cash: [`CFKind::Notional`] (signed, so XCCY
///   exchanges and amortizing redemptions are included),
///   [`CFKind::Amortization`], [`CFKind::PrePayment`],
///   [`CFKind::RevolvingDraw`], [`CFKind::RevolvingRepayment`],
///   [`CFKind::Recovery`]
/// - Credit protection cash: [`CFKind::ProtectionPayment`]
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
/// Each eligible payment is converted once at its payment-date FX rate.
/// Principal enters `total` without being labeled coupon or interest income.
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
            .map(|cash| (flow.get_date(), cash.total.amount()))
        })
        .filter(|result| !matches!(result, Ok((_, amount)) if *amount == 0.0))
        .collect()
}

/// Interest and fee cash; principal settlement is economic cash but not income.
fn is_period_income(kind: CFKind) -> bool {
    match kind {
        CFKind::Fixed
        | CFKind::Floating
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
                | CFKind::ProtectionPayment
                | CFKind::DerivativeSettlement
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
            cf.get_date() >= start_date && cf.get_date() < end_date
        } else {
            cf.get_date() > start_date && cf.get_date() <= end_date
        };
        if received && is_period_economic_cash(cf.get_kind()) {
            if cf.get_kind() == CFKind::Notional
                && cf.get_amount().amount() < 0.0
                && skip_issue_draw_on == Some(cf.get_date())
            {
                continue;
            }
            let payment = curves
                .convert_money(
                    cf.get_amount(),
                    base_currency,
                    cf.get_date(),
                    finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
                )?
                .amount;
            cash.total = cash.total.checked_add(payment)?;
            if is_period_income(cf.get_kind()) {
                cash.income = cash.income.checked_add(payment)?;
            }
        }
    }
    Ok(cash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::cashflow::CashFlowParams;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
    use time::Month;
    use time::macros::date;

    fn test_flow(day: u8, amount: f64, kind: CFKind) -> finstack_quant_core::cashflow::CashFlow {
        test_flow_in(
            day,
            Money::new(amount, Currency::USD).expect("valid money fixture"),
            kind,
        )
    }

    fn test_flow_in(
        day: u8,
        amount: Money,
        kind: CFKind,
    ) -> finstack_quant_core::cashflow::CashFlow {
        let date = Date::from_calendar_date(2025, Month::January, day).expect("flow date");
        let params = if !kind.is_principal_like() {
            CashFlowParams::new(date, None, amount, kind, 0.5, None)
        } else if kind.is_cash_settlement() {
            CashFlowParams::new(date, None, amount, kind, 0.5, None)
                .with_principal_delta(amount.checked_neg())
                .with_principal_date(date)
        } else {
            CashFlowParams::new(date, None, Money::zero(amount.currency()), kind, 0.5, None)
                .with_principal_delta(amount)
                .with_principal_date(date)
        };
        params.build().expect("valid cashflow")
    }

    #[test]
    fn period_cash_converts_each_payment_at_its_date() {
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
            .insert_fx(FxMatrix::new(std::sync::Arc::new(DatedFx), None));
        let euro_receipt = test_flow_in(2, Money::from((100_i64, Currency::EUR)), CFKind::Fixed);
        let euro_payment = test_flow_in(3, Money::from((-50_i64, Currency::EUR)), CFKind::Fixed);
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
        assert!(
            collect_cashflows_from_flows(
                &flows,
                &missing_fx,
                date!(2025 - 01 - 01),
                date!(2025 - 01 - 10),
                Currency::USD,
                None,
                true
            )
            .is_err()
        );
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
            test_flow(10, 25_000.0, CFKind::Fixed),
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

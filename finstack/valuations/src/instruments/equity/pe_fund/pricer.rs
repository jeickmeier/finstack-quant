use crate::instruments::equity::pe_fund::waterfall::{AllocationLedger, EquityWaterfallEngine};
use crate::instruments::equity::pe_fund::PrivateMarketsFund;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

pub(crate) fn run_waterfall(fund: &PrivateMarketsFund) -> finstack_core::Result<AllocationLedger> {
    for event in &fund.events {
        if event.amount.currency() != fund.currency {
            return Err(finstack_core::Error::CurrencyMismatch {
                expected: fund.currency,
                actual: event.amount.currency(),
            });
        }
    }
    let engine = EquityWaterfallEngine::new(&fund.waterfall_spec);
    engine.run(&fund.events)
}

pub(crate) fn lp_cashflows(
    fund: &PrivateMarketsFund,
) -> finstack_core::Result<Vec<(finstack_core::dates::Date, Money)>> {
    let ledger = run_waterfall(fund)?;
    Ok(ledger.lp_cashflows())
}

pub(crate) fn compute_pv(
    fund: &PrivateMarketsFund,
    curves: &MarketContext,
) -> finstack_core::Result<Money> {
    if let Some(ref discount_curve_id) = fund.discount_curve_id {
        use crate::instruments::common_impl::discountable::Discountable;
        let flows = lp_cashflows(fund)?;
        let disc = curves.get_discount(discount_curve_id.as_str())?;
        flows.npv(
            disc.as_ref(),
            disc.base_date(),
            Some(fund.waterfall_spec.irr_basis),
        )
    } else {
        let ledger = run_waterfall(fund)?;
        let residual_value = ledger
            .rows
            .last()
            .map(|r| r.lp_unreturned)
            .unwrap_or_else(|| Money::new(0.0, fund.currency));
        Ok(residual_value)
    }
}

/// Resolve the effective valuation date for a private markets fund.
///
/// Private markets funds intentionally ignore the caller's requested `as_of`
/// and anchor valuation to their own state:
///
/// - When a discount curve is configured, the curve's `base_date()` is used.
/// - Otherwise, the latest event date is used (IRR-only / undiscounted path).
///
/// If neither anchor is available (curve lookup fails, or no events exist),
/// the requested date is returned unchanged; the subsequent
/// [`compute_pv`] call then surfaces the underlying error.
pub(crate) fn resolve_as_of(
    fund: &PrivateMarketsFund,
    market: &MarketContext,
    requested: Date,
) -> Date {
    if let Some(ref discount_curve_id) = fund.discount_curve_id {
        market
            .get_discount(discount_curve_id.as_str())
            .map(|disc| disc.base_date())
            .unwrap_or(requested)
    } else {
        fund.events
            .iter()
            .map(|evt| evt.date)
            .max()
            .unwrap_or(requested)
    }
}

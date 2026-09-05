//! Pricing and metric helpers for equity instruments.
//!
use crate::instruments::equity::pe_fund::waterfall::{
    AllocationLedger, EquityWaterfallEngine, FundEventKind,
};
use crate::instruments::equity::pe_fund::PrivateMarketsFund;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

pub(crate) fn run_waterfall(
    fund: &PrivateMarketsFund,
) -> finstack_quant_core::Result<AllocationLedger> {
    for event in &fund.events {
        if event.amount.currency() != fund.currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
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
) -> finstack_quant_core::Result<Vec<(finstack_quant_core::dates::Date, Money)>> {
    let ledger = run_waterfall(fund)?;
    ledger.lp_cashflows()
}

/// Holder-view present value of the fund position.
///
/// Uses one residual-value mode at a time:
/// - stated `unrealized_nav` as of `as_of`, with no future realization events; or
/// - projected future LP cashflows, with no NAV mark.
///
/// Realized flows on or before `as_of` are sunk and excluded.
pub(crate) fn compute_pv(
    fund: &PrivateMarketsFund,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    if fund.unrealized_nav.is_some()
        && fund.events.iter().any(|event| {
            event.date > as_of
                && matches!(
                    event.kind,
                    FundEventKind::Distribution | FundEventKind::Proceeds
                )
        })
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "PrivateMarketsFund '{}' cannot combine unrealized_nav with future \
             distribution/proceeds events after {as_of}; choose a NAV mark or \
             projected-cashflow valuation mode",
            fund.id
        )));
    }
    let nav = match fund.unrealized_nav {
        Some(nav) => {
            if nav.currency() != fund.currency {
                return Err(finstack_quant_core::Error::CurrencyMismatch {
                    expected: fund.currency,
                    actual: nav.currency(),
                });
            }
            nav
        }
        None => Money::from((0_i64, fund.currency)),
    };

    let future_flows: Vec<(Date, Money)> = lp_cashflows(fund)?
        .into_iter()
        .filter(|(d, _)| *d > as_of)
        .collect();

    let future_pv = if let Some(ref discount_curve_id) = fund.discount_curve_id {
        use finstack_quant_core::cashflow::Discountable;
        let disc = curves.get_discount(discount_curve_id.as_str())?;
        // Discount and cashflow cutoff share the caller's valuation date.
        future_flows.npv(disc.as_ref(), as_of)?
    } else {
        let total: f64 = future_flows.iter().map(|(_, m)| m.amount()).sum();
        Money::new(total, fund.currency)?
    };

    future_pv.checked_add(nav)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::equity::pe_fund::waterfall::{FundEvent, WaterfallSpec};
    use finstack_quant_core::currency::Currency;
    use time::macros::date;

    fn fully_realized_fund() -> PrivateMarketsFund {
        // 100% LP promote tier: the LP receives the full 2M distribution.
        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .promote_tier(0.0, 1.0, 0.0)
            .build()
            .expect("spec should build");
        let events = vec![
            FundEvent::contribution(
                date!(2020 - 01 - 01),
                Money::from((1_000_000_i64, Currency::USD)),
            ),
            FundEvent::distribution(
                date!(2025 - 01 - 01),
                Money::from((2_000_000_i64, Currency::USD)),
            ),
        ];
        PrivateMarketsFund::new("PMF-M13", Currency::USD, spec, events)
    }

    #[test]
    fn fully_realized_fund_prices_to_zero() {
        // Holder view: all flows are on or before the requested valuation
        // date, so the residual position value is zero.
        let fund = fully_realized_fund();
        let market = MarketContext::new();
        let as_of = date!(2025 - 06 - 01);
        let pv = fund.value(&market, as_of).expect("pv should compute");
        assert!(
            pv.amount().abs() < 1e-9,
            "fully realized fund should price to ~0, got {}",
            pv.amount()
        );
    }

    #[test]
    fn unrealized_nav_adds_to_pv() {
        let fund =
            fully_realized_fund().with_unrealized_nav(Money::from((750_000_i64, Currency::USD)));
        let market = MarketContext::new();
        let as_of = date!(2025 - 06 - 01);
        let pv = fund.value(&market, as_of).expect("pv should compute");
        assert!(
            (pv.amount() - 750_000.0).abs() < 1e-9,
            "PV should equal the stated unrealized NAV, got {}",
            pv.amount()
        );
    }

    #[test]
    fn canonical_pricing_includes_future_lp_flows_undiscounted_without_curve() {
        let fund = fully_realized_fund();
        let market = MarketContext::new();
        let as_of = date!(2024 - 01 - 01);
        let pv = fund.value(&market, as_of).expect("pv should compute");
        assert!(
            (pv.amount() - 2_000_000.0).abs() < 1e-6,
            "future LP distribution should be included, got {}",
            pv.amount()
        );
    }

    #[test]
    fn unrealized_nav_rejects_future_realization_events() {
        let fund =
            fully_realized_fund().with_unrealized_nav(Money::from((750_000_i64, Currency::USD)));
        let error = compute_pv(&fund, &MarketContext::new(), date!(2024 - 01 - 01))
            .expect_err("NAV plus future distributions must fail");
        assert!(error.to_string().contains("valuation mode"));
    }

    #[test]
    fn unrealized_nav_currency_mismatch_errors() {
        let fund = fully_realized_fund().with_unrealized_nav(Money::from((100_i64, Currency::EUR)));
        let market = MarketContext::new();
        let result = compute_pv(&fund, &market, date!(2025 - 01 - 01));
        assert!(matches!(
            result,
            Err(finstack_quant_core::Error::CurrencyMismatch { .. })
        ));
    }
}

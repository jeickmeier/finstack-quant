//! DCF pricer implementation.

use super::DiscountedCashFlow;
use crate::instruments::common_impl::traits::Instrument;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_core::{dates::Date, market_data::context::MarketContext, money::Money};

/// Pricer for Discounted Cash Flow instruments.
pub(crate) struct DcfPricer;

/// Compute the DCF equity present value.
///
/// # Discounting basis
///
/// Explicit cash flows and the terminal value always discount at the
/// instrument's own `wacc` via `(1 + wacc)^{-t}` (the
/// previous behavior silently switched to risk-free curve discounting when
/// the named curve happened to be loaded, changing the PV of risky cash
/// flows with no spread adjustment). The PV is identical whether or not
/// [`DiscountedCashFlow::discount_curve_id`] is loaded in `market`.
///
/// Rate sensitivity comes from the risk-free component embedded in the WACC
/// (`wacc = rf + risk_premium`): the `Dv01`/`BucketedDv01` metrics bump that
/// rf component inside the rate (see
/// [`pv_with_rf_bump`]), not the discounting regime.
pub(crate) fn compute_pv(
    dcf: &DiscountedCashFlow,
    _market: &MarketContext,
    _as_of: Date,
) -> finstack_core::Result<Money> {
    // DCF is anchored to `dcf.valuation_date`; the trait-level `as_of` is
    // intentionally ignored to keep discount timing deterministic for a
    // configured valuation scenario.
    let equity_value = pv_with_rf_bump(dcf, &|_| 0.0)?;
    Ok(Money::new(equity_value, dcf.currency))
}

/// Equity PV with the risk-free component of the WACC bumped by `bump_at(t)`
/// (absolute, decimal) at each cashflow tenor `t` in years.
///
/// The bump applies everywhere the WACC appears: per-flow discounting, the
/// terminal-value capitalization (Gordon/H-model `1/(wacc − g)`), and the
/// terminal discounting — which is the analytic `∂PV/∂rf` under the additive
/// decomposition `wacc = rf + risk_premium`. `bump_at = |_| 0.0` reproduces
/// the unbumped PV exactly.
pub(crate) fn pv_with_rf_bump(
    dcf: &DiscountedCashFlow,
    bump_at: &dyn Fn(f64) -> f64,
) -> finstack_core::Result<f64> {
    // Terminal tenor: ExitMultiple discounts at the full horizon t_n (a
    // point-in-time sale price); Gordon/H-model keep the mid-year-adjusted
    // tenor (flow-stream proxies). See `terminal_discount_years`.
    let t_term = dcf.terminal_discount_years()?;
    let bump_term = bump_at(t_term);

    // Terminal value capitalized at the bumped WACC (validates WACC > g).
    let mut dcf_term = dcf.clone();
    dcf_term.wacc = dcf.wacc + bump_term;
    let terminal_value = dcf_term.calculate_terminal_value()?;
    let pv_terminal = terminal_value / (1.0 + dcf.wacc + bump_term).powf(t_term);

    let pv_explicit: f64 = dcf
        .flows
        .iter()
        .map(|(date, amount)| {
            let t = dcf.discount_years(dcf.valuation_date, *date);
            amount / (1.0 + dcf.wacc + bump_at(t)).powf(t)
        })
        .sum();

    let equity_value = pv_explicit + pv_terminal - dcf.effective_net_debt();
    dcf.apply_valuation_discounts(equity_value)
}

impl Pricer for DcfPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::DCF, ModelKey::Discounting)
    }

    #[tracing::instrument(
        name = "dcf_equity.discounting.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let dcf = instrument
            .as_any()
            .downcast_ref::<DiscountedCashFlow>()
            .ok_or_else(|| PricingError::type_mismatch(InstrumentType::DCF, instrument.key()))?;

        let equity_value = compute_pv(dcf, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(dcf).model(ModelKey::Discounting),
            )
        })?;

        Ok(ValuationResult::stamped(dcf.id(), as_of, equity_value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::equity::dcf_equity::types::{DiscountedCashFlow, TerminalValueSpec};
    use finstack_core::currency::Currency;
    use finstack_core::dates::Date;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn build_simple_dcf() -> DiscountedCashFlow {
        let valuation_date =
            Date::from_calendar_date(2025, Month::January, 1).expect("valid valuation date");
        let flow_date =
            Date::from_calendar_date(2026, Month::January, 1).expect("valid cashflow date");

        DiscountedCashFlow {
            id: InstrumentId::new("TEST-DCF-PRICER"),
            currency: Currency::USD,
            flows: vec![(flow_date, 100.0)],
            wacc: 0.10,
            terminal_value: TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
            net_debt: 0.0,
            valuation_date,
            discount_curve_id: CurveId::new("USD-OIS"),
            mid_year_convention: false,
            terminal_flow_override: None,
            equity_bridge: None,
            shares_outstanding: None,
            dilution_securities: Vec::new(),
            valuation_discounts: None,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: crate::instruments::common_impl::traits::Attributes::default(),
        }
    }

    #[test]
    fn compute_pv_matches_instrument_value() {
        let dcf = build_simple_dcf();
        let market = MarketContext::new();
        let expected = dcf
            .value(&market, dcf.valuation_date)
            .expect("instrument value should succeed");

        let via_pricer = compute_pv(&dcf, &market, dcf.valuation_date).expect("pricer pv");

        assert_eq!(via_pricer, expected);
    }
}

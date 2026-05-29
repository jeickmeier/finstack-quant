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
/// # Discounting basis (two distinct rates, by design)
///
/// This model deliberately uses two separate rates:
/// - **Discount rate** — explicit cash flows *and* the terminal value are
///   present-valued with the market discount curve named by
///   [`DiscountedCashFlow::discount_curve_id`] when that curve is loaded in
///   `market`. This is what gives the instrument its rate sensitivity; the
///   `Dv01`/`BucketedDv01`/`EnterpriseValue` metrics bump exactly this curve.
///   When the curve is **not** loaded, discounting falls back to the
///   instrument's own `wacc` via `(1 + wacc)^t`.
/// - **Terminal cap rate** — the terminal value itself is *capitalized* at
///   `wacc` inside [`DiscountedCashFlow::calculate_terminal_value`] (the
///   Gordon/H-model `1 / (wacc - g)` factor), independent of the discount rate.
///
/// Pairing a WACC cap rate with curve discounting is intentional: the exit value
/// is a long-run, growth-adjusted multiple (WACC-based), while the path back to
/// today is discounted at observable market rates. Consequently the PV moves with
/// the loaded curve. The metric calculators in `metrics/mod.rs` mirror this exact
/// convention so the reported PV and its sensitivities stay consistent.
pub(crate) fn compute_pv(
    dcf: &DiscountedCashFlow,
    market: &MarketContext,
    _as_of: Date,
) -> finstack_core::Result<Money> {
    // DCF is anchored to `dcf.valuation_date`; the trait-level `as_of` is
    // intentionally ignored to keep discount timing deterministic for a
    // configured valuation scenario.
    // Validate terminal value constraints upfront via calculate_terminal_value().
    // This catches WACC <= growth for Gordon Growth and H-Model.
    let terminal_value = dcf.calculate_terminal_value()?;
    let bridge_amount = dcf.effective_net_debt();

    let enterprise_value = if let Ok(discount_curve) = market.get_discount(&dcf.discount_curve_id) {
        let pv_explicit: f64 = dcf
            .flows
            .iter()
            .map(|(date, amount)| {
                let years = dcf.discount_years(dcf.valuation_date, *date);
                let df = discount_curve.df(years);
                amount * df
            })
            .sum();

        let pv_terminal = if let Some((terminal_date, _)) = dcf.flows.last() {
            let years = dcf.discount_years(dcf.valuation_date, *terminal_date);
            let df = discount_curve.df(years);
            terminal_value * df
        } else {
            0.0
        };

        pv_explicit + pv_terminal
    } else {
        let pv_explicit = dcf.calculate_pv_explicit_flows();
        let pv_terminal = dcf.discount_terminal_value(terminal_value)?;
        pv_explicit + pv_terminal
    };

    let equity_value = enterprise_value - bridge_amount;
    let equity_value = dcf.apply_valuation_discounts(equity_value)?;

    Ok(Money::new(equity_value, dcf.currency))
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

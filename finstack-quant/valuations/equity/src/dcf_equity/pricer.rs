//! DCF pricer implementation.

use super::DiscountedCashFlow;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::{
    Error, Result, dates::Date, market_data::context::MarketContext, money::Money,
};
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, expect_inst,
};
use finstack_quant_valuations_core::results::ValuationResult;

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
/// `DiscountedCashFlow::discount_curve_id` is loaded in `market`.
///
/// WACC sensitivity is reported as `custom("dcf::wacc01")`, which bumps the
/// instrument-owned WACC rather than any market curve.
pub(crate) fn compute_pv(
    dcf: &DiscountedCashFlow,
    _market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    if as_of != dcf.valuation_date {
        return Err(finstack_quant_core::Error::Validation(format!(
            "DCF '{}' must be priced at its configured valuation_date {}; got as_of {}",
            dcf.id, dcf.valuation_date, as_of
        )));
    }
    let equity_value = pv_with_rf_bump(dcf, &|_| 0.0)?;
    Money::from_model_value(equity_value, dcf.currency)
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
) -> finstack_quant_core::Result<f64> {
    let (pv_explicit, pv_terminal) = pv_components_with_rf_bump(dcf, bump_at)?;
    let mut equity = NeumaierAccumulator::new();
    equity.add(enterprise_value(pv_explicit, pv_terminal)?)?;
    equity.add(-dcf.effective_net_debt())?;
    dcf.apply_valuation_discounts(equity.get_total()?)
}

/// Enterprise-value components `(pv_explicit, pv_terminal)` at the WACC with
/// its risk-free component bumped by `bump_at(t)` (absolute, decimal) at each
/// tenor `t` in years; `bump_at = |_| 0.0` gives the unbumped components.
pub(crate) fn pv_components_with_rf_bump(
    dcf: &DiscountedCashFlow,
    bump_at: &dyn Fn(f64) -> f64,
) -> finstack_quant_core::Result<(f64, f64)> {
    let t_term = dcf.terminal_discount_years()?;
    let terminal_wacc = dcf.wacc + bump_at(t_term);
    let terminal_value = dcf.terminal_value_at_wacc(terminal_wacc)?;
    let pv_terminal = discount_cashflow(terminal_value, terminal_wacc, t_term)?;

    let mut explicit = NeumaierAccumulator::new();
    for &(date, amount) in &dcf.flows {
        let years = dcf.explicit_flow_discount_years(date)?;
        explicit.add(discount_cashflow(
            amount.amount(),
            dcf.wacc + bump_at(years),
            years,
        )?)?;
    }
    Ok((explicit.get_total()?, pv_terminal))
}

pub(super) fn enterprise_value(pv_explicit: f64, pv_terminal: f64) -> Result<f64> {
    let value = pv_explicit + pv_terminal;
    if !value.is_finite() {
        return Err(Error::Validation(
            "DCF enterprise value is non-finite".into(),
        ));
    }
    Ok(value)
}

pub(super) fn discount_cashflow(amount: f64, wacc: f64, years: f64) -> Result<f64> {
    if !wacc.is_finite() || wacc <= -1.0 {
        return Err(Error::Validation(
            "DCF discount rate must be a finite annual effective rate above -100%".into(),
        ));
    }
    let denominator = (1.0 + wacc).powf(years);
    let present_value = amount / denominator;
    if !(denominator.is_finite() && denominator > 0.0 && present_value.is_finite()) {
        return Err(Error::Validation(
            "DCF discount factor or present value cannot be represented".into(),
        ));
    }
    Ok(present_value)
}

impl Pricer for DcfPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Dcf, ModelKey::Discounting)
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
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let dcf = expect_inst::<DiscountedCashFlow>(instrument, InstrumentType::Dcf)?;

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
    use crate::dcf_equity::types::{
        DiscountedCashFlow, EquityBridge, ForecastTiming, TerminalValueSpec,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::types::InstrumentId;
    use finstack_quant_valuations_core::instruments::Instrument;
    use time::Month;

    fn build_simple_dcf() -> DiscountedCashFlow {
        let valuation_date =
            Date::from_calendar_date(2025, Month::January, 1).expect("valid valuation date");
        let flow_date =
            Date::from_calendar_date(2026, Month::January, 1).expect("valid cashflow date");

        DiscountedCashFlow {
            id: InstrumentId::new("TEST-DCF-PRICER"),
            currency: Currency::USD,
            flows: vec![(flow_date, Money::from((100_i64, Currency::USD)))],
            wacc: 0.10,
            terminal_value: TerminalValueSpec::GordonGrowth {
                stable_growth_rate: 0.02,
            },
            valuation_date,
            forecast_timing: ForecastTiming::DatedReceipts,
            terminal_date: flow_date,
            normalized_annual_fcff: Some(Money::from((100_i64, Currency::USD))),
            equity_bridge: EquityBridge::default(),
            shares_outstanding: None,
            dilution_securities: Vec::new(),
            valuation_discounts: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: finstack_quant_core::types::Attributes::default(),
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

    #[test]
    fn canonical_pricing_uses_and_stamps_configured_valuation_date() {
        let dcf = build_simple_dcf();
        let requested = dcf.valuation_date + time::Duration::days(30);
        let result = dcf
            .price_with_metrics(
                &MarketContext::new(),
                requested,
                &[],
                finstack_quant_valuations_core::instruments::PricingOptions::default(),
            )
            .expect("canonical DCF pricing");

        assert_eq!(result.as_of, dcf.valuation_date);
    }
}

#[cfg(test)]
mod checked_wacc_tests {
    use super::*;

    #[test]
    fn out_of_domain_wacc_and_denominators_are_not_silently_accepted() {
        assert!(discount_cashflow(100.0, -1.0, 0.0).is_err());
        assert!(discount_cashflow(100.0, f64::NAN, 1.0).is_err());
        assert!(discount_cashflow(100.0, 1.0, 2000.0).is_err());
        assert!(discount_cashflow(100.0, -0.5, 2000.0).is_err());
        assert!(enterprise_value(f64::MAX, f64::MAX).is_err());
        assert_eq!(
            discount_cashflow(100.0, 0.25, 1.0).expect("finite PV"),
            80.0
        );
    }
}

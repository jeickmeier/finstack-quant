//! Equity spot metric calculators and registry registration.
//!
//! Computes price per share, share exposure, dividend yield, forward price,
//! and delta using the shared metrics framework.

use crate::spot::Equity;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{
    MetricCalculator, MetricContext, MetricId, MetricRegistry,
};

/// Computes the price per share for an `Equity`.
pub(crate) struct PricePerShareCalculator;

impl MetricCalculator for PricePerShareCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let equity: &Equity = context.instrument_as()?;
        let m = equity.price_per_share(&context.curves, context.as_of)?;
        Ok(m.amount())
    }
}

/// Computes the effective number of shares for an `Equity`.
pub(crate) struct SharesCalculator;

impl MetricCalculator for SharesCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let equity: &Equity = context.instrument_as()?;
        Ok(equity.effective_quantity())
    }
}

/// Computes the dividend yield from `div_yield_id`, or 0.0 when it is unset.
pub(crate) struct DividendYieldCalculator;

impl MetricCalculator for DividendYieldCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let equity: &Equity = context.instrument_as()?;
        equity.dividend_yield(&context.curves)
    }
}

/// Computes the forward price per share over a horizon in years.
///
/// Horizon resolution order:
/// 1) Try `MarketContext::price("{ticker}-FWD_T")` as a unitless scalar (years)
/// 2) Fallback to 0.0 (spot) when the key is not found
pub(crate) struct ForwardPricePerShareCalculator;

impl MetricCalculator for ForwardPricePerShareCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let equity: &Equity = context.instrument_as()?;
        let key = format!("{}-FWD_T", equity.ticker);
        let t = match context.curves.get_price(&key) {
            Ok(scalar) => scalar.get_unitless().ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "Forward horizon scalar '{key}' must be a unitless number of years, \
                     not a monetary price"
                ))
            })?,
            Err(err) if err.kind() == finstack_quant_core::ErrorKind::NotFound => 0.0,
            Err(err) => return Err(err),
        };
        let money = equity.forward_price_per_share(&context.curves, context.as_of, t)?;
        Ok(money.amount())
    }
}

/// Delta calculator for equity spot.
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let equity: &Equity = context.instrument_as()?;
        let delta = equity.quantity.unwrap_or(1.0);

        context.computed.insert(
            MetricId::composite(&MetricId::Delta, &[equity.ticker.as_str()]),
            delta,
        );

        Ok(delta)
    }
}

/// Register all Equity metrics with the registry
pub(crate) fn register_equity_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Equity,
        metrics: [
            (EquityPricePerShare, PricePerShareCalculator),
            (EquityShares, SharesCalculator),
            (EquityDividendYield, DividendYieldCalculator),
            (EquityForwardPrice, ForwardPricePerShareCalculator),
            (Delta, DeltaCalculator, FactorQuantity),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations_core::instruments::Instrument;
    use std::sync::Arc;
    use time::Month;

    fn test_equity_and_market() -> (Equity, MarketContext, Date) {
        let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("date");
        let disc = DiscountCurve::builder("USD")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.05_f64).exp()),
                (5.0, (-0.25_f64).exp()),
            ])
            .build()
            .expect("discount curve");
        let equity = Equity::new("EQ-AAPL", "AAPL", Currency::USD).with_quoted_spot(100.0);
        let market = MarketContext::new()
            .try_insert(disc)
            .expect("insert discount");
        (equity, market, as_of)
    }

    fn eval_forward_price(
        equity: &Equity,
        market: MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let base_value = equity.value(&market, as_of)?;
        let inst: Arc<dyn Instrument> = Arc::new(equity.clone());
        let mut ctx = MetricContext::new(
            inst,
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );
        ForwardPricePerShareCalculator.calculate(&mut ctx)
    }

    #[test]
    fn forward_price_defaults_to_spot_when_horizon_key_is_absent() {
        let (equity, market, as_of) = test_equity_and_market();
        let fwd = eval_forward_price(&equity, market, as_of).expect("absent FWD_T uses t=0");
        assert!((fwd - 100.0).abs() < 1e-12);
    }

    #[test]
    fn forward_price_uses_unitless_horizon_scalar() {
        let (equity, market, as_of) = test_equity_and_market();
        let market = market.insert_price("AAPL-FWD_T", MarketScalar::unitless(1.0).expect("1y"));
        let fwd = eval_forward_price(&equity, market, as_of).expect("unitless FWD_T");
        let expected = 100.0 * 0.05_f64.exp();
        assert!((fwd - expected).abs() < 1e-9);
    }

    #[test]
    fn forward_price_rejects_monetary_horizon_scalar() {
        let (equity, market, as_of) = test_equity_and_market();
        let market = market.insert_price(
            "AAPL-FWD_T",
            MarketScalar::price(Money::from((1_i64, Currency::USD))),
        );
        let err = eval_forward_price(&equity, market, as_of)
            .expect_err("monetary scalar must be rejected, not defaulted to 0.0");
        assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
    }

    #[test]
    fn forward_price_rejects_negative_unitless_horizon() {
        let (equity, market, as_of) = test_equity_and_market();
        let market = market.insert_price("AAPL-FWD_T", MarketScalar::unitless(-0.5).expect("neg"));
        let err = eval_forward_price(&equity, market, as_of)
            .expect_err("negative horizon must propagate Validation error");
        assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
    }
}

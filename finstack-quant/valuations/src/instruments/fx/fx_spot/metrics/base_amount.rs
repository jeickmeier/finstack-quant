//! Base amount metric for `FxSpot`.
//!
//! Returns the base notional amount in base currency units.

use crate::instruments::fx::fx_spot::FxSpot;
use crate::metrics::{MetricCalculator, MetricContext};

fn base_amount(fx: &FxSpot) -> f64 {
    fx.effective_notional().amount()
}

/// Returns the base amount (notional) in base currency units.
pub struct BaseAmountCalculator;

impl MetricCalculator for BaseAmountCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let fx: &FxSpot = context.instrument_as()?;
        Ok(base_amount(fx))
    }
}

#[cfg(test)]
mod tests {
    use super::{base_amount, BaseAmountCalculator};
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::fx::fx_spot::FxSpot;
    use crate::metrics::{MetricCalculator, MetricContext};
    use finstack_quant_core::{
        currency::Currency, dates::Date, market_data::context::MarketContext, money::Money,
        types::InstrumentId,
    };
    use std::sync::Arc;
    use time::Month;

    fn d(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("should succeed"), day)
            .expect("should succeed")
    }

    fn sample_fx() -> FxSpot {
        FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
            .with_notional(Money::new(1_250_000.0, Currency::EUR))
            .expect("should succeed")
            .with_rate(1.18)
            .expect("should succeed")
    }

    #[test]
    fn helper_returns_base_notional() {
        let fx = sample_fx();
        assert!((base_amount(&fx) - 1_250_000.0).abs() < 1e-6);
    }

    #[test]
    fn calculator_matches_helper() {
        let fx = sample_fx();
        let as_of = d(2025, 2, 10);
        let base_value = fx
            .value(&MarketContext::new(), as_of)
            .expect("should succeed");
        let instrument: Arc<dyn crate::instruments::common_impl::traits::Instrument> = Arc::new(fx);
        let mut ctx = MetricContext::new(
            instrument,
            Arc::new(MarketContext::new()),
            as_of,
            base_value,
            MetricContext::default_config(),
        );
        let calc = BaseAmountCalculator;
        let amount = calc.calculate(&mut ctx).expect("should succeed");
        assert!((amount - 1_250_000.0).abs() < 1e-6);
    }
}

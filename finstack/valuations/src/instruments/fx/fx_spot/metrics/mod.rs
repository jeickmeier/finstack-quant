//! FX Spot metrics module.
//!
//! Provides metric calculators specific to `FxSpot`, split into focused files
//! to mirror the repository-wide metrics organization used by more complex
//! instruments (e.g., `cds`).
//!
//! Exposed metrics via `MetricId::custom("...")` under the instrument type
//! "FxSpot":
//! - `spot_rate`
//! - `base_amount`
//! - `inverse_rate`
//!
//! Note: Quote amount (PV in quote currency) is available in `ValuationResult.value`.

pub(crate) mod base_amount;
pub(crate) mod fx_delta;
pub(crate) mod inverse_rate;
pub(crate) mod spot_rate;

use crate::metrics::MetricRegistry;

/// Register all FX Spot metrics with the registry
pub(crate) fn register_fx_spot_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    // FX Delta (custom metric - FX spot sensitivity per 1%)
    registry.register_metric(
        MetricId::FxDelta,
        Arc::new(fx_delta::FxDeltaCalculator),
        &[InstrumentType::FxSpot],
    );
    // FX01 now uses the shared `GenericFx01Calculator` (1% relative spot
    // move) instead of the per-instrument trivial `notional * 0.0001` shim.
    // This aligns the unit convention with every attribution consumer.
    registry.register_metric(
        MetricId::Fx01,
        crate::metrics::sensitivities::fx01::arc_generic_fx01(),
        &[InstrumentType::FxSpot],
    );

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FxSpot,
        metrics: [
            (SpotRate, spot_rate::SpotRateCalculator),
            (BaseAmount, base_amount::BaseAmountCalculator),
            (InverseRate, inverse_rate::InverseRateCalculator),
        ]
    };
}

#[cfg(test)]
mod tests {
    use super::base_amount::BaseAmountCalculator;
    use crate::instruments::{FxSpot, Instrument};
    use crate::metrics::{MetricCalculator, MetricContext};
    use finstack_core::{
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
            .with_notional(Money::new(5_000_000.0, Currency::EUR))
            .expect("should succeed")
            .with_rate(1.24)
            .expect("should succeed")
    }

    fn context_for(inst: FxSpot, as_of: Date) -> MetricContext {
        let base_value = inst
            .value(&MarketContext::new(), as_of)
            .expect("should succeed");
        let instrument_arc: Arc<dyn Instrument> = Arc::new(inst);
        MetricContext::new(
            instrument_arc,
            Arc::new(MarketContext::new()),
            as_of,
            base_value,
            MetricContext::default_config(),
        )
    }

    #[test]
    fn base_amount_matches_notional() {
        let fx = sample_fx();
        let mut ctx = context_for(fx, d(2025, 1, 15));
        let calc = BaseAmountCalculator;
        let value = calc.calculate(&mut ctx).expect("should succeed");
        assert!((value - 5_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn quote_amount_is_result_value() {
        let fx = sample_fx();
        let as_of = d(2025, 1, 15);
        let base_value = fx
            .value(&MarketContext::new(), as_of)
            .expect("should succeed");
        let result = fx
            .price_with_metrics(
                &MarketContext::new(),
                as_of,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect("should succeed");
        // Quote amount is just the result.value (PV in quote currency)
        assert!((result.value.amount() - base_value.amount()).abs() < 1e-6);
    }

    // FxSpot intentionally does not register DV01/BucketedDv01: it has no
    // discount or forward curve dependency. FX risk is exposed through
    // FxDelta and Fx01.
}

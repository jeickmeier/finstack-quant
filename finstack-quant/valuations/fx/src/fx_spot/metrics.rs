//! FX Spot metrics module.
//!
//! Provides metric calculators specific to `FxSpot`.
//!
//! Exposed metrics under the instrument type "FxSpot":
//! - `spot_rate`
//! - `base_amount`
//! - `inverse_rate`
//!
//! Note: Quote amount (PV in quote currency) is available in `ValuationResult.value`.

use crate::fx_spot::FxSpot;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register all FX Spot metrics with the registry
pub(crate) fn register_fx_spot_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::sensitivities::fx01::GenericFx01Calculator;
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FxSpot,
        metrics: [
            // FxDelta and Fx01 are the same quantity (PV change per 1%
            // relative spot move) and share the generic calculator; both ids
            // are kept because consumers ask for either name.
            (FxDelta, GenericFx01Calculator),
            (Fx01, GenericFx01Calculator),
            (SpotRate, SpotRateCalculator),
            (BaseAmount, BaseAmountCalculator),
            (InverseRate, InverseRateCalculator),
        ]
    };
    Ok(())
}

/// Base amount metric for `FxSpot`.
///
/// Returns the base notional amount in base currency units.
fn base_amount(fx: &FxSpot) -> f64 {
    fx.notional.amount()
}

/// Returns the base amount (notional) in base currency units.
///
/// Returns the base notional amount in base currency units.
pub struct BaseAmountCalculator;

impl MetricCalculator for BaseAmountCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let fx: &FxSpot = context.instrument_as()?;
        Ok(base_amount(fx))
    }
}

/// Threshold below which an inversion is considered ill-conditioned.
const INVERSION_FLOOR: f64 = 1e-12;

/// Calculates the inverse of the spot rate (base per quote).
///
/// # Errors
///
/// Returns a `Validation` error when:
/// - Base notional magnitude is below `INVERSION_FLOOR`
/// - Realized spot magnitude is below `INVERSION_FLOOR`
/// - Either input is non-finite
///
/// A silent zero would mask configuration errors (zero notional, zero spot)
/// in calling code that assumes the inverse is meaningful.
///
/// Computes the inverse of the realized spot rate. Errors when inversion
/// would be numerically unstable rather than silently returning 0.
pub struct InverseRateCalculator;

impl MetricCalculator for InverseRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let fx: &FxSpot = context.instrument_as()?;
        let base_amt = fx.notional.amount();
        let pv = context.base_value.amount();

        if !base_amt.is_finite() || base_amt.abs() < INVERSION_FLOOR {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxSpot {}: cannot compute inverse rate, base notional is non-finite \
                 or below {INVERSION_FLOOR:.0e} (got {base_amt})",
                fx.id
            )));
        }

        let spot = pv / base_amt;
        if !spot.is_finite() || spot.abs() < INVERSION_FLOOR {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxSpot {}: cannot compute inverse rate, realized spot is non-finite \
                 or below {INVERSION_FLOOR:.0e} (got {spot})",
                fx.id
            )));
        }
        Ok(1.0 / spot)
    }
}

/// Epsilon for floating-point near-zero comparisons.
/// This threshold prevents division by extremely small numbers that would
/// produce numerically unstable results.
const EPSILON: f64 = 1e-15;

/// Calculates the FX spot rate as `quote_amount / base_amount`.
///
/// Returns an error when the base notional is near-zero because the realized
/// spot rate is undefined in that case.
///
/// Computes the realized spot rate as `quote_amount / base_amount`, where the
/// `quote_amount` is the instrument PV in quote currency and `base_amount` is
/// the effective base notional.
pub struct SpotRateCalculator;

impl MetricCalculator for SpotRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let fx: &FxSpot = context.instrument_as()?;
        let base_amt = fx.notional.amount();

        if base_amt.abs() < EPSILON {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxSpot spot_rate is undefined for near-zero base notional ({base_amt})"
            )));
        }

        Ok(context.base_value.amount() / base_amt)
    }
}

#[cfg(test)]
mod tests {
    use super::BaseAmountCalculator;
    use crate::fx_spot::FxSpot;
    use finstack_quant_core::{
        currency::Currency, dates::Date, market_data::context::MarketContext, money::Money,
        types::InstrumentId,
    };
    use finstack_quant_valuations_core::instruments::Instrument;
    use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
    use std::sync::Arc;
    use time::Month;

    fn d(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("should succeed"), day)
            .expect("should succeed")
    }

    fn sample_fx() -> FxSpot {
        FxSpot::builder()
            .id(InstrumentId::new("EURUSD"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .notional(Money::from((5_000_000_i64, Currency::EUR)))
            .quoted_spot(1.24)
            .build()
            .expect("valid FX spot")
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
                finstack_quant_valuations_core::instruments::PricingOptions::default(),
            )
            .expect("should succeed");
        // Quote amount is just the result.value (PV in quote currency)
        assert!((result.value.amount() - base_value.amount()).abs() < 1e-6);
    }

    // FxSpot intentionally does not register DV01/BucketedDv01: it has no
    // discount or forward curve dependency. FX risk is exposed through
    // FxDelta and Fx01.
}

#[cfg(test)]
mod base_amount_tests {
    use super::{base_amount, BaseAmountCalculator};
    use crate::fx_spot::FxSpot;
    use finstack_quant_core::{
        currency::Currency, dates::Date, market_data::context::MarketContext, money::Money,
        types::InstrumentId,
    };
    use finstack_quant_valuations_core::instruments::Instrument;
    use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
    use std::sync::Arc;
    use time::Month;

    fn d(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("should succeed"), day)
            .expect("should succeed")
    }

    fn sample_fx() -> FxSpot {
        FxSpot::builder()
            .id(InstrumentId::new("EURUSD"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .notional(Money::from((1_250_000_i64, Currency::EUR)))
            .quoted_spot(1.18)
            .build()
            .expect("valid FX spot")
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
        let instrument: Arc<dyn finstack_quant_valuations_core::instruments::Instrument> =
            Arc::new(fx);
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

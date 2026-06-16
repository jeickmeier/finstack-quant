use crate::instruments::rates::deposit::Deposit;
use crate::metrics::{MetricCalculator, MetricContext};
use rust_decimal::prelude::ToPrimitive;

/// Quoted rate passthrough for deposits.
///
/// Returns the quoted simple rate from the instrument.
///
/// # Errors
/// Returns an error if the deposit does not have a quoted rate set.
pub(crate) struct QuoteRateCalculator;

impl MetricCalculator for QuoteRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let deposit: &Deposit = context.instrument_as()?;
        deposit
            .quote_rate
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "QuoteRate (deposit has no quoted rate set)".to_string(),
                })
            })?
            .to_f64()
            .ok_or(finstack_quant_core::InputError::ConversionOverflow.into())
    }
}

use crate::instruments::rates::deposit::Deposit;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use rust_decimal::prelude::ToPrimitive;

/// Calculates implied DF(end) from quoted rate.
///
/// Computes DF(end) implied by the quoted simple rate using:
/// DF(end) = DF(start) / (1 + rate × year_fraction).
///
/// # Dependencies
/// Requires `DfStart` and `Yf` metrics to be computed first.
///
/// # Errors
/// Returns an error if:
/// - Quote rate is missing from the deposit
/// - Required metrics are missing
/// - Denominator (1 + rate × year_fraction) is zero or negative
pub(crate) struct DfEndFromQuoteCalculator;

impl MetricCalculator for DfEndFromQuoteCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::DfStart, MetricId::Yf]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let deposit: &Deposit = context.instrument_as()?;

        let r = deposit.quote_rate.ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "QuoteRate (required for implied DF calculation)".to_string(),
            })
        })?;
        let r = r
            .to_f64()
            .ok_or(finstack_quant_core::InputError::ConversionOverflow)?;

        let df_s = context
            .computed
            .get(&MetricId::DfStart)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "DfStart (required for implied DF calculation)".to_string(),
                })
            })?;
        let yf = context
            .computed
            .get(&MetricId::Yf)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "Yf (required for implied DF calculation)".to_string(),
                })
            })?;

        // Guard against zero or negative denominator which would produce
        // invalid discount factors (inf or negative)
        let denominator = 1.0 + r * yf;
        if denominator <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Invalid discount factor denominator: 1 + rate({:.4}) × yf({:.4}) = {:.6} must be positive",
                r, yf, denominator
            )));
        }

        Ok(df_s / denominator)
    }
}

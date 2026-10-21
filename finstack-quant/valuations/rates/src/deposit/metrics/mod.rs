//! Deposit-specific metrics module.
//!
//! Provides metric calculators for deposit instruments. Metrics include:
//! - Year fraction (instrument day-count)
//! - Discount factors at start and end dates
//! - Par (simple) rate
//! - Implied end-date discount factor from a quoted rate
//! - Quoted rate passthrough
//!
//! See unit tests and `examples/` for usage.

mod df;

mod df_end_from_quote;
mod par_rate;

pub(crate) use df_end_from_quote::DfEndFromQuoteCalculator;
pub(crate) use par_rate::DepositParRateCalculator;

use crate::deposit::Deposit;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;
use rust_decimal::prelude::ToPrimitive;

/// Registers all deposit metrics to a registry.
///
/// Each metric is registered with the "Deposit" instrument type to ensure
/// proper applicability filtering.
pub(crate) fn register_deposit_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Deposit,
        metrics: [
            (Yf, YearFractionCalculator),
            (DfStart, df::DfCalculator::START),
            (DfEnd, df::DfCalculator::END),
            (DepositParRate, DepositParRateCalculator),
            (DfEndFromQuote, DfEndFromQuoteCalculator),
            (QuoteRate, QuoteRateCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    };
    Ok(())
}

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
            .fixed_rate
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "fixed_rate (deposit has no fixed_rate set)".to_string(),
                })
            })?
            .to_f64()
            .ok_or(finstack_quant_core::InputError::ConversionOverflow.into())
    }
}

/// Calculates year fraction for deposits.
///
/// Computes the time period between effective start and end dates using the deposit's
/// day count convention.
pub(crate) struct YearFractionCalculator;

impl MetricCalculator for YearFractionCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let deposit: &Deposit = context.instrument_as()?;
        let effective_start = deposit.effective_start_date()?;
        let effective_end = deposit.effective_end_date()?;

        if effective_end <= effective_start {
            return Err(finstack_quant_core::Error::Validation(format!(
                "YearFraction: effective end date ({}) must be after effective start date ({})",
                effective_end, effective_start
            )));
        }

        deposit.day_count.year_fraction(
            effective_start,
            effective_end,
            finstack_quant_core::dates::DayCountContext::default(),
        )
    }
}

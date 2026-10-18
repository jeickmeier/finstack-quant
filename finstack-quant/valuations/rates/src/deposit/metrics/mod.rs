//! Deposit-specific metrics module.
//!
//! Provides metric calculators for deposit instruments, split into focused
//! files for clarity and parity with other instruments. Metrics include:
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
mod quote_rate;
mod year_fraction;

pub(crate) use df_end_from_quote::DfEndFromQuoteCalculator;
pub(crate) use par_rate::DepositParRateCalculator;
pub(crate) use quote_rate::QuoteRateCalculator;
pub(crate) use year_fraction::YearFractionCalculator;

use finstack_quant_valuations_core::metrics::MetricRegistry;

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
            (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    };
    Ok(())
}

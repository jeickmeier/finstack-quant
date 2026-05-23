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

mod df_end_from_quote;
mod par_rate;
mod quote_rate;
mod year_fraction;

pub(crate) use df_end_from_quote::DfEndFromQuoteCalculator;
pub(crate) use par_rate::DepositParRateCalculator;
pub(crate) use quote_rate::QuoteRateCalculator;
pub(crate) use year_fraction::YearFractionCalculator;

use crate::metrics::MetricRegistry;

/// Registers all deposit metrics to a registry.
///
/// Each metric is registered with the "Deposit" instrument type to ensure
/// proper applicability filtering.
pub(crate) fn register_deposit_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Deposit,
        metrics: [
            (Yf, YearFractionCalculator),
            (DfStart, crate::metrics::GenericDfStartCalculator),
            (DfEnd, crate::metrics::GenericDfEndCalculator),
            (DepositParRate, DepositParRateCalculator),
            (DfEndFromQuote, DfEndFromQuoteCalculator),
            (QuoteRate, QuoteRateCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::Deposit,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::Deposit,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    };
}

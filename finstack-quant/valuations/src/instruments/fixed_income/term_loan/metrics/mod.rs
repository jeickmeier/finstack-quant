//! Metrics for term loan instruments.
//!
//! This module provides risk and return metrics for term loans including:
//! - **Yield metrics**: YTM, YTC, YTW, YT2Y/3Y/4Y
//! - **Spread metrics**: Discount Margin, All-In Rate
//! - **Risk metrics**: DV01, CS01, Theta (bucketed and parallel)
//!
//! # Available Metrics
//!
//! ## Yield and Return
//!
//! - **YTM** (Yield-to-Maturity): IRR to final maturity
//! - **YTC** (Yield-to-Call): IRR to first call date
//! - **YTW** (Yield-to-Worst): Minimum yield across all call dates and maturity
//! - **YT2Y/3Y/4Y**: IRR to fixed 2/3/4-year horizons
//! - **All-In Rate**: Effective borrower cost including fees
//! - **OID EIR Amortization**: Effective interest rate amortization schedule
//!
//! ## Spread Metrics
//!
//! - **Discount Margin**: Additive spread for floating-rate loans to match price
//!
//! ## Risk Metrics
//!
//! - **DV01**: Dollar value of 1bp parallel shift (combined parallel + key-rate)
//! - **BucketedDV01**: Key-rate risk by tenor bucket
//! - **CS01**: Credit spread sensitivity (parallel)
//! - **BucketedCS01**: Credit spread sensitivity by tenor
//! - **Theta**: Time decay (1-day price change)
//!
//! # Quick Example
//!
//! ```text
//! use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
//! use finstack_quant_valuations::metrics::{MetricId, MetricRegistry};
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let loan = TermLoan::example().unwrap();
//! let market = MarketContext::new();
//! let as_of = Date::from_calendar_date(2025, Month::January, 15)?;
//!
//! // Compute base value
//! // let pv = loan.value(&market, as_of)?;
//!
//! // Request metrics
//! let metrics = vec![MetricId::Ytm, MetricId::Dv01, MetricId::custom("ytw")];
//! // let result = loan.price_with_metrics(&market, as_of, &metrics, crate::instruments::PricingOptions::default())?;
//! # Ok(())
//! # }
//! ```
//!
//! # Registration
//!
//! Metrics are registered via [`register_term_loan_metrics()`] in the global registry.
//!
//! # See Also
//!
//! - [`YtmCalculator`] for yield-to-maturity implementation
//! - [`DiscountMarginCalculator`] for floating-rate spread calculation
//! - [`AllInRateCalculator`] for borrower cost calculation

mod all_in_rate;
mod cs01;
mod discount_margin;
mod embedded_option_value;
pub(crate) mod irr_helpers;
mod oas;
mod oid_eir;
mod ytc;
mod ytm;
mod ytn;
mod ytw;

pub(crate) use all_in_rate::AllInRateCalculator;
pub(crate) use discount_margin::DiscountMarginCalculator;
pub(crate) use embedded_option_value::EmbeddedOptionValueCalculator;
pub(crate) use oas::OasCalculator;
pub(crate) use oid_eir::OidEirAmortizationCalculator;
pub(crate) use ytc::YtcCalculator;
pub(crate) use ytm::YtmCalculator;
pub(crate) use ytn::{Yt2yCalculator, Yt3yCalculator, Yt4yCalculator};
pub(crate) use ytw::YtwCalculator;

use crate::metrics::MetricRegistry;

/// Register all term loan metrics with the global registry.
///
/// Registers yield, spread, and risk metrics for term loan instruments.
/// Called automatically during registry initialization.
///
/// # Registered Metrics
///
/// - Standard: YTM, Discount Margin, DV01, BucketedDV01, CS01, BucketedCS01, Theta
/// - Custom: YTW (`"ytw"`), YTC (`"ytc"`), YT2Y/3Y/4Y, All-In Rate (`"all_in_rate"`),
///   OID EIR amortization (`"oid_eir_amortization"`)
///
/// # Examples
///
/// ```text
/// use finstack_quant_valuations::metrics::MetricRegistry;
/// use finstack_quant_valuations::instruments::fixed_income::term_loan::metrics::register_term_loan_metrics;
///
/// let mut registry = MetricRegistry::new();
/// register_term_loan_metrics(&mut registry);
/// ```
pub(crate) fn register_term_loan_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::TermLoan,
        metrics: [
            (Ytw, YtwCalculator),

            // Theta is now registered universally in metrics::standard_registry()

            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::TermLoan,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::TermLoan,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),

            // CS01 follows the active valuation model. The default credit-tree
            // path consumes the hazard curve and therefore delegates to the
            // canonical hazard rebootstrap. Explicit deterministic discounting
            // uses the market-standard z-spread fallback instead.
            (Cs01, crate::metrics::ZSpreadParallelCs01::<
                crate::instruments::TermLoan,
            >::hazard_when_credit_curve_and_model()),
            (BucketedCs01, crate::metrics::ZSpreadBucketedCs01::<
                crate::instruments::TermLoan,
            >::hazard_when_credit_curve_and_model()),
            (Cs01Hazard, crate::metrics::GenericParallelCs01Hazard::<
                crate::instruments::TermLoan,
            >::with_empty_credit_curve_zero()),
            (BucketedCs01Hazard, crate::metrics::GenericBucketedCs01Hazard::<
                crate::instruments::TermLoan,
            >::with_empty_credit_curve_zero()),
        ]
    }

    // Loan-specific metrics
    use crate::metrics::MetricId;
    use std::sync::Arc;

    registry.register_metric(
        MetricId::custom("all_in_rate"),
        Arc::new(AllInRateCalculator),
        &[InstrumentType::TermLoan],
    );
    registry.register_metric(
        MetricId::custom("oid_eir_amortization"),
        Arc::new(OidEirAmortizationCalculator),
        &[InstrumentType::TermLoan],
    );
    registry.register_metric(
        MetricId::Ytm,
        Arc::new(YtmCalculator),
        &[InstrumentType::TermLoan],
    );
    registry.register_metric(
        MetricId::DiscountMargin,
        Arc::new(DiscountMarginCalculator),
        &[InstrumentType::TermLoan],
    );

    // Callable-tree metrics
    registry.register_metric(
        MetricId::Oas,
        Arc::new(OasCalculator),
        &[InstrumentType::TermLoan],
    );
    registry.register_metric(
        MetricId::EmbeddedOptionValue,
        Arc::new(EmbeddedOptionValueCalculator),
        &[InstrumentType::TermLoan],
    );

    // Yield to first call (custom id: ytc)
    registry.register_metric(
        MetricId::custom("ytc"),
        Arc::new(YtcCalculator),
        &[InstrumentType::TermLoan],
    );

    // Yields to fixed horizons
    registry.register_metric(
        MetricId::custom("yt2y"),
        Arc::new(Yt2yCalculator),
        &[InstrumentType::TermLoan],
    );
    registry.register_metric(
        MetricId::custom("yt3y"),
        Arc::new(Yt3yCalculator),
        &[InstrumentType::TermLoan],
    );
    registry.register_metric(
        MetricId::custom("yt4y"),
        Arc::new(Yt4yCalculator),
        &[InstrumentType::TermLoan],
    );
}

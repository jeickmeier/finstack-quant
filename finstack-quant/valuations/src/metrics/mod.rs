//! Metrics framework for clean separation of pricing and financial measures.
//!
//! This module provides a trait-based architecture for computing financial
//! metrics independently from core pricing logic. Metrics can be computed
//! on-demand, have dependencies, and are cached for efficiency.
//!
//! # Metric Contract
//!
//! Metric values are returned as raw `f64` values, but they are not unitless.
//! The authoritative semantic contract for each measure lives on
//! [`crate::metrics::MetricId`],
//! including:
//!
//! - units such as currency, currency per bp, decimal rate, or vol point
//! - bump conventions such as per-1bp or per-1 vol point
//! - sign conventions for long-holder, payer/receiver, or spot-up interpretations
//! - distinctions between similarly named measures such as `Dv01`, `Pv01`,
//!   `YieldDv01`, and `Cs01Hazard`
//!
//! Consumers should interpret values from
//! [`crate::results::ValuationResult::measures`] through
//! [`crate::metrics::MetricId`] rather than assuming every `f64` is a currency
//! amount.
//!
//! # Key Features
//!
//! - **Trait-based design**: `MetricCalculator` trait for custom metric implementations
//! - **Dependency management**: Automatic computation ordering based on metric dependencies
//! - **Caching**: Built-in caching of intermediate results like cashflows and discount factors
//! - **Instrument-specific**: Metrics can be registered for specific instrument types
//! - **Standard registry**: Pre-configured registry with common financial metrics
//!
//! # Quick Start Examples
//!
//! ## Example 1: Computing Bucketed DV01 for a Bond
//!
//! ```ignore
//! use finstack_quant_valuations::instruments::{Bond, Instrument, PricingOptions};
//! use finstack_quant_valuations::metrics::MetricId;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use time::macros::date;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! // Setup: create an example bond and an (empty) market context.
//! // Note: real runs require a populated market context with required curves.
//! let as_of = date!(2025-01-01);
//! let bond = Bond::example().unwrap();
//! let market = MarketContext::new();
//! let metrics = vec![MetricId::BucketedDv01];
//!
//! // Price with metrics
//! let result = bond.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;
//!
//! // Access results
//! let pv = result.value.amount();
//! println!("Bond PV: ${:.2}", pv);
//!
//! // Get total DV01 (scalar)
//! if let Some(total_dv01) = result.measures.get(MetricId::BucketedDv01.as_str()) {
//!     println!("Total DV01: ${:.2} per bp", total_dv01);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Example 2: Computing Parallel DV01 for an Interest Rate Swap
//!
//! ```ignore
//! use finstack_quant_valuations::instruments::{Instrument, InterestRateSwap, PricingOptions};
//! use finstack_quant_valuations::metrics::MetricId;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use time::macros::date;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let as_of = date!(2025-01-01);
//! let swap = InterestRateSwap::example_standard()?;
//! let market = MarketContext::new();
//! let metrics = vec![MetricId::Dv01]; // Parallel DV01
//!
//! let result = swap.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;
//!
//! if let Some(dv01) = result.measures.get(MetricId::Dv01.as_str()) {
//!     println!("Swap DV01: ${:.2} per bp", dv01);
//!     // Negative DV01 means swap loses value when rates rise
//!     // (typical for receiver swaps)
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Example 3: Computing Theta (Time Decay) for an Option
//!
//! ```ignore
//! use finstack_quant_valuations::instruments::{EquityOption, Instrument, PricingOptions};
//! use finstack_quant_valuations::metrics::MetricId;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::create_date;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::money::Money;
//! use time::Month;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let as_of = create_date(2024, Month::January, 1)?;
//! let expiry = create_date(2024, Month::July, 1)?; // 6-month option
//!
//! let option = EquityOption::european_call(
//!     "OPT-001",
//!     "SPX",
//!     4500.0,
//!     expiry,
//!     Money::new(100.0, Currency::USD),
//! )?;
//! let market = MarketContext::new();
//! let metrics = vec![MetricId::Theta];
//!
//! let result = option.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;
//!
//! if let Some(theta) = result.measures.get(MetricId::Theta.as_str()) {
//!     println!("Option theta per day: ${:.2}", theta);
//!     // Negative theta = option loses value over time (time decay)
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Example 4: Computing Multiple Greeks for an Option
//!
//! ```ignore
//! use finstack_quant_valuations::instruments::{EquityOption, Instrument, PricingOptions};
//! use finstack_quant_valuations::metrics::MetricId;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::create_date;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::money::Money;
//! use time::Month;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let as_of = create_date(2024, Month::January, 1)?;
//! let option = EquityOption::european_call(
//!     "OPT-001",
//!     "SPX",
//!     4500.0,
//!     create_date(2024, Month::July, 1)?,
//!     Money::new(100.0, Currency::USD),
//! )?;
//! let market = MarketContext::new();
//! let metrics = vec![
//!     MetricId::Delta,
//!     MetricId::Gamma,
//!     MetricId::Vega,
//!     MetricId::Theta,
//!     MetricId::Rho,
//! ];
//!
//! let result = option.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;
//!
//! println!("Option Greeks:");
//! println!("  PV:    ${:.2}", result.value.amount());
//! println!(
//!     "  Delta: {:.4}",
//!     result.measures.get(MetricId::Delta.as_str()).unwrap_or(&0.0)
//! );
//! println!(
//!     "  Gamma: {:.4}",
//!     result.measures.get(MetricId::Gamma.as_str()).unwrap_or(&0.0)
//! );
//! println!(
//!     "  Vega:  {:.4}",
//!     result.measures.get(MetricId::Vega.as_str()).unwrap_or(&0.0)
//! );
//! println!(
//!     "  Theta: {:.4}",
//!     result.measures.get(MetricId::Theta.as_str()).unwrap_or(&0.0)
//! );
//! println!(
//!     "  Rho:   {:.4}",
//!     result.measures.get(MetricId::Rho.as_str()).unwrap_or(&0.0)
//! );
//! # Ok(())
//! # }
//! ```
//!
//! # Architecture
//!
//! - **`MetricId`**: Strongly-typed identifiers for all metrics
//! - **`MetricCalculator`**: Trait for implementing custom metrics
//! - **`MetricContext`**: Context containing instrument, market data, and cached results
//! - **`MetricRegistry`**: Registry for managing calculators and dependencies
//! - **Risk metrics**: Specialized calculators for DV01, bucketed risk, and time decay
//!
//! # References
//!
//! - Fixed-income risk measures: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
//! - Curve construction and sensitivities: `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
//! - VaR and Expected Shortfall interpretation: `docs/REFERENCES.md#mcneil-frey-embrechts-qrm`
//!

// Internal submodules (organized by concern)

mod core;
pub mod risk;
pub(crate) mod sensitivities;
mod shared;

// Core surface (supported)
pub use core::finite_difference::bump_surface_vol_absolute;
pub use core::ids::{MetricGroup, MetricId};
pub use core::registry::MetricRegistry;
pub use core::standard_registry::standard_registry;
pub use core::traits::{MetricCalculator, MetricContext, Structured2D};
/// Format a standard risk bucket (years) as a human-readable label.
pub use sensitivities::config::{
    format_bucket_label, STANDARD_BUCKETS_YEARS, STANDARD_BUCKET_LABELS,
};
pub use sensitivities::cross_factor::{CrossFactorCalculator, CrossFactorPair};
pub use sensitivities::theta::collect_cashflows_in_period;

// -----------------------------------------------------------------------------
// Crate-internal re-exports (NOT part of the public API)
// -----------------------------------------------------------------------------
//
// These are used across `finstack-quant-valuations` (instrument metric registries, etc.) but are not
// supported as a stable downstream API. Keep them `pub(crate)` so we can refactor module layout
// without creating public breakage surface.
pub(crate) use core::finite_difference::{
    bump_discount_curve_parallel, bump_scalar_price, bump_sizes, central_diff_by_half_bump,
    central_diff_by_width, central_diff_scalar_relative, replace_scalar_value,
    scalar_numeric_value, scaled_central_diff_by_width, VOL_POINTS_PER_ABSOLUTE_VOL,
};
pub(crate) use sensitivities::config::from_finstack_config_or_default as resolve_sensitivities_config;
pub(crate) use sensitivities::cross_factor::{
    make_credit_bumper, make_fx_bumper, make_rates_bumper, make_spot_bumper, make_vol_bumper,
};
pub(crate) use sensitivities::cs01::{
    GenericBucketedCs01, GenericBucketedCs01Hazard, GenericParallelCs01, GenericParallelCs01Hazard,
};
pub(crate) use sensitivities::cs01_z_spread::{
    ZSpreadBucketedCs01, ZSpreadCs01, ZSpreadCs01Inputs, ZSpreadParallelCs01,
};
pub(crate) use sensitivities::dv01::{Dv01CalculatorConfig, UnifiedDv01Calculator};
pub(crate) use sensitivities::fd_greeks::{
    GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVega, GenericFdVolga, HasDayCount,
    HasExpiry,
};
pub(crate) use sensitivities::option_greeks::OptionGreekCalculator;
pub(crate) use sensitivities::rf_component_dv01::{
    RfComponentDv01Calculator, RfComponentPriced, RfDv01Mode,
};
pub(crate) use sensitivities::theta::calculate_theta_date;
pub(crate) use sensitivities::vega::KeyRateVega;
pub(crate) use shared::df::{GenericDfEndCalculator, GenericDfStartCalculator};

// -----------------------------------------------------------------------------
// Macros
// -----------------------------------------------------------------------------

/// Define a trivial metric calculator that delegates to an instrument method or closure.
#[macro_export]
macro_rules! define_metric_calculator {
    (
        $(#[$meta:meta])*
        $name:ident,
        instrument = $instrument:ty,
        calc = |$inst:ident, $ctx:ident| $body:expr
        $(, deps = [$($dep:expr),* $(,)?])?
    ) => {
        $(#[$meta])*
        pub(crate) struct $name;

        impl $crate::metrics::MetricCalculator for $name {
            fn calculate(
                &self,
                $ctx: &mut $crate::metrics::MetricContext,
            ) -> finstack_quant_core::Result<f64> {
                let $inst: &$instrument = $ctx.instrument_as()?;
                let value: finstack_quant_core::Result<f64> = { $body };
                value
            }

            fn dependencies(&self) -> &[$crate::metrics::MetricId] {
                static DEPS: &[$crate::metrics::MetricId] = &[$($($dep),*)?];
                DEPS
            }
        }
    };
}

// -----------------------------------------------------------------------------
// Error helper functions
// -----------------------------------------------------------------------------

/// Create a NotFound error for missing metrics.
///
/// Use this when a metric dependency or required metric is not available.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::metrics::{metric_not_found, MetricId};
/// use finstack_quant_core::Result;
/// use finstack_quant_core::HashMap;
///
/// fn get_metric(id: MetricId, results: &HashMap<MetricId, f64>) -> Result<f64> {
///     results.get(&id).copied().ok_or_else(|| metric_not_found(id))
/// }
/// ```
/// Create a typed NotFound error for a missing metric.
///
/// # Arguments
///
/// * `metric` - Required metric identifier that was absent from a valuation
///   result or dependency set; it is embedded in the returned `metric:` ID.
#[inline]
pub fn metric_not_found(metric: MetricId) -> finstack_quant_core::Error {
    finstack_quant_core::InputError::NotFound {
        id: format!("metric:{metric:?}"),
    }
    .into()
}

/// Create a NotFound error for missing context fields.
///
/// Use this when a required field is not present in a context or configuration.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::metrics::context_not_found;
/// use finstack_quant_core::types::CurveId;
/// use finstack_quant_core::Result;
///
/// struct PricingContext {
///     discount_curve_id: Option<CurveId>,
/// }
///
/// impl PricingContext {
///     fn discount_curve_id(&self) -> Option<&CurveId> {
///         self.discount_curve_id.as_ref()
///     }
/// }
///
/// fn get_curve_id(context: &PricingContext) -> Result<&CurveId> {
///     context
///         .discount_curve_id()
///         .ok_or_else(|| context_not_found("discount_curve_id"))
/// }
/// ```
///
/// # Arguments
///
/// * `field` - Required context field name that was absent; it is embedded in
///   the returned `context.`-prefixed not-found identifier.
#[inline]
pub fn context_not_found(field: &str) -> finstack_quant_core::Error {
    finstack_quant_core::InputError::NotFound {
        id: format!("context.{field}"),
    }
    .into()
}

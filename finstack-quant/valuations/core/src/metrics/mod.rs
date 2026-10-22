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
//!   `YieldDv01`, and `Cs01`
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
//! - **Universal registry**: every family registry starts from the universal calculators
//!
//! # Quick Start
//!
//! The umbrella crate's `metrics` module documentation shows bucketed DV01 on a
//! bond, DV01 on a seasoned swap and the Greeks of an equity option; every
//! example calls `Instrument::price_with_metrics` with the metric ids defined here.
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

mod aggregation;
mod buckets;
mod cross_factor_pair;
pub(crate) mod finite_difference;
pub(crate) mod ids;
pub(crate) mod period_cash;
pub(crate) mod registration_macro;
pub(crate) mod registry;
pub mod risk;
#[doc(hidden)]
pub mod sensitivities;
pub(crate) mod traits;
pub(crate) mod universal;

// Core surface (supported)
pub use aggregation::is_additive_metric;
pub use buckets::{
    format_bucket_label, format_key_rate_label, parse_key_rate_label, STANDARD_BUCKETS_YEARS,
    STANDARD_BUCKET_LABELS,
};
pub use cross_factor_pair::CrossFactorPair;
pub use finite_difference::bump_surface_vol_absolute;
#[doc(hidden)]
pub use ids::closest_metric_names;
pub use ids::{MetricGroup, MetricId, MetricUnit};
pub use period_cash::{collect_period_cash, PeriodCash};
pub use registry::{MetricRegistry, MetricRegistryError};
pub use traits::{MetricCalculator, MetricContext, MetricSeed};
#[doc(hidden)]
pub use universal::{register_universal_metrics, PricerPublishedMetric};

// Family-facing finite-difference helpers and calculators.
//
// Instrument families compose these; they are not a supported downstream API
// and stay out of the rendered documentation. The remaining family-facing
// calculators live at `metrics::sensitivities::<module>`.
#[doc(hidden)]
pub use finite_difference::{
    bump_active_volatility, bump_discount_curve_parallel, bump_scalar_price,
    central_diff_by_half_bump, central_diff_by_width, central_diff_scalar_relative,
    replace_scalar_value, scalar_numeric_value, scaled_central_diff_by_width, CORRELATION_BUMP,
    VOL_POINTS_PER_ABSOLUTE_VOL,
};
#[doc(hidden)]
pub use sensitivities::cs01_z_spread::{
    quoted_z_spread, ZSpreadBucketedCs01, ZSpreadCs01, ZSpreadCs01Inputs, ZSpreadParallelCs01,
};
#[doc(hidden)]
pub use sensitivities::rf_component_dv01::{RfComponentDv01Calculator, RfComponentPriced};
#[doc(hidden)]
pub use sensitivities::vega::KeyRateVega;

// Error helper functions

/// Create a typed NotFound error for a missing metric.
///
/// Use this when a metric dependency or required metric is not available.
///
/// # Arguments
///
/// * `metric` - Required metric identifier that was absent from a valuation
///   result or dependency set; it is embedded in the returned `metric:` ID.
///
/// # Examples
///
/// ```
/// use finstack_quant_valuations_core::metrics::{metric_not_found, MetricId};
/// use finstack_quant_core::Result;
/// use finstack_quant_core::FxHashMap;
///
/// fn get_metric(id: MetricId, results: &FxHashMap<MetricId, f64>) -> Result<f64> {
///     results.get(&id).copied().ok_or_else(|| metric_not_found(id))
/// }
/// ```
#[inline]
pub fn metric_not_found(metric: MetricId) -> finstack_quant_core::Error {
    finstack_quant_core::InputError::NotFound {
        id: format!("metric:{metric}"),
    }
    .into()
}

/// Create a NotFound error for missing context fields.
///
/// Use this when a required field is not present in a context or configuration.
///
/// # Examples
///
/// ```
/// use finstack_quant_valuations_core::metrics::context_not_found;
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

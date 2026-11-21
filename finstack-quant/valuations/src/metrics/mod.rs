//! Metrics framework, registry, cross-instrument aggregation, and Historical VaR.

mod aggregation;
pub mod risk;

pub use aggregation::is_additive_metric;
pub use finstack_quant_valuations_core::metrics::returns;
#[doc(hidden)]
pub use finstack_quant_valuations_core::metrics::sensitivities;
#[doc(hidden)]
pub use finstack_quant_valuations_core::metrics::{
    CORRELATION_BUMP, PricerPublishedMetric, VOL_POINTS_PER_ABSOLUTE_VOL, active_minimum_vol,
    apply_parallel_surface_bumps_in_place, bump_active_volatility, bump_discount_curve_parallel,
    bump_scalar_price, bumped_volatility_override_dyn, central_diff_by_width,
    central_diff_scalar_relative, clone_with_crn_seed_if_supported, closest_metric_names,
    min_grid_vol, register_universal_metrics, replace_scalar_value, revert_scratch_bumps,
    scalar_numeric_value, scaled_central_diff_by_width, sensitivity_central_diff,
    volatility_override,
};
pub use finstack_quant_valuations_core::metrics::{
    MetricAggregation, MetricCalculator, MetricContext, MetricGroup, MetricId, MetricRegistry,
    MetricRegistryError, MetricSeed, MetricUnit, PeriodCash, STANDARD_BUCKET_LABELS,
    STANDARD_BUCKETS_YEARS, bump_surface_vol_absolute, collect_period_cash, format_bucket_label,
    format_key_rate_label, metric_not_found, parse_key_rate_label,
    validate_buckets_strictly_increasing,
};

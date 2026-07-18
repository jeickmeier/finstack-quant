//! Pricing options implementation used by the traits subsystem.
//!
// =============================================================================
// Pricing Options
// =============================================================================

use crate::metrics::risk::MarketHistory;
use crate::metrics::MetricRegistry;
use crate::pricer::{shared_standard_registry, ModelKey, PricerRegistry};
use finstack_quant_core::config::FinstackConfig;
use std::sync::Arc;

/// Optional overrides for a pricing-and-metrics request.
///
/// This struct consolidates optional parameters for `Instrument::price_with_metrics`,
/// replacing the proliferation of `_with_config`, `_with_market_history` variants.
///
/// # Examples
///
/// ## Basic usage (no options)
///
/// ```ignore
/// let result = instrument.price_with_metrics(
///     &market,
///     as_of,
///     &metrics,
///     PricingOptions::default(),
/// )?;
/// ```
///
/// ## With custom config
///
/// ```ignore
/// let opts = PricingOptions::default().with_config(&my_config);
/// let result = instrument.price_with_metrics(&market, as_of, &metrics, opts)?;
/// ```
///
/// ## With market history for VaR
///
/// ```ignore
/// let opts = PricingOptions::default().with_market_history(history);
/// let result = instrument.price_with_metrics(&market, as_of, &metrics, opts)?;
/// ```
#[derive(Clone, Default)]
pub struct PricingOptions {
    /// Optional configuration for metric computation (bump sizes, tolerances, etc.)
    pub config: Option<Arc<FinstackConfig>>,
    /// Optional market history for Historical VaR / Expected Shortfall metrics
    pub market_history: Option<Arc<MarketHistory>>,
    /// Optional explicit pricing model override.
    ///
    /// When `None`, [`super::Instrument::price_with_metrics`] uses
    /// [`super::Instrument::default_model`]. Set this to select a different registered
    /// pricing path, such as hazard-rate or tree/OAS pricing, without dropping
    /// down to [`crate::pricer::PricerRegistry`] directly.
    pub model: Option<ModelKey>,
    /// Optional explicit pricer registry override.
    pub registry: Option<Arc<PricerRegistry>>,
    /// Batch-local cache shared by finite-difference credit-risk calculations.
    pub(crate) hazard_recalibration_cache:
        Option<Arc<crate::calibration::bumps::hazard::HazardRecalibrationCache>>,
}

impl PricingOptions {
    /// Create new pricing options with no extras.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the configuration for metric computation.
    ///
    /// The config controls sensitivity bump sizes and other calculation parameters.
    pub fn with_config(mut self, cfg: &FinstackConfig) -> Self {
        self.config = Some(Arc::new(cfg.clone()));
        self
    }

    /// Set the market history for Historical VaR / Expected Shortfall.
    ///
    /// Required for computing `MetricId::HVar` and `MetricId::ExpectedShortfall`.
    pub fn with_market_history(mut self, history: Arc<MarketHistory>) -> Self {
        self.market_history = Some(history);
        self
    }

    /// Set the pricing model for this pricing request.
    ///
    /// Most callers can stay on [`super::Instrument::price_with_metrics`] and use this
    /// override only when they need a non-default registered model.
    pub fn with_model(mut self, model: ModelKey) -> Self {
        self.model = Some(model);
        self
    }

    /// Set an explicit pricer registry override for this pricing request.
    pub fn with_registry(mut self, registry: Arc<PricerRegistry>) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Attach a new hazard-recalibration cache for one immutable pricing batch.
    ///
    /// Clones of the returned options share the cache. Callers must create a
    /// fresh cache when the market snapshot changes.
    pub fn with_new_hazard_recalibration_cache(mut self) -> Self {
        self.hazard_recalibration_cache = Some(Arc::new(
            crate::calibration::bumps::hazard::HazardRecalibrationCache::default(),
        ));
        self
    }

    /// Set an explicit metric registry for this pricing request.
    ///
    /// The metric registry is attached to the selected pricer registry so the
    /// existing [`Self::registry`] field remains the single dispatch bundle.
    /// Call [`Self::with_registry`] first when overriding both registries; a
    /// later `with_registry` call replaces the complete registry selection.
    pub fn with_metric_registry(mut self, metric_registry: Arc<MetricRegistry>) -> Self {
        let registry = self
            .registry
            .as_deref()
            .cloned()
            .unwrap_or_else(|| shared_standard_registry().as_ref().clone())
            .with_metric_registry(metric_registry);
        self.registry = Some(Arc::new(registry));
        self
    }
}

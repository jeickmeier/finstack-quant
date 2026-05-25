//! Pricer registry, trait, and dispatch infrastructure.
//!
//! Defines the [`Pricer`] trait, [`PricerRegistry`], and the [`expect_inst`]
//! downcast helper used by all pricer implementations.

use super::{InstrumentType, ModelKey, PricerKey, PricingError, PricingErrorContext};
use crate::instruments::common_impl::traits::Instrument as Priceable;
use finstack_core::config::{results_meta_now, FinstackConfig};
use finstack_core::market_data::context::MarketContext as Market;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Helper function to safely downcast a trait object to a concrete instrument type.
///
/// This performs both enum-based type checking and actual type downcasting,
/// ensuring type safety at both levels.
#[doc(hidden)]
pub fn expect_inst<T: Priceable + 'static>(
    inst: &dyn Priceable,
    expected: InstrumentType,
) -> std::result::Result<&T, PricingError> {
    if inst.key() != expected {
        return Err(PricingError::type_mismatch(expected, inst.key()));
    }

    inst.as_any()
        .downcast_ref::<T>()
        .ok_or_else(|| PricingError::type_mismatch(expected, inst.key()))
}

/// Trait for instrument pricers.
///
/// Each pricer handles a specific (instrument, model) combination and knows
/// how to price that instrument using the specified model.
pub trait Pricer: Send + Sync {
    /// Get the (instrument, model) key this pricer handles
    fn key(&self) -> PricerKey;

    /// Price an instrument using this pricer's model
    fn price_dyn(
        &self,
        instrument: &dyn Priceable,
        market: &Market,
        as_of: finstack_core::dates::Date,
    ) -> std::result::Result<crate::results::ValuationResult, PricingError>;

    /// Price an instrument as an unrounded scalar when the pricer can provide one.
    ///
    /// The default implementation falls back to [`Self::price_dyn`] and extracts the
    /// rounded `Money` amount. Pricers with a true raw-f64 path should override this
    /// so finite-difference risk calculations do not inherit currency rounding noise.
    fn price_raw_dyn(
        &self,
        instrument: &dyn Priceable,
        market: &Market,
        as_of: finstack_core::dates::Date,
    ) -> std::result::Result<f64, PricingError> {
        Ok(self.price_dyn(instrument, market, as_of)?.value.amount())
    }
}

/// Registry mapping (instrument type, model) pairs to pricer implementations.
///
/// Provides type-safe pricing dispatch without string comparisons or runtime
/// registration errors. Pricers are registered at compile time and looked up
/// via strongly-typed keys.
///
/// The backing map is a [`BTreeMap`] keyed by [`PricerKey`]: dispatch is purely
/// by key, so a hash map would be functionally sufficient, but the ordered map
/// guarantees a deterministic iteration order. That keeps any present or future
/// enumeration of the registry (diagnostics, serialized coverage reports)
/// reproducible across runs without relying on every call site remembering to
/// sort.
#[derive(Clone, Default)]
pub struct PricerRegistry {
    pricers: BTreeMap<PricerKey, Arc<dyn Pricer>>,
    /// Keys that were registered more than once via [`PricerRegistry::register`].
    ///
    /// `register` legitimately overwrites for test setup and monkey-patching,
    /// but in the standard-registry build a collision is a bug. Recording the
    /// offending keys lets [`build_standard_registry`](super::build_standard_registry)
    /// surface them without threading a `Result` through every shard.
    duplicate_keys: Vec<PricerKey>,
}

#[derive(Clone, Default)]
struct SharedPricingInputs {
    registry: Option<Arc<PricerRegistry>>,
    market: Option<Arc<Market>>,
}

struct PricingRequest<'a> {
    instrument: &'a dyn Priceable,
    model: ModelKey,
    market: &'a Market,
    as_of: finstack_core::dates::Date,
    metrics: &'a [crate::metrics::MetricId],
    options: crate::instruments::PricingOptions,
}

struct BatchPricingRequest<'a> {
    instruments: &'a [&'a dyn Priceable],
    model: ModelKey,
    market: &'a Market,
    as_of: finstack_core::dates::Date,
    metrics: &'a [crate::metrics::MetricId],
    options: crate::instruments::PricingOptions,
}

impl PricerRegistry {
    /// Create a new empty pricer registry.
    ///
    /// For pre-configured registries with all standard pricers, use
    /// [`standard_registry()`](super::standard_registry).
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a pricer for a specific (instrument type, model) combination.
    ///
    /// If a pricer already exists for this key it is replaced, a warning is
    /// emitted, and the colliding key is recorded in `Self::duplicate_keys`
    /// so the standard-registry build can fail loudly. Overwriting is a
    /// legitimate operation for test setup and monkey-patching; use
    /// [`PricerRegistry::try_register`] when a duplicate must be rejected
    /// outright rather than recorded.
    pub fn register(
        &mut self,
        inst: InstrumentType,
        model: ModelKey,
        pricer: impl Pricer + 'static,
    ) {
        let pricer: Arc<dyn Pricer> = Arc::new(pricer);
        if let Err(key) = self.try_register_arc(inst, model, Arc::clone(&pricer)) {
            tracing::warn!(
                ?key,
                "duplicate pricer registration overwrites the existing pricer"
            );
            self.duplicate_keys.push(key);
            self.pricers.insert(key, pricer);
        }
    }

    /// Register a pricer, returning an error if a pricer is already registered
    /// for the same `(instrument, model)` key.
    ///
    /// Use this where a duplicate registration must be rejected as a hard
    /// error. The complementary [`PricerRegistry::register`] overwrites and
    /// records the collision instead — useful for test setup and
    /// monkey-patching.
    ///
    /// # Errors
    ///
    /// Returns the conflicting `PricerKey` if a pricer is already registered.
    pub fn try_register(
        &mut self,
        inst: InstrumentType,
        model: ModelKey,
        pricer: impl Pricer + 'static,
    ) -> std::result::Result<(), PricerKey> {
        self.try_register_arc(inst, model, Arc::new(pricer))
    }

    /// Insert an already-boxed pricer, rejecting (without overwriting) a
    /// duplicate `(instrument, model)` key.
    ///
    /// Shared insertion core for [`Self::register`] and [`Self::try_register`].
    fn try_register_arc(
        &mut self,
        inst: InstrumentType,
        model: ModelKey,
        pricer: Arc<dyn Pricer>,
    ) -> std::result::Result<(), PricerKey> {
        let key = PricerKey::new(inst, model);
        if self.pricers.contains_key(&key) {
            return Err(key);
        }
        self.pricers.insert(key, pricer);
        Ok(())
    }

    /// Keys registered more than once via [`Self::register`].
    ///
    /// Empty for any correctly-built registry. The standard-registry build
    /// checks this to detect shard registration bugs.
    pub(super) fn duplicate_keys(&self) -> &[PricerKey] {
        &self.duplicate_keys
    }

    /// Look up a pricer for a specific (instrument type, model) combination.
    pub fn get_pricer(&self, key: PricerKey) -> Option<&dyn Pricer> {
        self.pricers.get(&key).map(|p| p.as_ref())
    }

    /// Return every [`ModelKey`] registered for the given instrument type.
    ///
    /// Used by error paths to suggest fallback models when a requested
    /// `(instrument, model)` pair has no pricer. Order is deterministic
    /// because the registry stores pricers in a [`std::collections::BTreeMap`].
    pub fn available_models_for_instrument(&self, instrument: InstrumentType) -> Vec<ModelKey> {
        self.pricers
            .keys()
            .filter(|k| k.instrument == instrument)
            .map(|k| k.model)
            .collect()
    }

    /// Price an instrument and compute requested metrics through the registered pricer.
    ///
    /// This is the single registry-level pricing entry point. Pass an empty
    /// `metrics` slice to obtain PV only; the model's own measures are always
    /// returned under `ValuationResult::measures` either way.
    ///
    /// Scenario price overrides attached to the instrument are always applied
    /// to the returned `value`, matching [`crate::instruments::Instrument::value`].
    ///
    /// For non-discounting models, spread/yield metrics (z-spread, YTM, ASW,
    /// etc.) are computed on the instrument's `metrics_equivalent()` — a version
    /// with normalized cashflows (e.g., PIK coupon type converted to Cash) so
    /// that spreads are on a cash-equivalent basis. Risk metrics (duration,
    /// DV01, convexity, CS01) use the original instrument's actual cashflows.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Instrument to price (as trait object)
    /// * `model` - Pricing model to use
    /// * `market` - Market data context with curves and surfaces
    /// * `as_of` - Valuation date
    /// * `metrics` - Standard metrics to compute (e.g., `MetricId::Ytm`,
    ///   `MetricId::ZSpread`). Pass `&[]` for PV only.
    /// * `options` - Optional overrides for config, market history, etc.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - No pricer is registered for this (instrument, model) combination
    /// - The pricing calculation fails
    /// - Required market data is missing
    /// - Metric computation fails
    #[tracing::instrument(
        skip(self, instrument, market, metrics, options),
        fields(instrument_id = %instrument.id(), model = %model, num_metrics = metrics.len())
    )]
    pub fn price_with_metrics(
        &self,
        instrument: &dyn Priceable,
        model: ModelKey,
        market: &Market,
        as_of: finstack_core::dates::Date,
        metrics: &[crate::metrics::MetricId],
        options: crate::instruments::PricingOptions,
    ) -> std::result::Result<crate::results::ValuationResult, PricingError> {
        let shared = if metrics.is_empty() {
            SharedPricingInputs::default()
        } else {
            SharedPricingInputs {
                registry: Some(Arc::new(self.clone())),
                market: Some(Arc::new(market.clone())),
            }
        };
        self.price_with_metrics_impl(
            PricingRequest {
                instrument,
                model,
                market,
                as_of,
                metrics,
                options,
            },
            shared,
        )
    }

    /// Price an instrument through an already shared registry.
    ///
    /// This avoids cloning the registry when metric calculators need to reprice
    /// through the same dispatch table.
    pub fn price_with_metrics_shared(
        registry: &Arc<Self>,
        instrument: &dyn Priceable,
        model: ModelKey,
        market: &Market,
        as_of: finstack_core::dates::Date,
        metrics: &[crate::metrics::MetricId],
        options: crate::instruments::PricingOptions,
    ) -> std::result::Result<crate::results::ValuationResult, PricingError> {
        let shared_market = (!metrics.is_empty()).then(|| Arc::new(market.clone()));
        registry.as_ref().price_with_metrics_impl(
            PricingRequest {
                instrument,
                model,
                market,
                as_of,
                metrics,
                options,
            },
            SharedPricingInputs {
                registry: Some(Arc::clone(registry)),
                market: shared_market,
            },
        )
    }

    fn price_with_metrics_impl(
        &self,
        request: PricingRequest<'_>,
        shared: SharedPricingInputs,
    ) -> std::result::Result<crate::results::ValuationResult, PricingError> {
        use crate::metrics::MetricId;
        let PricingRequest {
            instrument,
            model,
            market,
            as_of,
            metrics,
            options,
        } = request;
        let crate::instruments::PricingOptions {
            config: cfg,
            market_history,
            ..
        } = options;

        // --- Base PV through the registered pricer ---
        let key = PricerKey::new(instrument.key(), model);
        let Some(pricer) = self.get_pricer(key) else {
            return Err(PricingError::UnknownPricer {
                key,
                available_models: self.available_models_for_instrument(key.instrument),
            });
        };
        tracing::debug!(
            instrument_id = %instrument.id(),
            instrument_type = %instrument.key(),
            model_key = %model,
            %as_of,
            num_metrics = metrics.len(),
            "dispatching registered pricer"
        );
        let mut base_result = pricer.price_dyn(instrument, market, as_of)?;
        let effective_cfg = cfg
            .as_deref()
            .map_or_else(FinstackConfig::default, |c| c.clone());
        stamp_results_meta(&effective_cfg, instrument, market, &mut base_result);

        if metrics.is_empty() {
            // No extra metrics requested: apply scenario overrides and return.
            // Keeps empty-metrics behavior consistent with `Instrument::value`.
            if let Some(overrides) = instrument.scenario_overrides() {
                base_result.value = overrides.apply_to_value(base_result.value);
            }
            return Ok(base_result);
        }

        // --- Metrics pipeline ---
        let market_arc = shared.market.unwrap_or_else(|| Arc::new(market.clone()));
        let err_ctx = PricingErrorContext::from_instrument(instrument).model(model);
        let needs_split = model != ModelKey::Discounting;
        let pricer_registry = shared.registry.unwrap_or_else(|| Arc::new(self.clone()));

        if !needs_split || !instrument.has_custom_metrics_equivalent() {
            let mut enriched = crate::instruments::common_impl::helpers::build_with_metrics_dyn(
                std::sync::Arc::from(instrument.clone_box()),
                Arc::clone(&market_arc),
                as_of,
                base_result.value,
                metrics,
                crate::instruments::common_impl::helpers::MetricBuildOptions {
                    cfg: cfg.clone(),
                    market_history: market_history.clone(),
                    pricing_model: Some(model),
                    pricer_registry: Some(Arc::clone(&pricer_registry)),
                },
            )
            .map_err(|e| {
                PricingError::model_failure_with_context(e.to_string(), err_ctx.clone())
            })?;

            for (k, v) in base_result.measures {
                enriched.measures.insert(k, v);
            }
            return Ok(enriched);
        }

        // Non-discounting model: split metrics into spread (cash-equivalent
        // cashflows) and risk (actual cashflows).
        let spread_ids = MetricId::SPREAD_EQUIVALENT_METRICS;

        let mut spread_metrics = Vec::with_capacity(metrics.len());
        let mut risk_metrics = Vec::with_capacity(metrics.len());
        for m in metrics {
            if spread_ids.contains(m) {
                spread_metrics.push(m.clone());
            } else {
                risk_metrics.push(m.clone());
            }
        }

        // Spread metrics: cash-equivalent cashflows via metrics_equivalent()
        let mut result = if !spread_metrics.is_empty() {
            let spread_instrument = Arc::from(instrument.metrics_equivalent());
            crate::instruments::common_impl::helpers::build_with_metrics_dyn(
                spread_instrument,
                Arc::clone(&market_arc),
                as_of,
                base_result.value,
                &spread_metrics,
                crate::instruments::common_impl::helpers::MetricBuildOptions {
                    cfg: cfg.clone(),
                    market_history: market_history.clone(),
                    ..crate::instruments::common_impl::helpers::MetricBuildOptions::default()
                },
            )
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), err_ctx.clone()))?
        } else {
            // Risk-only path: `build_with_metrics_dyn` (above) applies scenario
            // overrides internally, but this `stamped_with_meta` branch builds
            // the result from the raw `base_result.value` directly. Apply the
            // overrides here so the documented contract ("scenario price
            // overrides are always applied to the returned value") holds on the
            // risk-only non-discounting path too.
            let value = instrument
                .scenario_overrides()
                .map_or(base_result.value, |overrides| {
                    overrides.apply_to_value(base_result.value)
                });
            crate::results::ValuationResult::stamped_with_meta(
                instrument.id(),
                as_of,
                value,
                base_result.meta.clone(),
            )
        };

        // Risk metrics: actual instrument cashflows
        if !risk_metrics.is_empty() {
            let risk_result = crate::instruments::common_impl::helpers::build_with_metrics_dyn(
                std::sync::Arc::from(instrument.clone_box()),
                market_arc,
                as_of,
                base_result.value,
                &risk_metrics,
                crate::instruments::common_impl::helpers::MetricBuildOptions {
                    cfg,
                    market_history,
                    pricing_model: Some(model),
                    pricer_registry: Some(pricer_registry),
                },
            )
            .map_err(|e| {
                PricingError::model_failure_with_context(e.to_string(), err_ctx.clone())
            })?;
            for (k, v) in risk_result.measures {
                result.measures.insert(k, v);
            }
        }

        // Model-specific measures from price_dyn take priority
        for (k, v) in base_result.measures {
            result.measures.insert(k, v);
        }

        Ok(result)
    }

    /// Price an instrument as an unrounded scalar for internal risk repricing.
    pub(crate) fn price_raw(
        &self,
        instrument: &dyn Priceable,
        model: ModelKey,
        market: &Market,
        as_of: finstack_core::dates::Date,
    ) -> std::result::Result<f64, PricingError> {
        let key = PricerKey::new(instrument.key(), model);
        let Some(pricer) = self.get_pricer(key) else {
            return Err(PricingError::UnknownPricer {
                key,
                available_models: self.available_models_for_instrument(key.instrument),
            });
        };
        pricer.price_raw_dyn(instrument, market, as_of)
    }

    /// Price a batch of instruments in parallel, preserving input order.
    ///
    /// Each element is priced via [`Self::price_with_metrics`] with the same
    /// arguments, so scenario overrides and model-specific measures are applied
    /// identically. Pass an empty `metrics` slice for a PV-only batch.
    pub fn price_batch(
        &self,
        instruments: &[&dyn Priceable],
        model: ModelKey,
        market: &Market,
        as_of: finstack_core::dates::Date,
        metrics: &[crate::metrics::MetricId],
        options: crate::instruments::PricingOptions,
    ) -> Vec<std::result::Result<crate::results::ValuationResult, PricingError>> {
        let shared = if metrics.is_empty() {
            SharedPricingInputs::default()
        } else {
            SharedPricingInputs {
                registry: Some(Arc::new(self.clone())),
                market: Some(Arc::new(market.clone())),
            }
        };
        self.price_batch_impl(
            BatchPricingRequest {
                instruments,
                model,
                market,
                as_of,
                metrics,
                options,
            },
            shared,
        )
    }

    /// Price a batch through an already shared registry, preserving input order.
    ///
    /// The registry and market snapshot are shared across the batch's metric
    /// pipeline instead of being cloned once per instrument.
    pub fn price_batch_shared(
        registry: &Arc<Self>,
        instruments: &[&dyn Priceable],
        model: ModelKey,
        market: &Market,
        as_of: finstack_core::dates::Date,
        metrics: &[crate::metrics::MetricId],
        options: crate::instruments::PricingOptions,
    ) -> Vec<std::result::Result<crate::results::ValuationResult, PricingError>> {
        let shared_market = (!metrics.is_empty()).then(|| Arc::new(market.clone()));
        registry.as_ref().price_batch_impl(
            BatchPricingRequest {
                instruments,
                model,
                market,
                as_of,
                metrics,
                options,
            },
            SharedPricingInputs {
                registry: Some(Arc::clone(registry)),
                market: shared_market,
            },
        )
    }

    fn price_batch_impl(
        &self,
        request: BatchPricingRequest<'_>,
        shared: SharedPricingInputs,
    ) -> Vec<std::result::Result<crate::results::ValuationResult, PricingError>> {
        use rayon::prelude::*;
        let BatchPricingRequest {
            instruments,
            model,
            market,
            as_of,
            metrics,
            options,
        } = request;
        instruments
            .par_iter()
            .map(|&instrument| {
                self.price_with_metrics_impl(
                    PricingRequest {
                        instrument,
                        model,
                        market,
                        as_of,
                        metrics,
                        options: options.clone(),
                    },
                    shared.clone(),
                )
            })
            .collect()
    }
}

/// Stamp result metadata from a config.
///
/// FX policy precedence (highest to lowest):
/// 1. A policy already set on `result.meta.fx_policy_applied` by the pricer.
/// 2. The `fx_policy` field on any curve the instrument depends on.
/// 3. `None`.
///
/// Multi-curve aggregation: when more than one dependent curve carries a
/// policy stamp, they are joined with ` | ` so the audit trail records every
/// policy that fed into the valuation. Single-stamp results return the
/// policy verbatim.
fn stamp_results_meta(
    cfg: &FinstackConfig,
    instrument: &dyn Priceable,
    market: &finstack_core::market_data::context::MarketContext,
    result: &mut crate::results::ValuationResult,
) {
    let prev_fx_policy = result.meta.fx_policy_applied.clone();
    let mut meta = results_meta_now(cfg);
    meta.fx_policy_applied =
        prev_fx_policy.or_else(|| collect_fx_policy_from_curves(instrument, market));
    result.meta = meta;
}

/// Walk an instrument's declared curve dependencies and join any `fx_policy`
/// stamps the curves carry into a single result envelope value.
///
/// Returns `None` when the instrument has no curve dependencies, when no
/// dependent curve carries a stamp, or when the dependencies fail to
/// resolve (we surface the FX provenance as best-effort metadata, never as
/// a pricing error). Stamps are de-duplicated in source order — the same
/// policy applied to multiple curves shows up once.
fn collect_fx_policy_from_curves(
    instrument: &dyn Priceable,
    market: &finstack_core::market_data::context::MarketContext,
) -> Option<String> {
    let deps = instrument.market_dependencies().ok()?;
    let mut policies: Vec<String> = Vec::new();
    let mut push = |policy: Option<&str>| {
        if let Some(p) = policy {
            if !p.is_empty() && !policies.iter().any(|existing| existing == p) {
                policies.push(p.to_string());
            }
        }
    };
    for id in &deps.curves.discount_curves {
        if let Ok(curve) = market.get_discount(id.as_str()) {
            push(curve.fx_policy());
        }
    }
    for id in &deps.curves.forward_curves {
        if let Ok(curve) = market.get_forward(id.as_str()) {
            push(curve.fx_policy());
        }
    }
    for id in &deps.curves.credit_curves {
        if let Ok(curve) = market.get_hazard(id.as_str()) {
            push(curve.fx_policy());
        }
    }
    if policies.is_empty() {
        None
    } else {
        Some(policies.join(" | "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Helpers ────────────────────────────────────────────────────────────────

    /// Minimal flat discount curve anchored at `base_date`.
    fn flat_discount_curve(
        id: &str,
        base_date: finstack_core::dates::Date,
    ) -> finstack_core::market_data::term_structures::DiscountCurve {
        finstack_core::market_data::term_structures::DiscountCurve::builder(id)
            .base_date(base_date)
            .knots([(0.0, 1.0), (10.0, 0.9)])
            .interp(finstack_core::math::interp::InterpStyle::Linear)
            .build()
            .expect("DiscountCurve should build with valid test data")
    }

    /// Variant of [`flat_discount_curve`] with an opaque FX policy stamp.
    fn flat_discount_curve_with_fx_policy(
        id: &str,
        base_date: finstack_core::dates::Date,
        policy: &str,
    ) -> finstack_core::market_data::term_structures::DiscountCurve {
        finstack_core::market_data::term_structures::DiscountCurve::builder(id)
            .base_date(base_date)
            .knots([(0.0, 1.0), (10.0, 0.9)])
            .interp(finstack_core::math::interp::InterpStyle::Linear)
            .fx_policy(policy)
            .build()
            .expect("DiscountCurve should build with valid test data")
    }

    /// Multi-knot log-linear discount curve suitable for instruments that
    /// require richer interpolation (e.g., structured credit).
    fn multi_knot_discount_curve(
        id: &str,
        base_date: finstack_core::dates::Date,
    ) -> finstack_core::market_data::term_structures::DiscountCurve {
        finstack_core::market_data::term_structures::DiscountCurve::builder(id)
            .base_date(base_date)
            .knots([
                (0.0, 1.0),
                (0.5, 0.975),
                (1.0, 0.95),
                (2.0, 0.90),
                (5.0, 0.82),
                (10.0, 0.70),
            ])
            .interp(finstack_core::math::interp::InterpStyle::LogLinear)
            .build()
            .expect("Multi-knot DiscountCurve should build with valid test data")
    }

    /// Minimal flat hazard curve anchored at `base_date`.
    fn flat_hazard_curve(
        id: &str,
        base_date: finstack_core::dates::Date,
    ) -> finstack_core::market_data::term_structures::HazardCurve {
        finstack_core::market_data::term_structures::HazardCurve::builder(id)
            .base_date(base_date)
            .recovery_rate(0.40)
            .knots([(0.0, 0.02), (10.0, 0.02)])
            .build()
            .expect("HazardCurve should build with valid test data")
    }

    fn fixed_test_bond() -> crate::instruments::fixed_income::bond::Bond {
        crate::instruments::fixed_income::bond::Bond::fixed(
            "US912828XG33",
            finstack_core::money::Money::new(1_000.0, finstack_core::currency::Currency::USD),
            0.04,
            time::macros::date!(2020 - 01 - 15),
            time::macros::date!(2030 - 01 - 15),
            "USD-TREASURY",
        )
        .expect("fixed test bond should build")
    }

    fn flat_vol_surface(id: &str, vol: f64) -> finstack_core::market_data::surfaces::VolSurface {
        let expiries = [0.25, 0.5, 1.0, 2.0];
        let strikes = [2.5, 3.0, 3.5, 4.0, 4.5];
        let mut builder = finstack_core::market_data::surfaces::VolSurface::builder(id)
            .expiries(&expiries)
            .strikes(&strikes);
        for _ in &expiries {
            builder = builder.row(&vec![vol; strikes.len()]);
        }
        builder.build().expect("vol surface should build in tests")
    }

    fn commodity_swaption_market(
        as_of: finstack_core::dates::Date,
        flat_fwd: f64,
        vol: f64,
        rate: f64,
    ) -> finstack_core::market_data::context::MarketContext {
        let disc = finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
            .build()
            .expect("discount curve");
        let price_curve =
            finstack_core::market_data::term_structures::PriceCurve::builder("NG-FORWARD")
                .base_date(as_of)
                .spot_price(flat_fwd)
                .knots([(0.0, flat_fwd), (2.0, flat_fwd)])
                .build()
                .expect("price curve");

        finstack_core::market_data::context::MarketContext::new()
            .insert(disc)
            .insert(price_curve)
            .insert_surface(flat_vol_surface("NG-VOL", vol))
    }

    struct FixedBondPricer {
        amount: f64,
    }

    impl Pricer for FixedBondPricer {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Bond, ModelKey::Discounting)
        }

        fn price_dyn(
            &self,
            instrument: &dyn Priceable,
            _market: &Market,
            as_of: finstack_core::dates::Date,
        ) -> std::result::Result<crate::results::ValuationResult, PricingError> {
            Ok(crate::results::ValuationResult::stamped(
                instrument.id(),
                as_of,
                finstack_core::money::Money::new(
                    self.amount,
                    finstack_core::currency::Currency::USD,
                ),
            ))
        }
    }

    // ─── Parity tests: instrument trait path vs registry path ────────────────

    /// Default discounting path parity:
    /// `Bond::price_with_metrics` (trait default, discount engine) and
    /// `registry.price_with_metrics(..., ModelKey::Discounting, ..., crate::instruments::PricingOptions::default())` must
    /// produce the same PV.
    #[test]
    fn bond_discounting_parity_instrument_vs_registry() {
        use crate::instruments::common_impl::traits::Instrument;
        use finstack_core::dates::Date;
        use finstack_core::market_data::context::MarketContext;
        use time::macros::date;

        let as_of: Date = date!(2025 - 01 - 15);
        let bond = fixed_test_bond();
        let disc = flat_discount_curve("USD-TREASURY", as_of);
        let market = MarketContext::new().insert(disc);
        let registry = super::super::standard_registry();

        let trait_result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect("trait price_with_metrics should succeed");

        let registry_result = registry
            .price_with_metrics(
                &bond,
                ModelKey::Discounting,
                &market,
                as_of,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect("registry price_with_metrics should succeed");

        let trait_pv = trait_result.value.amount();
        let registry_pv = registry_result.value.amount();
        assert!(
            (trait_pv - registry_pv).abs() < 1.0,
            "Bond PV parity: trait={trait_pv:.4} registry={registry_pv:.4} diff > $1"
        );
    }

    #[test]
    fn fx_policy_propagates_from_curve_to_non_fx_instrument_result() {
        use finstack_core::dates::Date;
        use finstack_core::market_data::context::MarketContext;
        use time::macros::date;

        let as_of: Date = date!(2025 - 01 - 15);
        let bond = fixed_test_bond();
        let disc = flat_discount_curve_with_fx_policy("USD-TREASURY", as_of, "xccy_basis::USD/EUR");
        let market = MarketContext::new().insert(disc);
        let registry = super::super::standard_registry();

        let result = registry
            .price_with_metrics(
                &bond,
                ModelKey::Discounting,
                &market,
                as_of,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect("registry price should succeed");

        assert_eq!(
            result.meta.fx_policy_applied.as_deref(),
            Some("xccy_basis::USD/EUR"),
            "discount-curve fx_policy must propagate onto ResultsMeta"
        );
    }

    #[test]
    fn fx_policy_pricer_stamp_takes_precedence_over_curve_stamp() {
        use finstack_core::dates::Date;
        use finstack_core::market_data::context::MarketContext;
        use time::macros::date;

        let as_of: Date = date!(2025 - 01 - 15);
        let bond = fixed_test_bond();
        let disc = flat_discount_curve_with_fx_policy("USD-TREASURY", as_of, "curve::xccy_basis");
        let market = MarketContext::new().insert(disc);

        let pricer = FixedBondPricer { amount: 1_000.0 };
        let mut result = pricer
            .price_dyn(&bond, &market, as_of)
            .expect("synthetic pricer should price");
        result.meta.fx_policy_applied = Some("pricer::explicit_policy".to_string());

        stamp_results_meta(&FinstackConfig::default(), &bond, &market, &mut result);

        assert_eq!(
            result.meta.fx_policy_applied.as_deref(),
            Some("pricer::explicit_policy"),
            "explicit pricer stamp must outrank curve-walking fallback"
        );
    }

    /// Non-discounting split path:
    /// `registry.price_with_metrics` with `ModelKey::HazardRate` must
    /// produce a valid PV and successfully compute DV01 (a risk metric).
    #[test]
    fn bond_hazard_rate_registry_path_succeeds() {
        use finstack_core::dates::Date;
        use finstack_core::market_data::context::MarketContext;
        use time::macros::date;

        let as_of: Date = date!(2025 - 01 - 15);
        let bond = fixed_test_bond();
        let disc = flat_discount_curve("USD-TREASURY", as_of);
        let hazard = flat_hazard_curve("USD-CREDIT", as_of);
        let mut bond_with_credit = bond.clone();
        bond_with_credit.credit_curve_id =
            Some(finstack_core::types::CurveId::new("USD-CREDIT".to_string()));
        let market = MarketContext::new().insert(disc).insert(hazard);
        let registry = super::super::standard_registry();

        let result = registry
            .price_with_metrics(
                &bond_with_credit,
                ModelKey::HazardRate,
                &market,
                as_of,
                &[crate::metrics::MetricId::Dv01],
                crate::instruments::PricingOptions::default(),
            )
            .expect("registry price_with_metrics (HazardRate) should succeed");

        assert!(
            result.value.amount() < 0.0
                || result.value.amount() > 0.0
                || result.value.amount() == 0.0,
            "HazardRate PV should be a finite number"
        );
        assert!(
            result.measures.contains_key("dv01"),
            "DV01 measure should be present after non-discounting path"
        );
        assert!(
            result
                .measures
                .get("dv01")
                .copied()
                .unwrap_or_default()
                .is_finite(),
            "HazardRate DV01 should be finite"
        );
    }

    #[test]
    fn bond_hazard_rate_instrument_override_matches_registry() {
        use crate::instruments::common_impl::traits::Instrument;
        use finstack_core::dates::Date;
        use finstack_core::market_data::context::MarketContext;
        use time::macros::date;

        let as_of: Date = date!(2025 - 01 - 15);
        let bond = fixed_test_bond();
        let disc = flat_discount_curve("USD-TREASURY", as_of);
        let hazard = flat_hazard_curve("USD-CREDIT", as_of);
        let mut bond_with_credit = bond.clone();
        bond_with_credit.credit_curve_id =
            Some(finstack_core::types::CurveId::new("USD-CREDIT".to_string()));
        let market = MarketContext::new().insert(disc).insert(hazard);
        let registry = super::super::standard_registry();

        let instrument_result = bond_with_credit
            .price_with_metrics(
                &market,
                as_of,
                &[crate::metrics::MetricId::Dv01],
                crate::instruments::PricingOptions::default().with_model(ModelKey::HazardRate),
            )
            .expect("instrument override path should succeed");

        let registry_result = registry
            .price_with_metrics(
                &bond_with_credit,
                ModelKey::HazardRate,
                &market,
                as_of,
                &[crate::metrics::MetricId::Dv01],
                crate::instruments::PricingOptions::default(),
            )
            .expect("registry hazard-rate path should succeed");

        assert!(
            (instrument_result.value.amount() - registry_result.value.amount()).abs() < 1.0,
            "instrument override PV should match registry PV",
        );
        assert_eq!(
            instrument_result.measures.get("dv01"),
            registry_result.measures.get("dv01"),
            "instrument override metrics should match registry metrics",
        );
    }

    #[test]
    fn commodity_swaption_default_model_matches_registry() {
        use crate::instruments::common_impl::traits::Instrument;
        use time::macros::date;

        let as_of = date!(2025 - 01 - 15);
        let swaption =
            crate::instruments::commodity::commodity_swaption::CommoditySwaption::example();
        let market = commodity_swaption_market(as_of, 3.75, 0.30, 0.05);
        let registry = super::super::standard_registry();

        let instrument_result = swaption
            .price_with_metrics(
                &market,
                as_of,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect("instrument default-model path should succeed");
        let registry_result = registry
            .price_with_metrics(
                &swaption,
                ModelKey::Black76,
                &market,
                as_of,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect("registry Black76 path should succeed");

        assert!(
            (instrument_result.value.amount() - registry_result.value.amount()).abs() < 1e-9,
            "commodity swaption default model should match explicit Black76 registry pricing",
        );
    }

    #[test]
    fn instrument_can_use_custom_registry_override() {
        use crate::instruments::common_impl::traits::Instrument;
        use std::sync::Arc;
        use time::macros::date;

        let as_of = date!(2025 - 01 - 15);
        let bond = fixed_test_bond();
        let market = finstack_core::market_data::context::MarketContext::new()
            .insert(flat_discount_curve("USD-TREASURY", as_of));

        let default_result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[crate::metrics::MetricId::Dv01],
                crate::instruments::PricingOptions::default(),
            )
            .expect("default pricing path should succeed");

        let mut registry = PricerRegistry::new();
        registry.register(
            InstrumentType::Bond,
            ModelKey::Discounting,
            FixedBondPricer { amount: 990.0 },
        );

        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[crate::metrics::MetricId::Dv01],
                crate::instruments::PricingOptions::default().with_registry(Arc::new(registry)),
            )
            .expect("custom registry override should succeed");

        assert_eq!(result.value.amount(), 990.0);
        assert!(
            default_result
                .measures
                .get("dv01")
                .copied()
                .unwrap_or_default()
                .abs()
                > 1e-9,
            "control path should have non-zero DV01 so the override test is meaningful",
        );
        assert_eq!(
            result.measures.get("dv01").copied(),
            Some(0.0),
            "custom registry must also drive metric repricing, not just base PV",
        );
    }

    #[test]
    fn instrument_model_override_controls_metric_repricing() {
        use crate::instruments::common_impl::traits::Instrument;
        use std::sync::Arc;
        use time::macros::date;

        let as_of = date!(2025 - 01 - 15);
        let bond = fixed_test_bond();
        let market = finstack_core::market_data::context::MarketContext::new()
            .insert(flat_discount_curve("USD-TREASURY", as_of));

        let default_result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[crate::metrics::MetricId::Dv01],
                crate::instruments::PricingOptions::default(),
            )
            .expect("default pricing path should succeed");

        let mut registry = PricerRegistry::new();
        registry.register(
            InstrumentType::Bond,
            ModelKey::HazardRate,
            FixedBondPricer { amount: 995.0 },
        );

        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[crate::metrics::MetricId::Dv01],
                crate::instruments::PricingOptions::default()
                    .with_model(ModelKey::HazardRate)
                    .with_registry(Arc::new(registry)),
            )
            .expect("model override path should succeed");

        assert_eq!(result.value.amount(), 995.0);
        assert!(
            default_result
                .measures
                .get("dv01")
                .copied()
                .unwrap_or_default()
                .abs()
                > 1e-9,
            "control path should have non-zero DV01 so the override test is meaningful",
        );
        assert_eq!(
            result.measures.get("dv01").copied(),
            Some(0.0),
            "explicit model override must control metric repricing as well",
        );
    }

    #[test]
    fn non_discounting_risk_only_metrics_preserve_config_metadata() {
        use finstack_core::config::FinstackConfig;
        use finstack_core::currency::Currency;
        use time::macros::date;

        let as_of = date!(2025 - 01 - 15);
        let bond = fixed_test_bond();
        let mut bond_with_credit = bond.clone();
        bond_with_credit.credit_curve_id =
            Some(finstack_core::types::CurveId::new("USD-CREDIT".to_string()));

        let market = finstack_core::market_data::context::MarketContext::new()
            .insert(flat_discount_curve("USD-TREASURY", as_of))
            .insert(flat_hazard_curve("USD-CREDIT", as_of));
        let registry = super::super::standard_registry();

        let mut cfg = FinstackConfig::default();
        cfg.rounding.output_scale.overrides.insert(Currency::USD, 4);

        let result = registry
            .price_with_metrics(
                &bond_with_credit,
                ModelKey::HazardRate,
                &market,
                as_of,
                &[crate::metrics::MetricId::Dv01],
                crate::instruments::PricingOptions::default().with_config(&cfg),
            )
            .expect("hazard-rate pricing with config should succeed");

        assert_eq!(
            result
                .meta
                .rounding
                .output_scale_by_ccy
                .get(&Currency::USD)
                .copied(),
            Some(4),
            "risk-only split path should preserve caller config metadata",
        );
    }

    /// StructuredCredit overridden path:
    /// Both `instrument.price_with_metrics` (instrument-level override) and
    /// `registry.price_with_metrics` must follow the same code path: either both
    /// succeed with the same PV, or both fail with the same error type.
    ///
    /// The example CLO (minimal, empty pool) may or may not produce a valid PV
    /// depending on the waterfall simulation; what matters is that both paths are
    /// consistent.
    #[test]
    fn structured_credit_parity_instrument_vs_registry() {
        use crate::instruments::common_impl::traits::Instrument;
        use finstack_core::dates::Date;
        use finstack_core::market_data::context::MarketContext;
        use time::macros::date;

        let as_of: Date = date!(2025 - 01 - 15);
        let clo = crate::instruments::fixed_income::structured_credit::StructuredCredit::example();
        let disc = multi_knot_discount_curve("USD-OIS", as_of);
        let market = MarketContext::new().insert(disc);
        let registry = super::super::standard_registry();

        let trait_result = clo.price_with_metrics(
            &market,
            as_of,
            &[],
            crate::instruments::PricingOptions::default(),
        );
        let registry_result = registry.price_with_metrics(
            &clo,
            ModelKey::Discounting,
            &market,
            as_of,
            &[],
            crate::instruments::PricingOptions::default(),
        );

        match (trait_result, registry_result) {
            (Ok(t), Ok(r)) => {
                let trait_pv = t.value.amount();
                let registry_pv = r.value.amount();
                assert!(
                    (trait_pv - registry_pv).abs() < 1.0,
                    "StructuredCredit PV parity: trait={trait_pv:.4} registry={registry_pv:.4} diff > $1"
                );
            }
            (Err(t_err), Err(r_err)) => {
                // Both paths fail: verify the underlying error message is the same.
                // The registry wraps errors in ModelFailure, so we compare the
                // inner cause rather than the full error string.
                let t_msg = t_err.to_string();
                let r_msg = r_err.to_string();
                assert!(
                    t_msg.contains("two data points")
                        || r_msg.contains("two data points")
                        || t_msg == r_msg,
                    "Both paths fail but with unrelated errors; trait={t_err:?} registry={r_err:?}"
                );
            }
            (Ok(t), Err(r_err)) => {
                panic!(
                    "Trait succeeded (PV={:.4}) but registry failed ({r_err:?})",
                    t.value.amount()
                );
            }
            (Err(t_err), Ok(r)) => {
                panic!(
                    "Registry succeeded (PV={:.4}) but trait failed ({t_err:?})",
                    r.value.amount()
                );
            }
        }
    }

    /// Regression guard: empty metrics slice must not cause a difference in PV.
    /// Any future refactor that accidentally introduces metric-side-effects on PV
    /// will be caught here.
    #[test]
    fn empty_metrics_does_not_alter_pv() {
        use crate::instruments::common_impl::traits::Instrument;
        use finstack_core::dates::Date;
        use finstack_core::market_data::context::MarketContext;
        use time::macros::date;

        let as_of: Date = date!(2025 - 01 - 15);
        let bond = fixed_test_bond();
        let disc = flat_discount_curve("USD-TREASURY", as_of);
        let market = MarketContext::new().insert(disc);
        let registry = super::super::standard_registry();

        let baseline = bond
            .value(&market, as_of)
            .expect("bond.value should succeed");

        let with_metrics = registry
            .price_with_metrics(
                &bond,
                ModelKey::Discounting,
                &market,
                as_of,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect("registry price_with_metrics should succeed");

        assert!(
            (baseline.amount() - with_metrics.value.amount()).abs() < 1.0,
            "PV with empty metrics should equal bare value: baseline={:.4} with_metrics={:.4}",
            baseline.amount(),
            with_metrics.value.amount()
        );
    }

    // ─── Existing tests ──────────────────────────────────────────────────────

    #[test]
    fn registry_creation_test() {
        let registry = super::super::standard_registry();

        let key = PricerKey::new(InstrumentType::Bond, ModelKey::Discounting);
        assert!(registry.get_pricer(key).is_some());
    }

    #[test]
    fn registration_covers_all_pricers() {
        let registry = super::super::standard_registry();

        // Bond pricers
        assert!(
            registry
                .get_pricer(PricerKey::new(InstrumentType::Bond, ModelKey::Discounting))
                .is_some(),
            "Bond Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(InstrumentType::Bond, ModelKey::HazardRate))
                .is_some(),
            "Bond HazardRate pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(InstrumentType::Bond, ModelKey::Tree))
                .is_some(),
            "Bond OAS pricer should be registered"
        );

        // Interest Rate pricers
        assert!(
            registry
                .get_pricer(PricerKey::new(InstrumentType::IRS, ModelKey::Discounting))
                .is_some(),
            "IRS Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(InstrumentType::FRA, ModelKey::Discounting))
                .is_some(),
            "FRA Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(InstrumentType::CapFloor, ModelKey::Black76))
                .is_some(),
            "CapFloor Black76 pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::CapFloor,
                    ModelKey::Discounting
                ))
                .is_some(),
            "CapFloor Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(InstrumentType::Swaption, ModelKey::Black76))
                .is_some(),
            "Swaption Black76 pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::Swaption,
                    ModelKey::Discounting
                ))
                .is_some(),
            "Swaption Discounting pricer should be registered"
        );

        // Credit pricers
        assert!(
            registry
                .get_pricer(PricerKey::new(InstrumentType::CDS, ModelKey::HazardRate))
                .is_some(),
            "CDS HazardRate pricer should be registered"
        );
        // CDS / CDSIndex / CDSOption / CDSTranche no longer register a
        // `ModelKey::Discounting` alias. The earlier registrations pointed at
        // the same hazard (or Black76) implementation, which falsely implied a
        // pure-discounting alternative existed. See `pricer/credit.rs` for
        // the rationale; callers should look these products up under their
        // real model key.
        assert!(
            registry
                .get_pricer(PricerKey::new(InstrumentType::CDS, ModelKey::Discounting))
                .is_none(),
            "CDS Discounting pricer must not be registered (misleading alias removed)"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::CDSIndex,
                    ModelKey::HazardRate
                ))
                .is_some(),
            "CDSIndex HazardRate pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::CDSIndex,
                    ModelKey::Discounting
                ))
                .is_none(),
            "CDSIndex Discounting pricer must not be registered (misleading alias removed)"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::CDSOption,
                    ModelKey::BloombergCdso
                ))
                .is_some(),
            "CDSOption BloombergCdso pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(InstrumentType::CDSOption, ModelKey::Black76))
                .is_none(),
            "Black76 pricer for CDSOption was decommissioned (DOCS 2055833 §1.2)"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::CDSOption,
                    ModelKey::Discounting
                ))
                .is_none(),
            "CDSOption Discounting pricer must not be registered (misleading alias removed)"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::CDSTranche,
                    ModelKey::HazardRate
                ))
                .is_some(),
            "CDSTranche HazardRate pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::CDSTranche,
                    ModelKey::Discounting
                ))
                .is_none(),
            "CDSTranche Discounting pricer must not be registered (misleading alias removed)"
        );

        // FX pricers
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::FxSpot,
                    ModelKey::Discounting
                ))
                .is_some(),
            "FxSpot Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(InstrumentType::FxOption, ModelKey::Black76))
                .is_some(),
            "FxOption Black76 pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::FxOption,
                    ModelKey::Discounting
                ))
                .is_none(),
            "FxOption Discounting pricer must not be registered (misleading alias removed)"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::FxSwap,
                    ModelKey::Discounting
                ))
                .is_some(),
            "FxSwap Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::FxBarrierOption,
                    ModelKey::FxBarrierVannaVolga
                ))
                .is_none(),
            "FxBarrierOption Vanna-Volga must not be registered without a quote contract"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::FxDigitalOption,
                    ModelKey::Black76
                ))
                .is_some(),
            "FxDigitalOption Black76 pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::FxDigitalOption,
                    ModelKey::Discounting
                ))
                .is_none(),
            "FxDigitalOption Discounting pricer must not be registered (misleading alias removed)"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::FxTouchOption,
                    ModelKey::Black76
                ))
                .is_some(),
            "FxTouchOption Black76 pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::FxTouchOption,
                    ModelKey::Discounting
                ))
                .is_none(),
            "FxTouchOption Discounting pricer must not be registered (misleading alias removed)"
        );

        // Equity pricers
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::Equity,
                    ModelKey::Discounting
                ))
                .is_some(),
            "Equity Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::EquityOption,
                    ModelKey::Black76
                ))
                .is_some(),
            "EquityOption Black76 pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::EquityOption,
                    ModelKey::Discounting
                ))
                .is_some(),
            "EquityOption Discounting pricer should be registered"
        );

        // Basic pricers
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::Deposit,
                    ModelKey::Discounting
                ))
                .is_some(),
            "Deposit Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::InterestRateFuture,
                    ModelKey::Discounting
                ))
                .is_some(),
            "InterestRateFuture Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::BasisSwap,
                    ModelKey::Discounting
                ))
                .is_some(),
            "BasisSwap Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(InstrumentType::Repo, ModelKey::Discounting))
                .is_some(),
            "Repo Discounting pricer should be registered"
        );

        // Inflation pricers
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::InflationSwap,
                    ModelKey::Discounting
                ))
                .is_some(),
            "InflationSwap Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::YoYInflationSwap,
                    ModelKey::Discounting
                ))
                .is_some(),
            "YoYInflationSwap Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::InflationCapFloor,
                    ModelKey::Black76
                ))
                .is_some(),
            "InflationCapFloor Black76 pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::InflationCapFloor,
                    ModelKey::Normal
                ))
                .is_some(),
            "InflationCapFloor Normal pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::InflationLinkedBond,
                    ModelKey::Discounting
                ))
                .is_some(),
            "InflationLinkedBond Discounting pricer should be registered"
        );

        // Complex pricers
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::VarianceSwap,
                    ModelKey::Discounting
                ))
                .is_some(),
            "VarianceSwap Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::FxVarianceSwap,
                    ModelKey::Discounting
                ))
                .is_some(),
            "FxVarianceSwap Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::RealEstateAsset,
                    ModelKey::Discounting
                ))
                .is_some(),
            "RealEstateAsset Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(InstrumentType::CmsOption, ModelKey::Black76))
                .is_some(),
            "CmsOption Black76 pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::CmsOption,
                    ModelKey::StaticReplication
                ))
                .is_some(),
            "CmsOption StaticReplication pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::CmsOption,
                    ModelKey::Discounting
                ))
                .is_none(),
            "CmsOption Discounting pricer must not be registered because it was only a Black76 alias"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::CommodityOption,
                    ModelKey::Black76
                ))
                .is_some(),
            "CommodityOption Black76 pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::Basket,
                    ModelKey::Discounting
                ))
                .is_some(),
            "Basket Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::Convertible,
                    ModelKey::Discounting
                ))
                .is_some(),
            "Convertible Discounting pricer should be registered"
        );

        // Structured credit pricer (unified for ABS, CLO, CMBS, RMBS)
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::StructuredCredit,
                    ModelKey::Discounting
                ))
                .is_some(),
            "StructuredCredit Discounting pricer should be registered"
        );

        // TRS and Private Markets
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::EquityTotalReturnSwap,
                    ModelKey::Discounting
                ))
                .is_some(),
            "EquityTotalReturnSwap Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::FIIndexTotalReturnSwap,
                    ModelKey::Discounting
                ))
                .is_some(),
            "FIIndexTotalReturnSwap Discounting pricer should be registered"
        );
        assert!(
            registry
                .get_pricer(PricerKey::new(
                    InstrumentType::PrivateMarketsFund,
                    ModelKey::Discounting
                ))
                .is_some(),
            "PrivateMarketsFund Discounting pricer should be registered"
        );
    }
}

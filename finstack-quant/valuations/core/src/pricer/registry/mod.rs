//! Pricer registry, trait, and dispatch infrastructure.
//!
//! Defines the [`Pricer`] trait, [`PricerRegistry`], and the [`expect_inst`]
//! downcast helper used by all pricer implementations.

use super::{InstrumentType, ModelKey, PricerKey, PricingError, PricingErrorContext};
use crate::instruments::Instrument;
use finstack_quant_core::config::{FinstackConfig, results_meta};
use finstack_quant_core::market_data::context::MarketContext;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Downcast a boxed instrument to its concrete type, checking the
/// [`InstrumentType`] key first.
///
/// Every non-generic [`Pricer::price_dyn`] implementation should use this
/// instead of hand-rolling `as_any().downcast_ref()`: it checks the enum key
/// before the type-id downcast so a mismatch reports the expected and actual
/// instrument types.
///
/// # Arguments
///
/// * `inst` - The dynamically typed instrument handed to the pricer.
/// * `expected` - The [`InstrumentType`] this pricer handles; must equal
///   `inst.key()` and correspond to the concrete type `T`.
///
/// # Errors
///
/// Returns [`PricingError::TypeMismatch`] when `inst.key() != expected` or
/// when the concrete type is not `T`.
pub fn expect_inst<T: Instrument + 'static>(
    inst: &dyn Instrument,
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

    /// Resolve model-owned settings and cached analytics before metric evaluation.
    ///
    /// # Arguments
    ///
    /// * `context` - Validated base pricing request; implementations may install a
    ///   request-local repricing dispatch that rebuilds caches for shocked inputs.
    /// * `metrics` - Requested metric identifiers; expensive analytics should only
    ///   be prepared when needed by these calculations.
    fn seed_metric_context(
        &self,
        context: &mut crate::metrics::MetricContext,
        metrics: &[crate::metrics::MetricId],
    ) -> finstack_quant_core::Result<()> {
        let _ = (context, metrics);
        Ok(())
    }

    /// Price an instrument using this pricer's model.
    ///
    /// This is a low-level dispatch hook that assumes the instrument has already
    /// passed [`Instrument::validate_for_pricing`] and `as_of` has already been
    /// resolved through [`Instrument::resolve_pricing_as_of`]. Canonical callers
    /// must enter through [`PricerRegistry::price_with_metrics`] or its internal
    /// raw path.
    /// Implementations must return an unshocked result because the registry owns
    /// exactly-once scenario application.
    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<crate::results::ValuationResult, PricingError>;

    /// Price an instrument as an unrounded scalar when the pricer can provide one.
    ///
    /// The default implementation falls back to [`Self::price_dyn`] and extracts the
    /// rounded `Money` amount. Pricers with a true raw-f64 path should override this
    /// so finite-difference risk calculations do not inherit currency rounding noise.
    /// This is an unchecked, unshocked model kernel; canonical callers enter
    /// through the registry's raw-pricing path.
    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<f64, PricingError> {
        Ok(self.price_dyn(instrument, market, as_of)?.value.amount())
    }
}

/// Registry mapping (instrument type, model) pairs to pricer implementations.
///
/// Provides type-safe pricing dispatch: pricers are registered once at
/// registry construction and looked up via strongly-typed keys.
///
/// The ordered pricer map is shared behind [`Arc`]. Cloning a registry is
/// therefore O(1); registration and replacement use copy-on-write so cloned
/// registries remain independently mutable.
#[derive(Clone, Default)]
pub struct PricerRegistry {
    pricers: Arc<BTreeMap<PricerKey, Arc<dyn Pricer>>>,
}
/// Pricing path used consistently by base, bumped, theta, and scenario valuations.
#[derive(Clone, Default)]
pub enum PricingDispatch {
    /// Use each instrument's canonical default pricing path.
    #[default]
    InstrumentDefault,
    /// Reprice through one explicit model and registry pair.
    Registered {
        /// Model selected for every valuation through this dispatch.
        model: ModelKey,
        /// Registry that owns the selected model implementation.
        registry: Arc<PricerRegistry>,
    },
}

impl PricingDispatch {
    /// Construct an explicit registered pricing dispatch.
    ///
    /// # Arguments
    ///
    /// * `model` - Registered model used for base and risk repricing.
    /// * `registry` - Shared registry containing the selected model.
    pub fn registered(model: ModelKey, registry: Arc<PricerRegistry>) -> Self {
        Self::Registered { model, registry }
    }

    /// Select the dispatch for a host-supplied model key.
    ///
    /// This is the model argument of the host historical-VaR entry points
    /// (Rust `metrics::risk::calculate_var`): `"default"` keeps each
    /// instrument's canonical default path, any other key reprices every
    /// instrument through that model in `registry`.
    ///
    /// # Arguments
    ///
    /// * `model` - `"default"`, or a canonical model key such as
    ///   `"discounting"` or `"hazard_rate"` (the umbrella crate's `pricer::list_models`
    ///   lists them).
    /// * `registry` - Shared registry the explicit model is resolved in;
    ///   unused for `"default"`. Host entry points pass the standard
    ///   registry (`STANDARD.shared_pricers()`).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `model` is not a recognised model key.
    pub fn from_model(
        model: &str,
        registry: Arc<PricerRegistry>,
    ) -> finstack_quant_core::Result<Self> {
        if model == "default" {
            Ok(Self::InstrumentDefault)
        } else {
            Ok(Self::registered(super::parse_model_key(model)?, registry))
        }
    }

    /// Return the explicitly selected model, if any.
    #[must_use]
    pub fn model(&self) -> Option<ModelKey> {
        match self {
            Self::InstrumentDefault => None,
            Self::Registered { model, .. } => Some(*model),
        }
    }

    /// Price `instrument` as `Money` through this dispatch path.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Instrument to value.
    /// * `market` - Market context supplying curves, surfaces and scalars.
    /// * `as_of` - Valuation date.
    /// * `options` - Pricing options forwarded to the registry path; ignored
    ///   by `InstrumentDefault`, which calls `Instrument::value` directly.
    pub(crate) fn price_money(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
        options: crate::instruments::PricingOptions,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        match self {
            Self::Registered { model, registry } => Ok(registry
                .price_with_metrics(instrument, *model, market, as_of, &[], options)?
                .value),
            Self::InstrumentDefault => instrument.value(market, as_of),
        }
    }

    /// Price `instrument` as an unrounded `f64` through this dispatch path.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Instrument to value.
    /// * `market` - Market context supplying curves, surfaces and scalars.
    /// * `as_of` - Valuation date.
    #[doc(hidden)]
    pub fn price_raw(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        match self {
            Self::Registered { model, registry } => registry
                .price_raw(instrument, *model, market, as_of)
                .map_err(Into::into),
            Self::InstrumentDefault => instrument.value_raw(market, as_of),
        }
    }
}

impl PricerRegistry {
    /// Create a new empty pricer registry.
    ///
    /// For pre-configured registries with all standard pricers, use the
    /// umbrella crate's `pricer::finstack_quant_valuations::pricer::STANDARD.pricers()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a pricer under its authoritative key.
    ///
    /// Returns [`PricingError::DuplicateRegistration`] without mutating the
    /// registry when the key already exists. Use [`Self::replace`] for an
    /// intentional overwrite.
    ///
    /// # Arguments
    ///
    /// * `pricer` - Implementation whose [`Pricer::key`] selects the instrument/model pair.
    pub fn register(
        &mut self,
        pricer: impl Pricer + 'static,
    ) -> std::result::Result<(), PricingError> {
        let key = pricer.key();
        if self.pricers.contains_key(&key) {
            return Err(PricingError::DuplicateRegistration { key });
        }
        Arc::make_mut(&mut self.pricers).insert(key, Arc::new(pricer));
        Ok(())
    }

    /// Deliberately replace a pricer for test setup or controlled monkey-patching.
    ///
    /// # Arguments
    ///
    /// * `pricer` - Replacement implementation stored under its authoritative key.
    pub fn replace(&mut self, pricer: impl Pricer + 'static) {
        let key = pricer.key();
        Arc::make_mut(&mut self.pricers).insert(key, Arc::new(pricer));
    }

    /// Look up a pricer for a specific (instrument type, model) combination.
    ///
    /// The returned low-level pricer does not validate instruments, resolve the
    /// effective valuation date, or apply scenario overrides itself.
    /// Canonical pricing should use [`Self::price_with_metrics`].
    pub fn get_pricer(&self, key: PricerKey) -> Option<&dyn Pricer> {
        self.pricers.get(&key).map(|p| p.as_ref())
    }

    /// Validate an instrument, then resolve its registered pricer.
    ///
    /// Keeping this order in one helper ensures malformed instruments fail as
    /// invalid input before model lookup or any market-dependent computation.
    fn validated_pricer<'registry, 'instrument>(
        &'registry self,
        instrument: &'instrument dyn Instrument,
        model: ModelKey,
    ) -> std::result::Result<
        (
            crate::instruments::common_impl::helpers::ValidatedPricingLifecycle<
                'instrument,
                dyn Instrument + 'instrument,
            >,
            &'registry dyn Pricer,
        ),
        PricingError,
    > {
        let context = PricingErrorContext::from_instrument(instrument).model(model);
        let lifecycle =
            crate::instruments::common_impl::helpers::ValidatedPricingLifecycle::new(instrument)
                .map_err(|error| PricingError::from_core(error, context))?;

        let key = PricerKey::new(instrument.key(), model);
        let pricer = self
            .get_pricer(key)
            .ok_or_else(|| PricingError::UnknownPricer {
                key,
                available_models: self.available_models_for_instrument(key.instrument),
            })?;
        Ok((lifecycle, pricer))
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

    /// Return every distinct [`ModelKey`] that has at least one registered
    /// pricer, in ascending `ModelKey` order.
    ///
    /// This is registry-derived, not enum-derived: a [`ModelKey`] variant with
    /// no registered pricer is deliberately omitted, so the result describes
    /// real dispatch coverage rather than the full model vocabulary that
    /// `ModelKey::iter()` would advertise.
    ///
    /// # Returns
    ///
    /// Deduplicated model keys sorted ascending, which makes the order
    /// deterministic independently of the backing [`BTreeMap`]'s
    /// instrument-major key ordering.
    pub fn all_models(&self) -> Vec<ModelKey> {
        let mut models: Vec<ModelKey> = self.pricers.keys().map(|key| key.model).collect();
        models.sort_unstable();
        models.dedup();
        models
    }

    /// Return every registered [`ModelKey`] grouped by [`InstrumentType`].
    ///
    /// Like [`Self::all_models`] this is registry-derived: only instrument
    /// types that actually have a registered pricer appear as keys, and each
    /// entry lists only the models that can price that instrument. Both the
    /// outer map and each model list are in ascending key order, so the
    /// grouping is deterministic across runs.
    ///
    /// # Returns
    ///
    /// A [`BTreeMap`] from instrument type to its deduplicated, ascending
    /// model keys.
    pub fn all_models_grouped(&self) -> BTreeMap<InstrumentType, Vec<ModelKey>> {
        let mut grouped: BTreeMap<InstrumentType, Vec<ModelKey>> = BTreeMap::new();
        for key in self.pricers.keys() {
            grouped.entry(key.instrument).or_default().push(key.model);
        }
        for models in grouped.values_mut() {
            models.sort_unstable();
            models.dedup();
        }
        grouped
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
    /// * `as_of` - Valuation date that anchors discounting, accruals, and metric bumps
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
        instrument: &dyn Instrument,
        model: ModelKey,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
        metrics: &[crate::metrics::MetricId],
        options: crate::instruments::PricingOptions,
    ) -> std::result::Result<crate::results::ValuationResult, PricingError> {
        let crate::instruments::PricingOptions {
            config: cfg,
            market_history,
            metric_registry,
            recalibration_provider,
            explain,
            ..
        } = options;
        let requested_as_of = as_of;

        // --- Base PV through the registered pricer ---
        tracing::debug!(
            instrument_id = %instrument.id(),
            instrument_type = %instrument.key(),
            model_key = %model,
            %as_of,
            num_metrics = metrics.len(),
            "dispatching registered pricer"
        );
        let (lifecycle, pricer) = self.validated_pricer(instrument, model)?;
        let effective_as_of = lifecycle.effective_as_of(market, as_of);
        let err_ctx = PricingErrorContext::from_instrument(instrument).model(model);
        let mut base_result = pricer
            .price_dyn(instrument, market, effective_as_of)
            .map_err(|error| error.with_fallback_context(err_ctx.clone()))?;
        // The lifecycle owns date resolution. A model may populate a richer
        // result envelope, but it must not replace the canonical valuation
        // date used by downstream metrics and host-language callers.
        base_result.as_of = effective_as_of;
        let deps = instrument
            .market_dependencies()
            .map_err(|error| PricingError::from_core(error, err_ctx.clone()))?;
        let default_cfg;
        let effective_cfg = match cfg.as_deref() {
            Some(c) => c,
            None => {
                default_cfg = FinstackConfig::default();
                &default_cfg
            }
        };
        stamp_results_meta(effective_cfg, &deps, market, &mut base_result);

        // Scenario price adjustments are a valuation-boundary operation. The
        // model kernel remains unshocked; every result and metric context sees
        // this one adjusted value. A pricer whose detail payload restates the
        // value (the structured-credit stochastic pricer) returns an unshocked
        // headline and shocks that payload itself, so the two agree here.
        base_result.value = lifecycle
            .apply_value(base_result.value)
            .map_err(|error| PricingError::from_core(error, err_ctx.clone()))?;

        base_result.provenance = Some(
            valuation_provenance(
                instrument,
                model,
                requested_as_of,
                effective_cfg,
                !metrics.is_empty(),
                deps,
            )
            .map_err(|error| PricingError::from_core(error, err_ctx))?,
        );
        if explain.enabled && base_result.explanation.is_none() {
            base_result.explanation = Some(crate::instruments::cashflow_export::pricing_trace(
                instrument,
                market,
                model,
                &base_result,
                explain,
            ));
        }

        super::enrichment::enrich(super::enrichment::EnrichmentRequest {
            instrument,
            model,
            market,
            as_of: effective_as_of,
            metrics,
            cfg,
            market_history,
            metric_registry,
            recalibration_provider,
            pricer_registry: Arc::new(self.clone()),
            base_result,
        })
    }

    /// Price an instrument as an unrounded scalar for internal risk repricing.
    #[doc(hidden)]
    pub fn price_raw(
        &self,
        instrument: &dyn Instrument,
        model: ModelKey,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<f64, PricingError> {
        let (lifecycle, pricer) = self.validated_pricer(instrument, model)?;
        let effective_as_of = lifecycle.effective_as_of(market, as_of);
        let err_ctx = PricingErrorContext::from_instrument(instrument).model(model);
        let base_value = pricer
            .price_raw_dyn(instrument, market, effective_as_of)
            .map_err(|error| error.with_fallback_context(err_ctx))?;
        Ok(lifecycle.apply_raw_value(base_value))
    }
}

/// Record how a result was produced: model, requested date, declared market
/// dependencies, scenario adjustment and (for metric requests) bump sizes.
fn valuation_provenance(
    instrument: &dyn Instrument,
    model: ModelKey,
    requested_as_of: finstack_quant_core::dates::Date,
    cfg: &FinstackConfig,
    with_metrics: bool,
    market_dependencies: crate::instruments::MarketDependencies,
) -> finstack_quant_core::Result<crate::results::ValuationProvenance> {
    let overrides = instrument.get_metric_pricing_overrides();
    let sensitivity_bumps = if with_metrics {
        let resolved =
            crate::metrics::sensitivities::config::from_context_or_default(cfg, overrides)?;
        Some(crate::results::SensitivityBumps {
            rate_bump_bp: resolved.rate_bump_bp,
            credit_spread_bump_bp: resolved.credit_spread_bump_bp,
            spot_bump_decimal: resolved.spot_bump_decimal,
            vol_bump_decimal: resolved.vol_bump_decimal,
            ytm_bump_bp: resolved.ytm_bump_bp,
            adaptive_bumps: overrides.is_some_and(|o| o.bump_config.adaptive_bumps),
        })
    } else {
        None
    };
    Ok(crate::results::ValuationProvenance {
        model,
        requested_as_of,
        market_dependencies,
        scenario_price_shock_decimal: instrument
            .get_scenario_pricing_overrides()
            .and_then(|overrides| overrides.scenario_price_shock_decimal),
        sensitivity_bumps,
    })
}

/// Apply request-owned metadata while preserving model-owned audit fields.
///
/// Numeric mode and rounding come from the effective request config. Existing
/// model timestamps and version stamps are retained, with request defaults used
/// only when the model omitted them.
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
    deps: &crate::instruments::MarketDependencies,
    market: &finstack_quant_core::market_data::context::MarketContext,
    result: &mut crate::results::ValuationResult,
) {
    let mut previous = std::mem::take(&mut result.meta);
    previous.fx_policy_applied = previous
        .fx_policy_applied
        .or_else(|| collect_fx_policy_from_curves(deps, market));
    result.meta = previous.overlay_request(results_meta(cfg));
}

/// Attach computed metrics without replacing the model-produced result envelope.
///
/// Model measures are inserted last and therefore take precedence when a
/// model and a generic calculator emit the same metric ID.
pub(super) fn attach_metric_measures(
    result: &mut crate::results::ValuationResult,
    mut metric_measures: indexmap::IndexMap<crate::metrics::MetricId, f64>,
) -> finstack_quant_core::Result<()> {
    for (metric_id, &value) in &metric_measures {
        crate::metrics::registry::validate_measure(metric_id, value)?;
    }
    for (metric_id, value) in std::mem::take(&mut result.measures) {
        metric_measures.insert(metric_id, value);
    }
    result.measures = metric_measures;
    Ok(())
}

/// Walk an instrument's declared curve dependencies and join any `fx_policy`
/// stamps the curves carry into a single result envelope value.
///
/// Returns `None` when the instrument has no curve dependencies or no dependent
/// curve carries a stamp. Stamps are de-duplicated in source order.
fn collect_fx_policy_from_curves(
    deps: &crate::instruments::MarketDependencies,
    market: &finstack_quant_core::market_data::context::MarketContext,
) -> Option<String> {
    let mut policies: Vec<String> = Vec::new();
    let mut push = |policy: Option<&str>| {
        if let Some(p) = policy
            && !p.is_empty()
            && !policies.iter().any(|existing| existing == p)
        {
            policies.push(p.to_string());
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
mod tests;

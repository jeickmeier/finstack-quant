//! Pricer registry, trait, and dispatch infrastructure.
//!
//! Defines the [`Pricer`] trait, [`PricerRegistry`], and the [`expect_inst`]
//! downcast helper used by all pricer implementations.

use super::{InstrumentType, ModelKey, PricerKey, PricingError, PricingErrorContext};
use crate::instruments::common_impl::traits::Instrument as Priceable;
use finstack_quant_core::config::{results_meta, FinstackConfig};
use finstack_quant_core::market_data::context::MarketContext as Market;
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
    /// passed [`Priceable::validate_for_pricing`] and `as_of` has already been
    /// resolved through [`Priceable::resolve_pricing_as_of`]. Canonical callers
    /// must enter through [`PricerRegistry::price_with_metrics`] or its internal
    /// raw path.
    /// Implementations must return an unshocked result because the registry owns
    /// exactly-once scenario application.
    fn price_dyn(
        &self,
        instrument: &dyn Priceable,
        market: &Market,
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
        instrument: &dyn Priceable,
        market: &Market,
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
        instrument: &dyn Priceable,
        market: &Market,
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
        instrument: &dyn Priceable,
        market: &Market,
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
    /// umbrella crate's `pricer::standard_pricer_registry()`.
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
        instrument: &'instrument dyn Priceable,
        model: ModelKey,
        instrument_validated: bool,
    ) -> std::result::Result<
        (
            crate::instruments::common_impl::helpers::ValidatedPricingLifecycle<
                'instrument,
                dyn Priceable + 'instrument,
            >,
            &'registry dyn Pricer,
        ),
        PricingError,
    > {
        let context = PricingErrorContext::from_instrument(instrument).model(model);
        let lifecycle = if instrument_validated {
            crate::instruments::common_impl::helpers::ValidatedPricingLifecycle::from_validated(
                instrument,
            )
        } else {
            crate::instruments::common_impl::helpers::ValidatedPricingLifecycle::new(instrument)
                .map_err(|error| PricingError::from_core(error, context))?
        };

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
        instrument: &dyn Priceable,
        model: ModelKey,
        market: &Market,
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
            instrument_validated,
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
        let (lifecycle, pricer) = self.validated_pricer(instrument, model, instrument_validated)?;
        let effective_as_of = lifecycle.effective_as_of(market, as_of);
        let err_ctx = PricingErrorContext::from_instrument(instrument).model(model);
        let mut base_result = pricer
            .price_dyn(instrument, market, effective_as_of)
            .map_err(|error| error.with_fallback_context(err_ctx))?;
        // The lifecycle owns date resolution. A model may populate a richer
        // result envelope, but it must not replace the canonical valuation
        // date used by downstream metrics and host-language callers.
        base_result.as_of = effective_as_of;
        let effective_cfg = cfg
            .as_deref()
            .map_or_else(FinstackConfig::default, |c| c.clone());
        stamp_results_meta(&effective_cfg, instrument, market, &mut base_result);

        // Scenario price adjustments are a valuation-boundary operation. The
        // model kernel remains unshocked; every result and metric context sees
        // this one adjusted value. A pricer whose detail payload restates the
        // value (the structured-credit stochastic pricer) returns an unshocked
        // headline and shocks that payload itself, so the two agree here.
        base_result.value = lifecycle.apply_value(base_result.value).map_err(|error| {
            PricingError::from_core(
                error,
                crate::pricer::PricingErrorContext::from_instrument(instrument),
            )
        })?;

        base_result.provenance = Some(
            valuation_provenance(
                instrument,
                model,
                requested_as_of,
                &effective_cfg,
                !metrics.is_empty(),
            )
            .map_err(|error| {
                PricingError::from_core(error, PricingErrorContext::from_instrument(instrument))
            })?,
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
        instrument: &dyn Priceable,
        model: ModelKey,
        market: &Market,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<f64, PricingError> {
        let (lifecycle, pricer) = self.validated_pricer(instrument, model, false)?;
        let effective_as_of = lifecycle.effective_as_of(market, as_of);
        let err_ctx = PricingErrorContext::from_instrument(instrument).model(model);
        let base_value = pricer
            .price_raw_dyn(instrument, market, effective_as_of)
            .map_err(|error| error.with_fallback_context(err_ctx))?;
        Ok(lifecycle.apply_raw_value(base_value))
    }
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
/// Record how a result was produced: model, requested date, declared market
/// dependencies, scenario adjustment and (for metric requests) bump sizes.
fn valuation_provenance(
    instrument: &dyn Priceable,
    model: ModelKey,
    requested_as_of: finstack_quant_core::dates::Date,
    cfg: &FinstackConfig,
    with_metrics: bool,
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
        market_dependencies: instrument.market_dependencies()?,
        scenario_price_shock_decimal: instrument
            .get_scenario_pricing_overrides()
            .and_then(|overrides| overrides.scenario_price_shock_decimal),
        sensitivity_bumps,
    })
}

#[doc(hidden)]
pub fn stamp_results_meta(
    cfg: &FinstackConfig,
    instrument: &dyn Priceable,
    market: &finstack_quant_core::market_data::context::MarketContext,
    result: &mut crate::results::ValuationResult,
) {
    let mut previous = result.meta.clone();
    previous.fx_policy_applied = previous
        .fx_policy_applied
        .or_else(|| collect_fx_policy_from_curves(instrument, market));
    result.meta = previous.overlay_request(results_meta(cfg));
}

/// Attach computed metrics without replacing the model-produced result envelope.
///
/// Model measures are inserted last and therefore take precedence when a
/// model and a generic calculator emit the same metric ID.
pub(super) fn attach_metric_measures(
    result: &mut crate::results::ValuationResult,
    mut metric_measures: indexmap::IndexMap<crate::metrics::MetricId, f64>,
) {
    for (metric_id, value) in std::mem::take(&mut result.measures) {
        metric_measures.insert(metric_id, value);
    }
    result.measures = metric_measures;
}

/// Walk an instrument's declared curve dependencies and join any `fx_policy`
/// stamps the curves carry into a single result envelope value.
///
/// Returns `None` when the instrument has no curve dependencies, no dependent
/// curve carries a stamp, or dependency lookup fails. Stamps are de-duplicated
/// in source order.
fn collect_fx_policy_from_curves(
    instrument: &dyn Priceable,
    market: &finstack_quant_core::market_data::context::MarketContext,
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    // ─── Parity tests: instrument trait path vs registry path ────────────────

    #[derive(Clone)]
    struct InvalidTestInstrument {
        id: finstack_quant_core::types::InstrumentId,
        attributes: finstack_quant_core::types::Attributes,
        base_calls: Arc<AtomicUsize>,
        raw_calls: Arc<AtomicUsize>,
    }

    crate::impl_empty_cashflow_provider!(
        InvalidTestInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Priceable for InvalidTestInstrument {
        /// Test mock: reads no market data.
        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<
            crate::instruments::common_impl::dependencies::MarketDependencies,
        > {
            Ok(crate::instruments::common_impl::dependencies::MarketDependencies::new())
        }

        crate::instruments::common_impl::traits::impl_instrument_base!(
            InstrumentType::Bond,
            *crate::pricer::Family::unregistered()
        );

        fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
            Err(finstack_quant_core::Error::Validation(
                "synthetic invalid instrument".to_string(),
            ))
        }

        fn base_value(
            &self,
            _market: &Market,
            _as_of: finstack_quant_core::dates::Date,
        ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
            self.base_calls.fetch_add(1, Ordering::SeqCst);
            Ok(finstack_quant_core::money::Money::from((
                100_i64,
                finstack_quant_core::currency::Currency::USD,
            )))
        }

        fn base_value_raw(
            &self,
            _market: &Market,
            _as_of: finstack_quant_core::dates::Date,
        ) -> finstack_quant_core::Result<f64> {
            self.raw_calls.fetch_add(1, Ordering::SeqCst);
            Ok(100.123_456_789)
        }
    }

    #[derive(Clone)]
    struct RawLifecycleInstrument {
        id: finstack_quant_core::types::InstrumentId,
        attributes: finstack_quant_core::types::Attributes,
        raw_value: f64,
        effective_as_of: finstack_quant_core::dates::Date,
        raw_calls: Arc<AtomicUsize>,
        scenario: crate::instruments::ScenarioPricingOverrides,
    }

    crate::impl_empty_cashflow_provider!(
        RawLifecycleInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Priceable for RawLifecycleInstrument {
        /// Test mock: reads no market data.
        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<
            crate::instruments::common_impl::dependencies::MarketDependencies,
        > {
            Ok(crate::instruments::common_impl::dependencies::MarketDependencies::new())
        }

        crate::instruments::common_impl::traits::impl_instrument_base!(
            InstrumentType::Bond,
            *crate::pricer::Family::unregistered()
        );

        fn resolve_pricing_as_of(
            &self,
            _market: &Market,
            _requested: finstack_quant_core::dates::Date,
        ) -> finstack_quant_core::dates::Date {
            self.effective_as_of
        }

        fn base_value(
            &self,
            _market: &Market,
            as_of: finstack_quant_core::dates::Date,
        ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
            assert_eq!(as_of, self.effective_as_of);
            Ok(finstack_quant_core::money::Money::new(
                self.raw_value,
                finstack_quant_core::currency::Currency::USD,
            )
            .expect("valid money fixture"))
        }

        fn base_value_raw(
            &self,
            _market: &Market,
            as_of: finstack_quant_core::dates::Date,
        ) -> finstack_quant_core::Result<f64> {
            assert_eq!(as_of, self.effective_as_of);
            self.raw_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.raw_value)
        }

        fn base_value_raw_with_currency(
            &self,
            _market: &Market,
            as_of: finstack_quant_core::dates::Date,
        ) -> finstack_quant_core::Result<(f64, finstack_quant_core::currency::Currency)> {
            assert_eq!(as_of, self.effective_as_of);
            self.raw_calls.fetch_add(1, Ordering::SeqCst);
            Ok((self.raw_value, finstack_quant_core::currency::Currency::USD))
        }

        fn get_scenario_pricing_overrides(
            &self,
        ) -> Option<&crate::instruments::ScenarioPricingOverrides> {
            Some(&self.scenario)
        }
    }

    struct CountingBondPricer {
        calls: Arc<AtomicUsize>,
    }

    impl Pricer for CountingBondPricer {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Bond, ModelKey::Discounting)
        }

        fn price_dyn(
            &self,
            instrument: &dyn Priceable,
            _market: &Market,
            as_of: finstack_quant_core::dates::Date,
        ) -> std::result::Result<crate::results::ValuationResult, PricingError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(crate::results::ValuationResult::stamped(
                instrument.id(),
                as_of,
                finstack_quant_core::money::Money::from((
                    100_i64,
                    finstack_quant_core::currency::Currency::USD,
                )),
            ))
        }
    }

    #[test]
    fn canonical_pricing_routes_validate_before_dispatch_or_model_resolution() {
        use crate::instruments::Instrument;
        use time::macros::date;

        let base_calls = Arc::new(AtomicUsize::new(0));
        let raw_calls = Arc::new(AtomicUsize::new(0));
        let pricer_calls = Arc::new(AtomicUsize::new(0));
        let instrument = InvalidTestInstrument {
            id: finstack_quant_core::types::InstrumentId::new("INVALID-TEST"),
            attributes: finstack_quant_core::types::Attributes::default(),
            base_calls: Arc::clone(&base_calls),
            raw_calls: Arc::clone(&raw_calls),
        };
        let market = Market::new();
        let as_of = date!(2025 - 01 - 15);
        let mut registry = PricerRegistry::new();
        registry
            .register(CountingBondPricer {
                calls: Arc::clone(&pricer_calls),
            })
            .expect("unique test pricer registration");

        let registry_err = registry
            .price_with_metrics(
                &instrument,
                ModelKey::Discounting,
                &market,
                as_of,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect_err("invalid registry request must fail validation");
        assert!(matches!(registry_err, PricingError::InvalidInput { .. }));

        let metric_err = registry
            .price_with_metrics(
                &instrument,
                ModelKey::Discounting,
                &market,
                as_of,
                &[crate::metrics::MetricId::Dv01],
                crate::instruments::PricingOptions::default(),
            )
            .expect_err("invalid metric request must fail validation");
        assert!(matches!(metric_err, PricingError::InvalidInput { .. }));

        let raw_err = registry
            .price_raw(&instrument, ModelKey::Discounting, &market, as_of)
            .expect_err("invalid raw request must fail validation");
        assert!(matches!(raw_err, PricingError::InvalidInput { .. }));

        let unknown_model_err = registry
            .price_with_metrics(
                &instrument,
                ModelKey::HazardRate,
                &market,
                as_of,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect_err("validation must precede unknown-model resolution");
        assert!(matches!(
            unknown_model_err,
            PricingError::InvalidInput { .. }
        ));

        let registry = Arc::new(registry);
        let trait_err = instrument
            .price_with_metrics(
                &market,
                as_of,
                &[],
                crate::instruments::PricingOptions::default().with_registry(registry),
            )
            .expect_err("trait pricing request must fail validation");
        assert!(matches!(
            trait_err,
            crate::Error::Pricing(PricingError::InvalidInput { .. })
        ));

        let value_err = instrument
            .value(&market, as_of)
            .expect_err("direct value request must fail validation");
        assert!(matches!(
            value_err,
            finstack_quant_core::Error::Validation(_)
        ));

        let raw_value_err = instrument
            .value_raw(&market, as_of)
            .expect_err("direct raw value request must fail validation");
        assert!(matches!(
            raw_value_err,
            finstack_quant_core::Error::Validation(_)
        ));

        let raw_currency_err = instrument
            .value_raw_with_currency(&market, as_of)
            .expect_err("currency-aware raw value request must fail validation");
        assert!(matches!(
            raw_currency_err,
            finstack_quant_core::Error::Validation(_)
        ));

        assert_eq!(pricer_calls.load(Ordering::SeqCst), 0);
        assert_eq!(base_calls.load(Ordering::SeqCst), 0);
        assert_eq!(raw_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn raw_lifecycle_preserves_precision_resolves_date_and_applies_scenario_once() {
        use crate::instruments::Instrument;
        use time::macros::date;

        let requested_as_of = date!(2025 - 01 - 15);
        let effective_as_of = date!(2025 - 01 - 17);
        let raw_value = 123.456_789_123;
        let raw_calls = Arc::new(AtomicUsize::new(0));
        let instrument = RawLifecycleInstrument {
            id: finstack_quant_core::types::InstrumentId::new("RAW-LIFECYCLE"),
            attributes: finstack_quant_core::types::Attributes::default(),
            raw_value,
            effective_as_of,
            raw_calls: Arc::clone(&raw_calls),
            scenario: crate::instruments::ScenarioPricingOverrides::default()
                .with_scenario_price_shock_decimal(-0.10),
        };
        let market = Market::new();
        let mut registry = PricerRegistry::new();
        registry
            .register(crate::instruments::common_impl::GenericInstrumentPricer::<
                RawLifecycleInstrument,
            >::discounting(InstrumentType::Bond))
            .expect("unique test pricer registration");

        let base_raw = instrument
            .base_value_raw(&market, effective_as_of)
            .expect("unchecked raw kernel");
        let direct_raw = instrument
            .value_raw(&market, requested_as_of)
            .expect("direct raw lifecycle");
        let (direct_raw_with_currency, direct_currency) = instrument
            .value_raw_with_currency(&market, requested_as_of)
            .expect("currency-aware direct raw lifecycle");
        let registry_raw = registry
            .price_raw(&instrument, ModelKey::Discounting, &market, requested_as_of)
            .expect("registry raw lifecycle");
        let result = registry
            .price_with_metrics(
                &instrument,
                ModelKey::Discounting,
                &market,
                requested_as_of,
                &[],
                crate::instruments::PricingOptions::default(),
            )
            .expect("registry money lifecycle");
        let expected_money = result.value;
        let mut metric_context = crate::metrics::MetricContext::new(
            Arc::new(instrument),
            Arc::new(market.clone()),
            effective_as_of,
            expected_money,
            crate::metrics::MetricContext::default_config(),
        );
        metric_context.set_pricer_dispatch(PricingDispatch::registered(
            ModelKey::Discounting,
            Arc::new(registry),
        ));
        let metric_reprice = metric_context
            .reprice_money(&market, requested_as_of)
            .expect("metric scenario lifecycle");

        assert_eq!(base_raw, raw_value);
        let expected = raw_value * 0.9;
        assert_eq!(direct_raw, expected);
        assert_eq!(direct_raw_with_currency, expected);
        assert_eq!(
            direct_currency,
            finstack_quant_core::currency::Currency::USD
        );
        assert_eq!(registry_raw, expected);
        assert_eq!(result.as_of, effective_as_of);
        assert_eq!(metric_reprice, expected_money);
        assert_eq!(raw_calls.load(Ordering::SeqCst), 4);
    }
}

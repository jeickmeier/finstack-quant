//! Core traits for the metrics framework.
//!
//! Defines the fundamental interfaces for implementing and using financial
//! metrics. The `MetricCalculator` trait enables custom metric implementations,
//! while `MetricContext` provides the execution environment with caching.

use crate::instruments::Instrument;
use crate::metrics::risk::MarketHistory;
use crate::metrics::MetricId;
use crate::pricer::PricingDispatch;
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::cashflow::CashFlow;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;

use finstack_quant_core::config::FinstackConfig;
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::sync::Arc;

/// Core trait for metric calculators.
///
/// Each calculator computes a single metric value based on the provided context.
/// Calculators can declare dependencies on other metrics for efficient computation
/// ordering and caching. Implement this trait to create custom financial metrics.
///
/// See unit tests and `examples/` for usage.
pub trait MetricCalculator: Send + Sync {
    /// Computes the metric value based on the provided context.
    ///
    /// This method should implement the core calculation logic for the metric.
    /// It can access cached results from `context.computed` for dependencies.
    ///
    /// # Arguments
    /// * `context` - Metric context containing instrument, market data, and cached results
    ///
    /// # Returns
    /// The computed metric value as a `Result<f64>`
    ///
    /// # Errors
    /// Returns an error if the metric cannot be computed due to missing data
    /// or invalid instrument configuration.
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64>;

    /// Lists metric IDs this calculator depends on.
    ///
    /// Dependencies will be computed first and made available via
    /// `context.computed`. The registry uses this to determine computation order.
    ///
    /// # Returns
    /// Slice of metric IDs that must be computed before this metric
    fn dependencies(&self) -> &[MetricId] {
        &[]
    }

    /// Lists metric IDs this calculator depends on, given the runtime context.
    ///
    /// Override this when the dependency set is not statically known — for
    /// example when it varies with instrument-level pricing overrides. The
    /// registry calls this (not [`dependencies`](Self::dependencies)) when
    /// building the computation order, so a calculator that reads
    /// `context.computed` for a config-dependent metric **must** declare it
    /// here or it will be missing whenever the caller did not happen to
    /// request it earlier in the list.
    ///
    /// The default implementation defers to
    /// [`dependencies`](Self::dependencies), so calculators with a static
    /// dependency set need not implement this.
    ///
    /// # Arguments
    /// * `context` - Metric context, for inspecting the instrument and overrides
    ///
    /// # Returns
    /// Metric IDs that must be computed before this metric
    fn dynamic_dependencies<'a>(&'a self, _context: &MetricContext) -> Cow<'a, [MetricId]> {
        Cow::Borrowed(self.dependencies())
    }
}

/// Generic 2D structured metric container.
///
/// Rows and columns are labeled; values are a rectangular matrix of size
/// `rows.len() x cols.len()`.
#[derive(Debug, Clone)]
struct Structured2D {
    /// Row labels (e.g., expiries, tenors)
    rows: Vec<String>,
    /// Column labels (e.g., strikes, bumps)
    cols: Vec<String>,
    /// Matrix values; `values[r][c]` corresponds to `rows[r]`, `cols[c]`
    values: Vec<Vec<f64>>,
}

impl Structured2D {
    /// Describes why the matrix shape is invalid.
    fn shape_error(&self) -> Option<String> {
        if self.rows.is_empty() || self.cols.is_empty() {
            return Some(format!(
                "2D structured metric must have non-empty rows and columns (rows={}, cols={})",
                self.rows.len(),
                self.cols.len()
            ));
        }
        if self.values.len() != self.rows.len() {
            return Some(format!(
                "2D structured metric row count mismatch: rows={}, value_rows={}",
                self.rows.len(),
                self.values.len()
            ));
        }
        let expected_cols = self.cols.len();
        for (idx, row) in self.values.iter().enumerate() {
            if row.len() != expected_cols {
                return Some(format!(
                    "2D structured metric column count mismatch at row {idx}: cols={}, value_cols={}",
                    expected_cols,
                    row.len()
                ));
            }
        }
        None
    }
}

/// Immutable inputs shared by every calculator in one metric request.
///
/// Not re-exported from [`crate::metrics`]; the registry → metric boundary
/// uses [`crate::instruments::PricingOptions`] plus crate-private
/// [`crate::instruments::common_impl::helpers::MetricBuildOptions`].
pub struct MetricPricingInputs {
    /// Instrument being valued.
    pub instrument: Arc<dyn Instrument>,
    /// Immutable market snapshot.
    pub curves: Arc<MarketContext>,
    /// Optional historical scenarios used by historical risk metrics.
    market_history: Option<Arc<MarketHistory>>,
    /// Pricing path reused for every revaluation.
    pricing_dispatch: PricingDispatch,
    /// Valuation date.
    pub as_of: Date,
    /// Base present value.
    pub base_value: Money,
    /// Metric-only risk overrides.
    metric_pricing_overrides: Option<crate::instruments::pricing_overrides::MetricPricingOverrides>,
    /// Shared numerical and reporting configuration.
    finstack_config: Arc<FinstackConfig>,
}

/// State a pricer computed while producing a base value and hands to metric
/// evaluation, so metric calculators reuse it instead of recomputing it.
///
/// A pricer attaches one through
/// [`ValuationResult::with_metric_seed`](crate::results::ValuationResult::with_metric_seed);
/// the registry installs it into the [`MetricContext`] before the instrument's
/// own [`seed_metric_context`](crate::instruments::Instrument::seed_metric_context)
/// runs. The seed belongs to that one valuation request (instrument, market and
/// as-of date) and is never serialized.
pub trait MetricSeed: std::fmt::Debug + Send + Sync {
    /// Write the pricer's state into `context`.
    ///
    /// # Arguments
    ///
    /// * `context` - Metric context of the valuation request that produced
    ///   this seed, before the instrument seeds it.
    fn seed(&self, context: &mut MetricContext);
}

/// Context containing all data needed for metric calculations.
///
/// Provides access to the instrument, market data, base valuation,
/// and any previously computed metrics. Supports caching of intermediate
/// results like cashflows and discount factors to improve performance.
///
/// # Key Features
///
/// - **Instrument data**: Access to the instrument being valued
/// - **Market curves**: Discount and forward curves for calculations
/// - **Cached results**: Previously computed metrics for dependency resolution
/// - **Cashflow caching**: Optional caching of instrument cashflows
/// - **Metadata**: Discount curve ID and day count convention
pub struct MetricContext {
    /// Immutable pricing inputs. Field access remains available through
    /// [`Deref`](std::ops::Deref), while mutation is restricted to setup methods.
    inputs: MetricPricingInputs,

    /// Reusable mutable market snapshot for finite-difference calculations.
    market_scratch: Option<MarketContext>,

    /// Provider for quote-space curve recalibration.
    recalibration_provider: Option<Arc<dyn crate::recalibration::RecalibrationProvider>>,

    /// Previously computed metrics (by ID).
    pub computed: finstack_quant_core::FxHashMap<MetricId, f64>,

    /// Previously computed 1D bucketed metrics (by ID).
    ///
    /// Example: `MetricId::BucketedDv01` -> [("1m", v1), ("3m", v2), ...]
    pub computed_series: finstack_quant_core::FxHashMap<MetricId, Vec<(String, f64)>>,

    /// Cached cashflows for the instrument.
    pub cashflows: Option<Vec<(Date, Money)>>,

    /// Cached detailed cashflows with CFKind metadata.
    pub tagged_cashflows: Option<Vec<CashFlow>>,

    /// Cached internal cashflow schedule with full structural metadata
    /// (notional path, principal events, funding legs).
    ///
    /// Populated lazily by instrument-specific callers when several metric
    /// calculators need the same expensive schedule build (e.g., term loan
    /// YTM/YTC/YTW/DM/all-in-rate all consume the same `CashFlowSchedule`).
    /// Stored as `Arc` so callers can hand out cheap clones without holding
    /// a long-lived borrow of the context. The cache is keyed implicitly to
    /// a single `(instrument, context.curves, as_of)` evaluation — DO NOT
    /// reuse a `MetricContext` across different markets or as-of dates.
    /// Bump-and-reprice paths (DV01/CS01) sidestep this safely because they
    /// call `reprice_raw(bumped_market, …)` which goes through
    /// `Instrument::value_raw` directly without consulting the cache.
    pub internal_schedule: Option<Arc<CashFlowSchedule>>,

    /// Family-owned caches keyed by type; see [`MetricContext::ext`].
    extensions: finstack_quant_core::FxHashMap<TypeId, Box<dyn Any + Send + Sync>>,

    /// Cached discount curve ID.
    pub discount_curve_id: Option<CurveId>,

    /// Cached day count convention.
    pub day_count: Option<DayCount>,

    /// Original notional amount for price calculations.
    ///
    /// For structured credit: typically pool original balance or tranche original balance.
    /// For bonds: face amount. For other instruments: principal amount.
    /// Used by price calculators to avoid instrument downcasts.
    pub notional: Option<Money>,
}

impl std::ops::Deref for MetricContext {
    type Target = MetricPricingInputs;

    fn deref(&self) -> &Self::Target {
        &self.inputs
    }
}

impl MetricContext {
    /// Returns a new [`Arc`] containing the default [`FinstackConfig`].
    #[inline]
    pub fn default_config() -> Arc<FinstackConfig> {
        Arc::new(FinstackConfig::default())
    }

    /// Creates a new metric context.
    ///
    /// # Arguments
    /// * `instrument` - The instrument to value
    /// * `curves` - Market curves for discounting and forwarding
    /// * `as_of` - Valuation date
    /// * `base_value` - Base present value of the instrument
    /// * `finstack_config` - Shared configuration controlling tolerances and feature flags
    ///
    /// See unit tests and `examples/` for usage.
    pub fn new(
        instrument: Arc<dyn Instrument>,
        curves: Arc<MarketContext>,
        as_of: Date,
        base_value: Money,
        finstack_config: Arc<FinstackConfig>,
    ) -> Self {
        Self {
            inputs: MetricPricingInputs {
                instrument,
                curves,
                market_history: None,
                pricing_dispatch: PricingDispatch::InstrumentDefault,
                as_of,
                base_value,
                metric_pricing_overrides: None,
                finstack_config,
            },
            market_scratch: None,
            recalibration_provider: None,
            computed: finstack_quant_core::FxHashMap::default(),
            computed_series: finstack_quant_core::FxHashMap::default(),
            cashflows: None,
            tagged_cashflows: None,
            internal_schedule: None,
            extensions: finstack_quant_core::FxHashMap::default(),
            discount_curve_id: None,
            day_count: None,
            notional: None,
        }
    }

    /// Run a finite-difference calculation against a reusable scratch market.
    ///
    /// On success, retain the scratch market for the next calculation. On error,
    /// discard it because the calculation may have left a bump unreverted.
    ///
    /// # Arguments
    ///
    /// * `f` - Calculation borrowing this metric context and its mutable market
    ///   scratch copy. Restore every market bump before returning `Ok`; an `Err`
    ///   discards the copy and the next call starts from the original market.
    #[inline]
    pub fn with_market_scratch<T>(
        &mut self,
        f: impl FnOnce(&Self, &mut MarketContext) -> finstack_quant_core::Result<T>,
    ) -> finstack_quant_core::Result<T> {
        let mut scratch = self
            .market_scratch
            .take()
            .unwrap_or_else(|| self.curves.as_ref().clone());
        let result = f(self, &mut scratch);
        if result.is_ok() {
            self.market_scratch = Some(scratch);
        } else {
            // The failing path may have exited before reverting a bump token.
            // Discard the scratch copy instead of caching a contaminated market.
            self.market_scratch = None;
        }
        result
    }

    /// Access the finstack configuration associated with this context.
    #[inline]
    pub fn get_config(&self) -> &FinstackConfig {
        &self.finstack_config
    }

    /// Clone the shared finstack configuration.
    #[inline]
    pub fn config_arc(&self) -> Arc<FinstackConfig> {
        Arc::clone(&self.finstack_config)
    }

    /// Returns the metric-only overrides, if any.
    #[inline]
    pub fn get_metric_pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::MetricPricingOverrides> {
        self.metric_pricing_overrides.as_ref()
    }

    /// Read a metric already computed for this request.
    ///
    /// Calculators that declare `id` as a dependency read its value here; the
    /// registry computes dependencies first, so a miss means the dependency was
    /// not requested or failed.
    ///
    /// # Arguments
    ///
    /// * `id` - Metric to look up in [`Self::computed`]; its value is returned
    ///   in the metric's own unit (see [`MetricId::unit`]).
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::InputError::NotFound`] built by
    /// [`crate::metrics::metric_not_found`] when `id` has no computed value.
    #[inline]
    pub fn computed_metric(&self, id: &MetricId) -> finstack_quant_core::Result<f64> {
        self.computed
            .get(id)
            .copied()
            .ok_or_else(|| crate::metrics::metric_not_found(id.clone()))
    }

    /// Borrow the instrument's cached holder-view cashflows.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::InputError::NotFound`] with id
    /// `context.cashflows` when the pricer did not populate
    /// [`Self::cashflows`].
    #[inline]
    pub fn require_cashflows(&self) -> finstack_quant_core::Result<&[(Date, Money)]> {
        self.cashflows
            .as_deref()
            .ok_or_else(|| crate::metrics::context_not_found("cashflows"))
    }

    /// Borrow the discount curve identifier the pricer recorded for this request.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::InputError::NotFound`] with id
    /// `discount_curve_id` when [`Self::discount_curve_id`] is unset.
    #[inline]
    pub fn require_discount_curve_id(&self) -> finstack_quant_core::Result<&CurveId> {
        self.discount_curve_id.as_ref().ok_or_else(|| {
            finstack_quant_core::InputError::NotFound {
                id: "discount_curve_id".to_string(),
            }
            .into()
        })
    }

    /// Returns a reference to the market history, if set.
    #[inline]
    pub fn get_market_history(&self) -> Option<&MarketHistory> {
        self.market_history.as_deref()
    }

    /// Clone the injected provider for nested historical-risk calculations.
    pub fn get_recalibration_provider(
        &self,
    ) -> Option<Arc<dyn crate::recalibration::RecalibrationProvider>> {
        self.recalibration_provider.clone()
    }

    /// Attach the batch-local quote recalibration provider.
    pub fn set_recalibration_provider(
        &mut self,
        provider: Option<Arc<dyn crate::recalibration::RecalibrationProvider>>,
    ) {
        self.recalibration_provider = provider;
    }

    /// Recalibrate linked discount and forward curves for a parallel quote bump.
    pub fn bump_rate_market_cached(
        &self,
        discount_curve_id: &CurveId,
        forward_curve_id: &CurveId,
        bump_bp: f64,
    ) -> finstack_quant_core::Result<Arc<MarketContext>> {
        self.rebuild_rate_market(
            crate::recalibration::RateMarketRecalibrationRequest::LinkedDiscountForward {
                market: Arc::clone(&self.curves),
                discount_curve_id: discount_curve_id.clone(),
                forward_curve_id: forward_curve_id.clone(),
                bump: crate::recalibration::QuoteBump::ParallelBp(bump_bp),
            },
            "dv01",
        )
    }

    /// Recalibrate a single OIS curve for a parallel quote bump.
    pub fn bump_single_ois_rate_market_cached(
        &self,
        curve_id: &CurveId,
        bump_bp: f64,
    ) -> finstack_quant_core::Result<Arc<MarketContext>> {
        self.rebuild_rate_market(
            crate::recalibration::RateMarketRecalibrationRequest::SingleOis {
                market: Arc::clone(&self.curves),
                curve_id: curve_id.clone(),
                bump: crate::recalibration::QuoteBump::ParallelBp(bump_bp),
            },
            "dv01",
        )
    }

    /// Execute one rate-market replay through the injected provider.
    pub fn rebuild_rate_market(
        &self,
        request: crate::recalibration::RateMarketRecalibrationRequest,
        operation: &str,
    ) -> finstack_quant_core::Result<Arc<MarketContext>> {
        self.recalibration_provider_ref(operation)?
            .rebuild_rate_market(&request)
    }

    /// Execute one discount-curve replay through the injected provider.
    pub fn rebuild_discount_curve(
        &self,
        request: crate::recalibration::DiscountCurveRecalibrationRequest,
        operation: &str,
    ) -> finstack_quant_core::Result<
        Arc<finstack_quant_core::market_data::term_structures::DiscountCurve>,
    > {
        self.recalibration_provider_ref(operation)?
            .rebuild_discount_curve(&request)
    }

    /// Return exact ordered quote bindings for bucketed spread risk.
    pub fn hazard_spread_risk_buckets(
        &self,
        hazard: &finstack_quant_core::market_data::term_structures::HazardCurve,
    ) -> finstack_quant_core::Result<Vec<crate::recalibration::HazardSpreadRiskBucket>> {
        self.recalibration_provider_ref("bucketed_cs01")?
            .hazard_spread_risk_buckets(hazard)
    }

    /// Execute one hazard replay through the injected provider.
    pub fn rebuild_hazard_curve(
        &self,
        request: crate::recalibration::HazardRecalibrationRequest,
        operation: &str,
    ) -> finstack_quant_core::Result<
        Arc<finstack_quant_core::market_data::term_structures::HazardCurve>,
    > {
        self.recalibration_provider_ref(operation)?
            .rebuild_hazard_curve(&request)
    }

    fn recalibration_provider_ref(
        &self,
        operation: &str,
    ) -> finstack_quant_core::Result<&dyn crate::recalibration::RecalibrationProvider> {
        self.recalibration_provider
            .as_deref()
            .ok_or_else(|| crate::recalibration::provider_missing(operation))
    }

    /// Clone the injected provider for nested pricing operations.
    pub fn recalibration_provider(
        &self,
        operation: &str,
    ) -> finstack_quant_core::Result<Arc<dyn crate::recalibration::RecalibrationProvider>> {
        self.recalibration_provider
            .as_ref()
            .cloned()
            .ok_or_else(|| crate::recalibration::provider_missing(operation))
    }

    /// Clone the pricing dispatch for use in sub-contexts.
    #[inline]
    pub fn clone_pricer_dispatch(&self) -> PricingDispatch {
        self.pricing_dispatch.clone()
    }

    /// Return the explicitly selected pricing model, if this metric request
    /// was entered through the registered-pricer path.
    #[inline]
    pub fn pricing_model(&self) -> Option<crate::pricer::ModelKey> {
        self.pricing_dispatch.model()
    }

    /// Attach market history to this context (used by Historical VaR metrics).
    pub fn with_market_history(mut self, history: Arc<MarketHistory>) -> Self {
        self.inputs.market_history = Some(history);
        self
    }

    /// Set the pricing path reused by every downstream repricing operation.
    pub fn set_pricer_dispatch(&mut self, dispatch: PricingDispatch) {
        self.inputs.pricing_dispatch = dispatch;
    }

    /// Set metric-only overrides used by downstream calculators.
    pub fn set_metric_pricing_overrides(
        &mut self,
        overrides: Option<crate::instruments::pricing_overrides::MetricPricingOverrides>,
    ) {
        self.inputs.metric_pricing_overrides = overrides;
    }

    /// Temporarily replace the instrument view during a scoped metric calculation.
    ///
    /// Every family cache stored with [`MetricContext::set_ext`] describes the
    /// previous instrument view, so all of them are dropped.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Instrument the context evaluates from now on; the
    ///   caller restores the original view when the scoped calculation ends.
    pub fn set_instrument(&mut self, instrument: Arc<dyn Instrument>) {
        self.inputs.instrument = instrument;
        self.extensions.clear();
    }

    /// Borrow the family cache of type `T` stored on this context, if any.
    ///
    /// Instrument families keep intermediate results that several of their
    /// calculators share (a bond's quoted workout path, a deal's projected
    /// tranche cashflows) here, keyed by the cache's own type, so the
    /// context itself names no family. A cache is valid for the current
    /// instrument view only; [`MetricContext::set_instrument`] drops them all.
    ///
    /// # Returns
    ///
    /// The stored value when a cache of exactly type `T` was set.
    pub fn ext<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.extensions
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<T>())
    }

    /// Store `value` as the family cache of type `T`, replacing any previous
    /// value of that type.
    ///
    /// # Arguments
    ///
    /// * `value` - Cache contents for the current instrument view; later
    ///   [`MetricContext::ext`] calls with the same type return it.
    pub fn set_ext<T: Any + Send + Sync>(&mut self, value: T) {
        self.extensions.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Replace the request's base value during test or nested metric setup.
    ///
    /// # Arguments
    ///
    /// * `base_value` - Present value the dependent calculators read as the
    ///   instrument's base value from now on.
    #[doc(hidden)]
    pub fn set_base_value(&mut self, base_value: Money) {
        self.inputs.base_value = base_value;
    }

    /// Reprice the context instrument using the active dispatch path.
    pub fn reprice_money(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        self.reprice_instrument_money(self.instrument.as_ref(), market, as_of)
    }

    /// Reprice the context instrument as a raw amount using the active dispatch path.
    pub fn reprice_raw(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.reprice_instrument_raw(self.instrument.as_ref(), market, as_of)
    }

    /// Reprice an arbitrary instrument using the active dispatch path.
    pub fn reprice_instrument_money(
        &self,
        instrument: &dyn Instrument,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let options = crate::instruments::PricingOptions::default().with_config(self.get_config());
        self.pricing_dispatch
            .price_money(instrument, market, as_of, options)
    }

    /// Reprice an arbitrary instrument as a raw amount using the active dispatch path.
    pub fn reprice_instrument_raw(
        &self,
        instrument: &dyn Instrument,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.pricing_dispatch.price_raw(instrument, market, as_of)
    }

    /// Return the instrument's canonical cashflow schedule flows with CFKind metadata.
    pub fn tagged_cashflows_cached(&mut self) -> finstack_quant_core::Result<&Vec<CashFlow>> {
        if self.tagged_cashflows.is_none() {
            let schedule = self
                .instrument
                .cashflow_schedule(&self.curves, self.as_of)?;
            self.tagged_cashflows = Some(schedule.into_flows());
        }
        self.tagged_cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation("tagged cashflows were not cached".to_string())
        })
    }

    /// The context market with every coupon fixing the canonical schedule
    /// projected in `[as_of, horizon]` added as an exact-date `FIXING:`
    /// observation at its as-of projection, for repricing at `horizon` with
    /// the market held fixed.
    ///
    /// Existing observations keep priority and the context market is
    /// unchanged. Theta rolls the returned market's curves; carry
    /// decomposition and iterative breakeven reprice on it unrolled.
    ///
    /// # Arguments
    ///
    /// * `horizon` - Date the caller reprices at, on or after `as_of`. An
    ///   equal date returns the market unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error when the schedule cannot be built or a crossed
    /// fixing has no finite as-of projection (see
    /// `finstack_quant_cashflows::fixings::materialize_fixings`).
    pub fn fixings_held_market(&self, horizon: Date) -> finstack_quant_core::Result<MarketContext> {
        let schedule = self
            .instrument
            .cashflow_schedule(&self.curves, self.as_of)?;
        finstack_quant_cashflows::fixings::materialize_fixings(
            &self.curves,
            [&schedule],
            self.as_of,
            horizon,
        )
    }

    /// Downcast the instrument to a specific concrete type.
    ///
    /// # Returns
    /// Reference to the concrete instrument type if the downcast succeeds
    ///
    /// # Errors
    /// Returns an error if the instrument is not of the expected type
    #[inline(never)] // Prevent inlining to reduce coverage metadata conflicts
    pub fn instrument_as<T: 'static>(&self) -> finstack_quant_core::Result<&T> {
        self.instrument.as_any().downcast_ref::<T>().ok_or_else(|| {
            finstack_quant_core::InputError::NotFound {
                id: format!(
                    "instrument downcast: expected {}, got {} (id={})",
                    std::any::type_name::<T>(),
                    self.instrument.key(),
                    self.instrument.id(),
                ),
            }
            .into()
        })
    }

    /// Store a 1D bucketed series under `base_metric_id` and flatten into
    /// `computed` using a stable composite key per bucket.
    pub fn store_bucketed_series<I, K>(&mut self, base_metric_id: MetricId, series: I)
    where
        I: IntoIterator<Item = (K, f64)>,
        K: Into<String>,
    {
        let collected: Vec<(String, f64)> =
            series.into_iter().map(|(k, v)| (k.into(), v)).collect();

        for (label, value) in &collected {
            let key = MetricId::composite(&base_metric_id, &[label.as_str()]);
            self.computed.insert(key, *value);
        }

        self.computed_series.insert(base_metric_id, collected);
    }

    /// Store a 2D structured metric (rows x cols) under `base_metric_id` and
    /// flatten each cell into `computed` using stable composite keys
    /// `base::row::col`.
    pub fn store_matrix2d<I, J, RS, CS>(
        &mut self,
        base_metric_id: MetricId,
        rows: I,
        cols: J,
        values: Vec<Vec<f64>>,
    ) -> finstack_quant_core::Result<()>
    where
        I: IntoIterator<Item = RS>,
        J: IntoIterator<Item = CS>,
        RS: Into<String>,
        CS: Into<String>,
    {
        let rows: Vec<String> = rows.into_iter().map(Into::into).collect();
        let cols: Vec<String> = cols.into_iter().map(Into::into).collect();
        let matrix = Structured2D { rows, cols, values };
        if let Some(reason) = matrix.shape_error() {
            return Err(finstack_quant_core::Error::Validation(reason));
        }
        for (r_idx, r_label) in matrix.rows.iter().enumerate() {
            for (c_idx, c_label) in matrix.cols.iter().enumerate() {
                let key =
                    MetricId::composite(&base_metric_id, &[r_label.as_str(), c_label.as_str()]);
                self.computed.insert(key, matrix.values[r_idx][c_idx]);
            }
        }
        Ok(())
    }

    /// Retrieves a previously stored 1D bucketed series.
    pub fn get_series(&self, id: &MetricId) -> Option<&[(String, f64)]> {
        self.computed_series.get(id).map(|v| v.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_composite_key_preserves_distinct_non_alphanumeric_labels() {
        let hyphen = MetricId::composite(&MetricId::BucketedDv01, &["USD-OIS"]);
        let underscore = MetricId::composite(&MetricId::BucketedDv01, &["USD_OIS"]);

        assert_ne!(hyphen, underscore);
        assert_eq!(hyphen.as_str(), "bucketed_dv01::USD-OIS");
        assert_eq!(underscore.as_str(), "bucketed_dv01::USD_OIS");
    }
}

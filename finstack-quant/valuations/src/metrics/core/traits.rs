//! Core traits for the metrics framework.
//!
//! Defines the fundamental interfaces for implementing and using financial
//! metrics. The `MetricCalculator` trait enables custom metric implementations,
//! while `MetricContext` provides the execution environment with caching.

use crate::cashflow::builder::schedule::CashFlowSchedule;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::structured_credit::TrancheCashflows;
use crate::metrics::risk::MarketHistory;
use crate::metrics::MetricId;
use crate::pricer::{ModelKey, PricerRegistry};
use finstack_quant_core::cashflow::CashFlow;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;

use finstack_quant_core::config::FinstackConfig;
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
pub struct Structured2D {
    /// Row labels (e.g., expiries, tenors)
    pub rows: Vec<String>,
    /// Column labels (e.g., strikes, bumps)
    pub cols: Vec<String>,
    /// Matrix values; `values[r][c]` corresponds to `rows[r]`, `cols[c]`
    pub values: Vec<Vec<f64>>,
}

impl Structured2D {
    /// Validates that `values` is a rectangular matrix matching label sizes.
    pub fn validate_shape(&self) -> bool {
        self.shape_error().is_none()
    }

    /// Describes why the matrix shape is invalid.
    pub fn shape_error(&self) -> Option<String> {
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
    /// The instrument being valued.
    pub instrument: Arc<dyn Instrument>,

    /// Market curves for discounting and forwarding.
    pub curves: Arc<MarketContext>,

    /// Reusable scratch copy of [`Self::curves`] for finite-difference bump/reprice metrics.
    ///
    /// A metric set shares one [`MetricContext`], so this cache avoids cloning
    /// the full market container separately for each Greek/DV01 calculator.
    market_scratch: Option<MarketContext>,

    /// Batch-local hazard recalibrations shared across position metric contexts.
    hazard_recalibration_cache:
        Option<Arc<crate::calibration::bumps::hazard::HazardRecalibrationCache>>,

    /// Optional market history for historical scenario revaluation (e.g., Historical VaR).
    ///
    /// This is intentionally **not** stored inside `finstack_quant_core::MarketContext` to keep
    /// the core market container strongly typed and fully serializable.
    market_history: Option<Arc<MarketHistory>>,

    /// Pricing model to reuse for bump-and-reprice metrics.
    pricing_model: Option<ModelKey>,

    /// Pricer registry to reuse for bump-and-reprice metrics.
    pricer_registry: Option<Arc<PricerRegistry>>,

    /// Valuation date.
    pub as_of: Date,

    /// Base present value of the instrument.
    pub base_value: Money,

    /// Previously computed metrics (by ID).
    pub computed: finstack_quant_core::HashMap<MetricId, f64>,

    /// Previously computed 1D bucketed metrics (by ID).
    ///
    /// Example: `MetricId::BucketedDv01` -> [("1m", v1), ("3m", v2), ...]
    pub computed_series: finstack_quant_core::HashMap<MetricId, Vec<(String, f64)>>,

    /// Previously computed 2D structured metrics (by ID).
    ///
    /// Example: vega surface with rows=expiries, cols=strikes
    pub computed_matrix: finstack_quant_core::HashMap<MetricId, Structured2D>,

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
    pub(crate) internal_schedule: Option<Arc<CashFlowSchedule>>,

    /// Tranche-level detailed cashflow results (for structured credit)
    pub detailed_tranche_cashflows: Option<TrancheCashflows>,

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

    /// Optional instrument-owned pricing inputs needed by specific metrics.
    instrument_overrides: Option<crate::instruments::InstrumentPricingOverrides>,

    /// Optional metric-only overrides to control risk calculations (e.g., bumps, theta horizon).
    metric_overrides: Option<crate::instruments::MetricPricingOverrides>,

    /// Finstack configuration (tolerances + versioned extensions).
    ///
    /// This is used by metric calculators to resolve user-facing defaults
    /// (e.g., risk bump sizes) and to keep results reproducible.
    finstack_config: Arc<FinstackConfig>,
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
            instrument,
            curves,
            market_scratch: None,
            hazard_recalibration_cache: None,
            market_history: None,
            pricing_model: None,
            pricer_registry: None,
            as_of,
            base_value,
            computed: finstack_quant_core::HashMap::default(),
            computed_series: finstack_quant_core::HashMap::default(),
            computed_matrix: finstack_quant_core::HashMap::default(),
            cashflows: None,
            tagged_cashflows: None,
            internal_schedule: None,
            detailed_tranche_cashflows: None,
            discount_curve_id: None,
            day_count: None,
            notional: None,
            instrument_overrides: None,
            metric_overrides: None,
            finstack_config,
        }
    }

    /// Take the reusable market scratch context, cloning the base market only
    /// the first time a finite-difference metric needs a mutable copy.
    #[inline]
    pub(crate) fn take_market_scratch(&mut self) -> MarketContext {
        self.market_scratch
            .take()
            .unwrap_or_else(|| self.curves.as_ref().clone())
    }

    /// Return an unbumped scratch context for reuse by the next metric.
    #[inline]
    pub(crate) fn put_market_scratch(&mut self, scratch: MarketContext) {
        self.market_scratch = Some(scratch);
    }

    /// Run a finite-difference calculation against the reusable scratch market
    /// and return it to the context even when the calculation fails.
    #[inline]
    pub(crate) fn with_market_scratch<T>(
        &mut self,
        f: impl FnOnce(&Self, &mut MarketContext) -> finstack_quant_core::Result<T>,
    ) -> finstack_quant_core::Result<T> {
        let mut scratch = self.take_market_scratch();
        let result = f(self, &mut scratch);
        if result.is_ok() {
            self.put_market_scratch(scratch);
        } else {
            // The failing path may have exited before reverting a bump token.
            // Discard the scratch copy instead of caching a contaminated market.
            self.market_scratch = None;
        }
        result
    }

    /// Access the finstack configuration associated with this context.
    #[inline]
    pub fn config(&self) -> &FinstackConfig {
        &self.finstack_config
    }

    /// Clone the shared finstack configuration.
    #[inline]
    pub fn config_arc(&self) -> Arc<FinstackConfig> {
        Arc::clone(&self.finstack_config)
    }

    /// Returns the metric-only overrides, if any.
    #[inline]
    pub(crate) fn get_metric_overrides(
        &self,
    ) -> Option<&crate::instruments::MetricPricingOverrides> {
        self.metric_overrides.as_ref()
    }

    /// Returns the instrument-owned pricing overrides, if any.
    #[inline]
    pub(crate) fn get_instrument_overrides(
        &self,
    ) -> Option<&crate::instruments::InstrumentPricingOverrides> {
        self.instrument_overrides.as_ref()
    }

    /// Returns a reference to the market history, if set.
    #[inline]
    pub(crate) fn get_market_history(&self) -> Option<&MarketHistory> {
        self.market_history.as_deref()
    }

    /// Attach the batch-local hazard recalibration cache.
    pub(crate) fn set_hazard_recalibration_cache(
        &mut self,
        cache: Option<Arc<crate::calibration::bumps::hazard::HazardRecalibrationCache>>,
    ) {
        self.hazard_recalibration_cache = cache;
    }

    /// Recalibrate a hazard curve, reusing an identical batch-local result.
    pub(crate) fn bump_hazard_spreads_cached(
        &self,
        hazard: &finstack_quant_core::market_data::term_structures::HazardCurve,
        market: &MarketContext,
        bump: &crate::calibration::bumps::BumpRequest,
        discount_id: Option<&CurveId>,
        doc_clause: Option<crate::market::conventions::ids::CdsDocClause>,
        cds_valuation_convention: Option<
            crate::instruments::credit_derivatives::cds::CdsValuationConvention,
        >,
    ) -> finstack_quant_core::Result<
        Arc<finstack_quant_core::market_data::term_structures::HazardCurve>,
    > {
        crate::calibration::bumps::hazard::bump_hazard_spreads_cached(
            self.hazard_recalibration_cache.as_deref(),
            hazard,
            market,
            bump,
            discount_id,
            doc_clause,
            cds_valuation_convention,
        )
    }

    /// Clones the pricing dispatch pair (model + registry) for use in sub-contexts.
    #[inline]
    pub(crate) fn clone_pricer_dispatch(&self) -> (Option<ModelKey>, Option<Arc<PricerRegistry>>) {
        (self.pricing_model, self.pricer_registry.clone())
    }

    /// Attach market history to this context (used by Historical VaR metrics).
    pub fn with_market_history(mut self, history: Arc<MarketHistory>) -> Self {
        self.market_history = Some(history);
        self
    }

    /// Reuse a specific pricer registry/model pair for metric repricing.
    pub fn set_pricer_dispatch(
        &mut self,
        pricing_model: Option<ModelKey>,
        pricer_registry: Option<Arc<PricerRegistry>>,
    ) {
        self.pricing_model = pricing_model;
        self.pricer_registry = pricer_registry;
    }

    /// Set instrument-owned pricing inputs used by downstream calculators.
    pub fn set_instrument_overrides(
        &mut self,
        overrides: Option<crate::instruments::InstrumentPricingOverrides>,
    ) {
        self.instrument_overrides = overrides;
    }

    /// Set metric-only overrides used by downstream calculators.
    pub fn set_metric_overrides(
        &mut self,
        overrides: Option<crate::instruments::MetricPricingOverrides>,
    ) {
        self.metric_overrides = overrides;
    }

    /// Value the instrument through the active canonical dispatch path.
    pub fn instrument_value_with_scenario(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        self.reprice_money(market, as_of)
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
        if let (Some(model), Some(registry)) = (self.pricing_model, self.pricer_registry.as_ref()) {
            let options = crate::instruments::PricingOptions::default().with_config(self.config());
            return Ok(crate::pricer::PricerRegistry::price_with_metrics_shared(
                registry,
                instrument,
                model,
                market,
                as_of,
                &[],
                options,
            )?
            .value);
        }
        instrument.value(market, as_of)
    }

    /// Reprice an arbitrary instrument as a raw amount using the active dispatch path.
    pub fn reprice_instrument_raw(
        &self,
        instrument: &dyn Instrument,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        if let (Some(model), Some(registry)) = (self.pricing_model, self.pricer_registry.as_ref()) {
            return registry
                .price_raw(instrument, model, market, as_of)
                .map_err(Into::into);
        }
        instrument.value_raw(market, as_of)
    }

    /// Return the instrument's signed canonical cashflows, computing and
    /// caching them on first access.
    ///
    /// Many metric calculators (YTM, YTC, YTW, DM, all-in-rate, embedded option
    /// value, OID-EIR, …) all need the same cashflow schedule. Without this
    /// cache, evaluating N metrics on a long DDTL reruns the cashflow builder
    /// N times. Subsequent calls return the cached vector.
    pub fn cashflows_cached(&mut self) -> finstack_quant_core::Result<&Vec<(Date, Money)>> {
        if self.cashflows.is_none() {
            let flows = self.instrument.dated_cashflows(&self.curves, self.as_of)?;
            self.cashflows = Some(flows);
        }
        self.cashflows
            .as_ref()
            .ok_or_else(|| finstack_quant_core::InputError::Invalid.into())
    }

    /// Return the instrument's canonical cashflow schedule flows with CFKind metadata.
    pub(crate) fn tagged_cashflows_cached(
        &mut self,
    ) -> finstack_quant_core::Result<&Vec<CashFlow>> {
        if self.tagged_cashflows.is_none() {
            let schedule = self
                .instrument
                .cashflow_schedule(&self.curves, self.as_of)?;
            self.tagged_cashflows = Some(schedule.into_flows());
        }
        self.tagged_cashflows
            .as_ref()
            .ok_or_else(|| finstack_quant_core::InputError::Invalid.into())
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
        self.computed_matrix.insert(base_metric_id, matrix);
        Ok(())
    }

    /// Retrieves a previously stored 1D bucketed series.
    pub fn get_series(&self, id: &MetricId) -> Option<&[(String, f64)]> {
        self.computed_series.get(id).map(|v| v.as_slice())
    }

    /// Retrieves a previously stored 2D structured metric.
    pub fn get_matrix2d(&self, id: &MetricId) -> Option<&Structured2D> {
        self.computed_matrix.get(id)
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
        assert_eq!(hyphen.as_str(), "bucketed_dv01::USD_x2dOIS");
        assert_eq!(underscore.as_str(), "bucketed_dv01::USD_x5fOIS");
    }
}

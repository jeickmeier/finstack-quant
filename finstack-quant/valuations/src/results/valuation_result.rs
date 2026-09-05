//! Valuation result schema, measures, explanations, and serialization.
//!
use crate::metrics::MetricId;
use crate::pricer::ModelKey;
use finstack_quant_core::config::{results_meta_now, FinstackConfig, ResultsMeta};
use finstack_quant_core::dates::Date;
use finstack_quant_core::explain::ExplanationTrace;
use finstack_quant_core::money::Money;
use finstack_quant_core::wire::SchemaVersion;
use finstack_quant_covenants::CovenantReport;

use indexmap::IndexMap;

fn serialize_path_count<S>(value: &usize, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let value = u32::try_from(*value).map_err(serde::ser::Error::custom)?;
    serializer.serialize_u32(value)
}

/// Model-specific typed valuation details.
///
/// These details are for rich structured outputs that do not fit the scalar
/// `measures` map while still belonging in the standard valuation envelope.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ValuationDetails {
    /// Recursive leg valuation and primitive exposure detail for a composite.
    Composite(crate::instruments::CompositeValuationDetails),
    /// Credit-derivative pricing metadata.
    CreditDerivative(CreditDerivativeValuationDetails),
    /// Scenario-waterfall structured credit stochastic pricing result.
    StructuredCreditStochastic(
        crate::instruments::fixed_income::structured_credit::StochasticPricingResult,
    ),
    /// FX-bearing instrument pricing metadata (forwards, options, quanto, …).
    ///
    /// Surfaces the [`FxMatrix`](finstack_quant_core::money::fx::FxMatrix) lookup
    /// chain that backed the spot rate used at pricing time, satisfying the
    /// project-wide FX-policy-visibility invariant for instruments that
    /// resolve their spot through the matrix.
    Fx(FxValuationDetails),
    /// Monte Carlo estimator and reproducibility metadata.
    MonteCarlo(MonteCarloValuationDetails),
}

/// Metadata for CDS-family valuation paths.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CreditDerivativeValuationDetails {
    /// Registered model key used by the pricer.
    pub model_key: ModelKey,
    /// CDS integration method actually used, when applicable.
    pub integration_method: Option<String>,
}

/// Metadata for FX-bearing valuation paths (FX forwards, FX options, quanto).
///
/// Captures the FX policy applied at pricing time so downstream audit /
/// reconciliation can trace whether a quoted rate came from a direct
/// market quote or via triangulation through the matrix pivot currency.
/// Mirrors the cross-cutting invariant documented in
/// `.claude/rules/project-description.md` ("FX policy visibility: Applied
/// conversion strategy recorded per layer (e.g., valuations, statements,
/// portfolio)").
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct FxValuationDetails {
    /// `true` when the FX spot was obtained via triangulation through the
    /// [`FxMatrix`](finstack_quant_core::money::fx::FxMatrix) pivot currency
    /// rather than a direct quote. Mirrors
    /// [`FxRateResult.triangulated`](finstack_quant_core::money::fx::FxRateResult).
    /// `None` when the instrument resolved spot from an explicit market
    /// scalar (`fx_rate_id`) rather than the matrix.
    pub fx_triangulated: Option<bool>,
}

/// Reproducibility and convergence diagnostics for a Monte Carlo valuation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct MonteCarloValuationDetails {
    /// Registered model used for the simulation.
    pub model_key: ModelKey,
    /// Sampling standard error of the discounted PV mean in the result
    /// currency. For LSMC valuations, this measures pricing-path uncertainty
    /// under the frozen fitted exercise policy. It excludes regression
    /// approximation, time-grid discretization, and model error.
    pub standard_error: f64,
    /// Number of independent paths used to fit an exercise or control policy.
    ///
    /// Zero for Monte Carlo engines that do not have a separate training
    /// stage.
    #[serde(serialize_with = "serialize_path_count")]
    pub training_paths: usize,
    /// Total factor paths simulated in the policy-training stage, including
    /// antithetic partners. Zero when no policy is trained.
    #[serde(serialize_with = "serialize_path_count")]
    pub training_simulated_paths: usize,
    /// Independent paths used to fit state-conditional make-whole reference
    /// values. Zero when no stochastic make-whole stage is required.
    #[serde(serialize_with = "serialize_path_count")]
    pub make_whole_training_paths: usize,
    /// Total factor paths simulated for state-conditional make-whole training,
    /// including antithetic partners. Zero when that stage is absent.
    #[serde(serialize_with = "serialize_path_count")]
    pub make_whole_training_simulated_paths: usize,
    /// Number of independent path estimators contributing to the mean.
    #[serde(serialize_with = "serialize_path_count")]
    pub estimator_paths: usize,
    /// Total number of simulated paths, including antithetic partners.
    #[serde(serialize_with = "serialize_path_count")]
    pub simulated_paths: usize,
    /// Deterministic random seed used for the run.
    pub seed: u64,
    /// Simulation times in year fractions, including zero and maturity.
    pub time_grid: Vec<f64>,
    /// Whether antithetic variates were enabled.
    pub antithetic: bool,
    /// Whether Sobol quasi-random sampling was enabled.
    pub sobol: bool,
    /// Whether Brownian-bridge ordering was enabled for Sobol paths.
    pub brownian_bridge: bool,
}

/// Complete valuation result envelope with NPV, risk metrics, and metadata.
///
/// This is the primary output structure returned by pricing operations.
/// It contains the instrument's present value, computed risk metrics,
/// calculation metadata, and optional covenant checks or explainability traces.
///
/// # Interpretation Contract
///
/// `ValuationResult` intentionally separates:
///
/// - [`Self::value`]: the canonical present value as [`Money`], with currency
///   information preserved
/// - [`Self::measures`]: additional scalar measures keyed by [`MetricId`]
/// - [`Self::meta`]: execution and policy context needed to interpret the result
///
/// Consumers should **not** assume every entry in `measures` is a currency amount.
/// Measure semantics, units, bump conventions, and sign conventions are defined
/// by [`MetricId`] and the producing API.
///
/// # Structure
///
/// - **Value**: Present value in the instrument's native currency
/// - **Measures**: Risk metrics as key-value pairs (e.g., "dv01" → 500.0)
/// - **Metadata**: Calculation context (rounding, numeric mode, timing)
/// - **Covenants**: Optional covenant compliance results
/// - **Explanation**: Optional computation trace for debugging
///
/// # See Also
///
/// - [`crate::metrics::MetricId`] for metric meanings, units, and bump/sign conventions
/// - [`crate::results`] for the public result-module surface
///
/// # Metadata Stamping
///
/// Results are stamped with metadata indicating:
/// - Numeric mode (Decimal vs f64)
/// - Rounding policy applied
/// - FX policy for cross-currency calculations
/// - Calculation timestamp and duration
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust
/// use finstack_quant_valuations::results::ValuationResult;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::dates::create_date;
/// use time::Month;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let as_of = create_date(2025, Month::January, 15)?;
/// let pv = Money::from((1_000_000_i64, Currency::USD));
///
/// let result = ValuationResult::stamped("BOND-001", as_of, pv);
///
/// assert_eq!(result.instrument_id, "BOND-001");
/// assert_eq!(result.value.amount(), 1_000_000.0);
/// assert_eq!(result.value.currency(), Currency::USD);
/// # Ok(())
/// # }
/// ```
///
/// ## With Metrics
///
/// ```rust
/// use finstack_quant_valuations::results::ValuationResult;
/// use finstack_quant_valuations::metrics::MetricId;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::dates::create_date;
/// use indexmap::IndexMap;
/// use time::Month;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let as_of = create_date(2025, Month::January, 15)?;
/// let pv = Money::from((1_000_000_i64, Currency::USD));
///
/// let mut measures: IndexMap<MetricId, f64> = IndexMap::new();
/// measures.insert(MetricId::custom("ytm"), 0.0475);
/// measures.insert(MetricId::custom("modified_duration"), 4.25);
/// measures.insert(MetricId::custom("dv01"), 425.0);
///
/// let result = ValuationResult::stamped("BOND-001", as_of, pv)
///     .with_measures(measures);
///
/// assert_eq!(result.metric_str("ytm"), Some(0.0475));
/// assert_eq!(result.metric_str("dv01"), Some(425.0));
/// # Ok(())
/// # }
/// ```
///
/// ## With Covenants
///
/// ```rust
/// use finstack_quant_valuations::results::ValuationResult;
/// use finstack_quant_covenants::CovenantReport;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::dates::create_date;
/// use time::Month;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let as_of = create_date(2025, Month::January, 15)?;
/// let pv = Money::from((1_000_000_i64, Currency::USD));
///
/// let covenant = CovenantReport {
///     covenant_type: "dscr".to_string(),
///     covenant_id: None,
///     passed: true,
///     actual_value: Some(1.5),
///     threshold: Some(1.25),
///     details: Some("DSCR: 1.50x >= 1.25x".to_string()),
///     headroom: Some(0.25),
///     meta: Default::default(),
/// };
///
/// let result = ValuationResult::stamped("LOAN-001", as_of, pv)
///     .with_covenant("dscr_test", covenant);
///
/// assert!(result.all_covenants_passed());
/// assert_eq!(result.failed_covenants().len(), 0);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ValuationResult {
    /// Required wire-format schema version. Only numeric `1` is accepted.
    pub schema_version: SchemaVersion,

    /// Unique identifier for the priced instrument.
    pub instrument_id: String,

    /// Valuation date (T+0) for the calculation.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,

    /// Present value in the instrument's native currency.
    ///
    /// This is the primary pricing output and is **always available** regardless
    /// of which metrics are requested. The PV is **not** included in the `measures`
    /// map - it is provided here as a `Money` type with full currency information.
    ///
    /// For cross-currency instruments, this may be in a different currency than
    /// the base calculation currency.
    ///
    /// # Example
    /// ```rust
    /// # use finstack_quant_valuations::results::ValuationResult;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::dates::create_date;
    /// # use time::Month;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let as_of = create_date(2025, Month::January, 15)?;
    /// # let pv = Money::from((1_000_000_i64, Currency::USD));
    /// # let result = ValuationResult::stamped("BOND-001", as_of, pv);
    /// // PV is always in result.value, not in measures
    /// let pv_money = result.value;  // Money type
    /// let pv_amount = result.value.amount();  // f64 value
    /// let currency = result.value.currency();  // Currency type
    /// # Ok(())
    /// # }
    /// ```
    pub value: Money,

    /// Computed risk measures and financial metrics.
    ///
    /// Contains **derived risk metrics** such as DV01, Delta, Vega, etc.
    /// The present value (PV) is **not** included here - it is available
    /// in the `value` field above.
    ///
    /// Keys are strongly-typed metric IDs (serialized as strings such as
    /// "ytm", "dv01", "delta"). Use `MetricId` helpers for consistent lookups.
    ///
    /// # Interpretation
    ///
    /// Entries in this map are heterogeneous by design:
    /// - some are currency amounts (`jump_to_default`)
    /// - some are currency-per-bump sensitivities (`dv01`, `vega`, `rho`)
    /// - some are decimal rates or probabilities (`ytm`, `default_probability`)
    /// - some are ratios or counts (`tvpi_lp`, `constituent_count`)
    ///
    /// Always interpret a measure together with its [`MetricId`] contract.
    pub measures: IndexMap<MetricId, f64>,

    /// Optional rich model-specific pricing detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<ValuationDetails>,

    /// Calculation metadata and policy stamps.
    ///
    /// Contains:
    /// - Numeric mode (Decimal vs f64)
    /// - Rounding context and precision
    /// - FX policy for cross-currency calculations
    /// - Calculation timing information
    pub meta: ResultsMeta,

    /// Covenant compliance results for structured products.
    ///
    /// Present only for instruments with covenants (loans, structured credit).
    /// Each covenant is keyed by its identifier with pass/fail status and details.
    pub covenants: Option<IndexMap<String, CovenantReport>>,

    /// Optional computation explanation trace.
    ///
    /// Enabled via `ExplainOpts` in configuration. Provides step-by-step
    /// trace of calculations for debugging and auditability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<ExplanationTrace>,
}

impl ValuationResult {
    /// Create a basic valuation result with NPV and default metadata.
    ///
    /// Constructs a result with just the present value, using default
    /// configuration for metadata stamping (Decimal mode, default rounding).
    /// For custom metadata, use [`stamped_with_meta()`](Self::stamped_with_meta).
    ///
    /// # Arguments
    ///
    /// * `instrument_id` - Unique identifier for the priced instrument
    /// * `as_of` - Valuation date
    /// * `value` - Present value in the instrument's currency
    ///
    /// # Returns
    ///
    /// `ValuationResult` with NPV and default metadata (no metrics or covenants)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::results::ValuationResult;
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::dates::create_date;
    /// use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let as_of = create_date(2025, Month::January, 15)?;
    /// let pv = Money::from((1_000_000_i64, Currency::USD));
    ///
    /// let result = ValuationResult::stamped("BOND-001", as_of, pv);
    ///
    /// assert_eq!(result.instrument_id, "BOND-001");
    /// assert_eq!(result.value.amount(), 1_000_000.0);
    /// assert!(result.measures.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    pub fn stamped(instrument_id: &str, as_of: Date, value: Money) -> Self {
        // Default stamping uses default configuration; callers needing custom
        // policy should construct core `ResultsMeta` and use
        // `stamped_with_meta` to avoid creating a fresh config here.
        let meta = results_meta_now(&FinstackConfig::default());
        Self::stamped_with_meta(instrument_id, as_of, value, meta)
    }

    /// Create a valuation result using a provided configuration.
    ///
    /// This helper ensures the metadata stamp matches the exact `FinstackConfig`
    /// used during pricing, avoiding mismatches between execution policy and
    /// reported metadata.
    pub fn stamped_with_config(
        instrument_id: &str,
        as_of: Date,
        value: Money,
        cfg: &FinstackConfig,
    ) -> Self {
        let meta = results_meta_now(cfg);
        Self::stamped_with_meta(instrument_id, as_of, value, meta)
    }

    /// Create a valuation result with caller-provided metadata.
    ///
    /// Use this in hot paths when you already have `ResultsMeta` available
    /// to avoid constructing a default `FinstackConfig`. This is the
    /// performance-optimized constructor for repeated valuations.
    ///
    /// # Arguments
    ///
    /// * `instrument_id` - Unique identifier for the priced instrument
    /// * `as_of` - Valuation date stamped onto the result for audit and replay
    /// * `value` - Present value in the instrument's currency
    /// * `meta` - Pre-constructed metadata with policy stamps
    ///
    /// # Returns
    ///
    /// `ValuationResult` with NPV and provided metadata
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::results::ValuationResult;
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::dates::create_date;
    /// use finstack_quant_core::config::{FinstackConfig, results_meta_now};
    /// use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let as_of = create_date(2025, Month::January, 15)?;
    /// let pv = Money::from((1_000_000_i64, Currency::USD));
    ///
    /// // Pre-construct metadata once for batch pricing
    /// let config = FinstackConfig::default();
    /// let meta = results_meta_now(&config);
    ///
    /// let result = ValuationResult::stamped_with_meta("BOND-001", as_of, pv, meta);
    /// assert_eq!(result.instrument_id, "BOND-001");
    /// # Ok(())
    /// # }
    /// ```
    pub fn stamped_with_meta(
        instrument_id: &str,
        as_of: Date,
        value: Money,
        meta: ResultsMeta,
    ) -> Self {
        Self {
            schema_version: SchemaVersion::CURRENT,
            instrument_id: instrument_id.to_string(),
            as_of,
            value,
            measures: IndexMap::new(),
            details: None,
            meta,
            covenants: None,
            explanation: None,
        }
    }

    /// Attach an explanation trace for debugging and auditability.
    ///
    /// Explanation traces provide step-by-step computation logs showing
    /// intermediate calculations and data flow. Enable via `ExplainOpts`
    /// in configuration.
    ///
    /// # Arguments
    ///
    /// * `trace` - Explanation trace from the computation
    ///
    /// # Returns
    ///
    /// `Self` for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::results::ValuationResult;
    /// use finstack_quant_core::explain::ExplanationTrace;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::dates::create_date;
    /// # use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let as_of = create_date(2025, Month::January, 15)?;
    /// # let pv = Money::from((1_000_000_i64, Currency::USD));
    ///
    /// let trace = ExplanationTrace::new("bond_pricing");
    /// // Add trace entries using TraceEntry variants (see explain module for available types)
    ///
    /// let result = ValuationResult::stamped("BOND-001", as_of, pv)
    ///     .with_explanation(trace);
    ///
    /// assert!(result.explanation.is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_explanation(mut self, trace: ExplanationTrace) -> Self {
        self.explanation = Some(trace);
        self
    }

    /// Attach computed risk metrics to the result.
    ///
    /// Replaces any existing measures with the provided map. Metrics
    /// are keyed by [`MetricId`] values (for example `MetricId::Ytm`,
    /// `MetricId::Dv01`).
    ///
    /// # Arguments
    ///
    /// * `measures` - Map of metric identifier to computed value
    ///
    /// # Returns
    ///
    /// `Self` for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::results::ValuationResult;
    /// use finstack_quant_valuations::metrics::MetricId;
    /// use indexmap::IndexMap;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::dates::create_date;
    /// # use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let as_of = create_date(2025, Month::January, 15)?;
    /// # let pv = Money::from((1_000_000_i64, Currency::USD));
    /// let mut measures = IndexMap::new();
    /// measures.insert(MetricId::custom("ytm"), 0.0475);
    /// measures.insert(MetricId::custom("modified_duration"), 4.25);
    ///
    /// let result = ValuationResult::stamped("BOND-001", as_of, pv)
    ///     .with_measures(measures);
    ///
    /// assert_eq!(result.measures.len(), 2);
    /// assert_eq!(
    ///     result.metric(MetricId::custom("ytm")),
    ///     Some(0.0475)
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_measures(mut self, measures: IndexMap<MetricId, f64>) -> Self {
        self.measures = measures;
        self
    }

    /// Attach rich model-specific valuation details.
    ///
    /// # Arguments
    ///
    /// * `details` - Model-specific valuation payload (tree nodes, MC stats, cashflow
    ///   breakdown) stored on the result.
    pub fn with_details(mut self, details: ValuationDetails) -> Self {
        self.details = Some(details);
        self
    }

    /// Get a metric by `MetricId`.
    ///
    /// # Arguments
    ///
    /// * `id` - Metric identifier to look up in `self.measures`.
    pub fn metric(&self, id: MetricId) -> Option<f64> {
        self.measures.get(&id).copied()
    }

    /// Get a metric by its exact string identifier.
    pub fn metric_str(&self, id: &str) -> Option<f64> {
        self.measures.get(id).copied()
    }

    /// Return structured components and values for a composite base metric.
    ///
    /// Scalar entries stored directly under `base` are excluded. Matching
    /// composite entries retain the insertion order of the [`IndexMap`] in
    /// [`Self::measures`], which is also their deterministic serde wire order.
    /// Components are decoded with the canonical [`MetricId`] composite-key
    /// codec. Malformed legacy escape markers remain literal. When distinct
    /// wire keys would decode to duplicate coordinates, all members of that
    /// collision group retain their literal wire components so no value is
    /// omitted or deduplicated.
    pub fn metric_series(&self, base: &MetricId) -> Vec<(Vec<String>, f64)> {
        let components = MetricId::decode_series_components(base, self.measures.keys());
        self.measures
            .iter()
            .zip(components)
            .filter_map(|((_, value), components)| {
                components.map(|components| (components, *value))
            })
            .collect()
    }

    /// Look up a measure by string key, tolerating the legacy escaped wire form.
    ///
    /// Tries an exact match first (`metric_str`). On a miss the composite
    /// keys with the same base are compared on their decoded components, so a
    /// caller holding a key persisted by an earlier release
    /// (`pv01::USD_x2dOIS`) still resolves the literal key written today
    /// (`pv01::USD-OIS`), and vice versa. Scalar keys never match anything
    /// but themselves.
    ///
    /// # Arguments
    ///
    /// * `id` - Metric key in either the literal (`pv01::USD-OIS`) or legacy
    ///   escaped (`pv01::USD_x2dOIS`) composite form, or a scalar metric name.
    pub fn metric_str_decoded(&self, id: &str) -> Option<f64> {
        if let Some(value) = self.metric_str(id) {
            return Some(value);
        }
        let requested = MetricId::custom(id);
        let base = requested.base();
        let wanted = requested.decode_components(&base)?;
        self.measures.iter().find_map(|(key, value)| {
            (key.decode_components(&base).as_ref() == Some(&wanted)).then_some(*value)
        })
    }

    /// Measure keys ranked by case-folded similarity to `id`, best first.
    ///
    /// Used to build actionable "did you mean" diagnostics when a lookup
    /// misses; the ranking is the same one unknown-metric errors use.
    ///
    /// # Arguments
    ///
    /// * `id` - The key the caller asked for (any case, literal or legacy form).
    /// * `limit` - Maximum number of keys returned.
    pub fn closest_metric_keys(&self, id: &str, limit: usize) -> Vec<String> {
        crate::metrics::closest_metric_names(id, self.measures.keys().map(MetricId::as_str), limit)
    }

    /// Unit family of every measure, keyed by wire metric key.
    ///
    /// Composite keys inherit their base metric's unit; custom metrics report
    /// [`MetricUnit::Unknown`](crate::metrics::MetricUnit::Unknown). Order
    /// follows `measures` insertion order.
    pub fn metric_units(&self) -> IndexMap<String, crate::metrics::MetricUnit> {
        self.measures
            .keys()
            .map(|key| (key.to_string(), key.unit()))
            .collect()
    }

    /// Attach multiple covenant reports to the result.
    ///
    /// Replaces any existing covenant reports with the provided map.
    /// Used for structured products with multiple compliance tests.
    ///
    /// # Arguments
    ///
    /// * `covenants` - Map of covenant identifier to compliance report
    ///
    /// # Returns
    ///
    /// `Self` for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::results::ValuationResult;
    /// use finstack_quant_covenants::CovenantReport;
    /// use indexmap::IndexMap;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::dates::create_date;
    /// # use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let as_of = create_date(2025, Month::January, 15)?;
    /// # let pv = Money::from((1_000_000_i64, Currency::USD));
    /// let mut covenants = IndexMap::new();
    /// covenants.insert("dscr".to_string(), CovenantReport {
    ///     covenant_type: "dscr".to_string(),
    ///     covenant_id: None,
    ///     passed: true,
    ///     actual_value: Some(1.5),
    ///     threshold: Some(1.25),
    ///     details: Some("DSCR test passed".to_string()),
    ///     headroom: Some(0.25),
    ///     meta: Default::default(),
    /// });
    ///
    /// let result = ValuationResult::stamped("LOAN-001", as_of, pv)
    ///     .with_covenants(covenants);
    ///
    /// assert!(result.all_covenants_passed());
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_covenants(mut self, covenants: IndexMap<String, CovenantReport>) -> Self {
        self.covenants = Some(covenants);
        self
    }

    /// Add a single covenant report to the result.
    ///
    /// Preserves existing covenant reports and adds a new one.
    /// Convenient for incrementally building covenant results.
    ///
    /// # Arguments
    ///
    /// * `key` - Covenant identifier (e.g., "dscr_test", "ltv_check")
    /// * `report` - Covenant compliance report
    ///
    /// # Returns
    ///
    /// `Self` for method chaining
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::results::ValuationResult;
    /// use finstack_quant_covenants::CovenantReport;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::dates::create_date;
    /// # use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let as_of = create_date(2025, Month::January, 15)?;
    /// # let pv = Money::from((1_000_000_i64, Currency::USD));
    /// let result = ValuationResult::stamped("LOAN-001", as_of, pv)
    ///     .with_covenant("dscr", CovenantReport {
    ///         covenant_type: "dscr".to_string(),
    ///         covenant_id: None,
    ///         passed: true,
    ///         actual_value: Some(1.5),
    ///         threshold: Some(1.25),
    ///         details: None,
    ///         headroom: Some(0.25),
    ///         meta: Default::default(),
    ///     })
    ///     .with_covenant("ltv", CovenantReport {
    ///         covenant_type: "ltv".to_string(),
    ///         covenant_id: None,
    ///         passed: true,
    ///         actual_value: Some(0.70),
    ///         threshold: Some(0.80),
    ///         details: None,
    ///         headroom: Some(0.10),
    ///         meta: Default::default(),
    ///     });
    ///
    /// assert_eq!(result.covenants.as_ref().expect("should succeed").len(), 2);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_covenant(mut self, key: &str, report: CovenantReport) -> Self {
        let mut covenants = self.covenants.unwrap_or_default();
        covenants.insert(key.to_string(), report);
        self.covenants = Some(covenants);
        self
    }

    // Note: FX policy stamping is handled at the core `ResultsMeta` level.

    /// Check if all covenants passed their compliance tests.
    ///
    /// Returns `true` if there are no covenants or if all covenants passed.
    /// Use [`failed_covenants()`](Self::failed_covenants) to get specific failures.
    ///
    /// # Returns
    ///
    /// `true` if all covenants passed or no covenants present, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::results::ValuationResult;
    /// use finstack_quant_covenants::CovenantReport;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::dates::create_date;
    /// # use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let as_of = create_date(2025, Month::January, 15)?;
    /// # let pv = Money::from((1_000_000_i64, Currency::USD));
    /// let result = ValuationResult::stamped("LOAN-001", as_of, pv)
    ///     .with_covenant("dscr", CovenantReport {
    ///         covenant_type: "dscr".to_string(),
    ///         covenant_id: None,
    ///         passed: true,
    ///         actual_value: Some(1.5),
    ///         threshold: Some(1.25),
    ///         details: None,
    ///         headroom: Some(0.25),
    ///         meta: Default::default(),
    ///     });
    ///
    /// assert!(result.all_covenants_passed());
    /// # Ok(())
    /// # }
    /// ```
    pub fn all_covenants_passed(&self) -> bool {
        self.covenants
            .as_ref()
            .map(|c| c.values().all(|r| r.passed))
            .unwrap_or(true)
    }

    /// Get list of failed covenant identifiers.
    ///
    /// Returns identifiers of covenants that did not pass their compliance
    /// tests. Empty vector if all covenants passed or no covenants present.
    ///
    /// # Returns
    ///
    /// Vector of covenant identifiers that failed
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::results::ValuationResult;
    /// use finstack_quant_covenants::CovenantReport;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::dates::create_date;
    /// # use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let as_of = create_date(2025, Month::January, 15)?;
    /// # let pv = Money::from((1_000_000_i64, Currency::USD));
    /// let result = ValuationResult::stamped("LOAN-001", as_of, pv)
    ///     .with_covenant("dscr", CovenantReport {
    ///         covenant_type: "dscr".to_string(),
    ///         covenant_id: None,
    ///         passed: false,
    ///         actual_value: Some(1.1),
    ///         threshold: Some(1.25),
    ///         details: Some("DSCR below threshold".to_string()),
    ///         headroom: Some(-0.15),
    ///         meta: Default::default(),
    ///     });
    ///
    /// let failed = result.failed_covenants();
    /// assert_eq!(failed.len(), 1);
    /// assert_eq!(failed[0], "dscr");
    /// # Ok(())
    /// # }
    /// ```
    pub fn failed_covenants(&self) -> Vec<&str> {
        self.covenants
            .as_ref()
            .map(|c| {
                c.iter()
                    .filter(|(_, r)| !r.passed)
                    .map(|(k, _)| k.as_str())
                    .collect()
            })
            .unwrap_or_default()
    }
}

// Note: a previous revision exposed `impl Index<MetricId>` / `impl Index<&MetricId>`
// on `ValuationResult`, which silently panicked on missing keys. Callers must now
// use `metric(id)` / `metric_str(id)` (option-returning), or index the backing `measures` map directly when a
// missing key truly is a programming error at the call site.

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::explain::ExplanationTrace;
    use finstack_quant_core::money::Money;
    use indexmap::IndexMap;
    use time::macros::date;

    #[test]
    fn metric_str_decoded_matches_literal_and_legacy_escaped_keys() {
        let mut measures = IndexMap::new();
        measures.insert(MetricId::Dv01, 1.0);
        measures.insert(MetricId::composite(&MetricId::Pv01, &["USD-OIS"]), 2.0);
        measures.insert(MetricId::custom("pv01::EUR_x2dOIS"), 3.0);
        let result = ValuationResult::stamped(
            "K",
            date!(2025 - 01 - 15),
            Money::from((0_i64, Currency::USD)),
        )
        .with_measures(measures);

        assert_eq!(result.metric_str_decoded("dv01"), Some(1.0));
        assert_eq!(result.metric_str_decoded("pv01::USD-OIS"), Some(2.0));
        assert_eq!(result.metric_str_decoded("pv01::USD_x2dOIS"), Some(2.0));
        assert_eq!(result.metric_str_decoded("pv01::EUR-OIS"), Some(3.0));
        assert_eq!(result.metric_str_decoded("pv01::GBP-OIS"), None);
        assert_eq!(result.metric_str_decoded("dv02"), None);
        assert_eq!(
            result.closest_metric_keys("DV01", 1),
            vec!["dv01".to_string()]
        );

        let units = result.metric_units();
        assert_eq!(units["dv01"], crate::metrics::MetricUnit::Currency);
        assert_eq!(units["pv01::USD-OIS"], crate::metrics::MetricUnit::Currency);
    }

    #[test]
    fn metric_str_is_exact_and_accessors_return_option_or_result() {
        let mut measures = IndexMap::new();
        measures.insert(MetricId::Dv01, 12.5);
        measures.insert(MetricId::custom("dv01_extra"), 99.0);

        let result = ValuationResult::stamped(
            "TEST",
            date!(2025 - 01 - 02),
            Money::from((1_i64, Currency::USD)),
        )
        .with_measures(measures);

        assert_eq!(result.metric_str("dv01"), Some(12.5));
        assert_eq!(result.metric_str("dv01_extra"), Some(99.0));
        assert_eq!(result.metric_str("dv"), None);
        assert_eq!(result.metric(MetricId::Dv01), Some(12.5));
        assert_eq!(result.metric(MetricId::Dv01), Some(12.5));
        assert_eq!(result.metric(MetricId::Cs01), None);
    }

    #[test]
    fn metric_series_decodes_components_in_measure_insertion_order() {
        let mut measures = IndexMap::new();
        measures.insert(MetricId::BucketedDv01, -3.0);
        measures.insert(
            MetricId::composite(&MetricId::BucketedDv01, &["USD-OIS", "10y"]),
            -1.0,
        );
        measures.insert(MetricId::composite(&MetricId::Cs01, &["ACME"]), 8.0);
        measures.insert(
            MetricId::composite(&MetricId::BucketedDv01, &["EUR/USD", ""]),
            -2.0,
        );

        let result = ValuationResult::stamped(
            "SERIES",
            date!(2025 - 01 - 02),
            Money::from((1_i64, Currency::USD)),
        )
        .with_measures(measures);

        assert_eq!(
            result.metric_series(&MetricId::BucketedDv01),
            vec![
                (vec!["USD-OIS".to_string(), "10y".to_string()], -1.0),
                (vec!["EUR/USD".to_string(), String::new()], -2.0),
            ]
        );
    }

    #[test]
    fn metric_series_preserves_all_legacy_and_escaped_collision_entries() {
        let mut measures = IndexMap::new();
        measures.insert(MetricId::custom("bucketed_dv01::curve-ray"), -1.0);
        measures.insert(MetricId::custom("bucketed_dv01::curve_x2dray"), -2.0);
        measures.insert(MetricId::custom("bucketed_dv01::curve_xray"), -3.0);

        let result = ValuationResult::stamped(
            "LEGACY-SERIES",
            date!(2025 - 01 - 02),
            Money::from((1_i64, Currency::USD)),
        )
        .with_measures(measures);

        assert_eq!(
            result.metric_series(&MetricId::BucketedDv01),
            vec![
                (vec!["curve-ray".to_string()], -1.0),
                (vec!["curve_x2dray".to_string()], -2.0),
                (vec!["curve_xray".to_string()], -3.0),
            ]
        );
    }

    #[test]
    fn metric_series_resolves_transitive_legacy_escape_collisions() {
        let mut measures = IndexMap::new();
        measures.insert(MetricId::custom("bucketed_dv01::curve-ray"), -1.0);
        measures.insert(MetricId::custom("bucketed_dv01::curve_x2dray"), -2.0);
        measures.insert(MetricId::custom("bucketed_dv01::curve_x5fx2dray"), -3.0);

        let result = ValuationResult::stamped(
            "TRANSITIVE-LEGACY-SERIES",
            date!(2025 - 01 - 02),
            Money::from((1_i64, Currency::USD)),
        )
        .with_measures(measures);

        assert_eq!(
            result.metric_series(&MetricId::BucketedDv01),
            vec![
                (vec!["curve-ray".to_string()], -1.0),
                (vec!["curve_x2dray".to_string()], -2.0),
                (vec!["curve_x5fx2dray".to_string()], -3.0),
            ]
        );
    }

    #[test]
    fn stamped_with_config_round_trips_metadata_fields() {
        let as_of = date!(2025 - 01 - 15);
        let pv = Money::from((1_i64, Currency::USD));
        let cfg = FinstackConfig::default();
        let stamped = ValuationResult::stamped_with_config("CFG-1", as_of, pv, &cfg);
        assert_eq!(stamped.instrument_id, "CFG-1");
        assert_eq!(
            stamped.meta.numeric_mode,
            finstack_quant_core::config::NUMERIC_MODE_F64
        );
    }

    #[test]
    fn serde_roundtrip_keeps_covenant_and_explanation() {
        let as_of = date!(2025 - 02 - 01);
        let pv = Money::from((10_i64, Currency::EUR));
        let mut covenants = IndexMap::new();
        covenants.insert(
            "dscr".to_string(),
            CovenantReport {
                covenant_type: "dscr".to_string(),
                covenant_id: None,
                passed: false,
                actual_value: Some(1.0),
                threshold: Some(1.2),
                details: None,
                headroom: None,
                meta: finstack_quant_core::config::ResultsMeta::default(),
            },
        );
        let trace = ExplanationTrace::new("unit");
        let original = ValuationResult::stamped("TR", as_of, pv)
            .with_covenants(covenants)
            .with_explanation(trace);
        let json = serde_json::to_string(&original);
        assert!(json.is_ok(), "valuation result should serialize");
        if let Ok(json) = json {
            let back = serde_json::from_str::<ValuationResult>(&json);
            assert!(back.is_ok(), "valuation result should deserialize");
            if let Ok(back) = back {
                assert_eq!(back.failed_covenants(), vec!["dscr"]);
                assert!(back.explanation.is_some());
            }
        }
    }

    #[test]
    fn credit_derivative_details_serialize_canonical_model_key() {
        let details = CreditDerivativeValuationDetails {
            model_key: ModelKey::HazardRate,
            integration_method: Some("isda_standard_model".to_string()),
        };

        let json = serde_json::to_value(details).expect("details should serialize");
        assert_eq!(json["model_key"], "hazard_rate");
    }
}

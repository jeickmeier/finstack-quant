//! Valuation result schema, measures, explanations, and serialization.
//!
use super::details::ValuationDetails;
use crate::metrics::MetricId;
use finstack_quant_core::config::{FinstackConfig, ResultsMeta, results_meta};
use finstack_quant_core::dates::Date;
use finstack_quant_core::explain::ExplanationTrace;
use finstack_quant_core::money::Money;
use finstack_quant_core::wire::SchemaVersion;

use indexmap::IndexMap;

/// Complete valuation result envelope with NPV, risk metrics, and metadata.
///
/// This is the primary output structure returned by pricing operations.
/// It contains the instrument's present value, computed risk metrics,
/// calculation metadata, and optional explainability traces.
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
/// use finstack_quant_valuations_core::results::ValuationResult;
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
/// use finstack_quant_valuations_core::results::ValuationResult;
/// use finstack_quant_valuations_core::metrics::MetricId;
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ValuationResult {
    /// Required wire-format schema version. Only numeric `1` is accepted.
    pub schema_version: SchemaVersion,

    /// Unique identifier for the priced instrument.
    pub instrument_id: String,

    /// Valuation date (T+0) for the calculation.
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
    /// # use finstack_quant_valuations_core::results::ValuationResult;
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

    /// Optional computation explanation trace.
    ///
    /// Requested with `PricingOptions::with_explain`. For discounting and
    /// hazard-rate valuations it holds one `cashflow_pv` entry per projected
    /// cashflow, reconciled to `value`; for other models it holds a single
    /// `computation_step` entry saying no per-flow decomposition exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<ExplanationTrace>,

    /// How the result was produced: model, requested date, declared market
    /// dependencies, scenario adjustment and sensitivity bump sizes.
    ///
    /// Stamped by the pricer registry. Absent on results constructed directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<super::ValuationProvenance>,

    /// Pricer state handed to metric evaluation of the same request; never
    /// serialized. See [`MetricSeed`](crate::metrics::MetricSeed).
    #[serde(skip)]
    pub(crate) metric_seed: Option<std::sync::Arc<dyn crate::metrics::MetricSeed>>,
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
    /// `ValuationResult` with NPV and default metadata (no metrics)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations_core::results::ValuationResult;
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
        let meta = results_meta(&FinstackConfig::default());
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
    /// use finstack_quant_valuations_core::results::ValuationResult;
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::dates::create_date;
    /// use finstack_quant_core::config::{FinstackConfig, results_meta};
    /// use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let as_of = create_date(2025, Month::January, 15)?;
    /// let pv = Money::from((1_000_000_i64, Currency::USD));
    ///
    /// // Pre-construct metadata once for batch pricing
    /// let config = FinstackConfig::default();
    /// let meta = results_meta(&config);
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
            explanation: None,
            provenance: None,
            metric_seed: None,
        }
    }

    /// Hand state computed while pricing to the metric evaluation of this
    /// request.
    ///
    /// The registry installs the seed into the metric context before the
    /// instrument seeds it, so metrics reuse the pricer's work. The seed is not
    /// serialized and does not change any reported value.
    ///
    /// # Arguments
    ///
    /// * `seed` - State produced by the pricer for this instrument, market and
    ///   valuation date.
    ///
    /// # Returns
    ///
    /// `Self` for method chaining
    #[must_use]
    pub fn with_metric_seed(
        mut self,
        seed: std::sync::Arc<dyn crate::metrics::MetricSeed>,
    ) -> Self {
        self.metric_seed = Some(seed);
        self
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
    /// use finstack_quant_valuations_core::results::ValuationResult;
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
    /// use finstack_quant_valuations_core::results::ValuationResult;
    /// use finstack_quant_valuations_core::metrics::MetricId;
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
    /// codec. Canonical key ingestion prevents aliases between coordinates.
    pub fn metric_series(&self, base: &MetricId) -> Vec<(Vec<String>, f64)> {
        self.measures
            .iter()
            .filter_map(|(key, value)| {
                key.decode_components(base)
                    .map(|components| (components, *value))
            })
            .collect()
    }

    /// Measure keys ranked by case-folded similarity to `id`, best first.
    ///
    /// Used to build actionable "did you mean" diagnostics when a lookup
    /// misses; the ranking is the same one unknown-metric errors use.
    ///
    /// # Arguments
    ///
    /// * `id` - The key the caller asked for (any case, in canonical wire form).
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
}

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
    fn metric_str_matches_only_the_exact_canonical_wire_key() {
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

        assert_eq!(result.metric_str("dv01"), Some(1.0));
        assert_eq!(result.metric_str("pv01::USD-OIS"), Some(2.0));
        assert_eq!(result.metric_str("pv01::USD_x2dOIS"), None);
        assert_eq!(result.metric_str("pv01::EUR_x5fx2dOIS"), Some(3.0));
        assert_eq!(result.metric_str("pv01::GBP-OIS"), None);
        assert_eq!(result.metric_str("dv02"), None);
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
    fn metric_series_preserves_distinct_literal_escape_labels() {
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
    fn metric_series_preserves_nested_literal_escape_labels() {
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
    fn stamped_with_meta_round_trips_config_metadata_fields() {
        let as_of = date!(2025 - 01 - 15);
        let pv = Money::from((1_i64, Currency::USD));
        let cfg = FinstackConfig::default();
        let stamped = ValuationResult::stamped_with_meta("CFG-1", as_of, pv, results_meta(&cfg));
        assert_eq!(stamped.instrument_id, "CFG-1");
        assert_eq!(
            stamped.meta.numeric_mode,
            finstack_quant_core::config::NUMERIC_MODE_F64
        );
    }

    #[test]
    fn serde_roundtrip_keeps_explanation() {
        let as_of = date!(2025 - 02 - 01);
        let pv = Money::from((10_i64, Currency::EUR));
        let trace = ExplanationTrace::new("unit");
        let original = ValuationResult::stamped("TR", as_of, pv).with_explanation(trace);
        let json = serde_json::to_string(&original);
        assert!(json.is_ok(), "valuation result should serialize");
        if let Ok(json) = json {
            let back = serde_json::from_str::<ValuationResult>(&json);
            assert!(back.is_ok(), "valuation result should deserialize");
            if let Ok(back) = back {
                assert!(back.explanation.is_some());
            }
        }
    }

    #[test]
    fn credit_derivative_details_serialize_canonical_model_key() {
        use super::super::details::CreditDerivativeValuationDetails;
        use crate::pricer::ModelKey;
        let details = CreditDerivativeValuationDetails {
            model_key: ModelKey::HazardRate,
            integration_method: Some("isda_standard_model".to_string()),
        };

        let json = serde_json::to_value(details).expect("details should serialize");
        assert_eq!(json["model_key"], "hazard_rate");
    }
}

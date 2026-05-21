//! Metric registry and computation engine.
//!
//! Manages metric calculators with dependency resolution, caching, and batch
//! computation. The registry handles which metrics apply to which instrument
//! types and ensures efficient computation ordering.

use super::ids::MetricId;
use super::traits::{MetricCalculator, MetricContext};

use crate::pricer::InstrumentType;
use finstack_core::HashMap;
use std::sync::Arc;

/// Registry for metric calculators.
///
/// Manages metric calculators with dependency resolution, caching, and batch
/// computation. Also handles which metrics apply to which instrument types.
///
/// # Key Features
///
/// - **Calculator management**: Register and retrieve metric calculators
/// - **Dependency resolution**: Automatic computation ordering based on dependencies
/// - **Instrument applicability**: Metrics can be restricted to specific instrument types
/// - **Batch computation**: Compute multiple metrics efficiently
///
/// See unit tests and `examples/` for usage.
#[derive(Clone)]
pub struct MetricRegistry {
    entries: HashMap<MetricId, MetricEntry>,
}

#[derive(Clone, Default)]
struct MetricEntry {
    default: Option<Arc<dyn MetricCalculator>>,
    per_instrument: HashMap<InstrumentType, Arc<dyn MetricCalculator>>,
}

impl MetricEntry {
    fn get_for(&self, instrument_type: InstrumentType) -> Option<&Arc<dyn MetricCalculator>> {
        self.per_instrument
            .get(&instrument_type)
            .or(self.default.as_ref())
    }

    fn applies_to(&self, instrument_type: InstrumentType) -> bool {
        self.per_instrument.contains_key(&instrument_type) || self.default.is_some()
    }
}

impl MetricRegistry {
    /// Creates a new empty registry.
    ///
    /// See unit tests and `examples/` for usage.
    pub fn new() -> Self {
        Self {
            entries: HashMap::default(),
        }
    }
}

impl Default for MetricRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricRegistry {
    /// Registers a metric calculator with explicit ID and applicability.
    ///
    /// If a calculator with the same ID already exists, it will be replaced.
    /// The `applicable_to` parameter specifies which instrument types this metric
    /// applies to. An empty slice means it applies to all instruments.
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the metric
    /// * `calculator` - Implementation of the metric calculation
    /// * `applicable_to` - Instrument types this metric applies to (empty = all)
    ///
    /// # Returns
    /// Mutable reference to self for method chaining
    ///
    /// See unit tests and `examples/` for usage.
    pub fn register_metric(
        &mut self,
        id: MetricId,
        calculator: Arc<dyn MetricCalculator>,
        applicable_to: &[InstrumentType],
    ) -> &mut Self {
        let entry = self.entries.entry(id).or_default();
        if applicable_to.is_empty() {
            entry.default = Some(calculator);
        } else {
            for instrument_type in applicable_to {
                entry
                    .per_instrument
                    .insert(*instrument_type, Arc::clone(&calculator));
            }
        }
        self
    }

    /// Checks if a metric is registered.
    ///
    /// # Arguments
    /// * `id` - Metric ID to check
    ///
    /// # Returns
    /// `true` if the metric is registered, `false` otherwise
    ///
    /// See unit tests and `examples/` for usage.
    pub fn has_metric(&self, id: MetricId) -> bool {
        self.entries.contains_key(&id)
    }

    /// Gets a list of all registered metric IDs.
    ///
    /// # Returns
    /// Vector of all registered metric IDs
    ///
    /// See unit tests and `examples/` for usage.
    pub fn available_metrics(&self) -> Vec<MetricId> {
        let mut v: Vec<MetricId> = self.entries.keys().cloned().collect();
        v.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        v
    }

    /// Gets metrics applicable to a specific instrument type.
    ///
    /// Returns all metrics that either apply to all instruments (empty applicability)
    /// or specifically apply to the given instrument type.
    ///
    /// # Arguments
    /// * `instrument_type` - Type of instrument (e.g., "Bond", "IRS")
    ///
    /// # Returns
    /// Vector of metric IDs applicable to the instrument type
    ///
    /// See unit tests and `examples/` for usage.
    pub fn metrics_for_instrument(&self, instrument_type: InstrumentType) -> Vec<MetricId> {
        let mut v: Vec<MetricId> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.applies_to(instrument_type))
            .map(|(id, _)| id.clone())
            .collect();
        v.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        v
    }

    /// Checks if a metric is applicable to a specific instrument type.
    ///
    /// A metric is applicable if it's registered and either applies to all
    /// instruments (empty applicability list) or specifically applies to
    /// the given instrument type.
    ///
    /// # Arguments
    /// * `metric_id` - Metric ID to check
    /// * `instrument_type` - Type of instrument (e.g., "Bond", "IRS")
    ///
    /// # Returns
    /// `true` if the metric is applicable, `false` otherwise
    ///
    /// See unit tests and `examples/` for usage.
    pub fn is_applicable(&self, metric_id: &MetricId, instrument_type: InstrumentType) -> bool {
        self.entries
            .get(metric_id)
            .map(|entry| entry.applies_to(instrument_type))
            .unwrap_or(false)
    }

    /// Gets registered metrics organized by [`super::ids::MetricGroup`].
    ///
    /// Returns only groups that have at least one registered metric. Within
    /// each group, metrics are sorted alphabetically. Useful for building
    /// discovery UIs and documentation.
    pub fn available_metrics_grouped(&self) -> Vec<(super::ids::MetricGroup, Vec<MetricId>)> {
        super::ids::MetricGroup::ALL
            .iter()
            .filter_map(|group| {
                let mut members: Vec<MetricId> = group
                    .metrics()
                    .iter()
                    .filter(|m| self.entries.contains_key(*m))
                    .cloned()
                    .collect();
                if members.is_empty() {
                    None
                } else {
                    members.sort_by(|a, b| a.as_str().cmp(b.as_str()));
                    Some((*group, members))
                }
            })
            .collect()
    }

    /// Computes specific metrics with dependency resolution in strict mode.
    ///
    /// Handles dependency resolution, ordering, caching of intermediate results,
    /// and strict error handling. Metrics are computed in the correct order based
    /// on their dependencies, and results are cached in the context.
    ///
    /// **This method defaults to strict mode** (breaking change from v0.7.0).
    /// Any missing metric, non-applicable metric, or calculation failure will
    /// immediately return an error. For lenient behavior, handle errors
    /// explicitly or use `Instrument::price_with_metrics`.
    ///
    /// # Arguments
    /// * `metric_ids` - Vector of metric IDs to compute
    /// * `context` - Metric context containing instrument and market data
    ///
    /// # Returns
    /// HashMap mapping metric IDs to computed values.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Any requested metric is not registered (`Error::UnknownMetric`)
    /// - Any metric is not applicable to the instrument type (`Error::MetricNotApplicable`)
    /// - Any metric calculation fails (`Error::MetricCalculationFailed`)
    /// - Circular dependencies are detected (`Error::CircularDependency`)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_valuations::metrics::{MetricContext, MetricId, MetricRegistry};
    /// # fn example(registry: &MetricRegistry, mut context: MetricContext) -> finstack_core::Result<()> {
    /// // Strict mode (default): fails fast on any error
    /// let metrics = vec![MetricId::Dv01, MetricId::Convexity];
    /// let results = registry.compute(&metrics, &mut context)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// See unit tests and `examples/` for usage.
    pub fn compute(
        &self,
        metric_ids: &[MetricId],
        context: &mut MetricContext,
    ) -> finstack_core::Result<HashMap<MetricId, f64>> {
        let _span = tracing::debug_span!(
            "metric_registry.compute",
            metric_count = metric_ids.len(),
            instrument_type = %context.instrument.key(),
        )
        .entered();

        // Build dependency graph and compute order for this instrument type
        let instrument_type = context.instrument.key();
        let order = self.resolve_dependencies(metric_ids, instrument_type)?;

        // Compute metrics in dependency order (consume order to avoid cloning MetricId)
        for metric_id in order.into_iter() {
            // Skip if already computed
            if context.computed.contains_key(&metric_id) {
                continue;
            }

            // Check if metric is registered
            let Some(entry) = self.entries.get(&metric_id) else {
                if metric_ids.contains(&metric_id) {
                    return Err(finstack_core::Error::unknown_metric(
                        metric_id.as_str(),
                        self.available_metrics()
                            .iter()
                            .map(|m| m.as_str().to_string())
                            .collect(),
                    ));
                }
                continue;
            };

            // Check if calculator exists for this instrument type
            let Some(calc) = entry.get_for(instrument_type) else {
                if metric_ids.contains(&metric_id) {
                    return Err(finstack_core::Error::metric_not_applicable(
                        metric_id.as_str(),
                        instrument_type.to_string(),
                    ));
                }
                continue;
            };

            // Compute metric
            match calc.calculate(context) {
                Ok(value) => {
                    context.computed.insert(metric_id, value);
                }
                Err(err) => {
                    return Err(finstack_core::Error::metric_calculation_failed(
                        metric_id.as_str(),
                        err,
                    ));
                }
            }
        }

        // Return only the requested metrics
        let mut results = HashMap::default();
        for id in metric_ids {
            if let Some(&value) = context.computed.get(id) {
                results.insert(id.clone(), value);
            }
        }

        Ok(results)
    }

    /// Computes all registered metrics applicable to the instrument.
    ///
    /// This is a convenience method that finds all applicable metrics
    /// for the instrument type and computes them all at once. Useful
    /// for comprehensive analysis or when you want all available metrics.
    ///
    /// # Arguments
    /// * `context` - Metric context containing instrument and market data
    ///
    /// # Returns
    /// HashMap mapping all applicable metric IDs to computed values
    ///
    /// See unit tests and `examples/` for usage.
    pub fn compute_all(
        &self,
        context: &mut MetricContext,
    ) -> finstack_core::Result<HashMap<MetricId, f64>> {
        let instrument_type = context.instrument.key();
        let applicable = self.metrics_for_instrument(instrument_type);
        self.compute(&applicable, context)
    }

    /// Resolves dependencies and returns computation order.
    ///
    /// Uses topological sorting to ensure dependencies are computed first.
    /// This prevents circular dependencies and ensures efficient computation.
    ///
    /// Missing metrics or unavailable calculators are gracefully skipped without
    /// causing the entire resolution to fail. Errors are only raised for
    /// circular dependencies.
    ///
    /// # Arguments
    /// * `metric_ids` - Vector of metric IDs to resolve dependencies for
    /// * `instrument_type` - Type of instrument (for calculator lookup)
    ///
    /// # Returns
    /// Vector of metric IDs in dependency order (dependencies first)
    ///
    /// # Errors
    /// Returns [`Error::CircularDependency`] if circular dependencies are detected
    fn resolve_dependencies(
        &self,
        metric_ids: &[MetricId],
        instrument_type: InstrumentType,
    ) -> finstack_core::Result<Vec<MetricId>> {
        let mut visited = finstack_core::HashSet::default();
        let mut order = Vec::new();
        let mut temp_mark = finstack_core::HashSet::default();
        let mut path = Vec::new();

        for id in metric_ids {
            // Propagate errors (especially circular dependencies)
            self.visit_metric(
                id.clone(),
                instrument_type,
                &mut visited,
                &mut temp_mark,
                &mut order,
                &mut path,
            )?;
        }

        Ok(order)
    }

    /// DFS visit for topological sort with cycle detection.
    ///
    /// Builds the dependency path during recursion to provide detailed
    /// circular dependency diagnostics.
    fn visit_metric(
        &self,
        id: MetricId,
        instrument_type: InstrumentType,
        visited: &mut finstack_core::HashSet<MetricId>,
        temp_mark: &mut finstack_core::HashSet<MetricId>,
        order: &mut Vec<MetricId>,
        path: &mut Vec<MetricId>,
    ) -> finstack_core::Result<()> {
        if visited.contains(&id) {
            return Ok(());
        }

        if temp_mark.contains(&id) {
            // Circular dependency detected - build the cycle path
            path.push(id.clone());
            let cycle_start = path.iter().position(|m| m == &id).unwrap_or(0);
            let cycle_path: Vec<String> = path[cycle_start..]
                .iter()
                .map(|m| m.as_str().to_string())
                .collect();

            return Err(finstack_core::Error::circular_dependency(cycle_path));
        }

        temp_mark.insert(id.clone());
        path.push(id.clone());

        // Get calculator and process dependencies
        // If metric not found or not applicable, just skip it gracefully
        if let Some(entry) = self.entries.get(&id) {
            if let Some(calc) = entry.get_for(instrument_type) {
                let deps = calc.dependencies();
                for dep_id in deps.iter() {
                    // Propagate errors from dependencies
                    self.visit_metric(
                        dep_id.clone(),
                        instrument_type,
                        visited,
                        temp_mark,
                        order,
                        path,
                    )?;
                }
            }
        }

        temp_mark.remove(&id);
        path.pop();
        visited.insert(id.clone());
        // Move id into order (last use)
        order.push(id);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::metrics::core::ids::{MetricGroup, MetricId};
    use crate::metrics::core::traits::{MetricCalculator, MetricContext};
    use crate::pricer::InstrumentType;
    use crate::results::ValuationResult;
    use finstack_core::currency::Currency;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::money::Money;
    use std::sync::Arc;
    use time::Date;

    // Mock instrument for testing
    #[derive(Clone)]
    struct MockInstrument {
        instrument_type: InstrumentType,
        attrs: Attributes,
    }

    crate::impl_empty_cashflow_provider!(
        MockInstrument,
        crate::cashflow::builder::CashflowRepresentation::NoResidual
    );

    impl MockInstrument {
        fn new(instrument_type: InstrumentType) -> Self {
            Self {
                instrument_type,
                attrs: Attributes::default(),
            }
        }
    }

    impl Instrument for MockInstrument {
        fn id(&self) -> &str {
            "mock"
        }

        fn key(&self) -> InstrumentType {
            self.instrument_type
        }

        fn base_value(&self, _ctx: &MarketContext, _as_of: Date) -> finstack_core::Result<Money> {
            Money::try_new(100.0, Currency::USD)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn attributes(&self) -> &Attributes {
            &self.attrs
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attrs
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn price_with_metrics(
            &self,
            _market: &MarketContext,
            _as_of: Date,
            _metrics: &[MetricId],
            _options: crate::instruments::common_impl::traits::PricingOptions,
        ) -> finstack_core::Result<ValuationResult> {
            unimplemented!()
        }
    }

    // Mock calculator that always succeeds
    struct SuccessCalculator {
        value: f64,
        deps: Vec<MetricId>,
    }

    impl MetricCalculator for SuccessCalculator {
        fn calculate(&self, _ctx: &mut MetricContext) -> finstack_core::Result<f64> {
            Ok(self.value)
        }

        fn dependencies(&self) -> &[MetricId] {
            &self.deps
        }
    }

    // Mock calculator that always fails
    struct FailCalculator;

    impl MetricCalculator for FailCalculator {
        fn calculate(&self, _ctx: &mut MetricContext) -> finstack_core::Result<f64> {
            Err(finstack_core::Error::Input(
                finstack_core::InputError::Invalid,
            ))
        }
    }

    // Mock calculator with circular dependency
    struct CircularCalculator {
        deps: Vec<MetricId>,
    }

    impl MetricCalculator for CircularCalculator {
        fn calculate(&self, _ctx: &mut MetricContext) -> finstack_core::Result<f64> {
            Ok(0.0)
        }

        fn dependencies(&self) -> &[MetricId] {
            &self.deps
        }
    }

    fn create_test_context() -> MetricContext {
        let instrument = Arc::new(MockInstrument::new(InstrumentType::Bond));
        let market = Arc::new(MarketContext::new());
        let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
        let base_value = Money::try_new(100.0, Currency::USD).unwrap();
        MetricContext::new(
            instrument,
            market,
            as_of,
            base_value,
            MetricContext::default_config(),
        )
    }

    #[test]
    fn test_strict_mode_unknown_metric() {
        let registry = MetricRegistry::new();
        let mut context = create_test_context();

        // Request unknown metric in strict mode (default)
        let result = registry.compute(&[MetricId::Dv01], &mut context);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, finstack_core::Error::UnknownMetric { .. }));

        // Extract metric_id from error
        if let finstack_core::Error::UnknownMetric { metric_id, .. } = err {
            assert_eq!(metric_id, "dv01");
        }
    }

    #[test]
    fn test_strict_mode_calculation_failure() {
        let mut registry = MetricRegistry::new();
        registry.register_metric(
            MetricId::Dv01,
            Arc::new(FailCalculator),
            &[], // Applies to all instruments
        );

        let mut context = create_test_context();

        // Request metric that fails in strict mode
        let result = registry.compute(&[MetricId::Dv01], &mut context);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            finstack_core::Error::MetricCalculationFailed { .. }
        ));

        // Extract metric_id from error
        if let finstack_core::Error::MetricCalculationFailed { metric_id, .. } = err {
            assert_eq!(metric_id, "dv01");
        }
    }

    #[test]
    fn test_strict_mode_not_applicable() {
        let mut registry = MetricRegistry::new();
        registry.register_metric(
            MetricId::Dv01,
            Arc::new(SuccessCalculator {
                value: 100.0,
                deps: Vec::new(),
            }),
            &[InstrumentType::IRS], // Only applies to IRS, not Bond
        );

        let mut context = create_test_context(); // MockInstrument has type Bond

        // Request metric not applicable to Bond
        let result = registry.compute(&[MetricId::Dv01], &mut context);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            finstack_core::Error::MetricNotApplicable { .. }
        ));

        // Extract fields from error
        if let finstack_core::Error::MetricNotApplicable {
            metric_id,
            instrument_type,
        } = err
        {
            assert_eq!(metric_id, "dv01");
            // InstrumentType::Bond displays as "bond" (snake_case)
            assert_eq!(instrument_type, "bond");
        }
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut registry = MetricRegistry::new();

        // Create circular dependency: A -> B -> A
        let metric_a = MetricId::custom("metric_a");
        let metric_b = MetricId::custom("metric_b");

        registry.register_metric(
            metric_a.clone(),
            Arc::new(CircularCalculator {
                deps: vec![metric_b.clone()],
            }),
            &[],
        );
        registry.register_metric(
            metric_b.clone(),
            Arc::new(CircularCalculator {
                deps: vec![metric_a.clone()],
            }),
            &[],
        );

        let mut context = create_test_context();

        // Request metric with circular dependency
        let result = registry.compute(&[metric_a], &mut context);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            finstack_core::Error::CircularDependency { .. }
        ));

        // Extract path from error
        if let finstack_core::Error::CircularDependency { path } = err {
            // Path should contain both metrics
            assert!(path.iter().any(|m| m.contains("metric_a")));
            assert!(path.iter().any(|m| m.contains("metric_b")));
        }
    }

    #[test]
    fn test_dependency_resolution_error_propagation() {
        let mut registry = MetricRegistry::new();

        // Create dependency chain: A -> B -> C (circular: C -> A)
        let metric_a = MetricId::custom("metric_a");
        let metric_b = MetricId::custom("metric_b");
        let metric_c = MetricId::custom("metric_c");

        registry.register_metric(
            metric_a.clone(),
            Arc::new(CircularCalculator {
                deps: vec![metric_b.clone()],
            }),
            &[],
        );
        registry.register_metric(
            metric_b.clone(),
            Arc::new(CircularCalculator {
                deps: vec![metric_c.clone()],
            }),
            &[],
        );
        registry.register_metric(
            metric_c.clone(),
            Arc::new(CircularCalculator {
                deps: vec![metric_a.clone()],
            }),
            &[],
        );

        let mut context = create_test_context();

        // Request metric with nested circular dependency
        let result = registry.compute(&[metric_a], &mut context);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            finstack_core::Error::CircularDependency { .. }
        ));
    }

    #[test]
    fn test_strict_mode_is_default() {
        let mut registry = MetricRegistry::new();
        registry.register_metric(MetricId::Dv01, Arc::new(FailCalculator), &[]);

        let mut context = create_test_context();

        // Default compute() should use strict mode and fail
        let result = registry.compute(&[MetricId::Dv01], &mut context);
        assert!(result.is_err());
    }

    #[test]
    fn test_dependency_ordering() {
        let mut registry = MetricRegistry::new();

        // Create dependency chain: A -> B -> C
        let metric_a = MetricId::custom("metric_a");
        let metric_b = MetricId::custom("metric_b");
        let metric_c = MetricId::custom("metric_c");

        registry.register_metric(
            metric_a.clone(),
            Arc::new(CircularCalculator {
                deps: vec![metric_b.clone()],
            }),
            &[],
        );
        registry.register_metric(
            metric_b.clone(),
            Arc::new(CircularCalculator {
                deps: vec![metric_c.clone()],
            }),
            &[],
        );
        registry.register_metric(
            metric_c.clone(),
            Arc::new(SuccessCalculator {
                value: 1.0,
                deps: Vec::new(),
            }),
            &[],
        );

        let mut context = create_test_context();

        // Compute should resolve dependencies correctly
        let result = registry.compute(&[metric_a], &mut context);

        assert!(result.is_ok());
        // If we got here, dependencies were resolved in correct order
    }

    #[test]
    fn available_metrics_grouped_is_deterministic_and_standard_only() {
        let mut registry = MetricRegistry::new();

        for id in [
            MetricId::ThetaRollDown,
            MetricId::Dv01,
            MetricId::CleanPrice,
            MetricId::Theta,
            MetricId::custom("user_defined_metric"),
        ] {
            registry.register_metric(
                id,
                Arc::new(SuccessCalculator {
                    value: 1.0,
                    deps: Vec::new(),
                }),
                &[],
            );
        }

        let first = registry.available_metrics_grouped();
        let second = registry.available_metrics_grouped();
        assert_eq!(first, second);

        assert_eq!(
            first,
            vec![
                (MetricGroup::Pricing, vec![MetricId::CleanPrice]),
                (
                    MetricGroup::Carry,
                    vec![MetricId::Theta, MetricId::ThetaRollDown],
                ),
                (MetricGroup::Sensitivity, vec![MetricId::Dv01]),
            ]
        );
    }
}

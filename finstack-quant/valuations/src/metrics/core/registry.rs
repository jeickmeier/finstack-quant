//! Metric registry and computation engine.
//!
//! Manages metric calculators with dependency resolution, caching, and batch
//! computation. The registry handles which metrics apply to which instrument
//! types and ensures efficient computation ordering.

use super::ids::MetricId;
use super::traits::{MetricCalculator, MetricContext};

use crate::pricer::InstrumentType;
use finstack_quant_core::HashMap;
use std::sync::Arc;

/// Metric-registration configuration errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MetricRegistryError {
    /// A default or instrument-specific calculator already owns the key.
    #[error(
        "Duplicate metric registration for metric={metric}{}",
        instrument.map_or(String::new(), |value| format!(" instrument={value}"))
    )]
    DuplicateRegistration {
        /// Metric identifier that already has a calculator.
        metric: MetricId,
        /// Instrument-specific collision, or `None` for a default calculator.
        instrument: Option<InstrumentType>,
    },
}

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
    /// Register a metric calculator with explicit ID and applicability.
    ///
    /// Duplicate default or instrument-specific registrations return
    /// [`MetricRegistryError::DuplicateRegistration`] without mutation. Use
    /// [`Self::replace_metric`] for intentional overwrites.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique metric identifier.
    /// * `calculator` - Metric calculation implementation.
    /// * `applicable_to` - Instrument types for this calculator; empty means
    ///   the default calculator for every instrument.
    pub fn register_metric(
        &mut self,
        id: MetricId,
        calculator: Arc<dyn MetricCalculator>,
        applicable_to: &[InstrumentType],
    ) -> std::result::Result<(), MetricRegistryError> {
        let duplicate_instrument = self.entries.get(&id).and_then(|entry| {
            if applicable_to.is_empty() {
                entry.default.as_ref().map(|_| None)
            } else {
                applicable_to
                    .iter()
                    .find(|instrument_type| entry.per_instrument.contains_key(instrument_type))
                    .copied()
                    .map(Some)
            }
        });
        if let Some(instrument) = duplicate_instrument {
            return Err(MetricRegistryError::DuplicateRegistration {
                metric: id,
                instrument,
            });
        }
        self.replace_metric(id, calculator, applicable_to)
    }

    /// Deliberately replace metric registrations for tests or controlled overrides.
    ///
    /// # Arguments
    ///
    /// * `id` - Canonical metric identifier whose registration is replaced.
    /// * `calculator` - Calculator installed for the selected metric and instrument scope.
    /// * `applicable_to` - Instrument types receiving the calculator. An empty slice
    ///   replaces the metric's default calculator.
    pub fn replace_metric(
        &mut self,
        id: MetricId,
        calculator: Arc<dyn MetricCalculator>,
        applicable_to: &[InstrumentType],
    ) -> std::result::Result<(), MetricRegistryError> {
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
        Ok(())
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

    /// Narrow a candidate metric list to the entries this registry can produce
    /// for one instrument type.
    ///
    /// [`Self::compute`] is strict: it rejects anything requested that has no
    /// calculator for the instrument. That is the right behavior for a
    /// caller-chosen list, but an engine that evaluates a fixed *superset*
    /// against heterogeneous instruments — a composite's legs, a standard Greek
    /// set, an attribution menu — wants the supported part of that superset
    /// instead. Such callers narrow here, so dropping the rest is a visible
    /// decision at the call site rather than a silent filter inside pricing.
    ///
    /// # Arguments
    ///
    /// * `metric_ids` - Candidate identifiers, in caller order; the returned
    ///   subset preserves that order and keeps duplicates as given.
    /// * `instrument_type` - Instrument type whose registered calculators
    ///   decide which candidates survive.
    ///
    /// # Returns
    ///
    /// The candidates for which [`Self::is_applicable`] holds. An empty result
    /// means this registry can produce none of them for `instrument_type`.
    pub fn applicable_subset(
        &self,
        metric_ids: &[MetricId],
        instrument_type: InstrumentType,
    ) -> Vec<MetricId> {
        metric_ids
            .iter()
            .filter(|metric_id| self.is_applicable(metric_id, instrument_type))
            .cloned()
            .collect()
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
    /// **This method is strict.** Unregistered and non-applicable requests are
    /// rejected up front, before any calculator runs, so the reported failure
    /// does not depend on dependency ordering; a calculator that then fails
    /// aborts the call as well. The returned map therefore holds a value for
    /// every requested identifier or the call returns an error — a requested
    /// metric is never silently omitted. Callers that legitimately tolerate
    /// partial support across heterogeneous instruments must narrow
    /// `metric_ids` themselves, for example with [`Self::is_applicable`].
    ///
    /// # Arguments
    /// * `metric_ids` - Metric identifiers to compute, in caller order. Every
    ///   entry must be registered and have a calculator for the context's
    ///   instrument type, unless the context already carries a value a pricer
    ///   seeded for it. The first entry failing that test decides the error.
    /// * `context` - Metric context containing instrument and market data.
    ///   Its `computed` map both satisfies already-seeded requests and receives
    ///   every value produced here, including intermediate dependencies.
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
    /// ```
    /// use finstack_quant_valuations::metrics::{MetricContext, MetricId, MetricRegistry};
    /// # fn example(registry: &MetricRegistry, mut context: MetricContext) -> finstack_quant_core::Result<()> {
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
    ) -> finstack_quant_core::Result<HashMap<MetricId, f64>> {
        let _span = tracing::debug_span!(
            "metric_registry.compute",
            metric_count = metric_ids.len(),
            instrument_type = %context.instrument.key(),
        )
        .entered();

        let instrument_type = context.instrument.key();
        self.reject_unsupported(metric_ids, instrument_type, context)?;
        let order = self.resolve_dependencies(metric_ids, instrument_type, context)?;

        // Compute metrics in dependency order (consume order to avoid cloning
        // MetricId). Requested identifiers were already accepted above; entries
        // reached here without a calculator are dependencies pulled in by the
        // topological sort, which are skipped rather than rejected.
        for metric_id in order.into_iter() {
            if context.computed.contains_key(&metric_id) {
                continue;
            }

            let Some(calc) = self
                .entries
                .get(&metric_id)
                .and_then(|entry| entry.get_for(instrument_type))
            else {
                continue;
            };

            match calc.calculate(context) {
                Ok(value) => {
                    context.computed.insert(metric_id, value);
                }
                Err(err) => {
                    return Err(finstack_quant_core::Error::metric_calculation_failed(
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

    /// Reject requested metrics this registry cannot produce for the
    /// instrument, before any calculator runs.
    ///
    /// Running this ahead of computation keeps the reported failure
    /// deterministic: an unsupported request is reported as such rather than
    /// being masked by whichever other requested metric happened to be
    /// evaluated first in dependency order.
    ///
    /// # Arguments
    ///
    /// * `metric_ids` - Metric identifiers the caller explicitly requested, in
    ///   caller order; the first unsupported entry in that order decides the
    ///   error. Dependencies pulled in later are not checked here, because a
    ///   calculator may declare optional dependencies it tolerates missing.
    /// * `instrument_type` - Instrument type whose registered calculators
    ///   decide applicability; it also appears verbatim in the error message.
    /// * `context` - Metric context whose `computed` map is consulted first: a
    ///   value a pricer already seeded satisfies the request on its own and
    ///   needs no registered calculator.
    ///
    /// # Errors
    ///
    /// Returns `Error::UnknownMetric`, carrying the closest registered names,
    /// when an identifier has no entry at all, and
    /// `Error::MetricNotApplicable`, naming both the metric and
    /// `instrument_type`, when an entry exists but exposes no calculator for
    /// this instrument type.
    fn reject_unsupported(
        &self,
        metric_ids: &[MetricId],
        instrument_type: InstrumentType,
        context: &MetricContext,
    ) -> finstack_quant_core::Result<()> {
        for metric_id in metric_ids {
            if context.computed.contains_key(metric_id) {
                continue;
            }
            let Some(entry) = self.entries.get(metric_id) else {
                return Err(finstack_quant_core::Error::unknown_metric(
                    metric_id.as_str(),
                    super::ids::closest_metric_names(
                        metric_id.as_str(),
                        self.available_metrics().iter().map(MetricId::as_str),
                        super::ids::MAX_METRIC_SUGGESTIONS,
                    ),
                ));
            };
            if !entry.applies_to(instrument_type) {
                return Err(finstack_quant_core::Error::metric_not_applicable(
                    metric_id.as_str(),
                    instrument_type.to_string(),
                ));
            }
        }
        Ok(())
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
    /// Returns `Error::CircularDependency` if circular dependencies are detected
    fn resolve_dependencies(
        &self,
        metric_ids: &[MetricId],
        instrument_type: InstrumentType,
        context: &MetricContext,
    ) -> finstack_quant_core::Result<Vec<MetricId>> {
        let mut state = TopoSortState {
            visited: finstack_quant_core::HashSet::default(),
            temp_mark: finstack_quant_core::HashSet::default(),
            order: Vec::new(),
            path: Vec::new(),
        };

        for id in metric_ids {
            // Propagate errors (especially circular dependencies)
            self.visit_metric(id.clone(), instrument_type, context, &mut state)?;
        }

        Ok(state.order)
    }

    /// DFS visit for topological sort with cycle detection.
    ///
    /// Builds the dependency path during recursion to provide detailed
    /// circular dependency diagnostics.
    fn visit_metric(
        &self,
        id: MetricId,
        instrument_type: InstrumentType,
        context: &MetricContext,
        state: &mut TopoSortState,
    ) -> finstack_quant_core::Result<()> {
        if state.visited.contains(&id) {
            return Ok(());
        }

        if state.temp_mark.contains(&id) {
            // Cycle: `id` is already on `path`; push again and slice from its first occurrence.
            state.path.push(id.clone());
            let cycle_start = state
                .path
                .iter()
                .position(|m| m == &id)
                .unwrap_or(state.path.len() - 1);
            let cycle_path: Vec<String> = state.path[cycle_start..]
                .iter()
                .map(|m| m.as_str().to_string())
                .collect();

            return Err(finstack_quant_core::Error::circular_dependency(cycle_path));
        }

        state.temp_mark.insert(id.clone());
        state.path.push(id.clone());

        // If metric not found or not applicable, just skip it gracefully
        if let Some(entry) = self.entries.get(&id) {
            if let Some(calc) = entry.get_for(instrument_type) {
                // Context-aware so calculators whose dependency set varies with
                // instrument overrides (e.g. Breakeven's sensitivity metric)
                // declare what they actually read from `context.computed`.
                let deps = calc.dynamic_dependencies(context);
                for dep_id in deps.iter() {
                    // Propagate errors from dependencies
                    self.visit_metric(dep_id.clone(), instrument_type, context, state)?;
                }
            }
        }

        state.temp_mark.remove(&id);
        state.path.pop();
        state.visited.insert(id.clone());
        // Move id into order (last use)
        state.order.push(id);

        Ok(())
    }
}

/// Mutable traversal state for the dependency topological sort.
///
/// Bundled into one struct so [`MetricRegistry::visit_metric`] stays within a
/// sane argument count as it recurses.
struct TopoSortState {
    /// Metrics already emitted to `order`.
    visited: finstack_quant_core::HashSet<MetricId>,
    /// Metrics on the current DFS stack, for cycle detection.
    temp_mark: finstack_quant_core::HashSet<MetricId>,
    /// Resolved computation order, dependencies first.
    order: Vec<MetricId>,
    /// Current DFS path, for circular-dependency diagnostics.
    path: Vec<MetricId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::metrics::core::ids::{MetricGroup, MetricId};
    use crate::metrics::core::traits::{MetricCalculator, MetricContext};
    use crate::pricer::InstrumentType;
    use crate::results::ValuationResult;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
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
        /// Test mock: reads no market data.
        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<
            crate::instruments::common_impl::dependencies::MarketDependencies,
        > {
            Ok(crate::instruments::common_impl::dependencies::MarketDependencies::new())
        }

        fn id(&self) -> &str {
            "mock"
        }

        fn key(&self) -> InstrumentType {
            self.instrument_type
        }

        fn base_value(
            &self,
            _ctx: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            Money::new(100.0, Currency::USD)
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

        // Mock: this path is never exercised by the registry tests.
        #[allow(clippy::unimplemented)]
        fn price_with_metrics(
            &self,
            _market: &MarketContext,
            _as_of: Date,
            _metrics: &[MetricId],
            _options: crate::instruments::common_impl::traits::PricingOptions,
        ) -> crate::Result<ValuationResult> {
            unimplemented!()
        }
    }

    // Mock calculator that always succeeds
    struct SuccessCalculator {
        value: f64,
        deps: Vec<MetricId>,
    }

    impl MetricCalculator for SuccessCalculator {
        fn calculate(&self, _ctx: &mut MetricContext) -> finstack_quant_core::Result<f64> {
            Ok(self.value)
        }

        fn dependencies(&self) -> &[MetricId] {
            &self.deps
        }
    }

    // Mock calculator that always fails
    struct FailCalculator;

    impl MetricCalculator for FailCalculator {
        fn calculate(&self, _ctx: &mut MetricContext) -> finstack_quant_core::Result<f64> {
            Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::Invalid,
            ))
        }
    }

    // Mock calculator with circular dependency
    struct CircularCalculator {
        deps: Vec<MetricId>,
    }

    impl MetricCalculator for CircularCalculator {
        fn calculate(&self, _ctx: &mut MetricContext) -> finstack_quant_core::Result<f64> {
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
        let base_value = Money::new(100.0, Currency::USD).unwrap();
        MetricContext::new(
            instrument,
            market,
            as_of,
            base_value,
            MetricContext::default_config(),
        )
    }

    #[test]
    fn duplicate_registration_is_rejected_and_replace_is_explicit() {
        let calculator = |value| {
            Arc::new(SuccessCalculator {
                value,
                deps: Vec::new(),
            }) as Arc<dyn MetricCalculator>
        };
        let mut registry = MetricRegistry::new();
        registry
            .register_metric(MetricId::Dv01, calculator(1.0), &[])
            .expect("first metric registration");
        let error = registry
            .register_metric(MetricId::Dv01, calculator(2.0), &[])
            .expect_err("duplicate metric registration must fail");
        assert_eq!(
            error,
            MetricRegistryError::DuplicateRegistration {
                metric: MetricId::Dv01,
                instrument: None,
            }
        );

        let mut context = create_test_context();
        assert_eq!(
            registry.compute(&[MetricId::Dv01], &mut context).unwrap()["dv01"],
            1.0
        );

        registry
            .replace_metric(MetricId::Dv01, calculator(2.0), &[])
            .expect("explicit metric replacement");
        let mut context = create_test_context();
        assert_eq!(
            registry.compute(&[MetricId::Dv01], &mut context).unwrap()["dv01"],
            2.0
        );
    }

    #[test]
    fn test_strict_mode_unknown_metric() {
        let registry = MetricRegistry::new();
        let mut context = create_test_context();

        // Request unknown metric in strict mode (default)
        let result = registry.compute(&[MetricId::Dv01], &mut context);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            finstack_quant_core::Error::UnknownMetric { .. }
        ));

        // Extract metric_id from error
        if let finstack_quant_core::Error::UnknownMetric { metric_id, .. } = err {
            assert_eq!(metric_id, "dv01");
        }
    }

    #[test]
    fn test_strict_mode_calculation_failure() {
        let mut registry = MetricRegistry::new();
        registry
            .register_metric(
                MetricId::Dv01,
                Arc::new(FailCalculator),
                &[], // Applies to all instruments
            )
            .expect("unique test metric registration");

        let mut context = create_test_context();

        // Request metric that fails in strict mode
        let result = registry.compute(&[MetricId::Dv01], &mut context);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            finstack_quant_core::Error::MetricCalculationFailed { .. }
        ));

        // Extract metric_id from error
        if let finstack_quant_core::Error::MetricCalculationFailed { metric_id, .. } = err {
            assert_eq!(metric_id, "dv01");
        }
    }

    #[test]
    fn test_strict_mode_not_applicable() {
        let mut registry = MetricRegistry::new();
        registry
            .register_metric(
                MetricId::Dv01,
                Arc::new(SuccessCalculator {
                    value: 100.0,
                    deps: Vec::new(),
                }),
                &[InstrumentType::Irs], // Only applies to IRS, not Bond
            )
            .expect("unique test metric registration");

        let mut context = create_test_context(); // MockInstrument has type Bond

        // Request metric not applicable to Bond
        let result = registry.compute(&[MetricId::Dv01], &mut context);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            finstack_quant_core::Error::MetricNotApplicable { .. }
        ));

        // Extract fields from error
        if let finstack_quant_core::Error::MetricNotApplicable {
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

        registry
            .register_metric(
                metric_a.clone(),
                Arc::new(CircularCalculator {
                    deps: vec![metric_b.clone()],
                }),
                &[],
            )
            .expect("unique test metric registration");
        registry
            .register_metric(
                metric_b,
                Arc::new(CircularCalculator {
                    deps: vec![metric_a.clone()],
                }),
                &[],
            )
            .expect("unique test metric registration");

        let mut context = create_test_context();

        // Request metric with circular dependency
        let result = registry.compute(&[metric_a], &mut context);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            finstack_quant_core::Error::CircularDependency { .. }
        ));

        // Extract path from error
        if let finstack_quant_core::Error::CircularDependency { path } = err {
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

        registry
            .register_metric(
                metric_a.clone(),
                Arc::new(CircularCalculator {
                    deps: vec![metric_b.clone()],
                }),
                &[],
            )
            .expect("unique test metric registration");
        registry
            .register_metric(
                metric_b,
                Arc::new(CircularCalculator {
                    deps: vec![metric_c.clone()],
                }),
                &[],
            )
            .expect("unique test metric registration");
        registry
            .register_metric(
                metric_c,
                Arc::new(CircularCalculator {
                    deps: vec![metric_a.clone()],
                }),
                &[],
            )
            .expect("unique test metric registration");

        let mut context = create_test_context();

        // Request metric with nested circular dependency
        let result = registry.compute(&[metric_a], &mut context);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            finstack_quant_core::Error::CircularDependency { .. }
        ));
    }

    #[test]
    fn test_strict_mode_is_default() {
        let mut registry = MetricRegistry::new();
        registry
            .register_metric(MetricId::Dv01, Arc::new(FailCalculator), &[])
            .expect("unique test metric registration");

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

        registry
            .register_metric(
                metric_a.clone(),
                Arc::new(CircularCalculator {
                    deps: vec![metric_b.clone()],
                }),
                &[],
            )
            .expect("unique test metric registration");
        registry
            .register_metric(
                metric_b,
                Arc::new(CircularCalculator {
                    deps: vec![metric_c.clone()],
                }),
                &[],
            )
            .expect("unique test metric registration");
        registry
            .register_metric(
                metric_c,
                Arc::new(SuccessCalculator {
                    value: 1.0,
                    deps: Vec::new(),
                }),
                &[],
            )
            .expect("unique test metric registration");

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
            registry
                .register_metric(
                    id,
                    Arc::new(SuccessCalculator {
                        value: 1.0,
                        deps: Vec::new(),
                    }),
                    &[],
                )
                .expect("unique test metric registration");
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

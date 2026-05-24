//! Parameterized builder for constructing [`ScenarioSpec`](crate::ScenarioSpec) from templates.

use crate::{OperationSpec, ScenarioEngine, ScenarioSpec};
use finstack_core::currency::Currency;
use finstack_core::market_data::hierarchy::ResolutionMode;
use indexmap::IndexMap;

/// A builder for constructing [`ScenarioSpec`] values with parameterized overrides.
///
/// Template factories return builders pre-populated with conventional curve, surface,
/// equity, and FX identifiers. Consumers can override those identifiers to match their
/// own market data conventions before calling [`build`](Self::build).
#[derive(Debug, Clone)]
pub struct ScenarioSpecBuilder {
    id: String,
    name: Option<String>,
    description: Option<String>,
    operations: Vec<OperationSpec>,
    priority: i32,
    resolution_mode: ResolutionMode,
    curve_overrides: IndexMap<String, String>,
    equity_overrides: IndexMap<String, String>,
    fx_overrides: IndexMap<(Currency, Currency), (Currency, Currency)>,
}

impl ScenarioSpecBuilder {
    /// Create a new builder with the given scenario identifier.
    ///
    /// Prefer calling [`ScenarioSpec::builder`](crate::ScenarioSpec::builder)
    /// from user-facing code so the builder entry point is discoverable from
    /// the built type itself.
    ///
    /// # Arguments
    ///
    /// - `id`: Stable identifier for the scenario that will be built.
    ///
    /// # Returns
    ///
    /// A builder with no operations, default priority `0`, and the default
    /// hierarchy resolution mode.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            description: None,
            operations: Vec::new(),
            priority: 0,
            resolution_mode: ResolutionMode::default(),
            curve_overrides: IndexMap::new(),
            equity_overrides: IndexMap::new(),
            fx_overrides: IndexMap::new(),
        }
    }

    /// Override the scenario identifier.
    ///
    /// # Arguments
    ///
    /// - `id`: Replacement scenario identifier.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Set the human-readable scenario name.
    ///
    /// # Arguments
    ///
    /// - `name`: Display name to store in the final [`ScenarioSpec`].
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the optional scenario description.
    ///
    /// # Arguments
    ///
    /// - `description`: Freeform text describing the scenario intent.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the composition priority. Lower values are applied first.
    ///
    /// # Arguments
    ///
    /// - `priority`: Ordering key used by [`crate::ScenarioEngine::try_compose`].
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set how overlapping hierarchy-targeted operations should resolve.
    ///
    /// # Arguments
    ///
    /// - `resolution_mode`: Hierarchy merge policy to store in the final
    ///   [`ScenarioSpec`].
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn resolution_mode(mut self, resolution_mode: ResolutionMode) -> Self {
        self.resolution_mode = resolution_mode;
        self
    }

    /// Append a single operation to the builder.
    ///
    /// # Arguments
    ///
    /// - `operation`: Scenario operation to append in insertion order.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn with_operation(mut self, operation: OperationSpec) -> Self {
        self.operations.push(operation);
        self
    }

    /// Append multiple operations to the builder.
    ///
    /// # Arguments
    ///
    /// - `operations`: Operations to append in insertion order.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn with_operations(mut self, operations: Vec<OperationSpec>) -> Self {
        self.operations.extend(operations);
        self
    }

    /// Override a conventional curve or surface identifier with a user-specific one.
    ///
    /// This applies to curve, volatility-surface, and base-correlation operations.
    ///
    /// # Arguments
    ///
    /// - `default_id`: Identifier encoded by the template.
    /// - `user_id`: Replacement identifier to use in the built scenario.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn override_curve(mut self, default_id: &str, user_id: &str) -> Self {
        self.curve_overrides
            .insert(default_id.to_string(), user_id.to_string());
        self
    }

    /// Override a conventional equity identifier with a user-specific one.
    ///
    /// # Arguments
    ///
    /// - `default_id`: Equity identifier encoded by the template.
    /// - `user_id`: Replacement identifier to use in the built scenario.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn override_equity(mut self, default_id: &str, user_id: &str) -> Self {
        self.equity_overrides
            .insert(default_id.to_string(), user_id.to_string());
        self
    }

    /// Override a conventional FX pair with a user-specific pair.
    ///
    /// If the replacement pair reverses the original base/quote orientation,
    /// the builder converts the stored percentage shock into the reciprocal FX
    /// move so that economic direction is preserved.
    ///
    /// # Arguments
    ///
    /// - `default`: Template FX pair.
    /// - `user`: Replacement FX pair.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn override_fx(
        mut self,
        default: (Currency, Currency),
        user: (Currency, Currency),
    ) -> Self {
        self.fx_overrides.insert(default, user);
        self
    }

    /// Compose multiple builders into a single builder.
    ///
    /// The composed builder inherits the engine defaults, including the default `"composed"`
    /// identifier, so callers can override it with [`id`](Self::id) when needed.
    ///
    /// # Arguments
    ///
    /// - `builders`: Builders to convert into specs and compose in priority order.
    ///
    /// # Returns
    ///
    /// A new builder containing the composed operations.
    #[must_use]
    pub fn compose(builders: Vec<ScenarioSpecBuilder>) -> Self {
        let specs = builders
            .into_iter()
            .map(ScenarioSpecBuilder::into_spec_without_validation)
            .collect();
        // Builder composition remains unchecked so callers can keep using the
        // fluent builder and validate at the final `build()` boundary.
        let composed = ScenarioEngine::new().compose_inner(specs);

        Self {
            id: composed.id,
            name: composed.name,
            description: composed.description,
            operations: composed.operations,
            priority: composed.priority,
            resolution_mode: composed.resolution_mode,
            curve_overrides: IndexMap::new(),
            equity_overrides: IndexMap::new(),
            fx_overrides: IndexMap::new(),
        }
    }

    /// Resolve overrides and validate the resulting [`ScenarioSpec`].
    ///
    /// # Returns
    ///
    /// A validated [`ScenarioSpec`] with all configured identifier overrides
    /// applied.
    ///
    /// # Errors
    ///
    /// Returns any validation error raised by [`ScenarioSpec::validate`], such
    /// as empty identifiers, invalid operations, or multiple time-roll
    /// operations.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::currency::Currency;
    /// use finstack_scenarios::{CurveKind, OperationSpec, ScenarioSpecBuilder};
    ///
    /// let spec = ScenarioSpecBuilder::new("rates")
    ///     .with_operation(OperationSpec::CurveParallelBp {
    ///         curve_kind: CurveKind::Discount,
    ///         curve_id: "USD_SOFR".into(),
    ///         discount_curve_id: None,
    ///         bp: 25.0,
    ///     })
    ///     .override_curve("USD_SOFR", "USD_OIS")
    ///     .override_fx((Currency::USD, Currency::EUR), (Currency::USD, Currency::EUR))
    ///     .build()?;
    ///
    /// assert_eq!(spec.operations.len(), 1);
    /// # Ok::<(), finstack_scenarios::Error>(())
    /// ```
    pub fn build(mut self) -> crate::Result<ScenarioSpec> {
        self.resolve_overrides();

        let spec = ScenarioSpec {
            id: self.id,
            name: self.name,
            description: self.description,
            operations: self.operations,
            priority: self.priority,
            resolution_mode: self.resolution_mode,
        };
        spec.validate()?;
        Ok(spec)
    }

    fn into_spec_without_validation(mut self) -> ScenarioSpec {
        self.resolve_overrides();

        ScenarioSpec {
            id: self.id,
            name: self.name,
            description: self.description,
            operations: self.operations,
            priority: self.priority,
            resolution_mode: self.resolution_mode,
        }
    }

    fn resolve_overrides(&mut self) {
        for operation in &mut self.operations {
            match operation {
                OperationSpec::CurveParallelBp { curve_id, .. }
                | OperationSpec::CurveNodeBp { curve_id, .. }
                | OperationSpec::VolIndexParallelPts { curve_id, .. }
                | OperationSpec::VolIndexNodePts { curve_id, .. } => {
                    if let Some(replacement) = self.curve_overrides.get(curve_id.as_str()) {
                        *curve_id = replacement.as_str().into();
                    }
                }
                OperationSpec::VolSurfaceParallelPct { surface_id, .. }
                | OperationSpec::VolSurfaceBucketPct { surface_id, .. }
                | OperationSpec::BaseCorrParallelPts { surface_id, .. }
                | OperationSpec::BaseCorrBucketPts { surface_id, .. } => {
                    if let Some(replacement) = self.curve_overrides.get(surface_id.as_str()) {
                        *surface_id = replacement.as_str().into();
                    }
                }
                OperationSpec::EquityPricePct { ids, .. } => {
                    for id in ids {
                        if let Some(replacement) = self.equity_overrides.get(id.as_str()) {
                            *id = replacement.clone();
                        }
                    }
                }
                OperationSpec::MarketFxPct { base, quote, pct } => {
                    if let Some((new_base, new_quote)) = self.fx_overrides.get(&(*base, *quote)) {
                        if (*new_base, *new_quote) == (*quote, *base) {
                            *pct = reciprocal_fx_pct(*pct);
                        }
                        *base = *new_base;
                        *quote = *new_quote;
                    }
                }
                _ => {}
            }
        }
    }
}

fn reciprocal_fx_pct(pct: f64) -> f64 {
    if pct <= -100.0 {
        return f64::NAN;
    }

    ((1.0 / (1.0 + pct / 100.0)) - 1.0) * 100.0
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::{CurveKind, OperationSpec, VolSurfaceKind};
    use finstack_core::currency::Currency;
    use finstack_core::market_data::hierarchy::ResolutionMode;

    #[test]
    fn test_builder_basic_construction() {
        let builder = ScenarioSpecBuilder::new("test_scenario")
            .name("Test Scenario")
            .description("A test scenario")
            .priority(5);

        let spec = builder.build().expect("should build");
        assert_eq!(spec.id, "test_scenario");
        assert_eq!(spec.name.as_deref(), Some("Test Scenario"));
        assert_eq!(spec.description.as_deref(), Some("A test scenario"));
        assert_eq!(spec.priority, 5);
        assert!(spec.operations.is_empty());
    }

    #[test]
    fn test_builder_with_operations() {
        let spec = ScenarioSpecBuilder::new("rates")
            .with_operation(OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD-SOFR".into(),
                discount_curve_id: None,
                bp: 100.0,
            })
            .with_operation(OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Forward,
                curve_id: "EUR-ESTR".into(),
                discount_curve_id: None,
                bp: -50.0,
            })
            .build()
            .expect("should build");

        assert_eq!(spec.operations.len(), 2);
    }

    #[test]
    fn test_builder_preserves_explicit_resolution_mode() {
        let spec = ScenarioSpecBuilder::new("hierarchy")
            .resolution_mode(ResolutionMode::Cumulative)
            .build()
            .expect("should build");

        assert_eq!(spec.resolution_mode, ResolutionMode::Cumulative);
    }

    #[test]
    fn test_builder_compose_preserves_resolution_mode() {
        let composed = ScenarioSpecBuilder::compose(vec![
            ScenarioSpecBuilder::new("one").resolution_mode(ResolutionMode::Cumulative),
            ScenarioSpecBuilder::new("two").resolution_mode(ResolutionMode::Cumulative),
        ])
        .build()
        .expect("should build");

        assert_eq!(composed.resolution_mode, ResolutionMode::Cumulative);
    }

    #[test]
    fn test_builder_curve_override() {
        let spec = ScenarioSpecBuilder::new("test")
            .with_operation(OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD-SOFR".into(),
                discount_curve_id: None,
                bp: 100.0,
            })
            .with_operation(OperationSpec::CurveNodeBp {
                curve_kind: CurveKind::Forward,
                curve_id: "USD-SOFR".into(),
                discount_curve_id: None,
                nodes: vec![("5Y".into(), 25.0)],
                match_mode: crate::TenorMatchMode::Interpolate,
            })
            .with_operation(OperationSpec::BaseCorrParallelPts {
                surface_id: "USD-SOFR".into(),
                points: 0.05,
            })
            .with_operation(OperationSpec::BaseCorrBucketPts {
                surface_id: "USD-SOFR".into(),
                detachment_bps: Some(vec![300]),
                maturities: Some(vec!["5Y".into()]),
                points: 0.03,
            })
            .override_curve("USD-SOFR", "MY_CUSTOM_SOFR")
            .build()
            .expect("should build");

        match &spec.operations[0] {
            OperationSpec::CurveParallelBp { curve_id, .. } => {
                assert_eq!(curve_id, "MY_CUSTOM_SOFR");
            }
            _ => panic!("unexpected operation type"),
        }

        match &spec.operations[1] {
            OperationSpec::CurveNodeBp { curve_id, .. } => {
                assert_eq!(curve_id, "MY_CUSTOM_SOFR");
            }
            _ => panic!("unexpected operation type"),
        }

        match &spec.operations[2] {
            OperationSpec::BaseCorrParallelPts { surface_id, .. } => {
                assert_eq!(surface_id, "MY_CUSTOM_SOFR");
            }
            _ => panic!("unexpected operation type"),
        }

        match &spec.operations[3] {
            OperationSpec::BaseCorrBucketPts { surface_id, .. } => {
                assert_eq!(surface_id, "MY_CUSTOM_SOFR");
            }
            _ => panic!("unexpected operation type"),
        }
    }

    #[test]
    fn test_builder_equity_override() {
        let spec = ScenarioSpecBuilder::new("test")
            .with_operation(OperationSpec::EquityPricePct {
                ids: vec!["SPX".into(), "NDX".into()],
                pct: -20.0,
            })
            .override_equity("SPX", "MY_SPX_INDEX")
            .build()
            .expect("should build");

        match &spec.operations[0] {
            OperationSpec::EquityPricePct { ids, .. } => {
                assert!(ids.contains(&"MY_SPX_INDEX".to_string()));
                assert!(ids.contains(&"NDX".to_string()));
                assert!(!ids.contains(&"SPX".to_string()));
            }
            _ => panic!("unexpected operation type"),
        }
    }

    #[test]
    fn test_builder_fx_override() {
        let spec = ScenarioSpecBuilder::new("test")
            .with_operation(OperationSpec::MarketFxPct {
                base: Currency::EUR,
                quote: Currency::USD,
                pct: -10.0,
            })
            .override_fx(
                (Currency::EUR, Currency::USD),
                (Currency::GBP, Currency::USD),
            )
            .build()
            .expect("should build");

        match &spec.operations[0] {
            OperationSpec::MarketFxPct { base, quote, .. } => {
                assert_eq!(base, &Currency::GBP);
                assert_eq!(quote, &Currency::USD);
            }
            _ => panic!("unexpected operation type"),
        }
    }

    #[test]
    fn test_builder_fx_override_inverts_pct_for_reversed_pair() {
        let spec = ScenarioSpecBuilder::new("test")
            .with_operation(OperationSpec::MarketFxPct {
                base: Currency::EUR,
                quote: Currency::USD,
                pct: -10.0,
            })
            .override_fx(
                (Currency::EUR, Currency::USD),
                (Currency::USD, Currency::EUR),
            )
            .build()
            .expect("should build");

        match &spec.operations[0] {
            OperationSpec::MarketFxPct { base, quote, pct } => {
                assert_eq!(base, &Currency::USD);
                assert_eq!(quote, &Currency::EUR);
                assert!((*pct - 11.111_111_111_111_11).abs() < 1.0e-12);
            }
            _ => panic!("unexpected operation type"),
        }
    }

    #[test]
    fn test_builder_fx_override_rejects_reversed_pair_below_negative_100_percent() {
        let result = ScenarioSpecBuilder::new("test")
            .with_operation(OperationSpec::MarketFxPct {
                base: Currency::EUR,
                quote: Currency::USD,
                pct: -150.0,
            })
            .override_fx(
                (Currency::EUR, Currency::USD),
                (Currency::USD, Currency::EUR),
            )
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_compose() {
        let builder1 = ScenarioSpecBuilder::new("rates")
            .priority(0)
            .with_operation(OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD-SOFR".into(),
                discount_curve_id: None,
                bp: 100.0,
            });

        let builder2 = ScenarioSpecBuilder::new("equity")
            .priority(1)
            .with_operation(OperationSpec::EquityPricePct {
                ids: vec!["SPX".into()],
                pct: -20.0,
            });

        let composed = ScenarioSpecBuilder::compose(vec![builder1, builder2]).id("hybrid");
        let spec = composed.build().expect("should build");

        assert_eq!(spec.id, "hybrid");
        assert_eq!(spec.operations.len(), 2);
    }

    #[test]
    fn test_builder_vol_surface_override() {
        let spec = ScenarioSpecBuilder::new("test")
            .with_operation(OperationSpec::VolSurfaceParallelPct {
                surface_kind: VolSurfaceKind::Equity,
                surface_id: "SPX_VOL".into(),
                pct: 50.0,
            })
            .with_operation(OperationSpec::VolSurfaceBucketPct {
                surface_kind: VolSurfaceKind::Equity,
                surface_id: "SPX_VOL".into(),
                tenors: Some(vec!["1M".into()]),
                strikes: Some(vec![100.0]),
                pct: 25.0,
            })
            .override_curve("SPX_VOL", "MY_VOL_SURFACE")
            .build()
            .expect("should build");

        match &spec.operations[0] {
            OperationSpec::VolSurfaceParallelPct { surface_id, .. } => {
                assert_eq!(surface_id, "MY_VOL_SURFACE");
            }
            _ => panic!("unexpected operation type"),
        }

        match &spec.operations[1] {
            OperationSpec::VolSurfaceBucketPct { surface_id, .. } => {
                assert_eq!(surface_id, "MY_VOL_SURFACE");
            }
            _ => panic!("unexpected operation type"),
        }
    }

    #[test]
    fn test_builder_validation_empty_id() {
        let result = ScenarioSpecBuilder::new("").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_with_operations_batch() {
        let ops = vec![
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "A".into(),
                discount_curve_id: None,
                bp: 10.0,
            },
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "B".into(),
                discount_curve_id: None,
                bp: 20.0,
            },
        ];

        let spec = ScenarioSpecBuilder::new("test")
            .with_operations(ops)
            .build()
            .expect("should build");

        assert_eq!(spec.operations.len(), 2);
    }
}

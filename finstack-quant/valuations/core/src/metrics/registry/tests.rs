use super::*;
use crate::instruments::Instrument;
use crate::metrics::ids::{MetricGroup, MetricId};
use crate::metrics::traits::{MetricCalculator, MetricContext};
use crate::pricer::InstrumentType;
use crate::results::ValuationResult;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use std::sync::Arc;

// Mock instrument for testing
#[derive(Clone)]
struct MockInstrument {
    instrument_type: InstrumentType,
    attrs: Attributes,
    dependencies: crate::instruments::MarketDependencies,
}

crate::impl_empty_cashflow_provider!(
    MockInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl MockInstrument {
    fn new(instrument_type: InstrumentType) -> Self {
        Self {
            instrument_type,
            attrs: Attributes::default(),
            dependencies: crate::instruments::MarketDependencies::new(),
        }
    }
}

impl Instrument for MockInstrument {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }
    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::MarketDependencies> {
        Ok(self.dependencies.clone())
    }

    fn id(&self) -> &str {
        "mock"
    }

    fn key(&self) -> InstrumentType {
        self.instrument_type
    }

    fn base_value(&self, _ctx: &MarketContext, _as_of: Date) -> finstack_quant_core::Result<Money> {
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
        _options: crate::instruments::PricingOptions,
    ) -> std::result::Result<ValuationResult, crate::pricer::PricingError> {
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
        Err(finstack_quant_core::Error::Validation(
            "mock calculator always fails".to_string(),
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

    registry.replace_metric(MetricId::Dv01, calculator(2.0), &[]);
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
fn nonfinite_metric_output_names_the_metric() {
    let mut registry = MetricRegistry::new();
    registry
        .register_metric(
            MetricId::Dv01,
            Arc::new(SuccessCalculator {
                value: f64::NAN,
                deps: Vec::new(),
            }),
            &[],
        )
        .unwrap();
    let err = registry
        .compute(&[MetricId::Dv01], &mut create_test_context())
        .unwrap_err();
    assert!(matches!(
        err,
        finstack_quant_core::Error::MetricCalculationFailed { metric_id, .. }
            if metric_id == "dv01"
    ));
}

#[test]
fn dv01_rejects_one_missing_forward_curve() {
    use crate::metrics::sensitivities::dv01::{Dv01CalculatorConfig, UnifiedDv01Calculator};
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use time::macros::date;

    let mut instrument = MockInstrument::new(InstrumentType::Bond);
    instrument.dependencies.add_forward_curve("PRESENT");
    instrument.dependencies.add_forward_curve("MISSING");
    let forward = ForwardCurve::builder("PRESENT")
        .base_date(date!(2026 - 01 - 01))
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 0.97)])
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let market = MarketContext::new().try_insert(forward).unwrap();
    let err = UnifiedDv01Calculator::new(Dv01CalculatorConfig::parallel_combined())
        .collect_curves(&instrument, &market)
        .unwrap_err();
    assert!(err.to_string().contains("MISSING"));
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

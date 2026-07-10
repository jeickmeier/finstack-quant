//! Tests for all curve type variants (Forecast, Hazard, Inflation).

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::market_data::term_structures::InflationCurve;
use finstack_quant_scenarios::{
    CurveKind, ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec, TenorMatchMode,
};
use finstack_quant_statements::FinancialModelSpec;
use time::Month;

#[test]
fn test_forecast_curve_parallel_shock() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Create forward curve (3M = 0.25 years)
    let curve = ForwardCurve::builder("USD_LIBOR_3M", 0.25)
        .base_date(base_date)
        .knots(vec![(0.0, 0.05), (1.0, 0.055), (5.0, 0.06)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "forward_shock".into(),
        name: Some("Forward Curve Shock".into()),
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Forward,
            curve_id: "USD_LIBOR_3M".into(),
            discount_curve_id: None,
            bp: 25.0, // +25bp
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 1);

    // Verify curve was bumped
    let bumped_curve = market.get_forward("USD_LIBOR_3M").unwrap();
    let forwards = bumped_curve.forwards();
    assert!(forwards[0] > 0.05, "Forward rate should be bumped");
}

#[test]
fn test_par_cds_parallel_shock() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
        .build()
        .unwrap();

    // Create hazard curve
    let curve = HazardCurve::builder("CORP_BBB")
        .base_date(base_date)
        .recovery_rate(0.4)
        .knots(vec![(0.0, 0.0), (1.0, 0.02), (5.0, 0.025)])
        .par_spreads(vec![(1.0, 120.0), (5.0, 150.0)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(discount).insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "hazard_shock".into(),
        name: Some("Hazard Curve Shock".into()),
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::ParCDS,
            curve_id: "CORP_BBB".into(),
            discount_curve_id: Some("USD-OIS".into()),
            bp: 50.0, // +50bp credit spread widening
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 1);

    // Verify curve exists with original ID
    let bumped_curve = market.get_hazard("CORP_BBB").unwrap();
    assert_eq!(bumped_curve.id().as_str(), "CORP_BBB");
}

#[test]
fn test_inflation_curve_parallel_shock() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Create inflation curve
    let curve = InflationCurve::builder("US_CPI")
        .base_date(base_date)
        .base_cpi(300.0)
        .knots(vec![
            (0.0, 300.0),
            (1.0, 306.0), // ~2% inflation
            (5.0, 330.0),
        ])
        .build()
        .unwrap();

    // Add dependency
    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve).insert(ois);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "inflation_shock".into(),
        name: Some("Inflation Curve Shock".into()),
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Inflation,
            curve_id: "US_CPI".into(),
            discount_curve_id: None,
            bp: 100.0, // +100bp = +1% inflation
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 1);

    // Verify curve was bumped
    let bumped_curve = market.get_inflation_curve("US_CPI").unwrap();
    let cpi_levels = bumped_curve.cpi_levels();
    assert!(cpi_levels[1] > 306.0, "CPI level should be bumped");
}

#[test]
fn test_forecast_curve_node_shock() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let curve = ForwardCurve::builder("USD_LIBOR_3M", 0.25)
        .base_date(base_date)
        .knots(vec![(0.0, 0.05), (1.0, 0.055), (5.0, 0.06)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "forward_node".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::Forward,
            curve_id: "USD_LIBOR_3M".into(),
            discount_curve_id: None,
            nodes: vec![("1Y".into(), 50.0)],
            match_mode: TenorMatchMode::Exact,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 1);
}

#[test]
fn test_par_cds_node_shock() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Create discount curve (needed for recalibration)
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
        .build()
        .unwrap();

    let curve = HazardCurve::builder("CORP_BBB")
        .base_date(base_date)
        .recovery_rate(0.4)
        .knots(vec![(0.0, 0.0), (1.0, 0.02), (5.0, 0.025)])
        .par_spreads(vec![(1.0, 120.0), (5.0, 150.0)]) // Adds Par Spreads (in bps) to enable re-calibration path
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(discount).insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "par_cds_node".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::ParCDS,
            curve_id: "CORP_BBB".into(),
            discount_curve_id: None,
            nodes: vec![("5Y".into(), 25.0)],
            match_mode: TenorMatchMode::Exact,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    // Par CDS node shocks should now succeed via approximate hazard bump
    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 1);

    // Check bumped value
    let bumped = market.get_hazard("CORP_BBB").unwrap();
    // Delta spread = 25bp, R=0.4 => Delta Lambda approx 25bp / 0.6 = 41.67bp
    // Original 5Y lambda = 0.025. New approx 0.025 + 0.004167 = 0.029167

    // We can just verify it changed in the right direction
    // After recalibration, knots may have changed, so interpolate at 5.0
    let val_5y = bumped.hazard_rate(5.0);
    assert!(
        val_5y > 0.025,
        "Hazard rate should increase from Par CDS spread bump: got {}",
        val_5y
    );
}

#[test]
fn test_inflation_curve_node_shock() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let curve = InflationCurve::builder("US_CPI")
        .base_date(base_date)
        .base_cpi(300.0)
        .knots(vec![(0.0, 300.0), (1.0, 306.0), (5.0, 330.0)])
        .build()
        .unwrap();

    // Add dependency
    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve).insert(ois);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "inflation_node".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::Inflation,
            curve_id: "US_CPI".into(),
            discount_curve_id: None,
            nodes: vec![("1Y".into(), 50.0)],
            match_mode: TenorMatchMode::Exact,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 1);
}

#[test]
fn test_discount_curve_id_preservation() {
    // Regression test: ensure curve ID is preserved after shock
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("USD_SOFR")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "test".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD_SOFR".into(),
            discount_curve_id: None,
            bp: 50.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    engine.apply(&scenario, &mut ctx).unwrap();

    // Verify original ID is preserved
    let bumped = market.get_discount("USD_SOFR").unwrap();
    assert_eq!(bumped.id().as_str(), "USD_SOFR");
}

#[test]
fn test_all_curve_types_in_one_scenario() {
    // Test applying shocks to multiple curve types simultaneously
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD_SOFR")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98)])
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("USD_LIBOR_3M", 0.25)
        .base_date(base_date)
        .knots(vec![(0.0, 0.05), (1.0, 0.055)])
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("CORP_BBB")
        .base_date(base_date)
        .recovery_rate(0.4)
        .knots(vec![(0.0, 0.0), (1.0, 0.02), (5.0, 0.025)])
        .par_spreads(vec![(1.0, 120.0), (5.0, 150.0)])
        .build()
        .unwrap();

    let inflation = InflationCurve::builder("US_CPI")
        .base_date(base_date)
        .base_cpi(300.0)
        .knots(vec![(0.0, 300.0), (1.0, 306.0)])
        .build()
        .unwrap();

    // Add fallback discount curve for inflation
    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98)])
        .build()
        .unwrap();

    let mut market = MarketContext::new()
        .insert(disc)
        .insert(ois) // Added for inflation bump dependency
        .insert(fwd)
        .insert(hazard)
        .insert(inflation);

    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "all_curves".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD_SOFR".into(),
                discount_curve_id: None,
                bp: 25.0,
            },
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Forward,
                curve_id: "USD_LIBOR_3M".into(),
                discount_curve_id: None,
                bp: 30.0,
            },
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::ParCDS,
                curve_id: "CORP_BBB".into(),
                discount_curve_id: Some("USD-OIS".into()),
                bp: 50.0,
            },
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Inflation,
                curve_id: "US_CPI".into(),
                discount_curve_id: Some("USD-OIS".into()),
                bp: 100.0,
            },
        ],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 4);

    // Verify all curves still accessible with original IDs
    assert!(market.get_discount("USD_SOFR").is_ok());
    assert!(market.get_forward("USD_LIBOR_3M").is_ok());
    assert!(market.get_hazard("CORP_BBB").is_ok());
    assert!(market.get_inflation_curve("US_CPI").is_ok());
}

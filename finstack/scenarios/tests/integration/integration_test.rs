//! Integration tests for scenarios crate.

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::surfaces::VolSurface;
use finstack_core::market_data::term_structures::BaseCorrelationCurve;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_scenarios::{
    CurveKind, ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec, VolSurfaceKind,
};
use finstack_statements::FinancialModelSpec;
use time::Month;

#[test]
fn test_curve_parallel_shock() {
    // Setup market with discount curve
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve);

    // Setup empty model
    let mut model = FinancialModelSpec::new("test", vec![]);

    // Create scenario with parallel curve shock
    let scenario = ScenarioSpec {
        id: "rate_shock".into(),
        name: Some("Rate Shock".into()),
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD-OIS".into(),
            discount_curve_id: None,
            bp: 50.0, // +50bp
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    // Apply scenario
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

    // Verify the bumped curve exists with original ID (ID is preserved for instrument references)
    let bumped_curve = market.get_discount("USD-OIS").unwrap();

    // The curve should be bumped using solve-to-par logic.
    // This differs from simple zero-rate shifting:
    // df_bumped(t) approx df_original(t) * exp(-bp/10000 * t) but exact par rates drive it.
    let df_1y = bumped_curve.df(1.0);
    // Solve-to-par result with settlement_days=0 (for synthetic curve re-calibration)
    let expected_df_1y = 0.974888;
    assert!(
        (df_1y - expected_df_1y).abs() < 1e-4,
        "Expected DF(1Y) ≈ {:.6} after +50bp shock, got {:.6}",
        expected_df_1y,
        df_1y
    );

    // Also verify 5Y point
    let df_5y = bumped_curve.df(5.0);
    // Verify direction only as bootstrap details vary.
    // Original DF ~ 0.90. Solve-to-par implies par rate increased by 50bp, so DF drops.
    assert!(df_5y < 0.90, "DF(5Y) should drop significantly");
}

#[test]
fn test_equity_price_shock() {
    // Setup market with equity price
    let mut market = MarketContext::new()
        .insert_price("SPY", MarketScalar::Price(Money::new(450.0, Currency::USD)));

    // Setup empty model
    let mut model = FinancialModelSpec::new("test", vec![]);

    // Create scenario with equity shock
    let scenario = ScenarioSpec {
        id: "equity_shock".into(),
        name: Some("Equity Shock".into()),
        description: None,
        operations: vec![OperationSpec::EquityPricePct {
            ids: vec!["SPY".into()],
            pct: -10.0, // -10%
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    // Apply scenario
    let engine = ScenarioEngine::new();
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
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

    // Verify shocked price
    let shocked = market.get_price("SPY").unwrap();
    match shocked {
        MarketScalar::Price(money) => {
            let expected = 450.0 * 0.9; // -10%
            assert!((money.amount() - expected).abs() < 1e-6);
        }
        MarketScalar::Unitless(_) => panic!("Expected Price scalar"),
    }
}

#[test]
fn test_scenario_composition() {
    // Create two scenarios
    let s1 = ScenarioSpec {
        id: "base".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD-OIS".into(),
            discount_curve_id: None,
            bp: 25.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let s2 = ScenarioSpec {
        id: "overlay".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "EUR-OIS".into(),
            discount_curve_id: None,
            bp: 30.0,
        }],
        priority: 1,
        resolution_mode: Default::default(),
    };

    // Compose scenarios
    let engine = ScenarioEngine::new();
    let composed = engine
        .try_compose(vec![s1, s2])
        .expect("compose should succeed");

    assert_eq!(composed.operations.len(), 2);
    assert_eq!(composed.id, "base+overlay");
}

#[test]
fn test_vol_surface_parallel_shock() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Create volatility surface
    let surface = VolSurface::builder("SPX")
        .expiries(&[0.25, 0.5, 1.0])
        .strikes(&[90.0, 100.0, 110.0])
        .row(&[0.20, 0.18, 0.22])
        .row(&[0.21, 0.19, 0.23])
        .row(&[0.22, 0.20, 0.24])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert_surface(surface);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "vol_parallel".into(),
        name: Some("Vol Parallel Shock".into()),
        description: None,
        operations: vec![OperationSpec::VolSurfaceParallelPct {
            surface_kind: VolSurfaceKind::Equity,
            surface_id: "SPX".into(),
            pct: 15.0, // +15% vol increase
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

    // Verify shocked surface
    let shocked_surface = market.get_surface("SPX").unwrap();
    let val = shocked_surface
        .value_checked(1.0, 100.0)
        .expect("grid point lookup should succeed");
    let expected = 0.20 * 1.15;
    assert!((val - expected).abs() < 1e-6, "Vol should be shocked");
}

#[test]
fn test_base_correlation_parallel_shock() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Create base correlation curve
    let basecorr = BaseCorrelationCurve::builder("CDX_IG")
        .knots(vec![(3.0, 0.25), (7.0, 0.45), (10.0, 0.60)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(basecorr);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "basecorr_parallel".into(),
        name: Some("Base Corr Parallel Shock".into()),
        description: None,
        operations: vec![OperationSpec::BaseCorrParallelPts {
            surface_id: "CDX_IG".into(),
            points: 0.10, // +10 correlation points
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

    // Verify shocked curve exists (actual shock value testing is covered in bucket_filtering_test.rs)
    let shocked_curve = market.get_base_correlation("CDX_IG").unwrap();
    assert_eq!(shocked_curve.id().as_str(), "CDX_IG");
}

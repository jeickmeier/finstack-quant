//! Par cds bump tests for scenarios.

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::market_data::term_structures::HazardCurve;
use finstack_core::money::Money;
use finstack_scenarios::{
    CurveKind, ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec, TenorMatchMode,
};
use finstack_statements::FinancialModelSpec;
use finstack_valuations::instruments::Bond;
use finstack_valuations::instruments::Instrument;
use time::Month;

#[test]
fn test_par_cds_bump_integration() {
    // Setup market with hazard curve and discount curve
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Create discount curve (needed for recalibration)
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
        .build()
        .unwrap();

    // Create hazard curve with par spreads (needed for recalibration path)
    // Par spread ≈ hazard_rate * 10000 * (1 - recovery)
    // For 1Y: 0.01 * 10000 * 0.6 = 60 bp
    // For 5Y: 0.02 * 10000 * 0.6 = 120 bp
    let hazard = HazardCurve::builder("USD-CDS")
        .base_date(base_date)
        .recovery_rate(0.4)
        .knots(vec![(1.0, 0.01), (5.0, 0.02)])
        .par_spreads(vec![(1.0, 60.0), (5.0, 120.0)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(discount).insert(hazard);
    let mut model = FinancialModelSpec::new("test", vec![]);

    // Apply 10bp Par CDS bump at 5Y
    let scenario = ScenarioSpec {
        id: "par_cds_bump".into(),
        name: Some("Par CDS Bump".into()),
        description: None,
        operations: vec![OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::ParCDS,
            curve_id: "USD-CDS".into(),
            discount_curve_id: None,
            nodes: vec![("5Y".to_string(), 10.0)],
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

    engine
        .apply(&scenario, &mut ctx)
        .expect("Shock application should succeed");

    // Verify result
    let bumped = market.get_hazard("USD-CDS").unwrap();

    // Check lambda at 5.0 (after recalibration, knots may have changed, so interpolate)
    let l_5y = bumped.hazard_rate(5.0);
    let original_lambda = 0.02;

    // With recalibration, the relationship is more complex than a simple shift
    // The key is that the hazard rate should increase when the par spread is bumped up
    println!("Original: {}, Bumped: {}", original_lambda, l_5y);
    assert!(
        l_5y > original_lambda,
        "Hazard rate should increase from Par CDS spread bump: original {}, got {}",
        original_lambda,
        l_5y
    );
}

#[test]
fn test_par_cds_bump_reprices_credit_bond() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.97), (5.0, 0.85), (10.0, 0.70)])
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("USD-CDS")
        .base_date(base_date)
        .recovery_rate(0.4)
        .knots(vec![(1.0, 0.01), (5.0, 0.02)])
        .par_spreads(vec![(1.0, 60.0), (5.0, 120.0)])
        .build()
        .unwrap();

    let market = MarketContext::new().insert(discount).insert(hazard);
    let mut bond = Bond::fixed(
        "CREDIT-BOND",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        base_date,
        maturity,
        "USD-OIS",
    )
    .expect("bond");
    bond.credit_curve_id = Some("USD-CDS".into());

    let pv_base = bond.value(&market, base_date).expect("base pv").amount();

    let scenario = ScenarioSpec {
        id: "par_cds_parallel".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::ParCDS,
            curve_id: "USD-CDS".into(),
            discount_curve_id: Some("USD-OIS".into()),
            bp: 25.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let mut market_after = market;
    let mut model = FinancialModelSpec::new("test", vec![]);
    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market_after,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };
    engine.apply(&scenario, &mut ctx).expect("scenario apply");

    let pv_bumped = bond
        .value(&market_after, base_date)
        .expect("bumped pv")
        .amount();

    assert!(
        pv_bumped < pv_base,
        "wider par spreads should lower long bond PV: base={pv_base}, bumped={pv_bumped}"
    );
}

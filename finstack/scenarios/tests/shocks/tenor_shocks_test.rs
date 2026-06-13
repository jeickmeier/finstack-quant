//! Tests for tenor-based curve node shocks.

use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_scenarios::{
    CurveKind, ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec, TenorMatchMode,
};
use finstack_statements::FinancialModelSpec;
use time::Month;

#[test]
fn test_tenor_exact_match() {
    // Setup market with discount curve
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.98),  // 1Y pillar
            (5.0, 0.90),  // 5Y pillar
            (10.0, 0.80), // 10Y pillar
        ])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    // Create scenario with exact tenor matching at 5Y
    let scenario = ScenarioSpec {
        id: "tenor_exact".into(),
        name: Some("Tenor Exact Match".into()),
        description: None,
        operations: vec![OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD-OIS".into(),
            discount_curve_id: None,
            nodes: vec![("5Y".into(), 25.0)], // +25bp at 5Y
            match_mode: TenorMatchMode::Exact,
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

    // Verify the actual shock was applied
    // Note: The new scenario engine updates the curve in-place (same ID in market context).
    // It does NOT create a suffixed ID like "USD-OIS_bump_25bp" anymore.
    let bumped_curve = market.get_discount("USD-OIS").unwrap();
    let df_5y = bumped_curve.df(5.0);
    // For an exact-match tenor shock, the 5Y node must move in the expected direction.
    // We assert directional correctness and a tight-ish numerical band for determinism.
    assert!(df_5y < 0.90, "DF(5Y) should decrease after +25bp shock");
    assert!(
        (df_5y - 0.888705).abs() < 1e-4,
        "Expected DF(5Y) ≈ {:.6}, got {:.6}",
        0.888705,
        df_5y
    );
}

#[test]
fn test_tenor_exact_not_found() {
    // Setup market with discount curve
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    // Try to shock at 3Y which doesn't exist
    let scenario = ScenarioSpec {
        id: "tenor_not_found".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD-OIS".into(),
            discount_curve_id: None,
            nodes: vec![("3Y".into(), 25.0)], // 3Y doesn't exist
            match_mode: TenorMatchMode::Exact,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    // Apply scenario - should fail
    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };

    let result = engine.apply(&scenario, &mut ctx);
    assert!(result.is_err(), "Expected error for non-existent tenor");
}

#[test]
fn test_tenor_interpolate_mode() {
    // Setup market with discount curve
    // Include knots at 2Y and 4Y so the triangular bump centered at 3Y (region 1.5Y-4.5Y) affects them
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96), // 2Y pillar - inside triangular region
            (4.0, 0.92), // 4Y pillar - inside triangular region
            (5.0, 0.90),
            (10.0, 0.80),
        ])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    // Store original DF at 3Y for comparison
    let original_df_3y = market.get_discount("USD-OIS").unwrap().df(3.0);

    // Shock at 3Y using interpolation
    // Triangular region: prev=1.5Y, target=3Y, next=4.5Y
    // Knots at 2Y and 4Y are inside this region and will be affected
    let scenario = ScenarioSpec {
        id: "tenor_interpolate".into(),
        name: Some("Tenor Interpolate".into()),
        description: None,
        operations: vec![OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD-OIS".into(),
            discount_curve_id: None,
            nodes: vec![("3Y".into(), 50.0)], // +50bp at 3Y (interpolated)
            match_mode: TenorMatchMode::Interpolate,
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

    // Verify shock was applied (interpolated at 3Y)
    // The curve ID remains "USD-OIS".
    let bumped_curve = market.get_discount("USD-OIS").unwrap();
    let df_3y = bumped_curve.df(3.0);
    // With interpolate mode, the shock is distributed via triangular weights
    // The 2Y and 4Y knots are affected, changing the interpolated DF at 3Y
    assert!(
        (df_3y - original_df_3y).abs() > 1e-6,
        "DF at 3Y should have changed after interpolated shock (original: {:.6}, bumped: {:.6})",
        original_df_3y,
        df_3y
    );
}

/// Off-pillar interpolated bumps must deliver the full requested shift at the
/// requested tenor, not the `Σw²`-attenuated value (50% at a midpoint).
///
/// Uses a forward curve because its node path applies the additive shift
/// directly to pillar forwards, making the linear-interpolation delivery math
/// exact.
#[test]
fn test_tenor_interpolate_delivers_full_bump_at_requested_tenor() {
    use finstack_core::market_data::term_structures::ForwardCurve;

    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = ForwardCurve::builder("USD-FWD-3M", 0.25)
        .base_date(base_date)
        .knots(vec![(1.0, 0.040), (2.0, 0.042), (3.0, 0.045)])
        .build()
        .unwrap();
    let mut market = MarketContext::new().insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    // Resolve "30M" exactly the way the adapter does (calendar arithmetic +
    // the curve's day count), so the delivery assertion targets the precise
    // year fraction the bump was centered on.
    let base_curve = market.get_forward("USD-FWD-3M").unwrap();
    let target_date = finstack_core::dates::Tenor::parse("30M")
        .unwrap()
        .add_to_date(
            base_date,
            None,
            finstack_core::dates::BusinessDayConvention::Unadjusted,
        )
        .unwrap();
    let t_star = base_curve
        .day_count()
        .year_fraction(
            base_date,
            target_date,
            finstack_core::dates::DayCountContext::default(),
        )
        .unwrap();
    assert!(
        t_star > 2.0 && t_star < 3.0,
        "resolved tenor {t_star} must land inside the 2Y-3Y segment"
    );
    let original_at_t_star = base_curve.rate(t_star);
    let original_mid = base_curve.rate(2.5); // midpoint of 2Y-3Y segment
    let original_asym = base_curve.rate(1.25); // asymmetric point in 1Y-2Y
    let original_1y = base_curve.rate(1.0);

    let scenario = ScenarioSpec {
        id: "full_delivery".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::Forward,
            curve_id: "USD-FWD-3M".into(),
            discount_curve_id: None,
            nodes: vec![("30M".into(), 50.0)], // +50bp at 2.5Y (segment midpoint)
            match_mode: TenorMatchMode::Interpolate,
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

    let bumped = market.get_forward("USD-FWD-3M").unwrap();
    // Exact delivery at the resolved tenor: the curve must move by the full
    // requested 50bp there (the old Σw² attenuation delivered only ~25bp).
    let realized = bumped.rate(t_star) - original_at_t_star;
    assert!(
        (realized - 0.0050).abs() < 1e-10,
        "requested +50bp at the resolved 30M point ({t_star:.4}Y) must be fully delivered, \
         realized {:.4}bp",
        realized * 1e4
    );
    // Near the nominal 2.5Y midpoint the realized move differs only by the
    // day-count basis between 2.5 and the resolved point — far above the 25bp
    // the attenuated scheme would have delivered.
    let realized_mid = bumped.rate(2.5) - original_mid;
    assert!(
        (realized_mid - 0.0050).abs() < 0.25e-4,
        "realized move at 2.5Y ({:.4}bp) should be within 0.25bp of the request",
        realized_mid * 1e4
    );
    // Pillars outside the bumped segment are untouched.
    assert!(
        (bumped.rate(1.0) - original_1y).abs() < 1e-12,
        "1Y pillar should be unaffected"
    );
    // The 1Y-2Y segment interpolates toward the bumped 2Y pillar but the
    // asymmetric point must not have received its own full bump.
    let leak = bumped.rate(1.25) - original_asym;
    assert!(
        leak < 0.0050,
        "off-segment leakage {:.4}bp should stay below the requested bump",
        leak * 1e4
    );
}

/// W1/W2 regression: `TenorNotFound` in `Exact` mode must identify the
/// curve, not "unknown".
#[test]
fn tenor_not_found_error_names_curve() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "bad_tenor".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD-OIS".into(),
            discount_curve_id: None,
            nodes: vec![("7Y".into(), 10.0)],
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

    let err = engine
        .apply(&scenario, &mut ctx)
        .expect_err("7Y is absent from the curve");
    let msg = err.to_string();
    assert!(
        msg.contains("USD-OIS"),
        "error should name the curve: {msg}"
    );
    assert!(
        !msg.contains("unknown"),
        "error should not say 'unknown': {msg}"
    );
}

/// W1 regression: extrapolation warning must include both range bounds, the
/// curve id, and use the "outside curve range" wording (not the old
/// max-only phrasing).
#[test]
fn extrapolation_warning_includes_both_bounds_and_curve_id() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90), (10.0, 0.80)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "above_range".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD-OIS".into(),
            discount_curve_id: None,
            // 15Y is beyond the 10Y max knot; flat extrapolation to 10Y knot.
            nodes: vec![("15Y".into(), 10.0)],
            match_mode: TenorMatchMode::Interpolate,
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
    let warning = report
        .warnings
        .iter()
        .map(ToString::to_string)
        .find(|w| w.contains("extrapolates"))
        .expect("above-range extrapolation must produce a warning");
    assert!(
        warning.contains("outside curve range"),
        "wording: {warning}"
    );
    assert!(warning.contains("0.00Y"), "min bound missing: {warning}");
    assert!(warning.contains("10.00Y"), "max bound missing: {warning}");
    assert!(warning.contains("USD-OIS"), "curve id missing: {warning}");
}

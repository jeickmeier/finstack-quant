//! Tests for all curve type variants (Forecast, Hazard, Inflation).

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, ForwardCurve, HazardCurve, InflationCurve, Seniority,
};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_scenarios::{
    CurveKind, ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec, TenorMatchMode,
};
use finstack_quant_statements::types::FinancialModelSpec;
use time::Month;

fn calibrate_test_hazard(
    base_date: Date,
    discount: &DiscountCurve,
    curve_id: &str,
    recovery_rate: f64,
    spreads: &[(&str, f64)],
) -> HazardCurve {
    use finstack_quant_calibration::api::{
        engine,
        market_datum::MarketDatum,
        prior_market::PriorMarketObject,
        schema::{
            CalibrationEnvelope, CalibrationPlan, CalibrationSchema, CalibrationStep,
            HazardCurveParams, StepParams,
        },
    };
    use finstack_quant_calibration::quotes::{CdsQuote, MarketQuote, Pillar, QuoteId};
    use finstack_quant_calibration::{CalibrationConfig, CalibrationMethod};
    use finstack_quant_core::types::CurveId;
    use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};

    let mut quotes = Vec::with_capacity(spreads.len());
    let mut quote_ids = Vec::with_capacity(spreads.len());
    for (tenor_str, spread_bp) in spreads {
        let id = QuoteId::new(format!("{curve_id}-{tenor_str}"));
        quote_ids.push(id.clone());
        let pillar_date = Tenor::parse(tenor_str)
            .expect("valid tenor")
            .add_to_date(
                base_date,
                None,
                finstack_quant_core::dates::BusinessDayConvention::Unadjusted,
            )
            .expect("valid pillar date");
        quotes.push(MarketDatum::from(MarketQuote::Cds(
            CdsQuote::CdsParSpread {
                id,
                entity: curve_id.to_string(),
                pillar: Pillar::Date(pillar_date),
                spread_bp: *spread_bp,
                recovery_rate,
                convention: CdsConventionKey {
                    currency: Currency::USD,
                    doc_clause: CdsDocClause::IsdaNa,
                },
            },
        )));
    }
    let mut quote_sets = finstack_quant_core::FxHashMap::default();
    quote_sets.insert("credit".to_string(), quote_ids);
    let envelope = CalibrationEnvelope {
        schema_url: None,
        schema: CalibrationSchema::CURRENT,
        plan: CalibrationPlan {
            id: format!("{curve_id}-cal"),
            description: None,
            quote_sets: quote_sets.into_iter().collect(),
            steps: vec![CalibrationStep {
                id: curve_id.to_string(),
                quote_set: "credit".to_string(),
                params: StepParams::Hazard(HazardCurveParams {
                    curve_id: CurveId::new(curve_id),
                    entity: curve_id.to_string(),
                    seniority: Seniority::Senior,
                    currency: Currency::USD,
                    base_date,
                    discount_curve_id: discount.id().clone(),
                    recovery_rate,
                    notional: 1.0,
                    method: CalibrationMethod::Bootstrap,
                    doc_clause: None,
                    cds_valuation_convention: None,
                }),
            }],
            settings: CalibrationConfig::default(),
        },
        market_data: quotes,
        prior_market: vec![PriorMarketObject::DiscountCurve(discount.clone())],
    };
    let calibrated = engine::calibrate(&envelope).expect("hazard calibration");
    MarketContext::try_from(calibrated.result.final_market)
        .expect("calibrated market")
        .get_hazard(curve_id)
        .expect("calibrated hazard")
        .as_ref()
        .clone()
}

/// 3M projection curve with 5.5% and 6.0% continuously compounded zero rates
/// at 1Y and 5Y.
fn sample_forward_curve(base_date: Date) -> ForwardCurve {
    ForwardCurve::from_zero_rates(
        "USD_LIBOR_3M",
        base_date,
        &[(1.0, 0.055), (5.0, 0.06)],
        finstack_quant_core::math::Compounding::Continuous,
        DayCount::Act365F,
        InterpStyle::LogLinear,
        ExtrapolationPolicy::Extend,
    )
    .unwrap()
}

#[test]
fn test_forecast_curve_parallel_shock() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let curve = sample_forward_curve(base_date);

    let mut market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve");
    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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

    // A parallel forward shock moves the continuously compounded projection
    // zero rate by the requested amount.
    let bumped_curve = market.get_forward("USD_LIBOR_3M").unwrap();
    let zero_1y = -bumped_curve.df(1.0).unwrap().ln();
    assert!(
        (zero_1y - 0.0575).abs() < 1e-12,
        "projection zero rate should be bumped +25bp, got {zero_1y}"
    );
}

#[test]
fn test_par_cds_parallel_shock() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
        .build()
        .unwrap();

    // Create calibrated hazard curve
    let curve = calibrate_test_hazard(
        base_date,
        &discount,
        "CORP_BBB",
        0.4,
        &[("1Y", 120.0), ("5Y", 150.0)],
    );

    let mut market = MarketContext::new()
        .try_insert(discount)
        .expect("insert market curve")
        .try_insert(curve)
        .expect("insert market curve");
    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let mut market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve")
        .try_insert(ois)
        .expect("insert market curve");
    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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

    let curve = sample_forward_curve(base_date);

    let mut market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve");
    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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

    // Keep the discount curve present so the market matches a normal credit setup.
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .knots(vec![(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
        .build()
        .unwrap();

    let curve = calibrate_test_hazard(
        base_date,
        &discount,
        "CORP_BBB",
        0.4,
        &[("1Y", 120.0), ("5Y", 150.0)],
    );
    let orig_5y = curve.hazard_rate(5.0).unwrap();

    let mut market = MarketContext::new()
        .try_insert(discount)
        .expect("insert market curve")
        .try_insert(curve)
        .expect("insert market curve");
    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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

    let report = engine.apply(&scenario, &mut ctx).unwrap();
    assert_eq!(report.operations_applied, 1);

    // Check bumped value
    let bumped = market.get_hazard("CORP_BBB").unwrap();
    let val_5y = bumped.hazard_rate(5.0).unwrap();
    assert!(
        val_5y > orig_5y,
        "Hazard rate should increase from Par CDS spread bump: {val_5y} vs {orig_5y}"
    );
}

#[test]
fn test_inflation_curve_node_shock() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let curve = InflationCurve::builder("US_CPI")
        .base_date(base_date)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
        .knots(vec![(0.0, 300.0), (1.0, 306.0), (5.0, 330.0)])
        .build()
        .unwrap();

    // Add dependency
    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let mut market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve")
        .try_insert(ois)
        .expect("insert market curve");
    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let mut market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve");
    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let fwd = sample_forward_curve(base_date);

    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .knots(vec![(0.0, 1.0), (1.0, 0.98)])
        .build()
        .unwrap();

    let hazard = calibrate_test_hazard(
        base_date,
        &ois,
        "CORP_BBB",
        0.4,
        &[("1Y", 120.0), ("5Y", 150.0)],
    );

    let inflation = InflationCurve::builder("US_CPI")
        .base_date(base_date)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
        .knots(vec![(0.0, 300.0), (1.0, 306.0)])
        .build()
        .unwrap();

    let mut market = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(ois)
        .expect("insert market curve") // Added for inflation bump dependency
        .try_insert(fwd)
        .expect("insert market curve")
        .try_insert(hazard)
        .expect("insert market curve")
        .try_insert(inflation)
        .expect("insert market curve");

    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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

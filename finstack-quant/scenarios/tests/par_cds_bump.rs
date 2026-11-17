//! Par cds bump tests for scenarios.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve, Seniority};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_core::money::Money;
use finstack_quant_scenarios::{
    CurveKind, ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec, TenorMatchMode,
};
use finstack_quant_statements::types::FinancialModelSpec;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
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

#[test]
fn test_par_cds_bump_integration() {
    // Setup market with hazard curve and discount curve
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

    let hazard = calibrate_test_hazard(
        base_date,
        &discount,
        "USD-CDS",
        0.4,
        &[("1Y", 60.0), ("5Y", 120.0)],
    );
    let original_lambda = hazard.hazard_rate(5.0).unwrap();

    let mut market = MarketContext::new().insert(discount).insert(hazard);
    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );

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

    // Check lambda at the shocked 5Y node.
    let l_5y = bumped.hazard_rate(5.0).unwrap();

    // A positive direct shock must increase the target hazard intensity.
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
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .knots(vec![(0.0, 1.0), (1.0, 0.97), (5.0, 0.85), (10.0, 0.70)])
        .build()
        .unwrap();

    let hazard = calibrate_test_hazard(
        base_date,
        &discount,
        "USD-CDS",
        0.4,
        &[("1Y", 60.0), ("5Y", 120.0)],
    );

    let market = MarketContext::new().insert(discount).insert(hazard);
    let mut bond = Bond::fixed(
        "CREDIT-BOND",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        base_date,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
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
    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );
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

/// A time roll combined with a discount bump and a par-CDS bump must succeed
/// even when a discount knot sits exactly on the roll tenor.
///
/// `roll_forward` shifts knot times to `t' = t − dt` and drops only knots with
/// `t' ≤ 0`, so a knot sitting on the roll tenor survives as a tiny positive
/// residual whenever the roll's realized day count does not divide the tenor
/// exactly (a 3M business-day roll of 89/90/91 days leaves `t = 0.25` at
/// `t' = 0.006164 / 0.003425 / 0.000685`).
///
/// The discount bump used to fabricate a synthetic deposit maturing
/// `round(t' · 365.25)` days out — 2, 1 or 0 days for those residuals — while
/// the synthetic index carried a two-business-day settlement lag. The accrual
/// start then landed on or after maturity and schedule construction failed with
/// `Invalid date range: start must be before end`. The bump no longer builds
/// any dates, so residual knots are harmless.
#[test]
fn roll_with_discount_and_par_cds_bump_survives_knot_on_roll_tenor() {
    let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();

    // Each roll tenor under test has a discount knot sitting exactly on it.
    for (period, tenor_years) in [
        ("1M", 1.0_f64 / 12.0),
        ("3M", 0.25_f64),
        ("6M", 0.5_f64),
        ("1Y", 1.0_f64),
    ] {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .knots(vec![
                (0.0, 1.0),
                (1.0 / 12.0, 0.9963),
                (0.25, 0.9888),
                (0.5, 0.9775),
                (1.0, 0.955),
                (2.0, 0.91),
                (3.0, 0.87),
                (5.0, 0.80),
                (10.0, 0.65),
            ])
            .build()
            .unwrap();

        let hazard = calibrate_test_hazard(
            base_date,
            &discount,
            "CORP-HAZARD",
            0.40,
            &[
                ("1Y", 116.639125),
                ("3Y", 134.717567),
                ("5Y", 147.417265),
                ("10Y", 166.063391),
            ],
        );

        let mut market = MarketContext::new().insert(discount).insert(hazard);
        let mut model = FinancialModelSpec::new(
            "test",
            finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                vec![],
            )
            .expect("valid period plan"),
        );

        let scenario = ScenarioSpec {
            id: "roll_and_bump".into(),
            name: None,
            description: None,
            operations: vec![
                OperationSpec::TimeRollForward {
                    period: period.into(),
                    apply_shocks: true,
                    roll_mode: finstack_quant_scenarios::TimeRollMode::BusinessDays,
                },
                OperationSpec::CurveParallelBp {
                    curve_kind: CurveKind::Discount,
                    curve_id: "USD-OIS".into(),
                    discount_curve_id: None,
                    bp: 100.0,
                },
                OperationSpec::CurveParallelBp {
                    curve_kind: CurveKind::ParCDS,
                    curve_id: "CORP-HAZARD".into(),
                    discount_curve_id: Some("USD-OIS".into()),
                    bp: 50.0,
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

        engine
            .apply(&scenario, &mut ctx)
            .unwrap_or_else(|e| panic!("roll {period} + discount bump + par-CDS bump failed: {e}"));

        // The discount bump must still be faithful on the rolled grid.
        let bumped = market.get_discount("USD-OIS").unwrap();
        let rolled_base = bumped.base_date();
        assert!(
            rolled_base > base_date,
            "roll {period} should advance the base date",
        );
        let probe = tenor_years.max(2.0);
        let bumped_df = bumped.df(probe).unwrap();
        assert!(
            bumped_df > 0.0 && bumped_df < 1.0,
            "roll {period}: bumped DF({probe}) out of range: {bumped_df}",
        );
    }
}

fn par_cds_market() -> (finstack_quant_core::dates::Date, MarketContext) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .knots(vec![(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
        .build()
        .unwrap();
    let hazard = calibrate_test_hazard(
        base_date,
        &discount,
        "USD-CDS",
        0.4,
        &[("1Y", 60.0), ("5Y", 120.0)],
    );
    (
        base_date,
        MarketContext::new().insert(discount).insert(hazard),
    )
}

fn apply_par_cds(
    market: &mut MarketContext,
    as_of: Date,
    spec: &ScenarioSpec,
) -> finstack_quant_scenarios::ApplicationReport {
    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );
    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of,
    };
    engine.apply(spec, &mut ctx).expect("apply")
}

#[test]
fn par_cds_bump_rejects_uncalibrated_hazard_curve() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .knots(vec![(0.0, 1.0), (5.0, 0.80)])
        .build()
        .unwrap();
    let hazard = HazardCurve::builder("USD-CDS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots(vec![(1.0, 0.01), (5.0, 0.02)])
        .build()
        .unwrap();
    let mut market = MarketContext::new().insert(discount).insert(hazard);
    let scenario = regression_scenario(
        "reject-uncalibrated",
        vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::ParCDS,
            curve_id: "USD-CDS".into(),
            discount_curve_id: Some("USD-OIS".into()),
            bp: 25.0,
        }],
    );
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };
    assert!(
        ScenarioEngine::new().apply(&scenario, &mut ctx).is_err(),
        "ParCDS bump must reject uncalibrated HazardCurve"
    );
}

#[test]
fn default_engine_solves_to_par_without_explicit_provider_wiring() {
    use finstack_quant_calibration::api::{engine, schema::CalibrationEnvelope};
    let envelope: CalibrationEnvelope = serde_json::from_str(include_str!(
        "../../calibration/examples/market_bootstrap/03_single_name_hazard.json"
    ))
    .unwrap();
    let calibrated = engine::calibrate(&envelope).expect("source calibration");
    let mut market = MarketContext::try_from(calibrated.result.final_market).unwrap();
    let base = market.get_hazard("ISSUER-A-CDS").unwrap().base_date();
    let before = market
        .get_hazard("ISSUER-A-CDS")
        .unwrap()
        .hazard_rate(5.0)
        .unwrap();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };
    ScenarioEngine::new()
        .apply(
            &regression_scenario(
                "solve",
                vec![OperationSpec::CurveParallelBp {
                    curve_kind: CurveKind::ParCDS,
                    curve_id: "ISSUER-A-CDS".into(),
                    discount_curve_id: Some("USD-OIS".into()),
                    bp: 25.0,
                }],
            ),
            &mut ctx,
        )
        .expect("the default engine carries a recalibration provider");

    let after = market
        .get_hazard("ISSUER-A-CDS")
        .unwrap()
        .hazard_rate(5.0)
        .unwrap();
    assert!(after > before, "{after} vs {before}");
}

#[test]
fn sequential_same_curve_first_order_ops_compound() {
    let (base_date, market) = par_cds_market();
    let engine = ScenarioEngine::new();

    let once = ScenarioSpec {
        id: "once".into(),
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
    let twice = ScenarioSpec {
        id: "twice".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::ParCDS,
                curve_id: "USD-CDS".into(),
                discount_curve_id: Some("USD-OIS".into()),
                bp: 25.0,
            },
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::ParCDS,
                curve_id: "USD-CDS".into(),
                discount_curve_id: Some("USD-OIS".into()),
                bp: 25.0,
            },
        ],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let mut once_market = market.clone();
    let mut twice_market = market;
    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );
    let mut ctx = ExecutionContext {
        market: &mut once_market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };
    engine.apply(&once, &mut ctx).expect("once");
    drop(ctx);
    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );
    let mut ctx = ExecutionContext {
        market: &mut twice_market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base_date,
    };
    engine.apply(&twice, &mut ctx).expect("twice");

    let h_once = once_market
        .get_hazard("USD-CDS")
        .unwrap()
        .hazard_rate(5.0)
        .unwrap();
    let h_twice = twice_market
        .get_hazard("USD-CDS")
        .unwrap()
        .hazard_rate(5.0)
        .unwrap();
    assert!(
        h_twice > h_once + 1e-6,
        "second sequential bump must not reuse the first cached curve: once={h_once} twice={h_twice}"
    );
}

#[test]
fn two_independent_par_cds_curves_match_separate_applies() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .knots(vec![(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
        .build()
        .unwrap();
    let ig = calibrate_test_hazard(
        base_date,
        &discount,
        "IG",
        0.4,
        &[("1Y", 60.0), ("5Y", 120.0)],
    );
    let hy = calibrate_test_hazard(
        base_date,
        &discount,
        "HY",
        0.3,
        &[("1Y", 350.0), ("5Y", 490.0)],
    );
    let market = MarketContext::new().insert(discount).insert(ig).insert(hy);

    let together = ScenarioSpec {
        id: "both".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::ParCDS,
                curve_id: "IG".into(),
                discount_curve_id: Some("USD-OIS".into()),
                bp: 10.0,
            },
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::ParCDS,
                curve_id: "HY".into(),
                discount_curve_id: Some("USD-OIS".into()),
                bp: 25.0,
            },
        ],
        priority: 0,
        resolution_mode: Default::default(),
    };
    let mut combined = market.clone();
    apply_par_cds(&mut combined, base_date, &together);

    let mut only_ig = market.clone();
    apply_par_cds(
        &mut only_ig,
        base_date,
        &ScenarioSpec {
            id: "ig".into(),
            name: None,
            description: None,
            operations: vec![together.operations[0].clone()],
            priority: 0,
            resolution_mode: Default::default(),
        },
    );
    let mut only_hy = market;
    apply_par_cds(
        &mut only_hy,
        base_date,
        &ScenarioSpec {
            id: "hy".into(),
            name: None,
            description: None,
            operations: vec![together.operations[1].clone()],
            priority: 0,
            resolution_mode: Default::default(),
        },
    );

    assert!(
        (combined.get_hazard("IG").unwrap().hazard_rate(5.0).unwrap()
            - only_ig.get_hazard("IG").unwrap().hazard_rate(5.0).unwrap())
        .abs()
            < 1e-12
    );
    assert!(
        (combined.get_hazard("HY").unwrap().hazard_rate(5.0).unwrap()
            - only_hy.get_hazard("HY").unwrap().hazard_rate(5.0).unwrap())
        .abs()
            < 1e-12
    );
}

#[test]
fn solve_to_par_after_roll_requotes_surviving_pillars_at_the_horizon() {
    use finstack_quant_calibration::{
        api::{engine, schema::CalibrationEnvelope},
        recalibration::CachedRecalibrationProvider,
    };
    use std::sync::Arc;
    let envelope: CalibrationEnvelope = serde_json::from_str(include_str!(
        "../../calibration/examples/market_bootstrap/03_single_name_hazard.json"
    ))
    .unwrap();
    let calibrated = engine::calibrate(&envelope).expect("source calibration");
    let source = MarketContext::try_from(calibrated.result.final_market).unwrap();
    let original = source.get_hazard("ISSUER-A-CDS").unwrap();
    let base = original.base_date();
    let engine = ScenarioEngine::new()
        .with_recalibration_provider(Arc::new(CachedRecalibrationProvider::default()));
    for (period, discount_bp) in [("1M", 0.0), ("2Y", 0.0), ("1M", 25.0)] {
        let mut horizon_pars = Vec::new();
        for bp in [0.0, 25.0] {
            let mut market = source.clone();
            let scenario = regression_scenario(
                "roll-credit",
                vec![
                    OperationSpec::TimeRollForward {
                        period: period.into(),
                        apply_shocks: true,
                        roll_mode: finstack_quant_scenarios::TimeRollMode::CalendarDays,
                    },
                    OperationSpec::CurveParallelBp {
                        curve_kind: CurveKind::Discount,
                        curve_id: "USD-OIS".into(),
                        discount_curve_id: None,
                        bp: discount_bp,
                    },
                    OperationSpec::CurveParallelBp {
                        curve_kind: CurveKind::ParCDS,
                        curve_id: "ISSUER-A-CDS".into(),
                        discount_curve_id: Some("USD-OIS".into()),
                        bp,
                    },
                ],
            );
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: None,
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base,
            };
            engine
                .apply(&scenario, &mut ctx)
                .expect("horizon credit scenario");
            let mut independent = source.clone();
            let mut independent_ctx = ExecutionContext {
                market: &mut independent,
                model: None,
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base,
            };
            ScenarioEngine::new()
                .with_recalibration_provider(Arc::new(CachedRecalibrationProvider::default()))
                .apply(&scenario, &mut independent_ctx)
                .expect("uncached horizon scenario");
            let expected = independent.get_hazard("ISSUER-A-CDS").unwrap();
            let actual = ctx.market.get_hazard("ISSUER-A-CDS").unwrap();
            for (t, _) in expected.knot_points() {
                assert!(
                    (actual.sp(t).unwrap() - expected.sp(t).unwrap()).abs() < 1e-12,
                    "cache must honor discount dependencies"
                );
            }
            let horizon = ctx.as_of;
            let curve = market.get_hazard("ISSUER-A-CDS").unwrap();
            let recipe = curve.hazard_calibration().expect("fresh horizon recipe");
            assert_eq!(recipe.hazard_params()["base_date"], horizon.to_string());
            assert!(
                recipe
                    .calibration_inputs()
                    .iter()
                    .all(|input| input.pillar_date > horizon)
            );
            if bp == 0.0 && discount_bp == 0.0 {
                let rolled = original.condition(horizon).unwrap();
                for (t, _) in rolled.knot_points() {
                    assert!((curve.sp(t).unwrap() - rolled.sp(t).unwrap()).abs() < 1e-8);
                }
            }
            horizon_pars.push(
                recipe
                    .spread_risk_inputs()
                    .iter()
                    .map(|input| input.quote["spread_bp"].as_f64().unwrap())
                    .collect::<Vec<_>>(),
            );
            // The returned recipe is usable by a subsequent independent request.
            let repeat = regression_scenario(
                "repeat",
                vec![OperationSpec::CurveParallelBp {
                    curve_kind: CurveKind::ParCDS,
                    curve_id: "ISSUER-A-CDS".into(),
                    discount_curve_id: Some("USD-OIS".into()),
                    bp: 0.0,
                }],
            );
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: None,
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: horizon,
            };
            engine.apply(&repeat, &mut ctx).expect("independent replay");
        }
        for (base, bumped) in horizon_pars[0].iter().zip(&horizon_pars[1]) {
            assert!((bumped - base - 25.0).abs() < 1e-8);
        }
    }
}

#[test]
fn standalone_roll_preserves_calibrated_hazard_for_a_later_zero_spread_shock() {
    use finstack_quant_calibration::{
        api::{engine, schema::CalibrationEnvelope},
        recalibration::CachedRecalibrationProvider,
    };
    use std::sync::Arc;

    let envelope: CalibrationEnvelope = serde_json::from_str(include_str!(
        "../../calibration/examples/market_bootstrap/03_single_name_hazard.json"
    ))
    .expect("calibration fixture");
    let calibrated = engine::calibrate(&envelope).expect("source calibration");
    let source = MarketContext::try_from(calibrated.result.final_market).expect("source market");
    let original = source.get_hazard("ISSUER-A-CDS").expect("source hazard");
    let base = original.base_date();
    let engine = ScenarioEngine::new()
        .with_recalibration_provider(Arc::new(CachedRecalibrationProvider::default()));
    for (period, apply_shocks) in [("1M", false), ("2Y", true)] {
        let unrelated_discount = DiscountCurve::builder("EUR-OIS")
            .base_date(base)
            .knots([(0.0, 1.0), (10.0, 0.8)])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("second discount curve");
        let mut market = source.clone().insert(unrelated_discount);
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: base,
        };
        engine
            .apply(
                &regression_scenario(
                    "standalone-roll",
                    vec![OperationSpec::TimeRollForward {
                        period: period.into(),
                        apply_shocks,
                        roll_mode: finstack_quant_scenarios::TimeRollMode::CalendarDays,
                    }],
                ),
                &mut ctx,
            )
            .expect("standalone roll");
        let horizon = ctx.as_of;
        let rolled = ctx
            .market
            .get_hazard("ISSUER-A-CDS")
            .expect("rolled hazard");
        let recipe = rolled.hazard_calibration().expect("usable horizon recipe");
        assert_eq!(recipe.hazard_params()["base_date"], horizon.to_string());
        assert_eq!(recipe.hazard_params()["discount_curve_id"], "USD-OIS");
        assert!(
            recipe
                .calibration_inputs()
                .iter()
                .all(|input| input.pillar_date > horizon)
        );
        let conditional = original.condition(horizon).expect("conditional hazard");
        for (t, _) in conditional.knot_points() {
            assert!((rolled.sp(t).unwrap() - conditional.sp(t).unwrap()).abs() < 1e-8);
        }
        engine
            .apply(
                &regression_scenario(
                    "later-zero-spread",
                    vec![OperationSpec::CurveParallelBp {
                        curve_kind: CurveKind::ParCDS,
                        curve_id: "ISSUER-A-CDS".into(),
                        discount_curve_id: Some("USD-OIS".into()),
                        bp: 0.0,
                    }],
                ),
                &mut ctx,
            )
            .expect("independent horizon replay");
        let replayed = ctx
            .market
            .get_hazard("ISSUER-A-CDS")
            .expect("replayed hazard");
        for (t, _) in rolled.knot_points() {
            assert!((rolled.sp(t).unwrap() - replayed.sp(t).unwrap()).abs() < 1e-8);
        }
    }
}

#[test]
fn default_provider_replays_calibrated_hazard_rolls_without_explicit_wiring() {
    use finstack_quant_calibration::api::{engine, schema::CalibrationEnvelope};
    use finstack_quant_core::market_data::scalars::MarketScalar;

    let envelope: CalibrationEnvelope = serde_json::from_str(include_str!(
        "../../calibration/examples/market_bootstrap/03_single_name_hazard.json"
    ))
    .expect("calibration fixture");
    let calibrated = engine::calibrate(&envelope).expect("source calibration");
    let source = MarketContext::try_from(calibrated.result.final_market)
        .expect("source market")
        .insert_price("SPOT", MarketScalar::unitless(100.0).expect("valid scalar"));
    let original = source.get_hazard("ISSUER-A-CDS").expect("hazard");
    let base = original.base_date();
    // `ScenarioEngine::new()` is given no provider: it carries the default one,
    // so a calibrated hazard keeps a usable recipe at the horizon instead of
    // failing or losing it, with or without further shocks.
    for roll_only in [false, true] {
        let mut market = source.clone();
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: base,
        };
        let mut operations = vec![OperationSpec::TimeRollForward {
            period: "1M".into(),
            apply_shocks: true,
            roll_mode: finstack_quant_scenarios::TimeRollMode::CalendarDays,
        }];
        if !roll_only {
            operations.push(OperationSpec::EquityPricePct {
                ids: vec!["SPOT".into()],
                pct: 10.0,
            });
        }
        ScenarioEngine::new()
            .apply(
                &regression_scenario("default-provider-roll", operations),
                &mut ctx,
            )
            .expect("default engine replays calibrated hazards");
        let horizon = ctx.as_of;
        assert!(horizon > base);
        let rolled = ctx
            .market
            .get_hazard("ISSUER-A-CDS")
            .expect("rolled hazard");
        let recipe = rolled.hazard_calibration().expect("usable horizon recipe");
        assert_eq!(recipe.hazard_params()["base_date"], horizon.to_string());
        let conditional = original.condition(horizon).expect("conditional hazard");
        for (t, _) in conditional.knot_points() {
            assert!((rolled.sp(t).unwrap() - conditional.sp(t).unwrap()).abs() < 1e-8);
        }
    }
}

#[test]
fn zero_day_business_rolls_preserve_recipes_and_allow_subsequent_shocks_without_replay() {
    use finstack_quant_calibration::{
        api::{engine, schema::CalibrationEnvelope},
        recalibration::CachedRecalibrationProvider,
    };
    use finstack_quant_core::market_data::context::MarketContextState;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use std::sync::Arc;
    use time::macros::date;

    // Jan 31 is Saturday: ModifiedFollowing would cross into February, so it
    // selects Friday Jan 30 and produces a genuine zero-day forward roll.
    let fixture =
        include_str!("../../calibration/examples/market_bootstrap/03_single_name_hazard.json")
            .replace("2026-05-08", "2026-01-30");
    let envelope: CalibrationEnvelope =
        serde_json::from_str(&fixture).expect("dated calibration fixture");
    let calibrated = engine::calibrate(&envelope).expect("source calibration");
    let source = MarketContext::try_from(calibrated.result.final_market)
        .expect("source market")
        .insert_price("SPOT", MarketScalar::unitless(100.0).expect("valid scalar"));
    let base = date!(2026 - 01 - 30);
    let original_hazard = source.get_hazard("ISSUER-A-CDS").expect("source hazard");
    let original_discount = source.get_discount("USD-OIS").expect("source discount");
    assert_eq!(original_hazard.base_date(), base);
    let original_recipe = original_hazard
        .hazard_calibration()
        .expect("source recipe")
        .clone();

    for with_provider in [false, true] {
        for apply_shocks in [false, true] {
            let mut market = source.clone();
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: None,
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base,
            };
            let scenario_engine = if with_provider {
                ScenarioEngine::new()
                    .with_recalibration_provider(Arc::new(CachedRecalibrationProvider::default()))
            } else {
                ScenarioEngine::new()
            };
            let report = scenario_engine
                .apply(
                    &regression_scenario(
                        "zero-day-roll",
                        vec![
                            OperationSpec::TimeRollForward {
                                period: "1D".into(),
                                apply_shocks,
                                roll_mode: finstack_quant_scenarios::TimeRollMode::BusinessDays,
                            },
                            OperationSpec::EquityPricePct {
                                ids: vec!["SPOT".into()],
                                pct: 10.0,
                            },
                        ],
                    ),
                    &mut ctx,
                )
                .expect("zero-day roll needs no replay");
            let roll = report.time_roll.expect("roll report");
            assert_eq!((roll.old_date, roll.new_date, roll.days), (base, base, 0));
            assert!(roll.instrument_carry.is_empty());
            assert_eq!(ctx.as_of, base);
            assert!(!report.changes.as_of_changed);
            let hazard = ctx.market.get_hazard("ISSUER-A-CDS").expect("hazard");
            assert!(
                Arc::ptr_eq(&hazard, &original_hazard),
                "zero-day roll must not rebuild hazards"
            );
            assert_eq!(hazard.hazard_calibration(), Some(&original_recipe));
            assert!(
                Arc::ptr_eq(
                    &ctx.market.get_discount("USD-OIS").expect("discount"),
                    &original_discount
                ),
                "zero-day roll must not rebuild discounts"
            );
            assert!(
                ctx.market
                    .get_price("SPOT")
                    .expect("spot")
                    .get_unitless()
                    .is_some_and(
                        |spot| (spot - if apply_shocks { 110.0 } else { 100.0 }).abs() < 1e-12
                    )
            );
            assert_eq!(report.operations_applied, if apply_shocks { 2 } else { 1 });
        }
    }
    let mut market = source;
    let original_snapshot = serde_json::to_value(
        MarketContextState::try_from(&market).expect("coherent market snapshot"),
    )
    .expect("snapshot");
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: base,
    };
    let report = ScenarioEngine::new()
        .apply(
            &regression_scenario(
                "no-op-roll",
                vec![OperationSpec::TimeRollForward {
                    period: "1D".into(),
                    apply_shocks: true,
                    roll_mode: finstack_quant_scenarios::TimeRollMode::BusinessDays,
                }],
            ),
            &mut ctx,
        )
        .expect("roll-only no-op")
        .time_roll
        .expect("roll report");
    assert_eq!(report.days, 0);
    assert_eq!(
        serde_json::to_value(
            MarketContextState::try_from(&*ctx.market).expect("coherent market snapshot")
        )
        .expect("unchanged snapshot"),
        original_snapshot
    );
}

fn regression_scenario(id: &str, operations: Vec<OperationSpec>) -> ScenarioSpec {
    ScenarioSpec {
        id: id.into(),
        operations,
        ..Default::default()
    }
}

#[test]
fn several_failing_par_cds_ops_report_the_first_op_in_order() {
    // Sixteen independent ParCDS replacements on missing hazard curves run as
    // one parallel batch on native targets. The error must name the first op's
    // curve on every run, as the serial (wasm32) path does.
    let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let scenario = ScenarioSpec {
        id: "missing_hazards".into(),
        name: None,
        description: None,
        operations: (0..16)
            .map(|i| OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::ParCDS,
                curve_id: format!("HZ-{i:03}").into(),
                discount_curve_id: None,
                bp: 10.0,
            })
            .collect(),
        priority: 0,
        resolution_mode: Default::default(),
    };
    let engine = ScenarioEngine::new();
    for _ in 0..20 {
        let mut market = MarketContext::new();
        let mut model = FinancialModelSpec::new(
            "test",
            finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                vec![],
            )
            .expect("valid period plan"),
        );
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: base_date,
        };
        let error = engine
            .apply(&scenario, &mut ctx)
            .expect_err("missing hazard curves must fail")
            .to_string();
        assert!(
            error.contains("HZ-000"),
            "expected the first op's curve, got {error}"
        );
    }
}

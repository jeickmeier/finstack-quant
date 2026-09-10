//! Independent spread-unit and crossed-reset contracts for historical scenarios.

use finstack_quant_core::{
    currency::Currency,
    dates::{DayCount, Tenor},
    market_data::{
        context::MarketContext,
        term_structures::{DiscountCurve, ForwardCurve, HazardCurve},
    },
    money::Money,
};
use finstack_quant_scenarios::{
    apply_time_roll_forward, CurveKind, ExecutionContext, HazardBumpMode, OperationSpec,
    ScenarioEngine, ScenarioSpec, TimeRollMode,
};
use finstack_quant_valuations::{
    instruments::{fixed_income::bond::CashflowSpec, Bond, Instrument},
    metrics::risk::{MarketScenario, RiskFactorShift, RiskFactorType},
};
use time::macros::date;

#[test]
fn production_history_requires_quote_recalibration_for_credit_spreads() {
    let origin = date!(2025 - 01 - 02);
    let market = MarketContext::new().insert(
        HazardCurve::builder("ACME-HZD")
            .base_date(origin)
            .recovery_rate(0.4)
            .knots([(1.0, 0.02), (5.0, 0.02)])
            .build()
            .expect("hazard"),
    );
    let scenario = MarketScenario::new(
        origin,
        vec![RiskFactorShift {
            factor: RiskFactorType::CreditSpread {
                curve_id: "ACME-HZD".into(),
                tenor_years: 5.0,
            },
            shift: 0.001,
        }],
    );
    let error = scenario
        .apply(&market, None)
        .expect_err("par-spread shocks require a provider");
    assert!(
        error.to_string().contains("recalibration provider"),
        "{error}"
    );
    assert_eq!(
        market
            .get_hazard("ACME-HZD")
            .expect("source")
            .hazard_rate(5.0),
        0.02
    );
}

#[test]
fn production_history_reuses_the_pricing_boundary_recalibration_provider() {
    use finstack_quant_calibration::{
        api::{engine, schema::CalibrationEnvelope},
        recalibration::CachedRecalibrationProvider,
    };
    use finstack_quant_valuations::{
        instruments::PricingOptions,
        metrics::{risk::MarketHistory, MetricId},
        recalibration::{
            HazardRecalibrationAction, HazardRecalibrationRequest, QuoteBump, RecalibrationProvider,
        },
    };
    use std::sync::Arc;
    let envelope: CalibrationEnvelope = serde_json::from_str(include_str!(
        "../../calibration/examples/market_bootstrap/03_single_name_hazard.json"
    ))
    .expect("calibration inputs");
    let calibrated = engine::execute(&envelope).expect("source calibration");
    let market =
        MarketContext::try_from(calibrated.result.final_market).expect("calibrated market");
    let hazard = market.get_hazard("ISSUER-A-CDS").expect("hazard");
    let origin = hazard.base_date();
    let recipe = hazard.hazard_calibration().expect("replay recipe");
    let tenor = recipe.spread_risk_inputs[0].pillar_time;
    let initial_spread = recipe.spread_risk_inputs[0].quote["spread_bp"]
        .as_f64()
        .expect("spread");
    let provider = Arc::new(CachedRecalibrationProvider::new());
    let rebuilt = provider
        .rebuild_hazard_curve(&HazardRecalibrationRequest {
            hazard: Arc::clone(&hazard),
            source_market: Arc::new(market.clone()),
            target_market: Arc::new(market.clone()),
            discount_curve_id: "USD-OIS".into(),
            doc_clause: None,
            cds_valuation_convention: None,
            deal_quote_override: None,
            action: HazardRecalibrationAction::SpreadBump(QuoteBump::TenorsBp(vec![(tenor, 10.0)])),
        })
        .expect("independent quote replay");
    let shifted_spread = rebuilt
        .hazard_calibration()
        .expect("shifted recipe")
        .spread_risk_inputs[0]
        .quote["spread_bp"]
        .as_f64()
        .expect("shifted spread");
    assert!((shifted_spread - initial_spread - 10.0).abs() < 1e-10);
    let shocked = market.clone().insert(rebuilt.as_ref().clone());
    let mut bond = Bond::fixed(
        "HVAR-CREDIT",
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("coupon"),
        origin,
        // Keep the bond inside the first shocked CDS pillar: later pillar
        // quotes stay fixed and can offset front-end credit exposure.
        date!(2027 - 05 - 08),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("credit bond");
    bond.credit_curve_id = Some("ISSUER-A-CDS".into());
    let expected = bond.value(&shocked, origin).expect("shocked PV").amount()
        - bond.value(&market, origin).expect("source PV").amount();
    let history = MarketHistory::new(
        origin,
        1,
        vec![MarketScenario::new(
            origin,
            vec![RiskFactorShift {
                factor: RiskFactorType::CreditSpread {
                    curve_id: "ISSUER-A-CDS".into(),
                    tenor_years: tenor,
                },
                shift: 0.001,
            }],
        )],
    );
    let result = bond
        .price_with_metrics(
            &market,
            origin,
            &[MetricId::HVar],
            PricingOptions::default()
                .with_market_history(Arc::new(history))
                .with_recalibration_provider(provider),
        )
        .expect("historical full revaluation");
    assert!(expected < 0.0, "credit widening loses value on a long bond");
    assert!(
        (result.measures["hvar"] - expected).abs() < 1e-6,
        "historical P&L must equal the explicitly replayed market P&L"
    );
}

#[test]
fn production_first_order_spread_shock_converts_by_loss_given_default() {
    let origin = date!(2025 - 01 - 02);
    for recovery in [0.0, 0.4, 0.8] {
        let mut market = MarketContext::new().insert(
            HazardCurve::builder("ACME-HZD")
                .base_date(origin)
                .recovery_rate(recovery)
                .knots([(1.0, 0.02), (5.0, 0.02)])
                .build()
                .expect("hazard"),
        );
        let spec = ScenarioSpec {
            id: "spread-units".into(),
            hazard_bump_mode: HazardBumpMode::FirstOrderShift,
            operations: vec![OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::ParCDS,
                curve_id: "ACME-HZD".into(),
                discount_curve_id: None,
                bp: 10.0,
            }],
            ..Default::default()
        };
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: origin,
        };
        ScenarioEngine::new()
            .apply(&spec, &mut ctx)
            .expect("first-order scenario");
        let actual = market
            .get_hazard("ACME-HZD")
            .expect("bumped hazard")
            .hazard_rate(5.0);
        let expected = 0.02 + 0.001 / (1.0 - recovery);
        assert!(
            (actual - expected).abs() < 1e-12,
            "R={recovery}: {actual} vs {expected}"
        );
    }
}

#[test]
fn production_time_roll_materializes_raw_index_fixings_before_curve_roll() {
    let origin = date!(2025 - 01 - 02);
    let reset = date!(2025 - 01 - 03);
    let mut bond = Bond::floating(
        "ROLL-FRN",
        Money::from((1_000_000_i64, Currency::USD)),
        "USD-SOFR-3M",
        150,
        reset,
        date!(2026 - 01 - 03),
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
    )
    .expect("floating bond");
    if let CashflowSpec::Floating(spec) = &mut bond.cashflow_spec {
        spec.rate_spec.reset_lag_days = 0;
    }
    let mut market = MarketContext::new()
        .insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(origin)
                .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.9)])
                .build()
                .expect("discount"),
        )
        .insert(
            ForwardCurve::builder("USD-SOFR-3M", 0.25)
                .base_date(origin)
                .day_count(DayCount::Act360)
                .knots([(0.0, 0.03), (1.0, 0.03), (2.0, 0.03)])
                .build()
                .expect("forward"),
        );
    let mut instruments: Vec<Box<dyn Instrument>> = vec![Box::new(bond)];
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of: origin,
    };
    let report = apply_time_roll_forward(&mut ctx, "4D", TimeRollMode::CalendarDays).expect("roll");
    assert!(
        report.failed_instruments.is_empty(),
        "{:?}",
        report.failed_instruments
    );
    let series = ctx
        .market
        .get_series("FIXING:USD-SOFR-3M")
        .expect("crossed fixing");
    let fixing = series.value_on_exact(reset).expect("reset observation");
    assert!(
        (fixing - 0.03).abs() < 1e-12,
        "raw index must exclude the 150bp coupon spread"
    );
    apply_time_roll_forward(&mut ctx, "1D", TimeRollMode::CalendarDays).expect("repeat roll");
    assert_eq!(
        ctx.market
            .get_series("FIXING:USD-SOFR-3M")
            .expect("retained series")
            .value_on_exact(reset)
            .expect("retained fixing"),
        fixing
    );
}

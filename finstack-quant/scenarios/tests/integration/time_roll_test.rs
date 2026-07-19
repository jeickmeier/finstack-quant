//! Tests for time roll-forward with carry/theta.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_scenarios::{
    ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec, TimeRollMode,
};
use finstack_quant_statements::FinancialModelSpec;
use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations::instruments::{Attributes, Bond, Instrument};
use time::Month;

#[test]
fn test_time_roll_1_day() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "roll_1d".into(),
        name: Some("Roll 1 Day".into()),
        description: None,
        operations: vec![OperationSpec::TimeRollForward {
            period: "1D".into(),
            apply_shocks: false,
            roll_mode: TimeRollMode::BusinessDays,
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

    let original_date = ctx.as_of;
    let report = engine.apply(&scenario, &mut ctx).unwrap();

    assert_eq!(report.operations_applied, 1);

    // Verify date advanced by 1 day
    let expected_date = base_date + time::Duration::days(1);
    assert_eq!(ctx.as_of, expected_date);
    assert_ne!(ctx.as_of, original_date);
}

/// W8 regression: TimeRollForward must reject negative day shifts. The engine
/// is only meaningful for forward time; a negative period (whether produced
/// by Tenor::parse or a downstream calculation) silently corrupts carry and
/// market-data roll. Either Tenor::parse rejects the string (preferred) or
/// apply_time_roll_forward's explicit guard does.
#[test]
fn test_time_roll_negative_period_is_rejected() {
    let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "backward_roll".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::TimeRollForward {
            period: "-1M".into(),
            apply_shocks: false,
            roll_mode: TimeRollMode::Approximate,
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
        .expect_err("backward roll must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("backward")
            || msg.to_ascii_lowercase().contains("negative")
            || msg.to_ascii_lowercase().contains("invalid"),
        "error should describe the negative period, got: {msg}"
    );
}

#[test]
fn test_time_roll_1_month() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "roll_1m".into(),
        name: Some("Roll 1 Month".into()),
        description: None,
        operations: vec![OperationSpec::TimeRollForward {
            period: "1M".into(),
            apply_shocks: false,
            roll_mode: TimeRollMode::BusinessDays,
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

    // Verify date advanced using calendar-aware month addition. 2025-01-01 + 1M
    // is Saturday 2025-02-01, so BusinessDays (ModifiedFollowing) carries the
    // target to Monday 2025-02-03 (33 calendar days).
    let expected_date = base_date + time::Duration::days(33);
    assert_eq!(ctx.as_of, expected_date);
}

#[test]
fn test_time_roll_1_year() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);

    let scenario = ScenarioSpec {
        id: "roll_1y".into(),
        name: Some("Roll 1 Year".into()),
        description: None,
        operations: vec![OperationSpec::TimeRollForward {
            period: "1Y".into(),
            apply_shocks: false,
            roll_mode: TimeRollMode::BusinessDays,
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

    // Verify date advanced by 365 days
    let expected_date = base_date + time::Duration::days(365);
    assert_eq!(ctx.as_of, expected_date);
}

#[test]
fn test_time_roll_with_bond_carry() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Setup discount curve
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(curve);
    let mut model = FinancialModelSpec::new("test", vec![]);

    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    // Create a bond instrument
    let mut instruments: Vec<Box<dyn Instrument>> = vec![Box::new(
        Bond::builder()
            .id("BOND1".into())
            .notional(finstack_quant_core::money::Money::new(100.0, Currency::USD))
            .issue_date(base_date)
            .maturity(base_date + time::Duration::days(730))
            .cashflow_spec(
                CashflowSpec::fixed(
                    0.05,
                    finstack_quant_core::dates::Tenor::annual(),
                    finstack_quant_core::dates::DayCount::Thirty360,
                )
                .expect("finite test coupon"),
            )
            .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .unwrap(),
    )];

    // 2025-01-01 + 1M is Saturday 2025-02-01; BusinessDays carries the target
    // to Monday 2025-02-03 (33 calendar days).
    let expected_date = base_date + time::Duration::days(33);
    use finstack_quant_valuations::metrics::MetricId;

    let pv_base = {
        instruments
            .first()
            .expect("bond instrument")
            .as_ref()
            .value(&market, base_date)
            .expect("pv at base as_of before roll")
            .amount()
    };

    let report = {
        let scenario = ScenarioSpec {
            id: "roll_bond_carry".into(),
            name: None,
            description: None,
            operations: vec![OperationSpec::TimeRollForward {
                period: "1M".into(),
                apply_shocks: false,
                roll_mode: TimeRollMode::BusinessDays,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        };
        let engine = ScenarioEngine::new();
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: Some(&mut instruments),
            rate_bindings: None,
            calendar: None,
            as_of: base_date,
        };
        let report = engine.apply(&scenario, &mut ctx).unwrap();
        assert_eq!(ctx.as_of, expected_date);
        report
    };

    assert_eq!(report.operations_applied, 1);

    // Reprice at the rolled horizon (explicit as_of, not the pre-roll base date).
    let rolled = instruments
        .first()
        .expect("bond instrument")
        .as_ref()
        .price_with_metrics(
            &market,
            expected_date,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("metrics at rolled as_of");
    assert!(
        rolled.value.amount().is_finite(),
        "rolled PV should be finite at rolled as_of"
    );
    assert_ne!(
        rolled.value.amount(),
        pv_base,
        "pricing at roll_report.new_date must differ from base_date PV"
    );
    let theta = *rolled
        .measures
        .get("theta")
        .expect("theta at rolled horizon");
    assert!(
        theta.is_finite(),
        "theta should be computed at rolled as_of"
    );
}

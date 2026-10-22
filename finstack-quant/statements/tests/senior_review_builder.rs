//! Failure-atomic capital-structure construction regressions.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, PeriodId, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Rate;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::capital_structure::{
    BondConventionParams, SwapConventions, SwapParams,
};
use finstack_quant_valuations::instruments::BondConvention;
use time::macros::date;

fn swap_params(id: &str, fixed_rate: f64) -> SwapParams {
    SwapParams {
        id: id.into(),
        notional: Money::from((100_000_i64, Currency::USD)),
        fixed_rate,
        start_date: date!(2025 - 01 - 01),
        maturity_date: date!(2030 - 01 - 01),
        discount_curve_id: "USD-OIS".into(),
        forward_curve_id: "USD-SOFR-3M".into(),
    }
}

fn reject_invalid_instruments(builder: &mut ModelBuilder) {
    let notional = Money::from((100_000_i64, Currency::USD));
    assert!(builder
        .add_bond(
            "bad-bond",
            notional,
            0.05,
            date!(2025 - 01 - 01),
            date!(2024 - 01 - 01),
            "USD-OIS"
        )
        .is_err());
    assert!(builder
        .add_bond_with_convention(BondConventionParams {
            id: "bad-regional-bond".into(),
            notional,
            coupon_rate: Rate::from_decimal(0.05).expect("valid coupon"),
            issue_date: date!(2025 - 01 - 01),
            maturity_date: date!(2024 - 01 - 01),
            convention: BondConvention::UsCorporate,
            discount_curve_id: "USD-OIS".into(),
        })
        .is_err());
    assert!(builder.add_swap(swap_params("bad-swap", f64::NAN)).is_err());
    assert!(builder
        .add_swap_with_conventions(
            swap_params("bad-regional-swap", f64::NAN),
            SwapConventions {
                fixed_frequency: Tenor::semi_annual(),
                fixed_day_count: DayCount::Thirty360,
                float_frequency: Tenor::quarterly(),
                float_day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
            },
        )
        .is_err());
}

#[test]
fn rejected_capital_instruments_preserve_both_builder_states() {
    let mut builder = {
        let mut model_builder = ModelBuilder::new("retained-model");
        model_builder
            .add_bond(
                "retained-bond",
                Money::from((100_000_i64, Currency::USD)),
                0.05,
                date!(2025 - 01 - 01),
                date!(2030 - 01 - 01),
                "USD-OIS",
            )
            .expect("valid initial bond");
        model_builder
    };
    reject_invalid_instruments(&mut builder);
    let quarter = PeriodId::quarter(2025, 1).expect("valid quarter");
    builder
        .periods("2025Q1..Q2", Some("2025Q1"))
        .expect("timeline")
        .value_scalar("retained-value", &[(quarter, 123.0)]);
    reject_invalid_instruments(&mut builder);
    builder
        .add_swap(swap_params("retried-swap", 0.04))
        .expect("successful retry");
    let model = builder.build().expect("retained model is valid");
    assert_eq!(model.id, "retained-model");
    assert_eq!(model.periods.len(), 2);
    assert!(model.periods[0].is_actual);
    assert!(model.has_node("retained-value"));
    let debt = model
        .capital_structure
        .expect("capital structure")
        .debt_instruments;
    assert_eq!(debt.len(), 2);
    assert_eq!(debt[0].id, "retained-bond");
    assert_eq!(debt[1].id, "retried-swap");
}

#[test]
fn failed_model_setters_preserve_state_and_allow_retry() {
    let q1 = PeriodId::quarter(2025, 1).expect("valid quarter");
    let mut retry = ModelBuilder::new("recoverable");
    let mut clean = ModelBuilder::new("recoverable");
    for builder in [&mut retry, &mut clean] {
        builder.value_scalar("revenue", &[(q1, 100.0)]);
        builder.with_meta("marker", serde_json::json!({"retained": true}));
    }
    assert!(retry.periods_explicit(Vec::new()).is_err());
    assert!(retry.periods("not a period range", None).is_err());
    retry
        .periods("2025Q1..Q2", Some("2025Q1"))
        .expect("retry timeline");
    assert!(retry.periods("2026Q1..Q2", None).is_err());
    assert!(retry.compute("revenue", "(").is_err());
    assert!(retry.compute("__reserved", "revenue").is_err());
    assert!(retry.availability_dates("unknown", &[]).is_err());
    assert!(retry
        .add_metric_from_registry(
            "missing.metric",
            &finstack_quant_statements::registry::Registry::new()
        )
        .is_err());
    clean
        .periods("2025Q1..Q2", Some("2025Q1"))
        .expect("clean timeline");
    for builder in [&mut retry, &mut clean] {
        builder.forecast(
            "revenue",
            finstack_quant_statements::types::ForecastSpec::forward_fill(),
        );
        builder
            .compute("profit", "revenue * 0.5")
            .expect("valid formula");
    }
    let actual = retry.build().expect("build after rejected setters");
    let expected = clean.build().expect("build without rejected setters");
    assert_eq!(
        serde_json::to_value(actual).expect("serialize actual"),
        serde_json::to_value(expected).expect("serialize expected")
    );
}

#[test]
fn failed_mixed_formula_preserves_values_forecast_name_and_parent() {
    use finstack_quant_statements::types::{AmountOrScalar, ForecastSpec};
    let q1 = PeriodId::quarter(2025, 1).expect("valid quarter");
    let mut parent = ModelBuilder::new("mixed-recovery");
    parent
        .periods("2025Q1..Q2", Some("2025Q1"))
        .expect("timeline");
    parent.value_scalar("base", &[(q1, 100.0)]);
    parent.with_meta("marker", serde_json::json!("retained"));
    let mut mixed = parent.mixed("mixed");
    mixed.values(&[(q1, AmountOrScalar::scalar(40.0))]);
    mixed.forecast(ForecastSpec::forward_fill());
    mixed.name("Retained name");
    assert!(mixed.formula("(").is_err());
    mixed.formula("base * 0.5").expect("retry formula");
    let ready = mixed.build().expect("finish mixed node");
    let model = ready.build().expect("finish parent model");
    let node = model.nodes.get("mixed").expect("mixed node retained");
    assert_eq!(node.name.as_deref(), Some("Retained name"));
    assert_eq!(node.formula_text.as_deref(), Some("base * 0.5"));
    assert!(node.forecast.is_some());
    assert_eq!(
        node.values.as_ref().expect("values retained").get(&q1),
        Some(&AmountOrScalar::scalar(40.0))
    );
    assert!(model.has_node("base"));
    assert_eq!(
        model.meta.get("marker"),
        Some(&serde_json::json!("retained"))
    );
}

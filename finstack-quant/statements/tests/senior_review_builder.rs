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

fn reject_invalid_instruments<State>(builder: &mut ModelBuilder<State>) {
    let notional = Money::from((100_000_i64, Currency::USD));
    assert!(builder
        .try_add_bond(
            "bad-bond",
            notional,
            0.05,
            date!(2025 - 01 - 01),
            date!(2024 - 01 - 01),
            "USD-OIS"
        )
        .is_err());
    assert!(builder
        .try_add_bond_with_convention(BondConventionParams {
            id: "bad-regional-bond".into(),
            notional,
            coupon_rate: Rate::from_decimal(0.05).expect("valid coupon"),
            issue_date: date!(2025 - 01 - 01),
            maturity_date: date!(2024 - 01 - 01),
            convention: BondConvention::UsCorporate,
            discount_curve_id: "USD-OIS".into(),
        })
        .is_err());
    assert!(builder
        .try_add_swap(swap_params("bad-swap", f64::NAN))
        .is_err());
    assert!(builder
        .try_add_swap_with_conventions(
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
    let mut builder = ModelBuilder::new("retained-model")
        .add_bond(
            "retained-bond",
            Money::from((100_000_i64, Currency::USD)),
            0.05,
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            "USD-OIS",
        )
        .expect("valid initial bond");
    reject_invalid_instruments(&mut builder);
    let quarter = PeriodId::quarter(2025, 1).expect("valid quarter");
    let mut builder = builder
        .periods("2025Q1..Q2", Some("2025Q1"))
        .expect("timeline")
        .value_scalar("retained-value", &[(quarter, 123.0)]);
    reject_invalid_instruments(&mut builder);
    builder
        .try_add_swap(swap_params("retried-swap", 0.04))
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

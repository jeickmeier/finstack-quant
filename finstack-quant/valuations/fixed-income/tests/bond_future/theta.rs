//! Bond-future theta before the physical delivery window.

use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Rate;
use finstack_quant_valuations_core::contract_specs::BondFutureSpecs;
use finstack_quant_valuations_core::instruments::{
    Instrument, ListedFutureTerms, Position, PricingOptions,
};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_core::results::ValuationResult;
use finstack_quant_valuations_fixed_income::bond::{Bond, FloatingBondParams};
use finstack_quant_valuations_fixed_income::bond_future::{BondFuture, DeliverableBond};
use time::macros::date;

fn future(position: Position) -> BondFuture {
    let ctd = Bond::fixed(
        "CTD",
        Money::from((100_000_i64, Currency::USD)),
        Rate::from_decimal(0.04).expect("coupon"),
        date!(2024 - 01 - 15),
        date!(2034 - 01 - 15),
        StubKind::ShortFront,
        "USD-TREASURY",
    )
    .expect("CTD bond");
    BondFuture::builder()
        .id("TYH5".into())
        .terms(
            ListedFutureTerms::new(
                10.0,
                1_000.0,
                Currency::USD,
                125.5,
                date!(2025 - 03 - 20),
                date!(2025 - 03 - 31),
                position,
            )
            .expect("future terms"),
        )
        .delivery_start(date!(2025 - 03 - 21))
        .contract_specs(BondFutureSpecs::ust_10y())
        .deliverable_basket(vec![DeliverableBond {
            bond_id: ctd.id.clone(),
            conversion_factor: 0.8234,
        }])
        .ctd_bond(ctd)
        .discount_curve_id("USD-TREASURY".into())
        .repo_curve_id("USD-REPO".into())
        .build()
        .expect("bond future")
}

fn market(as_of: Date) -> MarketContext {
    let discount = DiscountCurve::builder("USD-TREASURY")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80), (10.0, 0.65)])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("Treasury curve");
    let repo = DiscountCurve::builder("USD-REPO")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.98)])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("repo curve");
    MarketContext::new().insert(discount).insert(repo)
}

fn floating_ctd_future() -> BondFuture {
    let mut future = future(Position::Long);
    // A caller may supply a contractual conversion factor for a floating CTD;
    // only the optional standard conversion-factor calculation requires fixed coupons.
    future.ctd_bond = Some(
        Bond::floating(FloatingBondParams {
            id: "CTD".into(),
            notional: Money::from((100_000_i64, Currency::USD)),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: 100.into(),
            issue_date: date!(2024 - 01 - 15),
            maturity: date!(2034 - 01 - 15),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            discount_curve_id: "USD-TREASURY".into(),
        })
        .expect("floating CTD"),
    );
    future
}

fn theta_result(
    future: &BondFuture,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_valuations_core::Result<ValuationResult> {
    future.price_with_metrics(
        market,
        as_of,
        &[
            MetricId::ThetaCarry,
            MetricId::ThetaRollDown,
            MetricId::ThetaPeriodDays,
            MetricId::Theta,
        ],
        PricingOptions::default(),
    )
}

#[test]
fn theta_matches_held_market_repricing_across_ctd_coupon() {
    let as_of = date!(2025 - 01 - 14);
    let rolled_date = date!(2025 - 01 - 16);
    let market = market(as_of);
    let rolled_market = market.roll_forward(2).expect("rolled market");
    let mut long = future(Position::Long);
    long.metric_pricing_overrides.theta_period = Some(Tenor::parse("2D").expect("tenor"));
    let ctd_schedule = long
        .ctd_bond
        .as_ref()
        .expect("embedded CTD")
        .cashflow_schedule(&market, as_of)
        .expect("CTD schedule");
    assert!(
        ctd_schedule
            .get_flows()
            .iter()
            .any(|flow| { flow.date == date!(2025 - 01 - 15) && flow.amount.amount() > 0.0 })
    );

    let expected = long
        .value(&rolled_market, rolled_date)
        .expect("rolled future PV")
        .checked_sub(long.value(&market, as_of).expect("base future PV"))
        .expect("same currency")
        .amount();
    let result = theta_result(&long, &market, as_of).expect("pre-delivery theta");
    assert_eq!(result.value.currency(), Currency::USD);
    assert!((result.measures["theta"] - expected).abs() < 1e-8);
    assert_eq!(result.measures["theta_carry"], 0.0);
    assert_eq!(result.measures["theta_roll_down"], result.measures["theta"]);
    assert_eq!(result.measures["theta_period_days"], 2.0);

    let mut short = long.clone();
    short.terms.position = Position::Short;
    let short_result = theta_result(&short, &market, as_of).expect("short theta");
    assert!((short_result.measures["theta"] + result.measures["theta"]).abs() < 1e-8);

    let error = long
        .cashflow_schedule(&market, as_of)
        .expect_err("future delivery must not become a standalone cash schedule");
    assert!(error.to_string().contains("typed delivery result"));
}

#[test]
fn theta_holds_a_live_exchange_quote_fixed() {
    let as_of = date!(2025 - 01 - 14);
    let mut future = future(Position::Long);
    future.terms.quoted_price = Some(126.0);
    let result = theta_result(&future, &market(as_of), as_of).expect("quoted theta");
    assert_eq!(result.measures["theta"], 0.0);
    assert_eq!(result.measures["theta_carry"], 0.0);
    assert_eq!(result.measures["theta_period_days"], 1.0);
}

#[test]
fn quoted_theta_does_not_require_irrelevant_ctd_projection_inputs() {
    let as_of = date!(2025 - 01 - 14);
    let market = MarketContext::new();
    let mut future = floating_ctd_future();
    future.terms.quoted_price = Some(126.0);
    assert!(
        future
            .ctd_bond
            .as_ref()
            .expect("CTD")
            .cashflow_schedule(&market, as_of)
            .is_err(),
        "the fixture deliberately lacks CTD projection inputs"
    );
    let value = future.value(&market, as_of).expect("quoted future PV");
    assert_eq!(value.amount(), 5_000.0);
    let result = theta_result(&future, &market, as_of).expect("quoted future theta");
    assert_eq!(result.measures["theta"], 0.0);
    assert_eq!(result.measures["theta_carry"], 0.0);
    assert_eq!(result.measures["theta_period_days"], 1.0);
}

#[test]
fn official_mark_theta_does_not_require_ctd_inputs_before_delivery() {
    let market = MarketContext::new();
    let mut future = floating_ctd_future();
    future.delivery_start = date!(2025 - 03 - 25);
    future.terms.quoted_price = Some(124.5);
    future.terms.settlement_price = Some(126.0);

    for (as_of, expected) in [
        (date!(2025 - 03 - 20), 15_000.0),
        (date!(2025 - 03 - 21), 0.0),
    ] {
        let result = theta_result(&future, &market, as_of).expect("official-mark theta");
        assert_eq!(result.measures["theta"], expected);
        assert_eq!(result.measures["theta_carry"], 0.0);
        assert_eq!(result.measures["theta_roll_down"], expected);
        assert_eq!(result.measures["theta_period_days"], 1.0);
    }

    future.terms.settlement_price = None;
    let error = theta_result(&future, &market, date!(2025 - 03 - 20))
        .expect_err("the rolled mark must still require official settlement");
    assert!(error.to_string().contains("requires settlement_price"));
}

#[test]
fn theta_breakdown_metrics_work_as_single_metric_requests() {
    let as_of = date!(2025 - 01 - 14);
    let future = future(Position::Long);
    let market = market(as_of);
    let theta = future
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            PricingOptions::default(),
        )
        .expect("theta")
        .measures["theta"];
    for (metric, expected) in [
        (MetricId::ThetaCarry, 0.0),
        (MetricId::ThetaRollDown, theta),
        (MetricId::ThetaPeriodDays, 1.0),
    ] {
        let result = future
            .price_with_metrics(
                &market,
                as_of,
                std::slice::from_ref(&metric),
                PricingOptions::default(),
            )
            .expect("standalone theta component");
        assert_eq!(result.measures[&metric], expected);
    }
}

#[test]
fn theta_rejects_horizons_reaching_physical_delivery() {
    let as_of = date!(2025 - 03 - 20);
    let mut future = future(Position::Long);
    // A final price cannot turn physical delivery into an ordinary cash receipt.
    future.terms.settlement_price = Some(126.0);
    let error = theta_result(&future, &market(as_of), as_of)
        .expect_err("one-day horizon reaches the delivery start");
    assert!(
        error
            .to_string()
            .contains("theta horizon reaches physical delivery on 2025-03-21")
    );

    future.metric_pricing_overrides.theta_period = Some(Tenor::monthly());
    let error = theta_result(&future, &market(as_of), as_of)
        .expect_err("expiry-capping must not bypass physical delivery");
    assert!(error.to_string().contains("typed delivery result"));
}

#[test]
fn theta_rejects_an_in_progress_delivery_window() {
    let as_of = date!(2025 - 03 - 25);
    let mut future = future(Position::Long);
    future.terms.settlement_price = Some(126.0);
    let error = theta_result(&future, &market(as_of), as_of)
        .expect_err("delivery requires typed asset and cash state");
    assert!(error.to_string().contains("typed delivery result"));
}

#[test]
fn theta_is_zero_after_final_settlement() {
    let as_of = date!(2025 - 04 - 01);
    let future = future(Position::Long);
    let result = theta_result(&future, &market(as_of), as_of).expect("settled theta");
    assert_eq!(result.value.amount(), 0.0);
    for metric in [
        "theta",
        "theta_carry",
        "theta_roll_down",
        "theta_period_days",
    ] {
        assert_eq!(result.measures[metric], 0.0);
    }
}

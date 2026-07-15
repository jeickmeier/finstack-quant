use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::BondValuator;
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations::models::{
    short_rate_keys, ShortRateTree, ShortRateTreeConfig, TreeModel,
};
use time::macros::date;

fn market(as_of: Date) -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.85), (10.0, 0.70)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    MarketContext::new().insert(curve)
}

fn callable_bond(as_of: Date) -> Bond {
    let mut bond = Bond::fixed(
        "CALLABLE-FRIC",
        Money::new(1000.0, Currency::USD),
        0.06,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    let mut schedule = CallPutSchedule::default();
    schedule.calls.push(CallPut {
        start_date: date!(2027 - 01 - 01),
        end_date: date!(2027 - 01 - 01),
        price_pct_of_par: 102.0,
        make_whole: None,
    });
    bond.call_put = Some(schedule);
    bond
}

#[test]
fn call_friction_raises_callable_price_toward_straight() {
    let as_of = date!(2025 - 01 - 01);
    let market = market(as_of);

    let discount_curve = market.get_discount("USD-OIS").unwrap();
    let dc = discount_curve.day_count();
    let maturity = date!(2030 - 01 - 01);
    let time_to_maturity = dc
        .year_fraction(as_of, maturity, DayCountContext::default())
        .unwrap();

    let steps = 60usize;
    let vol = 0.01;
    let mut tree = ShortRateTree::new(ShortRateTreeConfig {
        steps,
        volatility: vol,
        ..Default::default()
    });
    tree.calibrate(
        &CurveId::new("USD-OIS"),
        discount_curve.as_ref(),
        time_to_maturity,
    )
    .unwrap();

    let mut vars = HashMap::<&'static str, f64>::default();
    vars.insert(short_rate_keys::OAS, 0.0);

    // Straight bond (no callability), priced on the same tree.
    let mut straight = callable_bond(as_of);
    straight.call_put = None;
    let valuator_straight =
        BondValuator::new(straight, &market, as_of, time_to_maturity, steps).unwrap();
    let price_straight = tree
        .price(vars.clone(), time_to_maturity, &market, &valuator_straight)
        .unwrap();

    // Callable with friction = 0
    let mut callable0 = callable_bond(as_of);
    callable0.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_call_friction_cents(0.0);
    let valuator0 = BondValuator::new(callable0, &market, as_of, time_to_maturity, steps).unwrap();
    let price0 = tree
        .price(vars.clone(), time_to_maturity, &market, &valuator0)
        .unwrap();

    // Callable with friction = 200 cents (= 2.00 points)
    let mut callable200 = callable_bond(as_of);
    callable200.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_call_friction_cents(200.0);
    let valuator200 =
        BondValuator::new(callable200, &market, as_of, time_to_maturity, steps).unwrap();
    let price200 = tree
        .price(vars, time_to_maturity, &market, &valuator200)
        .unwrap();

    assert!(
        price200 >= price0,
        "Callable PV should increase with friction. price0={} price200={}",
        price0,
        price200
    );
    assert!(
        price_straight >= price200,
        "Straight PV should be >= callable PV. straight={} price200={}",
        price_straight,
        price200
    );
}

//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_models::{ShortRateTree, ShortRateTreeConfig};
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::BondValuator;
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::pricing_overrides::{
    InstrumentPricingOverrides, ShortRateTreeModel,
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
        Money::new(1000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.06).expect("valid rate fixture"),
        as_of,
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();

    let mut schedule = CallPutSchedule::default();
    schedule.calls.push(CallPut {
        start: date!(2027 - 01 - 01),
        end: date!(2027 - 01 - 01),
        price_pct_of_par: 102.0,
        make_whole: None,
    });
    bond.instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.01);
    bond.call_put = Some(schedule);
    bond
}

#[test]
fn call_friction_raises_callable_price_toward_straight() {
    let as_of = date!(2025 - 01 - 01);
    let market = market(as_of);

    let discount_curve = market.get_discount("USD-OIS").unwrap();
    let day_count = discount_curve.day_count();
    let maturity = date!(2030 - 01 - 01);
    let time_to_maturity = day_count
        .year_fraction(as_of, maturity, DayCountContext::default())
        .unwrap();

    let steps = 60usize;
    let vol = 0.01;
    let mut tree = ShortRateTree::new(ShortRateTreeConfig {
        steps,
        volatility: vol,
        ..Default::default()
    });
    tree.calibrate(discount_curve.as_ref(), time_to_maturity)
        .unwrap();

    let oas_bp = 0.0;

    // Straight bond (no callability), priced on the same tree.
    let mut straight = callable_bond(as_of);
    straight.call_put = None;
    let valuator_straight =
        BondValuator::new(straight, &market, as_of, time_to_maturity, steps).unwrap();
    let price_straight = tree.price(oas_bp, &valuator_straight).unwrap();

    // Callable with friction = 0
    let mut callable0 = callable_bond(as_of);
    callable0.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_call_friction_cents(0.0);
    let valuator0 = BondValuator::new(callable0, &market, as_of, time_to_maturity, steps).unwrap();
    let price0 = tree.price(oas_bp, &valuator0).unwrap();

    // Callable with friction = 200 cents (= 2.00 points)
    let mut callable200 = callable_bond(as_of);
    callable200.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_call_friction_cents(200.0);
    let valuator200 =
        BondValuator::new(callable200, &market, as_of, time_to_maturity, steps).unwrap();
    let price200 = tree.price(oas_bp, &valuator200).unwrap();

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

/// Bit-exact callable-bond values at a non-zero OAS: the OAS reaches the
/// valuator and the short-rate induction unchanged.
#[test]
fn callable_bond_tree_prices_are_bit_pinned() {
    use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_oas;
    use finstack_quant_valuations::pricer::ModelKey;

    let as_of = date!(2025 - 01 - 01);
    let market = market(as_of);
    let discount_curve = market.get_discount("USD-OIS").unwrap();
    let time_to_maturity = discount_curve
        .day_count()
        .year_fraction(as_of, date!(2030 - 01 - 01), DayCountContext::default())
        .unwrap();

    let steps = 60usize;
    let mut bits = Vec::new();
    for config in [
        ShortRateTreeConfig::ho_lee(steps, 0.01),
        ShortRateTreeConfig::bdt(steps, 0.20),
        ShortRateTreeConfig::black_karasinski(steps, 0.20, 0.03),
    ] {
        let mut tree = ShortRateTree::new(config);
        tree.calibrate(discount_curve.as_ref(), time_to_maturity)
            .unwrap();
        let oas_bp = 75.0;
        let valuator = BondValuator::new(
            callable_bond(as_of),
            &market,
            as_of,
            time_to_maturity,
            steps,
        )
        .unwrap();
        bits.push(tree.price(oas_bp, &valuator).unwrap().to_bits());
    }
    // Through the bond tree pricer: Black-Derman-Toy lattice, then the
    // Hull-White trinomial lattice.
    let mut bdt_bond = callable_bond(as_of);
    bdt_bond
        .instrument_pricing_overrides
        .model_config
        .tree_model = Some(ShortRateTreeModel::BlackDermanToy);
    bdt_bond.instrument_pricing_overrides.model_config.bdt_sigma = Some(0.20);
    let mut hw_bond = callable_bond(as_of);
    hw_bond.instrument_pricing_overrides = hw_bond
        .instrument_pricing_overrides
        .with_hw1f_sigma(0.01)
        .with_hw1f_mean_reversion(0.03);
    for bond in [bdt_bond, hw_bond] {
        bits.push(
            price_from_oas(&bond, &market, as_of, ModelKey::Tree, 0.0075)
                .unwrap()
                .to_bits(),
        );
    }

    assert_eq!(
        bits,
        [
            4_652_395_558_412_783_183_u64,
            4_652_417_570_083_781_352,
            4_652_420_412_340_861_640,
            4_652_412_935_098_629_595,
            4_652_317_577_597_138_786,
        ]
    );
}

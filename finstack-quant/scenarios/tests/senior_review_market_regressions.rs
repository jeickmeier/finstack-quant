//! Regression coverage for collection-qualified shocks and cached composites.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, PriceCurve, PriceCurveKind,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::PriceId;
use finstack_quant_scenarios::engine::ScenarioMarketTarget;
use finstack_quant_scenarios::{
    ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec, TenorMatchMode,
};
use finstack_quant_valuations::instruments::{
    CompositeInstrument, CompositeLegSpec, CompositeSpec, Instrument, InstrumentJson,
    RebalanceRule, WeightingMethod,
};
use finstack_quant_valuations::pricer::InstrumentType;
use time::macros::date;

fn scenario(operations: Vec<OperationSpec>) -> ScenarioSpec {
    ScenarioSpec {
        id: "review-regression".into(),
        name: None,
        description: None,
        operations,
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    }
}

#[test]
fn price_and_surface_shocks_preserve_their_collection_with_colliding_ids() {
    let as_of = date!(2025 - 01 - 01);
    let discount = DiscountCurve::builder("AAPL")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .build()
        .expect("discount curve");
    let surface = VolSurface::builder("AAPL")
        .expiries(&[0.5, 1.0])
        .strikes(&[100.0])
        .row(&[0.20])
        .row(&[0.20])
        .build()
        .expect("surface");
    let mut market = MarketContext::new()
        .insert(discount)
        .insert_surface(surface)
        .insert_price(
            "AAPL",
            MarketScalar::Price(Money::from((100_i64, Currency::USD))),
        );
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of,
    };
    let report = ScenarioEngine::new()
        .apply(
            &scenario(vec![OperationSpec::EquityPricePct {
                ids: vec!["AAPL".into()],
                pct: 10.0,
            }]),
            &mut ctx,
        )
        .expect("price shock");
    let MarketScalar::Price(price) = ctx.market.get_price("AAPL").expect("price") else {
        panic!("monetary price");
    };
    assert!((price.amount() - 110.0).abs() < 1e-12);
    assert_eq!(
        report.changes.market_targets,
        vec![ScenarioMarketTarget::EquityPrice {
            spot_id: PriceId::new("AAPL")
        }]
    );
    assert_eq!(
        ctx.market.get_discount("AAPL").expect("discount").df(1.0),
        0.95
    );
    let unshocked_vol = finstack_quant_models::volatility::get_surface_vol(
        &ctx.market.get_surface("AAPL").expect("surface"),
        1.0,
        100.0,
    )
    .expect("vol");
    assert_eq!(unshocked_vol, 0.20);

    let report = ScenarioEngine::new()
        .apply(
            &scenario(vec![
                OperationSpec::VolSurfaceParallelPct {
                    vol_surface_id: "AAPL".into(),
                    pct: 10.0,
                },
                OperationSpec::EquityPricePct {
                    ids: vec!["AAPL".into()],
                    pct: 10.0,
                },
            ]),
            &mut ctx,
        )
        .expect("surface and composed price shocks");
    let MarketScalar::Price(price) = ctx.market.get_price("AAPL").expect("price") else {
        panic!("monetary price");
    };
    assert!((price.amount() - 121.0).abs() < 1e-12);
    assert_eq!(
        ctx.market.get_discount("AAPL").expect("discount").df(1.0),
        0.95
    );
    let shocked_vol = finstack_quant_models::volatility::get_surface_vol(
        &ctx.market.get_surface("AAPL").expect("surface"),
        1.0,
        100.0,
    )
    .expect("vol");
    assert!((shocked_vol - 0.22).abs() < 1e-12);
    assert_eq!(
        report.changes.market_targets,
        vec![
            ScenarioMarketTarget::VolSurface {
                vol_surface_id: "AAPL".into()
            },
            ScenarioMarketTarget::EquityPrice {
                spot_id: PriceId::new("AAPL")
            },
        ]
    );
}

#[test]
fn absent_base_correlation_targets_do_not_bump_an_unrelated_price() {
    let mut market = MarketContext::new().insert_price("COLLISION", MarketScalar::Unitless(0.2));
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: date!(2025 - 01 - 01),
    };
    ScenarioEngine::new()
        .apply(
            &scenario(vec![OperationSpec::BaseCorrParallelPts {
                surface_id: "COLLISION".into(),
                points: 0.1,
            }]),
            &mut ctx,
        )
        .expect_err("base-correlation target must exist");
    assert!(
        matches!(market.get_price("COLLISION").expect("price remains"), MarketScalar::Unitless(v) if *v == 0.2)
    );
}

#[test]
fn descendant_shocks_replace_warmed_composite_legs_and_preserve_state_and_overrides() {
    let as_of = date!(2025 - 01 - 01);
    let mut composite = CompositeInstrument::example().expect("composite");
    composite
        .scenario_pricing_overrides
        .scenario_price_shock_decimal = Some(0.05);
    let original_state = serde_json::to_value(&composite.state).expect("state");
    let mut market = MarketContext::new();
    // Pricing materializes the boxed legs before the scenario is applied.
    assert!(
        (composite
            .value(&market, as_of)
            .expect("warm value")
            .amount()
            - 10.5)
            .abs()
            < 1e-12
    );
    let mut instruments: Vec<Box<dyn Instrument>> = vec![Box::new(composite)];
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of,
    };
    let report = ScenarioEngine::new()
        .apply(
            &scenario(vec![OperationSpec::InstrumentPricePctByType {
                instrument_types: vec![InstrumentType::Equity],
                pct: 10.0,
            }]),
            &mut ctx,
        )
        .expect("descendant shock");
    assert_eq!(report.changes.changed_instrument_indices, vec![0]);
    let shocked = instruments[0]
        .as_any()
        .downcast_ref::<CompositeInstrument>()
        .expect("composite remains");
    assert_eq!(
        serde_json::to_value(&shocked.state).expect("state"),
        original_state
    );
    assert_eq!(
        shocked
            .scenario_pricing_overrides
            .scenario_price_shock_decimal,
        Some(0.05)
    );
    assert!(
        (shocked
            .value(&market, as_of)
            .expect("updated value")
            .amount()
            - 11.55)
            .abs()
            < 1e-12
    );
    assert_eq!(
        shocked.value(&market, as_of).expect("same cached value"),
        shocked.clone().value(&market, as_of).expect("fresh value")
    );
}

#[test]
fn descendant_shocks_refresh_warmed_nested_composite_values() {
    let as_of = date!(2025 - 01 - 01);
    let child = CompositeInstrument::example().expect("child composite");
    let parent = CompositeSpec::new(
        "PARENT",
        Currency::USD,
        Money::from((100_i64, Currency::USD)),
        vec![
            CompositeLegSpec::new(
                "COMPOSITE-EXAMPLE",
                InstrumentJson::Composite(Box::new(child)),
                2.0,
            ),
            CompositeLegSpec::new(
                "PARENT-SPOT",
                InstrumentJson::Equity(
                    finstack_quant_valuations::instruments::Equity::new(
                        "PARENT-SPOT",
                        "PARENT-SPOT",
                        Currency::USD,
                    )
                    .with_quantity(1.0)
                    .with_quoted_spot(50.0),
                ),
                1.0,
            ),
        ],
        WeightingMethod::FixedQuantity,
        RebalanceRule::Manual,
    )
    .initialize_fixed(as_of)
    .expect("parent state")
    .instrument;
    let mut market = MarketContext::new();
    assert!(
        (parent
            .value(&market, as_of)
            .expect("warm nested value")
            .amount()
            - 70.0)
            .abs()
            < 1e-12
    );
    let mut instruments: Vec<Box<dyn Instrument>> = vec![Box::new(parent)];
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of,
    };
    ScenarioEngine::new()
        .apply(
            &scenario(vec![OperationSpec::InstrumentPricePctByType {
                instrument_types: vec![InstrumentType::Equity],
                pct: 10.0,
            }]),
            &mut ctx,
        )
        .expect("nested descendant shock");
    assert!(
        (instruments[0]
            .value(&market, as_of)
            .expect("updated nested value")
            .amount()
            - 77.0)
            .abs()
            < 1e-12
    );
}

#[test]
fn vol_index_interpolated_shocks_deliver_all_overlapping_tenors_on_native_interpolants() {
    let as_of = date!(2025 - 01 - 01);
    for interp in [
        InterpStyle::Linear,
        InterpStyle::LogLinear,
        InterpStyle::MonotoneConvex,
    ] {
        let base = PriceCurve::builder("VIX")
            .kind(PriceCurveKind::VolIndex)
            .base_date(as_of)
            .day_count(DayCount::Thirty360)
            .spot_price(20.0)
            .interp(interp)
            .knots([(0.0, 20.0), (1.0, 20.0), (3.0, 20.0), (5.0, 20.0)])
            .build()
            .expect("vol index");
        let mut market = MarketContext::new().insert(base);
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        ScenarioEngine::new()
            .apply(
                &scenario(vec![OperationSpec::VolIndexNodePts {
                    curve_id: "VIX".into(),
                    nodes: vec![("2Y".into(), 1.0), ("4Y".into(), 2.0)],
                    match_mode: TenorMatchMode::Interpolate,
                }]),
                &mut ctx,
            )
            .expect("simultaneous vol index shocks");
        let shocked = market
            .get_vol_index_curve("VIX")
            .expect("vol index remains");
        assert_eq!(shocked.interp_style(), interp);
        assert!(
            (shocked.price(2.0) - 21.0).abs() < 1e-8,
            "{interp:?}: 2Y {}",
            shocked.price(2.0)
        );
        assert!(
            (shocked.price(4.0) - 22.0).abs() < 1e-8,
            "{interp:?}: 4Y {}",
            shocked.price(4.0)
        );
    }
}

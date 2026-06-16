//! Forward curve dependency completeness tests.
//!
//! Ensures instruments declaring forward curve dependencies can be priced
//! with only the declared market data present. For commodity options,
//! the forward price can come from either:
//! 1. A PriceCurve (preferred)
//! 2. Cost-of-carry model using spot price and discount curve (fallback)

use crate::finstack_quant_test_utils::{
    flat_discount_with_tenor, flat_price_curve, flat_vol_surface,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::commodity::commodity_option::CommodityOption;
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::MarketDependencies;
use time::macros::date;

fn build_market_from_deps(
    deps: &MarketDependencies,
    as_of: Date,
    spot_currency: Currency,
) -> MarketContext {
    let mut market = MarketContext::new();

    for curve_id in deps.curves.discount_curves.iter() {
        market = market.insert(flat_discount_with_tenor(
            curve_id.as_str(),
            as_of,
            0.02,
            5.0,
        ));
    }
    // Use PriceCurve for commodity forward prices (not ForwardCurve which is for rates)
    for curve_id in deps.curves.forward_curves.iter() {
        market = market.insert(flat_price_curve(curve_id.as_str(), as_of, 80.0, 5.0));
    }
    for surface_id in deps.vol_surface_ids.iter() {
        market = market.insert_surface(flat_vol_surface(
            surface_id.as_str(),
            &[0.25, 0.5, 1.0],
            &[50.0, 75.0, 100.0],
            0.2,
        ));
    }
    for spot_id in deps.spot_ids.iter() {
        market = market.insert_price(
            spot_id,
            MarketScalar::Price(Money::new(100.0, spot_currency)),
        );
    }

    market
}

#[test]
fn test_forward_curve_dependencies_complete() {
    let as_of = date!(2025 - 01 - 01);
    let mut option = CommodityOption::example();
    option.spot_id = Some("WTI-SPOT".to_string());

    let deps =
        MarketDependencies::from_instrument_json(&InstrumentJson::CommodityOption(option.clone()))
            .expect("from_instrument_json");
    let market = build_market_from_deps(&deps, as_of, option.underlying.currency);

    let result = option.value(&market, as_of);
    assert!(
        result.is_ok(),
        "Commodity option pricing with minimal market should succeed, got: {:?}",
        result.err()
    );
}

#[test]
fn test_missing_forward_curve_with_spot_succeeds() {
    // When spot_id is present, commodity option can derive forward via cost-of-carry
    let as_of = date!(2025 - 01 - 01);
    let mut option = CommodityOption::example();
    option.spot_id = Some("WTI-SPOT".to_string());

    let deps =
        MarketDependencies::from_instrument_json(&InstrumentJson::CommodityOption(option.clone()))
            .expect("from_instrument_json");

    let mut market = MarketContext::new();
    for curve_id in deps.curves.discount_curves.iter() {
        market = market.insert(flat_discount_with_tenor(
            curve_id.as_str(),
            as_of,
            0.02,
            5.0,
        ));
    }
    for surface_id in deps.vol_surface_ids.iter() {
        market = market.insert_surface(flat_vol_surface(
            surface_id.as_str(),
            &[0.25, 0.5, 1.0],
            &[50.0, 75.0, 100.0],
            0.2,
        ));
    }
    for spot_id in deps.spot_ids.iter() {
        market = market.insert_price(
            spot_id,
            MarketScalar::Price(Money::new(100.0, option.underlying.currency)),
        );
    }

    let result = option.value(&market, as_of);
    // Should SUCCEED - cost-of-carry fallback: F = S * exp(r * T)
    assert!(
        result.is_ok(),
        "Commodity option with spot price should succeed via cost-of-carry fallback, got: {:?}",
        result.err()
    );
}

#[test]
fn test_missing_both_forward_and_spot_fails() {
    // When neither PriceCurve nor spot_id is available, pricing should fail
    let as_of = date!(2025 - 01 - 01);
    let mut option = CommodityOption::example();
    option.spot_id = None; // No spot fallback

    let deps =
        MarketDependencies::from_instrument_json(&InstrumentJson::CommodityOption(option.clone()))
            .expect("from_instrument_json");

    let mut market = MarketContext::new();
    for curve_id in deps.curves.discount_curves.iter() {
        market = market.insert(flat_discount_with_tenor(
            curve_id.as_str(),
            as_of,
            0.02,
            5.0,
        ));
    }
    for surface_id in deps.vol_surface_ids.iter() {
        market = market.insert_surface(flat_vol_surface(
            surface_id.as_str(),
            &[0.25, 0.5, 1.0],
            &[50.0, 75.0, 100.0],
            0.2,
        ));
    }
    // No spot price and no PriceCurve

    let result = option.value(&market, as_of);
    assert!(
        result.is_err(),
        "Commodity option pricing should fail when both PriceCurve and spot are missing"
    );
}

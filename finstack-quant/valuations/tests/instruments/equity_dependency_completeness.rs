//! Equity dependency completeness tests.
//!
//! Ensures instruments that declare equity market data dependencies
//! (spot + vol surface) can be priced with only those dependencies provided.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DateExt, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::commodity::commodity_option::CommodityOption;
use finstack_quant_valuations::instruments::CommodityUnderlyingParams;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{
    ExerciseStyle, OptionType, PricingOverrides, SettlementType,
};
use time::macros::date;

fn build_discount_curve(id: &str, rate: f64) -> DiscountCurve {
    let as_of = date!(2025 - 01 - 01);
    DiscountCurve::builder(id)
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, (-rate).exp()),
            (5.0f64, (-rate * 5.0).exp()),
            (10.0f64, (-rate * 10.0).exp()),
        ])
        .build()
        .expect("Discount curve construction should succeed")
}

fn build_forward_curve(id: &str, tenor_years: f64, rate: f64) -> ForwardCurve {
    let as_of = date!(2025 - 01 - 01);
    ForwardCurve::builder(id, tenor_years)
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .reset_lag(2)
        .knots([(0.0, rate), (1.0, rate + 0.003), (5.0, rate + 0.006)])
        .build()
        .expect("Forward curve construction should succeed")
}

fn build_vol_surface(id: &str) -> VolSurface {
    VolSurface::builder(id)
        .expiries(&[0.25, 1.0])
        .strikes(&[90.0, 100.0, 110.0])
        .row(&[0.25, 0.24, 0.23])
        .row(&[0.22, 0.21, 0.20])
        .build()
        .expect("Vol surface construction should succeed")
}

#[test]
fn test_commodity_option_equity_dependencies_complete() {
    let as_of = date!(2025 - 01 - 01);
    let expiry = as_of.add_months(6);

    let option = CommodityOption::builder()
        .id(InstrumentId::new("WTI-OPT-DEPS"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .strike(100.0)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .quantity(1_000.0)
        .multiplier(1.0)
        .settlement(SettlementType::Cash)
        .forward_curve_id(CurveId::new("WTI-FWD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("WTI-VOL"))
        .spot_id("WTI-SPOT".into())
        .day_count(DayCount::Act365F)
        .pricing_overrides(PricingOverrides::default())
        .build()
        .expect("Commodity option construction should succeed");

    let market_deps = option.market_dependencies().expect("market_dependencies");
    let curve_deps = &market_deps.curves;

    let mut market = MarketContext::new();
    for id in curve_deps.discount_curves.iter() {
        market = market.insert(build_discount_curve(id.as_str(), 0.03));
    }
    for id in curve_deps.forward_curves.iter() {
        market = market.insert(build_forward_curve(id.as_str(), 0.25, 0.04));
    }
    for volatility in &market_deps.volatility_dependencies {
        market = market.insert_surface(build_vol_surface(volatility.surface_id.as_str()));
    }
    for spot_id in &market_deps.spot_ids {
        market = market.insert_price(
            spot_id,
            MarketScalar::Price(Money::new(100.0, Currency::USD)),
        );
    }

    let result = option.value(&market, as_of);
    assert!(
        result.is_ok(),
        "Commodity option pricing with minimal market should succeed, got: {:?}",
        result.err()
    );
}

#[test]
fn test_missing_equity_spot_fails() {
    let as_of = date!(2025 - 01 - 01);
    let expiry = as_of.add_months(6);

    let option = CommodityOption::builder()
        .id(InstrumentId::new("WTI-OPT-SPOT-MISSING"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .strike(100.0)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .quantity(1_000.0)
        .multiplier(1.0)
        .settlement(SettlementType::Cash)
        .forward_curve_id(CurveId::new("WTI-FWD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("WTI-VOL"))
        .spot_id("WTI-SPOT".into())
        .day_count(DayCount::Act365F)
        .pricing_overrides(PricingOverrides::default())
        .build()
        .expect("Commodity option construction should succeed");

    let market_deps = option.market_dependencies().expect("market_dependencies");
    let curve_deps = &market_deps.curves;

    let mut market = MarketContext::new();
    for id in curve_deps.discount_curves.iter() {
        market = market.insert(build_discount_curve(id.as_str(), 0.03));
    }
    for id in curve_deps.forward_curves.iter() {
        market = market.insert(build_forward_curve(id.as_str(), 0.25, 0.04));
    }
    for volatility in &market_deps.volatility_dependencies {
        market = market.insert_surface(build_vol_surface(volatility.surface_id.as_str()));
    }

    let result = option.value(&market, as_of);
    assert!(
        result.is_err(),
        "Commodity option pricing should fail when the spot price is missing"
    );
}

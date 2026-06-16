//! FX dependency completeness tests.
//!
//! Ensures FX instruments declare FX pair dependencies and that pricing
//! succeeds when the FX matrix is provided.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DateExt, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{FxDigitalOption, FxForward, InstrumentJson};
use finstack_quant_valuations::instruments::{FxPair, MarketDependencies};
use std::sync::Arc;
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

fn build_fx_matrix(base: Currency, quote: Currency, rate: f64) -> FxMatrix {
    let provider = Arc::new(SimpleFxProvider::new());
    provider.set_quote(base, quote, rate).expect("valid rate");
    FxMatrix::new(provider)
}

#[test]
fn test_fx_forward_dependencies_complete() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = as_of.add_months(6);

    let forward = FxForward::builder()
        .id(InstrumentId::new("EURUSD-FWD-DEPS"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(maturity)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .build()
        .expect("FX forward construction should succeed");

    let deps =
        MarketDependencies::from_instrument_json(&InstrumentJson::FxForward(forward.clone()))
            .expect("from_instrument_json");
    assert!(
        deps.fx_pairs
            .contains(&FxPair::new(Currency::EUR, Currency::USD)),
        "FX forward should declare its FX pair dependency"
    );

    let mut market = MarketContext::new();
    for id in deps.curves.discount_curves {
        market = market.insert(build_discount_curve(id.as_str(), 0.03));
    }
    market = market.insert_fx(build_fx_matrix(Currency::EUR, Currency::USD, 1.10));

    let result = forward.value(&market, as_of);
    assert!(
        result.is_ok(),
        "FX forward pricing with minimal market should succeed, got: {:?}",
        result.err()
    );
}

#[test]
fn test_missing_fx_matrix_fails() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = as_of.add_months(6);

    let forward = FxForward::builder()
        .id(InstrumentId::new("EURUSD-FWD-MISSING-FX"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(maturity)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .build()
        .expect("FX forward construction should succeed");

    let deps =
        MarketDependencies::from_instrument_json(&InstrumentJson::FxForward(forward.clone()))
            .expect("from_instrument_json");
    let mut market = MarketContext::new();
    for id in deps.curves.discount_curves {
        market = market.insert(build_discount_curve(id.as_str(), 0.03));
    }

    let result = forward.value(&market, as_of);
    assert!(
        result.is_err(),
        "FX forward pricing should fail when the FX matrix is missing"
    );
}

#[test]
fn test_fx_digital_direct_dependencies_match_json_dependencies() {
    let option = FxDigitalOption::example().expect("FX digital example");

    let json_deps =
        MarketDependencies::from_instrument_json(&InstrumentJson::FxDigitalOption(option.clone()))
            .expect("from_instrument_json");
    let direct_deps = option
        .market_dependencies()
        .expect("direct market_dependencies");

    assert_eq!(
        direct_deps.curves.discount_curves, json_deps.curves.discount_curves,
        "direct dependencies should declare the same discount curves as JSON dependencies"
    );
    assert_eq!(
        direct_deps.vol_surface_ids, json_deps.vol_surface_ids,
        "direct dependencies should declare the FX vol surface"
    );
    assert_eq!(
        direct_deps.fx_pairs, json_deps.fx_pairs,
        "direct dependencies should declare the FX pair"
    );
}

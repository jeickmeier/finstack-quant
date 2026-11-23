use super::option::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::{
    VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use std::sync::Arc;
use time::macros::date;

fn market(as_of: Date) -> MarketContext {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, (-0.04_f64).exp())])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("valid discount");
    let foreign = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, (-0.02_f64).exp())])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("valid foreign");
    let vol = VolSurface::builder("EURUSD-VOL")
        .expiries(&[1.0])
        .secondary_values(&[1.1])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&[0.20])
        .build()
        .expect("valid vol");
    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 1.1)])
        .expect("valid rate");
    let fx = FxMatrix::new(provider, None);
    MarketContext::new()
        .try_insert(discount)
        .expect("insert market curve")
        .try_insert(foreign)
        .expect("insert market curve")
        .insert_surface(vol)
        .insert_fx(fx)
        .insert_price(
            "EURUSD-SPOT",
            MarketScalar::price(Money::new(1.1, Currency::USD).expect("valid money fixture")),
        )
}

#[test]
fn shared_fx_option_inputs_reconstruct_date_based_discount_rates() {
    let as_of = date!(2026 - 01 - 01);
    let expiry = date!(2027 - 01 - 01);
    let market = market(as_of);

    let inputs = collect_fx_option_inputs(FxOptionInputRequest {
        market: &market,
        as_of,
        base_currency: Currency::EUR,
        quote_currency: Currency::USD,
        expiry,
        delivery: expiry,
        spot_basis: crate::market_inputs::FxSpotBasis::Cash,
        day_count: DayCount::Act365F,
        domestic_discount_curve_id: &CurveId::new("USD-OIS"),
        foreign_discount_curve_id: &CurveId::new("EUR-OIS"),
        vol_surface_id: "EURUSD-VOL",
        strike: 1.1,
        instrument_pricing_overrides: &InstrumentPricingOverrides::default(),
        spot_id: None,
        rate_context: "test",
    })
    .expect("inputs should resolve");

    let domestic_df = market
        .get_discount("USD-OIS")
        .expect("discount")
        .df_between_dates(as_of, expiry)
        .expect("df");
    let foreign_df = market
        .get_discount("EUR-OIS")
        .expect("foreign")
        .df_between_dates(as_of, expiry)
        .expect("df");

    assert!(((-inputs.r_domestic * inputs.t).exp() - domestic_df).abs() < 1e-12);
    assert!(((-inputs.r_foreign * inputs.t).exp() - foreign_df).abs() < 1e-12);
    assert_eq!(inputs.spot, 1.1);
    assert_eq!(inputs.sigma, 0.20);
}

#[test]
fn shared_fx_option_inputs_can_source_spot_from_scalar_id() {
    let as_of = date!(2026 - 01 - 01);
    let expiry = date!(2027 - 01 - 01);
    let market = market(as_of);
    let spot_id = finstack_quant_core::types::PriceId::new("EURUSD-SPOT");

    let inputs = collect_fx_option_inputs(FxOptionInputRequest {
        market: &market,
        as_of,
        base_currency: Currency::EUR,
        quote_currency: Currency::USD,
        expiry,
        delivery: expiry,
        spot_basis: crate::market_inputs::FxSpotBasis::Cash,
        day_count: DayCount::Act365F,
        domestic_discount_curve_id: &CurveId::new("USD-OIS"),
        foreign_discount_curve_id: &CurveId::new("EUR-OIS"),
        vol_surface_id: "EURUSD-VOL",
        strike: 1.1,
        instrument_pricing_overrides: &InstrumentPricingOverrides::default(),
        spot_id: Some(&spot_id),
        rate_context: "test",
    })
    .expect("inputs should resolve");

    assert_eq!(inputs.spot, 1.1);
}

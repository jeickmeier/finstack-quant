use crate::common::builders::{test_market, test_option};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::fx::{FxMatrix, FxQuery, SimpleFxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{Attributes, ExerciseStyle, FxOption, OptionType};
use finstack_quant_valuations::instruments::{InstrumentPricingOverrides, SettlementType};
use finstack_quant_valuations::instruments::{
    OptionGreekKind, OptionGreeksProvider, OptionGreeksRequest,
};
use finstack_quant_valuations::metrics::MetricId;
use std::sync::Arc;
use time::macros::date;

#[test]
fn equity_option_provider_matches_registered_metrics_and_omits_foreign_rho() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let option = test_option(expiry).div_yield_id("SPOT_DIV").build();
    let market = test_market(as_of).div_yield(0.02).build();

    let metrics = [
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Theta,
        MetricId::Rho,
        MetricId::Vanna,
        MetricId::Volga,
    ];
    let priced = option
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("equity metrics should compute");

    let delta = option
        .option_greeks(
            &market,
            as_of,
            &OptionGreeksRequest {
                greek: OptionGreekKind::Delta,
                base_pv: None,
            },
        )
        .expect("delta request should succeed");
    assert_eq!(delta.delta, priced.measures.get(&MetricId::Delta).copied());
    assert_eq!(delta.foreign_rho_bp, None);

    let foreign_rho = option
        .option_greeks(
            &market,
            as_of,
            &OptionGreeksRequest {
                greek: OptionGreekKind::ForeignRho,
                base_pv: None,
            },
        )
        .expect("unsupported greek requests should still succeed");
    assert_eq!(foreign_rho.foreign_rho_bp, None);

    let volga = option
        .option_greeks(
            &market,
            as_of,
            &OptionGreeksRequest {
                greek: OptionGreekKind::Volga,
                base_pv: Some(priced.value.amount()),
            },
        )
        .expect("volga request should succeed");
    assert_eq!(volga.volga, priced.measures.get(&MetricId::Volga).copied());
}

#[test]
fn fx_option_provider_matches_registered_foreign_rho_and_volga() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let option = build_fx_call_option(expiry, 1.20, 1_000_000.0);
    let market = build_fx_market(as_of, 1.20, 0.15, 0.03, 0.01);

    let metrics = [MetricId::Rho, MetricId::ForeignRho, MetricId::Volga];
    let priced = option
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("fx metrics should compute");

    let foreign_rho = option
        .option_greeks(
            &market,
            as_of,
            &OptionGreeksRequest {
                greek: OptionGreekKind::ForeignRho,
                base_pv: None,
            },
        )
        .expect("foreign rho request should succeed");
    assert_eq!(
        foreign_rho.foreign_rho_bp,
        priced.measures.get(&MetricId::ForeignRho).copied()
    );

    let volga = option
        .option_greeks(
            &market,
            as_of,
            &OptionGreeksRequest {
                greek: OptionGreekKind::Volga,
                base_pv: Some(priced.value.amount()),
            },
        )
        .expect("volga request should succeed");
    assert_eq!(volga.volga, priced.measures.get(&MetricId::Volga).copied());
}

fn build_fx_market(
    as_of: Date,
    spot: f64,
    vol: f64,
    r_domestic: f64,
    r_foreign: f64,
) -> MarketContext {
    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.25, (-r_domestic * 0.25).exp()),
            (1.0, (-r_domestic).exp()),
            (5.0, (-r_domestic * 5.0).exp()),
        ])
        .build()
        .expect("usd curve should build");
    let eur_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.25, (-r_foreign * 0.25).exp()),
            (1.0, (-r_foreign).exp()),
            (5.0, (-r_foreign * 5.0).exp()),
        ])
        .build()
        .expect("eur curve should build");
    let vol_surface = VolSurface::builder("EURUSD-VOL")
        .expiries(&[0.25, 0.5, 1.0, 2.0, 5.0])
        .strikes(&[0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4])
        .row(&[vol; 7])
        .row(&[vol; 7])
        .row(&[vol; 7])
        .row(&[vol; 7])
        .row(&[vol; 7])
        .build()
        .expect("vol surface should build");

    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quote(Currency::EUR, Currency::USD, spot)
        .expect("fx quote should build");
    let fx_matrix = FxMatrix::new(provider);
    let _ = fx_matrix
        .rate(FxQuery::new(Currency::EUR, Currency::USD, as_of))
        .expect("fx rate should exist");

    MarketContext::new()
        .insert(usd_curve)
        .insert(eur_curve)
        .insert_surface(vol_surface)
        .insert_fx(fx_matrix)
}

fn build_fx_call_option(expiry: Date, strike: f64, notional: f64) -> FxOption {
    FxOption::builder()
        .id(InstrumentId::new("FX-CALL-CONSOLIDATION"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .strike(strike)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .day_count(DayCount::Act365F)
        .notional(Money::new(notional, Currency::EUR))
        .settlement(SettlementType::Cash)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("fx option should build")
}

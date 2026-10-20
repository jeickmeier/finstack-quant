//! FX barrier option Greeks bump the same inputs pricing reads.
//!
//! Pricing takes volatility from the `implied_volatility` override when set
//! (else the surface) and spot from `fx_spot_id` when set (else the FX
//! matrix). The finite-difference Greeks must bump those same sources.

use std::sync::Arc;

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::BarrierType;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations_core::instruments::Monitoring;
use finstack_quant_valuations_core::instruments::{
    Attributes, Instrument, OptionType, PricingOptions,
};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_fx::fx_barrier_option::FxBarrierOption;
use finstack_quant_valuations_test_support::date::date;
use finstack_quant_valuations_test_support::discount_forward_curves::flat_discount_with_tenor;
use finstack_quant_valuations_test_support::volatility::flat_vol_surface;

const SPOT: f64 = 1.10;
const BUMP: f64 = 0.01;

fn as_of() -> finstack_quant_core::dates::Date {
    date(2024, 1, 1)
}

fn fx_matrix(spot: f64) -> FxMatrix {
    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quote(Currency::EUR, Currency::USD, spot)
        .expect("fx quote");
    FxMatrix::new(provider)
}

/// Curves plus an FX matrix; the vol surface is optional.
fn matrix_market(spot: f64, surface_vol: Option<f64>) -> MarketContext {
    let market = MarketContext::new()
        .insert(flat_discount_with_tenor("USD-OIS", as_of(), 0.03, 5.0))
        .insert(flat_discount_with_tenor("EUR-OIS", as_of(), 0.01, 5.0))
        .insert_fx(fx_matrix(spot));
    match surface_vol {
        Some(vol) => market.insert_surface(flat_vol_surface(
            "EURUSD-VOL",
            &[0.25, 0.5, 1.0, 2.0, 5.0],
            &[0.9, 1.0, 1.1, 1.2, 1.3],
            vol,
        )),
        None => market,
    }
}

/// Up-and-out call far from the barrier, priced off the FX matrix.
fn option(implied_volatility: Option<f64>) -> FxBarrierOption {
    let mut option = FxBarrierOption::builder()
        .id(InstrumentId::new("FXBAR-GREEK-SOURCES"))
        .strike(1.10)
        .barrier(1.60)
        .option_type(OptionType::Call)
        .barrier_type(BarrierType::UpAndOut)
        .expiry(date(2025, 1, 1))
        .monitoring_start_date_opt(Some(as_of()))
        .notional(Money::new(1_000_000.0, Currency::EUR).expect("notional"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .monitoring(Monitoring::Continuous)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .attributes(Attributes::new())
        .build()
        .expect("FX barrier option should build");
    option
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility = implied_volatility;
    option
}

fn metric(option: &FxBarrierOption, market: &MarketContext, id: MetricId) -> f64 {
    let result = option
        .price_with_metrics(
            market,
            as_of(),
            std::slice::from_ref(&id),
            PricingOptions::default(),
        )
        .expect("metric pricing");
    result.metric(id).expect("metric present")
}

fn pv(option: &FxBarrierOption, market: &MarketContext) -> f64 {
    option.value(market, as_of()).expect("pv").amount()
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{what}: expected {expected}, got {actual}"
    );
}

#[test]
fn vol_greeks_bump_the_implied_volatility_override() {
    // Surface at 0.25 is present but unused: the 0.15 override drives pricing.
    let market = matrix_market(SPOT, Some(0.25));
    let base = option(Some(0.15));
    let up = option(Some(0.15 + BUMP));
    let down = option(Some(0.15 - BUMP));

    let width = BUMP * 100.0;
    let vega_ref = (pv(&up, &market) - pv(&down, &market)) / (2.0 * width);
    let volga_ref =
        (pv(&up, &market) - 2.0 * pv(&base, &market) + pv(&down, &market)) / (width * width);
    let delta_at = |inst: &FxBarrierOption| {
        (pv(inst, &matrix_market(SPOT * (1.0 + BUMP), Some(0.25)))
            - pv(inst, &matrix_market(SPOT * (1.0 - BUMP), Some(0.25))))
            / (2.0 * SPOT * BUMP)
    };
    let vanna_ref = (delta_at(&up) - delta_at(&down)) / (2.0 * width);

    let vega = metric(&base, &market, MetricId::Vega);
    assert!(vega.abs() > 1.0, "vega must not be zero, got {vega}");
    assert_close(vega, vega_ref, "override vega");
    assert_close(
        metric(&base, &market, MetricId::Volga),
        volga_ref,
        "override volga",
    );
    assert_close(
        metric(&base, &market, MetricId::Vanna),
        vanna_ref,
        "override vanna",
    );

    // Without a surface in the market the override alone prices and risks.
    let no_surface = matrix_market(SPOT, None);
    assert_close(
        metric(&base, &no_surface, MetricId::Vega),
        vega_ref,
        "override vega without surface",
    );
}

#[test]
fn spot_greeks_bump_the_fx_matrix_without_spot_id() {
    let market = matrix_market(SPOT, Some(0.15));
    let inst = option(None);
    assert!(inst.fx_spot_id.is_none());

    let pv_up = pv(&inst, &matrix_market(SPOT * (1.0 + BUMP), Some(0.15)));
    let pv_dn = pv(&inst, &matrix_market(SPOT * (1.0 - BUMP), Some(0.15)));
    let bump_size = SPOT * BUMP;
    let delta_ref = (pv_up - pv_dn) / (2.0 * bump_size);
    let gamma_ref = (pv_up - 2.0 * pv(&inst, &market) + pv_dn) / (bump_size * bump_size);

    assert_close(
        metric(&inst, &market, MetricId::Delta),
        delta_ref,
        "matrix delta",
    );
    assert_close(
        metric(&inst, &market, MetricId::Gamma),
        gamma_ref,
        "matrix gamma",
    );
    assert!(metric(&inst, &market, MetricId::Vanna).is_finite());
}

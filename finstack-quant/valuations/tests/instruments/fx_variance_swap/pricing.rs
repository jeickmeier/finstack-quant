//! Pricing tests for FX variance swaps.

use crate::finstack_quant_test_utils::{date, flat_discount_with_tenor, flat_vol_surface};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::equity::variance_swap::RealizedVarMethod;
use finstack_quant_valuations::instruments::fx::fx_variance_swap::{FxVarianceSwap, PayReceive};
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use std::sync::Arc;

#[test]
fn test_forward_variance_flat_surface() {
    let as_of = date(2025, 1, 2);
    let maturity = date(2026, 1, 2);

    let dom_curve = flat_discount_with_tenor("USD-OIS", as_of, 0.02, 2.0);
    let for_curve = flat_discount_with_tenor("EUR-OIS", as_of, 0.01, 2.0);

    let expiries = [1.0];
    let strikes = [0.8, 1.0, 1.2, 1.4, 1.6];
    let vol = 0.20;
    let vol_surface = flat_vol_surface("EURUSD-VOL", &expiries, &strikes, vol);

    let fx_provider = Arc::new(SimpleFxProvider::new());
    fx_provider
        .set_quote(Currency::EUR, Currency::USD, 1.25)
        .expect("valid rate");
    let fx_matrix = FxMatrix::new(fx_provider);

    let ctx = MarketContext::new()
        .insert(dom_curve)
        .insert(for_curve)
        .insert_surface(vol_surface)
        .insert_fx(fx_matrix);

    let swap = FxVarianceSwap::builder()
        .id(InstrumentId::new("FXVAR-EURUSD"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .spot_id("EURUSD".to_string())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .strike_variance(0.04)
        .start_date(as_of)
        .maturity(maturity)
        .observation_freq(Tenor::daily())
        .base_calendar_id("TARGET2".to_string())
        .quote_calendar_id("USNY".to_string())
        .realized_var_method(RealizedVarMethod::CloseToClose)
        .side(PayReceive::Receive)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .day_count(DayCount::Act365F)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let fwd_var = swap.remaining_forward_variance(&ctx, as_of).unwrap();
    assert!(
        (fwd_var - vol * vol).abs() < 5e-3,
        "forward variance {}",
        fwd_var
    );

    let fair_swap = FxVarianceSwap::builder()
        .id(InstrumentId::new("FXVAR-EURUSD-FAIR"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .spot_id("EURUSD".to_string())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .strike_variance(fwd_var)
        .start_date(as_of)
        .maturity(maturity)
        .observation_freq(Tenor::daily())
        .base_calendar_id("TARGET2".to_string())
        .quote_calendar_id("USNY".to_string())
        .realized_var_method(RealizedVarMethod::CloseToClose)
        .side(PayReceive::Receive)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .day_count(DayCount::Act365F)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let pv = fair_swap.value(&ctx, as_of).unwrap();
    assert!(pv.amount().abs() < 1e-6 * fair_swap.notional.amount());
}

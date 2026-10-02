//! Pricing tests for FX variance swaps.

use crate::instruments::test_support::date::date;
use crate::instruments::test_support::discount_forward_curves::flat_discount_with_tenor;
use crate::instruments::test_support::volatility::flat_vol_surface;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, DayCountContext, Tenor};
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
    // A dense strip isolates the variance model from a five-strike
    // quadrature error, which previously consumed most of the tolerance.
    let strikes: Vec<_> = (50..=600).map(|i| f64::from(i) * 0.005).collect();
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
        .spot_id("EURUSD".into())
        .notional(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .strike_variance(0.04)
        .start_date(as_of)
        .maturity(maturity)
        .observation_frequency(Tenor::daily())
        .base_calendar_id("TARGET2".into())
        .quote_calendar_id("USNY".into())
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
    let dates = swap.observation_dates().expect("observation schedule");
    // Under deterministic rates and volatility each log return has variance
    // sigma² dt and mean (r_dom-r_for-sigma²/2) dt. Normalize their squared
    // moments by the contractual count, not by one calendar year.
    let expected_sum: f64 = dates
        .windows(2)
        .map(|window| {
            let dt = DayCount::Act365F
                .year_fraction(window[0], window[1], DayCountContext::default())
                .expect("return interval");
            vol * vol * dt + ((0.02 - 0.01 - 0.5 * vol * vol) * dt).powi(2)
        })
        .sum();
    let expected = swap.annualization_factor() * expected_sum / (dates.len() - 1) as f64;
    assert!(
        (fwd_var - expected).abs() < 1e-5,
        "forward variance {fwd_var} vs discrete lognormal expectation {expected}"
    );

    let fair_swap = FxVarianceSwap::builder()
        .id(InstrumentId::new("FXVAR-EURUSD-FAIR"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .spot_id("EURUSD".into())
        .notional(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .strike_variance(fwd_var)
        .start_date(as_of)
        .maturity(maturity)
        .observation_frequency(Tenor::daily())
        .base_calendar_id("TARGET2".into())
        .quote_calendar_id("USNY".into())
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

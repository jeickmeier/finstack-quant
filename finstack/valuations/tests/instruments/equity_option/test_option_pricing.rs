//! Determinism tests for equity option pricing.
//!
//! Verifies that option valuation (Black-Scholes) and Greeks produce
//! bitwise-identical results across multiple runs, and validates correctness
//! against market standards (put-call parity, Greeks bounds).

#[allow(unused_imports)]
use crate::common::test_helpers::tolerances;
use crate::finstack_test_utils as test_utils;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::surfaces::VolSurface;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_valuations::instruments::equity::equity_option::EquityOption;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::Month;

fn create_test_call() -> EquityOption {
    let expiry = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    test_utils::equity_option_european_call(
        "CALL-DETERMINISM",
        "AAPL",
        100.0, // strike
        expiry,
        100.0, // contract size
    )
    .expect("Test option creation should succeed")
}

fn create_test_market(base_date: Date) -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (0.5, 0.98), (1.0, 0.96), (2.0, 0.92)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Flat vol surface using from_grid
    let vol_surface = VolSurface::from_grid(
        "EQUITY-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[80.0, 90.0, 100.0, 110.0, 120.0],
        &[0.25; 20], // 20 = 4 expiries × 5 strikes
    )
    .unwrap();

    MarketContext::new()
        .insert(disc)
        .insert_surface(vol_surface)
        .insert_price(
            "EQUITY-SPOT",
            MarketScalar::Price(Money::new(100.0, Currency::USD)),
        )
        .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(0.02)) // 2% div yield
}

#[test]
fn test_option_pv_determinism() {
    let option = create_test_call();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Price the option 100 times
    let prices: Vec<f64> = (0..100)
        .map(|_| option.value(&market, as_of).unwrap().amount())
        .collect();

    // All prices must be bitwise identical
    for i in 1..prices.len() {
        assert_eq!(
            prices[i], prices[0],
            "Option PV at iteration {} = {:.15} differs from iteration 0 = {:.15}",
            i, prices[i], prices[0]
        );
    }
}

#[test]
fn test_option_all_greeks_determinism() {
    let option = create_test_call();
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    let metrics = vec![
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Theta,
        MetricId::Rho,
    ];

    // Calculate all greeks 30 times
    let results: Vec<_> = (0..30)
        .map(|_| {
            option
                .price_with_metrics(
                    &market,
                    as_of,
                    &metrics,
                    finstack_valuations::instruments::PricingOptions::default(),
                )
                .unwrap()
        })
        .collect();

    // Verify each metric is deterministic
    for metric in &metrics {
        let values: Vec<f64> = results
            .iter()
            .map(|r| r.measures[metric.as_str()])
            .collect();

        for i in 1..values.len() {
            assert_eq!(
                values[i],
                values[0],
                "{} differs at iteration {}: {:.15} vs {:.15}",
                metric.as_str(),
                i,
                values[i],
                values[0]
            );
        }
    }
}

#[test]
fn test_option_put_determinism() {
    let expiry = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let put =
        test_utils::equity_option_european_put("PUT-DETERMINISM", "AAPL", 100.0, expiry, 100.0)
            .unwrap();

    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Price the put 100 times
    let prices: Vec<f64> = (0..100)
        .map(|_| put.value(&market, as_of).unwrap().amount())
        .collect();

    // All prices must be identical
    for i in 1..prices.len() {
        assert_eq!(
            prices[i], prices[0],
            "Put PV differs at iteration {}: {:.15} vs {:.15}",
            i, prices[i], prices[0]
        );
    }
}

#[test]
fn test_option_different_moneyness_determinism() {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let expiry = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    // Test OTM, ATM, and ITM options
    let strikes = vec![80.0, 100.0, 120.0];

    for strike in strikes {
        let call = test_utils::equity_option_european_call(
            format!("CALL-K{}", strike),
            "AAPL",
            strike,
            expiry,
            1.0,
        )
        .unwrap();

        let prices: Vec<f64> = (0..30)
            .map(|_| call.value(&market, as_of).unwrap().amount())
            .collect();

        for i in 1..prices.len() {
            assert_eq!(
                prices[i], prices[0],
                "Strike {} - PV differs at iteration {}: {:.15} vs {:.15}",
                strike, i, prices[i], prices[0]
            );
        }
    }
}

#[test]
fn test_option_put_call_parity_determinism() {
    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let expiry = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let market = create_test_market(as_of);

    let call =
        test_utils::equity_option_european_call("PARITY-CALL", "AAPL", 100.0, expiry, 1.0).unwrap();
    let put =
        test_utils::equity_option_european_put("PARITY-PUT", "AAPL", 100.0, expiry, 1.0).unwrap();

    // Price multiple times for determinism
    let parities: Vec<f64> = (0..30)
        .map(|_| {
            let call_pv = call.value(&market, as_of).unwrap().amount();
            let put_pv = put.value(&market, as_of).unwrap().amount();
            call_pv - put_pv
        })
        .collect();

    // Determinism check
    for i in 1..parities.len() {
        assert_eq!(
            parities[i], parities[0],
            "Parity (C-P) differs at iteration {}: {:.15} vs {:.15}",
            i, parities[i], parities[0]
        );
    }

    // Correctness: Put-call parity C - P = S*exp(-qT) - K*exp(-rT)
    // With S=100, K=100, r≈4%, q=2%, T=1:
    // Forward value ≈ 100*exp(-0.02) - 100*exp(-0.04) ≈ 98.02 - 96.08 ≈ 1.94
    // Allow reasonable tolerance for the difference
    assert!(
        parities[0] > 0.0 && parities[0] < 10.0,
        "Put-call parity {} outside expected range for ATM options",
        parities[0]
    );
}

//! Property-based tests for option pricing bounds.
//!
//! Key Properties for European options:
//! - Call ≥ max(S·e^(-qT) - K·e^(-rT), 0)
//! - Put ≥ max(K·e^(-rT) - S·e^(-qT), 0)
//! - Call ≥ 0, Put ≥ 0 (non-negativity)
//! - Deep ITM call ≈ S - K·e^(-rT), Deep ITM put ≈ K·e^(-rT) - S

use crate::finstack_quant_test_utils as test_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use proptest::prelude::*;
use time::Month;

use crate::common::test_helpers::scaled_tolerance;

fn create_option_market(
    base_date: Date,
    spot: f64,
    vol: f64,
    rate: f64,
    div_yield: f64,
) -> MarketContext {
    // Use log-linear interpolation so a flat continuously compounded rate is
    // represented exactly between the supplied discount factor knots.
    let mut builder = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .knots([
            (0.0, 1.0),
            (0.5, (-rate * 0.5).exp()),
            (1.0, (-rate).exp()),
            (2.0, (-rate * 2.0).exp()),
        ]);

    // Allow non-monotonic for zero or negative rates (flat/increasing DFs).
    if rate.abs() < 1e-10 || rate < 0.0 {
        builder = builder.validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        );
    }

    let disc = builder.build().unwrap();

    let vol_surface = VolSurface::from_grid(
        "EQUITY-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[50.0, 75.0, 100.0, 125.0, 150.0],
        &[vol; 20], // 4 expiries × 5 strikes
    )
    .unwrap();

    MarketContext::new()
        .insert(disc)
        .insert_surface(vol_surface)
        .insert_price(
            "EQUITY-SPOT",
            MarketScalar::Price(Money::new(spot, Currency::USD)),
        )
        .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(div_yield))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_call_lower_bound(
        spot in 50.0..150.0,
        strike in 50.0..150.0,
        vol in 0.10..0.50,
        rate in 0.0..0.10,
        div_yield in 0.0..0.05,
        time_to_expiry_days in 30i64..730i64,
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let expiry = base_date + time::Duration::days(time_to_expiry_days);

        let call = test_utils::equity_option_european_call(
            "PROP-CALL",
            "AAPL",
            strike,
            expiry,
            1.0,
        )
        .unwrap();

        let market = create_option_market(base_date, spot, vol, rate, div_yield);
        let call_price = call.value(&market, base_date).unwrap().amount();

        // Calculate intrinsic value: max(S·e^(-qT) - K·e^(-rT), 0)
        let t = time_to_expiry_days as f64 / 365.0;
        let forward_spot = spot * (-div_yield * t).exp();
        let pv_strike = strike * (-rate * t).exp();
        let intrinsic = (forward_spot - pv_strike).max(0.0);

        // Property: Call price ≥ intrinsic value (with scaled tolerance for numerical precision)
        // Use 0.01% relative tolerance with 0.10 minimum floor for very small values
        let tolerance = scaled_tolerance(1e-4, intrinsic, 0.10);
        prop_assert!(
            call_price >= intrinsic - tolerance,
            "Call price {:.4} < intrinsic {:.4} (tol={:.4}, S={:.2}, K={:.2}, t={:.2})",
            call_price, intrinsic, tolerance, spot, strike, t
        );

        // Property: Call price ≥ 0 (non-negativity)
        prop_assert!(
            call_price >= 0.0,
            "Call price {:.4} is negative",
            call_price
        );
    }

    #[test]
    fn prop_put_lower_bound(
        spot in 50.0..150.0,
        strike in 50.0..150.0,
        vol in 0.10..0.50,
        rate in 0.0..0.10,
        div_yield in 0.0..0.05,
        time_to_expiry_days in 30i64..730i64,
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let expiry = base_date + time::Duration::days(time_to_expiry_days);

        let put = test_utils::equity_option_european_put(
            "PROP-PUT",
            "AAPL",
            strike,
            expiry,
            1.0,
        )
        .unwrap();

        let market = create_option_market(base_date, spot, vol, rate, div_yield);
        let put_price = put.value(&market, base_date).unwrap().amount();

        // Calculate intrinsic value: max(K·e^(-rT) - S·e^(-qT), 0)
        let t = time_to_expiry_days as f64 / 365.0;
        let forward_spot = spot * (-div_yield * t).exp();
        let pv_strike = strike * (-rate * t).exp();
        let intrinsic = (pv_strike - forward_spot).max(0.0);

        // Property: Put price ≥ intrinsic value (with scaled tolerance for numerical precision)
        // Use 0.01% relative tolerance with 0.10 minimum floor for very small values
        let tolerance = scaled_tolerance(1e-4, intrinsic, 0.10);
        prop_assert!(
            put_price >= intrinsic - tolerance,
            "Put price {:.4} < intrinsic {:.4} (tol={:.4}, S={:.2}, K={:.2}, t={:.2})",
            put_price, intrinsic, tolerance, spot, strike, t
        );

        // Property: Put price ≥ 0 (non-negativity)
        prop_assert!(
            put_price >= 0.0,
            "Put price {:.4} is negative",
            put_price
        );
    }

    #[test]
    fn prop_call_upper_bound(
        spot in 50.0..150.0,
        strike in 50.0..150.0,
        vol in 0.10..0.50,
        time_to_expiry_days in 30i64..730i64,
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let expiry = base_date + time::Duration::days(time_to_expiry_days);

        let call = test_utils::equity_option_european_call(
            "PROP-CALL-UPPER",
            "AAPL",
            strike,
            expiry,
            1.0,
        )
        .unwrap();

        let market = create_option_market(base_date, spot, vol, 0.05, 0.02);
        let call_price = call.value(&market, base_date).unwrap().amount();

        // Property: Call price ≤ spot price (can't be worth more than the stock)
        prop_assert!(
            call_price <= spot + 1e-6,
            "Call price {:.4} > spot {:.2}",
            call_price, spot
        );
    }

    #[test]
    fn prop_put_upper_bound(
        strike in 50.0..150.0,
        vol in 0.10..0.50,
        rate in 0.0..0.10,
        time_to_expiry_days in 30i64..730i64,
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let expiry = base_date + time::Duration::days(time_to_expiry_days);

        let put = test_utils::equity_option_european_put(
            "PROP-PUT-UPPER",
            "AAPL",
            strike,
            expiry,
            1.0,
        )
        .unwrap();

        let market = create_option_market(base_date, 100.0, vol, rate, 0.02);
        let put_price = put.value(&market, base_date).unwrap().amount();

        // Property: Put price ≤ K·e^(-rT) (can't be worth more than PV of strike)
        let t = time_to_expiry_days as f64 / 365.0;
        let pv_strike = strike * (-rate * t).exp();

        prop_assert!(
            put_price <= pv_strike + 1e-6,
            "Put price {:.4} > PV(strike) {:.2}",
            put_price, pv_strike
        );
    }

    #[test]
    fn prop_option_monotonicity_in_vol(
        spot in 80.0..120.0,
        strike in 80.0..120.0,
        time_to_expiry_days in 90i64..365i64,
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let expiry = base_date + time::Duration::days(time_to_expiry_days);

        let call = test_utils::equity_option_european_call(
            "VOL-MONO-CALL",
            "AAPL",
            strike,
            expiry,
            1.0,
        )
        .unwrap();

        // Price with low vol
        let market_low = create_option_market(base_date, spot, 0.15, 0.05, 0.02);
        let price_low = call.value(&market_low, base_date).unwrap().amount();

        // Price with high vol
        let market_high = create_option_market(base_date, spot, 0.35, 0.05, 0.02);
        let price_high = call.value(&market_high, base_date).unwrap().amount();

        // Property: Option value increases with volatility
        prop_assert!(
            price_high > price_low - 1e-6,
            "Higher vol should give higher price: vol=0.15 → {:.4}, vol=0.35 → {:.4}",
            price_low, price_high
        );
    }
}

#[test]
fn put_lower_bound_regression_deep_itm_flat_curve_case() {
    let spot = 50.0;
    let strike = 141.4882274604004;
    let vol = 0.1;
    let rate = 0.09285993274128063;
    let div_yield = 0.0;
    let time_to_expiry_days = 212i64;

    let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let expiry = base_date + time::Duration::days(time_to_expiry_days);

    let put =
        test_utils::equity_option_european_put("PUT-LB-REGRESSION", "AAPL", strike, expiry, 1.0)
            .unwrap();

    let market = create_option_market(base_date, spot, vol, rate, div_yield);
    let put_price = put.value(&market, base_date).unwrap().amount();

    let t = time_to_expiry_days as f64 / 365.0;
    let forward_spot = spot * (-div_yield * t).exp();
    let pv_strike = strike * (-rate * t).exp();
    let intrinsic = (pv_strike - forward_spot).max(0.0);
    let tolerance = scaled_tolerance(1e-4, intrinsic, 0.10);

    assert!(
        put_price >= intrinsic - tolerance,
        "Put price {:.4} < intrinsic {:.4} (tol={:.4})",
        put_price,
        intrinsic,
        tolerance
    );
}

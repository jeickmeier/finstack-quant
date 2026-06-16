//! Shared test fixtures and helpers for inflation swap tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationInterpolation, InflationLag,
};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
use finstack_quant_core::money::Money;

/// Build a flat discount curve with given rate
pub fn flat_discount(
    id: &str,
    base: Date,
    rate: f64,
) -> finstack_quant_core::Result<DiscountCurve> {
    let knots = vec![
        (0.0, 1.0),
        (1.0, (-rate).exp()),
        (5.0, (-rate * 5.0).exp()),
        (10.0, (-rate * 10.0).exp()),
        (30.0, (-rate * 30.0).exp()),
    ];

    // Ensure all discount factors are positive (handle floating point precision issues)
    let knots: Vec<(f64, f64)> = knots
        .into_iter()
        .map(|(t, df): (f64, f64)| {
            if t.abs() < 1e-15 {
                (t, 1.0) // First DF must be exactly 1.0
            } else {
                (t, df.max(1e-10)) // Ensure minimum positive value for others
            }
        })
        .collect();

    let mut builder = DiscountCurve::builder(id)
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots(knots);

    // For zero or negative rates, DFs may be flat or increasing
    // Use Linear interpolation since MonotoneConvex expects decreasing values
    if rate.abs() < 1e-10 || rate < 0.0 {
        builder = builder
            .validation(
                finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                    allow_non_monotonic: true,
                    forward_floor: None,
                },
            )
            .interp(finstack_quant_core::math::interp::InterpStyle::Linear);
    }

    builder.build()
}

/// Build a flat inflation curve with constant CPI growth rate
pub fn flat_inflation_curve(
    id: &str,
    base: Date,
    base_cpi: f64,
    annual_inflation_rate: f64,
) -> finstack_quant_core::Result<InflationCurve> {
    let mut knots = vec![
        (0.0, base_cpi),
        (1.0, base_cpi * (1.0 + annual_inflation_rate)),
        (5.0, base_cpi * (1.0 + annual_inflation_rate).powf(5.0)),
        (10.0, base_cpi * (1.0 + annual_inflation_rate).powf(10.0)),
        (30.0, base_cpi * (1.0 + annual_inflation_rate).powf(30.0)),
    ];

    // Ensure all CPI values are positive (handle floating point precision issues and extreme deflation)
    knots = knots
        .into_iter()
        .map(|(t, cpi)| (t, cpi.max(1e-10))) // Ensure minimum positive value
        .collect();

    InflationCurve::builder(id)
        .base_date(base)
        .base_cpi(base_cpi.max(1e-10)) // Also ensure base_cpi is positive
        .knots(knots)
        .build()
}

/// Build a realistic inflation curve with term structure
pub fn realistic_inflation_curve(id: &str, base: Date, base_cpi: f64) -> InflationCurve {
    // Realistic forward inflation: front-end 2.5%, mid 2.0%, long 1.8%
    let knots = vec![
        (0.0, base_cpi),
        (0.25, base_cpi * 1.00625), // 2.5% for Q1
        (0.5, base_cpi * 1.0125),   // 2.5% annualized
        (1.0, base_cpi * 1.025),    // 2.5% at 1Y
        (2.0, base_cpi * 1.047),    // ~2.3% avg to 2Y
        (5.0, base_cpi * 1.105),    // ~2.0% avg to 5Y
        (10.0, base_cpi * 1.197),   // ~1.85% avg to 10Y
        (30.0, base_cpi * 1.703),   // ~1.8% avg to 30Y
    ];
    InflationCurve::builder(id)
        .base_date(base)
        .base_cpi(base_cpi)
        .knots(knots)
        .build()
        .unwrap()
}

/// Build a simple inflation index with historical observations
pub fn simple_index(
    id: &str,
    base: Date,
    base_cpi: f64,
    ccy: Currency,
    lag: InflationLag,
) -> InflationIndex {
    // Provide two years of monthly history so lagged seasoned swaps can use
    // realized fixings rather than falling back to projected curve values.
    let monthly_growth = 1.002_f64;
    let observations = (0..=24)
        .rev()
        .map(|months_back| {
            let date = base.add_months(-months_back);
            let value = base_cpi / monthly_growth.powf(months_back as f64);
            (date, value)
        })
        .collect();
    InflationIndex::new(id, observations, ccy)
        .unwrap()
        .with_interpolation(InflationInterpolation::Linear)
        .with_lag(lag)
}

/// Build a comprehensive market context for testing
pub fn standard_market(as_of: Date, inflation_rate: f64, discount_rate: f64) -> MarketContext {
    let disc = flat_discount("USD-OIS", as_of, discount_rate)
        .unwrap_or_else(|_| panic!("Failed to build discount curve with rate {}", discount_rate));
    let infl_curve =
        flat_inflation_curve("US-CPI-U", as_of, 300.0, inflation_rate).unwrap_or_else(|_| {
            panic!(
                "Failed to build inflation curve with rate {}",
                inflation_rate
            )
        });
    let index = simple_index(
        "US-CPI-U",
        as_of,
        300.0,
        Currency::USD,
        InflationLag::Months(3),
    );

    MarketContext::new()
        .insert(disc)
        .insert(infl_curve)
        .insert_inflation_index("US-CPI-U", index)
}

/// Build a market with realistic curves
pub fn realistic_market(as_of: Date) -> MarketContext {
    let disc = flat_discount("USD-OIS", as_of, 0.045).unwrap(); // 4.5% nominal rate
    let infl_curve = realistic_inflation_curve("US-CPI-U", as_of, 300.0);
    let index = simple_index(
        "US-CPI-U",
        as_of,
        300.0,
        Currency::USD,
        InflationLag::Months(3),
    );

    MarketContext::new()
        .insert(disc)
        .insert(infl_curve)
        .insert_inflation_index("US-CPI-U", index)
}

/// Standard notional for tests
pub fn standard_notional() -> Money {
    Money::new(1_000_000.0, Currency::USD)
}

/// Large notional for sensitivity tests
pub fn large_notional() -> Money {
    Money::new(100_000_000.0, Currency::USD)
}

/// Tolerance for PV checks (0.01 basis point of notional)
pub fn pv_tolerance(notional: Money) -> f64 {
    notional.amount() * 1e-6
}

/// Tolerance for rate checks (0.01 bp)
pub fn rate_tolerance() -> f64 {
    1e-6
}

/// Tolerance for greek checks (relative)
/// Market standard: 2% for analytical vs finite difference comparison
pub fn greek_tolerance() -> f64 {
    0.02 // 2% relative tolerance for analytic vs FD approximations
}

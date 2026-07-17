// Shared risk-metric test helper fragment included by valuation source tests.
use crate::instruments::Bond;
use crate::metrics::risk::{MarketHistory, MarketScenario, RiskFactorShift, RiskFactorType};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use time::macros::date;

/// Standard valuation date used by risk tests.
pub fn sample_as_of() -> Date {
    date!(2024 - 01 - 01)
}

/// Deterministic USD-OIS discount curve used by test scenarios.
pub fn usd_ois_curve(as_of: Date) -> Result<DiscountCurve> {
    DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(vec![(0.0, 1.0), (5.0, 0.85), (10.0, 0.70)])
        .build()
}

/// Market context containing the standard USD-OIS discount curve.
pub fn usd_ois_market(as_of: Date) -> Result<MarketContext> {
    let curve = usd_ois_curve(as_of)?;
    Ok(MarketContext::new().insert(curve))
}

/// Convenience helper for constructing fixed-rate USD bonds for tests.
#[allow(clippy::expect_used)] // Test utility function with known valid inputs
pub fn standard_bond(id: &str, as_of: Date, maturity: Date) -> Bond {
    Bond::fixed(
        id,
        Money::new(100_000.0, Currency::USD),
        0.05,
        as_of,
        maturity,
        "USD-OIS",
    )
    .expect("standard_bond should build successfully")
}

/// Single-factor USD-OIS rate scenario used in VaR tests.
pub fn usd_ois_rate_scenario(date: Date, shift: f64) -> MarketScenario {
    MarketScenario::new(
        date,
        vec![RiskFactorShift {
            factor: RiskFactorType::DiscountRate {
                curve_id: CurveId::from("USD-OIS"),
                tenor_years: 5.0,
            },
            shift,
        }],
    )
}

/// Build a MarketHistory from a slice of (date, shift) tuples for USD-OIS rates.
pub fn history_from_rate_shifts(as_of: Date, shifts: &[(Date, f64)]) -> MarketHistory {
    let scenarios = shifts
        .iter()
        .map(|(date, shift)| usd_ois_rate_scenario(*date, *shift))
        .collect();
    MarketHistory::new(as_of, shifts.len() as u32, scenarios)
}

/// Convenience wrapper when explicit window sizing is needed.
pub fn history_from_scenarios(
    as_of: Date,
    window_days: u32,
    scenarios: Vec<MarketScenario>,
) -> MarketHistory {
    MarketHistory::new(as_of, window_days, scenarios)
}

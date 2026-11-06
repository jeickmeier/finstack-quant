#![cfg(test)]

//! Comprehensive FX Swap test suite following market standards.
//!
//! This module provides thorough test coverage for FX swap pricing, metrics,
//! and edge cases, organized into logical submodules for maintainability.
//!
//! Test organization:
//! - `fixtures`: Common test data and market setup
//! - `pricing`: Core valuation tests (PV, contract rates, edge cases)
//! - `metrics`: Individual metric calculator tests
//! - `integration`: Multi-metric and scenario tests
//! - `edge_cases`: Boundary conditions and error handling

// Note: Submodules (fixtures, pricing, metrics, integration, edge_cases)
// are declared in mod.rs to avoid duplicate module declarations.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_valuations_fx::fx_swap::FxSwap;
use time::Month;

#[test]
fn standard_swap_tenor_preserves_explicit_end_of_month_policy() {
    let swap = FxSwap::from_trade_date(
        finstack_quant_valuations_fx::fx_swap::FxSwapTradeDateParams {
            id: "EURUSD-SWAP-1M-EOM".into(),
            base_currency: Currency::EUR,
            quote_currency: Currency::USD,
            trade_date: Date::from_calendar_date(2024, Month::January, 29).expect("valid date"),
            far_tenor: Tenor::parse("1M").expect("valid tenor"),
            notional: Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"),
            domestic_discount_curve_id: "USD-OIS".into(),
            foreign_discount_curve_id: "EUR-OIS".into(),
            base_calendar_id: None,
            quote_calendar_id: None,
            settlement_days: 2,
            business_day_convention: BusinessDayConvention::Unadjusted,
            end_of_month: true,
        },
    )
    .expect("swap should build");

    assert_eq!(
        swap.near_date,
        Date::from_calendar_date(2024, Month::January, 31).expect("valid date")
    );
    assert_eq!(
        swap.far_date,
        Date::from_calendar_date(2024, Month::February, 29).expect("valid date")
    );
}

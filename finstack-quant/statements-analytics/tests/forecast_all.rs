//! Cargo only discovers integration tests that are direct children of
//! `tests/`. Nested forecast modules are included here so they run.

#[path = "forecast/forecast_backtesting_tests.rs"]
mod forecast_backtesting_tests;

//! Cashflow builder integration tests.
//!
//! This module contains tests for the cashflow builder, covering:
//!
//! - **amortization**: Notional and amortization spec validation
//! - **schedule**: Schedule generation, flow ordering, stub detection, outstanding tracking
//! - **credit_models**: PSA/SDA prepayment and default model golden values
//! - **principal_events**: Principal event date validation

mod amortization;
mod conventions;
mod credit_models;
mod floating_rate;
mod principal_events;
mod schedule;

//! Capital Structure Integration
//!
//! This module provides integration between financial models and capital structure
//! (debt instruments like bonds, swaps, loans). Valuation-backed instrument JSON
//! construction is available through the default `valuation-integration` feature.
//!
//! ## Features
//! - Construct bonds, swaps, and other debt instruments from specifications
//! - Generate cashflow schedules from instruments
//! - Aggregate cashflows by period (interest, principal, fees)
//! - DSL access to capital structure metrics via `cs.*` namespace
//!
//! ## DSL Integration
//! The `cs.*` namespace provides access to capital structure data in formulas:
//! - `cs.interest_expense.{instrument_id}` - Interest expense for specific instrument
//! - `cs.interest_expense.total` - Total interest expense across all instruments
//! - `cs.principal_payment.{instrument_id}` - Principal payment for specific instrument
//! - `cs.principal_payment.total` - Total principal payments across all instruments
//! - `cs.debt_balance.{instrument_id}` - Outstanding debt balance for specific instrument
//! - `cs.debt_balance.total` - Total outstanding debt balance
//!
//! Cashflows are classified by `CFKind` from `finstack-cashflows`. Outstanding
//! balances use `outstanding_by_date()`.
//!
//! ## Limitations
//! - Waterfall allocation within a payment category is single-class pro-rata;
//!   there is no intra-category tranche seniority (see [`WaterfallSpec`]).
//! - Prepayment penalties, call premiums, and original issue discount (OID)
//!   are not modeled: prepayments apply at par and no OID accretion occurs.
//!
//! ## Example
//! ```ignore
//! use finstack_core::market_data::{context::MarketContext, DiscountCurve};
//! use finstack_statements::prelude::*;
//!
//! use time::macros::date;
//!
//! # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
//! let issue_date = date!(2025-01-15);
//! let maturity_date = date!(2030-01-15);
//!
//! let model = ModelBuilder::new("LBO Model")
//!     .periods("2025Q1..2025Q4", Some("2025Q1"))?
//!     .value("revenue", &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))])
//!     // Add debt instruments
//!     .add_bond(
//!         "BOND-001",
//!         Money::new(10_000_000.0, Currency::USD),
//!         0.05, // 5% coupon
//!         issue_date,
//!         maturity_date,
//!         "USD-OIS",
//!     )?
//!     // Reference in formulas
//!     .compute("interest_expense", "cs.interest_expense.BOND-001")?
//!     .compute(
//!         "total_debt_service",
//!         "cs.interest_expense.total + cs.principal_payment.total",
//!     )?
//!     .build()?;
//!
//! let market_ctx = MarketContext::new().insert(
//!     DiscountCurve::builder("USD-OIS")
//!         .base_date(issue_date)
//!         .knots([(0.0, 1.0), (5.0, 0.90)])
//!         .build()?,
//! );
//!
//! let mut evaluator = Evaluator::new();
//! let results = evaluator.evaluate_with_market(&model, &market_ctx, issue_date)?;
//! let q1_interest = results.get("interest_expense", &PeriodId::quarter(2025, 1));
//! # let _ = q1_interest;
//! # Ok(())
//! # }
//! ```

mod builder;
mod cashflows;
pub(crate) mod integration;
pub(crate) mod period_flows;
mod state;
pub mod waterfall;
mod waterfall_spec;

// Curated public facade — preserves the same public type set as the old `types.rs`.
pub use cashflows::{CapitalStructureCashflows, CashflowBreakdown};
pub use integration::aggregate_instrument_cashflows;
#[cfg(feature = "valuation-integration")]
pub use integration::build_any_instrument_from_spec;
pub use period_flows::calculate_period_flows;
pub use state::CapitalStructureState;
pub use waterfall::{execute_waterfall, WaterfallPeriodResult};
pub use waterfall_spec::{EcfSweepSpec, PaymentPriority, PikToggleSpec, WaterfallSpec};

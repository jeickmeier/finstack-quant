//! Capital Structure Integration
//!
//! This module provides integration between financial models and capital structure
//! (debt instruments like bonds, swaps, loans). Valuation-backed instrument JSON
//! construction uses the canonical typed valuations instrument registry.
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
//! Cashflows are classified by `CFKind` from `finstack-quant-cashflows`. Outstanding
//! balances use `outstanding_by_date()`. Already-aggregated `cs.*` cash items
//! and balances convert on the inclusive period-end date with the explicit
//! `fx_policy`, not per contractual cashflow date; a cross-currency
//! conversion without a policy is an error.
//!
//! ## Limitations
//! - Waterfall allocation is pro-rata inside each payment class, walking class
//!   rank. Empty `payment_classes` is one implicit class (see `WaterfallSpec`).
//! - Prepayment penalties, call premiums, and original issue discount (OID)
//!   are not modeled: prepayments apply at par and no OID accretion occurs.
//!
//! ## Example
//! ```
//! use finstack_quant_core::market_data::{context::MarketContext, term_structures::DiscountCurve};
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::{PeriodId, StubKind};
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::types::{CurveId, InstrumentId, Rate};
//! use finstack_quant_statements::builder::ModelBuilder;
//! use finstack_quant_statements::evaluator::Evaluator;
//! use finstack_quant_statements::types::{AmountOrScalar, FinancialStatementInstrument};
//! use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
//!
//! use time::macros::date;
//!
//! # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
//! let issue_date = date!(2025-01-15);
//! let maturity_date = date!(2030-01-15);
//!
//! let model = {
//!     let mut model_builder = ModelBuilder::new("LBO Model");
//!     model_builder.periods("2025Q1..2025Q4", Some("2025Q1"))?;
//!     model_builder.value("revenue", &[
//!         (PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100.0)),
//!         (PeriodId::quarter(2025, 2).expect("valid period fixture"), AmountOrScalar::scalar(110.0)),
//!         (PeriodId::quarter(2025, 3).expect("valid period fixture"), AmountOrScalar::scalar(120.0)),
//!         (PeriodId::quarter(2025, 4).expect("valid period fixture"), AmountOrScalar::scalar(130.0)),
//!     ]);
//!     model_builder.availability_dates(
//!         "revenue",
//!         &[(PeriodId::quarter(2025, 1).expect("valid period fixture"), issue_date)],
//!     )?;
//!     // Build the instrument through its Valuations API and attach it.
//!     let bond = Bond::fixed(
//!         InstrumentId::new("BOND-001"),
//!         Money::from((10_000_000_i64, Currency::USD)),
//!         Rate::from_decimal(0.05)?, // 5% coupon
//!         issue_date,
//!         maturity_date,
//!         StubKind::ShortFront,
//!         CurveId::new("USD-OIS"),
//!     )?;
//!     model_builder.add_debt("BOND-001", FinancialStatementInstrument::Bond(bond));
//!     // Reference in formulas
//!     model_builder.compute("interest_expense", "cs.interest_expense.BOND-001")?;
//!     model_builder.compute(
//!         "total_debt_service",
//!         "cs.interest_expense.total + cs.principal_payment.total",
//!     )?;
//!     model_builder.build()?
//! };
//!
//! let market_ctx = MarketContext::new().insert(
//!     DiscountCurve::builder("USD-OIS")
//!         .base_date(issue_date)
//!         .day_count(finstack_quant_core::dates::DayCount::Act365F)
//!         .knots([(0.0, 1.0), (5.0, 0.90)])
//!         .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
//!         .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
//!         .build()?,
//! );
//!
//! let mut evaluator = Evaluator::new();
//! let results = evaluator.evaluate_with_market(
//!     &model,
//!     &market_ctx,
//!     issue_date,
//!     finstack_quant_statements::evaluator::ObservationCutoff::FullSnapshot,
//! )?;
//! let q1_interest = results.get("interest_expense", &PeriodId::quarter(2025, 1).expect("valid period fixture"));
//! # let _ = q1_interest;
//! # Ok(())
//! # }
//! ```

mod builder;
mod cashflows;
pub(crate) mod integration;
pub(crate) mod period_flows;
mod principal;
pub(crate) mod residual_schedule;
mod state;
pub(crate) mod waterfall;
mod waterfall_spec;

// Curated public facade for capital-structure types.
pub(crate) use cashflows::add_optional_money;
pub use cashflows::{
    CapitalStructureCashflows, CashflowBreakdown, DistressCashflow, PrepaymentOutcome,
    TOTAL_ROW_LABEL,
};
pub use integration::build_instrument_from_spec;
pub use period_flows::{
    SchedulePerspective, SuppliedSchedule, calculate_period_flows, scheduled_opening_principal,
};
pub(crate) use principal::PrincipalClaim;
pub(crate) use state::CapitalStructureState;
pub(crate) use waterfall_spec::reject_pool_output_dependence;
pub use waterfall_spec::{
    EcfSweepSpec, EcfTerm, FacilitySelection, PaymentClassSpec, PaymentPriority, PikToggleSpec,
    PrepaymentSpec, WaterfallSpec, default_priority_of_payments,
};

#[cfg(test)]
mod tests;

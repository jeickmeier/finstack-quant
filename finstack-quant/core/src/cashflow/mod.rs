//! Cashflow primitives and present value calculations.
//!
//! This module provides foundational types and functions for cashflow analysis,
//! including present value (NPV), internal rate of return (IRR/XIRR), and
//! discounting operations commonly used in fixed income and derivatives pricing.
//!
//! # When to use which API
//!
//! - Use [`crate::cashflow::npv`] / [`crate::cashflow::npv_with_ctx`] when discounting dated cashflows from a
//!   market curve. This is the pricing-oriented path used by most instruments.
//!   **Note:** these exclude flows dated on or before the valuation date
//!   (market-standard pricing semantics); use [`crate::cashflow::npv_with_options`]
//!   with [`crate::cashflow::NpvOptions::include_past_flows`] to include them.
//! - Use [`crate::cashflow::npv_amounts`] / [`crate::cashflow::npv_amounts_with_ctx`] for scalar cashflow studies
//!   driven by a single continuously compounded annual rate.
//! - Use [`crate::cashflow::irr`] for periodic cashflows and
//!   [`crate::cashflow::xirr`] / [`crate::cashflow::xirr_with_daycount`] /
//!   [`crate::cashflow::xirr_with_daycount_ctx`] for dated cashflows when the
//!   unknown quantity is the rate that makes NPV equal to zero.
//!
//! # Components
//!
//! - **Primitives** (`primitives`): Core types (`CashFlow`, `Notional`, `CFKind`)
//! - **Discounting** (`discounting`): Present value calculation with discount curves
//! - **XIRR** (`xirr`): IRR/XIRR metrics for investment analysis

//!
//! # Financial Concepts
//!
//! ## Net Present Value (NPV)
//!
//! The present value of a stream of future cashflows. In this module there are
//! two closely related conventions:
//!
//! - Curve-based NPV uses discount factors supplied by a market curve.
//! - Scalar `npv_amounts*` helpers convert a quoted annual rate into a
//!   continuously compounded discount factor over the chosen year-fraction
//!   basis.
//!
//! A generic scalar-rate form is:
//! ```text
//! NPV = Σ CF_i / (1 + r)^t_i
//! ```
//!
//! ## Internal Rate of Return (IRR)
//!
//! The discount rate that makes NPV = 0, representing the effective yield
//! of an investment:
//! ```text
//! 0 = Σ CF_i / (1 + IRR)^t_i
//! ```
//!
//! # Examples
//!
//! ## NPV Calculation
//!
//! For NPV with Money-denominated cashflows, use `npv()` with a discount curve:
//!
//! ```rust
//! use finstack_quant_core::cashflow::npv;
//! use finstack_quant_core::dates::DayCount;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::market_data::term_structures::FlatCurve;
//! use time::macros::date;
//!
//! let base = date!(2025 - 01 - 01);
//! let cf1 = (date!(2025 - 07 - 01), Money::new(1000.0, Currency::USD));
//! let cf2 = (date!(2026 - 01 - 01), Money::new(1000.0, Currency::USD));
//!
//! // Create a flat discount curve at 5% annual rate
//! let rate: f64 = 0.05;
//! let continuous_rate = (1.0 + rate).ln();
//! let curve = FlatCurve::new(continuous_rate, base, DayCount::Act365F, "EXAMPLE");
//!
//! let present_value = npv(&curve, base, &[cf1, cf2])?;
//! assert!(present_value.amount() > 0.0);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! For scalar cashflows, use `npv_amounts()`:
//!
//! ```rust
//! use finstack_quant_core::cashflow::npv_amounts;
//! use finstack_quant_core::dates::DayCount;
//! use time::macros::date;
//!
//! let base = date!(2025 - 01 - 01);
//! let flows = vec![
//!     (date!(2025 - 07 - 01), 1000.0),
//!     (date!(2026 - 01 - 01), 1000.0),
//! ];
//!
//! let present_value = npv_amounts(&flows, 0.05, Some(base), Some(DayCount::Act365F))?;
//! assert!(present_value > 0.0);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! ## IRR Calculation
//!
//! ```rust
//! use finstack_quant_core::cashflow::irr;
//!
//! // Initial investment followed by 4 quarterly returns (20% total return)
//! let cash_flows = vec![-10000.0, 3000.0, 3000.0, 3000.0, 3000.0];
//! let rate = irr(&cash_flows, None)?;
//! assert!(rate > 0.0); // Positive return
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! # References
//!
//! - Present value and discounting: `docs/REFERENCES.md#hull-options-futures`
//! - Fixed-income risk and rate interpretation:
//!   `docs/REFERENCES.md#tuckman-serrat-fixed-income`

mod discounting;
mod primitives;
mod xirr;

pub use discounting::{
    npv, npv_amounts, npv_amounts_with_ctx, npv_with_ctx, npv_with_options,
    sum_prediscounted_money, Discountable, NpvOptions,
};
pub use primitives::{CFKind, CashFlow, CashFlowAccrual};
pub use xirr::{irr, xirr, xirr_with_daycount, xirr_with_daycount_ctx};

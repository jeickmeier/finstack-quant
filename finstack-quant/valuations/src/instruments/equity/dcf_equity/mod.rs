//! Discounted Cash Flow (DCF) instruments for corporate valuation.
//!
//! DCF is the standard methodology for valuing companies based on projected
//! free cash flows. This module provides a first-class instrument for DCF
//! analysis, supporting Gordon Growth, Exit Multiple, and H-Model approaches
//! for terminal value calculation.
//!
//! # DCF Structure
//!
//! - **Explicit Period**: Projected free cash flows (typically 3-10 years)
//! - **Terminal Value**: Perpetuity value using Gordon Growth, Exit Multiple, or H-Model
//! - **Enterprise Value**: PV(explicit flows) + PV(terminal value)
//! - **Equity Value**: EV - Equity Bridge (or Net Debt) - Valuation Discounts
//!
//! # Key Features
//!
//! - **Mid-year convention**: Discount at `(t - 0.5)` for IB/PE practice
//! - **Structured equity bridge**: Total debt, cash, preferred, minority, non-op assets
//! - **Per-share value**: Diluted shares via treasury stock method
//! - **Valuation discounts**: DLOM, DLOC for private company valuations
//! - **H-Model**: Two-stage growth fade (Damodaran)
//!
//! # Valuation Formula
//!
//! ```text
//! EV = Σ FCF_t / (1 + WACC)^t + TV / (1 + WACC)^n
//! Equity = EV - Bridge Adjustments
//! FMV = Equity × (1 - DLOC) × (1 - DLOM)
//! ```
//!
//! # Terminal Value Methods
//!
//! ## Gordon Growth Model
//! ```text
//! TV = FCF_terminal × (1 + g) / (WACC - g)
//! ```
//!
//! ## Exit Multiple
//! ```text
//! TV = Terminal_Metric × Multiple
//! (e.g., EBITDA × 10x)
//! ```
//!
//! ## H-Model (Damodaran)
//! ```text
//! TV = FCF_T × (1+g_s)/(WACC-g_s) + FCF_T × H × (g_h-g_s)/(WACC-g_s)
//! ```
//!
//! # Use Cases
//!
//! - M&A valuation and deal pricing
//! - LBO analysis and sponsor returns
//! - Fairness opinions and 409A valuations
//! - Corporate strategy and capital allocation
//! - Public market intrinsic value (implied multiples)
//!
//! # Example
//!
//! ```rust
//! use finstack_quant_valuations::instruments::equity::dcf_equity::{DiscountedCashFlow, TerminalValueSpec};
//! use finstack_quant_core::types::InstrumentId;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Project free cash flows
//! let flows = vec![
//!     (Date::from_calendar_date(2026, Month::January, 1)?, 100_000.0),
//!     (Date::from_calendar_date(2027, Month::January, 1)?, 110_000.0),
//!     (Date::from_calendar_date(2028, Month::January, 1)?, 120_000.0),
//! ];
//!
//! let dcf = DiscountedCashFlow::builder()
//!     .id(InstrumentId::new("ACME-DCF"))
//!     .currency(Currency::USD)
//!     .flows(flows)
//!     .wacc(0.10) // 10% WACC
//!     .terminal_value(TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
//!     .net_debt(500_000.0)
//!     .valuation_date(Date::from_calendar_date(2025, Month::January, 1)?)
//!     .attributes(Default::default())
//!     .build()?;
//!
//! // Equity value is returned by the value() method
//! # Ok(())
//! # }
//! ```

pub(crate) mod metrics;
pub(crate) mod pricer;
mod types;

pub use types::{
    DilutionSecurity, DiscountedCashFlow, EquityBridge, TerminalValueSpec, ValuationDiscounts,
};

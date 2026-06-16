//! Money primitives and FX conversion utilities.
//!
//! This module provides:
//! - `Money` – a currency-tagged monetary amount with safe arithmetic
//! - `fx` – foreign-exchange interfaces and helpers used by conversions
//!
//! Arithmetic refuses to mix currencies unless converted explicitly using an
//! `FxProvider` (`fx::FxProvider`). Rounding follows per-currency scale with configurable
//! policies via [`crate::config`].
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::currency::Currency;
//! # fn main() -> finstack_quant_core::Result<()> {
//!
//! let gross = Money::new(125.25, Currency::USD);
//! let tax   = Money::new(10.00, Currency::USD);
//! let total = gross.checked_add(tax)?;
//! assert_eq!(format!("{}", total), "USD 135.25");
//! # Ok(())
//! # }
//! ```
//!
//! Default constructors ([`Money::new`][crate::money::Money::new],
//! [`Money::try_new`][crate::money::Money::try_new]) preserve the raw finite
//! amount without rounding -- internal pricing math relies on retained
//! precision. Use
//! [`Money::try_new_with_config`][crate::money::Money::try_new_with_config]
//! (or [`Money::new_with_config`][crate::money::Money::new_with_config]) when
//! you explicitly want ISO-4217 minor-unit rounding for ledger-style
//! construction.
/// Submodule for FX interfaces.
pub mod fx;

mod rounding;
mod types;

pub use types::{FormatOpts, Money};

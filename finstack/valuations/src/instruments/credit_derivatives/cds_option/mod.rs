//! CDS options (credit swaptions) with Black (1976) model.
//!
//! CDS options provide the right to buy or sell credit protection at a
//! predetermined spread. Also called credit swaptions, they are key
//! instruments for managing credit volatility.
//!
//! # Structure
//!
//! - **Payer option**: Right to buy protection (pay spread, receive if default)
//! - **Receiver option**: Right to sell protection (receive spread, pay if default)
//! - **Underlying**: Single-name CDS or index CDS
//! - **Strike**: CDS spread level
//!
//! # Pricing Model: Black (1976)
//!
//! CDS options are priced using Black (1976) on forward CDS spreads:
//!
//! ```text
//! Payer = RPV01 · [S_fwd · N(d₁) - K · N(d₂)]
//! Receiver = RPV01 · [K · N(-d₂) - S_fwd · N(-d₁)]
//! ```
//!
//! where RPV01 is the risky PV01 of the underlying CDS.
//!
//! # References
//!
//! - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit Derivatives*.
//!   Wiley Finance. Chapter 11: Credit Derivatives Options.
//!
//! - Pedersen, C. M. (2003). "Valuation of Portfolio Credit Default Swaptions."
//!   Lehman Brothers Quantitative Credit Research.
//!
//! # See Also
//!
//! - [`CDSOption`] for instrument struct
//! - [`crate::instruments::credit_derivatives::cds`] for underlying CDS pricing

#[doc(hidden)]
pub mod bloomberg_quadrature;
pub(crate) mod metrics;
pub(crate) mod parameters;
#[doc(hidden)]
pub mod pricer;
mod types;

pub use parameters::CDSOptionParams;
pub use types::{CDSOption, ProtectionStartConvention};

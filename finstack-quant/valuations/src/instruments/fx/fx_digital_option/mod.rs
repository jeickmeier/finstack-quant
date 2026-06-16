//! FX digital (binary) options using Garman-Kohlhagen adapted formulas.
//!
//! Digital options pay a fixed amount if the option expires in-the-money.
//! Two payout types are supported:
//!
//! - **Cash-or-nothing**: pays a fixed cash amount in the payout currency
//! - **Asset-or-nothing**: pays one unit of the foreign (base) currency
//!
//! # Pricing Model
//!
//! **Cash-or-nothing call:**
//! ```text
//! PV = e^{-r_d T} × N(d₂) × payout_amount
//! ```
//!
//! **Cash-or-nothing put:**
//! ```text
//! PV = e^{-r_d T} × N(-d₂) × payout_amount
//! ```
//!
//! **Asset-or-nothing call:**
//! ```text
//! PV = S × e^{-r_f T} × N(d₁) × notional
//! ```
//!
//! **Asset-or-nothing put:**
//! ```text
//! PV = S × e^{-r_f T} × N(-d₁) × notional
//! ```
//!
//! # Key Properties
//!
//! - Cash-or-nothing call + put = discounted payout (put-call parity for digitals)
//! - Asset-or-nothing call + put = discounted forward value
//! - A vanilla call = asset-or-nothing call - K × cash-or-nothing call
//!
//! # References
//!
//! - Reiner, E., & Rubinstein, M. (1991). "Unscrambling the Binary Code."
//!   *Risk Magazine*, 4(9), 75-83.
//! - Wystup, U. (2006). *FX Options and Structured Products*. Wiley.
//!
//! # See Also
//!
//! - [`FxDigitalOption`] for the instrument struct
//! - `pricer::FxDigitalOptionCalculator` for pricing calculations

/// FX digital option risk metrics
pub(crate) mod metrics;
/// FX digital option pricer implementation
pub(crate) mod pricer;
mod types;

pub use types::{DigitalPayoutType, FxDigitalOption};

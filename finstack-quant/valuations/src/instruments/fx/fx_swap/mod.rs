//! FX swap and FX forward instruments with interest rate parity pricing.
//!
//! FX swaps simultaneously exchange currencies at spot and re-exchange at
//! a forward date. FX forwards are single exchanges at a future date.
//! Pricing follows covered interest rate parity.
//!
//! # FX Swap Structure
//!
//! Two legs:
//! - **Near leg**: Exchange at spot rate (typically T+2)
//! - **Far leg**: Re-exchange at forward rate (e.g., 3M, 6M, 1Y later)
//!
//! The forward rate embeds the interest differential between currencies.
//!
//! # FX Forward Structure
//!
//! Single exchange at maturity:
//! - **Notional**: Amount in base currency
//! - **Forward rate**: Agreed exchange rate
//! - **Maturity**: Settlement date
//!
//! # Pricing: Covered Interest Rate Parity
//!
//! Forward exchange rate determined by no-arbitrage:
//!
//! ```text
//! F = S × (1 + r_quote × τ_quote) / (1 + r_base × τ_base)
//!   ≈ S × e^((r_quote - r_base) × T)  (continuous)
//! ```
//!
//! where:
//! - S = spot FX rate
//! - r_base, r_quote = interest rates in each currency
//! - τ = year fraction to settlement
//!
//! # Present Value
//!
//! For an FX forward position:
//!
//! ```text
//! PV_quote = Amount_base × (F_market - F_contract) × DF_quote(T)
//! ```
//!
//! # Market Usage
//!
//! - **Hedging**: Lock in future exchange rates
//! - **Carry trades**: Exploit interest rate differentials
//! - **Curve construction**: Calibrate cross-currency basis
//!
//! # See Also
//!
//! - `FxSwap` for instrument struct
//! - [`crate::instruments::fx::fx_spot`] for spot FX positions
//! - [`crate::instruments::fx::fx_option`] for FX options

pub(crate) mod metrics;
/// Shared pricing helper for CIP forward and PV calculations
pub(crate) mod pricing_helper;
mod types;

pub use crate::instruments::common_impl::parameters::FxUnderlyingParams;
pub use types::FxSwap;

// Builder provided by FinancialBuilder derive

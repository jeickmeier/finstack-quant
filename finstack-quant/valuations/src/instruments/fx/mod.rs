//! FX instruments and FX derivatives.
//!
//! This module provides foreign exchange instruments from spot and forwards
//! to exotic options. All instruments handle currency pairs correctly with
//! explicit base/quote conventions and support dual-curve discounting.
//!
//! # Features
//!
//! - **Spot & Forwards**: FX spot, outright forwards, NDFs
//! - **Swaps**: FX swaps with near/far legs
//! - **Vanilla Options**: European FX calls/puts (Garman-Kohlhagen)
//! - **Exotic Options**: Barriers, quantos
//! - **Volatility**: FX variance swaps
//!
//! # Currency Pair Convention
//!
//! FX instruments use the standard base/quote convention:
//! - `EUR/USD = 1.10` means 1 EUR = 1.10 USD
//! - Base currency is the numerator (EUR)
//! - Quote currency is the denominator (USD)
//!
//! # Pricing Models
//!
//! | Instrument | Model |
//! |------------|-------|
//! | Forwards | Interest rate parity |
//! | Vanilla Options | Garman-Kohlhagen (1983) |
//! | Barrier Options | Reiner-Rubinstein (1991), Monte Carlo |
//! | Quanto Options | Derman-Karasinski drift adjustment |
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_quant_valuations::instruments::fx::FxOption;
//!
//! // Use the example FX option (EUR/USD call)
//! let _option = FxOption::example().unwrap();
//! ```
//!
//! # Greeks
//!
//! FX options support Garman-Kohlhagen Greeks:
//! - **Delta (domestic)**: ∂V/∂S
//! - **Delta (foreign)**: Premium-adjusted delta
//! - **Gamma**: ∂²V/∂S²
//! - **Vega**: ∂V/∂σ
//! - **Rho (domestic)**: ∂V/∂r_d
//! - **Rho (foreign)**: ∂V/∂r_f
//!
//! # References
//!
//! - Garman, M. B., & Kohlhagen, S. W. (1983). "Foreign Currency Option Values."
//! - Reiner, E., & Rubinstein, M. (1991). "Breaking Down the Barriers."
//!
//! # See Also
//!
//! - [`FxOption`] for vanilla FX options
//! - [`FxForward`] for outright forwards
//! - [`FxBarrierOption`] for barrier options
//! - [`Ndf`] for non-deliverable forwards

/// FX barrier option module.
pub mod fx_barrier_option;
/// FX digital (binary) option module.
pub mod fx_digital_option;
/// FX forward module.
pub mod fx_forward;
/// FX option module - Vanilla FX options.
pub mod fx_option;
/// FX spot module - FX spot trades.
pub mod fx_spot;
/// FX swap module - FX swaps with near/far legs.
pub mod fx_swap;
/// FX touch option module - One-touch / no-touch options.
pub mod fx_touch_option;
/// FX variance swap module.
pub mod fx_variance_swap;
/// NDF module - Non-deliverable forwards.
pub mod ndf;
/// Quanto option module - Cross-currency quanto options.
pub mod quanto_option;
pub(crate) mod shared;

// Re-export primary types
pub use fx_barrier_option::FxBarrierOption;
pub use fx_digital_option::{DigitalPayoutType, FxDigitalOption};
pub use fx_forward::FxForward;
pub use fx_option::FxOption;
pub use fx_spot::FxSpot;
pub use fx_swap::FxSwap;
pub use fx_touch_option::{BarrierDirection, FxTouchOption, PayoutTiming, TouchType};
pub use fx_variance_swap::FxVarianceSwap;
pub use ndf::Ndf;
pub use quanto_option::QuantoOption;

//! Basis swap instruments for floating-for-floating rate exchanges.
//!
//! Basis swaps exchange two floating interest rate payments with different
//! tenors or indices, plus a fixed spread. Essential for multi-curve calibration
//! in the post-2008 framework where basis spreads between LIBOR tenors emerged.
//!
//! # Structure
//!
//! Two floating legs with:
//! - **Primary leg**: Floating rate index 1 (e.g., 3M-SOFR)
//! - **Reference leg**: Floating rate index 2 (e.g., 6M-SOFR)
//! - **Basis spread**: Fixed spread added to one leg
//!
//! # Pricing
//!
//! Present value is the difference between projected floating legs:
//!
//! ```text
//! PV = PV(Primary leg + spread) - PV(Reference leg)
//!    = Σ N·τᵢ·[Fwd₁(tᵢ) + spread]·DF(tᵢ) - Σ N·τⱼ·Fwd₂(tⱼ)·DF(tⱼ)
//! ```
//!
//! Note: By convention, the spread is added to the **primary leg** (not the reference leg).
//!
//! # Multi-Curve Calibration
//!
//! Basis swaps are used to calibrate the spread between forward curves:
//! - 3M-LIBOR vs 6M-LIBOR basis
//! - LIBOR vs OIS basis
//! - SOFR vs Fed Funds basis
//!
//! # Market Conventions
//!
//! - **USD**: Quarterly vs semi-annual tenors common
//! - **EUR**: 3M vs 6M EURIBOR basis
//! - **Spread quoting**: Typically in basis points on the shorter tenor leg
//!
//! # References
//!
//! - Ametrano, F. M., & Bianchetti, M. (2013). "Everything You Always Wanted
//!   to Know About Multiple Interest Rate Curve Bootstrapping but Were Afraid
//!   to Ask." *SSRN Working Paper*.
//!
//! - Fujii, M., Shimada, Y., & Takahashi, A. (2010). "A Note on Construction
//!   of Multiple Swap Curves with and without Collateral." *CARF Working Paper*.
//!
//! # See Also
//!
//! - [`BasisSwap`] for instrument struct
//! - `BasisSwapLeg` for leg specification
//! - Multi-curve calibration in [`calibration`](crate::calibration)

pub(crate) mod metrics;
mod types;

pub use types::{BasisSwap, BasisSwapLeg};

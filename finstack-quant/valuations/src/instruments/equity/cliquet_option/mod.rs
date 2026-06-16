//! Cliquet (ratchet) option instruments with periodic resets.
//!
//! Cliquet options are path-dependent options with returns calculated over
//! multiple periods with periodic resets. Also known as ratchet options,
//! they lock in gains at observation dates while maintaining upside potential.
//!
//! # Structure
//!
//! - **Reset dates**: Regular observation schedule (monthly, quarterly, annual)
//! - **Local returns**: Performance measured over each period
//! - **Global payoff**: Sum or product of local returns
//! - **Caps/floors**: Optional limits on each period's return
//!
//! Typical payoff:
//! ```text
//! Payoff = Notional × Σᵢ min(max(Sᵢ/Sᵢ₋₁ - 1, Floor), Cap)
//! ```
//!
//! # Pricing Method
//!
//! Cliquets require Monte Carlo simulation due to:
//! - Path dependency with multiple reset dates
//! - Complex return aggregation logic
//! - No closed-form solution exists
//!
//! # Market Usage
//!
//! Popular in structured products for:
//! - Equity-linked notes
//! - Index participation products
//! - Guaranteed return products
//!
//! # See Also
//!
//! - [`CliquetOption`] for instrument struct
//! - Monte Carlo pricer for path-dependent pricing

pub(crate) mod metrics;
pub mod monte_carlo;
pub(crate) mod pricer;
pub(crate) mod traits;
pub(crate) mod types;

pub use types::{CliquetOption, CliquetPayoffType};

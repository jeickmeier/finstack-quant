//! Constant Maturity Swap (CMS) options with convexity adjustments.
//!
//! CMS options are options on swap rates with constant tenor (e.g., always
//! referencing a 10-year swap rate). They require convexity adjustments to
//! account for the difference between forward swap rates and CMS rates.
//!
//! # CMS Rate
//!
//! Unlike a forward-starting swap (fixed maturity date), a CMS rate always
//! references a swap with fixed tenor from the reset date:
//! - CMS-10Y on reset date t always prices a 10Y swap starting at t
//!
//! # Convexity Adjustment
//!
//! CMS rates trade above forward swap rates due to convexity. The adjustment:
//!
//! ```text
//! CMS_Rate ≈ Forward_Swap_Rate + Convexity_Adjustment
//! ```
//!
//! where the adjustment depends on volatility and correlation structure.
//!
//! # Pricing Model
//!
//! CMS options use:
//! - **Black (1976)** on adjusted CMS forward rate
//! - **Replication methods**: Static replication via swaption portfolio
//! - **SABR/LMM models**: For accurate convexity and smile
//!
//! # References
//!
//! - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models - Theory and Practice*
//!   (2nd ed.). Springer. Chapter 13.7: CMS Products.
//!
//! - Hagan, P. S. (2003). "Convexity Conundrums: Pricing CMS Swaps, Caps, and
//!   Floors." *Wilmott Magazine*, March, 38-44.
//!
//! # See Also
//!
//! - [`CmsOption`] for instrument struct
//! - CMS option metrics module for risk metrics

pub(crate) mod metrics;
pub mod pricer;
pub mod replication_pricer;
pub(crate) mod types;

pub use types::CmsOption;

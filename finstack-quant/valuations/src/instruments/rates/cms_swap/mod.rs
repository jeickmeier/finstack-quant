//! Constant Maturity Swap (CMS) instrument.
//!
//! A CMS swap has one leg paying a CMS rate (the par swap rate for a fixed
//! reference tenor, e.g., 10Y) and the other leg paying a fixed or floating
//! rate. Unlike a forward-starting swap (fixed maturity date), the CMS rate
//! always references a swap with fixed tenor from the reset date.
//!
//! # Convexity Adjustment
//!
//! CMS rates trade above forward swap rates due to convexity. The adjustment:
//!
//! ```text
//! CMS_Rate ≈ Forward_Swap_Rate + Convexity_Adjustment
//! ```
//!
//! where the adjustment depends on volatility and the rate level, per the
//! Hagan (2003) linear swap rate model.
//!
//! # Pricing Model
//!
//! The CMS leg uses convexity-adjusted forward swap rates, while the funding
//! leg is priced as a standard fixed or floating leg.
//!
//! # Model Selection (long tenors / high vol)
//!
//! The default `Black76` pricer applies the **first-order Hagan adjustment**
//! with the ATM vol. For CMS tenors beyond ~10 years or high-vol regimes this
//! understates the adjustment by roughly 5–10 bp versus full smile-aware
//! static replication (Andersen & Piterbarg 2010, §16.2). For those trades,
//! select the **`StaticReplication`** model
//! ([`CmsSwapReplicationPricer`](pricer::CmsSwapReplicationPricer)): it prices
//! each coupon's payment-measure expectation with the same exact replication
//! engine as the [`CmsOption`](crate::instruments::rates::cms_option)
//! `StaticReplication` model, via the model-free parity
//! `E[S] = K + caplet(K) − floorlet(K)` at `K = F`, and prices embedded
//! caps/floors with smile-consistent replicated optionlets. Note the
//! replication model requires positive forward swap rates; use the Hagan /
//! Bachelier default in negative-rate regimes.
//!
//! # References
//!
//! - Hagan, P. S. (2003). "Convexity Conundrums: Pricing CMS Swaps, Caps, and
//!   Floors." *Wilmott Magazine*, March, 38-44.
//! - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models - Theory and Practice*
//!   (2nd ed.). Springer. Chapter 13.7: CMS Products.

pub(crate) mod metrics;
pub mod pricer;
pub(crate) mod types;

pub use types::{CmsSwap, FundingLeg, FundingLegSpec};

//! CDS index instruments (CDX, iTraxx) following ISDA standards.
//!
//! CDS indices provide standardized credit exposure to portfolios of reference
//! entities. The most common indices are CDX (North America) and iTraxx (Europe/Asia).
//! Pricing follows ISDA conventions adapted for index-specific features.
//!
//! # Index Structure
//!
//! A CDS index tracks a basket of single-name CDS:
//! - **Constituents**: Typically 125 (CDX IG) or 100 (iTraxx Main) entities
//! - **Equal weights**: Each name represents 1/N of the index
//! - **Fixed coupon**: Standard running spreads (25bp, 100bp, or 500bp)
//! - **Roll schedule**: New series every 6 months with updated constituents
//!
//! # Pricing Methods
//!
//! ## Intrinsic (Bottom-Up)
//!
//! Price index as weighted average of constituent CDS:
//!
//! ```text
//! PV_index = (1/N) · Σᵢ PV(CDSᵢ)
//! ```
//!
//! where PV(CDSᵢ) is the single-name CDS value for constituent i.
//!
//! **Advantages**: Accounts for individual credit quality
//! **Disadvantages**: Requires full constituent list and spreads
//!
//! ## Index-Level (Top-Down)
//!
//! Price directly using index spread and average recovery:
//!
//! ```text
//! PV_index = PV_prot(S_index, R_avg) - PV_prem(coupon)
//! ```
//!
//! **Advantages**: Fast, uses directly quoted index spread
//! **Disadvantages**: Ignores dispersion of constituent spreads
//!
//! # Default Handling
//!
//! When a constituent defaults:
//! - Protection payment: (1 - R) × (1/N) × Notional
//! - Index weight: Permanently reduced by 1/N
//! - Ongoing premium: Reduced proportionally
//!
//! # Major CDS Indices
//!
//! - **CDX.NA.IG**: 125 North American investment-grade names
//! - **CDX.NA.HY**: 100 North American high-yield names
//! - **iTraxx Europe**: 125 European investment-grade names
//! - **iTraxx Crossover**: 75 European sub-investment grade names
//! - **CDX.EM**: 14-40 emerging market sovereign names
//!
//! # ISDA Standards and References
//!
//! ## Industry Standards
//!
//! - ISDA (2009). "ISDA CDS Standard Model." Version 1.8.2.
//!   (Base model extended to indices)
//!
//! - Markit (ongoing). "CDX and iTraxx Index Rules."
//!   (Index composition and roll conventions)
//!
//! ## Academic References
//!
//! - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit Derivatives*.
//!   Wiley Finance. Chapter 7: Index Products.
//!
//! - Laurent, J.-P., & Gregory, J. (2005). "Basket Default Swaps, CDOs and
//!   Factor Copulas." *Journal of Risk*, 7(4), 103-122.
//!
//! # Implementation Notes
//!
//! - Supports both intrinsic (bottom-up) and index-level (top-down) pricing
//! - Default handling reduces index factor appropriately
//! - IMM roll dates and standardized maturities
//! - Average recovery rate computed from constituents or assumed
//!
//! # Examples
//!
//! See [`CDSIndex`] for construction and usage examples.
//!
//! # See Also
//!
//! - `CDSIndex` for index CDS struct
//! - `CDSIndexConstituent` for constituent entity information
//! - `IndexPricing` for intrinsic vs index-level pricing mode
//! - index metrics module for index risk metrics

pub(crate) mod metrics;
pub(crate) mod parameters;
pub(crate) mod pricer;
mod types;

pub use parameters::CDSIndexParams;
pub use types::{
    CDSIndex, CDSIndexConstituent, ConstituentResult, IndexParSpreadResult, IndexPricing,
    IndexResult, ParSpreadMethod,
};

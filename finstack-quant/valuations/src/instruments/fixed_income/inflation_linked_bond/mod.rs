//! Inflation-linked bonds (TIPS, linkers) with real yield pricing.
//!
//! Inflation-linked bonds provide protection against inflation by adjusting
//! principal and/or coupons based on a consumer price index. Major markets
//! include US TIPS, UK Index-Linked Gilts, and Euro inflation-linked bonds.
//!
//! # Indexation Methods
//!
//! - **Capital indexation**: Principal indexed by CPI ratio (TIPS, most linkers)
//! - **Interest indexation**: Coupons indexed, principal fixed (Canada RRBs)
//! - **Zero-coupon**: Principal indexed at maturity only
//!
//! # Pricing
//!
//! Inflation-linked bonds are priced using the **Nominal Pricing Method** (Fisher Equation):
//!
//! ```text
//! PV = Σ [CF_nom(t) × DF_nom(t)]
//! ```
//!
//! where:
//! - `CF_nom(t) = CF_real × IndexRatio(t)` (Projected Nominal Cashflow)
//! - `DF_nom(t)` = Nominal Discount Factor from the discount curve
//!
//! The `discount_curve_id` should point to a **Nominal** discount curve (e.g., USD-OIS or specialized Nominal curve).
//!
//! ## Real Yield
//!
//! The `real_yield` method calculates the internal rate of return of the **Real (Unadjusted) Cashflows**
//! against the **Real Price**:
//!
//! ```text
//! Real Price (Dirty) = Quoted Real Clean Price + Real Accrued Interest
//! Real Price = Σ [CF_real(t) × (1 + y)^-(t)]
//! ```
//!
//! This aligns with standard market quoting conventions (e.g., TIPS).
//!
//! # Index Ratio Calculation
//!
//! ```text
//! Index Ratio = CPI(t) / CPI(base)
//! ```
//!
//! with lag adjustments (typically 3 months for TIPS, 8 months for UK Gilts).
//!
//! # Deflation Protection
//!
//! - **Floor at par**: Principal never falls below 100 (TIPS, most linkers)
//! - **No floor**: Principal can fall below 100 (some Euro linkers)
//!
//! # Market Conventions
//!
//! ## US TIPS
//! - **Index**: CPI-U (non-seasonally adjusted)
//! - **Lag**: 3 months
//! - **Coupon**: Semi-annual, ACT/ACT
//! - **Deflation floor**: Yes (principal ≥ 100)
//!
//! ## UK Index-Linked Gilts
//! - **Index**: RPI (pre-2005) or CPI (post-2005)
//! - **Lag**: 8 months (old), 3 months (new)
//! - **Coupon**: Semi-annual, ACT/ACT
//! - **Deflation floor**: Yes
//!
//! ## Euro Inflation-Linked
//! - **Index**: HICP ex-tobacco
//! - **Lag**: 3 months
//! - **Coupon**: Annual, ACT/ACT
//! - **Deflation floor**: Varies by issuer
//!
//! # Key Metrics
//!
//! - **Real yield**: Yield over inflation
//! - **Breakeven inflation**: Implied average inflation
//! - **Inflation01**: Sensitivity to 1bp CPI change
//!
//! # References
//!
//! - Deacon, M., Derry, A., & Mirfendereski, D. (2004). *Inflation-Indexed
//!   Securities: Bonds, Swaps and Other Derivatives* (2nd ed.). Wiley.
//!
//! - Barclays Capital (2011). "The Barclays Capital Guide to Inflation-Linked Bonds."
//!
//! # See Also
//!
//! - [`InflationLinkedBond`] for instrument struct
//! - [`crate::instruments::fixed_income::inflation_linked_bond::IndexationMethod`] for indexation type
//! - [`crate::instruments::fixed_income::inflation_linked_bond::DeflationProtection`] for floor specifications
//! - inflation-linked bond metrics module for inflation-specific risk metrics

pub(crate) mod metrics;
pub(crate) mod parameters;
/// Inflation-linked bond pricer implementation
pub(crate) mod pricer;
mod types;

pub use parameters::InflationLinkedBondParams;
pub use types::{DeflationProtection, IndexationMethod, InflationLinkedBond};

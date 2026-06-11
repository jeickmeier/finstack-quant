//! Repurchase agreement (repo) instruments with collateral haircuts.
//!
//! Repos are short-term secured lending transactions where one party sells
//! securities to another with an agreement to repurchase at a higher price.
//! Widely used for short-term funding and liquidity management.
//!
//! # Repo Structure
//!
//! - **Cash lender**: Provides cash, receives collateral
//! - **Cash borrower**: Provides collateral, receives cash
//! - **Repo rate**: Interest rate implicit in price difference
//! - **Haircut**: Collateral value discount for credit protection
//!
//! # Types
//!
//! - **Classic repo**: Sell-and-repurchase of specific securities
//! - **General collateral (GC)**: Any eligible security as collateral
//! - **Triparty repo**: Third party manages collateral
//! - **Reverse repo**: Cash lender's perspective (opposite of repo)
//!
//! # Pricing
//!
//! Repo effectively borrows/lends cash at the repo rate:
//!
//! ```text
//! Repurchase_price = Sale_price × (1 + Repo_rate × τ)
//! ```
//!
//! Present value from cash lender perspective (cash out at start, repayment
//! received at maturity):
//!
//! ```text
//! PV = -Sale_price + Repurchase_price × DF(maturity)
//! ```
//!
//! # Haircut Calculation
//!
//! Haircut protects against collateral value decline:
//!
//! ```text
//! Cash_lent = Collateral_value × (1 - Haircut%)
//! Collateral_value = Cash_lent / (1 - Haircut%)
//! ```
//!
//! Typical haircuts:
//! - **Treasuries**: 0-2%
//! - **Investment-grade bonds**: 2-5%
//! - **Equities**: 5-15%
//! - **High-yield bonds**: 10-25%
//!
//! # Market Conventions
//!
//! - **Day count**: ACT/360 (USD/EUR), ACT/365 (GBP)
//! - **Term**: Overnight to 1 year typical
//! - **Settlement**: T+0, T+1, or T+2 depending on collateral
//!
//! # Key Metrics
//!
//! - **Repo rate**: Implied borrowing/lending rate
//! - **Haircut01**: Sensitivity to haircut changes
//! - **Collateral price01**: Sensitivity to collateral value
//!
//! # See Also
//!
//! - [`Repo`] for instrument struct
//! - [`CollateralSpec`] for collateral details
//! - [`RepoType`] for classic vs GC vs triparty

pub(crate) mod metrics;
mod types;

pub use types::{CollateralSpec, CollateralType, Repo, RepoBuilder, RepoType};

// Builder is generated via derive on `Repo`.

//! Multi-asset baskets (ETFs, custom indices, portfolios) valued as their net
//! asset value.
//!
//! # Basket Structure
//!
//! A basket holds constituents, an expense ratio, a reporting currency and a
//! position notional. Each constituent references either a market-data price
//! (`ConstituentReference::MarketData`) or a full instrument definition
//! (`ConstituentReference::Instrument`) priced through its own valuation, and
//! is held either by explicit `units` or by a `weight`.
//!
//! # Pricing
//!
//! The NAV per basket unit sums the constituent contributions, each converted
//! into the reporting currency under the basket's FX policy:
//!
//! ```text
//! contribution_i = Price_i × Units_i   (unit-held constituent)
//! contribution_i = Price_i × w_i       (weight-held constituent)
//! NAV            = Σᵢ contribution_i − drag
//! drag           = Σᵢ contribution_i × Expense_ratio / days_in_year
//! ```
//!
//! The drag is a single day of fees on the day basis in
//! [`BasketPricingConfig`](crate::instruments::exotics::basket::BasketPricingConfig) (365.25 by default). The basket PV is the NAV
//! scaled by the notional amount.
//!
//! # Metrics
//!
//! - **Constituent count**: number of holdings
//! - **Expense ratio**: the configured ratio, in percent
//! - **Constituent delta**: basket PV change for a +1% move in each
//!   constituent price
//! - **Weight risk**: basket PV change for a 1bp weight reallocation into each
//!   constituent, funded pro rata from the others
//!
//! # References
//!
//! - Gastineau, G. L. (2010). *The Exchange-Traded Funds Manual* (2nd ed.). Wiley.
//!
//! # Module Organization
//!
//! - `types`: `Basket` instrument struct and constituent definitions
//! - `pricer`: NAV calculation
//! - `metrics`: Basket-specific metrics

pub(crate) mod metrics;
pub(crate) mod pricer;
pub(crate) mod types;

// Builder is generated via derive on `Basket`.
pub use types::{
    Basket, BasketAssetType, BasketConstituent, BasketPricingConfig, ConstituentReference,
};

//! Multi-asset basket instruments for ETFs, indices, and portfolios.
//!
//! basket modeling for:
//! - **ETFs**: Exchange-traded funds with equity/bond constituents
//! - **Custom indices**: Proprietary index construction
//! - **Portfolio instruments**: Multi-asset portfolios with rebalancing
//! - **Worst-of products**: Exotic options on basket minimum
//! - **Best-of products**: Exotic options on basket maximum
//!
//! # Basket Structure
//!
//! A basket consists of:
//! - **Constituents**: Individual assets (equities, bonds, other instruments)
//! - **Weights**: Allocation to each constituent (sum to 1.0 or arbitrary)
//! - **Currency**: Base currency for basket valuation
//! - **Expense ratio**: Annual management/tracking fees
//!
//! # Constituent Types
//!
//! Constituents can be referenced as:
//!
//! - **Market data**: Lookup from market data feed
//!   - Asset type: Equity, Bond, Cash
//!   - Price ID for market data resolution
//!
//! - **Instrument reference**: Direct instrument object
//!   - Full pricing via instrument's own valuation
//!   - Supports any instrument type (bonds, options, etc.)
//!
//! - **Mixed**: Some market data, some direct instruments
//!
//! # Pricing
//!
//! Basket value is the weighted sum of constituent values:
//!
//! ```text
//! Basket_value = Σᵢ wᵢ × Price(constituent_i) - Fees
//! ```
//!
//! For unit-based weighting:
//! ```text
//! Basket_value = Σᵢ Units_i × Price(constituent_i) - Fees
//! ```
//!
//! # NAV Calculation
//!
//! Net Asset Value per share/unit:
//!
//! ```text
//! NAV = (Total_assets - Fees) / Shares_outstanding
//! ```
//!
//! # Expense Ratio
//!
//! Annual fees accrue continuously:
//! ```text
//! Fee_accrual = Basket_value × Expense_ratio × (Days/365)
//! ```
//!
//! # Rebalancing
//!
//! Baskets may rebalance:
//! - **Fixed weights**: Periodic rebalancing to target weights
//! - **Float weights**: Market value weighted (no rebalancing)
//! - **Threshold rebalancing**: Only when drift exceeds tolerance
//!
//! # Use Cases
//!
//! ## ETF Modeling
//! - Track index replication
//! - Compute tracking error
//! - Estimate creation/redemption costs
//!
//! ## Exotic Basket Options
//! - **Worst-of call**: Payoff = max(min(S₁, S₂, ..., Sₙ) - K, 0)
//! - **Best-of put**: Payoff = max(K - max(S₁, S₂, ..., Sₙ), 0)
//! - **Average basket**: Payoff based on weighted average
//!
//! ## Portfolio Instruments
//! - Custom portfolio tracking
//! - Performance attribution
//! - Risk aggregation
//!
//! # Key Metrics
//!
//! - **NAV**: Net asset value
//! - **Constituent delta**: Sensitivity to each constituent
//! - **Weight risk**: Sensitivity to weight changes
//! - **Asset exposure**: Exposure by asset type
//! - **Constituent count**: Number of holdings
//!
//! # References
//!
//! ## ETF and Index Construction
//!
//! - Gastineau, G. L. (2010). *The Exchange-Traded Funds Manual* (2nd ed.). Wiley.
//!
//! - Madhavan, A. (2016). "Exchange-Traded Funds and the New Dynamics of Investing."
//!   Oxford University Press.
//!
//! ## Basket Option Pricing
//!
//! - Ju, N. (2002). "Pricing Asian and Basket Options via Taylor Expansion."
//!   *Journal of Computational Finance*, 5(3), 79-103.
//!
//! - Curran, M. (1994). "Valuing Asian and Portfolio Options by Conditioning on
//!   the Geometric Mean Price." *Management Science*, 40(12), 1705-1711.
//!
//! # Implementation Notes
//!
//! - **Flexible constituent types**: Market data references or direct instruments
//! - **Currency safety**: All constituent values converted to basket currency
//! - **Performance**: Caches constituent valuations during metrics calculation
//! - **Rebalancing**: Configurable rebalancing logic
//!
//! # Module Organization
//!
//! - `types`: `Basket` instrument struct and constituent definitions
//! - `pricer`: Basket valuation calculator
//! - `metrics`: Constituent-level risk metrics
//!
//! # See Also
//!
//! - [`Basket`] for main instrument struct
//! - `BasketConstituent` for constituent specification
//! - `ConstituentReference` for market data vs instrument references
//! - `AssetType` for constituent asset classification

pub(crate) mod metrics;
pub(crate) mod pricer;
pub(crate) mod types;

// Re-export main types for convenience
// Builder is generated via derive on `Basket`.
#[doc(hidden)]
pub use metrics::{AssetExposureCalculator, ConstituentCountCalculator, ExpenseRatioCalculator};
pub use pricer::BasketCalculator;
pub use types::{AssetType, Basket, BasketConstituent, BasketPricingConfig, ConstituentReference};

// Generic pricer implementation is used directly via common_impl::GenericInstrumentPricer

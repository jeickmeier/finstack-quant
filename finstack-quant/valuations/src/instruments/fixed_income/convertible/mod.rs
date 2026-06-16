//! Convertible bond instruments with embedded equity conversion features.
//!
//! Convertible bonds combine fixed income and equity characteristics,
//! providing bondholders the option to convert into common stock at
//! predetermined conversion ratios. Pricing requires hybrid models
//! accounting for both credit and equity risks.
//!
//! # Structure
//!
//! - **Bond component**: Fixed coupons, principal, credit spread
//! - **Conversion option**: Right to convert to equity at conversion price
//! - **Call provision**: Issuer can force conversion (soft call)
//! - **Put provision**: Holder can put back to issuer
//!
//! # Conversion Mechanics
//!
//! - **Conversion ratio**: Shares received per bond unit
//! - **Conversion price**: Effective stock price = Par / Conversion ratio
//! - **Conversion value**: Stock price × Conversion ratio
//! - **Conversion premium**: (Bond price - Conversion value) / Conversion value
//!
//! # Pricing Models
//!
//! - **Tree methods**: Trinomial trees with credit and equity factors
//! - **Partial differential equations**: Finite difference methods
//! - **Monte Carlo**: For complex features and path dependency
//!
//! # Key Features
//!
//! - **Credit quality**: Embedded CDS spread or hazard curve
//! - **Anti-dilution**: Adjustments for stock splits, dividends
//! - **Soft call**: Can only call if stock above trigger (e.g., 130% of conversion price)
//! - **Make-whole**: Additional compensation if called early
//!
//! # References
//!
//! - Tsiveriotis, K., & Fernandes, C. (1998). "Valuing Convertible Bonds with
//!   Credit Risk." *Journal of Fixed Income*, 8(2), 95-102.
//!
//! - Ayache, E., Forsyth, P. A., & Vetzal, K. R. (2003). "Valuation of
//!   Convertible Bonds with Credit Risk." *Journal of Derivatives*, 11(1), 9-29.
//!
//! # See Also
//!
//! - [`ConvertibleBond`] for instrument struct
//! - `ConversionSpec` for conversion terms
//! - `AntiDilutionPolicy` for adjustment policies

mod market_inputs;
pub(crate) mod metrics;
pub(crate) mod pricer;
mod types;

pub use types::{
    AntiDilutionPolicy, ConversionEvent, ConversionPolicy, ConversionSpec, ConvertibleBond,
    ConvertibleGreeks, DilutionEvent, DividendAdjustment, SoftCallTrigger,
};

// Re-export pricing helpers for benches/tools.
pub use pricer::{
    calculate_accrued_interest, calculate_conversion_premium, calculate_convertible_greeks,
    calculate_parity, price_convertible_bond, settlement_date, ConvertibleTreeType,
};

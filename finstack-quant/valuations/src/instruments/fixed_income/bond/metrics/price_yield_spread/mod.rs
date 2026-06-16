//! Price, yield, and spread metrics for bonds.

/// Asset swap spread calculators (par, market, forward)
pub(crate) mod asw;
/// Discount margin calculator
pub(crate) mod dm;
/// Embedded option value calculator
pub(crate) mod embedded_option_value;
/// I-spread (interpolated spread) calculator
pub(crate) mod i_spread;
/// Option-adjusted spread (OAS) calculator
pub(crate) mod oas;
/// Price calculators (clean and dirty)
pub(crate) mod prices;
/// Callable/putable bond OAS model vega
pub(crate) mod vega;
/// Yield-to-maturity (YTM) calculator
pub(crate) mod ytm;
/// Yield-to-worst (YTW) calculator
pub(crate) mod ytw;
/// Z-spread (zero-volatility spread) calculator
pub(crate) mod z_spread;

pub use asw::{AssetSwapMarketCalculator, AssetSwapParCalculator};
pub use dm::DiscountMarginCalculator;
pub(crate) use embedded_option_value::EmbeddedOptionValueCalculator;
pub(crate) use i_spread::ISpreadCalculator;
pub(crate) use oas::OasCalculator;
pub(crate) use prices::{CleanPriceCalculator, DirtyPriceCalculator};
pub(crate) use vega::BondVegaCalculator;
pub(crate) use ytm::YtmCalculator;
pub(crate) use ytw::YtwCalculator;
pub use z_spread::ZSpreadCalculator;

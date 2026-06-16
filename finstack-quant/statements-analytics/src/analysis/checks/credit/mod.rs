//! Credit-reasonableness checks.
//!
//! These checks flag leverage, coverage, free-cash-flow, and liquidity metrics
//! that fall outside configurable warning / error bands.

mod coverage;
mod fcf_sign;
mod leverage;
mod liquidity;
mod trend;

pub use coverage::CoverageFloorCheck;
pub use fcf_sign::FcfSignCheck;
pub use leverage::LeverageRangeCheck;
pub use liquidity::LiquidityRunwayCheck;
pub use trend::{TrendCheck, TrendDirection};

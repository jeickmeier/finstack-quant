//! Credit-reasonableness checks.
//!
//! These checks flag leverage, coverage, free-cash-flow, and liquidity metrics
//! that fall outside configurable warning / error bands.

pub(crate) mod coverage;
pub(crate) mod fcf_sign;
pub(crate) mod leverage;
pub(crate) mod liquidity;
pub(crate) mod trend;

pub(crate) use coverage::CoverageFloorCheck;
pub(crate) use fcf_sign::FcfSignCheck;
pub(crate) use leverage::LeverageRangeCheck;
pub(crate) use liquidity::LiquidityRunwayCheck;
pub(crate) use trend::{TrendCheck, TrendDirection};

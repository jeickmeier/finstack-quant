//! Common parameter types organized by purpose.
//!
//! This module provides shared parameter types used across multiple instruments:
//! - **underlying**: Underlying asset parameters (FX, equity, index)
//! - **market**: Market-specific parameters for options and derivatives
//! - **conventions**: Standard market conventions for bonds and swaps
//! - **bermudan_call**: Issuer call provision shared by callable rate exotics

/// Error for a swap float leg set to `simple_average`, which the swap
/// projectors do not implement (they compound or take one term forward).
pub fn simple_average_unsupported() -> finstack_quant_core::Error {
    finstack_quant_core::Error::Validation(
        "float_leg.compounding = simple_average is not supported on swap legs; \
         use simple or a compounded_* variant"
            .to_string(),
    )
}

pub(crate) mod bermudan_call;
pub(crate) mod bond_quote;
pub(crate) mod conventions;
pub(crate) mod convertible;
pub(crate) mod market;
pub(crate) mod merton_mc;
pub(crate) mod monitoring;
pub(crate) mod quanto;
pub(crate) mod trs_common;
pub(crate) mod underlying;
pub(crate) mod volatility;

pub use conventions::CommodityConvention;

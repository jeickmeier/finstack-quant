//! Common parameter types organized by purpose.
//!
//! This module provides shared parameter types used across multiple instruments:
//! - **underlying**: Underlying asset parameters (FX, equity, index)
//! - **legs**: Leg specifications for swaps and structured products
//! - **market**: Market-specific parameters for options and derivatives
//! - **conventions**: Standard market conventions for bonds and swaps
//! - **bermudan_call**: Issuer call provision shared by callable rate exotics

pub(crate) mod bermudan_call;
pub(crate) mod bond_quote;
pub(crate) mod conventions;
pub(crate) mod convertible;
pub mod legs;
pub(crate) mod market;
pub(crate) mod merton_mc;
pub(crate) mod monitoring;
pub(crate) mod quanto;
pub(crate) mod trs_common;
pub(crate) mod underlying;
pub(crate) mod volatility;

pub use conventions::CommodityConvention;

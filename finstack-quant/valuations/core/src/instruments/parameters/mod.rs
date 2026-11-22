//! Common parameter types organized by purpose.
//!
//! This module provides shared parameter types used across multiple instruments:
//! - **underlying**: Underlying asset parameters (FX, equity, index)
//! - **market**: Market-specific parameters for options and derivatives
//! - **conventions**: Standard market conventions for bonds and swaps
//! - **bermudan_call**: Issuer call provision shared by callable rate exotics

pub(crate) mod conventions;
pub(crate) mod market;
pub(crate) mod monitoring;
pub(crate) mod quanto;
pub(crate) mod trs_common;

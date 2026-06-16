//! Embedded JSON convention loaders.

/// Bond convention loader.
pub(crate) mod bond;
/// CDS convention loader.
pub(crate) mod cds;
/// FX convention loader.
pub(crate) mod fx;
/// FX option convention loader.
pub(crate) mod fx_option;
/// Inflation Swap convention loader.
pub(crate) mod inflation_swap;
/// Interest Rate Future convention loader.
pub(crate) mod ir_future;
/// Generic JSON loader.
pub(crate) mod json;
/// Option convention loader.
pub(crate) mod option;
/// Rate index convention loader.
pub(crate) mod rate_index;
/// Swaption convention loader.
pub(crate) mod swaption;
/// Cross-currency swap convention loader.
pub(crate) mod xccy;

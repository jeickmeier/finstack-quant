//! Embedded JSON convention loaders.
/// CDS convention loader.
pub(crate) mod cds;
/// Inflation Swap convention loader.
pub(crate) mod inflation_swap;
/// Interest Rate Future convention loader.
pub(crate) mod ir_future;
/// Generic JSON loader.
pub(crate) mod json;
/// Rate index convention loader.
pub(crate) mod rate_index;
/// Swaption convention loader.
pub(crate) mod swaption;
/// Cross-currency swap convention loader.
pub(crate) mod xccy;

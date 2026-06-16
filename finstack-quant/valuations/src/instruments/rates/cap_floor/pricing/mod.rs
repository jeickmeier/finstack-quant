//! Interest rate option pricing components.
//!
//! Exposes the pricing entrypoints for `CapFloor`. Core pricing
//! logic is now in the instrument struct itself. This module provides
//! Black model helpers and registry integration.

pub(crate) mod black;
pub(crate) mod normal;
pub(crate) mod payoff;
/// Cap/floor pricer implementation
pub(crate) mod pricer;

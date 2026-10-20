//! Interest rate option pricing components.
//!
//! Black model helpers and registry integration for `CapFloor`; the core
//! pricing logic lives on the instrument struct.

pub(crate) mod black;
pub(crate) mod normal;
pub(crate) mod payoff;
/// Cap/floor pricer implementation
pub(crate) mod pricer;
pub(crate) mod projection;

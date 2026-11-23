//! Interest rate option pricing components.
//!
//! Black and normal caplet/floorlet kernels (`black`, `normal`), forward
//! projection (`projection`), and the registered `CapFloor` pricer (`pricer`).
//! [`CapletFloorletInputs`] is the per-period payoff input they share.

pub(crate) mod black;
mod inputs;
pub(crate) mod normal;
/// Cap/floor pricer implementation
pub(crate) mod pricer;
pub(crate) mod projection;

pub(crate) use inputs::CapletFloorletInputs;

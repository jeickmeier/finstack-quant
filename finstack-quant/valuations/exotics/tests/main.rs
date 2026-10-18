//! Integration tests for `finstack-quant-valuations-exotics`: one target so the crate links once.

#[path = "barrier_option/mod.rs"]
mod barrier_option;
#[path = "commodity/mod.rs"]
mod commodity;
#[path = "commodity_spot_option/mod.rs"]
mod commodity_spot_option;
#[path = "exotic_harness/mod.rs"]
mod exotic_harness;
#[path = "lookback_option/mod.rs"]
mod lookback_option;

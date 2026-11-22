//! Integration tests for `finstack-quant-valuations-exotics`: one target so the crate links once.

#[path = "barrier_option/mod.rs"]
mod barrier_option;
#[path = "commodity/mod.rs"]
mod commodity;
#[path = "commodity_asian_option/mod.rs"]
mod commodity_asian_option;
#[path = "commodity_spot_option/mod.rs"]
mod commodity_spot_option;
#[path = "commodity_spread_option/mod.rs"]
mod commodity_spread_option;
#[path = "commodity_swaption/mod.rs"]
mod commodity_swaption;
#[path = "exotic_harness/mod.rs"]
mod exotic_harness;
#[path = "lookback_option/mod.rs"]
mod lookback_option;
mod test_asian_cv_notional;
mod test_cross_impl_parity;

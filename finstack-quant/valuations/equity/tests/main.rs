//! Integration tests for `finstack-quant-valuations-equity`: one target so the crate links once.

#[path = "autocallable/mod.rs"]
mod autocallable;
#[path = "equity/mod.rs"]
mod equity;
#[path = "private_market_fund/mod.rs"]
mod private_market_fund;
#[path = "variance_swap/mod.rs"]
mod variance_swap;

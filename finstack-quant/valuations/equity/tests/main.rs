//! Integration tests for `finstack-quant-valuations-equity`: one target so the crate links once.

mod support;

#[path = "autocallable/mod.rs"]
mod autocallable;
#[path = "equity/mod.rs"]
mod equity;
#[path = "equity_option/mod.rs"]
mod equity_option;
#[path = "equity_trs/mod.rs"]
mod equity_trs;
#[path = "private_markets_fund/mod.rs"]
mod private_markets_fund;
#[path = "variance_swap/mod.rs"]
mod variance_swap;

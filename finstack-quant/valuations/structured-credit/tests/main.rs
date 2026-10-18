//! Integration tests for `finstack-quant-valuations-structured-credit`: one target so the crate links once.

mod support;

#[path = "asset_backed_facility/mod.rs"]
mod asset_backed_facility;
#[path = "structured_credit/mod.rs"]
mod structured_credit;

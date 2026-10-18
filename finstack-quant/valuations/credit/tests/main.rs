//! Integration tests for `finstack-quant-valuations-credit`: one target so the crate links once.

#[path = "cds/mod.rs"]
mod cds;
#[path = "cds_index/mod.rs"]
mod cds_index;
#[path = "cds_option/mod.rs"]
mod cds_option;
#[path = "cds_tranche/mod.rs"]
mod cds_tranche;

//! Integration tests for `finstack-quant-valuations-fx`: one target so the crate links once.

#[path = "fx_barrier_option/mod.rs"]
mod fx_barrier_option;
#[path = "fx_forward/mod.rs"]
mod fx_forward;
#[path = "fx_option/mod.rs"]
mod fx_option;
#[path = "fx_swap/mod.rs"]
mod fx_swap;
#[path = "ndf/mod.rs"]
mod ndf;
#[path = "quanto_option/mod.rs"]
mod quanto_option;

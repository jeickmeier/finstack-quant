//! Integration tests for `finstack-quant-valuations-fx`: one target so the crate links once.

mod support;

#[path = "fx_barrier_option/mod.rs"]
mod fx_barrier_option;
#[path = "fx_forward/mod.rs"]
mod fx_forward;
#[path = "fx_option/mod.rs"]
mod fx_option;
mod fx_settlement;
#[path = "fx_spot/mod.rs"]
mod fx_spot;
#[path = "fx_swap/mod.rs"]
mod fx_swap;
#[path = "fx_variance_swap/mod.rs"]
mod fx_variance_swap;
#[path = "ndf/mod.rs"]
mod ndf;
#[path = "quanto_option/mod.rs"]
mod quanto_option;

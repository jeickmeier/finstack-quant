//! Integration tests for `finstack-quant-valuations-rates`: one target so the crate links once.

#[path = "support/mod.rs"]
#[allow(dead_code, unused_imports)]
pub(crate) mod support;

#[path = "basis_swap/mod.rs"]
mod basis_swap;
#[path = "cap_floor/mod.rs"]
mod cap_floor;
#[path = "cms_option/mod.rs"]
mod cms_option;
#[path = "fra/mod.rs"]
mod fra;
#[path = "inflation_cap_floor/mod.rs"]
mod inflation_cap_floor;
#[path = "inflation_swap/mod.rs"]
mod inflation_swap;
#[path = "ir_future/mod.rs"]
mod ir_future;
#[path = "irs/mod.rs"]
mod irs;
#[path = "repo/mod.rs"]
mod repo;
#[path = "xccy_swap/mod.rs"]
mod xccy_swap;

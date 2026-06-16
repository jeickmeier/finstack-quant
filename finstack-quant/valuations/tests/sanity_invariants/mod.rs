//! Sanity / invariant test modules.
//!
//! Each module verifies internal properties of pricing for a specific instrument
//! class -- par-rate self-consistency, pay/receive symmetry, DV01 magnitude.
//! NOT external-reference parity (see `tests/golden/` for that).

mod test_asian_cv_notional;
mod test_bond_pricing_parity;
mod test_cds_parity;
mod test_cross_impl_parity;
mod test_equity_option_parity;
mod test_fx_option_parity;
mod test_irs_parity;
mod test_quantlib_external_parity;

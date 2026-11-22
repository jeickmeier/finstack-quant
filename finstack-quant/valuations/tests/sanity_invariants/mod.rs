//! Sanity / invariant test modules.
//!
//! Each module verifies internal properties of pricing for a specific instrument
//! class -- par-rate self-consistency, pay/receive symmetry, DV01 magnitude.
//! NOT external-reference parity (see `tests/golden/` for that).

mod test_cds_parity;
mod test_quantlib_external_parity;

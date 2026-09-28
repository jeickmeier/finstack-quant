//! Sanity / invariant tests -- verify internal consistency of pricing.
//!
//! These tests do NOT compare against external references (QuantLib, Bloomberg).
//! They assert internal properties: par-rate self-consistency, pay/receive symmetry,
//! DV01 magnitude bands. For external-reference parity, see `tests/golden/`.

// Loaded once by `instruments::test_support`.
#[allow(unused_imports)]
pub(crate) use crate::instruments::test_support::credit as credit_support;
#[allow(unused_imports)]
pub(crate) use crate::instruments::test_support::rates as rates_support;

#[path = "sanity_invariants/mod.rs"]
mod sanity_invariants_tests;

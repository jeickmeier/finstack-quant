//! Return-floor verification metrics: MOIC and XIRR (to-maturity and to-worst).
//!
//! These metrics measure the investor's realized or projected return from
//! the **issuer's perspective**: the initial investment `V0` is derived
//! from the bond's issue price and notional, and all subsequent cashflows
//! received by the holder after the issue date count as inflows.
//!
//! # Metrics
//!
//! - [`moic::MoicCalculator`] — money multiple if held to maturity.
//! - [`moic::MoicToWorstCalculator`] — minimum money multiple across **all**
//!   exits: every call/put candidate AND the held-to-maturity path.
//! - [`xirr::XirrCalculator`] — annualized IRR (Act/365F) to maturity.
//! - [`xirr::XirrToWorstCalculator`] — minimum XIRR across **all** exits:
//!   every call/put candidate AND the held-to-maturity path.
//!
//! # Floor scope
//!
//! The return floor is **call-protection only** — it bounds early-redemption
//! returns, NOT the held-to-maturity path. The `*ToWorst` metrics include the
//! unfloored maturity path in their minimum, so they are **not** bounded below
//! by the floor target. When the bond's natural maturity return is below the
//! target, the maturity path is the worst case. The floor guarantee (every
//! early-call path meets the target) is verified by the
//! `xirr_floor_meets_target_at_each_call` test in
//! `bond/pricing/return_floor.rs`.

pub(crate) mod moic;
pub(crate) mod xirr;

//! Crate-private quote-to-instrument construction.
//!
//! Calibration targets use these builders to resolve conventions, accrual
//! dates, and instrument parameters. The module is not part of the public
//! crate surface.

/// Builders for credit instruments (CDS).
pub(crate) mod cds;
/// Builders for CDS Tranche instruments.
pub(crate) mod cds_tranche;
/// Context for building instruments.
pub(crate) mod context;
/// Shared helper functions for builders.
pub(crate) mod helpers;
/// Envelope for prepared quotes.
pub(crate) mod prepared;
/// Builders for rates instruments.
pub(crate) mod rates;
/// Builders for cross-currency swap instruments.
pub(crate) mod xccy;

pub(crate) use context::BuildCtx;

#[cfg(test)]
#[path = "tests/credit.rs"]
mod tests_credit;
#[cfg(test)]
#[path = "tests/quote_construction.rs"]
mod tests_quote_construction;
#[cfg(test)]
#[path = "tests/rates.rs"]
mod tests_rates;

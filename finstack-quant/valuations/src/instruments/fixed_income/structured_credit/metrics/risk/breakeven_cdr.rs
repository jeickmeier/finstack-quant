//! Break-even CDR (break-even constant default rate) for structured-credit
//! tranches.
//!
//! The break-even CDR is the smallest constant annual default rate at which a
//! tranche first takes a principal writedown, holding the deal's prepayment and
//! recovery assumptions fixed. It is the standard credit-underwriting measure of
//! how much collateral default a tranche can absorb before incurring a loss.
//!
//! Tranche writedown is monotonic non-decreasing in CDR, so the crossing is
//! located by bisection on a single deterministic re-projection of the tranche
//! cashflows per CDR.

use crate::cashflow::builder::DefaultModelSpec;
use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;

/// Upper bound of the CDR search (100% annual default rate).
const MAX_CDR: f64 = 1.0;

/// Bisection tolerance on the CDR axis (1e-5 = 0.1 bp of CDR).
const CDR_TOL: f64 = 1e-5;

/// Writedown (currency units) treated as the first dollar of principal loss.
const WRITEDOWN_EPS: f64 = 1.0;

/// Calculate the break-even CDR for a tranche.
///
/// # Arguments
///
/// * `deal` - The structured-credit deal owning the tranche.
/// * `tranche_id` - Identifier of the tranche to solve for.
/// * `context` - Market context for cashflow projection.
/// * `as_of` - Valuation date.
///
/// # Returns
///
/// The break-even constant CDR in decimal (e.g. `0.167` = 16.7%). Returns
/// [`MAX_CDR`] when the tranche is loss-remote across the whole search range,
/// and `0.0` when it is already impaired at zero defaults.
///
/// # Errors
///
/// Returns an error if the tranche cashflows cannot be projected for a probed
/// CDR.
pub fn calculate_tranche_breakeven_cdr(
    deal: &StructuredCredit,
    tranche_id: &str,
    context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    let writedown = |cdr: f64| -> Result<f64> {
        let mut bumped = deal.clone();
        bumped.credit_model.default_spec = DefaultModelSpec::constant_cdr(cdr);
        Ok(bumped
            .get_tranche_cashflows(tranche_id, context, as_of)?
            .total_writedown
            .amount())
    };

    // Loss-remote within the search range: report the upper bound.
    if writedown(MAX_CDR)? <= WRITEDOWN_EPS {
        return Ok(MAX_CDR);
    }
    // Already impaired at zero defaults (e.g. pre-existing losses).
    if writedown(0.0)? > WRITEDOWN_EPS {
        return Ok(0.0);
    }

    // Writedown is monotonic non-decreasing in CDR; bisect the crossing.
    let mut lo = 0.0_f64;
    let mut hi = MAX_CDR;
    while hi - lo > CDR_TOL {
        let mid = 0.5 * (lo + hi);
        if writedown(mid)? > WRITEDOWN_EPS {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Ok(hi)
}

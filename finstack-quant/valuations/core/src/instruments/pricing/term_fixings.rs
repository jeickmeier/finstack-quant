//! Term-index fixing helpers shared by rate-linked instruments.
//!
//! A term projection curve (`USD-SOFR-3M`, `EUR-EURIBOR-6M`) carries the index
//! tenor it projects; instruments that reset off such a curve validate the
//! tenor once and read fixings at reset dates through these two helpers.

use finstack_quant_core::Result;
use finstack_quant_core::dates::{Date, DayCountContext, Tenor};
use finstack_quant_core::market_data::term_structures::ForwardCurve;

/// Validate that a term-index curve represents the instrument's contractual
/// reset tenor.
///
/// # Arguments
///
/// * `curve` - Projection curve the instrument resets off; its `tenor()` is
///   the index tenor it was built for, in years.
/// * `tenor` - Contractual reset tenor of the instrument (for example `3M`);
///   compared with the curve tenor to within 1e-8 years.
/// * `instrument` - Instrument identifier named in the validation error.
///
/// # Errors
///
/// Returns `Error::Validation` when the curve tenor differs from `tenor`.
pub fn validate_term_curve_tenor(
    curve: &ForwardCurve,
    tenor: Tenor,
    instrument: &str,
) -> Result<()> {
    if let Some(curve_tenor) = curve.index_tenor()
        && curve_tenor != tenor
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{instrument} index_tenor {tenor} does not match forward curve '{}' tenor {curve_tenor}",
            curve.id()
        )));
    }
    Ok(())
}

/// Project a contractual term index rate over `[start, end)`.
///
/// Uses [`ForwardCurve::rate_between_dates`] with the supplied contractual
/// `index_accrual` year fraction.
///
/// # Arguments
///
/// * `curve` - Forward curve supplying normalized projection factors.
/// * `start` - Start date of the projected index period.
/// * `end` - End date of the projected index period (`end > start`).
/// * `index_accrual` - Positive contractual accrual year fraction for annualizing the growth factor.
///
/// # Errors
///
/// Returns an error when the period is non-positive, invalid, or falls outside
/// the forward curve domain.
pub fn term_fixing_for_period(
    curve: &ForwardCurve,
    start: Date,
    end: Date,
    index_accrual: f64,
) -> Result<f64> {
    let _ = DayCountContext::default();
    curve.rate_between_dates(start, end, index_accrual)
}

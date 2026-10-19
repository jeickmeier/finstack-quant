//! Term-index fixing helpers shared by rate-linked instruments.
//!
//! A term projection curve (`USD-SOFR-3M`, `EUR-EURIBOR-6M`) carries the index
//! tenor it projects; instruments that reset off such a curve validate the
//! tenor once and read fixings at reset dates through these two helpers.

use finstack_quant_core::dates::{Date, DayCountContext, Tenor};
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::Result;

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
    let expected = tenor.to_years();
    if (curve.tenor() - expected).abs() > 1e-8 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{instrument} index_tenor {expected} does not match forward curve '{}' tenor {}",
            curve.id(),
            curve.tenor()
        )));
    }
    Ok(())
}

/// Fixed-tenor index fixing at a reset date.
///
/// Converts `date` to curve time with the curve's own day count (negative
/// before the curve base date) and returns the curve's simple forward rate
/// there, as a decimal.
///
/// # Arguments
///
/// * `curve` - Term projection curve to read the fixing from.
/// * `date` - Reset date of the fixing.
///
/// # Errors
///
/// Returns an error when the curve day count cannot measure `date` against
/// the curve base date.
pub fn term_fixing_on_date(curve: &ForwardCurve, date: Date) -> Result<f64> {
    let t = curve.day_count().signed_year_fraction(
        curve.base_date(),
        date,
        DayCountContext::default(),
    )?;
    Ok(curve.rate(t))
}

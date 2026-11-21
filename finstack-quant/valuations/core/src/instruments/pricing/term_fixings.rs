//! Term-index tenor validation shared by rate-linked instruments.
//!
//! A term projection curve (`USD-SOFR-3M`, `EUR-EURIBOR-6M`) carries the index
//! tenor it projects; instruments that reset off such a curve validate the
//! tenor once through this helper before projecting forward rates via
//! [`ForwardCurve::rate_between_dates`].

use finstack_quant_core::Result;
use finstack_quant_core::dates::Tenor;
use finstack_quant_core::market_data::term_structures::ForwardCurve;

/// Validate that a term-index curve represents the instrument's contractual
/// reset tenor.
///
/// # Arguments
///
/// * `curve` - Projection curve the instrument resets off; its `index_tenor()` is
///   compared exactly against `tenor` when present.
/// * `tenor` - Contractual reset tenor of the instrument (for example `3M`).
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

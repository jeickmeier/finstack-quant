//! Canonical market tenor conversion for swaption underlying swaps.

use finstack_quant_core::dates::Date;

/// Convert effective swap dates to the canonical market tenor coordinate.
///
/// Swaption cubes are rectangular grids labeled by contractual month/year
/// tenors (for example, 2Y, 5Y, 10Y), not by day-count year fractions. Business
/// day adjustment and settlement lag can shorten or lengthen the actual accrual
/// interval by a few days without changing that market tenor label.
///
/// # Arguments
///
/// * `start` - Contractual effective date of the underlying swap.
/// * `end` - Contractual maturity date of the underlying swap.
///
/// # Errors
///
/// Returns a validation error when `end` is not after `start` or the dates do
/// not define a positive whole-month tenor.
pub fn contractual_swap_tenor_years(start: Date, end: Date) -> finstack_quant_core::Result<f64> {
    if end <= start {
        return Err(finstack_quant_core::Error::Validation(format!(
            "swaption underlying maturity {end} must be after effective start {start}"
        )));
    }
    let months = (end.year() - start.year()) * 12 + i32::from(end.month() as u8)
        - i32::from(start.month() as u8);
    if months <= 0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "swaption underlying dates {start} to {end} do not define a positive monthly tenor"
        )));
    }
    Ok(f64::from(months) / 12.0)
}

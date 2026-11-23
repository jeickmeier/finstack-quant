//! Per-month rate-vector indexing and unit-interval validation helpers.

/// Read a per-month vector at a seasoning month: month 0 and month 1 read
/// the first entry, months past the end hold the last one.
///
/// # Errors
///
/// Returns `Error::Validation` when the vector is empty or the entry is not
/// finite and non-negative; the caller names the field.
pub(super) fn vector_at(
    values: &[f64],
    seasoning_months: u32,
    name: &str,
) -> finstack_quant_core::Result<f64> {
    let Some(last) = values.len().checked_sub(1) else {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{name} must hold at least one value"
        )));
    };
    let index = (seasoning_months.saturating_sub(1) as usize).min(last);
    let value = values[index];
    if !value.is_finite() || value < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{name}[{index}] ({value}) must be finite and non-negative"
        )));
    }
    Ok(value)
}

/// Reject any per-month vector entry that is not a finite decimal in `[0, 1]`.
///
/// # Errors
///
/// Returns `Error::Validation` naming `name[index]` for the first offending entry.
pub(super) fn check_unit_vector(values: &[f64], name: &str) -> finstack_quant_core::Result<()> {
    for (index, value) in values.iter().enumerate() {
        if !value.is_finite() || !(0.0..=1.0).contains(value) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{name}[{index}] ({value}) must be a decimal in [0, 1]"
            )));
        }
    }
    Ok(())
}

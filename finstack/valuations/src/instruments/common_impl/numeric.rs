//! Shared numeric conversion helpers.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// Convert a decimal value to `f64` with an explicit validation error.
#[inline]
pub(crate) fn decimal_to_f64(value: Decimal, field_name: &str) -> finstack_core::Result<f64> {
    value.to_f64().ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "{} value {} cannot be converted to f64",
            field_name, value
        ))
    })
}

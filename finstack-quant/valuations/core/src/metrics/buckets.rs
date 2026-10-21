//! Standard risk-bucket grid and the bucket/key-rate label helpers used by
//! DV01, CS01 and key-rate sensitivities.

/// Standard risk bucket grid in years used for IR DV01 and credit CS01.
pub const STANDARD_BUCKETS_YEARS: [f64; 11] =
    [0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 15.0, 20.0, 30.0];

/// Standard bucket labels corresponding to [`STANDARD_BUCKETS_YEARS`].
pub const STANDARD_BUCKET_LABELS: [&str; 11] = [
    "3m", "6m", "1y", "2y", "3y", "5y", "7y", "10y", "15y", "20y", "30y",
];

/// Format a bucket time (in years) as a human-readable label.
///
/// For standard buckets (0.25, 0.5, 1, 2, 3, 5, 7, 10, 15, 20, 30 years),
/// returns the canonical label ("3m", "6m", "1y", etc.).
/// For non-standard buckets, dynamically formats as "{N}m" or "{N}y".
///
/// # Arguments
///
/// * `years` - Non-negative bucket tenor in years; canonical grid tenors use
///   stable labels and other tenors are rendered as rounded months or years.
///
/// # Examples
///
/// ```
/// use finstack_quant_valuations_core::metrics::format_bucket_label;
///
/// assert_eq!(format_bucket_label(0.25), "3m");
/// assert_eq!(format_bucket_label(1.0), "1y");
/// assert_eq!(format_bucket_label(10.0), "10y");
/// assert_eq!(format_bucket_label(0.75), "9m"); // non-standard
/// ```
#[inline]
pub fn format_bucket_label(years: f64) -> String {
    format_bucket_label_cow(years).into_owned()
}

/// Same as [`format_bucket_label`] but returns `Cow<'static, str>` to avoid
/// allocation for the 11 standard buckets (the common case in DV01/CS01 loops).
///
/// # Arguments
///
/// * `years` - Non-negative bucket tenor in years. A tenor within 0.01 years of a
///   standard grid point borrows its static label; any other tenor is rendered
///   as rounded months below one year and rounded years from one year up.
#[inline]
pub fn format_bucket_label_cow(years: f64) -> std::borrow::Cow<'static, str> {
    for (i, &bucket_time) in STANDARD_BUCKETS_YEARS.iter().enumerate() {
        if (years - bucket_time).abs() < 0.01 {
            return std::borrow::Cow::Borrowed(STANDARD_BUCKET_LABELS[i]);
        }
    }

    let s = if years < 1.0 {
        format!("{:.0}m", (years * 12.0).round())
    } else {
        format!("{:.0}y", years)
    };
    std::borrow::Cow::Owned(s)
}

/// Format a key-rate coordinate without rounding away its year fraction.
///
/// # Arguments
///
/// * `years` - Nonnegative finite bucket time in years. Standard coordinates
///   use their usual month/year names; other coordinates retain their exact
///   decimal year fraction.
pub fn format_key_rate_label(years: f64) -> String {
    let label = format_bucket_label(years);
    if parse_key_rate_label(&label).ok() == Some(years) {
        label
    } else {
        format!("{years}y")
    }
}

/// Recover the year coordinate from a key-rate sensitivity bucket label.
///
/// # Arguments
///
/// * `label` - Bucket label emitted by [`format_key_rate_label`], using years
///   (`"1.4y"`) or a standard month coordinate (`"3m"`).
///
/// # Errors
///
/// Returns a validation error for an unsupported unit, negative or non-finite
/// coordinate, or an invalid numeric value.
pub fn parse_key_rate_label(label: &str) -> finstack_quant_core::Result<f64> {
    let years = if let Some(value) = label.strip_suffix('y') {
        value.parse::<f64>().ok()
    } else if let Some(value) = label.strip_suffix('m') {
        value.parse::<f64>().ok().map(|months| months / 12.0)
    } else {
        None
    };
    years
        .filter(|years| years.is_finite() && *years >= 0.0)
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "invalid sensitivity bucket label '{label}'"
            ))
        })
}

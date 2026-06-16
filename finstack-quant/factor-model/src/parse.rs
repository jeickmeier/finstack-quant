//! Label normalization and enum parsing helpers used by factor-model enums.

/// Normalize a human-entered label into snake_case for matching.
#[must_use]
pub fn normalize_label(input: &str) -> String {
    input
        .trim()
        .chars()
        .flat_map(|ch| match ch {
            '-' | '/' | ' ' => '_'.to_lowercase(),
            c => c.to_lowercase(),
        })
        .collect()
}

/// Trait for enums that can be parsed from normalized string labels.
pub trait NormalizedEnum: Sized + Copy + 'static {
    /// Mapping of normalized keys to enum variants.
    const VARIANTS: &'static [(&'static str, Self)];
}

/// Parse a human-entered string into an enum that implements [`NormalizedEnum`].
///
/// # Errors
///
/// Returns `Err(String)` when no variant key matches the normalized input.
pub fn parse_normalized_enum<T: NormalizedEnum>(input: &str) -> Result<T, String> {
    let key = normalize_label(input);
    for &(label, variant) in T::VARIANTS {
        if key == label {
            return Ok(variant);
        }
    }
    Err(format!("unknown variant '{key}'"))
}

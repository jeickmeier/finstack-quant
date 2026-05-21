//! Label normalization and enum parsing helpers for human-entered identifiers.

/// Normalize a human-entered label into snake_case for matching.
///
/// Rules:
/// - trim whitespace
/// - case-insensitive (lowercased)
/// - convert `-`, `/`, and ` ` (space) to `_`
///
/// # Examples
/// ```ignore
/// use finstack_core::parse::normalize_label;
/// assert_eq!(normalize_label("Act/365F"), "act_365f");
/// assert_eq!(normalize_label("act-act ISDA"), "act_act_isda");
/// assert_eq!(normalize_label("  Bond Basis  "), "bond_basis");
/// ```
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
///
/// Implement this trait to enable [`parse_normalized_enum`] for your enum.
/// `VARIANTS` is a flat list of `(normalized_key, variant)` pairs; a single
/// variant may appear under multiple keys (aliases).
///
/// # Example
///
/// ```ignore
/// use finstack_core::parse::{NormalizedEnum, parse_normalized_enum};
///
/// #[derive(Debug, Clone, Copy, PartialEq)]
/// enum Color { Red, Blue }
///
/// impl NormalizedEnum for Color {
///   const VARIANTS: &'static [(&'static str, Self)] = &[
///     ("red",  Color::Red),
///     ("blue", Color::Blue),
///     ("b",    Color::Blue),
///   ];
/// }
///
/// assert_eq!(parse_normalized_enum::<Color>("RED").unwrap(), Color::Red);
/// assert_eq!(parse_normalized_enum::<Color>("b").unwrap(), Color::Blue);
/// assert!(parse_normalized_enum::<Color>("green").is_err());
/// ```
pub trait NormalizedEnum: Sized + Copy + 'static {
    /// Mapping of normalized keys to enum variants.
    const VARIANTS: &'static [(&'static str, Self)];
}

/// Parse a human-entered string into an enum that implements [`NormalizedEnum`].
///
/// The input is normalized via [`normalize_label`] before matching against
/// `T::VARIANTS`. On failure, returns a `String` error with a "did you mean"
/// suggestion derived from the variant keys.
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
    let keys: Vec<&str> = T::VARIANTS.iter().map(|&(k, _)| k).collect();
    let suggestions = crate::error::fuzzy_suggestions(&key, keys.into_iter());
    let hint = crate::error::format_suggestions(&suggestions);
    Err(format!("unknown variant '{key}'{hint}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_basic() {
        assert_eq!(normalize_label("Act/365F"), "act_365f");
        assert_eq!(normalize_label("ACT-ACT"), "act_act");
        assert_eq!(normalize_label("act/act ISDA"), "act_act_isda");
        assert_eq!(normalize_label("  Bond Basis  "), "bond_basis");
        assert_eq!(normalize_label("30/360"), "30_360");
        assert_eq!(normalize_label("already_snake"), "already_snake");
    }

    #[test]
    fn test_normalize_empty() {
        assert_eq!(normalize_label(""), "");
        assert_eq!(normalize_label("   "), "");
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum TestEnum {
        Foo,
        Bar,
        Baz,
    }

    impl NormalizedEnum for TestEnum {
        const VARIANTS: &'static [(&'static str, Self)] = &[
            ("foo", TestEnum::Foo),
            ("bar", TestEnum::Bar),
            ("b", TestEnum::Bar),
            ("baz", TestEnum::Baz),
        ];
    }

    #[test]
    fn test_parse_normalized_enum_ok() {
        assert_eq!(
            parse_normalized_enum::<TestEnum>("foo").unwrap(),
            TestEnum::Foo
        );
        assert_eq!(
            parse_normalized_enum::<TestEnum>("FOO").unwrap(),
            TestEnum::Foo
        );
        assert_eq!(
            parse_normalized_enum::<TestEnum>("Bar").unwrap(),
            TestEnum::Bar
        );
        assert_eq!(
            parse_normalized_enum::<TestEnum>("b").unwrap(),
            TestEnum::Bar
        );
    }

    #[test]
    fn test_parse_normalized_enum_err_with_suggestion() {
        let err = parse_normalized_enum::<TestEnum>("fo").unwrap_err();
        assert!(err.contains("foo"), "expected suggestion 'foo' in: {err}");
    }

    #[test]
    fn test_parse_normalized_enum_err_no_match() {
        let err = parse_normalized_enum::<TestEnum>("xyz").unwrap_err();
        assert!(err.contains("xyz"), "expected input in error: {err}");
    }
}

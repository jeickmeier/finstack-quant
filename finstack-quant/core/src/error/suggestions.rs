//! Fuzzy matching utilities for error suggestions.
//!
//! This module provides edit-distance-based fuzzy matching to suggest
//! similar identifiers when users make typos in curve, calendar, or
//! metric names.
//!
//! # Algorithm
//!
//! Uses Levenshtein edit distance for fuzzy matching:
//! - Levenshtein, V. I. (1966). "Binary codes capable of correcting deletions,
//!   insertions, and reversals." *Soviet Physics Doklady*, 10(8), 707-710.

/// Format suggestions for error messages.
///
/// Returns an empty string if no suggestions, otherwise formats them
/// for inclusion in an error message.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(format_suggestions(&[]), "");
/// assert_eq!(format_suggestions(&["foo".to_string()]), ". Did you mean 'foo'?");
/// assert_eq!(
///     format_suggestions(&["foo".to_string(), "bar".to_string()]),
///     ". Did you mean one of: foo, bar?"
/// );
/// ```
pub(crate) fn format_suggestions(suggestions: &[String]) -> String {
    if suggestions.is_empty() {
        String::new()
    } else if suggestions.len() == 1 {
        format!(". Did you mean '{}'?", suggestions[0])
    } else {
        format!(". Did you mean one of: {}?", suggestions.join(", "))
    }
}

/// Find fuzzy matches for a requested identifier among available options.
///
/// Returns up to 3 suggestions based on:
/// 1. Substring containment (case-insensitive)
/// 2. Edit distance ≤ 2
///
/// # Arguments
///
/// * `requested` - The identifier the user requested
/// * `available` - Iterator of available identifiers to match against
///
/// # Returns
///
/// A vector of up to 3 matching identifiers, sorted by edit distance
/// (best match first). Returns an empty vector if no matches found.
///
/// # Complexity
///
/// - **Time**: O(k × m × n) where k = number of available identifiers,
///   m = length of `requested`, n = average length of available identifiers.
///   Each edit distance computation is O(m × n).
/// - **Space**: O(m + n) for edit distance computation plus O(k) for results.
///
/// # Examples
///
/// ```ignore
/// let available = ["USD_OIS", "EUR_OIS", "GBP_OIS"];
/// let suggestions = fuzzy_suggestions("USD_OS", available.iter().copied());
/// assert!(suggestions.contains(&"USD_OIS".to_string()));
/// ```
pub(crate) fn fuzzy_suggestions<'a>(
    requested: &str,
    available: impl Iterator<Item = &'a str>,
) -> Vec<String> {
    let requested_lower = requested.to_lowercase();
    let requested_chars: Vec<char> = requested_lower.chars().collect();

    let mut scored: Vec<(String, usize)> = available
        .filter_map(|id| {
            let id_lower = id.to_lowercase();
            let dist = edit_distance(&requested_chars, &id_lower);
            if id_lower.contains(&requested_lower)
                || requested_lower.contains(&id_lower)
                || dist <= 2
            {
                Some((id.to_string(), dist))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by_key(|&(_, dist)| dist);
    scored.truncate(3);
    scored.into_iter().map(|(s, _)| s).collect()
}

/// Simple Levenshtein edit distance for fuzzy matching.
///
/// Computes the minimum number of single-character edits (insertions,
/// deletions, substitutions) needed to transform one string into another.
///
/// # Arguments
///
/// * `a_chars` - Pre-computed character slice of the first string (lowercase)
/// * `b` - The second string to compare against
///
/// # Returns
///
/// The edit distance between the two strings (0 means identical).
///
/// # Complexity
///
/// - **Time**: O(m × n) where m = `a_chars.len()`, n = `b.chars().count()`
/// - **Space**: O(n) using two-row dynamic programming optimization
///
/// # Algorithm
///
/// Uses the classic Wagner-Fischer dynamic programming algorithm with
/// space optimization (only two rows needed instead of full matrix).
///
/// # References
///
/// - Wagner, R. A., & Fischer, M. J. (1974). "The String-to-String Correction
///   Problem." *Journal of the ACM*, 21(1), 168-173.
/// - Levenshtein, V. I. (1966). "Binary codes capable of correcting deletions,
///   insertions, and reversals." *Soviet Physics Doklady*, 10(8), 707-710.
pub(crate) fn edit_distance(a_chars: &[char], b: &str) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    let b_len = b_chars.len();
    let a_len = a_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row = vec![0; b_len + 1];

    for (i, &a_char) in a_chars.iter().enumerate() {
        curr_row[0] = i + 1;
        for (j, &b_char) in b_chars.iter().enumerate() {
            let cost = if a_char == b_char { 0 } else { 1 };
            curr_row[j + 1] = (curr_row[j] + 1)
                .min(prev_row[j + 1] + 1)
                .min(prev_row[j] + cost);
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_suggestions_empty() {
        assert_eq!(format_suggestions(&[]), "");
    }

    #[test]
    fn test_format_suggestions_single() {
        assert_eq!(
            format_suggestions(&["foo".to_string()]),
            ". Did you mean 'foo'?"
        );
    }

    #[test]
    fn test_format_suggestions_multiple() {
        assert_eq!(
            format_suggestions(&["foo".to_string(), "bar".to_string()]),
            ". Did you mean one of: foo, bar?"
        );
    }

    #[test]
    fn test_edit_distance_identical() {
        let abc: Vec<char> = "abc".chars().collect();
        assert_eq!(edit_distance(&abc, "abc"), 0);
    }

    #[test]
    fn test_edit_distance_empty() {
        let empty: Vec<char> = vec![];
        assert_eq!(edit_distance(&empty, ""), 0);
        assert_eq!(edit_distance(&empty, "abc"), 3);
    }

    #[test]
    fn test_edit_distance_substitution() {
        let abc: Vec<char> = "abc".chars().collect();
        assert_eq!(edit_distance(&abc, "abd"), 1);
    }

    #[test]
    fn test_edit_distance_deletion() {
        let abc: Vec<char> = "abc".chars().collect();
        assert_eq!(edit_distance(&abc, "ab"), 1);
    }

    #[test]
    fn test_fuzzy_suggestions_substring() {
        let available = vec!["USD_OIS", "EUR_OIS", "GBP_OIS"];
        let suggestions = fuzzy_suggestions("USD", available.into_iter());
        assert!(suggestions.contains(&"USD_OIS".to_string()));
    }

    #[test]
    fn test_fuzzy_suggestions_edit_distance() {
        let available = vec!["USD_OIS", "EUR_OIS", "GBP_OIS"];
        let suggestions = fuzzy_suggestions("USD_OS", available.into_iter());
        assert!(suggestions.contains(&"USD_OIS".to_string()));
    }

    #[test]
    fn test_fuzzy_suggestions_max_three() {
        let available = vec!["A", "AA", "AAA", "AAAA", "AAAAA"];
        let suggestions = fuzzy_suggestions("A", available.into_iter());
        assert!(suggestions.len() <= 3);
    }
}

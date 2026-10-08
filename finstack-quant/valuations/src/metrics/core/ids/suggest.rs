//! Closest-match suggestions for unknown metric names.
//!
//! An unknown-metric error that lists every standard identifier is not
//! actionable. [`closest_metric_names`] ranks candidates by a case-folded
//! similarity so the error can carry only the handful the caller most likely
//! meant (`DV01` → `dv01`, `modified_duration` → `duration_mod`,
//! `accrued_interest` → `accrued`).

/// Maximum number of suggestions attached to an unknown-metric error.
pub(crate) const MAX_METRIC_SUGGESTIONS: usize = 5;

/// Rank `candidates` by similarity to `requested` and return the best `limit`.
///
/// Matching is case-insensitive. Candidates are scored in tiers, best first:
/// exact case-folded match, prefix match in either direction, substring match
/// in either direction, then everything else by Levenshtein edit distance.
/// Ties within a tier are broken by edit distance and then by name so the
/// output is deterministic.
///
/// # Arguments
///
/// * `requested` - The metric name the caller supplied (any case).
/// * `candidates` - Known metric identifiers to rank; duplicates are kept.
/// * `limit` - Maximum number of names returned; `0` yields an empty vector.
pub(crate) fn closest_metric_names<'a>(
    requested: &str,
    candidates: impl IntoIterator<Item = &'a str>,
    limit: usize,
) -> Vec<String> {
    let wanted = requested.trim().to_lowercase();
    let wanted_chars: Vec<char> = wanted.chars().collect();

    let mut scored: Vec<(u8, usize, String)> = candidates
        .into_iter()
        .map(|candidate| {
            let folded = candidate.to_lowercase();
            let distance = edit_distance(&wanted_chars, &folded);
            let tier = if folded == wanted {
                0
            } else if folded.starts_with(&wanted) || wanted.starts_with(&folded) {
                1
            } else if folded.contains(&wanted) || wanted.contains(&folded) {
                2
            } else {
                3
            };
            (tier, distance, candidate.to_string())
        })
        .collect();

    scored.sort();
    scored.truncate(limit);
    scored.into_iter().map(|(_, _, name)| name).collect()
}

/// Levenshtein edit distance between `a` (pre-split into chars) and `b`.
fn edit_distance(a: &[char], b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(ca != cb);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANDIDATES: &[&str] = &[
        "dv01",
        "duration_mod",
        "duration_mac",
        "accrued",
        "theta",
        "cs01",
        "bucketed_dv01",
        "yield_dv01",
    ];

    #[test]
    fn case_folded_exact_match_ranks_first() {
        let out = closest_metric_names("DV01", CANDIDATES.iter().copied(), 5);
        assert_eq!(out[0], "dv01");
        assert_eq!(out.len(), 5);
    }

    #[test]
    fn prefix_and_substring_matches_beat_edit_distance() {
        let out = closest_metric_names("duration", CANDIDATES.iter().copied(), 3);
        assert_eq!(&out[..2], ["duration_mac", "duration_mod"]);
        assert_eq!(out.len(), 3);
        let out = closest_metric_names("accrued_interest", CANDIDATES.iter().copied(), 1);
        assert_eq!(out, vec!["accrued"]);
    }

    #[test]
    fn limit_and_empty_inputs_are_respected() {
        assert!(closest_metric_names("dv01", CANDIDATES.iter().copied(), 0).is_empty());
        assert!(closest_metric_names("dv01", std::iter::empty(), 5).is_empty());
        assert_eq!(edit_distance(&['a', 'b'], "abc"), 1);
        assert_eq!(edit_distance(&[], "abc"), 3);
    }
}

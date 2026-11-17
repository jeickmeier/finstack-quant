//! Override forecast method: period-keyed levels with their own carry.
//!
//! Each keyed level is carried forward until the next key. Slots before the
//! first key carry the base when there is one and have no value otherwise.

use crate::error::{Error, Result};
use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;

/// Override: period-keyed levels, each carried forward until the next key.
///
/// # Arguments
///
/// * `base_value` - Level carried through the slots before the first key, in
///   the node's own units; `None` leaves those slots out of the result, which
///   the evaluator publishes as unavailable
/// * `forecast_periods` - Periods that require projected values
/// * `overrides` - Exact levels keyed by period id text (e.g. `"2025Q2"`); every
///   period must parse to one of `forecast_periods`, and no two keys may name
///   the same period
pub(super) fn apply_override(
    base_value: Option<f64>,
    forecast_periods: &[PeriodId],
    overrides: &IndexMap<String, f64>,
) -> Result<IndexMap<PeriodId, f64>> {
    if let Some(base) = base_value
        && !base.is_finite()
    {
        return Err(Error::forecast(format!(
            "base_value must be finite, got {base}"
        )));
    }

    let mut overrides_map = IndexMap::with_capacity(overrides.len());
    for (period_str, &value) in overrides {
        if !value.is_finite() {
            return Err(Error::forecast(format!(
                "Override value for period '{}' must be finite, got {}",
                period_str, value
            )));
        }
        let parsed: PeriodId = period_str
            .parse()
            .map_err(|e| Error::forecast(format!("Invalid override period '{period_str}': {e}")))?;
        if !forecast_periods.contains(&parsed) {
            return Err(Error::forecast(format!(
                "Override period '{period_str}' is not in the forecast periods"
            )));
        }
        if overrides_map.insert(parsed, value).is_some() {
            return Err(Error::forecast(format!(
                "Duplicate override for period '{parsed}': '{period_str}' aliases another period key"
            )));
        }
    }

    let mut results = IndexMap::new();
    let mut current_value = base_value;

    for period_id in forecast_periods {
        if let Some(&override_value) = overrides_map.get(period_id) {
            current_value = Some(override_value);
        }
        if let Some(value) = current_value {
            results.insert(*period_id, value);
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::PeriodId;

    #[test]
    fn test_override_all_periods() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
        ];

        let overrides: IndexMap<String, f64> = serde_json::from_value(serde_json::json!({
            "2025Q1": 120.0,
            "2025Q2": 130.0,
        }))
        .expect("overrides");

        let results = apply_override(Some(100.0), &periods, &overrides)
            .expect("apply_override should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(
            results[&PeriodId::quarter(2025, 1).expect("valid period fixture")],
            120.0
        );
        assert_eq!(
            results[&PeriodId::quarter(2025, 2).expect("valid period fixture")],
            130.0
        );
    }

    #[test]
    fn test_override_sparse_with_forward_fill() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
            PeriodId::quarter(2025, 3).expect("valid period fixture"),
            PeriodId::quarter(2025, 4).expect("valid period fixture"),
        ];

        let overrides: IndexMap<String, f64> = serde_json::from_value(serde_json::json!({
            "2025Q1": 120.0,
            "2025Q3": 130.0,
        }))
        .expect("overrides");

        let results = apply_override(Some(100.0), &periods, &overrides)
            .expect("apply_override should succeed");

        assert_eq!(results.len(), 4);
        assert_eq!(
            results[&PeriodId::quarter(2025, 1).expect("valid period fixture")],
            120.0
        );
        assert_eq!(
            results[&PeriodId::quarter(2025, 2).expect("valid period fixture")],
            120.0
        ); // Forward fill from Q1
        assert_eq!(
            results[&PeriodId::quarter(2025, 3).expect("valid period fixture")],
            130.0
        );
        assert_eq!(
            results[&PeriodId::quarter(2025, 4).expect("valid period fixture")],
            130.0
        ); // Forward fill from Q3
    }

    #[test]
    fn test_override_no_overrides_forward_fill_base() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
        ];

        let overrides: IndexMap<String, f64> =
            serde_json::from_value(serde_json::json!({})).expect("overrides");

        let results = apply_override(Some(100.0), &periods, &overrides)
            .expect("apply_override should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(
            results[&PeriodId::quarter(2025, 1).expect("valid period fixture")],
            100.0
        );
        assert_eq!(
            results[&PeriodId::quarter(2025, 2).expect("valid period fixture")],
            100.0
        );
    }

    #[test]
    fn test_override_rejects_unmatched_period() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];

        let overrides: IndexMap<String, f64> = serde_json::from_value(serde_json::json!({
            "2025Q2": 120.0,
        }))
        .expect("overrides");

        let err = apply_override(Some(100.0), &periods, &overrides)
            .expect_err("unmatched override period must be rejected");
        assert!(err.to_string().contains("not in the forecast periods"));
    }

    #[test]
    fn slots_before_the_first_key_have_no_value_without_a_base() {
        let periods: Vec<PeriodId> = (1..=4)
            .map(|q| PeriodId::quarter(2025, q).expect("valid period fixture"))
            .collect();
        let overrides: IndexMap<String, f64> =
            serde_json::from_value(serde_json::json!({ "2025Q3": 130.0 })).expect("overrides");

        let results = apply_override(None, &periods, &overrides).expect("override");

        assert_eq!(
            results.into_iter().collect::<Vec<_>>(),
            vec![(periods[2], 130.0), (periods[3], 130.0)]
        );
    }
}

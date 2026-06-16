//! Override forecast method for injecting explicit period-level values.
//!
//! The override method lets callers provide sparse adjustments that supersede
//! previously calculated projections. Any periods without an explicit override
//! will inherit the most recent value (forward fill).

use crate::error::{Error, Result};
use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;

/// Override: use explicit period-specific values, forward fill for the rest.
///
/// # Parameters
///
/// * `overrides` - Map of period_id string → value
pub(super) fn apply_override(
    base_value: f64,
    forecast_periods: &[PeriodId],
    params: &IndexMap<String, serde_json::Value>,
) -> Result<IndexMap<PeriodId, f64>> {
    // Extract overrides parameter
    let overrides_json = params.get("overrides").ok_or_else(|| {
        Error::forecast(
            "Missing 'overrides' parameter for Override method. \
             Expected a JSON object mapping period IDs to values (e.g., {\"2025Q2\": 120000}).",
        )
    })?;

    if !base_value.is_finite() {
        return Err(Error::forecast(format!(
            "base_value must be finite, got {}",
            base_value
        )));
    }

    let overrides_map: IndexMap<String, f64> = serde_json::from_value(overrides_json.clone())
        .map_err(|e| {
            Error::forecast(format!(
                "Invalid 'overrides' parameter: expected map of period_id → value. Error: {}",
                e
            ))
        })?;

    for (period_str, value) in &overrides_map {
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
    }

    let mut results = IndexMap::new();
    let mut current_value = base_value;

    for period_id in forecast_periods {
        // Check if there's an override for this period
        let period_str = period_id.to_string();
        if let Some(&override_value) = overrides_map.get(&period_str) {
            current_value = override_value;
        }
        // Otherwise forward fill current_value

        results.insert(*period_id, current_value);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::PeriodId;

    #[test]
    fn test_override_all_periods() {
        let periods = vec![PeriodId::quarter(2025, 1), PeriodId::quarter(2025, 2)];

        let mut params = IndexMap::new();
        let overrides = serde_json::json!({
            "2025Q1": 120.0,
            "2025Q2": 130.0,
        });
        params.insert("overrides".to_string(), overrides);

        let results =
            apply_override(100.0, &periods, &params).expect("apply_override should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[&PeriodId::quarter(2025, 1)], 120.0);
        assert_eq!(results[&PeriodId::quarter(2025, 2)], 130.0);
    }

    #[test]
    fn test_override_sparse_with_forward_fill() {
        let periods = vec![
            PeriodId::quarter(2025, 1),
            PeriodId::quarter(2025, 2),
            PeriodId::quarter(2025, 3),
            PeriodId::quarter(2025, 4),
        ];

        let mut params = IndexMap::new();
        let overrides = serde_json::json!({
            "2025Q1": 120.0,
            "2025Q3": 130.0,
        });
        params.insert("overrides".to_string(), overrides);

        let results =
            apply_override(100.0, &periods, &params).expect("apply_override should succeed");

        assert_eq!(results.len(), 4);
        assert_eq!(results[&PeriodId::quarter(2025, 1)], 120.0);
        assert_eq!(results[&PeriodId::quarter(2025, 2)], 120.0); // Forward fill from Q1
        assert_eq!(results[&PeriodId::quarter(2025, 3)], 130.0);
        assert_eq!(results[&PeriodId::quarter(2025, 4)], 130.0); // Forward fill from Q3
    }

    #[test]
    fn test_override_no_overrides_forward_fill_base() {
        let periods = vec![PeriodId::quarter(2025, 1), PeriodId::quarter(2025, 2)];

        let mut params = IndexMap::new();
        let overrides = serde_json::json!({});
        params.insert("overrides".to_string(), overrides);

        let results =
            apply_override(100.0, &periods, &params).expect("apply_override should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[&PeriodId::quarter(2025, 1)], 100.0);
        assert_eq!(results[&PeriodId::quarter(2025, 2)], 100.0);
    }

    #[test]
    fn test_override_missing_parameter_error() {
        let periods = vec![PeriodId::quarter(2025, 1)];
        let params = IndexMap::new();

        let result = apply_override(100.0, &periods, &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_override_rejects_unmatched_period() {
        let periods = vec![PeriodId::quarter(2025, 1)];

        let mut params = IndexMap::new();
        let overrides = serde_json::json!({
            "2025Q2": 120.0,
        });
        params.insert("overrides".to_string(), overrides);

        let err = apply_override(100.0, &periods, &params)
            .expect_err("unmatched override period must be rejected");
        assert!(err.to_string().contains("not in the forecast periods"));
    }
}

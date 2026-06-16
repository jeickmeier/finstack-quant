//! Deterministic forecast algorithms for projecting values across periods.
//!
//! The helpers in this module operate on raw numeric series and return
//! [`IndexMap`]s keyed by [`PeriodId`]. They are building blocks used by the
//! higher-level forecast engine but can also be invoked directly in custom
//! workflows or extensions.

use crate::error::{Error, Result};
use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;

/// Forward fill: carry the last observed value into every forecast period.
///
/// # Arguments
///
/// * `base_value` - Value to repeat
/// * `forecast_periods` - Periods that require projected values
pub(super) fn forward_fill(
    base_value: f64,
    forecast_periods: &[PeriodId],
) -> Result<IndexMap<PeriodId, f64>> {
    let mut results = IndexMap::new();

    for period_id in forecast_periods {
        results.insert(*period_id, base_value);
    }

    Ok(results)
}

/// Growth percentage: apply a constant compound growth rate each period.
///
/// The recurrence relation is `v[t] = v[t-1] * (1 + rate)`, where `rate`
/// represents the fractional growth between consecutive periods.
///
/// # Parameters
///
/// * `rate` - Growth rate per period (e.g., 0.05 for 5% growth)
pub(super) fn growth_pct(
    base_value: f64,
    forecast_periods: &[PeriodId],
    params: &IndexMap<String, serde_json::Value>,
) -> Result<IndexMap<PeriodId, f64>> {
    // Extract rate parameter
    let rate = params.get("rate").and_then(|v| v.as_f64()).ok_or_else(|| {
        Error::forecast(
            "Missing or invalid 'rate' parameter for GrowthPct forecast. \
             Expected a number (e.g., 0.05 for 5% growth).",
        )
    })?;
    if !base_value.is_finite() || !rate.is_finite() {
        return Err(Error::forecast(
            "GrowthPct forecast requires finite base_value and rate",
        ));
    }

    // Warn on extreme growth rates (>100% per period)
    if rate.abs() > 1.0 {
        tracing::warn!(
            "Growth rate {:.2}% exceeds 100% per period - verify this is intentional",
            rate * 100.0
        );
    }

    let mut results = IndexMap::new();
    let mut current_value = base_value;

    for period_id in forecast_periods {
        current_value *= 1.0 + rate;

        // Check for overflow/underflow
        if !current_value.is_finite() {
            return Err(Error::forecast(format!(
                "Overflow in compound growth calculation at period {:?}. \
                 Consider using smaller growth rate or fewer periods.",
                period_id
            )));
        }

        results.insert(*period_id, current_value);
    }

    Ok(results)
}

/// Curve percentage: apply period-specific growth rates supplied as a curve.
///
/// The recurrence relation is `v[t] = v[t-1] * (1 + curve[t])`, where `curve`
/// contains the growth factor for the *t*-th forecast period.
///
/// # Parameters
///
/// * `curve` - Array of growth rates, one per forecast period
pub(super) fn curve_pct(
    base_value: f64,
    forecast_periods: &[PeriodId],
    params: &IndexMap<String, serde_json::Value>,
) -> Result<IndexMap<PeriodId, f64>> {
    // Extract curve parameter
    let curve_json = params.get("curve").ok_or_else(|| {
        Error::forecast(
            "Missing 'curve' parameter for CurvePct forecast. \
             Expected an array of growth rates (e.g., [0.05, 0.06, 0.05]).",
        )
    })?;

    let curve: Vec<f64> = serde_json::from_value(curve_json.clone()).map_err(|e| {
        Error::forecast(format!(
            "Invalid 'curve' parameter: expected array of numbers. Error: {}",
            e
        ))
    })?;
    if !base_value.is_finite() {
        return Err(Error::forecast(
            "CurvePct forecast requires a finite base_value",
        ));
    }

    if curve.len() != forecast_periods.len() {
        return Err(Error::forecast(format!(
            "Curve length ({}) does not match number of forecast periods ({}). \
             Provide exactly one growth rate per forecast period.",
            curve.len(),
            forecast_periods.len()
        )));
    }

    let mut results = IndexMap::new();
    let mut current_value = base_value;

    for (i, period_id) in forecast_periods.iter().enumerate() {
        if !curve[i].is_finite() {
            return Err(Error::forecast(format!(
                "CurvePct growth rate at index {} must be finite, got {}",
                i, curve[i]
            )));
        }
        current_value *= 1.0 + curve[i];
        if !current_value.is_finite() {
            return Err(Error::forecast(format!(
                "CurvePct forecast produced a non-finite value at period {:?}",
                period_id
            )));
        }
        results.insert(*period_id, current_value);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::PeriodId;

    #[test]
    fn test_forward_fill() {
        let periods = vec![
            PeriodId::quarter(2025, 1),
            PeriodId::quarter(2025, 2),
            PeriodId::quarter(2025, 3),
        ];

        let results = forward_fill(100.0, &periods).expect("forward_fill should succeed");

        assert_eq!(results.len(), 3);
        assert_eq!(results[&PeriodId::quarter(2025, 1)], 100.0);
        assert_eq!(results[&PeriodId::quarter(2025, 2)], 100.0);
        assert_eq!(results[&PeriodId::quarter(2025, 3)], 100.0);
    }

    #[test]
    fn test_growth_pct() {
        let periods = vec![
            PeriodId::quarter(2025, 1),
            PeriodId::quarter(2025, 2),
            PeriodId::quarter(2025, 3),
        ];

        let mut params = IndexMap::new();
        params.insert("rate".to_string(), serde_json::json!(0.05));

        let results = growth_pct(100.0, &periods, &params).expect("growth_pct should succeed");

        assert_eq!(results.len(), 3);
        assert!((results[&PeriodId::quarter(2025, 1)] - 105.0).abs() < 0.01);
        assert!((results[&PeriodId::quarter(2025, 2)] - 110.25).abs() < 0.01);
        assert!((results[&PeriodId::quarter(2025, 3)] - 115.7625).abs() < 0.01);
    }

    #[test]
    fn test_growth_pct_negative() {
        let periods = vec![PeriodId::quarter(2025, 1)];

        let mut params = IndexMap::new();
        params.insert("rate".to_string(), serde_json::json!(-0.1)); // -10% decline

        let results = growth_pct(100.0, &periods, &params).expect("growth_pct should succeed");

        assert!((results[&PeriodId::quarter(2025, 1)] - 90.0).abs() < 0.01);
    }

    #[test]
    fn test_growth_pct_missing_rate_error() {
        let periods = vec![PeriodId::quarter(2025, 1)];
        let params = IndexMap::new();

        let result = growth_pct(100.0, &periods, &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_curve_pct() {
        let periods = vec![
            PeriodId::quarter(2025, 1),
            PeriodId::quarter(2025, 2),
            PeriodId::quarter(2025, 3),
        ];

        let mut params = IndexMap::new();
        params.insert("curve".to_string(), serde_json::json!([0.05, 0.06, 0.05]));

        let results = curve_pct(100.0, &periods, &params).expect("curve_pct should succeed");

        assert_eq!(results.len(), 3);
        assert!((results[&PeriodId::quarter(2025, 1)] - 105.0).abs() < 0.01);
        assert!((results[&PeriodId::quarter(2025, 2)] - 111.3).abs() < 0.01);
        assert!((results[&PeriodId::quarter(2025, 3)] - 116.865).abs() < 0.01);
    }

    #[test]
    fn test_curve_pct_length_mismatch_error() {
        let periods = vec![
            PeriodId::quarter(2025, 1),
            PeriodId::quarter(2025, 2),
            PeriodId::quarter(2025, 3),
        ];

        let mut params = IndexMap::new();
        params.insert("curve".to_string(), serde_json::json!([0.05, 0.06])); // Too short

        let result = curve_pct(100.0, &periods, &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_curve_pct_missing_curve_error() {
        let periods = vec![PeriodId::quarter(2025, 1)];
        let params = IndexMap::new();

        let result = curve_pct(100.0, &periods, &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_curve_pct_rejects_non_finite_output() {
        let periods = vec![PeriodId::quarter(2025, 1)];

        let mut params = IndexMap::new();
        params.insert("curve".to_string(), serde_json::json!([1.0]));

        let result = curve_pct(f64::MAX, &periods, &params);
        assert!(result.is_err(), "non-finite forecasts must be rejected");
    }

    /// Overflow in compound growth must be detected and returned as an error.
    #[test]
    fn test_growth_pct_overflow_error() {
        // Use extreme growth rate (10000% per period) to trigger overflow
        // With rate=100, starting at 1e10: 1e10 * 101^150 > f64::MAX (1.8e308)
        // After 100 periods we're at ~1e210, need ~50 more to overflow
        let periods: Vec<_> = (0..200)
            .map(|i| PeriodId::quarter(2025 + i / 4, ((i % 4) as u8) + 1))
            .collect();

        let mut params = IndexMap::new();
        params.insert("rate".to_string(), serde_json::json!(100.0)); // 10000% per period

        let err = growth_pct(1e10, &periods, &params).expect_err("overflow should error");
        assert!(err.to_string().contains("Overflow"));
    }

    /// High growth rates (>100% per period) should warn but still succeed.
    #[test]
    fn test_growth_pct_high_rate_no_error() {
        let periods = vec![PeriodId::quarter(2025, 1)];

        let mut params = IndexMap::new();
        params.insert("rate".to_string(), serde_json::json!(1.5)); // 150% per period

        let result = growth_pct(100.0, &periods, &params).expect("150% growth should succeed");
        assert!((result[&PeriodId::quarter(2025, 1)] - 250.0).abs() < 0.01);
    }
}

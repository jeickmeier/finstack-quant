//! Cross-instrument metric aggregation semantics.

use super::MetricId;

/// Describes whether a metric can be scaled and summed across instruments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum MetricAggregation {
    /// The metric is linear in position quantity and currency-convertible.
    Additive,
    /// The metric has instrument-specific meaning and must remain disaggregated.
    NonAdditive,
}

/// Return the aggregation policy for a metric identifier.
///
/// Structured metric keys such as `bucketed_dv01::USD-OIS::10y` inherit the
/// policy of their base identifier.
///
/// # Arguments
///
/// * `metric_id` - Metric identifier whose cross-instrument semantics are required.
#[must_use]
pub fn metric_aggregation(metric_id: &MetricId) -> MetricAggregation {
    let id = metric_id.as_str();
    if matches!(
        id,
        "delta" | "gamma" | "vega" | "vanna" | "volga" | "bucketed_vega"
    ) {
        return MetricAggregation::NonAdditive;
    }
    if [
        "delta::",
        "gamma::",
        "vega::",
        "vanna::",
        "volga::",
        "bucketed_vega::",
    ]
    .iter()
    .any(|prefix| {
        id.strip_prefix(prefix)
            .is_some_and(|factor| !factor.is_empty())
    }) {
        return MetricAggregation::Additive;
    }

    let base = id.split_once("::").map_or(id, |(base, _)| base);
    match base {
        "theta"
        | "dv01"
        | "cs01"
        | "rho"
        | "pv01"
        | "ir01"
        | "fx01"
        | "inflation01"
        | "dividend01"
        | "ir_convexity"
        | "cs_gamma"
        | "inflation_convexity"
        | "cross_gamma_rates_credit"
        | "cross_gamma_rates_vol"
        | "cross_gamma_spot_vol"
        | "cross_gamma_spot_credit"
        | "cross_gamma_fx_vol"
        | "cross_gamma_fx_rates"
        | "cross_gamma_credit_vol"
        | "index_delta"
        | "fx_delta"
        | "foreign_rho"
        | "bucketed_dv01"
        | "bucketed_cs01"
        | "accrued_interest"
        | "pv_fixed"
        | "pv_float"
        | "pv_primary"
        | "pv_reference" => MetricAggregation::Additive,
        _ => MetricAggregation::NonAdditive,
    }
}

/// Return whether a metric may be scaled and summed across instruments.
///
/// # Arguments
///
/// * `metric_id` - Metric identifier to test.
#[must_use]
pub fn is_additive_metric(metric_id: &MetricId) -> bool {
    metric_aggregation(metric_id) == MetricAggregation::Additive
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucketed_metric_inherits_base_policy() {
        assert!(is_additive_metric(&MetricId::custom(
            "bucketed_dv01::USD-OIS::10y"
        )));
        assert!(!is_additive_metric(&MetricId::Ytm));
    }

    #[test]
    fn scalar_greeks_require_qualified_risk_factors() {
        for metric in ["delta", "gamma", "vega", "vanna", "volga", "bucketed_vega"] {
            assert!(!is_additive_metric(&MetricId::custom(metric)));
        }
        for metric in [
            "delta::AAPL",
            "gamma::SPX",
            "vega::SPX_VOL",
            "vanna::EURUSD_VOL",
            "volga::SPX_VOL",
            "bucketed_vega::SPX_VOL::1y::100",
        ] {
            assert!(is_additive_metric(&MetricId::custom(metric)));
        }
    }
}

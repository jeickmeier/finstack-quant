//! Covenant operating metric identifiers and metric lookup sources.

use finstack_quant_core::HashMap;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fmt;
use std::sync::Arc;

/// String-backed identifier for a covenant operating metric.
///
/// These identifiers are conventionally aligned with `finstack-quant-statements`
/// node IDs such as `debt_to_ebitda`, `ebitda`, `interest_coverage`, and
/// `dscr`, but this crate intentionally has no compile-time dependency on
/// `finstack-quant-statements`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[repr(transparent)]
#[serde(transparent)]
pub struct CovenantMetricId(Arc<str>);

impl CovenantMetricId {
    /// Create a covenant metric identifier from a string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(Arc::from(id.into()))
    }

    /// Return the string form of this metric identifier.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CovenantMetricId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for CovenantMetricId {
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
    }
}

impl From<String> for CovenantMetricId {
    fn from(value: String) -> Self {
        Self(Arc::from(value))
    }
}

impl From<&String> for CovenantMetricId {
    fn from(value: &String) -> Self {
        Self(Arc::from(value.as_str()))
    }
}

impl Borrow<str> for CovenantMetricId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for CovenantMetricId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Source of covenant operating metric values.
pub trait CovenantMetricSource: Send + Sync {
    /// Return the metric value for the requested covenant operating metric.
    ///
    /// # Errors
    ///
    /// Returns an error when the metric is unavailable.
    fn get_metric(&self, metric: &CovenantMetricId) -> finstack_quant_core::Result<f64>;
}

/// Map-backed metric source for tests, bindings, and simple callers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct HashMapMetricSource {
    metrics: HashMap<CovenantMetricId, f64>,
}

impl HashMapMetricSource {
    /// Create an empty map-backed metric source.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a metric source from string-keyed metrics.
    pub fn from_pairs<I, K>(metrics: I) -> Self
    where
        I: IntoIterator<Item = (K, f64)>,
        K: Into<CovenantMetricId>,
    {
        Self {
            metrics: metrics.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        }
    }

    /// Parse a JSON object mapping metric ids to numeric values.
    ///
    /// This is the one metric-map parser shared by
    /// [`evaluate_engine`](crate::evaluate_engine) and host bindings that
    /// accept a JSON metric map.
    ///
    /// # Arguments
    ///
    /// * `json` - UTF-8 JSON object whose keys are metric identifiers (for
    ///   example `debt_to_ebitda`) and whose values are JSON numbers in the
    ///   units required by the covenant tests that read them.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when `json` is not
    /// a JSON object (`Invalid metric map JSON: ...`) or a value is not a JSON
    /// number (`Metric '<id>' must be a finite JSON number`).
    pub fn from_json(json: &str) -> finstack_quant_core::Result<Self> {
        let map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(json).map_err(|e| {
                finstack_quant_core::Error::Validation(format!("Invalid metric map JSON: {e}"))
            })?;
        let pairs = map
            .into_iter()
            .map(|(key, value)| match value.as_f64() {
                Some(number) => Ok((key, number)),
                None => Err(finstack_quant_core::Error::Validation(format!(
                    "Metric '{key}' must be a finite JSON number"
                ))),
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;
        Ok(Self::from_pairs(pairs))
    }

    /// Insert or replace a metric value.
    pub fn insert(&mut self, metric: impl Into<CovenantMetricId>, value: f64) -> Option<f64> {
        self.metrics.insert(metric.into(), value)
    }
}

impl CovenantMetricSource for HashMapMetricSource {
    fn get_metric(&self, metric: &CovenantMetricId) -> finstack_quant_core::Result<f64> {
        self.metrics
            .get(metric)
            .copied()
            .ok_or_else(|| finstack_quant_core::InputError::NotFound {
                id: format!("metric:{}", metric.as_str()),
            })
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_json_reads_numeric_metric_map() {
        let source = HashMapMetricSource::from_json(r#"{"debt_to_ebitda": 3.5, "dscr": 2}"#)
            .expect("numeric map parses");
        let leverage = source
            .get_metric(&CovenantMetricId::from("debt_to_ebitda"))
            .expect("metric present");
        assert_eq!(leverage, 3.5);
    }

    #[test]
    fn from_json_rejects_non_numbers_and_non_objects() {
        let error = HashMapMetricSource::from_json(r#"{"debt_to_ebitda": true}"#)
            .expect_err("bool metric must be rejected");
        assert_eq!(
            error.to_string(),
            "Validation error: Metric 'debt_to_ebitda' must be a finite JSON number"
        );
        let error = HashMapMetricSource::from_json("[1,2]").expect_err("array must be rejected");
        assert!(error
            .to_string()
            .starts_with("Validation error: Invalid metric map JSON:"));
    }
}

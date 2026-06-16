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
pub trait CovenantMetricSource {
    /// Return the metric value for the requested covenant operating metric.
    ///
    /// # Errors
    ///
    /// Returns an error when the metric is unavailable.
    fn get_metric(&mut self, metric: &CovenantMetricId) -> finstack_quant_core::Result<f64>;
}

/// Map-backed metric source for tests, bindings, and simple callers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

    /// Insert or replace a metric value.
    pub fn insert(&mut self, metric: impl Into<CovenantMetricId>, value: f64) -> Option<f64> {
        self.metrics.insert(metric.into(), value)
    }

    /// Borrow the underlying metric map.
    pub fn metrics(&self) -> &HashMap<CovenantMetricId, f64> {
        &self.metrics
    }
}

impl CovenantMetricSource for HashMapMetricSource {
    fn get_metric(&mut self, metric: &CovenantMetricId) -> finstack_quant_core::Result<f64> {
        self.metrics
            .get(metric)
            .copied()
            .ok_or_else(|| finstack_quant_core::InputError::NotFound {
                id: format!("metric:{}", metric.as_str()),
            })
            .map_err(Into::into)
    }
}

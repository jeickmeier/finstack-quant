//! Strongly-typed metric identifiers for compile-time validation.
//!
//! Provides a comprehensive set of metric IDs covering bond, IRS, deposit,
//! and risk metrics. Each ID is strongly-typed to prevent runtime errors
//! and enable compile-time validation of metric dependencies.
//!
//! # Metric Categories
//!
//! - **Bond metrics**: Yield, duration, convexity, pricing, credit spreads
//! - **IRS metrics**: DV01, annuity factors, par rates, present values
//! - **Deposit metrics**: Discount factors, par rates, year fractions
//! - **Risk metrics**: DV01 (standard for all parallel rate sensitivity), CS01, BucketedDV01, BucketedCS01, Theta, and all standardized "01" sensitivity metrics
//! - **Standardized sensitivity metrics**: Dividend01, Inflation01, Prepayment01, Default01, Severity01, Conversion01, CollateralHaircut01, CollateralPrice01, Nav01, Carry01, Hurdle01, Dv01Domestic, Dv01Foreign, Fx01, Npv01, SpreadDv01, Correlation01, FxVega, ConvexityAdjustmentRisk
//! - **Custom metrics**: User-defined metrics with dynamic identifiers

use finstack_quant_core::HashMap;
use serde::{Deserialize, Serialize};
use std::borrow::{Borrow, Cow};
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

mod composite;
mod credit;
mod equity_volatility;
mod group;
mod instrument;
mod rates;
mod risk;
mod standard;
mod suggest;
mod unit;

pub use group::MetricGroup;
pub use unit::MetricUnit;

pub(crate) use suggest::{closest_metric_names, MAX_METRIC_SUGGESTIONS};

/// Strongly-typed metric identifier.
///
/// Provides compile-time validation, autocomplete support, and safe refactoring
/// when metric names change. Covers bond, IRS, deposit, and risk metrics.
///
/// See unit tests and `examples/` for usage.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "String")]
pub struct MetricId(Cow<'static, str>);

impl MetricId {
    /// Creates a custom metric ID.
    ///
    /// Use this for user-defined metrics that aren't part of the standard set.
    /// `id` supplies literal labels, separated by `::` for composite coordinates.
    /// Reserved escape markers and empty coordinates are encoded canonically.
    /// Use `str::parse` to read an already encoded wire key, and
    /// [`Self::composite`] when a coordinate itself contains `::`.
    ///
    /// # Arguments
    ///
    /// * `id` - Literal custom metric name, optionally followed by `::`-separated
    ///   coordinate labels. Existing `_xHH` text is treated literally.
    pub fn custom(id: impl Into<String>) -> Self {
        let id = id.into();
        let mut key = String::with_capacity(id.len());
        for (index, component) in id.split("::").enumerate() {
            if index > 0 {
                key.push_str("::");
            }
            composite::encode_component(&mut key, component);
        }
        Self(Cow::Owned(key))
    }

    /// Returns the canonical wire representation.
    ///
    /// Standard names are lowercase snake_case; custom coordinates retain case.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Checks if this is a custom (non-standard) metric.
    ///
    /// Returns `true` if the metric was created via `custom()` and is not
    /// part of the standard set.
    pub fn is_custom(&self) -> bool {
        !metric_lookup().contains_key(self.as_str())
    }

    /// Parses a string into a MetricId with strict validation.
    ///
    /// Unlike `FromStr`, this method returns an error for unknown metric names
    /// rather than creating a custom metric. Use this for user-provided inputs
    /// where typos should be caught, not silently accepted.
    ///
    /// # Errors
    ///
    /// Returns `Error::UnknownMetric` if the string does not match any standard
    /// metric. The error carries the invalid metric name and at most
    /// `MAX_METRIC_SUGGESTIONS` (5) standard metrics ranked by case-folded
    /// similarity (`"DV01"` suggests `dv01`, `"modified_duration"` suggests
    /// `duration_mod`).
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_valuations::metrics::MetricId;
    ///
    /// // Parse known metric - succeeds
    /// let dv01 = MetricId::parse_strict("dv01").unwrap();
    /// assert_eq!(dv01, MetricId::Dv01);
    ///
    /// // Unknown metric - fails with error
    /// let result = MetricId::parse_strict("dv01x");
    /// assert!(result.is_err());
    /// ```
    ///
    /// # Custom metrics via FromStr
    ///
    /// To accept custom metrics, use `FromStr::from_str`
    /// or the `.parse()` method, which validates canonical composite encoding:
    ///
    /// ```
    /// use finstack_quant_valuations::metrics::MetricId;
    /// use std::str::FromStr;
    ///
    /// // FromStr allows custom metrics
    /// let custom = MetricId::from_str("my_custom_metric").unwrap();
    /// assert!(custom.is_custom());
    ///
    /// // Strict parsing rejects unknown metrics
    /// let result = MetricId::parse_strict("my_custom_metric");
    /// assert!(result.is_err());
    /// ```
    pub fn parse_strict(s: &str) -> finstack_quant_core::Result<Self> {
        if let Some(id) = metric_lookup().get(s) {
            Ok(id.clone())
        } else {
            Err(finstack_quant_core::Error::unknown_metric(
                s,
                closest_metric_names(
                    s,
                    Self::ALL_STANDARD.iter().map(MetricId::as_str),
                    MAX_METRIC_SUGGESTIONS,
                ),
            ))
        }
    }
}

impl fmt::Display for MetricId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Borrow<str> for MetricId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

// Lazy lookup table for FromStr
static METRIC_LOOKUP: OnceLock<HashMap<String, MetricId>> = OnceLock::new();

fn metric_lookup() -> &'static HashMap<String, MetricId> {
    METRIC_LOOKUP.get_or_init(|| {
        let mut map = HashMap::default();
        map.reserve(MetricId::ALL_STANDARD.len());
        for m in MetricId::ALL_STANDARD {
            // Names are already lower snake_case
            map.insert(m.as_str().to_string(), m.clone());
        }
        map
    })
}

impl FromStr for MetricId {
    type Err = finstack_quant_core::Error;

    /// Parses a string into a MetricId (permissive mode).
    ///
    /// Unknown scalar names become custom metrics. Composite keys must use
    /// canonical escaping; obsolete or malformed encodings return an error.
    /// Standard metrics are matched by their exact snake_case identifiers.
    ///
    /// **For user-provided inputs**, prefer `MetricId::parse_strict()` which
    /// rejects unknown metrics instead of silently creating custom metrics.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_valuations::metrics::MetricId;
    /// use std::str::FromStr;
    ///
    /// // Known metric - parsed as standard
    /// let dv01 = MetricId::from_str("dv01").unwrap();
    /// assert_eq!(dv01, MetricId::Dv01);
    /// assert!(!dv01.is_custom());
    ///
    /// // Unknown scalar names become custom metrics
    /// let custom = MetricId::from_str("my_metric").unwrap();
    /// assert!(custom.is_custom());
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(id) = metric_lookup().get(s) {
            Ok(id.clone())
        } else {
            Self::validate_wire(s)?;
            Ok(Self(Cow::Owned(s.to_string())))
        }
    }
}

#[cfg(test)]
mod tests;

impl TryFrom<String> for MetricId {
    type Error = finstack_quant_core::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

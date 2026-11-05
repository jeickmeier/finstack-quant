//! Discovery of what the standard registry prices and measures.
//!
//! Lists the registered models and metrics, per instrument type and in total,
//! and describes metric keys (unit, display group, bucket coordinates) for
//! host bindings and UIs. Everything here reads the umbrella's `STANDARD`
//! registry, so it lives in this crate rather than in valuations-core; the
//! items are lifted flat into `crate::pricer`.

use crate::metrics::{MetricId, parse_key_rate_label};
use finstack_quant_core::Error;
use std::collections::BTreeMap;

/// List all metric IDs in the standard metric registry.
pub fn list_standard_metrics() -> Vec<String> {
    crate::pricer::STANDARD
        .metrics()
        .available_metrics()
        .into_iter()
        .map(|id| id.to_string())
        .collect()
}

/// List all standard metrics grouped by display category.
pub fn list_standard_metrics_grouped() -> BTreeMap<String, Vec<String>> {
    crate::pricer::STANDARD
        .metrics()
        .available_metrics_grouped()
        .into_iter()
        .map(|(group, metrics)| {
            (
                group.display_name().to_string(),
                metrics
                    .into_iter()
                    .map(|metric| metric.to_string())
                    .collect(),
            )
        })
        .collect()
}

/// List every metric the standard registry holds for one instrument type,
/// grouped by display category.
///
/// This is the per-instrument counterpart to [`list_standard_metrics_grouped`]:
/// each group keeps only the metrics for which the standard registry's
/// [`MetricRegistry::is_applicable`](crate::metrics::MetricRegistry::is_applicable)
/// holds for `instrument_type`, and groups left empty are omitted. Applicable
/// metrics outside every [`MetricGroup`](crate::metrics::MetricGroup), such as
/// `all_in_rate` or `parity`, are listed under
/// [`MetricGroup::INSTRUMENT_SPECIFIC_DISPLAY_NAME`](crate::metrics::MetricGroup::INSTRUMENT_SPECIFIC_DISPLAY_NAME),
/// so the values of the map are exactly
/// [`MetricRegistry::metrics_for_instrument`](crate::metrics::MetricRegistry::metrics_for_instrument).
/// A listed metric is registered for the type; whether it computes under a
/// particular pricing model or market still depends on that model and market.
///
/// # Arguments
///
/// * `instrument_type` - Canonical [`InstrumentType`](crate::pricer::InstrumentType)
///   display string, matched exactly (snake case, for example `"bond"`,
///   `"fx_option"` or `"structured_credit"`); the same tag an instrument
///   envelope carries in its `type` field.
///
/// # Returns
///
/// A map from display-group name to that group's applicable metric IDs in
/// ascending order, in ascending group-name order.
///
/// # Errors
///
/// Returns `Error::Validation` when `instrument_type` is not a known
/// instrument type.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::pricer::metrics_for_instrument;
///
/// let fx = metrics_for_instrument("fx_option")?;
/// assert!(!fx.values().flatten().any(|id| id == "wal"));
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn metrics_for_instrument(
    instrument_type: &str,
) -> finstack_quant_core::Result<BTreeMap<String, Vec<String>>> {
    let instrument = instrument_type
        .parse::<super::InstrumentType>()
        .map_err(|e| {
            Error::Validation(format!("Unknown instrument type: '{instrument_type}'. {e}"))
        })?;
    let registry = crate::pricer::STANDARD.metrics();
    let mut groups: BTreeMap<String, Vec<String>> = registry
        .available_metrics_grouped()
        .into_iter()
        .filter_map(|(group, metrics)| {
            let ids: Vec<String> = metrics
                .into_iter()
                .filter(|metric| registry.is_applicable(metric, instrument))
                .map(|metric| metric.to_string())
                .collect();
            (!ids.is_empty()).then(|| (group.display_name().to_string(), ids))
        })
        .collect();
    let ungrouped: Vec<String> = registry
        .metrics_for_instrument(instrument)
        .into_iter()
        .filter(|metric| display_group(metric).is_none())
        .map(|metric| metric.to_string())
        .collect();
    if !ungrouped.is_empty() {
        groups.insert(
            crate::metrics::MetricGroup::INSTRUMENT_SPECIFIC_DISPLAY_NAME.to_string(),
            ungrouped,
        );
    }
    Ok(groups)
}

/// The [`MetricGroup`](crate::metrics::MetricGroup) holding `metric`, if any.
fn display_group(metric: &MetricId) -> Option<crate::metrics::MetricGroup> {
    crate::metrics::MetricGroup::ALL
        .iter()
        .copied()
        .find(|group| group.metrics().contains(metric))
}

/// Canonical per-key metric interpretation for host presentation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MetricMetadata {
    /// Original canonical wire key, retained without renaming.
    pub key: String,
    /// Base metric name without composite coordinates.
    pub metric: String,
    /// Decoded coordinate labels in original order; empty for scalar metrics.
    pub components: Vec<String>,
    /// How each entry of `components` orders along its axis, aligned with it.
    pub coordinates: Vec<MetricCoordinate>,
    /// Native unit family; undeclared custom and calculator-dependent units are unknown.
    pub unit: crate::metrics::MetricUnit,
    /// Native display group, the same name [`metrics_for_instrument`] lists the
    /// base metric under; absent for a base metric the standard registry does
    /// not hold.
    pub group: Option<String>,
    /// Whether this is a DV01/CS01 bucket with nonempty identifier and bucket coordinates.
    pub bucketed: bool,
}

/// Where one coordinate label sits on its axis.
///
/// A host sorts by `group` and then by `value`, so maturity and strike order
/// is not recovered by parsing label text.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MetricCoordinate {
    /// Text the value is scoped by: the `{surface}::` of a vega row, empty for
    /// a bare tenor or strike, and the whole label when it carries no number.
    pub group: String,
    /// Tenor in years (`3m` is 0.25, per [`parse_key_rate_label`]) or the
    /// decimal strike; `None` when the label carries no number.
    pub value: Option<f64>,
}

impl MetricCoordinate {
    /// Read a decoded coordinate label: a tenor or decimal after any final `::`.
    fn from_label(label: &str) -> Self {
        let tail = label.rfind("::").map_or(0, |index| index + 2);
        let (group, coordinate) = label.split_at(tail);
        let value = parse_key_rate_label(coordinate)
            .ok()
            .or_else(|| coordinate.parse::<f64>().ok().filter(|v| v.is_finite()));
        match value {
            Some(value) => Self {
                group: group.to_string(),
                value: Some(value),
            },
            None => Self {
                group: label.to_string(),
                value: None,
            },
        }
    }
}

/// Describe canonical metric keys without computing or transforming their values.
///
/// # Arguments
///
/// * `keys` - Canonical wire metric keys in desired output order. Qualified keys
///   retain their original spelling and use the native composite codec to decode
///   coordinates. A custom name the standard registry holds gets
///   [`MetricGroup::INSTRUMENT_SPECIFIC_DISPLAY_NAME`](crate::metrics::MetricGroup::INSTRUMENT_SPECIFIC_DISPLAY_NAME);
///   any other is accepted with no group. A custom name has a unit only when
///   [`MetricId::unit`] declares one, and is otherwise unknown.
///
/// # Returns
///
/// One metadata record per input, retaining order and duplicates. Empty input
/// returns an empty vector. Unit families do not imply a bump size or FX conversion.
///
/// # Errors
///
/// Returns an input error when a key uses malformed or obsolete composite encoding.
pub fn metric_metadata(keys: &[String]) -> finstack_quant_core::Result<Vec<MetricMetadata>> {
    keys.iter()
        .map(|key| {
            let id: MetricId = key.parse()?;
            let base = id.base();
            let components = id.decode_components(&base).unwrap_or_default();
            let group = match display_group(&base) {
                Some(group) => Some(group.display_name().to_string()),
                None => crate::pricer::STANDARD
                    .metrics()
                    .has_metric(base.clone())
                    .then(|| {
                        crate::metrics::MetricGroup::INSTRUMENT_SPECIFIC_DISPLAY_NAME.to_string()
                    }),
            };
            let bucketed = (base == MetricId::BucketedDv01 || base == MetricId::BucketedCs01)
                && components.len() == 2
                && components.iter().all(|component| !component.is_empty());
            Ok(MetricMetadata {
                key: key.clone(),
                metric: base.to_string(),
                coordinates: components
                    .iter()
                    .map(|label| MetricCoordinate::from_label(label))
                    .collect(),
                components,
                unit: id.unit(),
                group,
                bucketed,
            })
        })
        .collect()
}

/// List every pricing model key that has a pricer in the standard registry.
///
/// The list is **registry-derived**, not enum-derived: it reports real dispatch
/// coverage. A [`ModelKey`] variant that exists in the enum but has no
/// registered pricer is omitted, whereas iterating `ModelKey` itself would
/// advertise models that cannot price any instrument. Names are the canonical
/// `ModelKey` display strings (`"discounting"`, `"black76"`, …) accepted by
/// [`parse_model_key`] and by the `model` argument of the JSON pricing entry
/// points.
///
/// # Returns
///
/// Deduplicated canonical model keys in ascending [`ModelKey`] order.
pub fn list_models() -> Vec<String> {
    crate::pricer::STANDARD
        .pricers()
        .all_models()
        .into_iter()
        .map(|model| model.to_string())
        .collect()
}

/// List the standard registry's pricing models grouped by instrument type.
///
/// This is the grouped counterpart to [`list_models`] and shares its
/// registry-derived semantics: only instrument types with at least one
/// registered pricer appear, and each entry lists only the models that can
/// actually price that instrument. Keys are canonical [`crate::pricer::InstrumentType`]
/// display strings; values are canonical [`ModelKey`] display strings.
///
/// # Returns
///
/// A map from instrument type to its ascending, deduplicated model keys, in
/// ascending instrument-type order.
pub fn list_models_grouped() -> BTreeMap<String, Vec<String>> {
    crate::pricer::STANDARD
        .pricers()
        .all_models_grouped()
        .into_iter()
        .map(|(instrument, models)| {
            (
                instrument.to_string(),
                models.into_iter().map(|model| model.to_string()).collect(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_order_tenors_strikes_and_vega_rows_by_value() {
        let at = |label: &str| {
            let coordinate = MetricCoordinate::from_label(label);
            (coordinate.group, coordinate.value)
        };
        let bare = |value: f64| (String::new(), Some(value));
        assert_eq!(at("3m"), bare(0.25));
        assert_eq!(at("12m"), bare(1.0));
        assert_eq!(at("0.75y"), bare(0.75));
        assert_eq!(at("10y"), bare(10.0));
        assert_eq!(at("0.004"), bare(0.004));
        assert_eq!(at("-0.5"), bare(-0.5));
        assert_eq!(at("S::0.25y"), ("S::".to_string(), Some(0.25)));
        assert_eq!(at("USD-OIS"), ("USD-OIS".to_string(), None));
        assert_eq!(at(""), (String::new(), None));
        // A word that f64::from_str reads as a number is still text.
        assert_eq!(at("inf"), ("inf".to_string(), None));
    }

    #[test]
    fn metrics_for_instrument_filters_by_registered_applicability() {
        let offers = |ty: &str| -> bool {
            metrics_for_instrument(ty)
                .expect("known instrument type")
                .values()
                .flatten()
                .any(|id| id == "wal")
        };
        assert!(!offers("fx_option"), "fx_option must not offer wal");
        assert!(
            offers("structured_credit"),
            "structured_credit must offer wal"
        );

        let fx = metrics_for_instrument("fx_option").expect("fx_option");
        let all = list_standard_metrics_grouped();
        assert!(!fx.is_empty());
        for (group, ids) in &fx {
            assert!(!ids.is_empty(), "empty group {group} must be omitted");
            if group == crate::metrics::MetricGroup::INSTRUMENT_SPECIFIC_DISPLAY_NAME {
                continue;
            }
            let standard = all.get(group).expect("group of the standard listing");
            assert!(ids.iter().all(|id| standard.contains(id)));
        }
    }

    #[test]
    fn metrics_for_instrument_lists_exactly_what_the_registry_applies() {
        use crate::metrics::MetricGroup;
        assert!(
            MetricGroup::ALL
                .iter()
                .all(|group| group.display_name() != MetricGroup::INSTRUMENT_SPECIFIC_DISPLAY_NAME),
            "the ungrouped name must not merge with a real group"
        );
        for ty in [
            "bond",
            "term_loan",
            "revolving_credit",
            "convertible_bond",
            "fx_option",
        ] {
            let instrument: crate::pricer::InstrumentType = ty.parse().expect("known type");
            let mut listed: Vec<String> = metrics_for_instrument(ty)
                .expect("known type")
                .into_values()
                .flatten()
                .collect();
            listed.sort();
            let registered: Vec<String> = crate::pricer::STANDARD
                .metrics()
                .metrics_for_instrument(instrument)
                .iter()
                .map(ToString::to_string)
                .collect();
            assert_eq!(listed, registered, "{ty}");
        }
    }

    #[test]
    fn metrics_for_instrument_surfaces_custom_ids_and_metadata_agrees() {
        use crate::metrics::MetricGroup;
        let ungrouped = |ty: &str| {
            metrics_for_instrument(ty)
                .expect("known type")
                .remove(MetricGroup::INSTRUMENT_SPECIFIC_DISPLAY_NAME)
                .unwrap_or_default()
        };
        assert!(ungrouped("revolving_credit").contains(&"utilization_rate".to_string()));
        assert!(ungrouped("fx_swap").contains(&"carry_pv".to_string()));

        let metadata =
            metric_metadata(&["utilization_rate".to_string(), "no_such_metric".to_string()])
                .expect("metadata");
        assert_eq!(
            metadata[0].group.as_deref(),
            Some(MetricGroup::INSTRUMENT_SPECIFIC_DISPLAY_NAME)
        );
        assert_eq!(metadata[1].group, None);
    }

    #[test]
    fn metrics_for_instrument_rejects_unknown_type() {
        let err = metrics_for_instrument("not_an_instrument").expect_err("unknown type");
        assert!(matches!(err, Error::Validation(_)), "{err:?}");
    }

    #[test]
    fn metric_metadata_describes_canonical_keys() {
        let keys = vec![
            "ytm".to_string(),
            "bucketed_dv01::A_x3a_x3aB::5y".to_string(),
            "custom_metric".to_string(),
            "pv01::USD-OIS".to_string(),
            "ytm".to_string(),
        ];
        let metadata = metric_metadata(&keys).expect("metadata");
        assert_eq!(
            metadata
                .iter()
                .map(|entry| entry.key.as_str())
                .collect::<Vec<_>>(),
            keys.iter().map(String::as_str).collect::<Vec<_>>()
        );

        let ytm = &metadata[0];
        assert_eq!(ytm.metric, "ytm");
        assert_eq!(ytm.unit, crate::metrics::MetricUnit::Decimal);
        assert_eq!(ytm.group.as_deref(), Some("Pricing"));
        assert!(ytm.components.is_empty());
        assert!(!ytm.bucketed);

        let bucketed = &metadata[1];
        assert_eq!(bucketed.metric, "bucketed_dv01");
        assert_eq!(bucketed.unit, crate::metrics::MetricUnit::Currency);
        assert_eq!(bucketed.group.as_deref(), Some("Sensitivity"));
        assert_eq!(bucketed.components, vec!["A::B", "5y"]);
        assert_eq!(
            bucketed.coordinates,
            vec![
                MetricCoordinate {
                    group: "A::B".to_string(),
                    value: None
                },
                MetricCoordinate {
                    group: String::new(),
                    value: Some(5.0)
                },
            ]
        );
        assert!(bucketed.bucketed);

        let custom = &metadata[2];
        assert_eq!(custom.metric, "custom_metric");
        assert_eq!(custom.unit, crate::metrics::MetricUnit::Unknown);
        assert_eq!(custom.group, None);
        assert!(!custom.bucketed);

        let pv01 = &metadata[3];
        assert_eq!(pv01.metric, "pv01");
        assert_eq!(pv01.unit, crate::metrics::MetricUnit::Currency);
        assert_eq!(pv01.components, vec!["USD-OIS"]);
        assert!(!pv01.bucketed);

        assert!(metric_metadata(&[]).expect("empty").is_empty());
        assert!(metric_metadata(&["pv01::USD_x2dOIS".to_string()]).is_err());
    }
}

//! JSON document types and loader for stress templates.
//!
//! Defines the serde-facing [`JsonTemplateDocument`] and parsing helpers used
//! by both the embedded built-in registry and runtime JSON registration paths.

use crate::{Error, Result, ScenarioSpec, TemplateMetadata};
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};

const EMBEDDED_TEMPLATE_JSONS: [(&str, &str); 5] = [
    (
        "gfc_2008",
        include_str!("../../data/templates/gfc_2008.json"),
    ),
    (
        "covid_2020",
        include_str!("../../data/templates/covid_2020.json"),
    ),
    (
        "rate_shock_2022",
        include_str!("../../data/templates/rate_shock_2022.json"),
    ),
    (
        "svb_2023",
        include_str!("../../data/templates/svb_2023.json"),
    ),
    (
        "ltcm_1998",
        include_str!("../../data/templates/ltcm_1998.json"),
    ),
];

/// Return the embedded built-in template JSON payloads.
pub(crate) fn embedded_template_jsons() -> &'static [(&'static str, &'static str)] {
    &EMBEDDED_TEMPLATE_JSONS
}

/// Parse and validate a JSON template document.
///
/// `name` is only used to produce readable error messages.
pub(crate) fn parse_template_document(name: &str, json: &str) -> Result<JsonTemplateDocument> {
    let document: JsonTemplateDocument = serde_json::from_str(json).map_err(|error| {
        Error::validation(format!("failed to parse JSON template '{name}': {error}"))
    })?;

    document.validate().map_err(|error| match error {
        Error::Validation(message) => Error::validation(format!(
            "JSON template '{name}' failed validation: {message}"
        )),
        other => other,
    })?;

    Ok(document)
}

/// Load and validate all embedded built-in template documents.
pub(crate) fn load_embedded_documents() -> Result<Vec<JsonTemplateDocument>> {
    embedded_template_jsons()
        .iter()
        .map(|(name, json)| parse_embedded_document(name, json))
        .collect()
}

/// Load and validate a single embedded built-in template document by name.
#[cfg(test)]
pub(crate) fn load_embedded_document(name: &str) -> Result<JsonTemplateDocument> {
    let (_, json) = embedded_template_jsons()
        .iter()
        .find(|(candidate_name, _)| *candidate_name == name)
        .ok_or_else(|| Error::validation(format!("unknown embedded template '{name}'")))?;

    parse_embedded_document(name, json)
}

fn parse_embedded_document(name: &str, json: &str) -> Result<JsonTemplateDocument> {
    let document: JsonTemplateDocument = serde_json::from_str(json).map_err(|error| {
        Error::validation(format!(
            "failed to parse embedded template '{name}': {error}"
        ))
    })?;

    if document.metadata.id != name {
        return Err(Error::validation(format!(
            "embedded template lookup name '{name}' must match metadata.id '{}'",
            document.metadata.id
        )));
    }

    document.validate().map_err(|error| match error {
        Error::Validation(message) => Error::validation(format!(
            "embedded template '{name}' failed validation: {message}"
        )),
        other => other,
    })?;

    Ok(document)
}

/// Serde-facing JSON document for a composable stress template.
///
/// Drives both the embedded built-in loader and runtime JSON registration paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JsonTemplateDocument {
    /// Template metadata shared with the runtime registry layer.
    pub(crate) metadata: TemplateMetadata,
    /// Component scenarios keyed by deterministic component identifier.
    pub(crate) components: IndexMap<String, ScenarioSpec>,
    /// Composite scenario identity and ordered component references.
    pub(crate) composite: JsonCompositeTemplate,
}

impl JsonTemplateDocument {
    /// Validate document-level consistency across metadata, components, and composite.
    pub(crate) fn validate(&self) -> Result<()> {
        self.composite.validate()?;

        if self.metadata.id != self.composite.id {
            return Err(Error::validation("metadata.id must match composite.id"));
        }

        if self.metadata.components != self.composite.component_ids {
            return Err(Error::validation(
                "metadata.components must match component IDs".to_string(),
            ));
        }

        let composite_component_ids: IndexSet<&str> = self
            .composite
            .component_ids
            .iter()
            .map(String::as_str)
            .collect();

        for component_id in &self.composite.component_ids {
            if !self.components.contains_key(component_id) {
                return Err(Error::validation(format!(
                    "composite references missing component '{component_id}'"
                )));
            }
        }

        for (component_key, spec) in &self.components {
            if component_key != &spec.id {
                return Err(Error::validation(format!(
                    "component key '{component_key}' must match ScenarioSpec.id '{}'",
                    spec.id
                )));
            }

            if !composite_component_ids.contains(component_key.as_str()) {
                return Err(Error::validation(format!(
                    "component '{component_key}' is not referenced by composite.component_ids"
                )));
            }

            spec.validate().map_err(|error| {
                Error::validation(format!(
                    "component '{component_key}' failed validation: {error}"
                ))
            })?;
        }

        if self.components.len() != self.composite.component_ids.len() {
            return Err(Error::validation(
                "component map must match composite.component_ids membership".to_string(),
            ));
        }

        Ok(())
    }
}

/// Composite scenario identity plus ordered component references.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JsonCompositeTemplate {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    priority: i32,
    component_ids: Vec<String>,
}

impl JsonCompositeTemplate {
    /// Create a composite template from top-level identity fields and ordered references.
    ///
    /// Only used by tests; production paths construct `JsonCompositeTemplate` via serde.
    #[cfg(test)]
    pub(crate) fn new(
        id: impl Into<String>,
        name: Option<&str>,
        description: Option<&str>,
        priority: i32,
        component_ids: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.map(str::to_string),
            description: description.map(str::to_string),
            priority,
            component_ids,
        }
    }

    /// Validate the composite identity and ordered component references.
    pub(crate) fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            return Err(Error::validation("Scenario ID cannot be empty"));
        }

        if self.component_ids.is_empty() {
            return Err(Error::validation("component_ids cannot be empty"));
        }

        let mut seen = IndexSet::new();
        for component_id in &self.component_ids {
            if component_id.trim().is_empty() {
                return Err(Error::validation("component_ids cannot contain empty IDs"));
            }
            if !seen.insert(component_id) {
                return Err(Error::validation(format!(
                    "duplicate component_id '{component_id}'"
                )));
            }
        }

        Ok(())
    }

    /// Return the composite identifier.
    #[must_use]
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    /// Return the optional composite display name.
    #[must_use]
    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Return the optional composite description.
    #[must_use]
    pub(crate) fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Return the composite priority.
    #[must_use]
    pub(crate) fn priority(&self) -> i32 {
        self.priority
    }

    /// Return the ordered component identifiers referenced by the composite.
    #[must_use]
    pub(crate) fn component_ids(&self) -> &[String] {
        &self.component_ids
    }
}

#[cfg(test)]
mod loader_tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::{
        embedded_template_jsons, load_embedded_document, load_embedded_documents,
        parse_embedded_document, JsonTemplateDocument,
    };
    use crate::OperationSpec;
    use finstack_quant_core::currency::Currency;

    fn composite_component_ids(document: &JsonTemplateDocument) -> Vec<String> {
        serde_json::to_value(&document.composite).expect("serialize composite")["component_ids"]
            .as_array()
            .expect("component_ids should be an array")
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("component_id should be a string")
                    .to_string()
            })
            .collect()
    }

    fn assert_component_order(document: &JsonTemplateDocument, component_ids: &[&str]) {
        assert_eq!(
            document.metadata.components,
            component_ids
                .iter()
                .map(|component_id| (*component_id).to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            composite_component_ids(document),
            document.metadata.components
        );
        let loaded_component_ids: Vec<_> = document.components.keys().cloned().collect();
        assert_eq!(loaded_component_ids, document.metadata.components);
    }

    #[test]
    fn loads_all_embedded_documents() {
        let documents = load_embedded_documents().expect("embedded documents should load");

        assert_eq!(documents.len(), embedded_template_jsons().len());
        assert_eq!(documents.len(), 5);
        assert_eq!(
            documents
                .iter()
                .map(|document| document.metadata.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "gfc_2008",
                "covid_2020",
                "rate_shock_2022",
                "svb_2023",
                "ltcm_1998",
            ]
        );
    }

    #[test]
    fn loaded_documents_validate_successfully() {
        let documents = load_embedded_documents().expect("embedded documents should load");

        for document in documents {
            document
                .validate()
                .expect("embedded document should validate");
        }
    }

    #[test]
    fn can_load_named_builtins() {
        let gfc = load_embedded_document("gfc_2008").expect("gfc document should load");
        let covid = load_embedded_document("covid_2020").expect("covid document should load");
        let rate_shock =
            load_embedded_document("rate_shock_2022").expect("rate shock document should load");
        let svb = load_embedded_document("svb_2023").expect("svb document should load");
        let ltcm = load_embedded_document("ltcm_1998").expect("ltcm document should load");

        assert_eq!(gfc.metadata.id, "gfc_2008");
        assert_eq!(covid.metadata.id, "covid_2020");
        assert_eq!(rate_shock.metadata.id, "rate_shock_2022");
        assert_eq!(svb.metadata.id, "svb_2023");
        assert_eq!(ltcm.metadata.id, "ltcm_1998");
    }

    #[test]
    fn missing_embedded_name_errors_cleanly() {
        let error = load_embedded_document("does_not_exist").expect_err("missing name should fail");

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error
            .to_string()
            .contains("unknown embedded template 'does_not_exist'"));
    }

    #[test]
    fn rejects_embedded_document_when_lookup_name_does_not_match_metadata_id() {
        let raw_json = embedded_template_jsons()
            .iter()
            .find_map(|(name, json)| (*name == "gfc_2008").then_some(*json))
            .expect("gfc fixture should exist");
        let mut value: serde_json::Value = serde_json::from_str(raw_json).expect("parse fixture");

        value["metadata"]["id"] = serde_json::Value::String("other_template".into());

        let error = parse_embedded_document(
            "gfc_2008",
            &serde_json::to_string(&value).expect("serialize modified fixture"),
        )
        .expect_err("mismatched lookup name should fail");

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error.to_string().contains(
            "embedded template lookup name 'gfc_2008' must match metadata.id 'other_template'"
        ));
    }

    #[test]
    fn parsed_content_matches_expected_ids_and_component_order() {
        let gfc = load_embedded_document("gfc_2008").expect("gfc document should load");
        let covid = load_embedded_document("covid_2020").expect("covid document should load");
        let rate_shock =
            load_embedded_document("rate_shock_2022").expect("rate shock document should load");
        let svb = load_embedded_document("svb_2023").expect("svb document should load");
        let ltcm = load_embedded_document("ltcm_1998").expect("ltcm document should load");

        assert_component_order(
            &gfc,
            &[
                "gfc_2008_rates",
                "gfc_2008_credit",
                "gfc_2008_equity",
                "gfc_2008_vol",
                "gfc_2008_fx",
            ],
        );
        assert_component_order(
            &covid,
            &[
                "covid_2020_rates",
                "covid_2020_credit",
                "covid_2020_equity",
                "covid_2020_vol",
                "covid_2020_fx",
            ],
        );
        assert_component_order(
            &rate_shock,
            &[
                "rate_shock_2022_rates",
                "rate_shock_2022_credit",
                "rate_shock_2022_equity",
                "rate_shock_2022_vol",
                "rate_shock_2022_fx",
            ],
        );
        assert_component_order(
            &svb,
            &[
                "svb_2023_rates",
                "svb_2023_credit",
                "svb_2023_equity",
                "svb_2023_vol",
                "svb_2023_fx",
            ],
        );
        assert_component_order(
            &ltcm,
            &[
                "ltcm_1998_rates",
                "ltcm_1998_credit",
                "ltcm_1998_equity",
                "ltcm_1998_vol",
                "ltcm_1998_fx",
            ],
        );
    }

    #[test]
    fn new_builtins_have_expected_focused_content() {
        let rate_shock =
            load_embedded_document("rate_shock_2022").expect("rate shock document should load");
        let svb = load_embedded_document("svb_2023").expect("svb document should load");
        let ltcm = load_embedded_document("ltcm_1998").expect("ltcm document should load");

        let rate_credit = rate_shock
            .components
            .get("rate_shock_2022_credit")
            .expect("rate shock credit component should exist");
        assert_eq!(rate_credit.operations.len(), 2);

        let svb_credit = svb
            .components
            .get("svb_2023_credit")
            .expect("svb credit component should exist");
        assert!(svb_credit.operations.iter().any(|operation| {
            matches!(
                operation,
                OperationSpec::InstrumentSpreadBpByAttr { attrs, bp }
                    if attrs.get("sector").map(String::as_str) == Some("regional_banks")
                        && (*bp - 150.0).abs() < f64::EPSILON
            )
        }));

        let ltcm_fx = ltcm
            .components
            .get("ltcm_1998_fx")
            .expect("ltcm fx component should exist");
        assert!(ltcm_fx.operations.iter().any(|operation| {
            matches!(
                operation,
                OperationSpec::MarketFxPct { base, quote, pct }
                    if *base == Currency::BRL
                        && *quote == Currency::USD
                        && (*pct - -30.0).abs() < f64::EPSILON
            )
        }));
        assert!(ltcm_fx.operations.iter().any(|operation| {
            matches!(
                operation,
                OperationSpec::MarketFxPct { base, quote, pct }
                    if *base == Currency::RUB
                        && *quote == Currency::USD
                        && (*pct - -50.0).abs() < f64::EPSILON
            )
        }));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::{JsonCompositeTemplate, JsonTemplateDocument};
    use crate::{CurveKind, OperationSpec, ScenarioSpec, TemplateMetadata};
    use indexmap::{indexmap, IndexMap};
    use time::macros::date;

    fn component_spec(id: &str) -> ScenarioSpec {
        ScenarioSpec {
            id: id.to_string(),
            name: Some(format!("{id} component")),
            description: Some(format!("{id} description")),
            operations: vec![OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: format!("{id}_curve").into(),
                discount_curve_id: None,
                bp: 25.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        }
    }

    fn metadata(components: Vec<&str>) -> TemplateMetadata {
        TemplateMetadata {
            id: "gfc_2008".into(),
            name: "Global Financial Crisis 2008".into(),
            description: "Lehman collapse scenario".into(),
            event_date: date!(2008 - 09 - 15),
            asset_classes: vec![crate::AssetClass::Rates, crate::AssetClass::Credit],
            tags: vec!["systemic".into()],
            severity: crate::Severity::Severe,
            components: components.into_iter().map(str::to_string).collect(),
        }
    }

    fn composite(component_ids: Vec<&str>) -> JsonCompositeTemplate {
        JsonCompositeTemplate::new(
            "gfc_2008",
            Some("Global Financial Crisis 2008"),
            Some("Composite scenario"),
            10,
            component_ids.into_iter().map(str::to_string).collect(),
        )
    }

    fn valid_document() -> JsonTemplateDocument {
        JsonTemplateDocument {
            metadata: metadata(vec!["rates", "credit"]),
            components: indexmap! {
                "rates".into() => component_spec("rates"),
                "credit".into() => component_spec("credit"),
            },
            composite: composite(vec!["rates", "credit"]),
        }
    }

    #[test]
    fn valid_document_passes_validation() {
        valid_document()
            .validate()
            .expect("document should validate");
    }

    #[test]
    fn rejects_mismatched_metadata_and_composite_id() {
        let mut document = valid_document();
        document.composite.id = "other_id".into();

        let error = document.validate().expect_err("validation should fail");

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error
            .to_string()
            .contains("metadata.id must match composite.id"));
    }

    #[test]
    fn rejects_missing_referenced_component() {
        let document = JsonTemplateDocument {
            metadata: metadata(vec!["rates", "credit"]),
            components: indexmap! {
                "rates".into() => component_spec("rates"),
            },
            composite: composite(vec!["rates", "credit"]),
        };

        let error = document.validate().expect_err("validation should fail");

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error.to_string().contains("missing component 'credit'"));
    }

    #[test]
    fn rejects_mismatched_metadata_components() {
        let document = JsonTemplateDocument {
            metadata: metadata(vec!["rates", "vol"]),
            components: indexmap! {
                "rates".into() => component_spec("rates"),
                "credit".into() => component_spec("credit"),
            },
            composite: composite(vec!["rates", "credit"]),
        };

        let error = document.validate().expect_err("validation should fail");

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error
            .to_string()
            .contains("metadata.components must match component IDs"));
    }

    #[test]
    fn rejects_empty_composite_component_ids() {
        let document = JsonTemplateDocument {
            metadata: metadata(vec![]),
            components: IndexMap::new(),
            composite: composite(vec![]),
        };

        let error = document.validate().expect_err("validation should fail");

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error.to_string().contains("component_ids cannot be empty"));
    }

    #[test]
    fn rejects_duplicate_component_ids() {
        let document = JsonTemplateDocument {
            metadata: metadata(vec!["rates", "rates"]),
            components: indexmap! {
                "rates".into() => component_spec("rates"),
            },
            composite: composite(vec!["rates", "rates"]),
        };

        let error = document.validate().expect_err("validation should fail");

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error.to_string().contains("duplicate component_id 'rates'"));
    }

    #[test]
    fn validates_component_specs() {
        let document = JsonTemplateDocument {
            metadata: metadata(vec!["rates"]),
            components: indexmap! {
                "rates".into() => ScenarioSpec {
                    id: "rates".into(),
                    name: Some("Broken component".into()),
                    description: None,
                    operations: vec![OperationSpec::CurveParallelBp {
                        curve_kind: CurveKind::Discount,
                        curve_id: "".into(),
                        discount_curve_id: None,
                        bp: 25.0,
                    }],
                    priority: 0,
                    resolution_mode: Default::default(),
                },
            },
            composite: composite(vec!["rates"]),
        };

        let error = document.validate().expect_err("validation should fail");

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error
            .to_string()
            .contains("component 'rates' failed validation"));
    }

    #[test]
    fn rejects_component_key_spec_id_mismatch() {
        let document = JsonTemplateDocument {
            metadata: metadata(vec!["rates"]),
            components: indexmap! {
                "rates".into() => component_spec("other_rates"),
            },
            composite: composite(vec!["rates"]),
        };

        let error = document.validate().expect_err("validation should fail");

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error
            .to_string()
            .contains("component key 'rates' must match ScenarioSpec.id 'other_rates'"));
    }

    #[test]
    fn component_membership_validation_is_independent_of_map_insertion_order() {
        let document = JsonTemplateDocument {
            metadata: metadata(vec!["rates", "credit"]),
            components: indexmap! {
                "credit".into() => component_spec("credit"),
                "rates".into() => component_spec("rates"),
            },
            composite: composite(vec!["rates", "credit"]),
        };

        document
            .validate()
            .expect("validation should use composite.component_ids ordering");
    }

    #[test]
    fn serde_roundtrip_for_document_shape() {
        let document = valid_document();

        let json = serde_json::to_string(&document).expect("serialize");
        let value: serde_json::Value = serde_json::from_str(&json).expect("parse serialized json");
        assert_eq!(value["metadata"]["id"], "gfc_2008");
        assert_eq!(value["components"]["rates"]["id"], "rates");
        assert_eq!(value["composite"]["id"], "gfc_2008");
        assert_eq!(
            value["composite"]["component_ids"],
            serde_json::json!(["rates", "credit"])
        );
        assert!(value["composite"].get("operations").is_none());

        let roundtrip: JsonTemplateDocument = serde_json::from_str(&json).expect("deserialize");
        roundtrip.validate().expect("roundtrip should validate");

        let component_ids: Vec<_> = roundtrip.components.keys().cloned().collect();
        assert_eq!(
            component_ids,
            vec!["rates".to_string(), "credit".to_string()]
        );
    }
}

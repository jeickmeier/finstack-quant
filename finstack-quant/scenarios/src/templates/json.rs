//! JSON document type and loader for the embedded stress templates.
//!
//! Each built-in template is one [`JsonTemplateDocument`]. Its `metadata` is
//! the single source of the template's identity and component order.

use crate::{Error, Result, ScenarioSpec, TemplateMetadata};
use finstack_quant_core::FxHashSet;
use serde::{Deserialize, Serialize};

/// Embedded built-in template documents as `(file name, JSON)`, in registry
/// order. The file name is used only in error messages.
const EMBEDDED_TEMPLATE_JSONS: [(&str, &str); 5] = [
    (
        "gfc_2008.json",
        include_str!("../../data/templates/gfc_2008.json"),
    ),
    (
        "covid_2020.json",
        include_str!("../../data/templates/covid_2020.json"),
    ),
    (
        "rate_shock_2022.json",
        include_str!("../../data/templates/rate_shock_2022.json"),
    ),
    (
        "svb_2023.json",
        include_str!("../../data/templates/svb_2023.json"),
    ),
    (
        "ltcm_1998.json",
        include_str!("../../data/templates/ltcm_1998.json"),
    ),
];

/// Exact persistence marker for scenario-template documents.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum ScenarioTemplateSchema {
    /// The sole supported scenario-template contract.
    #[serde(rename = "finstack_quant.scenario_template/1")]
    ScenarioTemplate,
}

/// Parse and validate every embedded built-in template document.
pub(crate) fn load_embedded_documents() -> Result<Vec<JsonTemplateDocument>> {
    EMBEDDED_TEMPLATE_JSONS
        .iter()
        .map(|(file, json)| parse_embedded_document(file, json))
        .collect()
}

fn parse_embedded_document(file: &str, json: &str) -> Result<JsonTemplateDocument> {
    let document: JsonTemplateDocument = serde_json::from_str(json).map_err(|error| {
        Error::validation(format!(
            "failed to parse embedded template '{file}': {error}"
        ))
    })?;

    document.validate().map_err(|error| match error {
        Error::Validation(message) => Error::validation(format!(
            "embedded template '{file}' failed validation: {message}"
        )),
        other => other,
    })?;

    Ok(document)
}

/// Serde-facing JSON document for a composable stress template.
///
/// `components` holds the component scenarios in `metadata.components`
/// order; the composite scenario takes its id and name from `metadata` and
/// its operations from the components in that order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JsonTemplateDocument {
    /// Scenario-template schema marker.
    pub(crate) schema: ScenarioTemplateSchema,
    /// Template metadata: identity, discovery fields and component order.
    pub(crate) metadata: TemplateMetadata,
    /// Component scenarios, in `metadata.components` order.
    pub(crate) components: Vec<ScenarioSpec>,
    /// Composite-scenario fields that are not carried by `metadata`.
    pub(crate) composite: JsonCompositeTemplate,
}

impl JsonTemplateDocument {
    /// Check that `components` are exactly the scenarios `metadata.components`
    /// names, in that order, without duplicates, and that each one is valid.
    pub(crate) fn validate(&self) -> Result<()> {
        let id = &self.metadata.id;
        if id.trim().is_empty() {
            return Err(Error::validation("template metadata.id cannot be empty"));
        }
        if self.components.is_empty() {
            return Err(Error::validation(format!(
                "template '{id}' has no components"
            )));
        }

        let component_ids: Vec<&str> = self
            .components
            .iter()
            .map(|spec| spec.id.as_str())
            .collect();
        if component_ids != self.metadata.components {
            return Err(Error::validation(format!(
                "template '{id}' components {component_ids:?} must match metadata.components \
                 {:?} in order",
                self.metadata.components
            )));
        }

        let mut seen = FxHashSet::default();
        for spec in &self.components {
            if !seen.insert(spec.id.as_str()) {
                return Err(Error::validation(format!(
                    "template '{id}' has duplicate component '{}'",
                    spec.id
                )));
            }
            spec.validate().map_err(|error| {
                Error::validation(format!(
                    "component '{}' failed validation: {error}",
                    spec.id
                ))
            })?;
        }

        Ok(())
    }
}

/// Composite-scenario fields that differ from the template metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JsonCompositeTemplate {
    /// Description of the composite scenario.
    #[serde(default)]
    pub(crate) description: Option<String>,
    /// Composition priority of the composite scenario.
    #[serde(default)]
    pub(crate) priority: i32,
}

#[cfg(test)]
mod loader_tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::{load_embedded_documents, JsonTemplateDocument, EMBEDDED_TEMPLATE_JSONS};
    use crate::{OperationSpec, ScenarioSpec};
    use finstack_quant_core::currency::Currency;

    fn embedded_document(id: &str) -> JsonTemplateDocument {
        load_embedded_documents()
            .expect("embedded documents should load")
            .into_iter()
            .find(|document| document.metadata.id == id)
            .unwrap_or_else(|| panic!("embedded template {id} should exist"))
    }

    fn component<'a>(document: &'a JsonTemplateDocument, id: &str) -> &'a ScenarioSpec {
        document
            .components
            .iter()
            .find(|spec| spec.id == id)
            .unwrap_or_else(|| panic!("component {id} should exist"))
    }

    #[test]
    fn loads_all_embedded_documents_in_registry_order() {
        let documents = load_embedded_documents().expect("embedded documents should load");

        assert_eq!(documents.len(), EMBEDDED_TEMPLATE_JSONS.len());
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
    fn embedded_components_follow_metadata_order() {
        for document in load_embedded_documents().expect("embedded documents should load") {
            let id = &document.metadata.id;
            let expected: Vec<String> = ["rates", "credit", "equity", "vol", "fx"]
                .iter()
                .map(|suffix| format!("{id}_{suffix}"))
                .collect();
            assert_eq!(document.metadata.components, expected);
            assert_eq!(
                document
                    .components
                    .iter()
                    .map(|spec| spec.id.clone())
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn new_builtins_have_expected_focused_content() {
        let rate_shock = embedded_document("rate_shock_2022");
        let svb = embedded_document("svb_2023");
        let ltcm = embedded_document("ltcm_1998");

        assert_eq!(
            component(&rate_shock, "rate_shock_2022_credit")
                .operations
                .len(),
            2
        );

        let svb_credit = component(&svb, "svb_2023_credit");
        assert!(svb_credit.operations.iter().any(|operation| {
            matches!(
                operation,
                OperationSpec::InstrumentSpreadBpByAttr { attrs, bp }
                    if attrs.get("sector").map(String::as_str) == Some("regional_banks")
                        && (*bp - 150.0).abs() < f64::EPSILON
            )
        }));

        let ltcm_fx = component(&ltcm, "ltcm_1998_fx");
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

    use super::{JsonCompositeTemplate, JsonTemplateDocument, ScenarioTemplateSchema};
    use crate::{CurveKind, OperationSpec, ScenarioSpec, TemplateMetadata};
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
            hazard_bump_mode: Default::default(),
        }
    }

    fn document(order: &[&str], components: Vec<ScenarioSpec>) -> JsonTemplateDocument {
        JsonTemplateDocument {
            schema: ScenarioTemplateSchema::ScenarioTemplate,
            metadata: TemplateMetadata {
                id: "gfc_2008".into(),
                name: "Global Financial Crisis 2008".into(),
                description: "Lehman collapse scenario".into(),
                event_date: date!(2008 - 09 - 15),
                asset_classes: vec![crate::AssetClass::Rates, crate::AssetClass::Credit],
                tags: vec!["systemic".into()],
                severity: crate::TemplateSeverity::Severe,
                components: order.iter().map(|id| (*id).to_string()).collect(),
            },
            components,
            composite: JsonCompositeTemplate {
                description: Some("Composite scenario".into()),
                priority: 10,
            },
        }
    }

    fn validation_message(document: &JsonTemplateDocument) -> String {
        let error = document.validate().expect_err("validation should fail");
        assert!(matches!(error, crate::Error::Validation(_)));
        error.to_string()
    }

    #[test]
    fn valid_document_passes_validation() {
        document(
            &["rates", "credit"],
            vec![component_spec("rates"), component_spec("credit")],
        )
        .validate()
        .expect("document should validate");
    }

    #[test]
    fn rejects_components_out_of_metadata_order() {
        let message = validation_message(&document(
            &["rates", "credit"],
            vec![component_spec("credit"), component_spec("rates")],
        ));
        assert!(
            message.contains("must match metadata.components"),
            "{message}"
        );
    }

    #[test]
    fn rejects_component_missing_from_metadata() {
        let message = validation_message(&document(
            &["rates", "vol"],
            vec![component_spec("rates"), component_spec("credit")],
        ));
        assert!(
            message.contains("must match metadata.components"),
            "{message}"
        );
    }

    #[test]
    fn rejects_empty_components() {
        let message = validation_message(&document(&[], Vec::new()));
        assert!(message.contains("has no components"), "{message}");
    }

    #[test]
    fn rejects_duplicate_component_ids() {
        let message = validation_message(&document(
            &["rates", "rates"],
            vec![component_spec("rates"), component_spec("rates")],
        ));
        assert!(message.contains("duplicate component 'rates'"), "{message}");
    }

    #[test]
    fn validates_component_specs() {
        let mut broken = component_spec("rates");
        broken.operations = vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "".into(),
            discount_curve_id: None,
            bp: 25.0,
        }];
        let message = validation_message(&document(&["rates"], vec![broken]));
        assert!(
            message.contains("component 'rates' failed validation"),
            "{message}"
        );
    }
}

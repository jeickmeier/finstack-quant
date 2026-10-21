//! Template registry for stress test metadata and scenario specs.

use super::{json::JsonTemplateDocument, TemplateMetadata};
use crate::{Error, Result, ScenarioSpec};
use indexmap::IndexMap;

/// One registered template: its metadata, the composite scenario and the
/// component scenarios keyed by id, all validated when the registry is built.
struct RegisteredTemplate {
    metadata: TemplateMetadata,
    composite: ScenarioSpec,
    components: IndexMap<String, ScenarioSpec>,
}

impl RegisteredTemplate {
    /// Assemble the composite scenario from a template document: id and name
    /// from the metadata, operations from the components in order.
    fn from_document(document: JsonTemplateDocument) -> Result<Self> {
        document.validate()?;

        let JsonTemplateDocument {
            metadata,
            components,
            composite,
            ..
        } = document;

        let composite_operations = components
            .iter()
            .flat_map(|spec| spec.operations.iter().cloned())
            .collect::<Vec<_>>();
        let components: IndexMap<String, ScenarioSpec> = components
            .into_iter()
            .map(|spec| (spec.id.clone(), spec))
            .collect();
        let composite_spec = ScenarioSpec {
            id: metadata.id.clone(),
            name: Some(metadata.name.clone()),
            description: composite.description,
            operations: composite_operations,
            priority: composite.priority,
            resolution_mode: finstack_quant_core::market_data::hierarchy::ResolutionMode::default(),
            hazard_bump_mode: crate::HazardBumpMode::default(),
        };
        composite_spec.validate()?;

        Ok(Self {
            metadata,
            composite: composite_spec,
            components,
        })
    }
}

/// Registry of template metadata and clonable scenario specs.
///
/// The registry preserves insertion order for listing and filtering operations
/// so discovery APIs remain deterministic across runs.
pub struct TemplateRegistry {
    entries: IndexMap<String, RegisteredTemplate>,
}

impl TemplateRegistry {
    /// Build a registry from template documents, in order; a later document
    /// replaces an earlier one with the same id.
    fn from_documents(documents: Vec<JsonTemplateDocument>) -> Result<Self> {
        let mut entries = IndexMap::with_capacity(documents.len());
        for document in documents {
            let entry = RegisteredTemplate::from_document(document)?;
            entries.insert(entry.metadata.id.clone(), entry);
        }
        Ok(Self { entries })
    }

    /// Process-wide registry of the embedded built-in templates.
    ///
    /// The embedded JSON documents are parsed and validated once, on first
    /// access, and the resulting registry is shared for the lifetime of the
    /// process. Hosts that only read templates (list, build, decompose) should
    /// share this registry across calls.
    ///
    /// # Errors
    ///
    /// Returns the (cached) parse or validation error when the embedded
    /// documents are malformed.
    pub fn embedded_builtins() -> Result<&'static Self> {
        static REGISTRY: std::sync::OnceLock<Result<TemplateRegistry>> = std::sync::OnceLock::new();
        REGISTRY
            .get_or_init(|| super::json::load_embedded_documents().and_then(Self::from_documents))
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Look up a registered template, naming the identifier when it is unknown.
    fn entry(&self, template_id: &str) -> Result<&RegisteredTemplate> {
        self.entries
            .get(template_id)
            .ok_or_else(|| Error::validation(format!("Unknown template: '{template_id}'")))
    }

    /// Build a registered scenario template by identifier.
    ///
    /// # Arguments
    ///
    /// - `template_id`: Identifier returned by [`Self::list`].
    ///
    /// # Returns
    ///
    /// A validated, independently owned scenario specification.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when `template_id` is unknown.
    pub fn build(&self, template_id: &str) -> Result<ScenarioSpec> {
        Ok(self.entry(template_id)?.composite.clone())
    }

    /// Build one component of a registered scenario template.
    ///
    /// # Arguments
    ///
    /// - `template_id`: Identifier returned by [`Self::list`].
    /// - `component_id`: Component identifier returned by
    ///   [`Self::component_ids`].
    ///
    /// # Returns
    ///
    /// A validated, independently owned scenario specification for the selected
    /// component.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when either identifier is unknown.
    pub fn build_component(&self, template_id: &str, component_id: &str) -> Result<ScenarioSpec> {
        self.entry(template_id)?
            .components
            .get(component_id)
            .cloned()
            .ok_or_else(|| {
                Error::validation(format!(
                    "Unknown component '{component_id}' in template '{template_id}'"
                ))
            })
    }

    /// List component identifiers for a registered template.
    ///
    /// # Arguments
    ///
    /// - `template_id`: Identifier returned by [`Self::list`].
    ///
    /// # Returns
    ///
    /// Component identifiers in deterministic template order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when `template_id` is unknown.
    pub fn component_ids(&self, template_id: &str) -> Result<Vec<&str>> {
        Ok(self
            .entry(template_id)?
            .components
            .keys()
            .map(String::as_str)
            .collect())
    }

    /// List all registered template metadata in deterministic insertion order.
    ///
    /// # Returns
    ///
    /// Metadata references in the order the templates were registered.
    #[must_use]
    pub fn list(&self) -> Vec<&TemplateMetadata> {
        self.entries.values().map(|entry| &entry.metadata).collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::TemplateRegistry;
    use crate::templates::json::{
        JsonCompositeTemplate, JsonTemplateDocument, ScenarioTemplateSchema,
    };
    use crate::{
        AssetClass, CurveKind, OperationSpec, ScenarioSpec, TemplateMetadata, TemplateSeverity,
    };
    use time::macros::date;

    fn template_document(
        id: &str,
        tag: &str,
        asset_class: AssetClass,
        severity: TemplateSeverity,
        components: Vec<ScenarioSpec>,
    ) -> JsonTemplateDocument {
        let description = format!("Description for {id}");
        JsonTemplateDocument {
            schema: ScenarioTemplateSchema::ScenarioTemplate,
            metadata: TemplateMetadata {
                id: id.into(),
                name: format!("Template {id}"),
                description: description.clone(),
                event_date: date!(2008 - 09 - 15),
                asset_classes: vec![asset_class],
                tags: vec![tag.into()],
                severity,
                components: components.iter().map(|spec| spec.id.clone()).collect(),
            },
            components,
            composite: JsonCompositeTemplate {
                description: Some(description),
                priority: 0,
            },
        }
    }

    fn empty_component_spec(id: &str) -> ScenarioSpec {
        ScenarioSpec {
            id: id.into(),
            name: None,
            description: None,
            operations: Vec::new(),
            priority: 0,
            resolution_mode: Default::default(),
            hazard_bump_mode: Default::default(),
        }
    }

    fn registry_with_templates() -> TemplateRegistry {
        TemplateRegistry::from_documents(vec![
            template_document(
                "rates_shock",
                "systemic",
                AssetClass::Rates,
                TemplateSeverity::Severe,
                vec![json_component_spec(
                    "rates_shock_component",
                    "USD-SOFR",
                    100.0,
                )],
            ),
            template_document(
                "equity_shock",
                "equity",
                AssetClass::Equity,
                TemplateSeverity::Moderate,
                vec![empty_component_spec("equity_shock_component")],
            ),
            template_document(
                "hybrid_shock",
                "systemic",
                AssetClass::Credit,
                TemplateSeverity::Mild,
                vec![
                    json_component_spec("rates_shock", "USD-SOFR", 100.0),
                    empty_component_spec("equity_shock"),
                ],
            ),
        ])
        .expect("test templates should register")
    }

    fn registry_with(document: JsonTemplateDocument) -> TemplateRegistry {
        TemplateRegistry::from_documents(vec![document]).expect("json document should register")
    }

    fn collected_ids(entries: Vec<&TemplateMetadata>) -> Vec<&str> {
        entries.into_iter().map(|entry| entry.id.as_str()).collect()
    }

    fn json_component_spec(id: &str, curve_id: &str, bp: f64) -> ScenarioSpec {
        ScenarioSpec {
            id: id.to_string(),
            name: Some(format!("Component {id}")),
            description: Some(format!("Description for {id}")),
            operations: vec![OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: curve_id.into(),
                discount_curve_id: None,
                bp,
            }],
            priority: 0,
            resolution_mode: Default::default(),
            hazard_bump_mode: Default::default(),
        }
    }

    fn json_component_spec_with_priority(
        id: &str,
        curve_id: &str,
        bp: f64,
        priority: i32,
    ) -> ScenarioSpec {
        let mut spec = json_component_spec(id, curve_id, bp);
        spec.priority = priority;
        spec
    }

    fn json_document() -> JsonTemplateDocument {
        JsonTemplateDocument {
            schema: ScenarioTemplateSchema::ScenarioTemplate,
            metadata: TemplateMetadata {
                id: "json_template".into(),
                name: "JSON Template".into(),
                description: "Template registered from JSON".into(),
                event_date: date!(2020 - 03 - 16),
                asset_classes: vec![AssetClass::Rates, AssetClass::Equity],
                tags: vec!["systemic".into(), "json".into()],
                severity: TemplateSeverity::Severe,
                components: vec!["component_b".into(), "component_a".into()],
            },
            components: vec![
                json_component_spec("component_b", "B-CURVE", -25.0),
                json_component_spec("component_a", "A-CURVE", 50.0),
            ],
            composite: JsonCompositeTemplate {
                description: Some("Composite description from JSON".into()),
                priority: 7,
            },
        }
    }

    fn json_document_with_priority_order_conflict() -> JsonTemplateDocument {
        JsonTemplateDocument {
            schema: ScenarioTemplateSchema::ScenarioTemplate,
            metadata: TemplateMetadata {
                id: "priority_order_conflict".into(),
                name: "Priority Order Conflict".into(),
                description: "JSON order should beat component priority".into(),
                event_date: date!(2020 - 03 - 16),
                asset_classes: vec![AssetClass::Rates],
                tags: vec!["json".into()],
                severity: TemplateSeverity::Moderate,
                components: vec!["late_priority".into(), "early_priority".into()],
            },
            components: vec![
                json_component_spec_with_priority("late_priority", "LATE-CURVE", 10.0, 10),
                json_component_spec_with_priority("early_priority", "EARLY-CURVE", 20.0, -10),
            ],
            composite: JsonCompositeTemplate {
                description: Some("Composite order should follow metadata.components".into()),
                priority: 3,
            },
        }
    }

    fn curve_ids(scenario: &ScenarioSpec) -> Vec<&str> {
        scenario
            .operations
            .iter()
            .map(|operation| match operation {
                OperationSpec::CurveParallelBp { curve_id, .. } => curve_id.as_str(),
                _ => panic!("unexpected operation"),
            })
            .collect()
    }

    #[test]
    fn get_registered_template() {
        let registry = registry_with_templates();

        let template = registry
            .entries
            .get("rates_shock")
            .expect("template should exist");

        assert_eq!(template.metadata.name, "Template rates_shock");
        assert_eq!(template.metadata.tags, vec!["systemic"]);
        assert!(registry.entries.get("missing").is_none());
    }

    #[test]
    fn list() {
        let registry = registry_with_templates();

        assert_eq!(
            collected_ids(registry.list()),
            vec!["rates_shock", "equity_shock", "hybrid_shock"]
        );
    }

    #[test]
    fn filter_by_tag_equivalent_via_list() {
        let registry = registry_with_templates();

        let ids = collected_ids(
            registry
                .list()
                .into_iter()
                .filter(|metadata| metadata.tags.iter().any(|tag| tag == "systemic"))
                .collect(),
        );
        assert_eq!(ids, vec!["rates_shock", "hybrid_shock"]);
    }

    #[test]
    fn filter_by_asset_class_equivalent_via_list() {
        let registry = registry_with_templates();

        let ids = collected_ids(
            registry
                .list()
                .into_iter()
                .filter(|metadata| metadata.asset_classes.contains(&AssetClass::Equity))
                .collect(),
        );
        assert_eq!(ids, vec!["equity_shock"]);
    }

    #[test]
    fn filter_by_severity_equivalent_via_list() {
        let registry = registry_with_templates();

        let ids = collected_ids(
            registry
                .list()
                .into_iter()
                .filter(|metadata| metadata.severity == TemplateSeverity::Severe)
                .collect(),
        );
        assert_eq!(ids, vec!["rates_shock"]);
    }

    #[test]
    fn registry_builds_templates_and_components_by_id() {
        let registry = registry_with_templates();

        assert_eq!(
            registry
                .component_ids("hybrid_shock")
                .expect("template should exist"),
            vec!["rates_shock", "equity_shock"]
        );

        let composite = registry
            .build("hybrid_shock")
            .expect("template should build");
        let rates = registry
            .build_component("hybrid_shock", "rates_shock")
            .expect("rates component should build");
        let equity = registry
            .build_component("hybrid_shock", "equity_shock")
            .expect("equity component should build");

        assert_eq!(composite.id, "hybrid_shock");
        assert_eq!(rates.id, "rates_shock");
        assert_eq!(equity.id, "equity_shock");
    }

    #[test]
    fn registry_build_operations_reject_unknown_ids() {
        let registry = registry_with_templates();

        assert!(registry.build("missing").is_err());
        assert!(registry.component_ids("missing").is_err());
        assert!(registry.build_component("hybrid_shock", "missing").is_err());
    }

    #[test]
    fn embedded_builtins_register_all_builtins_in_order() {
        let registry =
            TemplateRegistry::embedded_builtins().expect("embedded builtins should load");

        assert_eq!(
            collected_ids(registry.list()),
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
    fn composite_uses_metadata_identity_and_component_order() {
        let registry = registry_with(json_document());
        let scenario = registry
            .build("json_template")
            .expect("json template should build");

        assert_eq!(scenario.id, "json_template");
        assert_eq!(scenario.name.as_deref(), Some("JSON Template"));
        assert_eq!(
            scenario.description.as_deref(),
            Some("Composite description from JSON")
        );
        assert_eq!(scenario.priority, 7);
        assert_eq!(curve_ids(&scenario), vec!["B-CURVE", "A-CURVE"]);
    }

    #[test]
    fn composite_order_follows_metadata_not_component_priority() {
        let registry = registry_with(json_document_with_priority_order_conflict());
        let scenario = registry
            .build("priority_order_conflict")
            .expect("json template should build");

        assert_eq!(curve_ids(&scenario), vec!["LATE-CURVE", "EARLY-CURVE"]);
    }

    #[test]
    fn registered_template_exposes_component_ids_and_metadata() {
        let registry = registry_with(json_document());
        let entry = registry
            .entries
            .get("json_template")
            .expect("json template should exist");

        assert_eq!(
            registry
                .component_ids("json_template")
                .expect("json template should exist"),
            vec!["component_b", "component_a"]
        );
        assert_eq!(entry.metadata.description, "Template registered from JSON");
        assert_eq!(entry.metadata.event_date, date!(2020 - 03 - 16));
        assert_eq!(
            entry.metadata.asset_classes,
            vec![AssetClass::Rates, AssetClass::Equity]
        );
        assert_eq!(entry.metadata.tags, vec!["systemic", "json"]);
        assert_eq!(entry.metadata.severity, TemplateSeverity::Severe);
    }

    #[test]
    fn from_documents_rejects_invalid_documents() {
        let mut document = json_document();
        document.metadata.components = vec!["wrong_component".into()];

        let Err(error) = TemplateRegistry::from_documents(vec![document]) else {
            panic!("invalid document should be rejected");
        };

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error.to_string().contains("must match metadata.components"));
    }
}

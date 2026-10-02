//! Template registry for stress test metadata and scenario specs.

use super::{json::JsonTemplateDocument, register_builtins, TemplateMetadata};
use crate::{Error, Result, ScenarioSpec};
use indexmap::IndexMap;

/// Registered template entry containing metadata and clonable, already
/// validated [`ScenarioSpec`] values.
///
/// Use [`RegisteredTemplate::build`] to get the full composite scenario, or
/// [`RegisteredTemplate::component`] to access an individual component by
/// identifier when a historical scenario is decomposed into reusable parts.
pub struct RegisteredTemplate {
    metadata: TemplateMetadata,
    composite: ScenarioSpec,
    components: IndexMap<String, ScenarioSpec>,
}

impl RegisteredTemplate {
    /// Build a registered template entry from a validated JSON template document.
    pub(crate) fn from_json_document(document: JsonTemplateDocument) -> Result<Self> {
        document.validate()?;

        let JsonTemplateDocument {
            metadata,
            mut components,
            composite,
            ..
        } = document;

        let ordered_component_specs = composite
            .component_ids()
            .iter()
            .map(|component_id| {
                let spec = components.shift_remove(component_id).ok_or_else(|| {
                    Error::internal(format!(
                        "validated JSON template missing component '{component_id}'"
                    ))
                })?;
                Ok((component_id.clone(), spec))
            })
            .collect::<Result<Vec<_>>>()?;

        let components: IndexMap<String, ScenarioSpec> =
            ordered_component_specs.iter().cloned().collect();
        let composite_operations = ordered_component_specs
            .iter()
            .flat_map(|(_, spec)| spec.operations.iter().cloned())
            .collect::<Vec<_>>();
        let composite_spec = ScenarioSpec {
            id: composite.id().to_string(),
            name: composite.name().map(str::to_string),
            description: composite.description().map(str::to_string),
            operations: composite_operations,
            priority: composite.priority(),
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

    /// Access the registered template metadata.
    ///
    /// # Returns
    ///
    /// The immutable metadata stored for this registered template.
    #[must_use]
    pub fn metadata(&self) -> &TemplateMetadata {
        &self.metadata
    }

    /// Clone the full composite scenario spec from the registered template.
    ///
    /// # Returns
    ///
    /// A validated, independently owned [`ScenarioSpec`] for the full
    /// registered template.
    #[must_use]
    pub fn build(&self) -> ScenarioSpec {
        self.composite.clone()
    }

    /// Clone one component scenario spec by component identifier.
    ///
    /// # Arguments
    ///
    /// - `id`: Component identifier listed in [`TemplateMetadata::components`].
    ///
    /// # Returns
    ///
    /// `Some(spec)` when a matching component exists, otherwise `None`.
    #[must_use]
    pub fn component(&self, id: &str) -> Option<ScenarioSpec> {
        self.components.get(id).cloned()
    }

    /// List registered component identifiers in deterministic insertion order.
    ///
    /// # Returns
    ///
    /// Component identifiers in the same order used when the composite
    /// template is assembled.
    #[must_use]
    pub fn component_ids(&self) -> Vec<&str> {
        self.components.keys().map(String::as_str).collect()
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
    /// Create an empty template registry with no built-in templates registered.
    ///
    /// Use [`Self::with_embedded_builtins`] to load the crate-owned historical
    /// stress templates through the fallible validation path.
    #[must_use]
    #[allow(clippy::new_without_default)] // Loading built-ins is fallible, so callers must choose explicitly.
    pub fn new() -> Self {
        Self {
            entries: IndexMap::new(),
        }
    }

    /// Create a registry preloaded with the crate-owned embedded built-in templates.
    ///
    /// # Returns
    ///
    /// A registry containing all embedded historical stress templates shipped
    /// with the crate.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded JSON documents cannot be parsed or fail
    /// validation.
    pub fn with_embedded_builtins() -> Result<Self> {
        let mut registry = Self::new();
        register_builtins(&mut registry)?;
        Ok(registry)
    }

    /// Process-wide registry of the embedded built-in templates.
    ///
    /// The embedded JSON documents are parsed and validated once, on first
    /// access, and the resulting registry is shared for the lifetime of the
    /// process. Hosts that only read templates (list, build, decompose) should
    /// use this instead of re-parsing via
    /// [`with_embedded_builtins`](Self::with_embedded_builtins) on every call.
    ///
    /// # Errors
    ///
    /// Returns the (cached) parse or validation error when the embedded
    /// documents are malformed.
    pub fn embedded_builtins() -> Result<&'static Self> {
        static REGISTRY: std::sync::OnceLock<Result<TemplateRegistry>> = std::sync::OnceLock::new();
        REGISTRY
            .get_or_init(Self::with_embedded_builtins)
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Register or replace a template from a parsed JSON document.
    pub(crate) fn register_json_document(&mut self, document: JsonTemplateDocument) -> Result<()> {
        let entry = RegisteredTemplate::from_json_document(document)?;
        self.entries.insert(entry.metadata.id.clone(), entry);
        Ok(())
    }
    /// Get a registered template entry by identifier.
    ///
    /// # Arguments
    ///
    /// - `id`: Template identifier to look up.
    ///
    /// # Returns
    ///
    /// `Some(entry)` if the template is registered, otherwise `None`.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&RegisteredTemplate> {
        self.entries.get(id)
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
        self.entries
            .get(template_id)
            .ok_or_else(|| Error::validation(format!("Unknown template: '{template_id}'")))
            .map(RegisteredTemplate::build)
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
        let entry = self
            .entries
            .get(template_id)
            .ok_or_else(|| Error::validation(format!("Unknown template: '{template_id}'")))?;
        entry.component(component_id).ok_or_else(|| {
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
        self.entries
            .get(template_id)
            .ok_or_else(|| Error::validation(format!("Unknown template: '{template_id}'")))
            .map(RegisteredTemplate::component_ids)
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
    use crate::templates::json::{JsonCompositeTemplate, JsonTemplateDocument};
    use crate::{
        AssetClass, CurveKind, OperationSpec, ScenarioSpec, TemplateMetadata, TemplateSeverity,
    };
    use indexmap::indexmap;
    use time::macros::date;

    fn template_document(
        id: &str,
        tag: &str,
        asset_class: AssetClass,
        severity: TemplateSeverity,
        component_specs: Vec<ScenarioSpec>,
    ) -> JsonTemplateDocument {
        let component_ids = component_specs
            .iter()
            .map(|spec| spec.id.clone())
            .collect::<Vec<_>>();
        let components = component_specs
            .into_iter()
            .map(|spec| (spec.id.clone(), spec))
            .collect();
        let name = format!("Template {id}");
        let description = format!("Description for {id}");
        let composite = JsonCompositeTemplate::new(
            id,
            Some(&name),
            Some(&description),
            0,
            component_ids.clone(),
        );

        JsonTemplateDocument {
            schema: crate::templates::json::ScenarioTemplateSchema::ScenarioTemplate,
            metadata: TemplateMetadata {
                id: id.into(),
                name,
                description,
                event_date: date!(2008 - 09 - 15),
                asset_classes: vec![asset_class],
                tags: vec![tag.into()],
                severity,
                components: component_ids,
            },
            components,
            composite,
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
        let mut registry = TemplateRegistry::new();

        for document in [
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
        ] {
            registry
                .register_json_document(document)
                .expect("test template should register");
        }

        registry
    }

    fn collected_ids(entries: Vec<&TemplateMetadata>) -> Vec<&str> {
        entries.into_iter().map(|entry| entry.id.as_str()).collect()
    }

    fn builtin_template_ids() -> Vec<&'static str> {
        vec![
            "gfc_2008",
            "covid_2020",
            "rate_shock_2022",
            "svb_2023",
            "ltcm_1998",
        ]
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
            schema: crate::templates::json::ScenarioTemplateSchema::ScenarioTemplate,
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
            components: indexmap! {
                "component_b".into() => json_component_spec("component_b", "B-CURVE", -25.0),
                "component_a".into() => json_component_spec("component_a", "A-CURVE", 50.0),
            },
            composite: JsonCompositeTemplate::new(
                "json_template",
                Some("Composite From JSON"),
                Some("Composite description from JSON"),
                7,
                vec!["component_b".into(), "component_a".into()],
            ),
        }
    }

    fn json_document_with_priority_order_conflict() -> JsonTemplateDocument {
        JsonTemplateDocument {
            schema: crate::templates::json::ScenarioTemplateSchema::ScenarioTemplate,
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
            components: indexmap! {
                "late_priority".into() => json_component_spec_with_priority("late_priority", "LATE-CURVE", 10.0, 10),
                "early_priority".into() => json_component_spec_with_priority("early_priority", "EARLY-CURVE", 20.0, -10),
            },
            composite: JsonCompositeTemplate::new(
                "priority_order_conflict",
                Some("Priority Order Conflict"),
                Some("Composite order should follow component_ids"),
                3,
                vec!["late_priority".into(), "early_priority".into()],
            ),
        }
    }

    fn json_document_without_composite_name() -> JsonTemplateDocument {
        JsonTemplateDocument {
            schema: crate::templates::json::ScenarioTemplateSchema::ScenarioTemplate,
            metadata: TemplateMetadata {
                id: "no_composite_name".into(),
                name: "No Composite Name".into(),
                description: "Composite name omitted in JSON".into(),
                event_date: date!(2020 - 03 - 16),
                asset_classes: vec![AssetClass::Rates],
                tags: vec!["json".into()],
                severity: TemplateSeverity::Mild,
                components: vec!["component_only".into()],
            },
            components: indexmap! {
                "component_only".into() => json_component_spec("component_only", "ONLY-CURVE", 5.0),
            },
            composite: JsonCompositeTemplate::new(
                "no_composite_name",
                None,
                Some("Composite description without a name"),
                2,
                vec!["component_only".into()],
            ),
        }
    }

    #[test]
    fn get_registered_template() {
        let registry = registry_with_templates();

        let template = registry.get("rates_shock").expect("template should exist");

        assert_eq!(template.metadata().name, "Template rates_shock");
        assert_eq!(template.metadata().tags, vec!["systemic"]);
    }

    #[test]
    fn get_missing() {
        let registry = registry_with_templates();

        assert!(registry.get("missing").is_none());
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
    fn with_embedded_builtins_registers_all_builtins() {
        let registry =
            TemplateRegistry::with_embedded_builtins().expect("embedded builtins should load");

        assert_eq!(collected_ids(registry.list()), builtin_template_ids());
    }

    #[test]
    fn embedded_registry_builds_all_builtins_and_components() {
        let registry =
            TemplateRegistry::with_embedded_builtins().expect("embedded builtins should load");
        for template_id in builtin_template_ids() {
            let scenario = registry.build(template_id).expect("scenario should build");
            let component_ids = registry
                .component_ids(template_id)
                .expect("components should be listed");

            assert_eq!(scenario.id, template_id);
            assert_eq!(component_ids.len(), 5);

            for component_id in component_ids {
                let component = registry
                    .build_component(template_id, component_id)
                    .expect("component should build");
                assert_eq!(component.id, component_id);
            }
        }
    }

    #[test]
    fn register_json_document_and_get_by_id() {
        let mut registry = TemplateRegistry::new();

        registry
            .register_json_document(json_document())
            .expect("json document should register");

        let entry = registry
            .get("json_template")
            .expect("json template should exist");

        assert_eq!(entry.metadata().id, "json_template");
        assert_eq!(entry.metadata().name, "JSON Template");
    }

    #[test]
    fn register_json_document_composite_uses_component_order_and_top_level_fields() {
        let mut registry = TemplateRegistry::new();
        registry
            .register_json_document(json_document())
            .expect("json document should register");
        let entry = registry
            .get("json_template")
            .expect("json template should exist");

        let scenario = entry.build();

        assert_eq!(scenario.id, "json_template");
        assert_eq!(scenario.name.as_deref(), Some("Composite From JSON"));
        assert_eq!(
            scenario.description.as_deref(),
            Some("Composite description from JSON")
        );
        assert_eq!(scenario.priority, 7);
        assert_eq!(scenario.operations.len(), 2);

        match &scenario.operations[0] {
            OperationSpec::CurveParallelBp { curve_id, bp, .. } => {
                assert_eq!(curve_id, "B-CURVE");
                assert_eq!(*bp, -25.0);
            }
            _ => panic!("unexpected operation"),
        }

        match &scenario.operations[1] {
            OperationSpec::CurveParallelBp { curve_id, bp, .. } => {
                assert_eq!(curve_id, "A-CURVE");
                assert_eq!(*bp, 50.0);
            }
            _ => panic!("unexpected operation"),
        }
    }

    #[test]
    fn register_json_document_composite_order_follows_component_ids_not_component_priority() {
        let mut registry = TemplateRegistry::new();
        registry
            .register_json_document(json_document_with_priority_order_conflict())
            .expect("json document should register");
        let entry = registry
            .get("priority_order_conflict")
            .expect("json template should exist");

        let scenario = entry.build();

        assert_eq!(scenario.operations.len(), 2);

        match &scenario.operations[0] {
            OperationSpec::CurveParallelBp { curve_id, .. } => {
                assert_eq!(curve_id, "LATE-CURVE");
            }
            _ => panic!("unexpected operation"),
        }

        match &scenario.operations[1] {
            OperationSpec::CurveParallelBp { curve_id, .. } => {
                assert_eq!(curve_id, "EARLY-CURVE");
            }
            _ => panic!("unexpected operation"),
        }
    }

    #[test]
    fn register_json_document_omitted_composite_name_stays_absent() {
        let mut registry = TemplateRegistry::new();
        registry
            .register_json_document(json_document_without_composite_name())
            .expect("json document should register");
        let entry = registry
            .get("no_composite_name")
            .expect("json template should exist");

        let scenario = entry.build();

        assert_eq!(scenario.id, "no_composite_name");
        assert_eq!(scenario.name, None);
        assert_eq!(
            scenario.description.as_deref(),
            Some("Composite description without a name")
        );
        assert_eq!(scenario.priority, 2);
    }

    #[test]
    fn register_json_document_exposes_component_ids_and_metadata() {
        let mut registry = TemplateRegistry::new();
        registry
            .register_json_document(json_document())
            .expect("json document should register");
        let entry = registry
            .get("json_template")
            .expect("json template should exist");

        assert_eq!(entry.component_ids(), vec!["component_b", "component_a"]);
        assert_eq!(
            entry.metadata().description,
            "Template registered from JSON"
        );
        assert_eq!(entry.metadata().event_date, date!(2020 - 03 - 16));
        assert_eq!(
            entry.metadata().asset_classes,
            vec![AssetClass::Rates, AssetClass::Equity]
        );
        assert_eq!(entry.metadata().tags, vec!["systemic", "json"]);
        assert_eq!(entry.metadata().severity, TemplateSeverity::Severe);
    }

    #[test]
    fn register_json_document_rejects_invalid_documents() {
        let mut registry = TemplateRegistry::new();
        let mut document = json_document();
        document.metadata.components = vec!["wrong_component".into()];

        let error = registry
            .register_json_document(document)
            .expect_err("invalid document should be rejected");

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error
            .to_string()
            .contains("metadata.components must match component IDs"));
    }

    #[test]
    fn new_registry_is_empty_until_templates_are_registered() {
        let registry = TemplateRegistry::new();

        assert!(registry.list().is_empty());
        assert!(registry.get("gfc_2008").is_none());
    }

    #[test]
    fn embedded_registry_provides_builtins_without_runtime_loading() {
        let registry =
            TemplateRegistry::with_embedded_builtins().expect("embedded builtins should load");

        assert!(registry.get("gfc_2008").is_some());
        assert!(registry.get("covid_2020").is_some());
        assert!(registry.get("rate_shock_2022").is_some());
        assert!(registry.get("svb_2023").is_some());
        assert!(registry.get("ltcm_1998").is_some());
    }
}

//! Template registry for stress test template metadata and builder factories.

use super::{
    json::{parse_template_document, JsonTemplateDocument},
    register_builtins, AssetClass, ScenarioSpecBuilder, Severity, TemplateMetadata,
};
use crate::{Error, Result, ScenarioSpec};
use indexmap::IndexMap;
use std::{collections::HashSet, fs, path::Path};

type TemplateFactory = dyn Fn() -> ScenarioSpecBuilder + Send + Sync;

/// Registered template entry containing metadata and fresh builder factories.
///
/// Use [`RegisteredTemplate::builder`] to build the full composite template, or
/// [`RegisteredTemplate::component`] to access an individual component builder
/// by identifier when a historical scenario is decomposed into reusable parts.
pub struct RegisteredTemplate {
    metadata: TemplateMetadata,
    factory: Box<TemplateFactory>,
    components: IndexMap<String, Box<TemplateFactory>>,
}

impl RegisteredTemplate {
    /// Build a registered template entry from a validated JSON template document.
    pub(crate) fn from_json_document(document: JsonTemplateDocument) -> Result<Self> {
        document.validate()?;

        let JsonTemplateDocument {
            metadata,
            mut components,
            composite,
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

        let ordered_component_builders = ordered_component_specs
            .iter()
            .cloned()
            .map(|(component_id, spec)| (component_id, scenario_spec_to_builder(spec)))
            .collect::<Vec<_>>();
        let composite_operations = ordered_component_specs
            .iter()
            .flat_map(|(_, spec)| spec.operations.iter().cloned())
            .collect::<Vec<_>>();
        let composite_id = composite.id().to_string();
        let composite_name = composite.name().map(str::to_string);
        let composite_description = composite.description().map(str::to_string);
        let composite_priority = composite.priority();
        let factory = Box::new(move || {
            let mut builder = ScenarioSpecBuilder::new(composite_id.clone())
                .with_operations(composite_operations.clone())
                .priority(composite_priority);
            if let Some(name) = composite_name.as_ref() {
                builder = builder.name(name.clone());
            }
            if let Some(description) = composite_description.as_ref() {
                builder = builder.description(description.clone());
            }
            builder
        });
        let components = ordered_component_builders
            .into_iter()
            .map(|(component_id, builder)| {
                let factory = Box::new(move || builder.clone()) as Box<TemplateFactory>;
                (component_id, factory)
            })
            .collect();

        Ok(Self {
            metadata,
            factory,
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

    /// Build a fresh scenario builder from the registered factory.
    ///
    /// # Returns
    ///
    /// A new [`ScenarioSpecBuilder`] for the full registered template.
    #[must_use]
    pub fn builder(&self) -> ScenarioSpecBuilder {
        (self.factory)()
    }

    /// Build a fresh component builder by component identifier.
    ///
    /// # Arguments
    ///
    /// - `id`: Component identifier listed in [`TemplateMetadata::components`].
    ///
    /// # Returns
    ///
    /// `Some(builder)` when a matching component exists, otherwise `None`.
    #[must_use]
    pub fn component(&self, id: &str) -> Option<ScenarioSpecBuilder> {
        self.components.get(id).map(|factory| factory())
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

/// Registry of template metadata and builder factories.
///
/// The registry preserves insertion order for listing and filtering operations
/// so discovery APIs remain deterministic across runs.
pub struct TemplateRegistry {
    entries: IndexMap<String, RegisteredTemplate>,
}

impl TemplateRegistry {
    /// Create an empty template registry with no built-in templates registered.
    ///
    /// Use [`Default::default()`] to obtain a registry preloaded with built-in
    /// historical stress templates.
    #[must_use]
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

    /// Register or replace a template and its builder factory.
    ///
    /// Registration clears `metadata.components` because this overload stores
    /// only a top-level builder factory.
    ///
    /// # Arguments
    ///
    /// - `metadata`: Discovery metadata for the template.
    /// - `factory`: Factory that produces a fresh builder on each call.
    pub fn register<F>(&mut self, metadata: TemplateMetadata, factory: F)
    where
        F: Fn() -> ScenarioSpecBuilder + Send + Sync + 'static,
    {
        let id = metadata.id.clone();
        let mut metadata = metadata;
        metadata.components.clear();
        self.entries.insert(
            id,
            RegisteredTemplate {
                metadata,
                factory: Box::new(factory),
                components: IndexMap::new(),
            },
        );
    }

    /// Register or replace a template with explicit component builder factories.
    ///
    /// # Arguments
    ///
    /// - `metadata`: Discovery metadata for the template.
    /// - `factory`: Factory for the composite template builder.
    /// - `components`: Component factories keyed by component identifier.
    pub fn register_with_components<F>(
        &mut self,
        metadata: TemplateMetadata,
        factory: F,
        components: Vec<(String, Box<TemplateFactory>)>,
    ) where
        F: Fn() -> ScenarioSpecBuilder + Send + Sync + 'static,
    {
        let id = metadata.id.clone();
        let mut metadata = metadata;
        let components: IndexMap<String, Box<TemplateFactory>> = components.into_iter().collect();
        metadata.components = components.keys().cloned().collect();
        self.entries.insert(
            id,
            RegisteredTemplate {
                metadata,
                factory: Box::new(factory),
                components,
            },
        );
    }

    /// Register or replace a template from a parsed JSON document.
    pub(crate) fn register_json_document(&mut self, document: JsonTemplateDocument) -> Result<()> {
        let entry = RegisteredTemplate::from_json_document(document)?;
        self.entries.insert(entry.metadata.id.clone(), entry);
        Ok(())
    }

    /// Parse, validate, and register a template from a runtime JSON string.
    ///
    /// # Arguments
    ///
    /// - `name`: Logical source name used in error messages.
    /// - `json`: JSON template document to parse and register.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`](crate::Error::Validation) if the JSON
    /// cannot be parsed, fails template validation, or duplicates an existing
    /// template identifier.
    pub fn register_json_template_str(&mut self, name: &str, json: &str) -> Result<()> {
        let document = parse_template_document(name, json)?;
        let entry = self.registered_runtime_json_entry(document)?;
        self.entries.insert(entry.metadata.id.clone(), entry);
        Ok(())
    }

    /// Load and register all runtime JSON templates from a directory.
    ///
    /// Only files with a lowercase `.json` extension are loaded. Files are
    /// processed in deterministic filename order.
    ///
    /// # Arguments
    ///
    /// - `path`: Directory containing runtime JSON template documents.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`](crate::Error::Validation) if the
    /// directory cannot be read, any JSON file cannot be parsed or validated,
    /// or duplicate template identifiers are found during the batch load.
    pub fn load_json_dir<P>(&mut self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let mut json_paths = fs::read_dir(path)
            .map_err(|error| {
                Error::validation(format!(
                    "failed to read JSON template directory '{}': {error}",
                    path.display()
                ))
            })?
            .map(|entry| {
                entry.map(|dir_entry| dir_entry.path()).map_err(|error| {
                    Error::validation(format!(
                        "failed to read JSON template directory '{}': {error}",
                        path.display()
                    ))
                })
            })
            .collect::<Result<Vec<_>>>()?;
        json_paths.retain(|json_path| {
            json_path.is_file()
                && json_path.extension().and_then(|ext| ext.to_str()) == Some("json")
        });
        json_paths.sort();

        let mut staged_entries = Vec::with_capacity(json_paths.len());
        let mut staged_ids = HashSet::with_capacity(json_paths.len());

        for json_path in json_paths {
            let json = fs::read_to_string(&json_path).map_err(|error| {
                Error::validation(format!(
                    "failed to read JSON template '{}': {error}",
                    json_path.display()
                ))
            })?;
            let name = json_path.display().to_string();
            let document = parse_template_document(&name, &json)?;
            let template_id = document.metadata.id.clone();
            if !staged_ids.insert(template_id.clone()) {
                return Err(Error::validation(format!(
                    "duplicate template ID '{template_id}' found in JSON directory load"
                )));
            }
            let entry = self.registered_runtime_json_entry(document)?;
            staged_entries.push((template_id, entry));
        }

        for (template_id, entry) in staged_entries {
            self.entries.insert(template_id, entry);
        }

        Ok(())
    }

    fn registered_runtime_json_entry(
        &self,
        document: JsonTemplateDocument,
    ) -> Result<RegisteredTemplate> {
        let template_id = document.metadata.id.clone();
        if self.entries.contains_key(&template_id) {
            return Err(Error::validation(format!(
                "template '{template_id}' is already registered"
            )));
        }
        RegisteredTemplate::from_json_document(document)
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

    /// List all registered template metadata in deterministic insertion order.
    ///
    /// # Returns
    ///
    /// Metadata references in the order the templates were registered.
    #[must_use]
    pub fn list(&self) -> Vec<&TemplateMetadata> {
        self.entries.values().map(|entry| &entry.metadata).collect()
    }

    /// Filter registered templates by tag in deterministic insertion order.
    ///
    /// # Arguments
    ///
    /// - `tag`: Exact tag to match.
    ///
    /// # Returns
    ///
    /// Matching templates in registry insertion order.
    #[must_use]
    pub fn filter_by_tag(&self, tag: &str) -> Vec<&TemplateMetadata> {
        self.entries
            .values()
            .filter(|entry| entry.metadata.tags.iter().any(|candidate| candidate == tag))
            .map(|entry| &entry.metadata)
            .collect()
    }

    /// Filter registered templates by asset class in deterministic insertion order.
    ///
    /// # Arguments
    ///
    /// - `asset_class`: Asset-class label to match.
    ///
    /// # Returns
    ///
    /// Matching templates in registry insertion order.
    #[must_use]
    pub fn filter_by_asset_class(&self, asset_class: AssetClass) -> Vec<&TemplateMetadata> {
        self.entries
            .values()
            .filter(|entry| entry.metadata.asset_classes.contains(&asset_class))
            .map(|entry| &entry.metadata)
            .collect()
    }

    /// Filter registered templates by severity in deterministic insertion order.
    ///
    /// # Arguments
    ///
    /// - `severity`: Severity level to match.
    ///
    /// # Returns
    ///
    /// Matching templates in registry insertion order.
    #[must_use]
    pub fn filter_by_severity(&self, severity: Severity) -> Vec<&TemplateMetadata> {
        self.entries
            .values()
            .filter(|entry| entry.metadata.severity == severity)
            .map(|entry| &entry.metadata)
            .collect()
    }
}

impl Default for TemplateRegistry {
    /// Create a registry preloaded with built-in historical stress templates.
    #[allow(clippy::unreachable)] // Embedded crate-owned templates are validated in tests.
    fn default() -> Self {
        Self::with_embedded_builtins().unwrap_or_else(|error| {
            unreachable!("crate-owned embedded templates must load successfully: {error}")
        })
    }
}

fn scenario_spec_to_builder(spec: ScenarioSpec) -> ScenarioSpecBuilder {
    let mut builder = ScenarioSpecBuilder::new(spec.id)
        .with_operations(spec.operations)
        .priority(spec.priority);
    if let Some(name) = spec.name {
        builder = builder.name(name);
    }
    if let Some(description) = spec.description {
        builder = builder.description(description);
    }
    builder
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::TemplateRegistry;
    use crate::templates::json::{JsonCompositeTemplate, JsonTemplateDocument};
    use crate::{
        AssetClass, CurveKind, OperationSpec, ScenarioSpec, ScenarioSpecBuilder, Severity,
        TemplateMetadata,
    };
    use indexmap::indexmap;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };
    use time::macros::date;

    fn metadata(
        id: &str,
        tag: &str,
        asset_class: AssetClass,
        severity: Severity,
        components: Vec<&str>,
    ) -> TemplateMetadata {
        TemplateMetadata {
            id: id.into(),
            name: format!("Template {id}"),
            description: format!("Description for {id}"),
            event_date: date!(2008 - 09 - 15),
            asset_classes: vec![asset_class],
            tags: vec![tag.into()],
            severity,
            components: components.into_iter().map(str::to_string).collect(),
        }
    }

    fn registry_with_templates() -> TemplateRegistry {
        let mut registry = TemplateRegistry::new();

        registry.register(
            metadata(
                "rates_shock",
                "systemic",
                AssetClass::Rates,
                Severity::Severe,
                vec![],
            ),
            || {
                ScenarioSpecBuilder::new("rates_shock").with_operation(
                    OperationSpec::CurveParallelBp {
                        curve_kind: CurveKind::Discount,
                        curve_id: "USD-SOFR".into(),
                        discount_curve_id: None,
                        bp: 100.0,
                    },
                )
            },
        );

        registry.register(
            metadata(
                "equity_shock",
                "equity",
                AssetClass::Equity,
                Severity::Moderate,
                vec![],
            ),
            || ScenarioSpecBuilder::new("equity_shock"),
        );

        registry.register_with_components(
            metadata(
                "hybrid_shock",
                "systemic",
                AssetClass::Credit,
                Severity::Mild,
                vec!["rates_shock", "equity_shock"],
            ),
            || ScenarioSpecBuilder::new("hybrid_shock"),
            vec![
                (
                    "rates_shock".into(),
                    Box::new(|| {
                        ScenarioSpecBuilder::new("rates_shock").with_operation(
                            OperationSpec::CurveParallelBp {
                                curve_kind: CurveKind::Discount,
                                curve_id: "USD-SOFR".into(),
                                discount_curve_id: None,
                                bp: 100.0,
                            },
                        )
                    }),
                ),
                (
                    "equity_shock".into(),
                    Box::new(|| ScenarioSpecBuilder::new("equity_shock")),
                ),
            ],
        );

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
            metadata: TemplateMetadata {
                id: "json_template".into(),
                name: "JSON Template".into(),
                description: "Template registered from JSON".into(),
                event_date: date!(2020 - 03 - 16),
                asset_classes: vec![AssetClass::Rates, AssetClass::Equity],
                tags: vec!["systemic".into(), "json".into()],
                severity: Severity::Severe,
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
            metadata: TemplateMetadata {
                id: "priority_order_conflict".into(),
                name: "Priority Order Conflict".into(),
                description: "JSON order should beat component priority".into(),
                event_date: date!(2020 - 03 - 16),
                asset_classes: vec![AssetClass::Rates],
                tags: vec!["json".into()],
                severity: Severity::Moderate,
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
            metadata: TemplateMetadata {
                id: "no_composite_name".into(),
                name: "No Composite Name".into(),
                description: "Composite name omitted in JSON".into(),
                event_date: date!(2020 - 03 - 16),
                asset_classes: vec![AssetClass::Rates],
                tags: vec!["json".into()],
                severity: Severity::Mild,
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

    fn runtime_json_document(
        template_id: &str,
        component_id: &str,
        curve_id: &str,
    ) -> JsonTemplateDocument {
        JsonTemplateDocument {
            metadata: TemplateMetadata {
                id: template_id.into(),
                name: format!("Runtime Template {template_id}"),
                description: format!("Runtime template loaded for {template_id}"),
                event_date: date!(2021 - 01 - 01),
                asset_classes: vec![AssetClass::Rates],
                tags: vec!["runtime".into()],
                severity: Severity::Moderate,
                components: vec![component_id.into()],
            },
            components: indexmap! {
                component_id.into() => json_component_spec(component_id, curve_id, 12.5),
            },
            composite: JsonCompositeTemplate::new(
                template_id,
                Some(&format!("Composite {template_id}")),
                Some(&format!("Composite description for {template_id}")),
                4,
                vec![component_id.into()],
            ),
        }
    }

    fn json_string(document: &JsonTemplateDocument) -> String {
        serde_json::to_string(document).expect("json document should serialize")
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let unique = format!(
                "finstack-quant-scenarios-{prefix}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("current time should be after UNIX_EPOCH")
                    .as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            fs::create_dir(&path).expect("test temp directory should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, file_name: &str, contents: &str) {
            fs::write(self.path.join(file_name), contents).expect("test file should be written");
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn register_and_get() {
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
    fn filter_by_tag() {
        let registry = registry_with_templates();

        assert_eq!(
            collected_ids(registry.filter_by_tag("systemic")),
            vec!["rates_shock", "hybrid_shock"]
        );
    }

    #[test]
    fn filter_by_asset_class() {
        let registry = registry_with_templates();

        assert_eq!(
            collected_ids(registry.filter_by_asset_class(AssetClass::Equity)),
            vec!["equity_shock"]
        );
    }

    #[test]
    fn filter_by_severity() {
        let registry = registry_with_templates();

        assert_eq!(
            collected_ids(registry.filter_by_severity(Severity::Severe)),
            vec!["rates_shock"]
        );
    }

    #[test]
    fn build_produces_fresh_builders() {
        let registry = registry_with_templates();
        let entry = registry
            .get("rates_shock")
            .expect("template entry should exist");

        let overridden = entry
            .builder()
            .override_curve("USD-SOFR", "CUSTOM-SOFR")
            .build()
            .expect("builder should build");

        let original = entry.builder().build().expect("builder should build");

        match &overridden.operations[0] {
            OperationSpec::CurveParallelBp { curve_id, .. } => {
                assert_eq!(curve_id, "CUSTOM-SOFR");
            }
            _ => panic!("unexpected operation"),
        }

        match &original.operations[0] {
            OperationSpec::CurveParallelBp { curve_id, .. } => {
                assert_eq!(curve_id, "USD-SOFR");
            }
            _ => panic!("unexpected operation"),
        }
    }

    #[test]
    fn register_with_components() {
        let registry = registry_with_templates();
        let entry = registry
            .get("hybrid_shock")
            .expect("template entry should exist");

        assert_eq!(entry.component_ids(), vec!["rates_shock", "equity_shock"]);

        let rates = entry
            .component("rates_shock")
            .expect("rates component should exist")
            .build()
            .expect("component should build");
        let equity = entry
            .component("equity_shock")
            .expect("equity component should exist")
            .build()
            .expect("component should build");

        assert_eq!(rates.id, "rates_shock");
        assert_eq!(equity.id, "equity_shock");
    }

    #[test]
    fn default_registry_registers_all_embedded_builtins() {
        let registry = TemplateRegistry::default();
        assert_eq!(collected_ids(registry.list()), builtin_template_ids());
    }

    #[test]
    fn with_embedded_builtins_loads_builtins_without_default() {
        let registry =
            TemplateRegistry::with_embedded_builtins().expect("embedded builtins should load");

        assert_eq!(collected_ids(registry.list()), builtin_template_ids());
    }

    #[test]
    fn default_registry_builds_all_builtins_and_components() {
        let registry = TemplateRegistry::default();
        for template_id in builtin_template_ids() {
            let entry = registry
                .get(template_id)
                .expect("builtin template should be registered");
            let scenario = entry.builder().build().expect("scenario should build");

            assert_eq!(scenario.id, template_id);
            assert_eq!(entry.component_ids().len(), 5);

            for component_id in entry.component_ids() {
                let component = entry
                    .component(component_id)
                    .expect("component should exist")
                    .build()
                    .expect("component should build");
                assert_eq!(component.id, component_id);
            }
        }
    }

    #[test]
    fn register_clears_metadata_components_when_no_component_factories_exist() {
        let mut registry = TemplateRegistry::new();

        registry.register(
            metadata(
                "standalone",
                "systemic",
                AssetClass::Rates,
                Severity::Mild,
                vec!["ghost_component"],
            ),
            || ScenarioSpecBuilder::new("standalone"),
        );

        let entry = registry
            .get("standalone")
            .expect("template entry should exist");

        assert!(entry.metadata().components.is_empty());
        assert!(entry.component("ghost_component").is_none());
    }

    #[test]
    fn register_with_components_normalizes_metadata_component_ids() {
        let mut registry = TemplateRegistry::new();

        registry.register_with_components(
            metadata(
                "normalized",
                "systemic",
                AssetClass::Credit,
                Severity::Moderate,
                vec!["wrong_component"],
            ),
            || ScenarioSpecBuilder::new("normalized"),
            vec![(
                "actual_component".into(),
                Box::new(|| ScenarioSpecBuilder::new("actual_component")),
            )],
        );

        let entry = registry
            .get("normalized")
            .expect("template entry should exist");

        assert_eq!(entry.metadata().components, vec!["actual_component"]);
        assert!(entry.component("actual_component").is_some());
        assert!(entry.component("wrong_component").is_none());
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
    fn register_json_document_component_builders_are_fresh_on_each_request() {
        let mut registry = TemplateRegistry::new();
        registry
            .register_json_document(json_document())
            .expect("json document should register");
        let entry = registry
            .get("json_template")
            .expect("json template should exist");

        let overridden = entry
            .component("component_b")
            .expect("component should exist")
            .override_curve("B-CURVE", "CUSTOM-CURVE")
            .build()
            .expect("component should build");
        let original = entry
            .component("component_b")
            .expect("component should exist")
            .build()
            .expect("component should build");

        match &overridden.operations[0] {
            OperationSpec::CurveParallelBp { curve_id, .. } => {
                assert_eq!(curve_id, "CUSTOM-CURVE");
            }
            _ => panic!("unexpected operation"),
        }

        match &original.operations[0] {
            OperationSpec::CurveParallelBp { curve_id, .. } => {
                assert_eq!(curve_id, "B-CURVE");
            }
            _ => panic!("unexpected operation"),
        }
    }

    #[test]
    fn register_json_document_composite_builder_uses_component_order_and_top_level_fields() {
        let mut registry = TemplateRegistry::new();
        registry
            .register_json_document(json_document())
            .expect("json document should register");
        let entry = registry
            .get("json_template")
            .expect("json template should exist");

        let scenario = entry.builder().build().expect("composite should build");

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

        let scenario = entry.builder().build().expect("composite should build");

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

        let scenario = entry.builder().build().expect("composite should build");

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
        assert_eq!(entry.metadata().severity, Severity::Severe);
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
    fn new_registry_is_empty_until_runtime_json_is_explicitly_loaded() {
        let registry = TemplateRegistry::new();

        assert!(registry.list().is_empty());
        assert!(registry.get("gfc_2008").is_none());
    }

    #[test]
    fn register_json_template_str_registers_valid_runtime_json_document() {
        let mut registry = TemplateRegistry::new();

        registry
            .register_json_template_str(
                "inline-template.json",
                &json_string(&runtime_json_document(
                    "runtime_inline",
                    "runtime_inline_component",
                    "INLINE-CURVE",
                )),
            )
            .expect("runtime json should register");

        let entry = registry
            .get("runtime_inline")
            .expect("runtime template should exist");

        assert_eq!(entry.metadata().id, "runtime_inline");
        assert_eq!(entry.component_ids(), vec!["runtime_inline_component"]);
    }

    #[test]
    fn register_json_template_str_rejects_invalid_json_with_clean_error() {
        let mut registry = TemplateRegistry::new();

        let error = registry
            .register_json_template_str("broken-template.json", "{ not valid json")
            .expect_err("invalid json should fail");

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error
            .to_string()
            .contains("failed to parse JSON template 'broken-template.json'"));
    }

    #[test]
    fn register_json_template_str_rejects_duplicate_template_ids() {
        let mut registry = TemplateRegistry::new();
        registry
            .register_json_template_str(
                "original-template.json",
                &json_string(&runtime_json_document(
                    "duplicate_runtime",
                    "duplicate_runtime_component",
                    "DUPLICATE-CURVE",
                )),
            )
            .expect("initial runtime json should register");

        let error = registry
            .register_json_template_str(
                "replacement-template.json",
                &json_string(&runtime_json_document(
                    "duplicate_runtime",
                    "replacement_component",
                    "REPLACEMENT-CURVE",
                )),
            )
            .expect_err("duplicate template id should fail");

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error
            .to_string()
            .contains("template 'duplicate_runtime' is already registered"));

        let entry = registry
            .get("duplicate_runtime")
            .expect("original template should still exist");
        assert_eq!(entry.component_ids(), vec!["duplicate_runtime_component"]);
    }

    #[test]
    fn load_json_dir_loads_multiple_files_in_deterministic_filename_order() {
        let mut registry = TemplateRegistry::new();
        let temp_dir = TestTempDir::new("ordered-runtime-json");
        temp_dir.write(
            "b_second.json",
            &json_string(&runtime_json_document(
                "loaded_from_b",
                "loaded_from_b_component",
                "B-CURVE",
            )),
        );
        temp_dir.write(
            "a_first.json",
            &json_string(&runtime_json_document(
                "loaded_from_a",
                "loaded_from_a_component",
                "A-CURVE",
            )),
        );

        registry
            .load_json_dir(temp_dir.path())
            .expect("json directory should load");

        assert_eq!(
            collected_ids(registry.list()),
            vec!["loaded_from_a", "loaded_from_b"]
        );
    }

    #[test]
    fn load_json_dir_ignores_non_json_files() {
        let mut registry = TemplateRegistry::new();
        let temp_dir = TestTempDir::new("ignore-non-json");
        temp_dir.write(
            "runtime.json",
            &json_string(&runtime_json_document(
                "loaded_runtime",
                "loaded_runtime_component",
                "RUNTIME-CURVE",
            )),
        );
        temp_dir.write("ignored.txt", "{ not valid json");

        registry
            .load_json_dir(temp_dir.path())
            .expect("json directory should load");

        assert_eq!(collected_ids(registry.list()), vec!["loaded_runtime"]);
        assert!(registry.get("ignored").is_none());
    }

    #[test]
    fn load_json_dir_rejects_duplicate_ids_within_directory_batch() {
        let mut registry = TemplateRegistry::new();
        let temp_dir = TestTempDir::new("duplicate-batch-json");
        temp_dir.write(
            "a_first.json",
            &json_string(&runtime_json_document(
                "duplicate_batch",
                "first_component",
                "FIRST-CURVE",
            )),
        );
        temp_dir.write(
            "b_second.json",
            &json_string(&runtime_json_document(
                "duplicate_batch",
                "second_component",
                "SECOND-CURVE",
            )),
        );

        let error = registry
            .load_json_dir(temp_dir.path())
            .expect_err("duplicate batch ids should fail");

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error
            .to_string()
            .contains("duplicate template ID 'duplicate_batch' found in JSON directory load"));
        assert!(registry.list().is_empty());
    }

    #[test]
    fn load_json_dir_leaves_registry_unchanged_when_a_later_file_fails() {
        let mut registry = TemplateRegistry::new();
        registry
            .register_json_template_str(
                "existing-template.json",
                &json_string(&runtime_json_document(
                    "existing_runtime",
                    "existing_runtime_component",
                    "EXISTING-CURVE",
                )),
            )
            .expect("initial runtime json should register");
        let temp_dir = TestTempDir::new("atomic-failure-json");
        temp_dir.write(
            "a_valid.json",
            &json_string(&runtime_json_document(
                "new_runtime",
                "new_runtime_component",
                "NEW-CURVE",
            )),
        );
        temp_dir.write("b_invalid.json", "{ invalid json");

        let error = registry
            .load_json_dir(temp_dir.path())
            .expect_err("directory load should fail");

        assert!(matches!(error, crate::Error::Validation(_)));
        assert!(error.to_string().contains("failed to parse JSON template"));
        assert_eq!(collected_ids(registry.list()), vec!["existing_runtime"]);
        let entry = registry
            .get("existing_runtime")
            .expect("existing template should remain");
        assert_eq!(entry.component_ids(), vec!["existing_runtime_component"]);
        assert!(registry.get("new_runtime").is_none());
    }

    #[test]
    fn default_registry_still_provides_builtins_without_runtime_loading() {
        let registry = TemplateRegistry::default();

        assert!(registry.get("gfc_2008").is_some());
        assert!(registry.get("covid_2020").is_some());
        assert!(registry.get("rate_shock_2022").is_some());
        assert!(registry.get("svb_2023").is_some());
        assert!(registry.get("ltcm_1998").is_some());
    }
}

//! Historical stress-template metadata, builders, and registry APIs.
//!
//! This module provides the reusable template layer that sits above raw
//! [`ScenarioSpec`](crate::ScenarioSpec) values. Most callers start with
//! [`TemplateRegistry`] to discover built-in or runtime-loaded templates and
//! then use [`ScenarioSpecBuilder`] to override identifiers before building a
//! concrete scenario.
//!
//! Built-in templates are embedded JSON documents shipped with the crate.
//! Runtime JSON templates use the same schema and can be loaded through
//! [`TemplateRegistry::register_json_template_str`] or
//! [`TemplateRegistry::load_json_dir`].
//!
//! For template discovery metadata, see [`TemplateMetadata`]. For scenario
//! execution, continue to [`crate::ScenarioEngine`].

mod builder;
mod json;
mod metadata;
mod registry;

pub use builder::ScenarioSpecBuilder;
pub use metadata::{AssetClass, Severity, TemplateMetadata};
pub use registry::{RegisteredTemplate, TemplateRegistry};

/// Register built-in templates into a registry.
fn register_builtins(registry: &mut TemplateRegistry) -> crate::Result<()> {
    let documents = json::load_embedded_documents()?;

    for document in documents {
        registry.register_json_document(document)?;
    }

    Ok(())
}

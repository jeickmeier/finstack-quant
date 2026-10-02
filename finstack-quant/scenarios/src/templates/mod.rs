//! Historical stress-template metadata and registry APIs.
//!
//! This module provides the reusable template layer that sits above raw
//! [`ScenarioSpec`] values. Most callers start with
//! [`TemplateRegistry`] to discover the built-in templates and then call
//! [`TemplateRegistry::build`] (or [`TemplateRegistry::build_component`]) to
//! get a concrete, validated scenario.
//!
//! Built-in templates are embedded JSON documents shipped with the crate and
//! are loaded through [`TemplateRegistry::with_embedded_builtins`].
//!
//! For template discovery metadata, see [`TemplateMetadata`]. For scenario
//! execution, continue to [`crate::ScenarioEngine`].

mod json;
mod metadata;
mod registry;

pub use metadata::{AssetClass, TemplateMetadata, TemplateSeverity};
pub use registry::{RegisteredTemplate, TemplateRegistry};

/// Register built-in templates into a registry.
fn register_builtins(registry: &mut TemplateRegistry) -> crate::Result<()> {
    let documents = json::load_embedded_documents()?;

    for document in documents {
        registry.register_json_document(document)?;
    }

    Ok(())
}

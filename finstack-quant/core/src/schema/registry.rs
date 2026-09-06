//! Schema artifact registry and rendering: [`SchemaArtifact`], [`generated_schema`]
//! and the shared common-definition table.
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};

use super::externalize::*;
use crate::{Error, Result};

/// JSON Schema dialect used by generated Finstack contracts.
pub const JSON_SCHEMA_DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

/// Stable base URI for shared schema definitions.
pub const COMMON_SCHEMA_BASE: &str = "https://finstack_quant.dev/schemas/common/1/";

/// Runtime serde contract that may own a generated JSON Schema artifact.
///
/// Requiring all three traits prevents schema-only shadow roots from entering
/// a registry: every root must be serializable, deserializable, and derivable.
pub trait SerdeSchema: Serialize + DeserializeOwned + JsonSchema {}

impl<T> SerdeSchema for T where T: Serialize + DeserializeOwned + JsonSchema {}

/// Typed common definitions eligible for assertion-checked externalization.
pub const COMMON_SCHEMA_DEFINITIONS: &[ExternalSchemaDefinition] = &[
    ExternalSchemaDefinition::new::<crate::types::Attributes>(
        "Attributes",
        "https://finstack_quant.dev/schemas/common/1/attributes.schema.json",
    ),
    ExternalSchemaDefinition::new::<crate::dates::BusinessDayConvention>(
        "BusinessDayConvention",
        "https://finstack_quant.dev/schemas/common/1/business_day_convention.schema.json",
    ),
    ExternalSchemaDefinition::new::<crate::currency::Currency>(
        "Currency",
        "https://finstack_quant.dev/schemas/common/1/currency.schema.json",
    ),
    ExternalSchemaDefinition::new::<crate::wire::DateWire>(
        "Date",
        "https://finstack_quant.dev/schemas/common/1/date.schema.json",
    ),
    ExternalSchemaDefinition::new::<crate::wire::DateWire>(
        "DateWire",
        "https://finstack_quant.dev/schemas/common/1/date.schema.json",
    ),
    ExternalSchemaDefinition::new::<crate::dates::DayCount>(
        "DayCount",
        "https://finstack_quant.dev/schemas/common/1/day_count.schema.json",
    ),
    ExternalSchemaDefinition::new::<crate::wire::DecimalWire>(
        "Decimal",
        "https://finstack_quant.dev/schemas/common/1/decimal.schema.json",
    ),
    ExternalSchemaDefinition::new::<crate::wire::DecimalWire>(
        "DecimalWire",
        "https://finstack_quant.dev/schemas/common/1/decimal.schema.json",
    ),
    ExternalSchemaDefinition::new::<crate::contract::Diagnostic>(
        "Diagnostic",
        "https://finstack_quant.dev/schemas/common/1/diagnostic.schema.json",
    ),
    ExternalSchemaDefinition::new::<crate::types::InstrumentId>(
        "Id",
        "https://finstack_quant.dev/schemas/common/1/id.schema.json",
    ),
    ExternalSchemaDefinition::new::<crate::money::Money>(
        "Money",
        "https://finstack_quant.dev/schemas/common/1/money.schema.json",
    ),
    ExternalSchemaDefinition::new::<crate::dates::Tenor>(
        "Tenor",
        "https://finstack_quant.dev/schemas/common/1/tenor.schema.json",
    ),
    ExternalSchemaDefinition::new::<crate::contract::ValidationReport>(
        "ValidationReport",
        "https://finstack_quant.dev/schemas/common/1/validation_report.schema.json",
    ),
];

/// Build a generated schema with stable canonical metadata.
///
/// The default `serde_json::Map` representation serializes keys in
/// lexicographic order, making pretty-printed output deterministic across
/// repeated runs. Validation assertions come directly from `T`; this helper
/// adds only stable document metadata.
///
/// # Arguments
///
/// * `schema_base` - Canonical URI prefix for the owning schema family,
///   including its trailing slash.
/// * `filename` - Version-directory filename appended to `schema_base`.
/// * `title` - Stable human-readable title for the generated root type.
/// * `description` - Stable description for the persisted contract.
///
/// # Errors
///
/// Returns [`Error::Internal`] if schemars output cannot be serialized as a
/// JSON object.
pub fn generated_schema<T: SerdeSchema>(
    schema_base: &str,
    filename: &str,
    title: &str,
    description: &str,
) -> Result<Value> {
    let generated = serde_json::to_value(schemars::schema_for!(T))
        .map_err(|error| Error::Internal(format!("serialize generated {title} schema: {error}")))?;
    let generated = generated.as_object().ok_or_else(|| {
        Error::Internal(format!("generated {title} schema must be a JSON object"))
    })?;
    let mut output = Map::new();
    output.insert(
        "$id".to_string(),
        Value::String(format!("{schema_base}{filename}")),
    );
    output.insert(
        "$schema".to_string(),
        Value::String(JSON_SCHEMA_DIALECT.to_string()),
    );
    output.insert("title".to_string(), Value::String(title.to_string()));
    output.insert(
        "description".to_string(),
        Value::String(description.to_string()),
    );
    for (key, value) in generated {
        if !matches!(key.as_str(), "$id" | "$schema" | "title" | "description") {
            output.insert(key.clone(), value.clone());
        }
    }
    Ok(Value::Object(output))
}

/// Direction of travel for a generated contract.
///
/// A consumer choosing what to send or how to read a reply needs this before it
/// needs anything else, and it cannot be derived from the schema document.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SchemaKind {
    /// A root document callers author and submit.
    Input,
    /// A root document the library emits and callers read.
    Output,
    /// A reusable definition referenced by roots, not submitted on its own.
    Component,
}

impl SchemaKind {
    /// Return the stable snake_case label used in the schema index.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
            Self::Component => "component",
        }
    }
}

/// One generated JSON Schema artifact owned by a Rust contract type.
///
/// The registry stores only stable metadata and a monomorphized
/// `schema_for!(T)` function. Existing checked-in JSON is never consulted when
/// generating the document.
#[derive(Clone, Copy)]
pub struct SchemaArtifact {
    /// Path relative to the owning crate's manifest directory.
    pub relative_path: &'static str,
    /// Canonical absolute `$id` for the schema document.
    pub id: &'static str,
    /// Stable human-readable title.
    pub title: &'static str,
    /// Stable contract description.
    pub description: &'static str,
    /// One-line summary for the schema index, or empty to reuse `description`.
    pub summary: &'static str,
    /// Whether callers author this document, read it, or only reference it.
    pub kind: SchemaKind,
    generator: fn(&SchemaArtifact) -> Result<Value>,
    examples: fn() -> Result<Vec<Value>>,
    packager: fn(&mut Value) -> Result<()>,
}

impl SchemaArtifact {
    /// Register a schema artifact generated directly from `T`.
    ///
    /// # Arguments
    ///
    /// * `relative_path` - Destination path relative to the owning crate.
    /// * `id` - Canonical absolute JSON Schema identifier.
    /// * `title` - Stable human-readable schema title.
    /// * `description` - Stable description of the persisted contract.
    #[must_use]
    pub const fn new<T: SerdeSchema>(
        relative_path: &'static str,
        id: &'static str,
        title: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            relative_path,
            id,
            title,
            description,
            summary: "",
            kind: SchemaKind::Component,
            generator: generate_artifact::<T>,
            examples: empty_examples,
            packager: no_op_packager,
        }
    }

    /// Declare this artifact a root document callers author or read.
    ///
    /// Registration defaults to [`SchemaKind::Component`], so only roots need
    /// to say so.
    ///
    /// # Arguments
    ///
    /// * `kind` - Direction of travel for this contract.
    #[must_use]
    pub const fn with_kind(mut self, kind: SchemaKind) -> Self {
        self.kind = kind;
        self
    }

    /// Attach a one-line summary for the schema index.
    ///
    /// Titles alone do not distinguish sibling contracts — every one of the
    /// seventy instrument artifacts shares a single generated description — so
    /// the index carries a per-artifact line instead.
    ///
    /// # Arguments
    ///
    /// * `summary` - One sentence stating what this contract governs.
    #[must_use]
    pub const fn with_summary(mut self, summary: &'static str) -> Self {
        self.summary = summary;
        self
    }

    /// Return the index summary, falling back to the contract description.
    #[must_use]
    pub const fn index_summary(&self) -> &'static str {
        if self.summary.is_empty() {
            self.description
        } else {
            self.summary
        }
    }

    /// Attach deterministic examples serialized from Rust values.
    ///
    /// # Arguments
    ///
    /// * `examples` - Function returning stable JSON examples for this
    ///   artifact. It must not read generated output or use time, randomness,
    ///   or environment-dependent values.
    #[must_use]
    pub const fn with_examples(mut self, examples: fn() -> Result<Vec<Value>>) -> Self {
        self.examples = examples;
        self
    }

    /// Attach a deterministic packaging pass that preserves validation assertions.
    ///
    /// # Arguments
    ///
    /// * `packager` - Function that may externalize equivalent `$defs`
    ///   references and prune definitions made unreachable by that rewrite.
    #[must_use]
    pub const fn with_packager(mut self, packager: fn(&mut Value) -> Result<()>) -> Self {
        self.packager = packager;
        self
    }

    /// Render this artifact exactly as the checked-in file is written.
    ///
    /// This is the single rendering path: registry metadata, the packager, the
    /// single-branch-union collapse, examples and key sorting all apply. Tests,
    /// bindings and tools must call this rather than `generated_schema`, which
    /// produces only the raw derived document and will drift from the artifact.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Internal`] if the schema or its examples cannot be
    /// built.
    pub fn generate(&self) -> Result<Value> {
        (self.generator)(self)
    }
}

pub(super) fn empty_examples() -> Result<Vec<Value>> {
    Ok(Vec::new())
}

pub(super) fn no_op_packager(_: &mut Value) -> Result<()> {
    Ok(())
}

/// Keywords that annotate a schema without constraining any instance.
///
/// A node carrying only these plus `oneOf` contributes no assertion of its own,
/// which is what makes collapsing its single branch safe.
pub(super) const ANNOTATION_KEYWORDS: &[&str] = &[
    "$comment",
    "default",
    "deprecated",
    "description",
    "examples",
    "readOnly",
    "title",
    "writeOnly",
];

/// Replace every single-branch `oneOf` with the branch itself.
///
/// schemars emits a one-variant enum as `{"oneOf": [branch]}`. That is
/// logically identical to `branch`, but it is not equivalent in *diagnostics*:
/// a validator reports a failing `oneOf` at the union node, so an error inside
/// the branch surfaces as "no branch matched" against the whole subtree instead
/// of pointing at the offending field. Seventy single-instrument schemas wrap
/// their payload this way, which is why a bond missing `maturity` reports at
/// `/instrument` with the entire instance attached.
///
/// The rewrite only fires when the wrapper carries no assertion of its own -
/// every sibling key must be in [`ANNOTATION_KEYWORDS`] - so the collapsed node
/// asserts exactly what the branch asserted. Sibling annotations win over the
/// branch's, keeping the field-level documentation that the wrapper carries.
pub(super) fn collapse_single_branch_unions(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for nested in object.values_mut() {
                collapse_single_branch_unions(nested);
            }

            let collapsible = object
                .get("oneOf")
                .and_then(Value::as_array)
                .is_some_and(|branches| branches.len() == 1 && branches[0].is_object())
                && object
                    .keys()
                    .all(|key| key == "oneOf" || ANNOTATION_KEYWORDS.contains(&key.as_str()));
            if !collapsible {
                return;
            }

            let Some(Value::Array(mut branches)) = object.remove("oneOf") else {
                return;
            };
            let Some(Value::Object(branch)) = branches.pop() else {
                return;
            };
            for (key, nested) in branch {
                object.entry(key).or_insert(nested);
            }
        }
        Value::Array(items) => {
            for item in items {
                collapse_single_branch_unions(item);
            }
        }
        _ => {}
    }
}

pub(super) fn generate_artifact<T: SerdeSchema>(artifact: &SchemaArtifact) -> Result<Value> {
    let (schema_base, filename) = artifact.id.rsplit_once('/').ok_or_else(|| {
        Error::Internal(format!(
            "schema id must contain a filename: {}",
            artifact.id
        ))
    })?;
    let mut schema = generated_schema::<T>(
        &format!("{schema_base}/"),
        filename,
        artifact.title,
        artifact.description,
    )?;
    (artifact.packager)(&mut schema)?;
    collapse_single_branch_unions(&mut schema);
    let examples = (artifact.examples)()?;
    if !examples.is_empty() {
        schema
            .as_object_mut()
            .ok_or_else(|| Error::Internal("generated schema must be an object".to_string()))?
            .insert("examples".to_string(), Value::Array(examples));
    }
    Ok(schema)
}

/// Find an artifact by exact identifier, path, or complete trailing filename.
///
/// # Arguments
///
/// * `artifacts` - Registry entries to search in their published order.
/// * `selector` - Exact schema `$id`, relative path, or trailing filename.
///   Empty strings and partial filename suffixes do not match.
///
/// # Errors
///
/// Returns an error when no entry matches the selector.
pub fn find_schema_artifact<'a>(
    artifacts: impl IntoIterator<Item = &'a SchemaArtifact>,
    selector: &str,
) -> Result<&'a SchemaArtifact> {
    let anchored = format!("/{selector}");
    artifacts
        .into_iter()
        .find(|artifact| {
            artifact.id == selector
                || artifact.relative_path == selector
                || artifact.relative_path.ends_with(&anchored)
        })
        .ok_or_else(|| {
            Error::Internal(format!(
                "no schema matches {selector:?}; call index() for published artifacts"
            ))
        })
}

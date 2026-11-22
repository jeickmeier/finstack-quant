//! Registry-backed JSON Schema access for every `*.schema` facade namespace.
//!
//! Each domain crate publishes its wire contracts through a `SchemaArtifact`
//! registry; the umbrella crate merges the eleven registries. The schemas are
//! rendered from those registries on demand and cached in-process, so a schema
//! read here always describes the wire format this build accepts.
//!
//! wasm-bindgen exports share one flat namespace, so per-crate
//! `index` / `get` / `validate` share three domain-parameterized Rust exports
//! (`domainSchemaIndex`, `domainSchemaGet`, `domainSchemaValidate`) and the JS
//! facade curries each crate's domain name through `exports/domainSchema.js`
//! into `<domain>.schema.{index,get,validate}`, mirroring Python
//! `finstack_quant.<domain>.schema`. Python returns JSON text from these
//! accessors; WASM returns the same documents as plain objects.

use crate::utils::input::{from_js_json, js_opt_string, js_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::schema::SchemaArtifact;
use indexmap::IndexMap;
use wasm_bindgen::prelude::*;

/// Profile rendered when `get` is called without one.
const DEFAULT_PROFILE: &str = "canonical";

/// Render one artifact in the requested (or canonical) profile.
fn render(artifact: &SchemaArtifact, profile: Option<JsValue>) -> Result<JsValue, JsValue> {
    let profile = js_opt_string(profile.as_ref(), "profile")?;
    let document = finstack_quant::schema::render_profile(
        artifact,
        profile.as_deref().unwrap_or(DEFAULT_PROFILE),
    )
    .map_err(to_js_err)?;
    to_js_value(&document)
}

/// Validate a payload against one artifact and report its failures.
fn report(artifact: &SchemaArtifact, payload: &JsValue) -> Result<JsValue, JsValue> {
    let payload: serde_json::Value = from_js_json(payload, "payload")?;
    let failures = finstack_quant::schema::validate(artifact, &payload).map_err(to_js_err)?;
    to_js_value(&failures)
}

// ---------------------------------------------------------------------------
// Per-domain registry (`<domain>.schema.*`, via `exports/domainSchema.js`)
// ---------------------------------------------------------------------------

/// List every JSON Schema published by one domain crate.
///
/// Each row carries `path`, `$id`, `title`, `type_name` (the Rust root type),
/// `summary`, `bytes` and `kind` (`input` for documents you author, `output`
/// for documents the library emits, `component` for shared definitions).
/// @param domain - Registered domain name from `schema.domains()`, e.g. `"core"`, `"valuations"`, or `"factor_model"`.
/// @returns `{ artifacts, schema_index_version }`, with one row per published schema.
/// @throws Error - Throws with kind `not_found` if `domain` is not a registered schema domain, kind `invalid_type` if `domain` is not a string, and kind `internal` if a schema cannot be rendered (does not occur for a released build).
#[wasm_bindgen(js_name = domainSchemaIndex, unchecked_return_type = "SchemaIndex")]
pub fn domain_schema_index(
    #[wasm_bindgen(unchecked_param_type = "string")] domain: JsValue,
) -> Result<JsValue, JsValue> {
    let domain = js_string(&domain, "domain")?;
    to_js_value(&finstack_quant::schema::index_for_domain(&domain).map_err(to_js_err)?)
}

/// Fetch one published JSON Schema of a domain crate.
/// @param domain - Registered domain name from `schema.domains()`, e.g. `"core"`, `"valuations"`, or `"factor_model"`.
/// @param selector - Schema `$id`, registry path or trailing filename from `index()`, e.g. `"market_context_state.schema.json"`. Partial filenames do not match.
/// @param profile - `"canonical"` (the default) for the validation contract, or `"llm"` for a self-contained projection with cross-document references inlined, intended for structured-output generation.
/// @returns JSON Schema document.
/// @throws Error - Throws with kind `not_found` if `domain` is not a registered schema domain or no schema in that domain matches `selector`, kind `validation` if `profile` is unknown, and kind `invalid_type` if any supplied argument is not a string.
#[wasm_bindgen(js_name = domainSchemaGet, unchecked_return_type = "Record<string, unknown>")]
pub fn domain_schema_get(
    #[wasm_bindgen(unchecked_param_type = "string")] domain: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] selector: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "SchemaProfile")] profile: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let domain = js_string(&domain, "domain")?;
    let selector = js_string(&selector, "selector")?;
    let registry = finstack_quant::schema::domain_registry(&domain).map_err(to_js_err)?;
    let artifact = finstack_quant_core::schema::find_schema_artifact(registry, &selector)
        .map_err(to_js_err)?;
    render(artifact, profile)
}

/// Validate a payload against one published JSON Schema of a domain crate.
///
/// A failure inside a tagged union is reported at the offending field of the
/// branch the payload most nearly matches, not at the union.
/// @param domain - Registered domain name from `schema.domains()`, e.g. `"core"`, `"valuations"`, or `"factor_model"`.
/// @param selector - Schema `$id`, registry path or trailing filename from `index()`.
/// @param payload - Document to check, as a plain object/array or as JSON text. Pass JSON text for a scalar payload (`'"USD"'`, `'1.5'`).
/// @returns One `{ pointer, message }` row per failure, where `pointer` is the JSON Pointer of the offending value; empty when the payload is valid.
/// @throws Error - Throws with kind `not_found` if `domain` is not a registered schema domain or no schema in that domain matches `selector`, kind `validation` if `payload` is JSON text that does not parse, and kind `invalid_type` if `domain` or `selector` is not a string or `payload` is neither JSON text nor a plain object/array.
#[wasm_bindgen(js_name = domainSchemaValidate, unchecked_return_type = "SchemaValidationFailure[]")]
pub fn domain_schema_validate(
    #[wasm_bindgen(unchecked_param_type = "string")] domain: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] selector: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string | object")] payload: JsValue,
) -> Result<JsValue, JsValue> {
    let domain = js_string(&domain, "domain")?;
    let selector = js_string(&selector, "selector")?;
    let registry = finstack_quant::schema::domain_registry(&domain).map_err(to_js_err)?;
    let artifact = finstack_quant_core::schema::find_schema_artifact(registry, &selector)
        .map_err(to_js_err)?;
    report(artifact, &payload)
}

// ---------------------------------------------------------------------------
// Whole-workspace registry (`schema.*`, Python `finstack_quant.schema`)
// ---------------------------------------------------------------------------

/// List every JSON Schema the workspace publishes, across all domains.
///
/// Each row carries `domain` alongside `path`, `$id`, `title`, `type_name`
/// (the Rust root type), `summary`, `bytes` and `kind`. Rows are sorted by
/// domain, then path.
/// @returns `{ artifacts, schema_index_version }`, with one row per published schema.
/// @throws Error - Throws with kind `internal` if a schema cannot be rendered (does not occur for a released build).
#[wasm_bindgen(js_name = schemaIndex, unchecked_return_type = "SchemaIndex")]
pub fn schema_index() -> Result<JsValue, JsValue> {
    to_js_value(&finstack_quant::schema::index().map_err(to_js_err)?)
}

/// Fetch one published JSON Schema from any domain.
/// @param selector - Schema `$id`, registry path or trailing filename from `index()`, e.g. `"bond.schema.json"`. Partial filenames do not match.
/// @param profile - `"canonical"` (the default) for the validation contract, or `"llm"` for a self-contained projection with cross-document references inlined, intended for structured-output generation.
/// @returns JSON Schema document.
/// @throws Error - Throws with kind `not_found` if no schema matches `selector`, kind `validation` if `profile` is unknown, and kind `invalid_type` if either argument is not a string.
#[wasm_bindgen(js_name = schemaGet, unchecked_return_type = "Record<string, unknown>")]
pub fn schema_get(
    #[wasm_bindgen(unchecked_param_type = "string")] selector: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "SchemaProfile")] profile: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let selector = js_string(&selector, "selector")?;
    render(
        finstack_quant::schema::find(&selector).map_err(to_js_err)?,
        profile,
    )
}

/// Validate a payload against one published JSON Schema from any domain.
///
/// A failure inside a tagged union is reported at the offending field of the
/// branch the payload most nearly matches, not at the union.
/// @param selector - Schema `$id`, registry path or trailing filename from `index()`.
/// @param payload - Document to check, as a plain object/array or as JSON text. Pass JSON text for a scalar payload (`'"USD"'`, `'1.5'`).
/// @returns One `{ pointer, message }` row per failure, where `pointer` is the JSON Pointer of the offending value; empty when the payload is valid.
/// @throws Error - Throws with kind `not_found` if no schema matches `selector`, kind `validation` if `payload` is JSON text that does not parse, and kind `invalid_type` if `selector` is not a string or `payload` is neither JSON text nor a plain object/array.
#[wasm_bindgen(js_name = schemaValidate, unchecked_return_type = "SchemaValidationFailure[]")]
pub fn schema_validate(
    #[wasm_bindgen(unchecked_param_type = "string")] selector: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string | object")] payload: JsValue,
) -> Result<JsValue, JsValue> {
    let selector = js_string(&selector, "selector")?;
    report(
        finstack_quant::schema::find(&selector).map_err(to_js_err)?,
        &payload,
    )
}

/// Domains that publish schemas, sorted.
/// @returns Domain names, each a `domain` value of the `index()` rows, e.g. `"core"`, `"valuations"`.
#[wasm_bindgen(js_name = schemaDomains)]
pub fn schema_domains() -> Vec<String> {
    finstack_quant::schema::domains()
        .into_iter()
        .map(str::to_string)
        .collect()
}

// ---------------------------------------------------------------------------
// Named accessors of single crates
// ---------------------------------------------------------------------------

/// The embedded cashflow schemas, keyed by `$id`.
///
/// These are the documents a JSON Schema resolver needs to follow the
/// cross-document `$ref`s of the cashflow contracts.
/// @returns Object mapping each schema `$id` to its JSON Schema document, in registry order.
/// @throws Error - Throws with kind `validation` if an embedded schema is malformed (does not occur for a released build).
#[wasm_bindgen(js_name = cashflowsSchemaResources, unchecked_return_type = "Record<string, Record<string, unknown>>")]
pub fn cashflows_schema_resources() -> Result<JsValue, JsValue> {
    let resources = finstack_quant_cashflows::schema::resources().map_err(to_js_err)?;
    let documents: IndexMap<&str, &serde_json::Value> = resources
        .iter()
        .map(|(uri, resource)| (uri.as_str(), resource.contents()))
        .collect();
    to_js_value(&documents)
}

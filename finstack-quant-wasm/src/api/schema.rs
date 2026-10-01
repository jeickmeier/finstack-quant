//! Registry-backed JSON Schema access for every `*.schema` facade namespace.
//!
//! Each domain crate publishes its wire contracts through a `SchemaArtifact`
//! registry; the umbrella crate merges the ten registries. The schemas are
//! rendered from those registries on demand, so a schema read here always
//! describes the wire format this build accepts.
//!
//! wasm-bindgen exports share one flat namespace, so the per-crate
//! `index` / `get` / `validate` triple is exported under crate-prefixed names
//! (`coreSchemaIndex`, …) and the JS facade maps them back to
//! `core.schema.index`, mirroring Python `finstack_quant.core.schema.index`.
//! Python returns JSON text from these accessors; WASM returns the same
//! documents as plain objects.

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
    to_js_value(&finstack_quant::schema::failures_to_value(&failures))
}

/// Emit the `index` / `get` / `validate` exports of one crate registry.
///
/// `$registry` is the crate's `&[SchemaArtifact]`; each function is given as
/// its Rust name and its flat wasm-bindgen export name.
macro_rules! schema_registry {
    (
        $registry:expr,
        $index:ident = $index_js:literal,
        $get:ident = $get_js:literal,
        $validate:ident = $validate_js:literal
    ) => {
        /// List every JSON Schema this crate publishes.
        ///
        /// Each row carries `path`, `$id`, `title`, `type_name` (the Rust root
        /// type), `summary`, `bytes` and `kind` (`input` for documents you
        /// author, `output` for documents the library emits, `component` for
        /// shared definitions).
        /// @returns `{ artifacts, schema_index_version }`, with one row per published schema.
        /// @throws Error - Throws with kind `internal` if a schema cannot be rendered (does not occur for a released build).
        #[wasm_bindgen(js_name = $index_js)]
        pub fn $index() -> Result<JsValue, JsValue> {
            to_js_value(
                &finstack_quant_core::schema::build_schema_index($registry).map_err(to_js_err)?,
            )
        }

        /// Fetch one published JSON Schema of this crate.
        /// @param selector - Schema `$id`, registry path or trailing filename from `index()`, e.g. `"market_context_state.schema.json"`. Partial filenames do not match.
        /// @param profile - `"canonical"` (the default) for the validation contract, or `"llm"` for a self-contained projection with cross-document references inlined, intended for structured-output generation.
        /// @returns JSON Schema document.
        /// @throws Error - Throws with kind `not_found` if no schema matches `selector`, kind `validation` if `profile` is unknown, and kind `invalid_type` if either argument is not a string.
        #[wasm_bindgen(js_name = $get_js)]
        pub fn $get(selector: JsValue, profile: Option<JsValue>) -> Result<JsValue, JsValue> {
            let selector = js_string(&selector, "selector")?;
            let artifact = finstack_quant_core::schema::find_schema_artifact($registry, &selector)
                .map_err(to_js_err)?;
            render(artifact, profile)
        }

        /// Validate a payload against one published JSON Schema of this crate.
        ///
        /// A failure inside a tagged union is reported at the offending field
        /// of the branch the payload most nearly matches, not at the union.
        /// @param selector - Schema `$id`, registry path or trailing filename from `index()`.
        /// @param payload - Document to check, as a plain object/array or as JSON text. Pass JSON text for a scalar payload (`'"USD"'`, `'1.5'`).
        /// @returns One `{ pointer, message }` row per failure, where `pointer` is the JSON Pointer of the offending value; empty when the payload is valid.
        /// @throws Error - Throws with kind `not_found` if no schema matches `selector`, kind `validation` if `payload` is JSON text that does not parse, and kind `invalid_type` if `selector` is not a string or `payload` is neither JSON text nor a plain object/array.
        #[wasm_bindgen(js_name = $validate_js)]
        pub fn $validate(selector: JsValue, payload: JsValue) -> Result<JsValue, JsValue> {
            let selector = js_string(&selector, "selector")?;
            let artifact = finstack_quant_core::schema::find_schema_artifact($registry, &selector)
                .map_err(to_js_err)?;
            report(artifact, &payload)
        }
    };
}

schema_registry!(
    finstack_quant_core::schema::ARTIFACTS,
    core_schema_index = "coreSchemaIndex",
    core_schema_get = "coreSchemaGet",
    core_schema_validate = "coreSchemaValidate"
);
schema_registry!(
    finstack_quant_attribution::schema::ARTIFACTS,
    attribution_schema_index = "attributionSchemaIndex",
    attribution_schema_get = "attributionSchemaGet",
    attribution_schema_validate = "attributionSchemaValidate"
);
schema_registry!(
    finstack_quant_calibration::json_schema::artifacts(),
    calibration_schema_index = "calibrationSchemaIndex",
    calibration_schema_get = "calibrationSchemaGet",
    calibration_schema_validate = "calibrationSchemaValidate"
);
schema_registry!(
    finstack_quant_cashflows::schema::ARTIFACTS,
    cashflows_schema_index = "cashflowsSchemaIndex",
    cashflows_schema_get = "cashflowsSchemaGet",
    cashflows_schema_validate = "cashflowsSchemaValidate"
);
schema_registry!(
    finstack_quant_margin::schema::ARTIFACTS,
    margin_schema_index = "marginSchemaIndex",
    margin_schema_get = "marginSchemaGet",
    margin_schema_validate = "marginSchemaValidate"
);
schema_registry!(
    finstack_quant_models::factor::schema::ARTIFACTS,
    factor_schema_index = "factorSchemaIndex",
    factor_schema_get = "factorSchemaGet",
    factor_schema_validate = "factorSchemaValidate"
);
schema_registry!(
    finstack_quant_portfolio::schema::ARTIFACTS,
    portfolio_schema_index = "portfolioSchemaIndex",
    portfolio_schema_get = "portfolioSchemaGet",
    portfolio_schema_validate = "portfolioSchemaValidate"
);
schema_registry!(
    finstack_quant_scenarios::schema::ARTIFACTS,
    scenarios_schema_index = "scenariosSchemaIndex",
    scenarios_schema_get = "scenariosSchemaGet",
    scenarios_schema_validate = "scenariosSchemaValidate"
);
schema_registry!(
    finstack_quant_statements::schema::ARTIFACTS,
    statements_schema_index = "statementsSchemaIndex",
    statements_schema_get = "statementsSchemaGet",
    statements_schema_validate = "statementsSchemaValidate"
);
schema_registry!(
    finstack_quant_valuations::schema::artifacts_slice(),
    valuations_schema_index = "valuationsSchemaIndex",
    valuations_schema_get = "valuationsSchemaGet",
    valuations_schema_validate = "valuationsSchemaValidate"
);

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
#[wasm_bindgen(js_name = schemaIndex)]
pub fn schema_index() -> Result<JsValue, JsValue> {
    to_js_value(&finstack_quant::schema::index().map_err(to_js_err)?)
}

/// Fetch one published JSON Schema from any domain.
/// @param selector - Schema `$id`, registry path or trailing filename from `index()`, e.g. `"bond.schema.json"`. Partial filenames do not match.
/// @param profile - `"canonical"` (the default) for the validation contract, or `"llm"` for a self-contained projection with cross-document references inlined, intended for structured-output generation.
/// @returns JSON Schema document.
/// @throws Error - Throws with kind `not_found` if no schema matches `selector`, kind `validation` if `profile` is unknown, and kind `invalid_type` if either argument is not a string.
#[wasm_bindgen(js_name = schemaGet)]
pub fn schema_get(selector: JsValue, profile: Option<JsValue>) -> Result<JsValue, JsValue> {
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
#[wasm_bindgen(js_name = schemaValidate)]
pub fn schema_validate(selector: JsValue, payload: JsValue) -> Result<JsValue, JsValue> {
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
#[wasm_bindgen(js_name = cashflowsSchemaResources)]
pub fn cashflows_schema_resources() -> Result<JsValue, JsValue> {
    let resources = finstack_quant_cashflows::schema::resources().map_err(to_js_err)?;
    let documents: IndexMap<&str, &serde_json::Value> = resources
        .iter()
        .map(|(uri, resource)| (uri.as_str(), resource.contents()))
        .collect();
    to_js_value(&documents)
}

/// JSON Schema of a versioned factor-model configuration (`FactorModelConfig`).
/// @returns JSON Schema document.
/// @throws Error - Throws with kind `validation` if the embedded schema is malformed (does not occur for a released build).
#[wasm_bindgen(js_name = factorModelConfigSchema)]
pub fn factor_model_config_schema() -> Result<JsValue, JsValue> {
    to_js_value(
        finstack_quant_models::factor::schema::factor_model_config_schema().map_err(to_js_err)?,
    )
}

/// JSON Schema of a serialized `CreditFactorModel`.
/// @returns JSON Schema document.
/// @throws Error - Throws with kind `validation` if the embedded schema is malformed (does not occur for a released build).
#[wasm_bindgen(js_name = creditFactorModelSchema)]
pub fn credit_factor_model_schema() -> Result<JsValue, JsValue> {
    to_js_value(
        finstack_quant_models::factor::schema::credit_factor_model_schema().map_err(to_js_err)?,
    )
}

/// JSON Schema of a serialized `CreditCalibrationConfig`.
/// @returns JSON Schema document.
/// @throws Error - Throws with kind `validation` if the embedded schema is malformed (does not occur for a released build).
#[wasm_bindgen(js_name = creditCalibrationConfigSchema)]
pub fn credit_calibration_config_schema() -> Result<JsValue, JsValue> {
    to_js_value(
        finstack_quant_models::factor::schema::credit_calibration_config_schema()
            .map_err(to_js_err)?,
    )
}

/// JSON Schema of a serialized `CreditCalibrationInputs`.
/// @returns JSON Schema document.
/// @throws Error - Throws with kind `validation` if the embedded schema is malformed (does not occur for a released build).
#[wasm_bindgen(js_name = creditCalibrationInputsSchema)]
pub fn credit_calibration_inputs_schema() -> Result<JsValue, JsValue> {
    to_js_value(
        finstack_quant_models::factor::schema::credit_calibration_inputs_schema()
            .map_err(to_js_err)?,
    )
}

/// JSON Schema of a serialized `FinancialModelSpec`.
/// @returns JSON Schema document.
/// @throws Error - Throws with kind `validation` if the schema cannot be built (does not occur for a released build).
#[wasm_bindgen(js_name = financialModelSpecSchema)]
pub fn financial_model_spec_schema() -> Result<JsValue, JsValue> {
    to_js_value(
        finstack_quant_statements::schema::financial_model_spec_schema().map_err(to_js_err)?,
    )
}

/// JSON Schema of a serialized `StatementResult`.
/// @returns JSON Schema document.
/// @throws Error - Throws with kind `validation` if the schema cannot be built (does not occur for a released build).
#[wasm_bindgen(js_name = statementResultSchema)]
pub fn statement_result_schema() -> Result<JsValue, JsValue> {
    to_js_value(finstack_quant_statements::schema::statement_result_schema().map_err(to_js_err)?)
}

/// JSON Schema of a serialized `NormalizationConfig`.
/// @returns JSON Schema document.
/// @throws Error - Throws with kind `validation` if the schema cannot be built (does not occur for a released build).
#[wasm_bindgen(js_name = normalizationConfigSchema)]
pub fn normalization_config_schema() -> Result<JsValue, JsValue> {
    to_js_value(
        finstack_quant_statements::schema::normalization_config_schema().map_err(to_js_err)?,
    )
}

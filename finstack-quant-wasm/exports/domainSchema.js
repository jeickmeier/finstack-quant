import * as wasm from '../pkg/finstack_quant_wasm.js';

// wasm-bindgen shares one flat export namespace and cannot export the same
// `index` / `get` / `validate` names per crate namespace without duplicating
// bindings for every domain. Currying the domain name here keeps one triple of
// Rust entry points while exposing `<domain>.schema.{index,get,validate}`.
export function domainSchema(domain) {
  return {
    /**
     * List every JSON Schema this crate publishes.
     *
     * Each row carries `path`, `$id`, `title`, `type_name` (the Rust root
     * type), `summary`, `bytes` and `kind` (`input` for documents you
     * author, `output` for documents the library emits, `component` for
     * shared definitions).
     * @returns `{ artifacts, schema_index_version }`, with one row per published schema.
     * @throws Error - Throws with kind `internal` if a schema cannot be rendered (does not occur for a released build).
     * @facadeType () => SchemaIndex
     */
    index: () => wasm.domainSchemaIndex(domain),
    /**
     * Fetch one published JSON Schema of this crate.
     * @param selector - Schema `$id`, registry path or trailing filename from `index()`, e.g. `"market_context_state.schema.json"`. Partial filenames do not match.
     * @param profile - `"canonical"` (the default) for the validation contract, or `"llm"` for a self-contained projection with cross-document references inlined, intended for structured-output generation.
     * @returns JSON Schema document.
     * @throws Error - Throws with kind `not_found` if no schema matches `selector`, kind `validation` if `profile` is unknown, and kind `invalid_type` if either argument is not a string.
     * @facadeType (selector: string, profile?: SchemaProfile) => Record<string, unknown>
     */
    get: (selector, profile) => wasm.domainSchemaGet(domain, selector, profile),
    /**
     * Validate a payload against one published JSON Schema of this crate.
     *
     * A failure inside a tagged union is reported at the offending field of
     * the branch the payload most nearly matches, not at the union.
     * @param selector - Schema `$id`, registry path or trailing filename from `index()`.
     * @param payload - Document to check, as a plain object/array or as JSON text. Pass JSON text for a scalar payload (`'"USD"'`, `'1.5'`).
     * @returns One `{ pointer, message }` row per failure, where `pointer` is the JSON Pointer of the offending value; empty when the payload is valid.
     * @throws Error - Throws with kind `not_found` if no schema matches `selector`, kind `validation` if `payload` is JSON text that does not parse, and kind `invalid_type` if `selector` is not a string or `payload` is neither JSON text nor a plain object/array.
     * @facadeType (selector: string, payload: string | object) => SchemaValidationFailure[]
     */
    validate: (selector, payload) => wasm.domainSchemaValidate(domain, selector, payload),
  };
}

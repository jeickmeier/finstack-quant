import * as wasm from '../../pkg/finstack_quant_wasm.js';

/**
 * Registry of the JSON Schemas one domain crate publishes.
 *
 * Schemas are rendered by this build, so they always describe the wire format it accepts.
 * The Python `finstack_quant.<crate>.schema` functions return the same documents as JSON text.
 * @example
 * ```typescript
 * import init, { analytics } from "finstack-quant-wasm";
 * await init();
 * const paths = analytics.schema.index().artifacts.map((row) => row.path);
 * const document = analytics.schema.get("correlation_result.schema.json");
 * const failures = analytics.schema.validate("correlation_result.schema.json", {});
 * console.log(paths.length, document.title, failures[0]?.message);
 * ```
 */
export const schema = {
  index: wasm.analyticsSchemaIndex,
  get: wasm.analyticsSchemaGet,
  validate: wasm.analyticsSchemaValidate,
};

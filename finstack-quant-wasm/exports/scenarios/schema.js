import * as wasm from '../../pkg/finstack_quant_wasm.js';

/**
 * Registry of the JSON Schemas one domain crate publishes.
 *
 * Schemas are rendered by this build, so they always describe the wire format it accepts.
 * The Python `finstack_quant.<crate>.schema` functions return the same documents as JSON text.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const paths = core.schema.index().artifacts.map((row) => row.path);
 * const document = core.schema.get("period_plan.schema.json");
 * const failures = core.schema.validate("period_plan.schema.json", {});
 * console.log(paths.length, document.title, failures[0]?.message);
 * ```
 */
export const schema = {
  index: wasm.scenariosSchemaIndex,
  get: wasm.scenariosSchemaGet,
  validate: wasm.scenariosSchemaValidate,
};

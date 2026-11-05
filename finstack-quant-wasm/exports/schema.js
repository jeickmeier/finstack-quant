import * as wasm from '../pkg/finstack_quant_wasm.js';

/**
 * Every JSON Schema the workspace publishes, across all domains.
 *
 * The per-crate `schema` namespaces list one crate each; this one merges them
 * and labels each `index()` row with its `domain`.
 * @example
 * ```typescript
 * import init, { schema } from "finstack-quant-wasm";
 * await init();
 * const failures = schema.validate("bond.schema.json", {});
 * console.log(schema.domains(), failures.map((failure) => failure.message));
 * ```
 */
export const schema = {
  index: wasm.schemaIndex,
  get: wasm.schemaGet,
  validate: wasm.schemaValidate,
  domains: wasm.schemaDomains,
};

import * as wasm from '../../pkg/finstack_quant_wasm.js';

/**
 * Registry of the canonical statements JSON Schemas.
 * @example
 * ```typescript
 * import init, { statements } from "finstack-quant-wasm";
 * await init();
 * const document = statements.schema.get("financial_model_spec.schema.json");
 * console.log(document.title);
 * ```
 */
export const schema = {
  index: wasm.statementsSchemaIndex,
  get: wasm.statementsSchemaGet,
  validate: wasm.statementsSchemaValidate,
};

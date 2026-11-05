import * as wasm from '../../../pkg/finstack_quant_wasm.js';

/**
 * Registry of the factor-model JSON Schemas.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const document = models.factor.schema.get("factor_model_config.schema.json");
 * console.log(document.title);
 * ```
 */
export const schema = {
  index: wasm.factorSchemaIndex,
  get: wasm.factorSchemaGet,
  validate: wasm.factorSchemaValidate,
};

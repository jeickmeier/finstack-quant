import * as wasm from '../../pkg/finstack_quant_wasm.js';

/**
 * Registry of the JSON Schemas the cashflows crate publishes, plus the embedded resolver resources.
 * @example
 * ```typescript
 * import init, { cashflows } from "finstack-quant-wasm";
 * await init();
 * const resources = cashflows.schema.resources();
 * console.log(Object.keys(resources).length);
 * ```
 */
export const schema = {
  index: wasm.cashflowsSchemaIndex,
  get: wasm.cashflowsSchemaGet,
  validate: wasm.cashflowsSchemaValidate,
  resources: wasm.cashflowsSchemaResources,
};

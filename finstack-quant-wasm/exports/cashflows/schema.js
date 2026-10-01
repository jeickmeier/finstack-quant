import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const schema = {
  index: wasm.cashflowsSchemaIndex,
  get: wasm.cashflowsSchemaGet,
  validate: wasm.cashflowsSchemaValidate,
  resources: wasm.cashflowsSchemaResources,
};

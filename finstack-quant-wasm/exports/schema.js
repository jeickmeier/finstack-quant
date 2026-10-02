import * as wasm from '../pkg/finstack_quant_wasm.js';

export const schema = {
  index: wasm.schemaIndex,
  get: wasm.schemaGet,
  validate: wasm.schemaValidate,
  domains: wasm.schemaDomains,
};
